import type { FastPlanTaskPlanExecution, FastPlanTaskPlanNodeStatus, FastPlanTaskPlanStatus } from './planTypes'
import { formatFastPlanFinalContent } from './finalAnswer'
import { normalizeFastPlanNodeStatus } from './restore'

/** Converts FastPlan persisted/runtime state into chat-renderer's public protocol. */
const localId = (message: any, index: number) => {
  const value = message.id
  return value !== null && value !== undefined && value !== ''
    ? String(value)
    : `fastplan-${message.role || 'message'}-${new Date(message.timestamp || 0).getTime() || index}-${index}`
}

export function adaptFastPlanMessage(message: any, index: number, options: any = {}) {
  const parts: any[] = []
  if (message.isOutline && message.outlineData) parts.push({ type: 'custom', kind: 'fastplan-outline', data: message })
  else if (message.isDocument && message.documentData) parts.push({ type: 'custom', kind: 'fastplan-document', data: message })
  else {
    if (message.content) parts.push({ type: 'text', text: message.content, format: message.role === 'assistant' ? 'markdown' : 'plain' })
    for (const attachment of message.attachments || []) {
      parts.push({ type: 'attachment', name: attachment.filename || attachment.name || '附件', url: attachment.url, size: attachment.size, mimeType: attachment.mimeType })
    }
  }
  const avatar = typeof options.modelAvatar === 'function' ? options.modelAvatar(options.modelName || '') : options.modelAvatar
  const modelId = message.model_id || options.modelId
  const modelName = message.model_name || message.model_id || options.modelName || 'FastPlan'
  return {
    protocolVersion: 1,
    id: localId(message, index),
    role: ['user', 'assistant', 'system', 'tool'].includes(message.role) ? message.role : 'system',
    createdAt: message.timestamp || Date.now(),
    model: message.role === 'assistant' ? { id: modelId, name: modelName, avatar } : undefined,
    status: message.isError ? 'error' : 'complete',
    parts,
    metadata: {
      contentKind: message.contentKind === 'final_answer' || message.isFinalAnswer === true
        ? 'final_answer'
        : 'ordinary_message',
      fastPlanMessage: message,
      fastPlanIndex: index,
    },
  }
}

export function adaptFastPlanMessages(messages: any[], options?: any) {
  if (!Array.isArray(messages)) throw new TypeError(`Invalid input: 'messages' must be an array, but received ${typeof messages}.`)
  const result: any[] = []
  for (const [index, message] of messages.entries()) {
    if (message === null || typeof message !== 'object') throw new TypeError(`Invalid message format at index ${index}: expected an object, but received ${typeof message}.`)
    if (message.isThinking) continue
    const previous = result[result.length - 1]?.metadata?.fastPlanMessage
    // WebSocket echo and optimistic persistence may contain adjacent duplicate user turns.
    if (message.role === 'user' && previous?.role === 'user'
      && (!message.turn_id || !previous.turn_id || message.turn_id === previous.turn_id)
      && (!message.request_id || !previous.request_id || message.request_id === previous.request_id)
      && String(message.content || '').trim() === String(previous.content || '').trim()) continue
    result.push(adaptFastPlanMessage(message, index, options))
  }
  mergePersistedFinalResults(result, options || {})
  return result
}

