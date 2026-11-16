import { afterEach, describe, expect, it, vi } from 'vitest'
import { fastPlanRequest, type FastPlanAccess } from './access'
import { createFastPlanExecutionController, type FastPlanExecutionController } from './executionController'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}
class Socket {
  static OPEN = 1
  static CONNECTING = 0
  static instances: Socket[] = []
  readyState = 0
  sent: any[] = []
  onopen?: () => void
  onmessage?: (event: { data: string }) => void
  onclose?: () => void
  constructor() { Socket.instances.push(this) }
  send(data: string) { this.sent.push(JSON.parse(data)) }
  open(managed: boolean) {
    this.readyState = 1; this.onopen?.()
    if (managed) this.onmessage?.({ data: '{"type":"authenticated"}' })
  }
  close() { this.readyState = 3 }
}
class Events { addEventListener() {} close() {} }
const tree = { id: 'tree-1', root: { children: [] }, metadata: {} }
const connection = { apiBase: 'http://localhost:5179', wsBase: 'ws://localhost:5179', parentToken: 'test' }
const model = { provider: 'test', model: 'test', base_url: 'http://model', auth_type: 'none' as const }
const runtimes: FastPlanExecutionController[] = []
function create(access?: FastPlanAccess) {
  const runtime = createFastPlanExecutionController({
    sessionId: 's', access, WebSocket: Socket as any, EventSource: Events as any,
    fetch: vi.fn(async () => new Response('{}')), persistUserMessages: false,
  })
  runtimes.push(runtime)
  return runtime
}
async function open(runtime: FastPlanExecutionController, managed = false) {
  runtime.connect()
  await Promise.resolve()
  const socket = Socket.instances[Socket.instances.length - 1]!
  socket.open(managed)
  return socket
}
afterEach(() => {
  for (const runtime of runtimes.splice(0)) runtime.disconnect()
  Socket.instances = []
  vi.restoreAllMocks()
})

