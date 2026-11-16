import { describe, expect, it } from 'vitest'
import { flattenTaskNodes, groupConfirmedHistoryByNode, lastPlanTaskId, revealedPhaseRows, taskProgress } from './taskDisplay'

const tree = { root: { children: [{ id: 'phase-1', name: '阶段一', children: [
  { id: 'one', name: '任务一', children: [{ id: 'child', name: '子任务' }] },
  { id: 'two', name: '任务二' },
] }] } }

describe('task display model', () => {
  it('creates recursive numbers without leading zero or indentation semantics', () => {
    expect(flattenTaskNodes(tree.root.children[0].children, '1').map(row => row.sequence))
      .toEqual(['1.1', '1.1.1', '1.2'])
  })

  it('reveals only started terminal tasks while retaining fixed phase totals', () => {
    const rows = revealedPhaseRows(tree, { one: { status: 'completed' }, child: { status: 'running' } })
    expect(rows[0].total).toBe(3)
    expect(rows[0].tasks.map(row => row.node.id)).toEqual(['one', 'child'])
  })

  it('counts skipped as success and failed separately', () => {
    expect(taskProgress(tree, { one: { status: 'completed' }, child: { status: 'skipped' }, two: { status: 'failed' } }))
      .toMatchObject({ total: 3, success: 2, failed: 1 })
  })

  it('finds the final task from the complete recursive plan order', () => {
    expect(lastPlanTaskId(tree)).toBe('two')
    expect(lastPlanTaskId({ root: { children: [] } })).toBeNull()
  })

  it('associates confirmations with task nodes and keeps unmatched legacy records', () => {
    const exact = { id: 'review-1', nodeId: 'two', completedNodeName: '任务二' }
    const legacy = { id: 'review-2', completedNodeName: '子任务' }
    const unmatched = { id: 'review-3', completedNodeName: '已删除任务' }

    expect(groupConfirmedHistoryByNode(tree, [exact, legacy, unmatched])).toEqual({
      byNodeId: { two: [exact], child: [legacy] },
      unmatched: [unmatched],
    })
  })
})