/** Uses the complete execution snapshot result for history display instead of the persisted short assistant summary. */
function mergePersistedFinalResults(messages: any[], options: any) {
  const executions = Array.isArray(options.executionHistory) ? options.executionHistory : []
  for (const execution of executions) {
    const anchorId = execution?.anchorMessageId
    if (anchorId === null || anchorId === undefined || anchorId === '') continue
    const anchorIndex = messages.findIndex(message => message.role === 'user' && String(message.id) === String(anchorId))
    if (anchorIndex < 0) continue
    const endIndex = messages.findIndex((message, index) => index > anchorIndex && message.role === 'user')
    const turnEnd = endIndex < 0 ? messages.length : endIndex
    const isCurrentExecution = execution?.treeId && options.currentTreeId
      && String(execution.treeId) === String(options.currentTreeId)
    const currentResult = isCurrentExecution && options.finalResult?.content
      ? options.finalResult
      : execution?.finalResult
    const currentFiles = isCurrentExecution && Array.isArray(options.resultFiles)
      ? options.resultFiles
      : Array.isArray(execution?.resultFiles) ? execution.resultFiles : []
    const content = formatFastPlanFinalContent(currentResult?.content, currentFiles)
    if (!content) continue

    const assistantIndexes = messages
      .map((message, index) => ({ message, index }))
      .filter(({ message, index }) => index > anchorIndex && index < turnEnd
        && message.role === 'assistant' && message.metadata?.fastPlanPlanningShell !== true)
    const explicitFinal = assistantIndexes.find(({ message }) => {
      const raw = message.metadata?.fastPlanMessage || {}
      const persistedExecutionResult = execution?.treeId
        && String(raw.treeId || raw.tree_id || '') === String(execution.treeId)
        && raw.msg_type === 'execution'
      return message.metadata?.contentKind === 'final_answer'
        || raw.isFinalAnswer === true
        || persistedExecutionResult
    })
    // A historical turn with one persisted assistant summary can be safely
    // upgraded in place. With multiple untyped assistant messages, create a
    // dedicated final answer rather than promoting an arbitrary message.
    const assistantIndex = explicitFinal?.index
      ?? (assistantIndexes.length === 1 ? assistantIndexes[0].index : -1)
    if (assistantIndex >= 0) {
      const message = messages[assistantIndex]
      const parts = [...(message.parts || [])]
      const textIndex = parts.findIndex(part => part.type === 'text')
      const textPart = { type: 'text', text: content, format: 'markdown' }
      if (textIndex >= 0) parts[textIndex] = { ...parts[textIndex], ...textPart }
      else parts.unshift(textPart)
      messages[assistantIndex] = { ...message, parts, metadata: {
        ...message.metadata,
        contentKind: 'final_answer',
        completeFinalResult: true,
      } }
      continue
    }

    messages.splice(turnEnd, 0, {
      protocolVersion: 1,
      id: `fastplan-final-${execution.treeId || anchorId}`,
      role: 'assistant',
      createdAt: currentResult.receivedAt || execution.createdAt || Date.now(),
      model: { id: options.modelId, name: options.modelName || 'FastPlan', avatar: options.modelAvatar || null },
      status: 'complete',
      parts: [{ type: 'text', text: content, format: 'markdown' }],
      metadata: { contentKind: 'final_answer', completeFinalResult: true, restoredFromExecution: true },
    })
  }
}

