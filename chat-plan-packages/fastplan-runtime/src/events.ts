import { detectFastPlanOutputFiles } from './outputs'
import { formatFastPlanFinalContent, normalizeFastPlanFinalAnswer } from './finalAnswer'
import { parseFastPlanThinking, pushFastPlanLog } from './thinking.js'
import { resolveExecutionStatus } from './executionStatus'

export type FastPlanExecutionMode = 'auto' | 'manual' | 'step'
export type FastPlanConfirmationPolicy = 'legacy' | 'strict'
export type FastPlanConnectionStatus = 'idle' | 'connecting' | 'connected' | 'reconnecting' | 'failed'

export interface FastPlanRouteStage {
  stage: string
  status: string
  summary: string
  payload: Record<string, any>
  seq?: number
  ts?: number
}

export interface FastPlanProcessTrace {
  turn_id?: string
  user_message_id?: string | number
  status?: string
  error?: string
  route_stages?: FastPlanRouteStage[]
  validation_flow?: any[]
  thinking_steps?: any[]
  thinking_stream?: string
  planning_logs?: any[]
}

export interface FastPlanRuntimeState {
  activeUserMessageId?: string | null; sessionId: string; messages: any[]; plannedTree: any; executionState: Record<string, any>; taskBubbles: string[]
  planningLogs: any[]; routeFlow: FastPlanRouteStage[]; validationFlow: any[]; thinkingSteps: any[]; thinkingStream: string; presentation: any
  processTrace: FastPlanProcessTrace | null
  finalResult: any; resultFiles: any[]; isPlanning: boolean; isPreviewing: boolean; isPaused: boolean
  treeCompleted: boolean; currentTreeId: string; humanRequest: any; humanRequestQueue: any[]
  /** Terminal outcome; null until execution ends. treeCompleted is only a lifecycle flag. */
  executionStatus: 'completed' | 'failed' | 'cancelled' | null
  humanInterventionRequired: boolean; executionMode: FastPlanExecutionMode; confirmationPolicy: FastPlanConfirmationPolicy; stepConfirmNodeId: string | null
  lastCompletedNodeId: string | null; confirmedHistory: any[]; pendingAutoConfirm: boolean; canSaveTemplate: boolean
  templateTreeData: any; lastPrompt: string; error: any; planningError: any
  executionHistory: any[]
  missingTools: any[]; sseRawLogs: any[]
  degradedSummary: string[]
  connectionStatus: FastPlanConnectionStatus; connectionError: Error | null; reconnectAttempt: number
}

export function createFastPlanRuntimeState(mode: FastPlanExecutionMode = 'auto', confirmationPolicy: FastPlanConfirmationPolicy = 'legacy'): FastPlanRuntimeState {
  return { activeUserMessageId: null, sessionId: '', messages: [], plannedTree: null, executionState: {}, taskBubbles: [], planningLogs: [], routeFlow: [],
    validationFlow: [], thinkingSteps: [], thinkingStream: '', presentation: null, processTrace: null, finalResult: null, resultFiles: [],
    isPlanning: false, isPreviewing: false, isPaused: false, treeCompleted: false, executionStatus: null, currentTreeId: '', humanRequest: null,
    humanRequestQueue: [], humanInterventionRequired: false, executionMode: mode, confirmationPolicy, stepConfirmNodeId: null,
    lastCompletedNodeId: null, confirmedHistory: [], pendingAutoConfirm: false, canSaveTemplate: false,
    templateTreeData: null, lastPrompt: '', error: null, planningError: null, executionHistory: [], missingTools: [], sseRawLogs: [], degradedSummary: [],
    connectionStatus: 'idle', connectionError: null, reconnectAttempt: 0 }
}

