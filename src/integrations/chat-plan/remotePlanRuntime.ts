import { reactive } from 'vue'
import { applyFastPlanEvent, createFastPlanRuntimeState, type FastPlanRuntimeState } from 'planflow-runtime'
import type { Message } from '@/types'

/** 会话级 live reducer，避免每帧从 display 反序列化丢字段。 */
const livePlanStates = new Map<string, FastPlanRuntimeState>()

/**
 * 过程卡实时态：不走 messages 整表刷新，PlanThinkingCard 直接订阅。
 * 赋值新对象即可触发视图更新（与 App remoteSession 同思路）。
 */
export const remotePlanLiveBySession = reactive<Record<string, Record<string, any>>>({})

export function setRemotePlanLive(sessionId: string, runtime: Record<string, any>) {
  remotePlanLiveBySession[sessionId] = runtime
}

export function clearRemotePlanLive(sessionId: string) {
  delete remotePlanLiveBySession[sessionId]
}

/** 把 Rust snapshot（snake_case）或运行时态整理成 PlanThinkingCard 可用的 camelCase。 */
export function normalizePlanRuntime(raw: Record<string, any> | null | undefined): Record<string, any> | null {
  if (!raw || typeof raw !== 'object') return null
  const plannedTree = raw.plannedTree ?? raw.planned_tree ?? null
  const executionState = raw.executionState ?? raw.execution_state ?? {}
  const validationFlow = raw.validationFlow ?? raw.validation_flow ?? []
  const routeFlow = raw.routeFlow ?? raw.route_flow ?? []
  const thinkingSteps = raw.thinkingSteps ?? raw.thinking_steps ?? []
  const thinkingStream = String(raw.thinkingStream ?? raw.thinking_stream ?? '')
  const planningLogs = raw.planningLogs ?? raw.planning_logs ?? []
  const taskBubbles = raw.taskBubbles ?? raw.task_bubbles ?? []
  return {
    controlProtocol: Number(raw.controlProtocol ?? raw.control_protocol ?? 0),
    controlWaitId: typeof (raw.controlWaitId ?? raw.control_wait_id) === 'string'
      ? (raw.controlWaitId ?? raw.control_wait_id) : null,
    plannedTree,
    executionState: executionState && typeof executionState === 'object' && !Array.isArray(executionState)
      ? { ...executionState }
      : {},
    validationFlow: Array.isArray(validationFlow) ? validationFlow.map((row: any) => ({ ...row })) : [],
    routeFlow: Array.isArray(routeFlow) ? routeFlow.map((row: any) => ({ ...row })) : [],
    thinkingSteps: Array.isArray(thinkingSteps) ? thinkingSteps.map((row: any) => ({ ...row })) : [],
    thinkingStream,
    planningLogs: Array.isArray(planningLogs) ? planningLogs.map((row: any) => ({ ...row })) : [],
    treeCompleted: !!(raw.treeCompleted ?? raw.tree_completed),
    executionStatus: raw.executionStatus ?? raw.execution_status ?? null,
    currentTreeId: String(raw.currentTreeId ?? raw.current_tree_id ?? ''),
    planningError: raw.planningError ?? raw.planning_error ?? null,
    isPlanning: !!(raw.isPlanning ?? raw.is_planning),
    isPreviewing: !!(raw.isPreviewing ?? raw.is_previewing),
    isPaused: !!(raw.isPaused ?? raw.is_paused),
    lastCompletedNodeId: raw.lastCompletedNodeId ?? raw.last_completed_node_id ?? null,
    stepConfirmNodeId: raw.stepConfirmNodeId ?? raw.step_confirm_node_id ?? null,
    humanRequest: raw.humanRequest ?? raw.human_request ?? null,
    executionMode: raw.executionMode ?? raw.execution_mode ?? 'auto',
    presentation: raw.presentation ?? null,
    taskBubbles: Array.isArray(taskBubbles) ? [...taskBubbles] : [],
    finalResult: raw.finalResult ?? raw.final_result ?? null,
  }
}

