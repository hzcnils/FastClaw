import { hydrateFastPlanExecutionState } from './restore'

const object = (value: any): any => {
  if (typeof value !== 'string') return value || {}
  try { return JSON.parse(value) || {} } catch { return {} }
}

/** One render snapshot per tree, not one panel per persistence write. */
export function restoreExecutionHistory(executions: any[], messages: any[]): any[] {
  const users = messages.filter(message => message.role === 'user')
  const canonical = (user: any) => {
    if (!user) return undefined
    let index = messages.indexOf(user)
    while (index > 0 && messages[index - 1].role === 'user'
      && !messages[index - 1].isResponse && !messages[index].isResponse
      && (!messages[index - 1].turn_id || !messages[index].turn_id || messages[index - 1].turn_id === messages[index].turn_id)
      && String(messages[index - 1].content).trim() === String(user.content).trim()) index--
    return messages[index]
  }
  const byId = (id: any) => id == null ? undefined : canonical(users.find(user => String(user.id) === String(id)))
  const groups = new Map<string, any[]>()
  for (const execution of executions) {
    const key = String(execution.tree_id || execution.id)
    groups.set(key, [...(groups.get(key) || []), execution])
  }
  return [...groups.values()].map(group => {
    group.sort((a, b) => new Date(a.created_at || 0).getTime() - new Date(b.created_at || 0).getTime() || Number(a.id) - Number(b.id))
    const treeId = group[0].tree_id
    const replies = messages.filter(message => message.role === 'assistant' && (message.tree_id || message.treeId) === treeId)
    let tree: any = null, saved: any = {}, trace: any = {}, status: any
    let anchor: any
    for (const message of replies) {
      trace = { ...trace, ...object(message.process_trace) }
      anchor ||= byId(trace.user_message_id)
    }
    for (const execution of group) {
      const state = object(execution.state_json)
      const nextTrace = object(state.__processTrace)
      const runtime = object(state.__runtime)
      tree = object(execution.tree_json).root ? object(execution.tree_json) : tree
      anchor ||= byId(execution.message_id) || byId(nextTrace.user_message_id)
        || canonical(users.find(user => user.turn_id && user.turn_id === (execution.turn_id || nextTrace.turn_id)))
        || byId(runtime.anchorMessageId)
      // Later full snapshots supersede earlier node states. Trace-only writes
      // can enrich identity/logs but cannot erase a full execution snapshot.
      const hasNodes = Object.keys(state).some(key => !key.startsWith('__'))
      if (hasNodes || !Object.keys(saved).some(key => !key.startsWith('__'))) {
        const previousFinalResult = saved.__finalResult
        saved = { ...saved, ...state }
        // tree_completed and final_answer snapshots are persisted independently.
        // A slower tree_completed request can therefore be inserted after the
        // final_answer snapshot. Do not let that later null erase a real result.
        if (previousFinalResult != null && state.__finalResult == null) {
          saved.__finalResult = previousFinalResult
        }
        status = execution.status
      }
      trace = { ...trace, ...nextTrace }
    }
    if (!anchor) anchor = canonical([...users].reverse().find(user =>
      new Date(user.timestamp).getTime() <= new Date(group[0].created_at || 0).getTime()))
    const restored = hydrateFastPlanExecutionState({ tree_id: treeId, tree, node_states: { ...saved, __processTrace: trace }, status })
    const errorMessage = restored.status === 'failed'
      ? [...replies].reverse().find(message => message.isError || message.msg_type === 'error') : undefined
    return { ...restored, id: treeId || group[0].id, treeId,
      anchorMessageId: anchor?.id == null ? null : String(anchor.id), turnId: trace.turn_id || anchor?.turn_id,
      createdAt: group[0].created_at, lastPrompt: anchor?.content || tree?.description || '',
      presentation: tree?.metadata?.presentation || null,
      confirmedHistory: object(saved.__runtime).confirmedHistory || [],
      error: errorMessage ? { message: errorMessage.content } : null,
    }
  }).sort((a, b) => new Date(a.createdAt || 0).getTime() - new Date(b.createdAt || 0).getTime())
}