const now = () => Date.now()
function eventTimestamp(event: any) {
  const raw = event?.ts ?? event?.meta?.ts
  if (raw === null || raw === undefined || raw === '') return now()
  const numeric = Number(raw)
  if (Number.isFinite(numeric)) return numeric > 0 && numeric < 100000000000 ? numeric * 1000 : numeric
  const parsed = Date.parse(String(raw))
  return Number.isFinite(parsed) ? parsed : now()
}
const addMessage = (s: FastPlanRuntimeState, role: string, content: string, extra = {}) => s.messages.push({ role, content, timestamp: new Date(), ...extra })
function activeTurnAssistant(s: FastPlanRuntimeState) {
  for (let index = s.messages.length - 1; index >= 0; index -= 1) {
    const message = s.messages[index]
    if (message?.role === 'user') return null
    if (message?.role === 'assistant' && !message.isThinking) return message
  }
  return null
}
function upsertFinalAssistantMessage(s: FastPlanRuntimeState, content: string, event: any) {
  const text = String(content || '').trim()
  if (!text) return
  const current = activeTurnAssistant(s)
  if (current) Object.assign(current, { content: text, contentKind: 'final_answer', isFinalAnswer: true, treeId: event.tree_id || s.currentTreeId,
    id: event.message_id || current.id, timestamp: current.timestamp || new Date() })
  else addMessage(s, 'assistant', text, { id: event.message_id, contentKind: 'final_answer', isFinalAnswer: true, treeId: event.tree_id || s.currentTreeId })
}
const isPhase = (s: FastPlanRuntimeState, id: string) => s.plannedTree?.root?.children?.some((p: any) => p.id === id) ?? false
const newNode = (id: string, name = id) => ({ id, name, status: 'pending', logs: [], workflowNodes: [], streamContent: '', output: '', error: null, startTime: null, endTime: null, durationMs: null, confidence: null, usage: null })

export function normalizeTokenUsage(...sources: any[]) {
  const number = (...values: any[]) => {
    for (const value of values) {
      if (value === null || value === undefined || value === '') continue
      const parsed = Number(value)
      if (Number.isFinite(parsed) && parsed >= 0) return parsed
    }
    return undefined
  }
  for (let source of sources) {
    if (typeof source === 'string') { try { source = JSON.parse(source) } catch { continue } }
    if (!source || typeof source !== 'object' || Array.isArray(source)) continue
    const usage = source.usage || source.token_usage || source.agent_metrics?.usage || source.metrics?.usage || source
    const input = number(usage.input, usage.input_tokens, usage.prompt_tokens, source.input_tokens, source.prompt_tokens)
    const output = number(usage.output, usage.output_tokens, usage.completion_tokens, source.output_tokens, source.completion_tokens)
    if (input !== undefined || output !== undefined) return { ...(input !== undefined ? { input } : {}), ...(output !== undefined ? { output } : {}) }
  }
  return null
}

function parseOutputMetrics(output: any) {
  let value = output
  if (typeof value === 'string') { try { value = JSON.parse(value) } catch { value = null } }
  if (!value || typeof value !== 'object' || Array.isArray(value)) return { confidence: null, durationMs: null, usage: null }
  const number = (...candidates: any[]) => {
    for (const candidate of candidates) {
      if (candidate === null || candidate === undefined || candidate === '') continue
      const parsed = Number(candidate)
      if (Number.isFinite(parsed)) return parsed
    }
    return null
  }
  return {
    confidence: number(value.confidence, value.agent_metrics?.confidence, value.metrics?.confidence, value.metadata?.confidence),
    durationMs: number(value.duration_ms, value.elapsed_ms, value.agent_metrics?.duration_ms, value.agent_metrics?.elapsed_ms, value.metrics?.duration_ms),
    usage: normalizeTokenUsage(value),
  }
}

export function initializeExecutionNodes(s: FastPlanRuntimeState) {
  s.executionState = {}; s.taskBubbles = []
  const visit = (nodes: any[]) => { for (const node of nodes || []) {
    s.executionState[node.id] = newNode(node.id, node.name); s.taskBubbles.push(node.id); visit(node.children || [])
  } }
  for (const phase of s.plannedTree?.root?.children || []) visit(phase.children || [])
}

function upsertValidation(s: FastPlanRuntimeState, data: any) {
  const row = { ...data, stage: data.stage || '', text: data.text || data.content || '' }
  const index = s.validationFlow.findIndex(item => item.stage === row.stage)
  index < 0 ? s.validationFlow.push(row) : Object.assign(s.validationFlow[index], row)
  if (row.stage === 'plan' && ['done', 'completed', 'success'].includes(String(row.status || '').toLowerCase())) {
    completePlanningProgress(s, row.duration_ms)
  }
}

