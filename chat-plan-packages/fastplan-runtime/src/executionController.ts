import { computed, reactive, ref } from 'vue'
import { publicFastPlanModel, type FastPlanRequestModel } from './access'
import { createFastPlanClient, type FastPlanClientOptions } from './client'
import { applyFastPlanEvent, createFastPlanRuntimeState, failFastPlanPlanning, hydrateFastPlanProcessTrace, type FastPlanConfirmationPolicy, type FastPlanExecutionMode, type FastPlanRuntimeState } from './events'
import { hydrateFastPlanExecutionState, normalizeFastPlanNodeStatus } from './restore'
import { restoreExecutionHistory } from './executionHistory'
import { resolveExecutionStatus } from './executionStatus'
import { createFastPlanSseTransport, type FastPlanEventSource } from './sseTransport'
import { createFastPlanWebSocketTransport } from './transport'
import { buildFastPlanInputSlots, injectFastPlanFiles } from './inputSlots.js'
import { waitForFastPlanOperation } from './cancellation'

export type FastPlanControllerEvent = 'state-change' | 'message-sent' | 'plan-completed' | 'execution-completed' | 'human-request' | 'error'
export interface FastPlanModelConfig { id?: string; model?: string; name?: string; displayName?: string; provider?: string; baseUrl?: string; base_url?: string }
export interface FastPlanSendOptions {
  requestId?: string
  model?: string | FastPlanModelConfig
  attachments?: unknown[]
  thinking?: boolean
  userOverride?: 'plan' | 'direct'
  metadata?: Record<string, unknown>
}
export interface FastPlanExecutionControllerOptions extends FastPlanClientOptions {
  sessionId?: string; wsBase?: string; model?: string | FastPlanModelConfig; executionMode?: FastPlanExecutionMode; confirmationPolicy?: FastPlanConfirmationPolicy
  uploadBase?: string; adminBase?: string; downloadBase?: string
  WebSocket?: typeof WebSocket; EventSource?: typeof EventSource; retainConnectionOnSessionSwitch?: boolean
  streamFlushMs?: number; autoConnect?: boolean; maxReconnectAttempts?: number; baseReconnectDelayMs?: number
  state?: FastPlanRuntimeState; autoConfirmPlan?: boolean; optimisticMessages?: boolean; persistUserMessages?: boolean
  onEvent?: (name: FastPlanControllerEvent, payload: any, state: FastPlanRuntimeState) => void
}

type StreamBuffer = { tokens: string[]; timer: ReturnType<typeof setTimeout> | null }
const now = () => Date.now()
const executionEventTypes = new Set(['node_started', 'node_completed', 'node_failed', 'streaming_token', 'agent_task_progress',
  'thinking_update', 'tree_completed', 'node_skipped', 'workflow_update', 'waiting_for_human', 'reflection', 'tree_paused',
  'tree_resumed', 'final_answer', 'tree_modified'])

function parseJson(value: any, fallback: any) {
  if (typeof value !== 'string') return value ?? fallback
  try { return JSON.parse(value) } catch { return fallback }
}
const clonePlain = <T>(value: T): T => JSON.parse(JSON.stringify(value))

function messageKey(event: any, sessionId: string) {
  if (event.message_id) return `message:${event.message_id}`
  if (event.type === 'plan_generated') return `plan:${event.tree?.id || event.tree?.tree_id || sessionId}`
  if (event.type === 'tree_started' || event.type === 'tree_completed') return `${event.type}:${event.tree_id}`
  if (event.type === 'assistant_message') return `assistant:${String(event.content || '').slice(0, 200)}`
  if (event.type === 'final_answer') return `final:${event.tree_id}:${String(event.content || event.final_output?.content || '').slice(0, 200)}`
  return ''
}