describe('controller cancellation boundaries', () => {
  function withNode(runtime: FastPlanExecutionController) {
    runtime.state.currentTreeId = 'tree-1'
    runtime.state.plannedTree = { ...structuredClone(tree), root: { children: [{ id: 'phase', children: [
      { id: 'node-1', node_type: { type: 'LlmTask', config: { prompt_template: 'old' } } },
    ] }] } }
    runtime.state.executionState['node-1'] = { id: 'node-1', status: 'failed', error: 'original failure', output: 'old' }
    return runtime.state.executionState['node-1']
  }

  it('does not change node edits offline; successful edits snapshot payloads and notify subscribers', async () => {
    const runtime = create(), node = withNode(runtime)
    const config = runtime.state.plannedTree.root.children[0].children[0].node_type.config
    expect(() => runtime.updateNodeOutput('node-1', 'new')).toThrow('not connected')
    expect(() => runtime.updateNextNodeConfig('node-1', { prompt_template: 'new' })).toThrow('not connected')
    expect(node.output).toBe('old'); expect(config.prompt_template).toBe('old')
    const socket = await open(runtime), changed = vi.fn(); runtime.on('state-change', changed)
    const output = { value: 'snapshot' }, changes = { arguments: { city: '西安' } }
    expect(runtime.updateNodeOutput('node-1', output)).toBe(true)
    expect(runtime.updateNextNodeConfig('node-1', changes)).toBe(true)
    output.value = 'late'; changes.arguments.city = '重庆'
    expect(node.output).toEqual({ value: 'snapshot' })
    expect(runtime.state.plannedTree.root.children[0].children[0].node_type.config.arguments.city).toBe('西安')
    expect(socket.sent.filter(item => item.type === 'edit_node')).toHaveLength(2)
    expect(changed).toHaveBeenCalledTimes(2)
    expect(runtime.updateNodeOutput('missing', 'no')).toBe(false)
  })

  it('coalesces a pending retry and leaves failures retryable rather than falsely running', async () => {
    const runtime = create(), node = withNode(runtime), pending = deferred<any>()
    const retry = vi.spyOn(runtime.client, 'retryTask').mockReturnValueOnce(pending.promise).mockResolvedValue({ ok: true })
    const first = runtime.retryNode('node-1'), duplicate = runtime.retryNode('node-1')
    expect(retry).toHaveBeenCalledTimes(1)
    expect(node).toMatchObject({ status: 'failed', error: 'original failure' })
    pending.reject(new Error('HTTP failure'))
    await expect(first).rejects.toThrow('HTTP failure'); await expect(duplicate).rejects.toThrow('HTTP failure')
    expect(node.status).toBe('failed')
    await runtime.retryNode('node-1')
    expect(node).toMatchObject({ status: 'running', error: null })
  })

  it.each(['cancelPlan', 'cancelExecution', 'disconnect', 'closeSession'] as const)('%s aborts retry and template-save waits without late mutations', async action => {
    const runtime = create(); await open(runtime); const node = withNode(runtime)
    runtime.state.canSaveTemplate = true; runtime.state.templateTreeData = runtime.state.plannedTree
    const retryGate = deferred<any>(), saveGate = deferred<any>()
    const retry = vi.spyOn(runtime.client, 'retryTask').mockReturnValue(retryGate.promise)
    const save = vi.spyOn(runtime.client, 'createTemplate').mockReturnValue(saveGate.promise)
    const retrying = runtime.retryNode('node-1'), saving = runtime.saveAsTemplate('saved')
    if (action === 'closeSession') runtime.closeSession('s')
    else runtime[action]()
    expect(retry.mock.calls[0][2]?.aborted).toBe(true); expect(save.mock.calls[0][1]?.aborted).toBe(true)
    await expect(retrying).resolves.toBe(false); await expect(saving).resolves.toBe(false)
    retryGate.reject(new Error('late')); saveGate.resolve({ id: 'saved' })
    await Promise.resolve()
    expect(node.status).toBe('failed'); expect(runtime.state.canSaveTemplate).toBe(true)
    expect(runtime.state.templateTreeData).not.toBeNull()
  })

  it.each(['completed', 'failed'] as const)('does not overwrite a newer %s node event when retry HTTP returns', async status => {
    const runtime = create(), node = withNode(runtime), gate = deferred<any>()
    vi.spyOn(runtime.client, 'retryTask').mockReturnValue(gate.promise)
    const task = runtime.retryNode('node-1')
    Object.assign(node, { status, error: 'new event', output: 'new output' })
    gate.resolve({ ok: true }); await task
    expect(node).toMatchObject({ status, error: 'new event', output: 'new output' })
  })

  it('keeps old retry completion isolated from a new tree reusing a node id', async () => {
    const runtime = create(); withNode(runtime)
    const gate = deferred<any>(), next = deferred<any>()
    const retry = vi.spyOn(runtime.client, 'retryTask').mockReturnValueOnce(gate.promise).mockReturnValueOnce(next.promise)
    const first = runtime.retryNode('node-1')
    runtime.state.currentTreeId = 'tree-2'
    runtime.state.executionState['node-1'] = { id: 'node-1', status: 'failed', error: 'new failure' }
    const second = runtime.retryNode('node-1')
    expect(retry).toHaveBeenCalledTimes(2)
    gate.resolve({ ok: true }); await expect(first).resolves.toBe(false)
    expect(runtime.state.executionState['node-1'].error).toBe('new failure')
    next.resolve({ ok: true }); await second
    expect(runtime.state.executionState['node-1'].status).toBe('running')
  })

  it('does not overwrite a repeated server failure with identical status/error text', async () => {
    const runtime = create(), node = withNode(runtime), gate = deferred<any>()
    vi.spyOn(runtime.client, 'retryTask').mockReturnValue(gate.promise)
    const task = runtime.retryNode('node-1')
    await runtime.processEvent({ type: 'node_failed', tree_id: 'tree-1', node_id: 'node-1', error: 'original failure' })
    gate.resolve({ ok: true }); await task
    expect(node).toMatchObject({ status: 'failed', error: 'original failure' })
  })

  it('coalesces identical template saves, preserves the result contract and rejects conflicting saves', async () => {
    const runtime = create(); withNode(runtime); runtime.state.canSaveTemplate = true
    const gate = deferred<any>(), save = vi.spyOn(runtime.client, 'createTemplate').mockReturnValue(gate.promise)
    const first = runtime.saveAsTemplate('name'), duplicate = runtime.saveAsTemplate('name')
    await expect(runtime.saveAsTemplate('other')).resolves.toBe(false)
    expect(save).toHaveBeenCalledTimes(1)
    gate.resolve({ id: 'saved-template' })
    await expect(first).resolves.toEqual({ id: 'saved-template' }); await expect(duplicate).resolves.toEqual({ id: 'saved-template' })
    expect(runtime.state.canSaveTemplate).toBe(false)
  })

  it.each(['success', 'failure'] as const)('does not clear a new template candidate after an old save %s', async outcome => {
    const runtime = create(); withNode(runtime); const gate = deferred<any>()
    vi.spyOn(runtime.client, 'createTemplate').mockReturnValue(gate.promise)
    const task = runtime.saveAsTemplate('original')
    runtime.state.plannedTree = { ...tree, id: 'tree-2' }
    runtime.state.templateTreeData = runtime.state.plannedTree; runtime.state.canSaveTemplate = true
    if (outcome === 'success') gate.resolve({ id: 'old-template' }); else gate.reject(new Error('late error'))
    await expect(task).resolves.toBe(false)
    expect(runtime.state.canSaveTemplate).toBe(true)
    expect(runtime.state.templateTreeData.id).toBe('tree-2')
  })

  it('keeps a failed template save retryable without changing its candidate', async () => {
    const runtime = create(); withNode(runtime); runtime.state.canSaveTemplate = true
    vi.spyOn(runtime.client, 'createTemplate').mockRejectedValueOnce(new Error('save failure')).mockResolvedValue({ id: 'ok' })
    await expect(runtime.saveAsTemplate('name')).rejects.toThrow('save failure')
    expect(runtime.state.canSaveTemplate).toBe(true)
    await expect(runtime.saveAsTemplate('name')).resolves.toEqual({ id: 'ok' })
  })

  it('resets the old terminal state for an explicit automatic-mode template preview and retains history', async () => {
    const runtime = create(); withNode(runtime)
    Object.assign(runtime.state, { treeCompleted: true, executionStatus: 'completed', finalResult: { content: 'old' },
      isPlanning: false, confirmedHistory: [{ detail: 'old confirmation' }] })
    const execute = vi.spyOn(runtime.client, 'executeTree').mockResolvedValue({ ok: true })
    const nextTree = { ...structuredClone(tree), id: 'tree-2' }
    const result = await runtime.executeTemplate(nextTree, [], { preview: true })
    expect(result.id).toBe('tree-2'); expect(result).not.toBe(nextTree)
    expect(runtime.state).toMatchObject({ currentTreeId: 'tree-2', isPreviewing: true, isPlanning: false,
      treeCompleted: false, executionStatus: null, finalResult: null, executionState: {}, confirmedHistory: [] })
    expect(runtime.state.executionHistory.at(-1)?.treeId).toBe('tree-1')
    expect(execute).not.toHaveBeenCalled()
    await runtime.confirmPlan(); expect(execute).toHaveBeenCalledTimes(1)
  })

  it('rejects overlapping template execution without cancelling the accepted request', async () => {
    const runtime = create(), gate = deferred<any>()
    const execute = vi.spyOn(runtime.client, 'executeTree').mockReturnValue(gate.promise)
    const first = runtime.executeTemplate(tree)
    await expect(runtime.executeTemplate({ ...tree, id: 'different' })).resolves.toBe(false)
    expect(execute).toHaveBeenCalledTimes(1)
    expect(execute.mock.calls[0][2]?.aborted).toBe(false)
    gate.resolve({ id: 'result' }); await expect(first).resolves.toEqual({ id: 'result' })
  })

  it.each(['retry', 'save'] as const)('does not send %s HTTP after cancellation during parent connection resolution', async command => {
    const gate = deferred<typeof connection>(), request = vi.fn(async () => new Response('{}'))
    const runtime = createFastPlanExecutionController({ sessionId: 's',
      access: { getConnection: () => gate.promise, resolveModel: async () => model }, fetch: request })
    runtimes.push(runtime); withNode(runtime)
    const task = command === 'retry' ? runtime.retryNode('node-1') : runtime.saveAsTemplate('name')
    runtime.disconnect()
    await expect(task).resolves.toBe(false)
    gate.resolve(connection); await Promise.resolve(); await Promise.resolve()
    expect(request).not.toHaveBeenCalled()
  })

  it('keeps an offline confirmation retryable without falsely leaving preview', async () => {
    const runtime = create()
    Object.assign(runtime.state, { plannedTree: structuredClone(tree), currentTreeId: tree.id, isPreviewing: true })
    await expect(runtime.confirmPlan()).rejects.toThrow('not connected')
    expect(runtime.state.isPreviewing).toBe(true)
    const socket = await open(runtime)
    await expect(runtime.confirmPlan()).resolves.toBe(true)
    expect(socket.sent.filter(item => item.type === 'confirm_plan')).toHaveLength(1)
    expect(runtime.state.isPreviewing).toBe(false)
  })
  it.each(['cancelPlan', 'cancelExecution', 'disconnect', 'closeSession'] as const)(
    '%s prevents a late model result from sending or mutating state', async action => {
      const pending = deferred<typeof model>()
      const runtime = create({ getConnection: async () => connection, resolveModel: () => pending.promise })
      const socket = await open(runtime, true)
      const errors = vi.fn(); runtime.on('error', errors)
      const sending = Promise.resolve(runtime.sendChat('old prompt'))
      if (action === 'closeSession') runtime.closeSession('s')
      else runtime[action]()
      pending.resolve(model)
      await expect(sending).resolves.toBe(false)
      expect(socket.sent.filter(item => item.type === 'chat')).toEqual([])
      expect(runtime.state.messages).toEqual([])
      expect(runtime.state.lastPrompt).toBe('')
      expect(errors).not.toHaveBeenCalled()
    },
  )

  it('finishes a cancelled model wait without waiting for the provider and ignores its late failure', async () => {
    const pending = deferred<typeof model>()
    const runtime = create({ getConnection: async () => connection, resolveModel: () => pending.promise })
    await open(runtime, true)
    const errors = vi.fn(); runtime.on('error', errors)
    const sending = Promise.resolve(runtime.sendChat('old'))
    runtime.cancelPlan()
    let result: unknown = 'still waiting'
    void sending.then(value => { result = value })
    // Drain promise reactions without resolving the provider or relying on a timer.
    for (let index = 0; index < 8; index++) await Promise.resolve()
    expect(result).toBe(false)
    pending.reject(new Error('late provider failure'))
    await Promise.resolve(); await Promise.resolve()
    expect(errors).not.toHaveBeenCalled()
    expect(runtime.state.executionStatus).toBe('cancelled')
  })

  it('invalidates pending send even when the stop packet cannot be delivered', async () => {
    const pending = deferred<typeof model>()
    const runtime = create({ getConnection: async () => connection, resolveModel: () => pending.promise })
    const sending = Promise.resolve(runtime.sendChat('old'))
    expect(() => runtime.cancelPlan()).toThrow('not connected')
    pending.resolve(model)
    await expect(sending).resolves.toBe(false)
    expect(runtime.state.messages).toEqual([])
  })

  it('does not revive auto-confirm on reconnect after an offline stop failed', async () => {
    const runtime = create()
    runtime.state.plannedTree = structuredClone(tree)
    runtime.state.pendingAutoConfirm = true
    expect(() => runtime.cancelPlan()).toThrow('not connected')
    const socket = await open(runtime)
    await Promise.resolve()
    expect(socket.sent).toEqual([])
    expect(runtime.state.pendingAutoConfirm).toBe(false)
  })

  it('only submits the newest request when model resolution returns out of order', async () => {
    const pending = deferred<typeof model>()
    const runtime = create({ getConnection: async () => connection,
      resolveModel: vi.fn().mockReturnValueOnce(pending.promise).mockResolvedValue(model) })
    const socket = await open(runtime, true)
    const old = runtime.sendChat('old')
    await expect(runtime.sendChat('new')).resolves.toBe(true)
    pending.reject(new Error('old failure'))
    await expect(old).resolves.toBe(false)
    expect(socket.sent.filter(item => item.type === 'chat').map(item => item.content)).toEqual(['new'])
    expect(runtime.state.planningError).toBeNull()
    expect(runtime.state.lastPrompt).toBe('new')
  })

  it('still reports a current model resolution failure', async () => {
    const runtime = create({ getConnection: async () => connection,
      resolveModel: async () => { throw new Error('current failure') } })
    const errors = vi.fn(); runtime.on('error', errors)
    await expect(runtime.sendChat('current')).rejects.toThrow('current failure')
    expect(errors).toHaveBeenCalledOnce()
  })

  it.each(['s', 'other'])('does not hydrate or clear a newer loading state from old load (%s)', async nextId => {
    const runtime = create()
    const oldData = deferred<any>(); const nextData = deferred<any>()
    vi.spyOn(runtime.client, 'getSession').mockReturnValueOnce(oldData.promise).mockReturnValueOnce(nextData.promise)
    vi.spyOn(runtime.client, 'getExecutionState').mockResolvedValue({ has_state: false })
    const changes = vi.fn(); runtime.on('state-change', changes)
    const old = runtime.loadSession('s', { connect: false, force: true })
    const next = runtime.loadSession(nextId, { connect: false, force: true })
    oldData.resolve({ messages: [{ id: 'old', role: 'user', content: 'old' }] })
    await old
    expect(runtime.loading.value).toBe(true)
    expect(runtime.state.messages).toEqual([])
    expect(changes).not.toHaveBeenCalled()
    nextData.resolve({ messages: [{ id: 'new', role: 'user', content: 'new' }] })
    await next
    expect(runtime.loading.value).toBe(false)
    expect(runtime.state.sessionId).toBe(nextId)
    expect(runtime.state.messages.map(item => item.content)).toEqual(['new'])
  })

  it('does not reconnect or revive a controller disconnected during history loading', async () => {
    const runtime = create(); const pending = deferred<any>()
    vi.spyOn(runtime.client, 'getSession').mockReturnValue(pending.promise)
    vi.spyOn(runtime.client, 'getExecutionState').mockResolvedValue({ has_state: false })
    const loading = runtime.loadSession('s')
    runtime.disconnect()
    pending.resolve({ messages: [{ id: 'old', role: 'user', content: 'old' }] })
    await loading
    expect(Socket.instances).toHaveLength(0)
    expect(runtime.state.messages).toEqual([])
    expect(runtime.loading.value).toBe(false)
  })

  it('does not auto-load when disposed before the initial microtask', async () => {
    const request = vi.fn(async () => new Response('{}'))
    const runtime = createFastPlanExecutionController({ sessionId: 's', autoConnect: true, fetch: request })
    runtimes.push(runtime)
    runtime.disconnect()
    await Promise.resolve(); await Promise.resolve()
    expect(request).not.toHaveBeenCalled()
  })

  it('rejects a stale model operation after A -> B -> A, without cancelling an unrelated session', async () => {
    const pending = deferred<typeof model>()
    const runtime = create({ getConnection: async () => connection, resolveModel: () => pending.promise })
    const socket = await open(runtime, true)
    const sending = runtime.sendChat('old A')
    runtime.connect('B'); runtime.connect('s')
    pending.resolve(model)
    await expect(sending).resolves.toBe(false)
    expect(socket.sent.filter(item => item.type === 'chat')).toEqual([])
    const next = runtime.sendChat('new A')
    runtime.closeSession('B')
    // The current A transport was replaced during switching; authenticate it.
    await Promise.resolve()
    Socket.instances[Socket.instances.length - 1]!.open(true)
    await expect(next).resolves.toBe(true)
  })

  it.each([true, false])('does not reopen preview on a stopped template result (success=%s)', async success => {
    const runtime = create(); await open(runtime)
    const pending = deferred<any>()
    vi.spyOn(runtime.client, 'executeTree').mockReturnValue(pending.promise)
    const executing = runtime.executeTemplate(tree)
    runtime.cancelPlan()
    if (success) pending.resolve({ ok: true })
    else pending.reject(new Error('late template failure'))
    await expect(executing).resolves.toBe(false)
    expect(runtime.state).toMatchObject({ isPreviewing: false, isPlanning: false, executionStatus: 'cancelled' })
  })

  it('keeps a current failed template retryable', async () => {
    const runtime = create()
    vi.spyOn(runtime.client, 'executeTree').mockRejectedValueOnce(new Error('current template failure')).mockResolvedValue({ ok: true })
    await expect(runtime.executeTemplate(tree)).rejects.toThrow('current template failure')
    expect(runtime.state).toMatchObject({ isPlanning: false, isPreviewing: true })
    await expect(runtime.confirmPlan()).resolves.toEqual({ ok: true })
    expect(runtime.state).toMatchObject({ isPlanning: true, isPreviewing: false })
  })

  it('does not remove a newer checkpoint with a reused node id', async () => {
    const runtime = create(); const pending = deferred<any>()
    runtime.state.currentTreeId = 'tree-1'
    runtime.state.humanRequestQueue = [{ tree_id: 'tree-1', node_id: 'node-1', prompt: 'old' }]
    vi.spyOn(runtime.client, 'respondCheckpoint').mockReturnValue(pending.promise)
    const answering = runtime.respondHumanCheckpoint({ value: 'continue' })
    runtime.state.humanRequestQueue = [{ tree_id: 'tree-1', node_id: 'node-1', prompt: 'new' }]
    pending.resolve({ ok: true }); await expect(answering).resolves.toBe(false)
    expect(runtime.state.humanRequestQueue.map(item => item.prompt)).toEqual(['new'])
    expect(runtime.state.confirmedHistory).toEqual([])
  })

  it('passes a cancelled signal to an already-started template HTTP request', async () => {
    const runtime = create(); await open(runtime)
    const pending = deferred<any>()
    const execute = vi.spyOn(runtime.client, 'executeTree').mockReturnValue(pending.promise)
    const executing = runtime.executeTemplate(tree)
    const signal = execute.mock.calls[0][2]!
    expect(signal.aborted).toBe(false)
    runtime.cancelPlan()
    expect(signal.aborted).toBe(true)
    await expect(executing).resolves.toBe(false)
    pending.resolve({ ok: true })
  })

  it('does not append a stopped checkpoint response to a subsequent turn', async () => {
    const runtime = create(); await open(runtime)
    const pending = deferred<any>()
    runtime.state.humanRequestQueue = [{ tree_id: 'tree-1', node_id: 'node-1' }]
    vi.spyOn(runtime.client, 'respondCheckpoint').mockReturnValue(pending.promise)
    const answering = runtime.respondHumanCheckpoint({ value: 'continue' })
    runtime.cancelPlan(); runtime.sendChat('new')
    pending.resolve({ ok: true })
    await expect(answering).resolves.toBe(false)
    expect(runtime.state.confirmedHistory).toEqual([])
  })

  it('drops auto-confirm queued before an immediate stop', async () => {
    const runtime = create(); const socket = await open(runtime)
    const processing = runtime.processEvent({ type: 'plan_generated', tree })
    runtime.cancelPlan()
    await processing; await Promise.resolve()
    expect(socket.sent.map(item => item.type)).toEqual(['cancel_plan'])
  })
})

describe('authenticated request cancellation', () => {
  it('does not start authentication or fetch if already aborted', async () => {
    const abort = new AbortController(); abort.abort()
    const getConnection = vi.fn(async () => connection)
    const request = vi.fn(async () => new Response('{}'))
    await expect(fastPlanRequest('/api/checkpoint', { signal: abort.signal }, {
      getConnection, resolveModel: async () => model,
    }, request)).rejects.toMatchObject({ name: 'AbortError' })
    expect(getConnection).not.toHaveBeenCalled()
    expect(request).not.toHaveBeenCalled()
  })
  it('does not dispatch after authentication returns for an aborted request', async () => {
    const pending = deferred<typeof connection>(); const abort = new AbortController()
    const request = vi.fn(async () => new Response('{}'))
    const response = fastPlanRequest('/api/checkpoint', { method: 'POST', signal: abort.signal }, {
      getConnection: () => pending.promise, resolveModel: async () => model,
    }, request)
    abort.abort(); pending.resolve(connection)
    await expect(response).rejects.toMatchObject({ name: 'AbortError' })
    expect(request).not.toHaveBeenCalled()
  })
})
