import { beforeEach, describe, expect, it, vi } from 'vitest'
import { applyFastPlanEvent, createFastPlanRuntimeState, normalizeTokenUsage } from './events'
import { createFastPlanExecutionController } from './executionController'
import { hydrateFastPlanExecutionState } from './restore'
import { adaptFastPlanExecutionHistory } from './chatAdapter'

const tree = { id: 'tree-1', root: { children: [{ id: 'phase-1', name: 'Phase', children: [
  { id: 'node-1', name: 'Collect', node_type: { type: 'LlmTask', config: {} } },
  { id: 'node-2', name: 'Write', node_type: { type: 'LlmTask', config: {} } },
] }] }, metadata: {} }

class MockSocket {
  static OPEN = 1; static CONNECTING = 0; static instances: MockSocket[] = []
  readyState = MockSocket.CONNECTING; sent: any[] = []; onopen: any; onmessage: any; onerror: any; onclose: any
  constructor(public url: string) { MockSocket.instances.push(this) }
  open() { this.readyState = MockSocket.OPEN; this.onopen?.({}) }
  send(value: string) { this.sent.push(JSON.parse(value)) }
  close() { this.readyState = 3 }
}
class MockEvents {
  listeners = new Map<string, Function[]>(); constructor(public url: string) {}
  addEventListener(name: string, fn: any) { this.listeners.set(name, [...(this.listeners.get(name) || []), fn]) }
  close() {}
}

