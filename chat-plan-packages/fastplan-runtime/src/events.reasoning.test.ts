import { describe, expect, it } from 'vitest'
import { applyFastPlanEvent, createFastPlanRuntimeState, hydrateFastPlanProcessTrace } from './events'

describe('planning reasoning stream', () => {
  it('appends model reasoning chunks for live planning details', () => {
    const state = createFastPlanRuntimeState()
    applyFastPlanEvent(state, { type: 'planning_start' })
    applyFastPlanEvent(state, { type: 'thinking_update', phase: 'planning', tree_id: 'planner', node_id: 'plan', data: { kind: 'reasoning', content: '## Plan\n' } })
    applyFastPlanEvent(state, { type: 'thinking_update', phase: 'planning', tree_id: 'planner', node_id: 'plan', data: { kind: 'reasoning', content: '- first task' } })

    expect(state.thinkingStream).toBe('## Plan\n- first task')
  })

  it('settles every running planning stage when model planning fails', () => {
    const state = createFastPlanRuntimeState()
    applyFastPlanEvent(state, { type: 'planning_start' })
    applyFastPlanEvent(state, {
      type: 'route_stage_update', stage: 'intent', status: 'running', summary: '正在分析用户意图',
    })
    applyFastPlanEvent(state, {
      type: 'thinking_update', phase: 'planning', tree_id: 'planner',
      data: { kind: 'plan_stage', stage: 'llm_progress', status: 'running', content: '正在调用大模型生成任务树' },
    })

    applyFastPlanEvent(state, {
      type: 'error', message: 'LLM error: model service HTTP 401: authentication failed (invalid_api_key)',
    })

    expect(state.isPlanning).toBe(false)
    expect(state.planningError?.message).toContain('HTTP 401')
    expect(state.routeFlow[0]).toMatchObject({ stage: 'intent', status: 'failed' })
    expect(state.validationFlow[0]).toMatchObject({ stage: 'llm_progress', status: 'failed' })
    expect(state.messages[state.messages.length - 1]).toMatchObject({ role: 'assistant', isError: true })
  })

  it('does not classify an execution error as a planning failure after a tree exists', () => {
    const state = createFastPlanRuntimeState()
    applyFastPlanEvent(state, { type: 'planning_start' })
    applyFastPlanEvent(state, { type: 'plan_generated', tree: { id: 'tree-1', root: { children: [] } } })
    applyFastPlanEvent(state, { type: 'tree_started', tree_id: 'tree-1' })
    applyFastPlanEvent(state, { type: 'error', message: 'execution service failed' })

    expect(state.planningError).toBeNull()
  })

  it('restores a persisted failed trace without reviving running stages', () => {
    const state = createFastPlanRuntimeState()
    hydrateFastPlanProcessTrace(state, {
      status: 'failed', error: 'HTTP 401: invalid_api_key',
      route_stages: [{ stage: 'intent', status: 'running', summary: 'Analyzing', payload: {} }],
      validation_flow: [{ stage: 'llm_progress', status: 'running', content: 'Planning' }],
    })

    expect(state.planningError?.message).toContain('HTTP 401')
    expect(state.routeFlow[0].status).toBe('failed')
    expect(state.validationFlow[0].status).toBe('failed')
  })
})