export function createFastPlanExecutionController(options: FastPlanExecutionControllerOptions = {}) {
  const client = createFastPlanClient(options)
  const defaults = createFastPlanRuntimeState(options.executionMode || 'auto', options.confirmationPolicy || 'legacy')
  const suppliedState = options.state as any
  if (suppliedState) for (const [key, value] of Object.entries(defaults)) if (suppliedState[key] === undefined) suppliedState[key] = value
  const state = reactive(suppliedState || defaults) as FastPlanRuntimeState
  state.sessionId = options.sessionId || state.sessionId || suppliedState?.currentSessionId || ''
  const connected = ref(false); const loading = ref(false)
  const listeners = new Map<FastPlanControllerEvent, Set<(payload: any, state: FastPlanRuntimeState) => void>>()
  const dedupe = new Map<string, number>(); const crossTransportDedupe = new Map<string, { source: string; timestamp: number }>()
  const buffers = new Map<string, StreamBuffer>(); const sessionCache = new Map<string, any>()
  const flushMs = options.streamFlushMs ?? 100
  let completionTimer: ReturnType<typeof setTimeout> | null = null
  const completedNotifications = new Set<string>()
  let templatePending = false
  let wiredEventSource: FastPlanEventSource | null = null
  // Belongs to the runtime, never to a renderer/KeepAlive lifecycle. A new
  // signal distinguishes even A -> B -> A and repeated loads of the same ID.
  let operationScope = new AbortController()
  const nodeRetries = new Map<string, { signal: AbortSignal; node: any; treeId: string; turnId: string | null | undefined; task: Promise<any> }>()
  const nodeEventRevisions = new WeakMap<object, number>()
  let templateSave: { signal: AbortSignal; key: string; task: Promise<any> } | null = null
  let templateExecution: { signal: AbortSignal; task: Promise<any> } | null = null
  function invalidatePendingOperations() {
    const previous = operationScope
    operationScope = new AbortController()
    loading.value = false
    previous.abort()
  }
  function captureOperation() {
    const scope = operationScope
    const sessionId = state.sessionId
    return { sessionId, signal: scope.signal,
      isCurrent: () => operationScope === scope && !scope.signal.aborted && state.sessionId === sessionId }
  }
  function queueAutoConfirm(tree: any) {
    const operation = captureOperation()
    const plannedTree = state.plannedTree
    queueMicrotask(() => {
      if (!operation.isCurrent() || state.plannedTree !== plannedTree || !state.pendingAutoConfirm) return
      confirmPlan(tree).catch(error => { if (operation.isCurrent()) reportError(error) })
    })
  }

  const emit = (name: FastPlanControllerEvent, payload: any) => {
    options.onEvent?.(name, payload, state)
    for (const listener of listeners.get(name) || []) listener(payload, state)
  }
  const notify = (event: any) => emit('state-change', { event, state })

  function flushNode(nodeId: string) {
    const buffer = buffers.get(nodeId); if (!buffer) return
    if (buffer.timer) clearTimeout(buffer.timer)
    const token = buffer.tokens.join(''); buffers.delete(nodeId)
    if (token) applyFastPlanEvent(state, { type: 'streaming_token', node_id: nodeId, token })
  }
  function flushAll() { for (const id of [...buffers.keys()]) flushNode(id) }
  function bufferToken(event: any) {
    let buffer = buffers.get(event.node_id)
    if (!buffer) { buffer = { tokens: [], timer: null }; buffers.set(event.node_id, buffer) }
    buffer.tokens.push(String(event.token || ''))
    if (!buffer.timer) buffer.timer = setTimeout(() => { flushNode(event.node_id); notify(event) }, flushMs)
  }
  function isStale(event: any) {
    if (event.session_id && state.sessionId && event.session_id !== state.sessionId) return true
    return !!(event.tree_id && !['planner', 'planning'].includes(event.tree_id) && executionEventTypes.has(event.type)
      && event.type !== 'tree_started' && (!state.currentTreeId || event.tree_id !== state.currentTreeId))
  }
  function isDuplicate(event: any) {
    const key = messageKey(event, state.sessionId); if (!key) return false
    const timestamp = Date.now(); const previous = dedupe.get(key)
    if (previous && (!key.startsWith('assistant:') || timestamp - previous < 1500)) return true
    dedupe.set(key, timestamp)
    if (dedupe.size > 200) for (const [item, timestamp] of dedupe) if (Date.now() - timestamp > 60000) dedupe.delete(item)
    return false
  }

  function crossTransportKey(event: any) {
    if (!event?.type || !event.node_id) return ''
    return [event.type, event.tree_id || '', event.node_id, event.message_id || '', event.token || '', event.phase || '',
      event.output || '', event.error || '', event.content || event.data?.content || ''].join('|')
  }
  function isCrossTransportDuplicate(event: any, source: string) {
    if (!source) return false
    const key = crossTransportKey(event); if (!key) return false
    const previous = crossTransportDedupe.get(key); const timestamp = Date.now()
    crossTransportDedupe.set(key, { source, timestamp })
    if (crossTransportDedupe.size > 500) for (const [item, value] of crossTransportDedupe) if (timestamp - value.timestamp > 2000) crossTransportDedupe.delete(item)
    return !!previous && previous.source !== source && timestamp - previous.timestamp < 1000
  }

  function emitExecutionCompleted(event: any) {
    const treeId = event.tree_id || state.currentTreeId || 'unknown'
    if (event.tree_id && event.tree_id !== state.currentTreeId) return
    if (completedNotifications.has(treeId)) return
    completedNotifications.add(treeId)
    emit('execution-completed', { event, finalResult: state.finalResult, files: state.resultFiles })
  }
  function syncExecutionHistory() {
    if (!state.plannedTree && !state.planningError && !state.routeFlow.length && !state.planningLogs.length) return
    const anchorMessageId = state.activeUserMessageId || [...state.messages].reverse().find(message => message.role === 'user' && !message.isResponse)?.id || null
    const entry = { id: state.currentTreeId || `turn:${anchorMessageId}`, treeId: state.currentTreeId || null, anchorMessageId,
      status: state.planningError ? 'failed' : resolveExecutionStatus(state), executionStatus: state.executionStatus,
      plannedTree: clonePlain(state.plannedTree), executionState: clonePlain(state.executionState), taskBubbles: [...state.taskBubbles],
      lastPrompt: state.lastPrompt, presentation: clonePlain(state.presentation),
      routeFlow: clonePlain(state.routeFlow), validationFlow: clonePlain(state.validationFlow),
      thinkingSteps: clonePlain(state.thinkingSteps), thinkingStream: state.thinkingStream,
      processTrace: clonePlain(state.processTrace), planningError: clonePlain(state.planningError),
      error: state.error ? { message: state.error.message || String(state.error) } : null,
      confirmedHistory: clonePlain(state.confirmedHistory),
      planningLogs: clonePlain(state.planningLogs), finalResult: clonePlain(state.finalResult), resultFiles: clonePlain(state.resultFiles),
      treeCompleted: state.treeCompleted, isPlanning: state.isPlanning, isPreviewing: false, isPaused: state.isPaused, executionMode: state.executionMode }
    const index = state.executionHistory.findIndex(item => entry.treeId ? item.treeId === entry.treeId : item.id === entry.id)
    index < 0 ? state.executionHistory.push(entry) : Object.assign(state.executionHistory[index], entry)
  }

  async function saveSnapshotState(sessionId: string, target: FastPlanRuntimeState, status?: string) {
    const treeId = target.currentTreeId || target.plannedTree?.id || target.plannedTree?.tree_id
    if (!sessionId || !treeId) return
    const nodeStates: Record<string, any> = {}
    for (const [id, node] of Object.entries(target.executionState)) nodeStates[id] = {
      ...node, streamContent: String(node.streamContent || '').slice(0, 5000), logs: (node.logs || []).slice(-50),
    }
    const anchorMessageId = target.activeUserMessageId || [...target.messages].reverse().find(message => message.role === 'user' && !message.isResponse)?.id || null
    await client.saveExecutionSnapshot(sessionId, { tree_id: treeId, tree_json: target.plannedTree,
      state_json: { ...nodeStates, __finalResult: target.finalResult, __resultFiles: target.resultFiles, __planningLogs: target.planningLogs,
        __processTrace: target.processTrace || { route_stages: target.routeFlow, validation_flow: target.validationFlow,
          thinking_steps: target.thinkingSteps, thinking_stream: target.thinkingStream, planning_logs: target.planningLogs },
        __runtime: {
          executionStatus: target.executionStatus,
          isPlanning: target.isPlanning,
          isPreviewing: target.isPreviewing,
          isPaused: target.isPaused,
          pendingAutoConfirm: target.pendingAutoConfirm,
          humanRequest: target.humanRequest,
          humanRequestQueue: target.humanRequestQueue,
          humanInterventionRequired: target.humanInterventionRequired,
          executionMode: target.executionMode,
          confirmationPolicy: target.confirmationPolicy,
          stepConfirmNodeId: target.stepConfirmNodeId,
          lastCompletedNodeId: target.lastCompletedNodeId,
          confirmedHistory: target.confirmedHistory,
          anchorMessageId,
        } },
      status: target.treeCompleted || target.executionStatus ? resolveExecutionStatus(target) : status || resolveExecutionStatus(target) })
  }
  const saveSnapshot = (status?: string) => saveSnapshotState(state.sessionId, state, status)

  async function processEvent(event: any, fromSessionId = state.sessionId, source = '') {
    if (fromSessionId !== state.sessionId) {
      const cached = sessionCache.get(fromSessionId)
      if (cached) {
        const stale = event.tree_id && !['planner', 'planning'].includes(event.tree_id) && executionEventTypes.has(event.type)
          && event.type !== 'tree_started' && cached.currentTreeId && event.tree_id !== cached.currentTreeId
        if (!stale) {
          applyFastPlanEvent(cached, event)
          if (['node_completed', 'node_failed', 'tree_completed', 'final_answer'].includes(event.type))
            saveSnapshotState(fromSessionId, cached, event.type === 'final_answer' ? 'completed' : undefined).catch(reportError)
        }
      }
      return false
    }
    const historical = event.tree_id && event.tree_id !== state.currentTreeId
      ? state.executionHistory.find(item => item.treeId === event.tree_id) : null
    if (historical && ['final_answer', 'tree_completed', 'node_completed', 'node_failed'].includes(event.type)) {
      // Completion/result events can arrive after the next prompt was submitted.
      // Apply them only to the archived turn and its own message slice.
      const start = state.messages.findIndex(message => String(message.id) === String(historical.anchorMessageId))
      const end = state.messages.findIndex((message, index) => index > start && message.role === 'user' && !message.isResponse)
      const previous = state.messages.slice(start < 0 ? 0 : start, end < 0 ? undefined : end)
      const target = Object.assign(createFastPlanRuntimeState(state.executionMode, state.confirmationPolicy), historical,
        { sessionId: state.sessionId, activeUserMessageId: historical.anchorMessageId, messages: [...previous] })
      applyFastPlanEvent(target, event)
      const { messages: updated, executionHistory: _history, ...snapshot } = target
      Object.assign(historical, snapshot, { status: resolveExecutionStatus(target) })
      if (start >= 0) state.messages.splice(start, previous.length, ...updated)
      await saveSnapshotState(state.sessionId, target).catch(reportError)
      notify({ type: 'history_updated', tree_id: event.tree_id })
      return true
    }
    if (isStale(event) || isDuplicate(event) || isCrossTransportDuplicate(event, source)) return false
    const eventNode = event.node_id ? state.executionState[event.node_id] : undefined
    if (eventNode) nodeEventRevisions.set(eventNode, (nodeEventRevisions.get(eventNode) || 0) + 1)
    if (event.type === 'streaming_token') { bufferToken(event); return true }
    if (event.node_id) flushNode(event.node_id)
    if (event.type === 'tree_completed') flushAll()
    applyFastPlanEvent(state, event)

    if (event.type === 'plan_generated') {
      emit('plan-completed', event.tree)
      if (state.isPreviewing) saveSnapshot('planned').catch(reportError)
      else if (options.autoConfirmPlan !== false) queueAutoConfirm(event.tree)
    } else if (event.type === 'tree_started' || event.type === 'node_started') {
      // Persist the active node as well as completions so a cold remount does not
      // reconstruct an already-running task as entirely pending.
      saveSnapshot('running').catch(reportError)
    } else if (event.type === 'node_completed') {
      const backendControlsStrictStep = state.confirmationPolicy === 'strict' && state.executionMode === 'step'
      if (state.isPaused && state.currentTreeId && !backendControlsStrictStep) pauseExecution()
      saveSnapshot('running').catch(reportError)
    } else if (event.type === 'waiting_for_human') {
      // Only legacy, untyped checkpoints retain the old auto-continue behavior.
      // Structured input/approval/choice interactions always require a person.
      if (state.executionMode === 'auto' && !event.interaction) {
        state.humanRequest = null; state.humanRequestQueue = []; state.isPaused = false
        respondHumanCheckpoint({ value: 'continue', action: 'continue' }, event).catch(reportError)
      } else {
        saveSnapshot('paused').catch(reportError)
        emit('human-request', event)
      }
    } else if (event.type === 'node_failed') {
      saveSnapshot(state.isPaused ? 'paused' : 'running').catch(reportError)
      emit('error', event)
    } else if (event.type === 'tree_paused') {
      saveSnapshot('paused').catch(reportError)
    } else if (event.type === 'tree_completed') {
      syncExecutionHistory()
      saveSnapshot().catch(reportError)
      if (completionTimer) clearTimeout(completionTimer)
      completionTimer = setTimeout(() => { completionTimer = null; emitExecutionCompleted(event) }, 750)
    } else if (event.type === 'final_answer') {
      syncExecutionHistory()
      saveSnapshot('completed').catch(reportError)
      if (completionTimer) { clearTimeout(completionTimer); completionTimer = null }
      emitExecutionCompleted(event)
    } else if (event.type === 'error') emit('error', event)
    notify(event); return true
  }

  function normalizeError(error: any, fallback = 'FastPlan 执行失败') {
    if (error instanceof Error) return error as Error & { code?: string; recoverable?: boolean; terminal?: boolean }
    const message = typeof error?.message === 'string' && error.message.trim() ? error.message : fallback
    const normalized: Error & { code?: string; recoverable?: boolean; terminal?: boolean; cause?: unknown } = new Error(message)
    if (typeof error?.code === 'string') normalized.code = error.code
    normalized.cause = error
    return normalized
  }
  function reportError(error: any) {
    const normalized = normalizeError(error)
    failFastPlanPlanning(state, normalized)
    state.error = normalized; emit('error', normalized); notify({ type: 'controller_error', error: normalized })
    return normalized
  }
  function settleError(error: any, fallback = 'FastPlan 执行失败') {
    const normalized = normalizeError(error, fallback)
    normalized.terminal = true; normalized.recoverable = false
    failFastPlanPlanning(state, normalized)
    const endedAt = now()
    const interrupted = normalized.code === 'FASTPLAN_RECONNECT_FAILED'
      ? '连接已中断，无法确认后端最终执行状态'
      : normalized.message
    for (const node of Object.values(state.executionState || {}) as any[]) {
      if (node?.status === 'running') Object.assign(node, {
        status: 'failed', error: interrupted, endTime: node.endTime || endedAt,
        durationMs: node.durationMs || (node.startTime ? Math.max(0, endedAt - Number(node.startTime)) : null),
        transportInterrupted: normalized.code === 'FASTPLAN_RECONNECT_FAILED',
      })
    }
    Object.assign(state, { error: normalized, connectionError: normalized, isPlanning: false, isPreviewing: false,
      isPaused: false, pendingAutoConfirm: false })
    emit('error', normalized); notify({ type: 'controller_error', error: normalized })
    return normalized
  }
  function handleTransportError(error: any, sessionId: string) {
    if (sessionId !== state.sessionId) return
    const normalized = normalizeError(error, 'FastPlan WebSocket 连接异常')
    if (normalized.code === 'FASTPLAN_CONNECTION_ERROR') {
      normalized.recoverable = true; state.connectionError = normalized; state.error = normalized
      notify({ type: 'connection_error', error: normalized }); return
    }
    reportError(normalized)
  }
  const socket = createFastPlanWebSocketTransport({ wsBase: options.wsBase, WebSocket: options.WebSocket, access: options.access,
    maxReconnectAttempts: options.maxReconnectAttempts, baseReconnectDelayMs: options.baseReconnectDelayMs,
    shouldReconnect: id => id === state.sessionId && (state.isPlanning || state.isPreviewing || state.isPaused),
    onConnectionChange: (id, value, detail) => {
      if (id !== state.sessionId) return
      connected.value = value
      state.connectionStatus = detail?.status || (value ? 'connected' : 'idle')
      state.reconnectAttempt = detail?.attempt || 0
      if (state.connectionStatus === 'connected') {
        const previous = state.connectionError
        state.connectionError = null
        if (state.error === previous) state.error = null
        if (state.pendingAutoConfirm && state.plannedTree) {
          queueAutoConfirm(state.plannedTree)
        }
      } else if (state.connectionStatus === 'reconnecting') {
        const transient: any = new Error(`FastPlan 连接中断，正在重连（第 ${state.reconnectAttempt} 次）`)
        transient.code = 'FASTPLAN_RECONNECTING'; transient.recoverable = true
        state.connectionError = transient; state.error = transient
      } else if (state.connectionStatus === 'failed') {
        const failed: any = new Error('FastPlan WebSocket 重连失败，请检查服务后重试')
        failed.code = 'FASTPLAN_RECONNECT_FAILED'
        settleError(failed, failed.message)
        return
      }
      notify({ type: 'connection_status', ...detail })
    },
    onMessage: (event, id) => { processEvent(event, id, 'ws').catch(reportError) }, onError: handleTransportError })
  const sse = createFastPlanSseTransport({ apiBase: options.apiBase, EventSource: options.EventSource, access: options.access, fetch: options.fetch })

  function send(payload: unknown) {
    if (!state.sessionId) throw new Error('FastPlan sessionId is required')
    if (!socket.send(state.sessionId, payload)) throw new Error('FastPlan WebSocket is not connected')
    return true
  }
  function connect(sessionId = state.sessionId) {
    if (!sessionId) throw new Error('FastPlan sessionId is required')
    if (state.sessionId !== sessionId) invalidatePendingOperations()
    if (state.sessionId && state.sessionId !== sessionId && !options.retainConnectionOnSessionSwitch) socket.close(state.sessionId)
    state.sessionId = sessionId; socket.connect(sessionId)
    openEvents(sessionId)
  }
  function openEvents(sessionId = state.sessionId) {
    if (!sessionId) throw new Error('FastPlan sessionId is required')
    const source = sse.open(sessionId)
    if (wiredEventSource === source) return source
    wiredEventSource = source
    for (const name of ['execution_log', 'thinking', 'message', 'node_completed', 'system', 'tool_missing']) source.addEventListener(name, (raw: MessageEvent) => {
      try {
        const event = JSON.parse(raw.data)
        if (!event.type && name === 'thinking') event.type = 'thinking_update'
        else if (!event.type && name !== 'message') event.type = name
        processEvent(event, sessionId, 'sse').catch(reportError)
      } catch (error) { reportError(error) }
    })
    return source
  }
  function closeEvents() { sse.close(); wiredEventSource = null }
  function disconnect(all = true) { invalidatePendingOperations(); flushAll(); if (completionTimer) clearTimeout(completionTimer); completionTimer = null; all ? socket.closeAll() : socket.close(state.sessionId); closeEvents(); connected.value = false }
  function closeSession(sessionId: string) {
    if (!sessionId) return
    if (sessionId === state.sessionId) invalidatePendingOperations()
    socket.close(sessionId)
    if (sessionId === state.sessionId) { closeEvents(); connected.value = false }
  }

  function resetForSession(sessionId: string) {
    invalidatePendingOperations()
    const mode = state.executionMode; const policy = state.confirmationPolicy
    Object.assign(state, createFastPlanRuntimeState(mode, policy)); state.sessionId = sessionId
    if ('currentSessionId' in state) (state as any).currentSessionId = sessionId
    dedupe.clear(); crossTransportDedupe.clear(); completedNotifications.clear(); templatePending = false; flushAll()
  }
  function hydrateSession(data: any) {
    state.messages = (data?.messages || []).map((item: any, index: number) => {
      const metadata = parseJson(item.metadata, {})
      return { id: item.id !== null && item.id !== undefined && item.id !== '' ? String(item.id) : `message-${index}`, role: item.role, content: item.content,
        timestamp: new Date(item.created_at || item.timestamp || Date.now()), ...metadata,
        model_id: item.model_id ?? undefined, model_name: item.model_name ?? undefined,
        isError: metadata.isError === true || metadata.msg_type === 'error' }
    })
    if ('todos' in state) (state as any).todos = Array.isArray(data?.todos) ? data.todos : []
    const executions = Array.isArray(data?.executions) ? data.executions : []
    const canonicalUser = (candidate: any) => {
      if (!candidate) return null
      const index = state.messages.indexOf(candidate)
      let result = candidate
      for (let cursor = index - 1; cursor >= 0; cursor--) {
        const previous = state.messages[cursor]
        if (previous.role !== 'user' || String(previous.content || '').trim() !== String(candidate.content || '').trim()) break
        result = previous
      }
      return result
    }
    const executionHistory = restoreExecutionHistory(executions, state.messages)
    const traceHistory = state.messages.flatMap((message: any, index: number) => {
      const trace = message.process_trace
      if (!trace || message.role === 'user') return []
      const preceding = canonicalUser([...state.messages.slice(0, index)].reverse().find((item: any) => item.role === 'user'))
      const tracedUser = canonicalUser(state.messages.find((item: any) => String(item.id) === String(trace.user_message_id)))
      const anchorMessageId = tracedUser?.id || preceding?.id
      if (!anchorMessageId) return []
      return [{ id: trace.turn_id || `trace-${message.id}`, treeId: null, anchorMessageId: String(anchorMessageId),
        createdAt: message.timestamp, status: trace.status || 'completed', plannedTree: null, executionState: {}, taskBubbles: [],
        routeFlow: trace.route_stages || [], validationFlow: trace.validation_flow || [], thinkingSteps: trace.thinking_steps || [],
        thinkingStream: trace.thinking_stream || '', planningLogs: trace.planning_logs || [], processTrace: trace,
        planningError: String(trace.status || '').toLowerCase() === 'failed'
          ? { message: trace.error || message.content || '任务规划失败', persisted: true }
          : null,
        treeCompleted: true, isPlanning: false, isPreviewing: false, isPaused: false, executionMode: state.executionMode }]
    })
    const plannedAnchors = new Set(executionHistory.map((item: any) => String(item.anchorMessageId)))
    state.executionHistory = [...executionHistory, ...traceHistory.filter((item: any) => !plannedAnchors.has(String(item.anchorMessageId)))]
      .sort((a: any, b: any) => new Date(a.createdAt || 0).getTime() - new Date(b.createdAt || 0).getTime())
    const latest = executionHistory[executionHistory.length - 1]
    if (latest) {
      Object.assign(state, latest)
      state.activeUserMessageId = latest.anchorMessageId || null
      state.humanRequest = state.humanRequestQueue[0] || null
    }
    const activeTrace = parseJson(data?.metadata, {})?.active_process_trace
    if (activeTrace) {
      hydrateFastPlanProcessTrace(state, activeTrace)
      state.activeUserMessageId = activeTrace.user_message_id == null ? null : String(activeTrace.user_message_id)
      state.isPlanning = activeTrace.status === 'running'
      state.plannedTree = null; state.executionState = {}; state.currentTreeId = ''; state.treeCompleted = false; state.executionStatus = null
      state.finalResult = null; state.resultFiles = []; state.isPreviewing = false
    } else if (!latest && traceHistory.length) {
      hydrateFastPlanProcessTrace(state, traceHistory[traceHistory.length - 1].processTrace)
      state.isPlanning = false
    }
  }

  function mergeHydratedExecutionState(incoming: any) {
    const currentTreeId = String(state.currentTreeId || state.plannedTree?.id || state.plannedTree?.tree_id || '')
    const incomingTreeId = String(incoming.currentTreeId || incoming.plannedTree?.id || incoming.plannedTree?.tree_id || '')
    const sameTree = !currentTreeId || !incomingTreeId || currentTreeId === incomingTreeId
    const currentNodes = state.executionState || {}
    const incomingNodes = incoming.executionState || {}
    const mergedNodes: Record<string, any> = { ...currentNodes }
    const terminal = new Set(['completed', 'failed', 'skipped'])
    for (const [id, nextNode] of Object.entries(incomingNodes) as Array<[string, any]>) {
      const previous = currentNodes[id]
      if (!previous) {
        mergedNodes[id] = nextNode
        continue
      }
      const previousStatus = normalizeFastPlanNodeStatus(previous.status)
      const nextStatus = normalizeFastPlanNodeStatus(nextNode?.status)
      // A slower snapshot must never roll a live/completed node backwards.
      if ((terminal.has(previousStatus) && !terminal.has(nextStatus))
        || (previousStatus === 'running' && nextStatus === 'pending')) {
        mergedNodes[id] = { ...nextNode, ...previous, status: previousStatus }
      } else {
        mergedNodes[id] = { ...previous, ...nextNode, status: nextStatus }
      }
    }
    const preserveTerminal = sameTree && state.treeCompleted && !incoming.treeCompleted
    const terminalState = preserveTerminal ? {
      treeCompleted: state.treeCompleted,
      executionStatus: state.executionStatus,
      finalResult: state.finalResult,
      resultFiles: state.resultFiles,
      isPlanning: false,
      isPreviewing: false,
      isPaused: false,
    } : null
    Object.assign(state, incoming, {
      executionState: mergedNodes,
      taskBubbles: [...new Set([...(state.taskBubbles || []), ...(incoming.taskBubbles || [])])],
    })
    if (terminalState) Object.assign(state, terminalState)
  }

  async function loadSession(sessionId: string, config: { connect?: boolean; force?: boolean } = {}) {
    if (!sessionId) throw new Error('FastPlan sessionId is required')
    const previousSessionId = state.sessionId
    if (previousSessionId && previousSessionId !== sessionId) {
      sessionCache.set(previousSessionId, JSON.parse(JSON.stringify(state)))
      closeEvents()
      if (!options.retainConnectionOnSessionSwitch) socket.close(previousSessionId)
    }
    resetForSession(sessionId); loading.value = true
    const operation = captureOperation()
    try {
      const cached = !config.force && sessionCache.get(sessionId)
      if (cached) Object.assign(state, cached)
      else {
        // Session history and current execution state are independent reads.
        // Run them together so managed access performs one in-flight readiness check.
        const [sessionResult, executionResult] = await Promise.allSettled([
          client.getSession(sessionId, operation.signal),
          client.getExecutionState(sessionId, operation.signal),
        ])
        if (!operation.isCurrent()) return state
        if (sessionResult.status === 'rejected') throw sessionResult.reason
        hydrateSession(sessionResult.value)
        const activeTrace = state.isPlanning && state.processTrace?.status === 'running'
        if (executionResult.status === 'fulfilled' && executionResult.value?.has_state !== false) {
          const execution = executionResult.value
          const runtime = execution?.node_states?.__runtime || {}
          const sameTurn = runtime.anchorMessageId != null && state.activeUserMessageId != null
            && String(runtime.anchorMessageId) === String(state.activeUserMessageId)
          const executionStatus = String(execution?.status || '').toLowerCase()
          const executionIsActive = ['planned', 'paused', 'running', 'executing'].includes(executionStatus)
          // A stale terminal execution must not hide a newer active planning trace.
          if (!activeTrace || sameTurn || executionIsActive) {
            mergeHydratedExecutionState(hydrateFastPlanExecutionState(execution))
          }
        }
      }
      const hasActiveTrace = state.isPlanning && state.processTrace?.status === 'running'
      if (cached && !hasActiveTrace) {
        try {
          const execution = await client.getExecutionState(sessionId, operation.signal)
          if (!operation.isCurrent()) return state
          if (execution?.has_state !== false) mergeHydratedExecutionState(hydrateFastPlanExecutionState(execution))
        } catch {}
      }
      if (!operation.isCurrent()) return state
      if (config.connect !== false) connect(sessionId)
      notify({ type: 'session_loaded', session_id: sessionId }); return state
    } finally { if (operation.isCurrent()) loading.value = false }
  }

  const modelPayload = (override?: string | FastPlanModelConfig) => {
    const value = override || options.model || ''
    return typeof value === 'string'
      ? { model: value, base_url: undefined, model_id: value, model_name: value }
      : { model: value.model, base_url: value.baseUrl || value.base_url, model_id: value.id, model_name: value.name || value.displayName || value.model }
  }
  function sendChat(content: string, config: FastPlanSendOptions = {}) {
    const text = content.trim(); if (!text) return false
    invalidatePendingOperations()
    const operation = captureOperation()
    if (options.access) {
      const access = options.access
      return (async () => {
        try {
          const model = await waitForFastPlanOperation(access.resolveModel(config.model || options.model), operation.signal)
          if (!operation.isCurrent()) return false
          return sendResolvedChat(text, config, model)
        } catch (error) {
          if (!operation.isCurrent()) return false
          throw settleError(error, 'FastPlan 模型配置读取失败')
        }
      })()
    }
    return sendResolvedChat(text, config)
  }
  function sendResolvedChat(text: string, config: FastPlanSendOptions, model?: FastPlanRequestModel) {
    // Enter planning optimistically so the model header and initial planning
    // placeholder render in the same frame as the user's submitted message.
    syncExecutionHistory()
    state.confirmedHistory = []
    state.activeUserMessageId = `temporary-${state.sessionId}-${config.requestId || globalThis.crypto.randomUUID()}`
    applyFastPlanEvent(state, { type: 'planning_start' })
    state.lastPrompt = text
    if (options.optimisticMessages !== false) state.messages.push({ id: state.activeUserMessageId, role: 'user', content: text, attachments: config.attachments || [], timestamp: new Date() })
    const publicModel = publicFastPlanModel(config.model || options.model)
    const publicIdentity = typeof publicModel === 'string'
      ? { model_id: publicModel, model_name: publicModel }
      : { model_id: publicModel?.id, model_name: publicModel?.name || publicModel?.displayName || publicModel?.model }
    const payload = { type: 'chat', content: text,
      ...(model ? { request_id: config.requestId || globalThis.crypto.randomUUID(), model_config: model } : modelPayload(config.model)),
      ...publicIdentity,
      attachments: config.attachments || [],
      ...(config.thinking === undefined ? {} : { thinking: config.thinking }),
      ...(config.userOverride ? { user_override: config.userOverride } : {}),
      ...(config.metadata ? { metadata: config.metadata } : {}) }
    try { send(payload) }
    catch (error) { throw settleError(error, 'FastPlan 消息发送失败') }
    // The wire payload is short lived. Never broadcast the request's credentials.
    const { model_config: _secret, ...publicPayload } = payload as any
    if (model) publicPayload.model = publicFastPlanModel(config.model || options.model)
    emit('message-sent', publicPayload); notify(publicPayload)
    if (options.persistUserMessages !== false) client.saveMessage(state.sessionId, { role: 'user', content: text, metadata: {
      attachments: config.attachments || [], ...(config.thinking === undefined ? {} : { thinking: config.thinking }),
      ...(config.userOverride ? { userOverride: config.userOverride } : {}), ...(config.metadata || {}) } }).catch(reportError)
    return true
  }
  async function confirmPlan(tree: any = state.plannedTree, attachments: any[] = []) {
    if (!tree) throw new Error('No FastPlan tree to confirm')
    const operation = captureOperation()
    let slots = tree.metadata?.input_slots
    if (!Array.isArray(slots) || !slots.length) { slots = buildFastPlanInputSlots(tree); tree.metadata = { ...(tree.metadata || {}), input_slots: slots } }
    const nodeById = (id: string) => { let found: any; const walk = (node: any) => { if (!node || found) return; if (node.id === id) found = node; else for (const child of node.children || []) walk(child) }; walk(tree.root); return found }
    const missingSlots = slots.filter((slot: any) => {
      const node = nodeById(slot.node_id); const key = slot.arg_key
      return !node?.node_type?.config?.arguments?.[key] && !node?.metadata?.[key]
    })
    if (missingSlots.length) {
      const normalizedFiles = attachments.map(file => ({ ...file, name: file.name || file.filename, path: file.path || file.abs_path || file.url }))
      const validationTree = { ...tree, metadata: { ...(tree.metadata || {}), input_slots: missingSlots } }
      const report = injectFastPlanFiles(validationTree, normalizedFiles); if (report.emptySlots.length) {
      state.isPreviewing = true; const error: any = new Error(`Missing required inputs: ${report.emptySlots.map((item: any) => item.label).join(', ')}`)
      error.code = 'FASTPLAN_INPUT_REQUIRED'; error.slots = report.emptySlots; throw error
    } }
    state.isPreviewing = false; state.pendingAutoConfirm = false
    if (templatePending) {
      templatePending = false; state.isPlanning = true
      try {
        const result = await waitForFastPlanOperation(client.executeTree(operation.sessionId, tree, operation.signal), operation.signal)
        if (!operation.isCurrent()) return false
        notify({ type: 'execute_template', tree }); return result
      } catch (error) {
        if (!operation.isCurrent()) return false
        templatePending = true; state.isPlanning = false; state.isPreviewing = true; throw error
      }
    }
    const command = state.executionMode === 'step'
      ? { type: 'confirm_plan', tree, execution_mode: 'step' }
      : { type: 'confirm_plan', tree }
    try {
      send(command); notify({ type: 'confirm_plan' }); return true
    } catch (error) {
      // A transport rejection did not accept the confirmation. Keep the same
      // preview available for retry instead of hiding its confirmation controls.
      if (operation.isCurrent()) state.isPreviewing = true
      throw error
    }
  }
  function cancelPlan() {
    // Invalidate before sending: even a disconnected stop must prevent an old
    // model/authentication callback from submitting the request later.
    invalidatePendingOperations()
    state.pendingAutoConfirm = false
    const result = send({ type: 'cancel_plan' })
    templatePending = false
    Object.assign(state, {
      isPlanning: false,
      isPreviewing: false,
      isPaused: false,
      pendingAutoConfirm: false,
      treeCompleted: true,
      executionStatus: 'cancelled',
      humanRequest: null,
      humanRequestQueue: [],
      humanInterventionRequired: false,
      stepConfirmNodeId: null,
      lastCompletedNodeId: null,
    })
    syncExecutionHistory()
    notify({ type: 'cancel_plan' })
    return result
  }
  function cancelExecution() { invalidatePendingOperations(); state.pendingAutoConfirm = false; const result = send({ type: 'cancel' }); state.isPlanning = false; state.isPreviewing = false; state.isPaused = false; notify({ type: 'cancel' }); return result }
  const pauseExecution = () => state.currentTreeId ? send({ type: 'pause_tree', tree_id: state.currentTreeId }) : false
  function resumeExecution() {
    if (!state.currentTreeId) return false
    const id = state.lastCompletedNodeId; let completedNodeName = ''
    for (const phase of state.plannedTree?.root?.children || []) { const node = phase.children?.find((item: any) => item.id === id); if (node) completedNodeName = node.name }
    const result = send({ type: 'resume_tree', tree_id: state.currentTreeId, node_id: id || undefined })
    state.confirmedHistory.push({ id: now(), timestamp: new Date(), completedNodeName })
    state.isPaused = false; state.stepConfirmNodeId = null; state.lastCompletedNodeId = null
    notify({ type: 'resume_tree', tree_id: state.currentTreeId }); return result
  }
  async function retryNode(nodeId: string) {
    const operation = captureOperation(), treeId = state.currentTreeId, turnId = state.activeUserMessageId
    const node = state.executionState[nodeId]
    if (!treeId || !node) return false
    const pending = nodeRetries.get(nodeId)
    if (pending?.signal === operation.signal && pending.node === node && pending.treeId === treeId && pending.turnId === turnId) return pending.task
    const previousStatus = node.status, previousError = node.error
    const eventRevision = nodeEventRevisions.get(node) || 0
    const isCurrent = () => operation.isCurrent() && state.currentTreeId === treeId
      && state.activeUserMessageId === turnId && state.executionState[nodeId] === node
    const job = { signal: operation.signal, node, treeId, turnId, task: Promise.resolve<any>(false) }
    nodeRetries.set(nodeId, job)
    job.task = (async () => {
      try {
        const result = await waitForFastPlanOperation(client.retryTask(treeId, nodeId, operation.signal), operation.signal)
        if (!isCurrent()) return false
        // A node event may already have supplied a newer running/completed/error state.
        if (result !== false && (nodeEventRevisions.get(node) || 0) === eventRevision
          && node.status === previousStatus && node.error === previousError) {
          Object.assign(node, { status: 'running', error: null })
          notify({ type: 'retry_node', tree_id: treeId, node_id: nodeId })
        }
        return result
      } catch (error) {
        if (!isCurrent()) return false
        throw error
      } finally { if (nodeRetries.get(nodeId) === job) nodeRetries.delete(nodeId) }
    })()
    return job.task
  }
  const skipNode = (nodeId: string) => send({ type: 'skip_node', tree_id: state.currentTreeId, node_id: nodeId })
  async function respondHumanCheckpoint(data: any, request = state.humanRequestQueue[0] || state.humanRequest) {
    if (!request) throw new Error('No pending human checkpoint')
    const operation = captureOperation()
    const turnId = state.activeUserMessageId
    const treeId = state.currentTreeId
    const queuedRequest = state.humanRequestQueue.find(item => item.node_id === request.node_id && item.tree_id === request.tree_id)
    const previousRequest = state.humanRequest
    const isCurrent = () => operation.isCurrent() && state.activeUserMessageId === turnId && state.currentTreeId === treeId
      && (!queuedRequest || state.humanRequestQueue.includes(queuedRequest))
    let result: any
    try {
      result = await waitForFastPlanOperation(client.respondCheckpoint(request.tree_id || treeId, request.node_id, data, operation.signal), operation.signal)
    } catch (error) {
      if (!isCurrent()) return false
      throw error
    }
    if (!isCurrent()) return false
    state.humanRequestQueue = state.humanRequestQueue.filter(item => item !== queuedRequest)
    if (state.humanRequest === previousRequest) state.humanRequest = state.humanRequestQueue[0] || null
    if (options.optimisticMessages !== false) {
      const values = data.values && typeof data.values === 'object' ? clonePlain(data.values) : null
      const fields = Array.isArray(request.interaction?.fields) ? request.interaction.fields : []
      const labels = new Map(fields.map((field: any) => [String(field.key), field.label || field.key]))
      const detail = values
        ? Object.entries(values).map(([key, value]) => `${labels.get(key) || key}：${String(value)}`).join('；')
        : String(data.value || data.action || data.response || 'continue')
      const isInput = request.interaction?.kind === 'input' || !!values
      state.confirmedHistory.push({
        id: now(), timestamp: new Date(),
        kind: isInput ? 'checkpoint_input' : 'checkpoint_decision',
        nodeId: request.node_id,
        title: isInput ? '已补充执行信息' : '已完成人工确认',
        completedNodeName: request.title || request.node_name || request.prompt || request.question || '',
        prompt: request.prompt || request.question || request.message || '',
        values,
        detail,
        decision: isInput ? undefined : data.value || data.action || data.response,
      })
    }
    return result
  }
  function findPlannedNode(nodeId: string) {
    const find = (node: any): any => node?.id === nodeId ? node : node?.children?.map(find).find(Boolean)
    return find(state.plannedTree?.root)
  }
  function updateNodeOutput(nodeId: string, output: any) {
    if (!state.currentTreeId || (!state.executionState[nodeId] && !findPlannedNode(nodeId))) return false
    const snapshot = output === undefined ? undefined : clonePlain(output)
    const event = { type: 'edit_node', tree_id: state.currentTreeId, node_id: nodeId, changes: { output: snapshot } }
    const result = send(event)
    if (state.executionState[nodeId]) state.executionState[nodeId].output = snapshot
    notify(event)
    return result
  }
  function updateNextNodeConfig(nodeId: string, changes: any) {
    const node = findPlannedNode(nodeId)
    if (!node) throw new Error(`Unknown FastPlan node: ${nodeId}`)
    if (!state.currentTreeId) return false
    const nodeType = { ...(node.node_type || {}), config: { ...(node.node_type?.config || {}), ...clonePlain(changes) } }
    const event = { type: 'edit_node', tree_id: state.currentTreeId, node_id: nodeId, changes: { node_type: nodeType } }
    const result = send(event)
    node.node_type = nodeType
    notify(event)
    return result
  }
  async function executeTemplate(template: any, attachments: any[] = [], config: { preview?: boolean } = {}) {
    if (templateExecution?.signal === operationScope.signal) return false
    const tree = clonePlain(template.tree_json || template.tree || template)
    invalidatePendingOperations()
    if (completionTimer) { clearTimeout(completionTimer); completionTimer = null }
    syncExecutionHistory()
    applyFastPlanEvent(state, { type: 'planning_start' })
    state.confirmedHistory = []; state.canSaveTemplate = false; state.templateTreeData = null
    state.currentTreeId = String(tree?.id || tree?.tree_id || '')
    state.isPlanning = false
    templatePending = true
    state.plannedTree = tree; state.presentation = tree?.metadata?.presentation || null
    state.isPreviewing = !!config.preview || state.executionMode === 'manual' || (state.confirmationPolicy === 'strict' && state.executionMode === 'step')
    if (state.isPreviewing) { notify({ type: 'template_preview', tree }); return tree }
    const job = { signal: operationScope.signal, task: confirmPlan(tree, attachments) }
    templateExecution = job
    try { return await job.task }
    finally { if (templateExecution === job) templateExecution = null }
  }
  async function saveAsTemplate(name: string, description = '', tree: any = state.plannedTree) {
    if (!tree) throw new Error('No FastPlan tree to save'); const copy = clonePlain(tree); copy.metadata = { ...(copy.metadata || {}), input_slots: copy.metadata?.input_slots || buildFastPlanInputSlots(copy) }
    const operation = captureOperation(), plannedTree = state.plannedTree, templateData = state.templateTreeData
    const turnId = state.activeUserMessageId, treeId = state.currentTreeId
    const payload = { name, description, tree_json: copy }, key = JSON.stringify(payload)
    if (templateSave?.signal === operation.signal) return templateSave.key === key ? templateSave.task : false
    const isCurrent = () => operation.isCurrent() && state.plannedTree === plannedTree && state.templateTreeData === templateData
      && state.currentTreeId === treeId && state.activeUserMessageId === turnId
    const job = { signal: operation.signal, key, task: Promise.resolve<any>(false) }
    templateSave = job
    job.task = (async () => {
      try {
        const result = await waitForFastPlanOperation(client.createTemplate(payload, operation.signal), operation.signal)
        if (!isCurrent()) return false
        if (result !== false && (tree === plannedTree || tree === templateData)) {
          state.canSaveTemplate = false; state.templateTreeData = null
          notify({ type: 'template_saved' })
        }
        return result
      } catch (error) {
        if (!isCurrent()) return false
        throw error
      } finally { if (templateSave === job) templateSave = null }
    })()
    return job.task
  }
  function on(name: FastPlanControllerEvent, listener: (payload: any, state: FastPlanRuntimeState) => void) {
    if (!listeners.has(name)) listeners.set(name, new Set()); listeners.get(name)!.add(listener); return () => listeners.get(name)?.delete(listener)
  }
  function setExecutionMode(mode: FastPlanExecutionMode) { state.executionMode = mode; notify({ type: 'execution_mode_changed', mode }) }

  if (options.autoConnect && state.sessionId) {
    const operation = captureOperation()
    queueMicrotask(() => {
      if (operation.isCurrent()) loadSession(operation.sessionId).catch(reportError)
    })
  }
  return { state, client, connected, loading, connect, openEvents, closeEvents, closeSession, disconnect, loadSession, processEvent, saveSnapshot, on, setExecutionMode, sendCommand: send,
    sendChat, confirmPlan, cancelPlan, cancelExecution, pauseExecution, resumeExecution, retryNode, skipNode,
    respondHumanCheckpoint, updateNodeOutput, updateNextNodeConfig, executeTemplate, saveAsTemplate,
    planStatus: computed(() => state.treeCompleted ? resolveExecutionStatus(state) : state.isPaused ? 'paused' : state.isPreviewing ? 'awaiting_confirmation' : state.isPlanning ? 'executing' : 'idle') }
}

export type FastPlanExecutionController = ReturnType<typeof createFastPlanExecutionController>
