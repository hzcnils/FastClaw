/** treeCompleted means ended, not succeeded. Explicit outcome wins over UI flags. */
export function resolveExecutionStatus(state: any): string {
  if (state.executionStatus) return state.executionStatus
  if (state.treeCompleted) {
    if (state.processTrace?.status === 'failed' || Object.values(state.executionState || {}).some((node: any) => node?.status === 'failed')) return 'failed'
    return 'completed'
  }
  return state.isPaused ? 'paused' : state.isPreviewing ? 'planned' : state.isPlanning ? 'running' : 'paused'
}
