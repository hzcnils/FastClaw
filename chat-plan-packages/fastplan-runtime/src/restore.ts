import { normalizeTokenUsage } from './events'

type ExecutionNode = Record<string, any>

export function normalizeFastPlanNodeStatus(value: unknown): string {
  const status = String(value || '').trim().toLowerCase().replace(/[-\s]+/g, '_')
  if (['complete', 'completed', 'success', 'succeeded', 'done'].includes(status)) return 'completed'
  if (['running', 'started', 'in_progress', 'executing', 'processing'].includes(status)) return 'running'
  if (['failed', 'failure', 'error'].includes(status)) return 'failed'
  if (['skipped', 'skip', 'cancelled', 'canceled'].includes(status)) return 'skipped'
  if (['paused', 'waiting_human', 'waiting_for_human'].includes(status)) return status
  return 'pending'
}

function nodeState(id: string, name = ''): ExecutionNode {
  return { id, name, status: 'pending', logs: [], streamContent: '', output: '', error: null, startTime: null, endTime: null, durationMs: null, confidence: null, usage: null }
}

function nodeIndex(tree: any) {
  const index = new Map<string, any>()
  const visit = (node: any) => { if (!node?.id) return; index.set(node.id, node); for (const child of node.children || []) visit(child) }
  visit(tree?.root)
  return index
}

/** Converts `/execution-state` response into the reusable FastPlan runtime state. */
export function hydrateFastPlanExecutionState(data: any) {
  const tree = data?.tree || null
  const index = nodeIndex(tree)
  const executionState: Record<string, ExecutionNode> = {}
  const taskBubbles: string[] = []
  const visit = (nodes: any[]) => { for (const task of nodes || []) {
    executionState[task.id] = nodeState(task.id, task.name); taskBubbles.push(task.id); visit(task.children || [])
  } }
  for (const phase of tree?.root?.children || []) visit(phase.children || [])
  const raw = data?.node_states && typeof data.node_states === 'object' && !Array.isArray(data.node_states) ? data.node_states : {}
  const { __finalResult, __resultFiles, __planningLogs, __processTrace, __runtime, ...savedNodes } = raw
  const ensure = (id: string, name = '') => {
    if (!executionState[id]) {
      executionState[id] = nodeState(id, name || index.get(id)?.name || id)
      if (id !== '0') taskBubbles.push(id)
    }
    return executionState[id]
  }
  for (const [id, saved] of Object.entries(savedNodes)) if (saved && typeof saved === 'object') {
    Object.assign(ensure(id, (saved as any).name), saved, { status: normalizeFastPlanNodeStatus((saved as any).status) })
  }
  // ★ 全局降级汇总重建（2026-08-23）：从已保存节点收集 degraded，恢复会话后
  // 最终结果顶部的降级警示仍然可见
  const degradedSummary: string[] = []
  for (const saved of Object.values(savedNodes)) {
    if (saved && Array.isArray((saved as any).degraded)) {
      for (const reason of (saved as any).degraded) if (!degradedSummary.includes(reason)) degradedSummary.push(reason)
    }
  }
  for (const output of Array.isArray(data?.node_outputs) ? data.node_outputs : []) {
    const id = output.node_id
    if (!id || id === '0') continue
    const target = ensure(id)
    const value = output.output_data
    const normalized = typeof value === 'string' ? value : value?.content || value?.summary || JSON.stringify(value || '')
    Object.assign(target, { status: output.status ? normalizeFastPlanNodeStatus(output.status) : target.status, output: normalized || target.output, error: output.error_message || target.error,
      usage: normalizeTokenUsage(output, value) || target.usage })
  }
  // Repair legacy snapshots that incorrectly encoded a failed execution as completed.
  let status = String(data?.status || '').trim().toLowerCase()
  if (['complete', 'success', 'succeeded', 'done'].includes(status)) status = 'completed'
  else if (['failure', 'error'].includes(status)) status = 'failed'
  else if (status === 'canceled') status = 'cancelled'
  else if (['started', 'in_progress', 'processing'].includes(status)) status = 'running'
  if (status === 'completed' && (__processTrace?.status === 'failed' || __runtime?.executionStatus === 'failed'
    || (!__runtime?.executionStatus && Object.values(executionState).some(node => node.status === 'failed')))) status = 'failed'
  const ended = ['completed', 'failed', 'cancelled'].includes(status)
  const humanRequestQueue = Array.isArray(__runtime?.humanRequestQueue) ? __runtime.humanRequestQueue : []
  const humanRequest = __runtime?.humanRequest || humanRequestQueue[0] || null
  const executionMode = __runtime?.executionMode || 'auto'
  const confirmationPolicy = __runtime?.confirmationPolicy || 'legacy'
  const planned = status === 'planned'
  const previewing = __runtime?.isPreviewing
    ?? (planned && (executionMode === 'manual' || (confirmationPolicy === 'strict' && executionMode === 'step')))
  const pendingAutoConfirm = __runtime?.pendingAutoConfirm
    ?? (planned && !previewing)
  return { currentTreeId: data?.tree_id || null, plannedTree: tree, executionState, taskBubbles, degradedSummary,
    status, executionStatus: ended ? status as 'completed' | 'failed' | 'cancelled' : null,
    finalResult: __finalResult || null,
    resultFiles: Array.isArray(__resultFiles) ? __resultFiles : [], planningLogs: Array.isArray(__planningLogs) ? __planningLogs : [],
    routeFlow: Array.isArray(__processTrace?.route_stages) ? __processTrace.route_stages : [],
    validationFlow: Array.isArray(__processTrace?.validation_flow) ? __processTrace.validation_flow : [],
    thinkingSteps: Array.isArray(__processTrace?.thinking_steps) ? __processTrace.thinking_steps : [],
    thinkingStream: String(__processTrace?.thinking_stream || ''), processTrace: __processTrace || null,
    treeCompleted: ended,
    isPlanning: __runtime?.isPlanning ?? (status === 'executing' || status === 'running'),
    isPreviewing: previewing,
    isPaused: __runtime?.isPaused ?? (status === 'paused'),
    pendingAutoConfirm,
    humanRequest,
    humanRequestQueue,
    humanInterventionRequired: __runtime?.humanInterventionRequired ?? !!humanRequest,
    executionMode,
    confirmationPolicy,
    stepConfirmNodeId: __runtime?.stepConfirmNodeId ?? null,
    lastCompletedNodeId: __runtime?.lastCompletedNodeId ?? null,
    confirmedHistory: Array.isArray(__runtime?.confirmedHistory) ? __runtime.confirmedHistory : [],
  }
}