function upsertRouteStage(s: FastPlanRuntimeState, event: any) {
  const row: FastPlanRouteStage = {
    stage: String(event.stage || ''),
    status: String(event.status || 'running'),
    summary: String(event.summary || ''),
    payload: event.payload && typeof event.payload === 'object' ? event.payload : {},
    seq: event.seq ?? event.meta?.seq,
    ts: event.ts ?? event.meta?.ts,
  }
  if (!row.stage) return
  const index = s.routeFlow.findIndex(item => item.stage === row.stage)
  index < 0 ? s.routeFlow.push(row) : Object.assign(s.routeFlow[index], row)
}

/** Applies a persisted backend trace to the same live state used by WebSocket events. */
export function hydrateFastPlanProcessTrace(s: FastPlanRuntimeState, trace: FastPlanProcessTrace | null | undefined) {
  if (!trace) return
  s.processTrace = trace
  s.routeFlow = Array.isArray(trace.route_stages) ? trace.route_stages.map(item => ({ ...item, payload: item.payload || {} })) : []
  s.validationFlow = Array.isArray(trace.validation_flow) ? trace.validation_flow.map(item => ({ ...item })) : []
  s.thinkingSteps = Array.isArray(trace.thinking_steps) ? trace.thinking_steps.map(item => ({ ...item })) : []
  s.thinkingStream = String(trace.thinking_stream || '')
  s.planningLogs = Array.isArray(trace.planning_logs) ? trace.planning_logs.map(item => ({ ...item })) : []
  if (String(trace.status || '').toLowerCase() === 'failed') applyPlanningFailureState(s, trace.error || '任务规划失败')
  else s.planningError = null
}

function completePlanningProgress(s: FastPlanRuntimeState, durationMs?: number) {
  const progress = s.validationFlow.find(item => item.stage === 'llm_progress')
  if (!progress) return
  Object.assign(progress, {
    status: 'done',
    text: '执行步骤编排完成',
    content: '执行步骤编排完成',
    duration_ms: durationMs ?? progress.elapsed_ms ?? progress.duration_ms,
  })
}

/** Finalizes an interrupted planning attempt so the UI cannot retain stale spinners. */
export function failFastPlanPlanning(s: FastPlanRuntimeState, error: any) {
  if (!s.isPlanning || s.plannedTree) return false
  applyPlanningFailureState(s, error)
  return true
}

function applyPlanningFailureState(s: FastPlanRuntimeState, error: any) {
  const message = String(error?.message || error?.error || error || '任务规划失败')
  const failure = {
    message,
    code: error?.code,
    statusCode: error?.statusCode ?? error?.status_code ?? error?.status,
    occurredAt: now(),
  }
  const failRunningRows = (rows: any[]) => {
    for (const row of rows || []) {
      if (String(row?.status || '').toLowerCase() === 'running') Object.assign(row, { status: 'failed', error: message })
    }
  }
  failRunningRows(s.routeFlow)
  failRunningRows(s.validationFlow)
  if (!s.validationFlow.some(row => ['request', 'llm_progress', 'plan'].includes(String(row?.stage || '')))) {
    s.validationFlow.push({ stage: 'plan', status: 'failed', text: '任务规划失败', content: '任务规划失败', error: message })
  }
  Object.assign(s, { planningError: failure, isPlanning: false, isPreviewing: false, pendingAutoConfirm: false })
}

function appendThinkingStep(s: FastPlanRuntimeState, data: any, content: string) {
  const idx = Number(data.step ?? 0)
  let item = s.thinkingSteps.find(step => Number(step.idx) === idx)
  if (!item) { item = { idx, label: data.label || '', content: '' }; s.thinkingSteps.push(item) }
  item.content += content
}