function displayRuntimeFromState(state: FastPlanRuntimeState): Record<string, any> {
  return normalizePlanRuntime({
    plannedTree: state.plannedTree,
    executionState: state.executionState,
    validationFlow: state.validationFlow,
    routeFlow: state.routeFlow,
    thinkingSteps: state.thinkingSteps,
    thinkingStream: state.thinkingStream,
    planningLogs: state.planningLogs,
    treeCompleted: state.treeCompleted,
    executionStatus: state.executionStatus,
    currentTreeId: state.currentTreeId,
    planningError: state.planningError,
    isPlanning: state.isPlanning,
    isPreviewing: state.isPreviewing,
    isPaused: state.isPaused,
    lastCompletedNodeId: state.lastCompletedNodeId,
    stepConfirmNodeId: state.stepConfirmNodeId,
    humanRequest: state.humanRequest,
    executionMode: state.executionMode,
    presentation: state.presentation,
    taskBubbles: state.taskBubbles,
    finalResult: state.finalResult,
  })!
}

/**
 * FastPlan WS / ACP 帧可能是 `{ type:'event', event:{ type:'route_stage_update', ... } }`。
 * applyFastPlanEvent 只认 e.type，必须先剥壳。
 */
export function normalizePlanEventFrame(
  event: Record<string, unknown> | null | undefined,
): Record<string, unknown> | null {
  if (!event || typeof event !== 'object') return null
  let frame: Record<string, unknown> = event
  for (let depth = 0; depth < 3; depth += 1) {
    const rawType = frame.type ?? frame.event
    const type = typeof rawType === 'string' ? rawType : ''
    if (type === 'event' || type === 'message') {
      const inner = frame.event ?? frame.data ?? frame.payload
      if (inner && typeof inner === 'object' && !Array.isArray(inner)) {
        frame = inner as Record<string, unknown>
        continue
      }
    }
    break
  }
  if (typeof frame.type !== 'string' || !frame.type) {
    const alt = frame.event
    if (typeof alt === 'string' && alt) frame = { ...frame, type: alt }
  }
  if (typeof frame.type !== 'string' || !frame.type) return null
  return frame
}

function getLiveState(sessionId: string): FastPlanRuntimeState {
  let state = livePlanStates.get(sessionId)
  if (!state) {
    state = createFastPlanRuntimeState()
    livePlanStates.set(sessionId, state)
  }
  return state
}

export function clearLivePlanState(sessionId: string) {
  livePlanStates.delete(sessionId)
  clearRemotePlanLive(sessionId)
}

function lastUserMessageIndex(messages: readonly Message[]): number {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    if (messages[i]?.role === 'user') return i
  }
  return -1
}

/**
 * 当前回合规划气泡：优先取「最后一条用户消息之后」的 plan 助手气泡。
 * 若没有，再找用户消息之前误插的空过程卡（plan 帧早于 user Text 落地时会出现）。
 */
export function findLivePlanMessageIndex(messages: readonly Message[]): number {
  const userIdx = lastUserMessageIndex(messages)
  for (let i = messages.length - 1; i > userIdx; i -= 1) {
    const message = messages[i]
    if (message?.role === 'assistant' && message.planRuntime) return i
  }
  if (userIdx < 0) {
    for (let i = messages.length - 1; i >= 0; i -= 1) {
      const message = messages[i]
      if (message?.role === 'assistant' && message.planRuntime) return i
    }
    return -1
  }
  // 孤儿过程卡：夹在上一轮用户与本轮用户之间（或列表最前）
  for (let i = userIdx - 1; i >= 0; i -= 1) {
    const message = messages[i]
    if (!message) continue
    if (message.role === 'user') break
    if (message.role === 'assistant' && message.planRuntime && !String(message.content || '').trim()) {
      return i
    }
  }
  return -1
}

/**
 * 用户 Text 晚于 plan 帧时：把误插在用户前的空过程卡挪到用户后，并去掉重复空卡。
 * @returns 是否改动了 messages 结构（需 publish）
 */
export function relocateOrphanPlanBubbles(messages: Message[]): boolean {
  if (!messages.length) return false
  const userIdx = lastUserMessageIndex(messages)
  if (userIdx < 0) return false

  const orphanIdxs: number[] = []
  for (let i = 0; i < userIdx; i += 1) {
    const message = messages[i]
    if (message?.role === 'assistant' && message.planRuntime && !String(message.content || '').trim()) {
      orphanIdxs.push(i)
    }
  }
  if (!orphanIdxs.length) return false

  // 保留最后一个孤儿（最新 live），其余删除
  const keepIdx = orphanIdxs[orphanIdxs.length - 1]
  const keep = { ...messages[keepIdx] }
  for (let i = orphanIdxs.length - 1; i >= 0; i -= 1) {
    messages.splice(orphanIdxs[i], 1)
  }
  // splice 后 userIdx 左移 orphanIdxs.length
  const newUserIdx = userIdx - orphanIdxs.length
  // 若用户后已有 plan 气泡，合并进该气泡并丢弃孤儿
  let afterPlan = -1
  for (let i = messages.length - 1; i > newUserIdx; i -= 1) {
    if (messages[i]?.role === 'assistant' && messages[i].planRuntime) {
      afterPlan = i
      break
    }
  }
  if (afterPlan >= 0) {
    const cur = messages[afterPlan]
    messages[afterPlan] = {
      ...cur,
      planRuntime: keep.planRuntime || cur.planRuntime,
      status: cur.content?.trim() ? cur.status : (keep.status || cur.status),
    }
    return true
  }
  messages.splice(newUserIdx + 1, 0, keep)
  return true
}