describe('FastPlan event contract', () => {
  it('archives all rendering state before the next prompt and routes late results to the previous turn', async () => {
    const runtime = createFastPlanExecutionController({ sessionId: 's', WebSocket: MockSocket as any,
      EventSource: MockEvents as any, fetch: async () => new Response('{}'), persistUserMessages: false })
    runtime.connect('s')
    MockSocket.instances[MockSocket.instances.length - 1].open()
    runtime.sendChat('first', { requestId: 'r1' })
    const firstId = runtime.state.activeUserMessageId
    Object.assign(runtime.state, { plannedTree: structuredClone(tree), currentTreeId: 'tree-1', treeCompleted: true,
      executionStatus: 'completed', isPlanning: false, presentation: { plan_summary: 'first plan' },
      routeFlow: [{ stage: 'intent', status: 'done', summary: 'first', payload: {} }],
      thinkingSteps: [{ label: 'first thinking' }], confirmedHistory: [{ id: 'review' }] })
    runtime.sendChat('second', { requestId: 'r2' })
    expect(runtime.state.executionHistory).toHaveLength(1)
    expect(runtime.state.executionHistory[0]).toMatchObject({ anchorMessageId: firstId, lastPrompt: 'first',
      presentation: { plan_summary: 'first plan' }, thinkingSteps: [{ label: 'first thinking' }] })
    expect(runtime.state.confirmedHistory).toEqual([])
    await runtime.processEvent({ type: 'final_answer', tree_id: 'tree-1', content: 'late first result' })
    expect(runtime.state.lastPrompt).toBe('second')
    expect(runtime.state.finalResult).toBeNull()
    expect(runtime.state.executionHistory[0].finalResult.content).toBe('late first result')
    expect(runtime.state.messages.map(message => message.content)).toEqual(['first', 'late first result', 'second'])
    runtime.disconnect()
  })

  it.each([true, false])('persists tree_completed success=%s independently of lifecycle and final_answer', async (success) => {
    const request = vi.fn(async () => new Response('{}'))
    const runtime = createFastPlanExecutionController({ sessionId: 'session-1', autoConnect: false, fetch: request })
    runtime.state.plannedTree = structuredClone(tree)
    await runtime.processEvent({ type: 'tree_started', tree_id: 'tree-1' })
    await runtime.processEvent({ type: 'tree_completed', tree_id: 'tree-1', success })
    await runtime.processEvent({ type: 'final_answer', tree_id: 'tree-1', content: 'summary, not a success signal' })
    const expected = success ? 'completed' : 'failed'
    expect(runtime.state.treeCompleted).toBe(true)
    expect(runtime.state.executionStatus).toBe(expected)
    expect(runtime.planStatus.value).toBe(expected)
    expect(runtime.state.executionHistory[0].status).toBe(expected)
    const snapshots = (request.mock.calls as any[][]).filter(([url]) => String(url).endsWith('/executions'))
    expect(snapshots).toHaveLength(3)
    expect(JSON.parse(String((snapshots[0] as any)[1].body)).status).toBe('running')
    for (const call of snapshots.slice(1)) expect(JSON.parse(String((call as any)[1].body))).toMatchObject({
      status: expected, state_json: { __runtime: { executionStatus: expected } },
    })
  })

  it('restores legacy false-success snapshots and recognizes all terminal states', () => {
    const restored = hydrateFastPlanExecutionState({ tree_id: 'tree-1', tree, status: 'completed',
      node_states: { 'node-1': { status: 'failed' }, 'node-2': { status: 'pending' } } })
    expect(restored).toMatchObject({ status: 'failed', executionStatus: 'failed', treeCompleted: true, isPlanning: false })
    for (const status of ['failed', 'cancelled', 'completed']) {
      expect(hydrateFastPlanExecutionState({ status })).toMatchObject({ executionStatus: status, treeCompleted: true })
    }
    // A tolerated failed node does not override an explicit successful tree outcome.
    expect(hydrateFastPlanExecutionState({ status: 'completed', node_states: {
      'node-1': { status: 'failed' }, __runtime: { executionStatus: 'completed' },
    } }).status).toBe('completed')
  })

  it('persists failed completion for a cached background session', async () => {
    const request = vi.fn(async (url: any) => new Response(String(url).endsWith('/execution-state')
      ? '{"has_state":false}' : '{"messages":[],"executions":[]}'))
    const runtime = createFastPlanExecutionController({ sessionId: 'session-1', autoConnect: false, fetch: request })
    runtime.state.plannedTree = structuredClone(tree)
    await runtime.processEvent({ type: 'tree_started', tree_id: 'tree-1' })
    await runtime.loadSession('session-2', { connect: false })
    await runtime.processEvent({ type: 'tree_completed', tree_id: 'tree-1', success: false }, 'session-1')
    await runtime.processEvent({ type: 'final_answer', tree_id: 'tree-1', content: 'error summary' }, 'session-1')
    const snapshots = (request.mock.calls as any[][]).filter(([url]) => String(url).endsWith('/session-1/executions'))
    expect(snapshots).toHaveLength(3)
    expect(JSON.parse(snapshots[0][1].body).status).toBe('running')
    for (const call of snapshots.slice(1)) expect(JSON.parse(call[1].body).status).toBe('failed')
    expect(runtime.state.executionStatus).toBeNull()
  })

  it('clears the previous terminal outcome when a new execution starts', () => {
    const state = createFastPlanRuntimeState()
    applyFastPlanEvent(state, { type: 'tree_completed', success: false })
    applyFastPlanEvent(state, { type: 'tree_started', tree_id: 'next' })
    expect(state.executionStatus).toBeNull()
    expect(state.treeCompleted).toBe(false)
  })

  it('normalizes split token usage from common event and output shapes', () => {
    expect(normalizeTokenUsage({ usage: { prompt_tokens: 2300, completion_tokens: 7100 } }))
      .toEqual({ input: 2300, output: 7100 })
    expect(normalizeTokenUsage({ agent_metrics: { usage: { input_tokens: 31 } } }))
      .toEqual({ input: 31 })
  })
  it.each([['auto', false, true], ['manual', true, false], ['step', false, true]] as const)(
    'handles %s plan mode', (mode, preview, autoConfirm) => {
      const state = createFastPlanRuntimeState(mode)
      applyFastPlanEvent(state, { type: 'planning_start' })
      applyFastPlanEvent(state, { type: 'plan_generated', tree })
      expect(state.isPreviewing).toBe(preview); expect(state.pendingAutoConfirm).toBe(autoConfirm)
    })

  it.each([['auto', false, true], ['manual', true, false], ['step', true, false]] as const)(
    'supports strict confirmation semantics for %s mode', (mode, preview, autoConfirm) => {
      const state = createFastPlanRuntimeState(mode, 'strict')
      applyFastPlanEvent(state, { type: 'planning_start' })
      applyFastPlanEvent(state, { type: 'plan_generated', tree })
      expect(state.isPreviewing).toBe(preview)
      expect(state.pendingAutoConfirm).toBe(autoConfirm)
    })

  it('pauses completed nodes only for step mode under the strict policy', () => {
    const manual = createFastPlanRuntimeState('manual', 'strict')
    manual.plannedTree = structuredClone(tree)
    applyFastPlanEvent(manual, { type: 'tree_started', tree_id: 'tree-1' })
    applyFastPlanEvent(manual, { type: 'node_completed', tree_id: 'tree-1', node_id: 'node-1', output: 'done' })
    expect(manual.isPaused).toBe(false)

    const step = createFastPlanRuntimeState('step', 'strict')
    step.plannedTree = structuredClone(tree)
    applyFastPlanEvent(step, { type: 'tree_started', tree_id: 'tree-1' })
    applyFastPlanEvent(step, { type: 'node_completed', tree_id: 'tree-1', node_id: 'node-1', output: 'done' })
    expect(step).toMatchObject({ isPaused: true, stepConfirmNodeId: 'node-1', lastCompletedNodeId: 'node-1' })
  })

  // ★ A5 回归（2026-08-23）：节点降级 → 全局汇总 → final_answer 结果顶部透出
  it('collects degraded reasons and surfaces them on the final result', () => {
    const state = createFastPlanRuntimeState('auto'); state.plannedTree = structuredClone(tree)
    applyFastPlanEvent(state, { type: 'tree_started', tree_id: 'tree-1' })
    applyFastPlanEvent(state, { type: 'node_completed', node_id: 'node-1', success: true, duration_ms: 5,
      output: '{"ok":true}', degraded: ['检索工具 `jmis_search` 返回零结果（无匹配）——当前结论无检索数据支撑'] })
    expect(state.executionState['node-1'].degraded).toHaveLength(1)
    expect(state.degradedSummary).toHaveLength(1)
    // 重复原因去重
    applyFastPlanEvent(state, { type: 'node_completed', node_id: 'node-2', success: true, duration_ms: 5,
      output: '{"ok":true}', degraded: ['检索工具 `jmis_search` 返回零结果（无匹配）——当前结论无检索数据支撑'] })
    expect(state.degradedSummary).toHaveLength(1)
    // 非降级节点不产生汇总
    applyFastPlanEvent(state, { type: 'node_completed', node_id: 'node-1', success: true, duration_ms: 5, output: '{"ok":true}' })
    expect(state.degradedSummary).toHaveLength(1)
    // final_answer 时 finalResult 携带 degraded
    applyFastPlanEvent(state, { type: 'final_answer', node_id: 'node-2', content: '结论', final_output: '结论' })
    expect(state.finalResult.degraded).toEqual(state.degradedSummary)
    // 新树开始重置
    applyFastPlanEvent(state, { type: 'tree_started', tree_id: 'tree-2' })
    expect(state.degradedSummary).toHaveLength(0)
  })

  it('assembles streaming nodes, logs, failure, retry-ready and skip states', () => {
    const state = createFastPlanRuntimeState('auto'); state.plannedTree = structuredClone(tree)
    applyFastPlanEvent(state, { type: 'tree_started', tree_id: 'tree-1' })
    applyFastPlanEvent(state, { type: 'node_started', tree_id: 'tree-1', node_id: 'node-1' })
    applyFastPlanEvent(state, { type: 'streaming_token', node_id: 'node-1', token: 'hello ' })
    applyFastPlanEvent(state, { type: 'agent_task_progress', node_id: 'node-1', phase: 'reasoning', content: 'why' })
    applyFastPlanEvent(state, { type: 'node_failed', node_id: 'node-1', error: 'boom' })
    applyFastPlanEvent(state, { type: 'node_skipped', node_id: 'node-2' })
    expect(state.executionState['node-1']).toMatchObject({ status: 'failed', streamContent: 'hello ', error: 'boom' })
    expect(state.error).toBeNull()
    expect(state.executionState['node-1'].logs[0].content).toBe('why')
    expect(state.executionState['node-2'].status).toBe('skipped')
  })

  it('clears the previous active execution when a new planning turn starts', () => {
    const state = createFastPlanRuntimeState('auto')
    Object.assign(state, {
      plannedTree: structuredClone(tree), treeCompleted: true, isPaused: true,
      executionState: { 'node-1': { status: 'completed' } }, taskBubbles: ['node-1'],
      finalResult: { content: 'old result' }, resultFiles: [{ name: 'old.docx' }],
      humanRequest: { node_id: 'node-1' }, humanRequestQueue: [{ node_id: 'node-1' }],
    })
    applyFastPlanEvent(state, { type: 'planning_start' })
    expect(state).toMatchObject({ plannedTree: null, executionState: {}, taskBubbles: [], treeCompleted: false,
      isPlanning: true, isPaused: false, finalResult: null, resultFiles: [], humanRequest: null, humanRequestQueue: [] })
  })

  it('finishes the live LLM planning heartbeat when the plan stage completes', () => {
    const state = createFastPlanRuntimeState('auto')
    applyFastPlanEvent(state, { type: 'planning_start' })
    applyFastPlanEvent(state, { type: 'thinking_update', phase: 'planning', data: {
      kind: 'plan_stage', stage: 'llm_progress', status: 'running', content: '正在编排执行步骤…', elapsed_ms: 1500,
    } })
    applyFastPlanEvent(state, { type: 'thinking_update', phase: 'planning', data: {
      kind: 'plan_stage', stage: 'plan', status: 'done', content: '规划完成', duration_ms: 2400,
    } })
    expect(state.validationFlow.find(row => row.stage === 'llm_progress')).toMatchObject({
      status: 'done', text: '执行步骤编排完成', duration_ms: 2400,
    })
  })

  it('renders route stages from the first event and keeps them separate from planner validation', () => {
    const state = createFastPlanRuntimeState('auto')
    applyFastPlanEvent(state, { type: 'planning_start' })
    applyFastPlanEvent(state, { type: 'route_stage_update', stage: 'intent', status: 'running', summary: '意图理解…', payload: {} })
    expect(state.routeFlow).toHaveLength(1)
    applyFastPlanEvent(state, { type: 'route_stage_update', stage: 'intent', status: 'done', summary: '意图理解完成', payload: { intent_type: 'data_query' } })
    applyFastPlanEvent(state, { type: 'thinking_update', phase: 'planning', tree_id: 'planner', data: {
      kind: 'plan_stage', stage: 'intent', status: 'done', content: '规划器意图已确认',
    } })
    expect(state.routeFlow).toHaveLength(1)
    expect(state.routeFlow[0]).toMatchObject({ status: 'done', payload: { intent_type: 'data_query' } })
    expect(state.validationFlow[0]).toMatchObject({ stage: 'intent', text: '规划器意图已确认' })
  })

  it('replays the reference planning event sequence through plan generation', () => {
    // Keep the protocol regression self-contained. The original captured log
    // was a local workspace artifact and is not part of the package source.
    const events = [
      { type: 'route_stage_update', stage: 'intent', status: 'running', summary: 'Analyzing intent', payload: {} },
      { type: 'route_stage_update', stage: 'intent', status: 'done', summary: 'Intent complete', payload: { intent_type: 'data_query' } },
      { type: 'route_stage_update', stage: 'retrieval', status: 'done', summary: 'Retrieval complete', payload: {} },
      { type: 'route_stage_update', stage: 'routing', status: 'done', summary: 'Routing complete', payload: { decision: 'plan' } },
      { type: 'thinking_update', phase: 'planning', data: { kind: 'plan_stage', stage: 'llm_progress', status: 'running', content: 'Building plan' } },
      ...Array.from({ length: 4 }, (_, step) => ({
        type: 'thinking_update', phase: 'planning', data: { kind: 'thinking_step', step, label: `Step ${step + 1}`, content: `Thought ${step + 1}` },
      })),
      { type: 'plan_generated', tree },
    ]
    const state = createFastPlanRuntimeState('auto')
    applyFastPlanEvent(state, { type: 'planning_start' })
    applyFastPlanEvent(state, events[0])
    expect(state.routeFlow).toHaveLength(1)
    events.slice(1).forEach((event: any) => applyFastPlanEvent(state, event))
    expect(state.routeFlow.map(row => row.stage)).toEqual(['intent', 'retrieval', 'routing'])
    expect(state.routeFlow.every(row => row.status === 'done')).toBe(true)
    expect(state.validationFlow.filter(row => row.stage === 'llm_progress')).toHaveLength(1)
    expect(state.thinkingSteps.length).toBeGreaterThanOrEqual(4)
    expect(state.plannedTree).toBeTruthy()
  })

  it('hydrates a persisted process trace', () => {
    const restored = hydrateFastPlanExecutionState({ tree_id: 'tree-1', tree, status: 'planned', node_states: {
      __processTrace: { route_stages: [{ stage: 'routing', status: 'done', summary: 'Plan', payload: { decision: 'plan' } }],
        validation_flow: [{ stage: 'plan', status: 'done', content: '规划完成' }], thinking_steps: [{ idx: 0, label: '需求理解', content: '三地天气' }] },
    } })
    expect(restored.routeFlow[0].payload.decision).toBe('plan')
    expect(restored.validationFlow[0].stage).toBe('plan')
    expect(restored.thinkingSteps[0].content).toBe('三地天气')
  })

  it('preserves completed-node confidence and duration from events and output envelopes', () => {
    const state = createFastPlanRuntimeState('auto'); state.plannedTree = structuredClone(tree)
    applyFastPlanEvent(state, { type: 'tree_started', tree_id: 'tree-1' })
    applyFastPlanEvent(state, { type: 'node_completed', node_id: 'node-1', duration_ms: 920,
      output: JSON.stringify({ content: 'done', agent_metrics: { confidence: 87, duration_ms: 1100 } }) })
    expect(state.executionState['node-1']).toMatchObject({ status: 'completed', confidence: 87, durationMs: 920 })

    applyFastPlanEvent(state, { type: 'node_completed', node_id: 'node-2', data: {
      confidence: 73, elapsed_ms: 640, output: 'second result',
    } })
    expect(state.executionState['node-2']).toMatchObject({ confidence: 73, durationMs: 640, output: 'second result' })
  })

  it('keeps partial input usage while streaming and replaces it with final usage', () => {
    const state = createFastPlanRuntimeState('auto'); state.plannedTree = structuredClone(tree)
    applyFastPlanEvent(state, { type: 'tree_started', tree_id: 'tree-1' })
    applyFastPlanEvent(state, { type: 'node_started', node_id: 'node-1', usage: { input_tokens: 120 } })
    applyFastPlanEvent(state, { type: 'streaming_token', node_id: 'node-1', token: '流式内容' })
    expect(state.executionState['node-1'].usage).toEqual({ input: 120 })
    applyFastPlanEvent(state, { type: 'node_completed', node_id: 'node-1', output: {
      content: '完成', usage: { input_tokens: 120, output_tokens: 48 },
    } })
    expect(state.executionState['node-1'].usage).toEqual({ input: 120, output: 48 })
  })

  it('keeps a duplicate start timestamp but resets timing when a node retries', () => {
    const state = createFastPlanRuntimeState('auto'); state.plannedTree = structuredClone(tree)
    applyFastPlanEvent(state, { type: 'tree_started', tree_id: 'tree-1' })
    applyFastPlanEvent(state, { type: 'node_started', node_id: 'node-1', ts: 1_787_428_800_000 })
    expect(state.executionState['node-1'].startTime).toBe(1_787_428_800_000)
    applyFastPlanEvent(state, { type: 'node_started', node_id: 'node-1', ts: 1_787_428_801_000 })
    expect(state.executionState['node-1'].startTime).toBe(1_787_428_800_000)
    applyFastPlanEvent(state, { type: 'node_completed', node_id: 'node-1', duration_ms: 900, output: 'done' })
    applyFastPlanEvent(state, { type: 'node_started', node_id: 'node-1', ts: 1_787_428_802_000 })
    expect(state.executionState['node-1']).toMatchObject({ status: 'running', startTime: 1_787_428_802_000, endTime: null, durationMs: null })
  })

  it('tracks pause, resume and a de-duplicated human queue', () => {
    const state = createFastPlanRuntimeState('manual')
    const request = { type: 'waiting_for_human', tree_id: 'tree-1', node_id: 'node-1', prompt: 'Continue?' }
    applyFastPlanEvent(state, request); applyFastPlanEvent(state, request)
    expect(state.humanRequestQueue).toHaveLength(1); expect(state.isPaused).toBe(true)
    applyFastPlanEvent(state, { type: 'tree_resumed' })
    expect(state.isPaused).toBe(false)
  })

  it('assembles final sources and downloadable files', () => {
    const state = createFastPlanRuntimeState()
    state.messages.push({ id: 'user-1', role: 'user', content: 'Create report', timestamp: new Date() })
    applyFastPlanEvent(state, { type: 'final_answer', tree_id: 'tree-1', content: 'Completed with a useful result.',
      final_sources: ['node-1'], final_output: { content: 'Completed with a useful result.', download: { files: [{ name: 'report.docx', download_url: '/api/download/report.docx' }] } } })
    expect(state.finalResult.finalSources).toEqual(['node-1'])
    expect(state.resultFiles[0]).toMatchObject({ name: 'report.docx', url: '/api/download/report.docx' })
    expect(state.messages[state.messages.length - 1]).toMatchObject({ role: 'assistant', content: 'Completed with a useful result.', contentKind: 'final_answer', isFinalAnswer: true })
    applyFastPlanEvent(state, { type: 'assistant_message', message_id: 'assistant-1', content: 'Completed with a useful result.' })
    expect(state.messages.filter(message => message.role === 'assistant')).toHaveLength(1)
    expect(state.messages[state.messages.length - 1]?.id).toBe('assistant-1')
  })

  it('turns generated-document boilerplate into a localized final answer', () => {
    const state = createFastPlanRuntimeState()
    state.messages.push({ id: 'user-1', role: 'user', content: 'Create report', timestamp: new Date() })
    applyFastPlanEvent(state, { type: 'final_answer', content: 'Task completed. 1 file(s) generated. Download: /api/download/run/output.docx',
      final_output: { download: { files: [{ name: 'output.docx', size: 7102, download_url: '/api/download/run/output.docx' }] } } })
    expect(state.finalResult.content).toBe('文档已生成，可在下方下载。')
    expect(state.messages[state.messages.length - 1]?.content).toBe('文档已生成，可在下方下载。')
    expect(state.resultFiles[0]).toMatchObject({ name: 'output.docx', size: 7102 })
  })

  it('enriches an earlier URL-only result with document size and MIME metadata', () => {
    const state = createFastPlanRuntimeState()
    state.resultFiles.push({ name: 'output.docx', url: '/api/download/run/output.docx' })
    applyFastPlanEvent(state, { type: 'final_answer', content: 'Task completed. 1 file(s) generated. Download: /api/download/run/output.docx',
      final_output: { result: { download: { files: [{
        name: 'output.docx', size: 7102,
        mime_type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
        download_url: '/api/download/run/output.docx',
      }] } } } })
    expect(state.resultFiles).toHaveLength(1)
    expect(state.resultFiles[0]).toMatchObject({ size: 7102,
      mime_type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document' })
  })

  it('upserts a final answer into an assistant message that arrived first', () => {
    const state = createFastPlanRuntimeState()
    state.messages.push({ id: 'user-1', role: 'user', content: 'Question', timestamp: new Date() })
    applyFastPlanEvent(state, { type: 'assistant_message', message_id: 'assistant-1', content: 'Draft summary' })
    applyFastPlanEvent(state, { type: 'final_answer', tree_id: 'tree-1', content: 'Final summary' })
    expect(state.messages.filter(message => message.role === 'assistant')).toHaveLength(1)
    expect(state.messages[state.messages.length - 1]).toMatchObject({ id: 'assistant-1', content: 'Final summary', contentKind: 'final_answer', isFinalAnswer: true })
  })

  it('restores historical execution snapshots', () => {
    const restored = hydrateFastPlanExecutionState({ tree_id: 'tree-1', tree, status: 'paused', node_states: {
      'node-1': { status: 'completed', output: 'done' }, __finalResult: { content: 'summary' }, __resultFiles: [{ name: 'a.txt', url: '/a.txt' }],
    } })
    expect(restored.currentTreeId).toBe('tree-1'); expect(restored.executionState['node-1'].status).toBe('completed')
    expect(restored.finalResult.content).toBe('summary')
  })
})

describe('FastPlan execution controller commands', () => {
  beforeEach(() => { MockSocket.instances = [] })

  it('finishes the local preview state immediately when a manual plan is cancelled', () => {
    const runtime = createFastPlanExecutionController({ sessionId: 'session-1', executionMode: 'manual',
      WebSocket: MockSocket as any, EventSource: MockEvents as any, fetch: vi.fn(async () => new Response('{}')) })
    runtime.connect(); const ws = MockSocket.instances[0]; ws.open()
    Object.assign(runtime.state, { activeUserMessageId: 'user-1', plannedTree: structuredClone(tree),
      currentTreeId: 'tree-1', isPlanning: false, isPreviewing: true, treeCompleted: false, executionStatus: null })
    const changes: any[] = []
    runtime.on('state-change', payload => changes.push(payload))

    expect(runtime.cancelPlan()).toBe(true)

    expect(ws.sent.at(-1)).toEqual({ type: 'cancel_plan' })
    expect(runtime.state).toMatchObject({
      isPlanning: false, isPreviewing: false, isPaused: false,
      pendingAutoConfirm: false, treeCompleted: true, executionStatus: 'cancelled',
    })
    expect(runtime.state.executionHistory.at(-1)).toMatchObject({ anchorMessageId: 'user-1', status: 'cancelled' })
    expect(runtime.planStatus.value).toBe('cancelled')
    expect(changes.at(-1)?.event).toEqual({ type: 'cancel_plan' })
    runtime.disconnect()
  })

  it('does not issue a second REST write when the WebSocket backend owns user persistence', () => {
    const request = vi.fn(async () => new Response('{}', { status: 200 }))
    const runtime = createFastPlanExecutionController({
      sessionId: 'session-1',
      WebSocket: MockSocket as any,
      EventSource: MockEvents as any,
      fetch: request,
      persistUserMessages: false,
    })
    runtime.connect()
    const ws = MockSocket.instances[0]
    ws.open()

    runtime.sendChat('save once')

    expect(ws.sent).toHaveLength(1)
    expect(ws.sent[0]).toMatchObject({ type: 'chat', content: 'save once' })
    expect(request).not.toHaveBeenCalled()
    runtime.disconnect()
  })

  it('sends the complete command contract and persists a chat turn', async () => {
    const request = vi.fn(async () => new Response('{}', { status: 200 }))
    const runtime = createFastPlanExecutionController({ sessionId: 'session-1', apiBase: 'http://api', wsBase: 'ws://api',
      WebSocket: MockSocket as any, EventSource: MockEvents as any, fetch: request,
      model: { id: 'model-row-id', name: 'Qwen Display', model: 'qwen', baseUrl: 'http://model' } })
    runtime.connect(); const ws = MockSocket.instances[0]; ws.open()
    runtime.state.plannedTree = structuredClone(tree); runtime.state.currentTreeId = 'tree-1'
    await runtime.sendChat('build it', { attachments: [{ filename: 'brief.txt', path: '/uploads/brief.txt' }],
      thinking: true, userOverride: 'direct', metadata: { source: 'host' } })
    expect(runtime.state).toMatchObject({ isPlanning: true, plannedTree: null, lastPrompt: 'build it' })
    runtime.state.plannedTree = structuredClone(tree); runtime.state.currentTreeId = 'tree-1'
    await runtime.confirmPlan(runtime.state.plannedTree); runtime.pauseExecution(); runtime.resumeExecution()
    runtime.skipNode('node-2'); runtime.updateNodeOutput('node-1', 'edited'); runtime.updateNextNodeConfig('node-2', { prompt: 'next' }); runtime.cancelPlan(); runtime.cancelExecution()
    expect(ws.sent.map(item => item.type)).toEqual(['chat', 'confirm_plan', 'pause_tree', 'resume_tree', 'skip_node', 'edit_node', 'edit_node', 'cancel_plan', 'cancel'])
    expect(ws.sent[0]).toMatchObject({
      model: 'qwen',
      base_url: 'http://model',
      model_id: 'model-row-id',
      model_name: 'Qwen Display',
    })
    expect(request).toHaveBeenCalledWith('http://api/api/sessions/session-1/messages', expect.objectContaining({ method: 'POST' }))
    const saveCall = (request as any).mock.calls.find((call: any[]) => String(call[0]).endsWith('/messages'))!
    const savedMessage = JSON.parse(String(saveCall[1]?.body))
    expect(savedMessage).toMatchObject({ role: 'user', content: 'build it' })
    expect(typeof savedMessage.metadata).toBe('string')
    expect(ws.sent[0]).toMatchObject({ thinking: true, user_override: 'direct', metadata: { source: 'host' } })
    expect(JSON.parse(savedMessage.metadata)).toEqual({ attachments: [{ filename: 'brief.txt', path: '/uploads/brief.txt' }],
      thinking: true, userOverride: 'direct', source: 'host' })
  })

  it('blocks missing attachment slots and injects provided files', async () => {
    const runtime = createFastPlanExecutionController({ sessionId: 'session-1', WebSocket: MockSocket as any,
      EventSource: MockEvents as any, fetch: vi.fn(async () => new Response('{}')) })
    runtime.connect(); MockSocket.instances[0].open()
    const fileTree: any = structuredClone(tree)
    fileTree.metadata.input_slots = [{ node_id: 'node-1', arg_key: 'file_path', label: 'Brief', accept: ['txt'], required: true }]
    await expect(runtime.confirmPlan(fileTree)).rejects.toMatchObject({ code: 'FASTPLAN_INPUT_REQUIRED' })
    await runtime.confirmPlan(fileTree, [{ filename: 'brief.txt', abs_path: '/uploads/brief.txt' }])
    const sent = MockSocket.instances[0].sent
    expect(sent[sent.length - 1].type).toBe('confirm_plan')
  })

  it('handles retry, human response, template execution and template saving', async () => {
    const request = vi.fn(async () => new Response('{}', { status: 200 }))
    const runtime = createFastPlanExecutionController({ sessionId: 'session-1', executionMode: 'manual', WebSocket: MockSocket as any,
      EventSource: MockEvents as any, fetch: request })
    runtime.connect(); MockSocket.instances[0].open(); runtime.state.currentTreeId = 'tree-1'; runtime.state.plannedTree = structuredClone(tree)
    runtime.state.executionState['node-1'] = { id: 'node-1', status: 'failed', error: 'boom' }
    await runtime.retryNode('node-1')
    runtime.state.humanRequestQueue = [{ tree_id: 'tree-1', node_id: 'node-1', prompt: '请提供地区',
      interaction: { kind: 'input', fields: [{ key: 'region', label: '地区' }] } }]
    const messageCount = runtime.state.messages.length
    await runtime.respondHumanCheckpoint({ action: 'submit', values: { region: '浙江省' } })
    expect(runtime.state.executionState['node-1'].status).toBe('running')
    expect(runtime.state.confirmedHistory[0]).toMatchObject({
      kind: 'checkpoint_input', title: '已补充执行信息', detail: '地区：浙江省', values: { region: '浙江省' },
    })
    await runtime.executeTemplate({ tree_json: tree })
    await runtime.confirmPlan(runtime.state.plannedTree)
    await runtime.saveAsTemplate('Reusable', 'contract test')
    expect(runtime.state.executionState).toEqual({})
    expect(runtime.state.humanRequestQueue).toHaveLength(0)
    expect(runtime.state.messages).toHaveLength(messageCount)
    expect(runtime.state.executionHistory.at(-1)?.confirmedHistory[0]).toMatchObject({
      kind: 'checkpoint_input', title: '已补充执行信息', detail: '地区：浙江省', values: { region: '浙江省' },
    })
    expect(runtime.state.isPreviewing).toBe(false)
    expect(runtime.state.isPlanning).toBe(true)
    expect((request as any).mock.calls.map((call: any[]) => String(call[0]))).toEqual(expect.arrayContaining([
      '/api/retry_task', '/api/checkpoint', '/api/execute_tree?session_id=session-1', '/api/templates',
    ]))
  })

  it('deduplicates matching WS/SSE node events and emits completion once after final answer', async () => {
    vi.useFakeTimers()
    const request = vi.fn(async () => new Response('{}', { status: 200 }))
    const completed = vi.fn()
    const runtime = createFastPlanExecutionController({ sessionId: 'session-1', WebSocket: MockSocket as any,
      EventSource: MockEvents as any, fetch: request, onEvent: (name, payload) => { if (name === 'execution-completed') completed(payload) } })
    runtime.state.plannedTree = structuredClone(tree)
    await runtime.processEvent({ type: 'tree_started', tree_id: 'tree-1' }, 'session-1', 'ws')
    const started = { type: 'node_started', tree_id: 'tree-1', node_id: 'node-1' }
    await runtime.processEvent(started, 'session-1', 'ws'); await runtime.processEvent(started, 'session-1', 'sse')
    const done = { type: 'node_completed', tree_id: 'tree-1', node_id: 'node-1', output: 'done' }
    await runtime.processEvent(done, 'session-1', 'ws'); await runtime.processEvent(done, 'session-1', 'sse')
    await runtime.processEvent({ type: 'tree_completed', tree_id: 'tree-1', success: true }, 'session-1', 'ws')
    await runtime.processEvent({ type: 'final_answer', tree_id: 'tree-1', content: 'A complete final answer.' }, 'session-1', 'ws')
    await vi.runAllTimersAsync()
    expect(runtime.state.taskBubbles.filter(id => id === 'node-1')).toHaveLength(1)
    expect(completed).toHaveBeenCalledTimes(1)
    vi.useRealTimers()
  })

  it('automatically responds to human checkpoints in auto mode', async () => {
    const request = vi.fn(async () => new Response('{}', { status: 200 }))
    const runtime = createFastPlanExecutionController({ sessionId: 'session-1', executionMode: 'auto', fetch: request,
      WebSocket: MockSocket as any, EventSource: MockEvents as any })
    runtime.state.currentTreeId = 'tree-1'
    await runtime.processEvent({ type: 'waiting_for_human', tree_id: 'tree-1', node_id: 'node-1', prompt: 'Continue?' })
    expect(request).toHaveBeenCalledWith('/api/checkpoint', expect.objectContaining({ method: 'POST' }))
    expect(runtime.state.humanRequestQueue).toHaveLength(0)
    expect(runtime.state.isPaused).toBe(false)
  })

  it('does not auto-submit a structured input checkpoint in auto mode', async () => {
    const request = vi.fn(async () => new Response('{}', { status: 200 }))
    const runtime = createFastPlanExecutionController({ sessionId: 'session-1', executionMode: 'auto', fetch: request,
      WebSocket: MockSocket as any, EventSource: MockEvents as any })
    runtime.state.currentTreeId = 'tree-1'
    await runtime.processEvent({ type: 'waiting_for_human', tree_id: 'tree-1', node_id: 'node-1',
      prompt: '请提供地区', interaction: { kind: 'input', fields: [{ key: 'region', label: '地区', required: true }] } })
    expect(request.mock.calls.some(([url]) => String(url) === '/api/checkpoint')).toBe(false)
    expect(request).toHaveBeenCalledWith('/api/sessions/session-1/executions', expect.objectContaining({ method: 'POST' }))
    expect(runtime.state.humanRequestQueue).toHaveLength(1)
    expect(runtime.state.isPaused).toBe(true)
  })

  it('waits for plan and node confirmation in strict step mode', async () => {
    const runtime = createFastPlanExecutionController({ sessionId: 'session-1', executionMode: 'step', confirmationPolicy: 'strict',
      WebSocket: MockSocket as any, EventSource: MockEvents as any, fetch: vi.fn(async () => new Response('{}')) })
    runtime.connect(); const ws = MockSocket.instances[0]; ws.open()
    await runtime.processEvent({ type: 'plan_generated', tree })
    await Promise.resolve()
    expect(runtime.state.isPreviewing).toBe(true)
    expect(ws.sent).toHaveLength(0)

    await runtime.confirmPlan(runtime.state.plannedTree)
    await runtime.processEvent({ type: 'tree_started', tree_id: 'tree-1' })
    await runtime.processEvent({ type: 'node_completed', tree_id: 'tree-1', node_id: 'node-1', output: 'done' })
    expect(runtime.state.isPaused).toBe(true)
    runtime.resumeExecution()
    expect(runtime.state.isPaused).toBe(false)
    expect(ws.sent[0]).toMatchObject({ type: 'confirm_plan', execution_mode: 'step' })
    expect(ws.sent[1]).toMatchObject({ type: 'resume_tree', tree_id: 'tree-1', node_id: 'node-1' })
    expect(ws.sent.map(item => item.type)).toEqual(['confirm_plan', 'resume_tree'])
  })

  it('normalizes native socket errors and settles after reconnect exhaustion', () => {
    const errors: any[] = []
    const runtime = createFastPlanExecutionController({ sessionId: 'session-1', maxReconnectAttempts: 0,
      WebSocket: MockSocket as any, EventSource: MockEvents as any, fetch: vi.fn(async () => new Response('{}')),
      onEvent: (name, payload) => { if (name === 'error') errors.push(payload) } })
    runtime.connect(); const ws = MockSocket.instances[0]; ws.open(); runtime.state.isPlanning = true
    runtime.state.executionState['node-1'] = { id: 'node-1', status: 'running', startTime: Date.now() - 100 }
    ws.onerror?.({ type: 'error' })
    ws.onclose?.({ code: 1006 })
    expect(runtime.state.connectionStatus).toBe('failed')
    expect(runtime.state.isPlanning).toBe(false)
    expect(runtime.state.error.message).toContain('FastPlan WebSocket')
    expect(runtime.state.executionState['node-1']).toMatchObject({
      status: 'failed', transportInterrupted: true,
      error: '连接已中断，无法确认后端最终执行状态',
    })
    expect(String(runtime.state.error)).not.toContain('[object Event]')
    expect(errors).toHaveLength(1)
  })

  it('hydrates host session metadata together with runtime execution state', async () => {
    const hostState: any = { ...createFastPlanRuntimeState(), currentSessionId: null, todos: [] }
    const request = vi.fn(async (url: RequestInfo | URL) => String(url).includes('/execution-state')
      ? new Response(JSON.stringify({ has_state: false }))
      : new Response(JSON.stringify({ messages: [{ id: 42, role: 'user', content: 'hello' }], todos: [{ id: 'todo-1', content: 'review' }], executions: [] })))
    const runtime = createFastPlanExecutionController({ state: hostState, fetch: request })
    await runtime.loadSession('session-1', { connect: false })
    expect(hostState.currentSessionId).toBe('session-1')
    expect(hostState.messages[0]).toMatchObject({ id: '42', content: 'hello' })
    expect(hostState.todos[0]).toMatchObject({ id: 'todo-1', content: 'review' })
  })

  it('binds every persisted execution to its conversation turn and updates retained background sessions', async () => {
    const sessions: Record<string, any> = {
      one: { messages: [
        { id: 'u1', role: 'user', content: 'first', created_at: '2026-01-01T00:00:00Z' },
        { id: 'u2', role: 'user', content: 'second', created_at: '2026-01-02T00:00:00Z' },
      ], executions: [
        { tree_id: 'tree-a', tree_json: tree, state_json: { 'node-1': { status: 'completed' } }, status: 'completed', created_at: '2026-01-01T00:01:00Z' },
        { tree_id: 'tree-b', tree_json: tree, state_json: { 'node-1': { status: 'running' } }, status: 'running', created_at: '2026-01-02T00:01:00Z' },
      ] },
      two: { messages: [], executions: [] },
    }
    const request = vi.fn(async (url: RequestInfo | URL) => {
      const value = String(url)
      if (value.includes('/execution-state')) return new Response(JSON.stringify({ has_state: false }))
      if (value.includes('/api/sessions/one')) return new Response(JSON.stringify(sessions.one))
      if (value.includes('/api/sessions/two')) return new Response(JSON.stringify(sessions.two))
      return new Response('{}')
    })
    const runtime = createFastPlanExecutionController({ fetch: request, retainConnectionOnSessionSwitch: true,
      WebSocket: MockSocket as any, EventSource: MockEvents as any })
    await runtime.loadSession('one', { connect: false })
    expect(runtime.state.executionHistory.map(item => item.anchorMessageId)).toEqual(['u1', 'u2'])
    expect(adaptFastPlanExecutionHistory(runtime.state)).toHaveLength(2)
    await runtime.loadSession('two', { connect: false })
    await runtime.processEvent({ type: 'node_completed', tree_id: 'tree-b', node_id: 'node-1', output: 'background done' }, 'one', 'ws')
    await runtime.loadSession('one', { connect: false })
    expect(runtime.state.executionState['node-1'].output).toBe('background done')
  })

  it('restores direct-answer traces from message metadata and anchors duplicate user echoes canonically', async () => {
    const trace = { turn_id: 'turn-direct', user_message_id: 2, status: 'completed', route_stages: [
      { stage: 'intent', status: 'done', summary: '意图理解完成', payload: { intent_type: 'data_query' } },
      { stage: 'routing', status: 'done', summary: 'Direct', payload: { decision: 'direct' } },
    ] }
    const request = vi.fn(async (url: RequestInfo | URL) => String(url).includes('/execution-state')
      ? new Response(JSON.stringify({ has_state: false }))
      : new Response(JSON.stringify({ messages: [
        { id: 1, role: 'user', content: 'hello', created_at: '2026-01-01T00:00:00Z' },
        { id: 2, role: 'user', content: 'hello', created_at: '2026-01-01T00:00:00Z' },
        { id: 3, role: 'assistant', content: 'world', metadata: JSON.stringify({ process_trace: trace }), created_at: '2026-01-01T00:00:01Z' },
      ], executions: [] })))
    const runtime = createFastPlanExecutionController({ fetch: request })
    await runtime.loadSession('session-1', { connect: false })
    expect(runtime.state.routeFlow[1].payload.decision).toBe('direct')
    expect(runtime.state.executionHistory[0].anchorMessageId).toBe('1')
    expect(adaptFastPlanExecutionHistory(runtime.state)[0]!.plan!.presentation!.routeStages).toHaveLength(2)
  })

  it('restores a failed planning trace as failed after refresh', async () => {
    const trace = { turn_id: 'turn-failed', user_message_id: 1, status: 'failed', error: 'HTTP 401: invalid_api_key', route_stages: [
      { stage: 'intent', status: 'running', summary: 'Analyzing', payload: {} },
    ], validation_flow: [{ stage: 'llm_progress', status: 'running', content: 'Planning' }] }
    const request = vi.fn(async (url: RequestInfo | URL) => String(url).includes('/execution-state')
      ? new Response(JSON.stringify({ has_state: false }))
      : new Response(JSON.stringify({ messages: [
        { id: 1, role: 'user', content: 'create a plan', created_at: '2026-01-01T00:00:00Z' },
        { id: 2, role: 'assistant', content: '规划失败: invalid_api_key', metadata: JSON.stringify({ msg_type: 'error', process_trace: trace }), created_at: '2026-01-01T00:00:01Z' },
      ], executions: [] })))
    const runtime = createFastPlanExecutionController({ fetch: request })

    await runtime.loadSession('session-1', { connect: false })

    expect(runtime.state.planningError?.message).toContain('HTTP 401')
    expect(runtime.state.routeFlow[0].status).toBe('failed')
    expect(runtime.state.messages[1].isError).toBe(true)
    const restoredPlan = adaptFastPlanExecutionHistory(runtime.state)[0]!.plan!
    expect(restoredPlan.status).toBe('failed')
    expect(restoredPlan.presentation!.routeStages![0].status).toBe('failed')
    expect(restoredPlan.presentation!.validation![0].status).toBe('failed')
  })

  it('keeps an active trace in front of an older execution snapshot after reload', async () => {
    const activeTrace = { turn_id: 'turn-active', user_message_id: 9, status: 'running', route_stages: [
      { stage: 'intent', status: 'running', summary: 'Analyzing', payload: {} },
    ] }
    const request = vi.fn(async (url: RequestInfo | URL) => {
      if (String(url).includes('/execution-state')) {
        return new Response(JSON.stringify({ has_state: true, tree_id: 'old-tree', tree, node_states: {}, status: 'completed' }))
      }
      return new Response(JSON.stringify({ metadata: { active_process_trace: activeTrace }, messages: [
        { id: 9, role: 'user', content: 'new request', created_at: '2026-01-02T00:00:00Z' },
      ], executions: [
        { tree_id: 'old-tree', tree_json: tree, state_json: {}, status: 'completed', created_at: '2026-01-01T00:01:00Z' },
      ] }))
    })
    const runtime = createFastPlanExecutionController({ fetch: request })
    await runtime.loadSession('session-1', { connect: false })
    expect(runtime.state.routeFlow[0].summary).toBe('Analyzing')
    expect(runtime.state.plannedTree).toBeNull()
    expect(runtime.state.executionHistory).toHaveLength(1)
    expect(request.mock.calls.some(call => String(call[0]).includes('/execution-state'))).toBe(true)
  })

  it('does not roll a completed session node back to pending during execution-state hydration', async () => {
    const request = vi.fn(async (url: RequestInfo | URL) => {
      if (String(url).includes('/execution-state')) {
        return new Response(JSON.stringify({ has_state: true, tree_id: 'tree-1', tree,
          node_states: { 'node-1': { status: 'queued' } }, status: 'running' }))
      }
      return new Response(JSON.stringify({ messages: [{ id: 1, role: 'user', content: 'run' }], executions: [
        { tree_id: 'tree-1', tree_json: tree, state_json: { 'node-1': { status: 'completed', output: 'done' } },
          status: 'running', created_at: '2026-01-01T00:00:00Z' },
      ] }))
    })
    const runtime = createFastPlanExecutionController({ fetch: request })

    await runtime.loadSession('session-1', { connect: false })

    expect(runtime.state.executionState['node-1']).toMatchObject({ status: 'completed', output: 'done' })
  })
})