function addFiles(s: FastPlanRuntimeState, raw: unknown, nodeId = '', nodeName = '') {
  for (const file of detectFastPlanOutputFiles(typeof raw === 'string' ? raw : JSON.stringify(raw || ''))) {
    const existing = s.resultFiles.find(item => item.url === file.url)
    if (!existing) {
      s.resultFiles.push({ ...file, nodeId, nodeName })
      continue
    }
    // A streaming/summary event may announce only the URL before the structured
    // skill result arrives. Enrich the existing entry instead of discarding size/MIME.
    Object.assign(existing, {
      ...file,
      name: file.name || existing.name,
      size: file.size ?? existing.size,
      mime_type: file.mime_type || existing.mime_type,
      isArchive: file.isArchive ?? existing.isArchive,
      nodeId: nodeId || existing.nodeId,
      nodeName: nodeName || existing.nodeName,
    })
  }
}

/** Complete FastPlan protocol reducer; commands, buffering and persistence are controller concerns. */
export function applyFastPlanEvent(s: FastPlanRuntimeState, e: any) {
  if (!e?.type || e.type === 'pong') return
  switch (e.type) {
    case 'session_connected': s.isPreviewing = e.status === 'waiting_confirm'; s.isPlanning = ['planning', 'executing'].includes(e.status); return
    case 'assistant_message': {
      s.isPlanning = false
      s.messages = s.messages.filter(item => !item.isThinking)
      const current = activeTurnAssistant(s)
      // ★ 商店候选（clawhub 搜索结果）结构化附带：消息带上 install_candidates，
      //   ChatMessage → StoreCandidateList 渲染安装按钮（历史刷新走消息元数据同键）
      const extra = {
        id: e.message_id,
        install_candidates: Array.isArray(e.install_candidates) && e.install_candidates.length ? e.install_candidates : undefined,
      }
      if (current?.isFinalAnswer) Object.assign(current, { content: String(e.content || current.content || ''), ...extra })
      else addMessage(s, 'assistant', e.content || '', extra)
      return
    }
    // A planning/model failure belongs to the assistant turn so the shared
    // renderer can apply its error presentation and contextual recovery hints.
    case 'error':
      failFastPlanPlanning(s, e)
      s.isPlanning = false; s.error = e; addMessage(s, 'assistant', e.message || 'FastPlan error', { isError: true }); return
    case 'planning_start':
      Object.assign(s, { plannedTree: null, executionState: {}, taskBubbles: [], presentation: null,
        finalResult: null, resultFiles: [], isPlanning: true, isPreviewing: false, isPaused: false,
        treeCompleted: false, executionStatus: null, currentTreeId: '', humanRequest: null, humanRequestQueue: [],
        humanInterventionRequired: false, stepConfirmNodeId: null, lastCompletedNodeId: null, error: null, planningError: null })
      s.planningLogs = []; s.routeFlow = []; s.validationFlow = []; s.thinkingSteps = []; s.thinkingStream = ''; s.processTrace = null; return
    case 'route_stage_update': upsertRouteStage(s, e); return
    case 'plan_generated':
      s.messages = s.messages.filter(item => !item.isThinking); s.plannedTree = e.tree || null; s.presentation = e.tree?.metadata?.presentation || null; s.isPlanning = false; s.planningError = null
      completePlanningProgress(s); s.treeCompleted = false; s.executionStatus = null; s.finalResult = null
      s.isPreviewing = s.executionMode === 'manual' || (s.confirmationPolicy === 'strict' && s.executionMode === 'step')
      s.pendingAutoConfirm = !s.isPreviewing; return
    case 'tree_started':
      s.messages = s.messages.filter(item => !item.isThinking); s.currentTreeId = e.tree_id || s.currentTreeId
      Object.assign(s, { isPlanning: true, isPreviewing: false, isPaused: false, treeCompleted: false, executionStatus: null, finalResult: null,
        resultFiles: [], humanRequest: null, humanRequestQueue: [], humanInterventionRequired: false,
        pendingAutoConfirm: false, stepConfirmNodeId: null, lastCompletedNodeId: null, error: null, planningError: null, degradedSummary: [] })
      initializeExecutionNodes(s); return
    case 'node_started': {
      if (!e.node_id || e.node_id === '0' || isPhase(s, e.node_id)) return
      const node = s.executionState[e.node_id] || (s.executionState[e.node_id] = newNode(e.node_id, e.node_name))
      if (!s.taskBubbles.includes(e.node_id)) s.taskBubbles.push(e.node_id)
      const alreadyRunning = node.status === 'running'
      Object.assign(node, {
        status: 'running',
        startTime: alreadyRunning && node.startTime ? node.startTime : eventTimestamp(e),
        endTime: alreadyRunning ? node.endTime : null,
        durationMs: alreadyRunning ? node.durationMs : null,
        confidence: alreadyRunning ? node.confidence : null,
        error: null,
        usage: normalizeTokenUsage(e, e.data) || node.usage,
      }); s.humanInterventionRequired = false
      if (s.executionMode === 'step') s.stepConfirmNodeId = e.node_id
      return
    }
    case 'streaming_token': { const node = s.executionState[e.node_id]; if (node) { node.streamContent += String(e.token || '').replace(/<think>[\s\S]*?<\/think>/gi, ''); node.usage = normalizeTokenUsage(e, e.data) || node.usage } return }
    case 'node_completed': {
      let internal = false; try { internal = JSON.parse(e.output || '').type === 'root' } catch {}
      if (internal) { delete s.executionState[e.node_id]; s.taskBubbles = s.taskBubbles.filter(id => id !== e.node_id); return }
      const node = s.executionState[e.node_id]
      const data = e.data || {}
      const output = e.output ?? data.output ?? node?.output
      const parsedMetrics = parseOutputMetrics(output)
      const durationMs = e.duration_ms ?? data.duration_ms ?? data.elapsed_ms ?? parsedMetrics.durationMs
      const confidence = e.confidence ?? data.confidence ?? parsedMetrics.confidence
      const usage = normalizeTokenUsage(e, data, parsedMetrics.usage, output)
      // ★ 降级原因（2026-08-23 新增）：节点"成功"但实质降级（如检索类工具空结果）。
      // 非空时前端在结果顶部明示——用户必须在看到结论之前知道数据没有真实支撑。
      const degraded = Array.isArray(e.degraded) ? e.degraded : (data.degraded || [])
      if (node) Object.assign(node, { status: 'completed', endTime: now(), output, durationMs, confidence, usage: usage || node.usage, degraded })
      // 并入全局降级汇总（最终结果顶部也要明示，不限于节点详情）
      for (const reason of degraded) {
        if (!s.degradedSummary.includes(reason)) s.degradedSummary.push(reason)
      }
      addFiles(s, output || node?.streamContent, e.node_id, e.node_name || node?.name); s.lastCompletedNodeId = e.node_id
      const shouldPause = s.confirmationPolicy === 'strict' ? s.executionMode === 'step' : s.executionMode !== 'auto'
      if (shouldPause) { s.isPaused = true; s.stepConfirmNodeId = e.node_id }
      return
    }
    case 'node_failed': {
      const node = s.executionState[e.node_id]
      if (node) Object.assign(node, { status: 'failed', endTime: now(), error: e.error || 'Node failed' })
      if (s.executionMode === 'step') {
        s.lastCompletedNodeId = e.node_id; s.stepConfirmNodeId = e.node_id; s.isPaused = true
      }
      return
    }
    case 'node_skipped': { const node = s.executionState[e.node_id]; if (node) Object.assign(node, { status: 'skipped', endTime: now() }); return }
    case 'agent_task_progress': {
      const node = s.executionState[e.node_id]; if (!node) return
      node.usage = normalizeTokenUsage(e, e.data) || node.usage
      const type = e.phase === 'tool_call' ? 'tool_call' : e.phase || 'info'; const last = node.logs[node.logs.length - 1]
      if (type === 'reasoning' && last?.type === 'reasoning') last.content += e.content || ''
      else node.logs.push({ type, toolName: e.tool_name || '', content: e.content || '' }); return
    }
    case 'thinking_update': {
      const data = e.data || {}; const content = data.content || e.content || ''; if (!content) return
      if (e.phase === 'planning' || ['planner', 'planning'].includes(e.tree_id)) {
        const kind = data.kind || e.kind
        if (kind === 'plan_stage') upsertValidation(s, { ...data, content })
        else if (kind === 'thinking_step') appendThinkingStep(s, { ...data, ...e }, content)
        else if (kind === 'reasoning') s.thinkingStream += content
        else s.planningLogs.push(parseFastPlanThinking(content, null) || { type: 'info', content })
      } else { const node = s.executionState[e.node_id]; if (node) pushFastPlanLog(node.logs, parseFastPlanThinking(content, e.phase)) }
      return
    }
    case 'planning_log': case 'plan_stage': { const content = e.data?.content || e.content || e.message || ''; if (content) s.planningLogs.push({ type: e.type, content, phase: e.phase }); return }
    case 'waiting_for_human':
      s.humanRequest = e; if (!s.humanRequestQueue.some(item => item.node_id === e.node_id)) s.humanRequestQueue.push({ ...e, timestamp: now() }); s.isPaused = true; return
    case 'human_intervention_required': s.humanInterventionRequired = true; s.isPaused = true; return
    case 'reflection': addMessage(s, 'system', `[${e.phase || 'reflection'}] ${e.content || ''}`, { isReflection: true }); return
    case 'tree_completed':
      Object.assign(s, { isPlanning: false, isPaused: false, treeCompleted: true, humanRequest: null, humanRequestQueue: [],
        humanInterventionRequired: false, stepConfirmNodeId: null, lastCompletedNodeId: null })
      s.executionStatus = e.success === false ? 'failed' : e.success === true ? 'completed' : resolveExecutionStatus(s) as 'completed' | 'failed'
      if (e.success && s.plannedTree) { s.canSaveTemplate = true; s.templateTreeData = s.plannedTree }; return
    case 'final_answer': {
      const value = normalizeFastPlanFinalAnswer(e)
      addFiles(s, e.final_output || value.finalOutput || value.content, value.finalNodeId || '', value.finalNodeName)
      value.content = formatFastPlanFinalContent(value.content, s.resultFiles)
      s.finalResult = { ...value, receivedAt: new Date().toISOString(), degraded: [...s.degradedSummary] }
      upsertFinalAssistantMessage(s, value.content, e)
      return
    }
    case 'plan_awaiting_confirmation':
      s.isPreviewing = true
      s.isPlanning = false
      s.pendingAutoConfirm = false
      if (e.execution_mode === 'manual' || e.execution_mode === 'step' || e.executionMode === 'manual' || e.executionMode === 'step') {
        s.executionMode = (e.execution_mode || e.executionMode) as FastPlanExecutionMode
      }
      return
    case 'plan_confirmed':
    case 'confirm_plan':
      s.isPreviewing = false
      s.isPlanning = true
      s.pendingAutoConfirm = false
      s.humanRequest = null
      s.humanRequestQueue = []
      return
    case 'plan_cancelled':
    case 'cancel_plan':
      Object.assign(s, {
        isPlanning: false, isPreviewing: false, isPaused: false,
        humanRequest: null, humanRequestQueue: [],
        stepConfirmNodeId: null, lastCompletedNodeId: null,
        executionStatus: 'cancelled',
      })
      return
    case 'plan_control': {
      const action = String(e.action || e.ctrl || '')
      if (action === 'confirm' || action === 'confirm_plan') {
        s.isPreviewing = false; s.isPlanning = true; s.pendingAutoConfirm = false
        s.humanRequest = null; s.humanRequestQueue = []
      } else if (action === 'resume' || action === 'resume_plan' || action === 'resume_tree' || action === 'continue') {
        s.isPaused = false; s.isPlanning = true; s.humanRequest = null
        s.stepConfirmNodeId = null; s.lastCompletedNodeId = null
      } else if (action === 'cancel' || action === 'cancel_plan' || action === 'stop') {
        Object.assign(s, {
          isPlanning: false, isPreviewing: false, isPaused: false,
          humanRequest: null, humanRequestQueue: [],
          stepConfirmNodeId: null, lastCompletedNodeId: null,
          executionStatus: 'cancelled',
        })
      }
      return
    }
    case 'tree_paused':
      s.isPaused = true
      s.isPlanning = false
      if (e.node_id || e.nodeId) {
        s.lastCompletedNodeId = String(e.node_id || e.nodeId)
        s.stepConfirmNodeId = s.lastCompletedNodeId
      }
      return
    case 'tree_resumed': s.isPaused = false; s.humanRequest = null; s.stepConfirmNodeId = null; s.lastCompletedNodeId = null; s.isPlanning = true; return
    case 'tree_modified': s.plannedTree = e.tree || s.plannedTree; return
    case 'tool_missing': {
      for (const tool of e.tools || []) if (!s.missingTools.some(item => item.originalName === tool.original_name)) {
        const match = (e.search_results || []).find((item: any) => item.tool === tool.original_name)
        s.missingTools.push({ originalName: tool.original_name, searchKeyword: tool.search_keyword, suggestions: match?.suggestions || [] })
      }
      return
    }
    case 'session_updated': {
      const sessions = (s as any).sessions
      if (e.field === 'title' && Array.isArray(sessions)) { const session = sessions.find((item: any) => item.id === e.session_id); if (session) session.title = e.value }
      return
    }
    case 'tool_call_started': {
      const node = s.executionState[e.node_id]; if (!node) return
      const data = e.data || {}
      const toolName = data.tool_name || data.tool || ''
      const argsPreview = data.arguments ? JSON.stringify(data.arguments) : ''
      node.logs.push({
        type: 'tool_call',
        status: 'running',
        toolName,
        toolType: data.tool_type || '',
        toolCallId: data.tool_call_id || '',
        arguments: data.arguments || null,   // 结构化入参（TaskBlock 参数提取优先用）
        content: `▶️ 调用 ${toolName}${argsPreview ? ' · 入参 ' + argsPreview.slice(0, 120) : ''}`,
      })
      return
    }
    case 'tool_call_completed': {
      const node = s.executionState[e.node_id]; if (!node) return
      const data = e.data || {}
      const toolName = data.tool_name || data.tool || ''
      // ★ 通用模式：与 started 同 tool_call_id 配对，就地更新（失败兜底 push 新行）
      let entry = null
      if (data.tool_call_id) {
        for (let i = node.logs.length - 1; i >= 0; i--) {
          const row = node.logs[i]
          if (row.toolCallId === data.tool_call_id && row.type === 'tool_call') { entry = row; break }
        }
      }
      const resultPreview = data.result ? JSON.stringify(data.result).slice(0, 120) : ''
      const errorText = data.error ? (typeof data.error === 'string' ? data.error : JSON.stringify(data.error)) : ''
      const content =
        data.status === 'success' ? `✅ ${toolName} · 出参 ${resultPreview || '(空)'}${data.duration_ms != null ? ` · ${data.duration_ms}ms` : ''}`
        : data.status === 'empty' ? `🈳 ${toolName} · 无返回数据${errorText ? '（' + errorText.slice(0, 80) + '）' : ''}`
        : `❌ ${toolName} 失败${errorText ? '：' + errorText.slice(0, 120) : ''}`
      const done = { type: 'tool_call', status: data.status || 'failed', toolName, toolType: data.tool_type || '', toolCallId: data.tool_call_id || '', durationMs: data.duration_ms != null ? data.duration_ms : null, content }
      if (entry) Object.assign(entry, done)
      else node.logs.push(done)
      return
    }
    case 'workflow_update': {
      const node = s.executionState[e.node_id]; if (!node) return
      const data = e.data || {}; const detail = data.event || {}; if (!node.workflowNodes) node.workflowNodes = []
      if (data.type === 'workflow_start') node.workflowNodes = []
      else if (data.type === 'node_start') node.workflowNodes.push({ id: detail.node_id || `wf_${now()}`, title: detail.node_title || '未知节点', status: 'running', elapsed_ms: null })
      else if (data.type === 'node_end') { const item = node.workflowNodes.find((row: any) => row.id === detail.node_id); if (item) Object.assign(item, { status: 'completed', elapsed_ms: detail.elapsed_ms || 0 }) }
      else if (data.type === 'workflow_end') node.workflowNodes.forEach((item: any) => { if (item.status === 'running') item.status = 'completed' })
      else if (data.type === 'error') node.workflowNodes.push({ id: `err_${now()}`, title: detail.message || '工作流错误', status: 'failed', elapsed_ms: null })
      if (data.content) node.logs.push({ type: 'info', content: data.content }); return
    }
  }
}