/**
 * 用户后若已有空助手气泡（带模型名的流式占位），把 planRuntime 挂上去，避免再插一张「assistant」空卡。
 * 否则新建专用过程卡（须带 modelId，否则标题会回退成 fallback「assistant」）。
 */
function pushPlanPlaceholder(
  messages: Message[],
  sessionId: string,
  planRuntime: Record<string, any>,
  modelId?: string | null,
): number {
  const resolvedModelId = String(modelId || '').trim() || null
  const userIdx = lastUserMessageIndex(messages)
  if (userIdx >= 0) {
    for (let i = userIdx + 1; i < messages.length; i += 1) {
      const message = messages[i]
      if (!message || message.role !== 'assistant') continue
      if (message.planRuntime) {
        if (resolvedModelId && !message.model_id) {
          messages[i] = { ...message, model_id: resolvedModelId }
        }
        return i
      }
      if (!String(message.content || '').trim()) {
        messages[i] = {
          ...message,
          planRuntime,
          model_id: message.model_id || resolvedModelId,
          status: message.status === 'completed' ? message.status : 'streaming',
        }
        return i
      }
      // 已有正文的助手消息：不再往前插，避免拆成两张卡
      break
    }
  }
  const bubble: Message = {
    id: `${sessionId}-plan-${Date.now()}`,
    conversation_id: sessionId,
    role: 'assistant',
    content: '',
    model_id: resolvedModelId,
    token_input: null,
    token_output: null,
    status: 'streaming',
    created_at: new Date().toISOString(),
    planRuntime,
  }
  if (userIdx >= 0 && userIdx < messages.length - 1) {
    messages.splice(userIdx + 1, 0, bubble)
    return userIdx + 1
  }
  messages.push(bubble)
  return messages.length - 1
}

/**
 * 同一用户回合后只保留一张带 planRuntime 的助手卡（合并 model_id / 内容）。
 * 修复：过程占位 + 模型流式气泡各挂一份 planRuntime → 界面出现两张相同过程卡。
 */
export function dedupePlanBubblesAfterUser(messages: Message[]): boolean {
  if (!messages.length) return false
  const userIdx = lastUserMessageIndex(messages)
  if (userIdx < 0) return false

  const planIdxs: number[] = []
  for (let i = userIdx + 1; i < messages.length; i += 1) {
    const message = messages[i]
    if (message?.role === 'assistant' && message.planRuntime) planIdxs.push(i)
  }
  if (planIdxs.length <= 1) return false

  // 优先保留：有正文 > 有 model_id > 更靠后
  let keepPos = planIdxs[planIdxs.length - 1]
  for (const idx of planIdxs) {
    const m = messages[idx]
    const cur = messages[keepPos]
    const mScore = (String(m.content || '').trim() ? 4 : 0) + (m.model_id ? 2 : 0) + 1
    const cScore = (String(cur.content || '').trim() ? 4 : 0) + (cur.model_id ? 2 : 0) + 1
    if (mScore > cScore) keepPos = idx
  }

  const keep = { ...messages[keepPos] }
  for (const idx of planIdxs) {
    if (idx === keepPos) continue
    const other = messages[idx]
    if (!keep.model_id && other.model_id) keep.model_id = other.model_id
    if (!String(keep.content || '').trim() && String(other.content || '').trim()) {
      keep.content = other.content
      keep.status = other.status
    }
    keep.planRuntime = other.planRuntime || keep.planRuntime
  }

  // 从后往前删，避免下标错位
  for (let i = planIdxs.length - 1; i >= 0; i -= 1) {
    if (planIdxs[i] !== keepPos) messages.splice(planIdxs[i], 1)
  }
  // splice 后重新定位 keep
  const newUserIdx = lastUserMessageIndex(messages)
  for (let i = messages.length - 1; i > newUserIdx; i -= 1) {
    if (messages[i]?.role === 'assistant' && messages[i].planRuntime) {
      messages[i] = keep
      break
    }
  }
  return true
}

