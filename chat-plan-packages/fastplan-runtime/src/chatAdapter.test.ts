import { describe, expect, it } from 'vitest'
import { adaptFastPlanMessages, adaptFastPlanTaskPlanExecution } from './chatAdapter'
import { createFastPlanRuntimeState } from './events'

describe('FastPlan chat history final result display', () => {
  it('replaces the persisted assistant summary with the complete execution result', () => {
    const messages = adaptFastPlanMessages([
      { id: 10, role: 'user', content: 'prepare report', timestamp: '2026-08-23T00:00:00Z' },
      { id: 11, role: 'assistant', content: 'short summary', timestamp: '2026-08-23T00:00:01Z' },
    ], {
      executionHistory: [{ treeId: 'tree-1', anchorMessageId: '10', finalResult: { content: 'complete final report' } }],
    })

    expect(messages[1].parts[0].text).toBe('complete final report')
    expect(messages[1].metadata.completeFinalResult).toBe(true)
    expect(messages[1].metadata.contentKind).toBe('final_answer')
  })

  it('keeps the persisted summary when the execution snapshot has no complete result', () => {
    const messages = adaptFastPlanMessages([
      { id: 10, role: 'user', content: 'prepare report' },
      { id: 11, role: 'assistant', content: 'short summary' },
    ], { executionHistory: [{ treeId: 'tree-1', anchorMessageId: '10', finalResult: null }] })

    expect(messages[1].parts[0].text).toBe('short summary')
  })

  it('uses the current runtime final result when it is newer than the history entry', () => {
    const messages = adaptFastPlanMessages([
      { id: 10, role: 'user', content: 'prepare report' },
      { id: 11, role: 'assistant', content: 'short summary' },
    ], {
      executionHistory: [{ treeId: 'tree-1', anchorMessageId: '10', finalResult: { content: 'older result' } }],
      currentTreeId: 'tree-1',
      finalResult: { content: 'new complete result' },
    })

    expect(messages[1].parts[0].text).toBe('new complete result')
  })

  it('localizes artifact boilerplate restored from an execution snapshot', () => {
    const messages = adaptFastPlanMessages([
      { id: 10, role: 'user', content: 'prepare document' },
      { id: 11, role: 'assistant', content: 'old response' },
    ], { executionHistory: [{ treeId: 'tree-1', anchorMessageId: '10',
      finalResult: { content: 'Task completed. 1 file(s) generated. Download: /api/download/run/output.docx' },
      resultFiles: [{ name: 'output.docx', url: '/api/download/run/output.docx' }] }] })

    expect(messages[1].parts[0].text).toBe('文档已生成，可在下方下载。')
  })

  it('creates a dedicated final answer instead of promoting an arbitrary assistant message', () => {
    const messages = adaptFastPlanMessages([
      { id: 10, role: 'user', content: 'prepare report' },
      { id: 11, role: 'assistant', content: 'preliminary note' },
      { id: 12, role: 'assistant', content: 'another ordinary message' },
    ], {
      executionHistory: [{ treeId: 'tree-1', anchorMessageId: '10', finalResult: { content: 'complete final report' } }],
    })

    expect(messages).toHaveLength(4)
    expect(messages[1].metadata.contentKind).toBe('ordinary_message')
    expect(messages[2].metadata.contentKind).toBe('ordinary_message')
    expect(messages[3]).toMatchObject({
      role: 'assistant',
      parts: [{ type: 'text', text: 'complete final report', format: 'markdown' }],
      metadata: { contentKind: 'final_answer', restoredFromExecution: true },
    })
  })

  it('uses explicit final-answer provenance when several assistant messages exist', () => {
    const messages = adaptFastPlanMessages([
      { id: 10, role: 'user', content: 'prepare report' },
      { id: 11, role: 'assistant', content: 'preliminary note' },
      { id: 12, role: 'assistant', content: 'short final', isFinalAnswer: true, treeId: 'tree-1' },
    ], {
      executionHistory: [{ treeId: 'tree-1', anchorMessageId: '10', finalResult: { content: 'complete final report' } }],
    })

    expect(messages).toHaveLength(3)
    expect(messages[1].metadata.contentKind).toBe('ordinary_message')
    expect(messages[2].parts[0].text).toBe('complete final report')
    expect(messages[2].metadata.contentKind).toBe('final_answer')
  })

  it('recognizes a persisted execution result by its tree metadata', () => {
    const messages = adaptFastPlanMessages([
      { id: 10, role: 'user', content: 'prepare report' },
      { id: 11, role: 'assistant', content: 'preliminary note' },
      { id: 12, role: 'assistant', content: 'persisted summary', msg_type: 'execution', tree_id: 'tree-1' },
    ], {
      executionHistory: [{ treeId: 'tree-1', anchorMessageId: '10', finalResult: { content: 'complete final report' } }],
    })

    expect(messages).toHaveLength(3)
    expect(messages[1].metadata.contentKind).toBe('ordinary_message')
    expect(messages[2].parts[0].text).toBe('complete final report')
    expect(messages[2].metadata.contentKind).toBe('final_answer')
  })
})

describe('FastPlan planning failure display state', () => {
  it('exposes a failed plan instead of a completed trace when planning failed', () => {
    const state = createFastPlanRuntimeState()
    state.routeFlow = [{ stage: 'intent', status: 'failed', summary: '意图理解失败', payload: {} }]
    state.planningError = { message: 'HTTP 401: invalid_api_key' }

    expect(adaptFastPlanTaskPlanExecution(state)?.status).toBe('failed')
  })
})
