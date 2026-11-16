import { describe, expect, it } from 'vitest'
import { restoreExecutionHistory } from './executionHistory'

const tree = (id: string) => ({ id, description: id, metadata: { presentation: { plan_summary: id } },
  root: { children: [{ id: 'phase', children: [{ id: 'task_1' }] }] } })

describe('multi-turn execution history', () => {
  it('merges duplicate snapshots, repairs temporary anchors and preserves each tree outcome', () => {
    const messages = [
      { id: '25', role: 'user', content: 'first', turn_id: 'turn-a' },
      { id: '26', role: 'user', content: 'first' },
      { id: '27', role: 'assistant', tree_id: 'a', isError: true, content: 'invalid_api_key',
        process_trace: { user_message_id: 25, turn_id: 'turn-a', status: 'failed', stage_logs: { intent: ['intent'] } } },
      { id: '54', role: 'user', content: 'second', turn_id: 'turn-b' },
      { id: '56', role: 'assistant', tree_id: 'b', process_trace: { user_message_id: 54, turn_id: 'turn-b' } },
    ]
    const record = (id: number, treeId: string, state: any, status = 'completed') => ({ id, tree_id: treeId,
      tree_json: tree(treeId), state_json: state, status, created_at: `2026-09-04T01:00:0${id}Z` })
    const result = restoreExecutionHistory([
      record(1, 'a', { __processTrace: { user_message_id: 25, status: 'failed' } }, 'failed'),
      record(2, 'a', { task_1: { status: 'failed', error: 'invalid_api_key' },
        __runtime: { anchorMessageId: 'temporary-a' }, __processTrace: { validation_flow: [{ stage: 'plan', status: 'done' }] } }),
      record(3, 'b', { __processTrace: { user_message_id: 54 } }),
      record(4, 'b', { task_1: { status: 'completed', output: 'second output' }, __runtime: { anchorMessageId: 'temporary-b' },
        __finalResult: { content: 'second result' } }),
    ].reverse(), messages)
    const first = result.find(entry => entry.treeId === 'a')!
    const second = result.find(entry => entry.treeId === 'b')!
    expect(result).toHaveLength(2)
    expect(first).toMatchObject({ anchorMessageId: '25', status: 'failed', lastPrompt: 'first',
      error: { message: 'invalid_api_key' }, presentation: { plan_summary: 'a' } })
    expect(first.processTrace.stage_logs.intent).toEqual(['intent'])
    expect(first.validationFlow).toEqual([{ stage: 'plan', status: 'done' }])
    expect(second).toMatchObject({ anchorMessageId: '54', status: 'completed', finalResult: { content: 'second result' } })
    expect(first.executionState.task_1.error).toBe('invalid_api_key')
    expect(second.executionState.task_1.output).toBe('second output')
  })

  it('maps a turn id to a real user id and does not merge intentional repeated prompts', () => {
    const messages = [{ id: '1', role: 'user', content: 'again', turn_id: 'a' },
      { id: '2', role: 'user', content: 'again', turn_id: 'b' }]
    const [result] = restoreExecutionHistory([{ id: 1, tree_id: 'b', turn_id: 'b', tree_json: tree('b'),
      state_json: {}, status: 'completed' }], messages)
    expect(result.anchorMessageId).toBe('2')
  })

  it('preserves a final result when a later full snapshot contains null', () => {
    const messages = [{ id: '1', role: 'user', content: 'draw a chart' }]
    const snapshots = [
      { id: 1, tree_id: 'chart', message_id: 1, tree_json: tree('chart'), status: 'completed',
        created_at: '2026-09-04T01:00:01Z', state_json: {
          task_1: { status: 'completed', output: 'chart data' },
          __finalResult: { content: '```echarts\n{"series":[]}\n```' },
        } },
      { id: 2, tree_id: 'chart', message_id: 1, tree_json: tree('chart'), status: 'completed',
        created_at: '2026-09-04T01:00:02Z', state_json: {
          task_1: { status: 'completed', output: 'chart data' },
          __finalResult: null,
        } },
    ]

    const [result] = restoreExecutionHistory(snapshots, messages)

    expect(result.finalResult).toEqual({ content: '```echarts\n{"series":[]}\n```' })
  })
})