export type ApplyRemotePlanResult = {
  applied: boolean
  /** 仅首次插入过程气泡时需要刷新 messages 列表 */
  createdBubble: boolean
}

function publishLive(sessionId: string, planRuntime: Record<string, any>) {
  setRemotePlanLive(sessionId, planRuntime)
}

/**
 * 将 ACP `plan` 内层事件落到 live planRuntime +（必要时）过程气泡。
 * 过程进度默认只更新 remotePlanLiveBySession，避免整表 messages 重算。
 * options.modelId：写入过程卡，供气泡标题显示当前模型名（缺省会回退成「assistant」）。
 */
export function applyRemotePlanEventToMessages(
  messages: Message[],
  sessionId: string,
  event: Record<string, unknown> | null | undefined,
  options?: { modelId?: string | null; executionMode?: string | null },
): ApplyRemotePlanResult {
  if (!messages) return { applied: false, createdBubble: false }
  const frame = normalizePlanEventFrame(event)
  if (!frame) return { applied: false, createdBubble: false }
  const type = String(frame.type || '')
  if (!type || type === 'fastplan_session') return { applied: false, createdBubble: false }
  const modelId = options?.modelId ?? null

  // 先纠正「规划卡在用户气泡前」的错序，避免再插第二条
  let structureChanged = relocateOrphanPlanBubbles(messages)
  if (dedupePlanBubblesAfterUser(messages)) structureChanged = true

  if (type === 'snapshot') {
    const snap = normalizePlanRuntime(
      (frame.planRuntime || frame.plan_runtime || null) as Record<string, any> | null,
    )
    if (!snap) return { applied: false, createdBubble: structureChanged }
    const state = getLiveState(sessionId)
    Object.assign(state, {
      plannedTree: snap.plannedTree,
      executionState: { ...snap.executionState },
      validationFlow: [...snap.validationFlow],
      routeFlow: [...snap.routeFlow],
      thinkingSteps: [...snap.thinkingSteps],
      thinkingStream: snap.thinkingStream,
      planningLogs: [...snap.planningLogs],
      treeCompleted: snap.treeCompleted,
      executionStatus: snap.executionStatus,
      currentTreeId: snap.currentTreeId,
      planningError: snap.planningError,
      isPlanning: snap.isPlanning,
      isPreviewing: snap.isPreviewing,
      isPaused: snap.isPaused,
      lastCompletedNodeId: snap.lastCompletedNodeId,
      stepConfirmNodeId: snap.stepConfirmNodeId,
      humanRequest: snap.humanRequest,
      executionMode: snap.executionMode,
      presentation: snap.presentation,
      taskBubbles: [...snap.taskBubbles],
      finalResult: snap.finalResult,
    })
    publishLive(sessionId, snap)
    let createdBubble = structureChanged
    const idx = findLivePlanMessageIndex(messages)
    if (idx < 0) {
      pushPlanPlaceholder(messages, sessionId, snap, modelId)
      createdBubble = true
    } else {
      const current = messages[idx]
      messages[idx] = {
        ...current,
        planRuntime: snap,
        model_id: current.model_id || (String(modelId || '').trim() || null),
      }
    }
    if (dedupePlanBubblesAfterUser(messages)) createdBubble = true
    return { applied: true, createdBubble }
  }

  const state = getLiveState(sessionId)
  const mode = options?.executionMode
  if (mode === 'auto' || mode === 'manual' || mode === 'step') state.executionMode = mode
  const hadBubble = findLivePlanMessageIndex(messages) >= 0
  if (type === 'planning_start' || !hadBubble) {
    if (type !== 'planning_start') applyFastPlanEvent(state, { type: 'planning_start' })
  }
  applyFastPlanEvent(state, frame)
  const display = displayRuntimeFromState(state)
  // The shared reducer does not own the remote backend's wait identity. Preserve it
  // across progress frames; only a published snapshot can open a protocol-v1 wait.
  const previous = remotePlanLiveBySession[sessionId]
  display.controlProtocol = Number(frame.controlProtocol ?? previous?.controlProtocol ?? 0)
  display.controlWaitId = previous?.controlWaitId ?? null
  const previousWait = planConfirmationFromRuntime(previous && { ...previous, controlProtocol: 0 })?.id
  const nextWait = planConfirmationFromRuntime({ ...display, controlProtocol: 0 })?.id
  if (['planning_start', 'tree_resumed', 'plan_confirmed', 'plan_cancelled', 'tree_completed', 'error'].includes(type)
    || !nextWait || nextWait !== previousWait) {
    display.controlWaitId = null
  }
  publishLive(sessionId, display)

  let createdBubble = structureChanged
  if (!hadBubble) {
    pushPlanPlaceholder(messages, sessionId, display, modelId)
    createdBubble = true
  } else if (type === 'planning_start') {
    const idx = findLivePlanMessageIndex(messages)
    if (idx >= 0) {
      const current = messages[idx]
      messages[idx] = {
        ...current,
        content: current.content?.trim() ? current.content : '',
        status: current.content?.trim() ? current.status : 'streaming',
        planRuntime: display,
        model_id: current.model_id || (String(modelId || '').trim() || null),
      }
    }
  } else if (modelId) {
    // 后续帧：若过程卡仍缺 model_id，补上（标题不再显示 assistant）
    const idx = findLivePlanMessageIndex(messages)
    if (idx >= 0 && !messages[idx].model_id) {
      messages[idx] = { ...messages[idx], model_id: String(modelId).trim() }
      createdBubble = true
    }
  }
  if (dedupePlanBubblesAfterUser(messages)) createdBubble = true
  return { applied: true, createdBubble }
}