export function adaptFastPlanTaskPlanExecution(state: any, options: { humanRequest?: any; manualReviews?: any[] } = {}): FastPlanTaskPlanExecution | null {
  const request = options.humanRequest || state.currentHumanRequest || state.humanRequestQueue?.[0] || state.humanRequest
  const reviews = options.manualReviews || state.manualReviews || state.confirmedHistory || []
  const hasPlanContent = state.plannedTree || state.isPlanning || state.isPreviewing || state.presentation || state.routeFlow?.length
    || state.validationFlow?.length || state.planningLogs?.length || request
  if (!hasPlanContent) return null
  const normalize = (status?: string): FastPlanTaskPlanNodeStatus => {
    const normalized = normalizeFastPlanNodeStatus(status)
    return (['completed', 'failed', 'running', 'skipped'].includes(normalized) ? normalized : 'pending') as FastPlanTaskPlanNodeStatus
  }
  const asText = (entry: any) => typeof entry === 'string' ? entry : entry?.message || entry?.content || entry?.text || ''
  const adaptNodes = (nodes: any[], prefix: string, parentId: string | null = null, depth = 0): any[] => nodes.flatMap((node: any, nodeIndex: number) => {
    const execution = state.executionState?.[node.id] || {}
    const sequence = `${prefix}.${nodeIndex + 1}`
    const current = {
      id: node.id || `node-${sequence}`,
      title: node.name || `任务 ${sequence}`,
      sequence,
      depth,
      parentId,
      status: normalize(execution.status),
      progress: typeof execution.progress === 'number' ? execution.progress : undefined,
      startedAt: execution.startTime,
      completedAt: execution.endTime,
      error: execution.error,
      output: execution.output || execution.streamContent || undefined,
    }
    return [current, ...adaptNodes(node.children || [], sequence, node.id || null, depth + 1)]
  })
  const phases = (state.plannedTree?.root?.children || []).map((phase: any, phaseIndex: number) => {
    const nodes = adaptNodes(phase.children || [], String(phaseIndex + 1), phase.id || null, 0)
    const statuses = nodes.map((node: any) => node.status)
    const status = statuses.includes('running') ? 'running'
      : statuses.includes('failed') ? 'failed'
      : statuses.length > 0 && statuses.every((item: string) => item === 'completed' || item === 'skipped') ? 'completed'
      : 'pending'
    return { id: phase.id || `phase-${phaseIndex}`, title: phase.name || `阶段 ${phaseIndex + 1}`, status, nodes }
  })
  const hasFailure = phases.some((phase: any) => phase.status === 'failed')
  const normalizePlanning = (status?: string): FastPlanTaskPlanNodeStatus => state.planningError && String(status || '').toLowerCase() === 'running'
    ? 'failed'
    : normalize(status)
  return {
    id: state.currentTreeId || state.plannedTree?.id || state.plannedTree?.tree_id || undefined,
    title: state.plannedTree?.description || 'FastPlan task',
    status: (state.executionStatus || (state.planningError ? 'failed' : state.treeCompleted ? (hasFailure ? 'failed' : 'completed') : state.isPaused ? 'paused' : state.isPreviewing ? 'awaiting_confirmation' : state.isPlanning ? (state.taskBubbles?.length ? 'executing' : 'planning') : hasFailure ? 'failed' : state.routeFlow?.length ? 'completed' : 'cancelled')) as FastPlanTaskPlanStatus,
    summary: state.presentation?.plan_summary || state.presentation?.direct_result,
    phases,
    presentation: {
      prompt: state.lastPrompt,
      routeStages: (state.routeFlow || []).map((row: any, index: number) => ({ id: String(row.id || row.stage || index),
        stage: row.stage || '', label: row.summary || row.stage || `路由 ${index + 1}`, status: normalizePlanning(row.status),
        detail: row.summary || '', payload: row.payload || {}, seq: row.seq, ts: row.ts })),
      thinkingSteps: (state.thinkingSteps || []).map((step: any, index: number) => ({ id: String(step.id || index), label: step.label || `步骤 ${index + 1}`, content: step.content || '' })).filter((step: any) => step.content),
      validation: (state.validationFlow || []).map((row: any, index: number) => ({ ...row,
        id: String(row.id || row.stage || index), stage: row.stage || '', label: row.label || row.stage || `校验 ${index + 1}`,
        detail: asText(row), status: normalizePlanning(row.status) })),
      logs: (state.planningLogs || []).map((row: any, index: number) => ({ id: String(row.id || index), text: asText(row), status: normalize(row.status) })).filter((row: any) => row.text),
      inputSlots: (state.plannedTree?.metadata?.input_slots || []).map((slot: any, index: number) => ({
        id: String(slot.id || `${slot.node_id || 'node'}.${slot.arg_key || slot.name || index}`),
        label: slot.label || slot.arg_key || slot.name || `输入 ${index + 1}`,
        required: slot.required !== false,
        description: slot.description || slot.hint || (Array.isArray(slot.accept) && slot.accept.length ? `支持 ${slot.accept.join(' / ')}` : undefined),
      })),
      executionMode: state.executionMode,
      humanRequest: request ? { title: request.title, question: request.question || request.prompt || request.message || '需要您的确认', options: request.options } : undefined,
      manualReviews: reviews.map((review: any, index: number) => ({
        id: review.id || String(index), title: review.title || review.completedNodeName || '已确认继续执行',
        detail: review.detail || (review.hadEdit ? '已修改' : undefined),
      })),
    },
  }
}

export function adaptFastPlanExecutionHistory(state: any): Array<{ anchorMessageId: string; plan: FastPlanTaskPlanExecution }> {
  return (state.executionHistory || []).flatMap((execution: any) => {
    if (!execution.anchorMessageId) return []
    const plan = adaptFastPlanTaskPlanExecution(execution)
    return plan ? [{ anchorMessageId: String(execution.anchorMessageId), plan }] : []
  })
}
