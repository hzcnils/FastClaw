import { describe, expect, it } from 'vitest'
import { hydrateFastPlanExecutionState } from './restore'

const tree = {
  id: 'tree-1',
  root: {
    id: 'root',
    children: [{ id: 'phase-1', children: [{ id: 'node-1', name: '第一步', children: [] }] }],
  },
}

describe('hydrateFastPlanExecutionState interaction state', () => {
  it('normalizes backend node and lifecycle status aliases', () => {
    const state = hydrateFastPlanExecutionState({
      tree_id: 'tree-1',
      tree,
      status: 'success',
      node_states: { 'node-1': { status: 'succeeded' } },
    })

    expect(state.status).toBe('completed')
    expect(state.treeCompleted).toBe(true)
    expect(state.executionState['node-1'].status).toBe('completed')
  })

  it('restores manual and human checkpoint state from the persisted runtime snapshot', () => {
    const request = { type: 'waiting_for_human', node_id: 'node-1', prompt: '请确认' }
    const state = hydrateFastPlanExecutionState({
      tree_id: 'tree-1',
      tree,
      status: 'paused',
      node_states: {
        'node-1': { status: 'completed' },
        __runtime: {
          executionMode: 'step',
          confirmationPolicy: 'strict',
          isPaused: true,
          humanRequestQueue: [request],
          humanInterventionRequired: true,
          stepConfirmNodeId: 'node-1',
          lastCompletedNodeId: 'node-1',
          confirmedHistory: [{ id: 1, detail: '继续' }],
        },
      },
    })

    expect(state).toMatchObject({
      executionMode: 'step',
      confirmationPolicy: 'strict',
      isPaused: true,
      humanRequest: request,
      humanInterventionRequired: true,
      stepConfirmNodeId: 'node-1',
      lastCompletedNodeId: 'node-1',
    })
    expect(state.confirmedHistory).toHaveLength(1)
  })

  it('marks an unconfirmed auto plan for idempotent confirmation after reconnect', () => {
    const state = hydrateFastPlanExecutionState({
      tree_id: 'tree-1',
      tree,
      status: 'planned',
      node_states: { __runtime: { executionMode: 'auto', confirmationPolicy: 'legacy' } },
    })

    expect(state.isPreviewing).toBe(false)
    expect(state.pendingAutoConfirm).toBe(true)
  })
})