/** 从消息 extra_data 恢复 planRuntime（历史回显）。 */
export function hydrateMessagePlanRuntime(message: Message, extraData: Record<string, unknown> | null | undefined): void {
  if (!message || message.role !== 'assistant' || !extraData) return
  const raw = extraData.planRuntime ?? extraData.plan_runtime
  if (!raw || typeof raw !== 'object') return
  const normalized = normalizePlanRuntime(raw as Record<string, any>)
  if (normalized) message.planRuntime = normalized
}

export function stampLivePlanExecutionMode(sessionId: string, mode: unknown) {
  const parsed = mode === 'manual' || mode === 'step' ? mode : 'auto'
  const state = livePlanStates.get(sessionId)
  if (state) state.executionMode = parsed
  const live = remotePlanLiveBySession[sessionId]
  if (live) setRemotePlanLive(sessionId, { ...live, executionMode: parsed })
}

export type PlanConfirmationOption = {
  value: string
  label: string
  action: string
  detail?: string
  tone?: 'normal' | 'danger'
}

export type PlanConfirmation = {
  id?: string
  kind: 'plan' | 'step'
  title: string
  question: string
  options: PlanConfirmationOption[]
}

/** 与 FastPlanConversation confirmationModel 同一套文案 / 选项结构。 */
export function planConfirmationFromRuntime(state: Record<string, any> | null | undefined): PlanConfirmation | null {
  if (!state || state.treeCompleted) return null
  // Do not expose a control until the backend has installed its corresponding slot.
  if (state.controlProtocol === 1 && !state.controlWaitId) return null
  const mode = String(state.executionMode || state.execution_mode || 'auto')
  if (mode === 'step' && state.isPaused) {
    const nodeId = state.lastCompletedNodeId || state.stepConfirmNodeId
    const node = (nodeId && state.executionState && state.executionState[nodeId]) || {}
    const failed = String(node.status || '').toLowerCase() === 'failed'
    const name = String(node.name || node.title || '').trim()
    return {
      id: `step:${state.currentTreeId || ''}:${nodeId || ''}`,
      kind: 'step',
      title: failed ? '当前步骤执行失败' : '当前步骤已完成',
      question: name
        ? `“${name}”${failed ? '执行失败' : '已完成'}，是否继续执行下一步？`
        : `当前步骤${failed ? '执行失败' : '已完成'}，是否继续执行下一步？`,
      options: [
        { value: 'continue', label: '继续下一步', action: 'resume', detail: '确认后继续执行后续任务' },
        { value: 'stop', label: '终止执行', action: 'cancel', detail: '停止当前规划的剩余任务', tone: 'danger' },
      ],
    }
  }
  if (state.isPreviewing) {
    return {
      id: `plan:${state.plannedTree?.id || state.plannedTree?.tree_id || state.currentTreeId || 'current'}:ready`,
      kind: 'plan',
      title: '确认执行当前规划',
      question: '规划已生成，确认后 FastPlan 才会开始执行。',
      options: [
        { value: 'confirm', label: '确认并执行', action: 'confirm', detail: '开始执行当前任务树' },
        { value: 'cancel', label: '取消任务', action: 'cancel', detail: '不执行当前规划', tone: 'danger' },
      ],
    }
  }
  return null
}

