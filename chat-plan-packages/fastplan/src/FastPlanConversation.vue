<script setup lang="ts">
import { computed, onBeforeUnmount, provide, reactive, ref, watch } from 'vue'
import { FASTPLAN_ACCESS_KEY, downloadFastPlanFile } from 'planflow-runtime'
import { ChatConversation, ChatMarkdownRenderer, type MessageActionContext } from 'chat-renderer'
import FastPlanAnswer from './FastPlanAnswer.vue'
import { FastPlanPlanningPanel, FastPlanTaskList } from 'planflow-planning-ui'
import { adaptFastPlanExecutionHistory, adaptFastPlanMessages, adaptFastPlanTaskPlanExecution, createFastPlanExecutionController, splitFastPlanFinalAnswer, extractFastPlanImages, type FastPlanExecutionController, type FastPlanSendOptions } from 'planflow-runtime'
import { collectDeferredFastPlanMessageIds, createFastPlanThemeStyle, findFastPlanFinalAssistantMessageId, normalizeFastPlanSubmission, resolveFastPlanComposerAttachments, resolveFastPlanDownloadUrl, summarizeFastPlanProcessText, withFastPlanAssistantShell } from './conversation'
import FastPlanContextItem from './FastPlanContextItem.vue'
import { useResponsiveDensity, type FastPlanDensity } from './useResponsiveDensity'
import { executeFastPlanInteraction, type FastPlanInteractionCommand } from './interaction'

const UntypedChatConversation = ChatConversation as any
let componentDisposed = false

// Keep SFC macros self-contained. Some Vite hosts compile workspace packages
// without exposing TypeScript to @vue/compiler-sfc's imported-type resolver.
const props = withDefaults(defineProps<{
  sessionId: string
  access?: any
  beforeSend?: (selection: unknown, prompt: string) => Promise<boolean>
  onControl?: (command: FastPlanInteractionCommand) => Promise<boolean>
  apiBase?: string
  wsBase?: string
  model?: string | Record<string, any>
  executionMode?: 'auto' | 'manual' | 'step'
  confirmationPolicy?: 'legacy' | 'strict'
  maxReconnectAttempts?: number
  baseReconnectDelayMs?: number
  uploadBase?: string
  adminBase?: string
  downloadBase?: string
  retainConnectionOnSessionSwitch?: boolean
  placeholder?: string
  composerDisabled?: boolean
  composerReadonly?: boolean
  composerActions?: any[]
  composerAttachments?: any[]
  planAttachments?: any[]
  resolveComposerAttachments?: (attachments: any[]) => any[]
  sendByCtrlEnter?: boolean
  showActions?: boolean
  actions?: Record<string, any>
  resolveActions?: (...args: any[]) => any[]
  autoConnect?: boolean
  theme?: 'light' | 'dark' | 'auto'
  themeTokens?: Record<string, string | undefined>
  showContext?: boolean
  contextItems?: any[]
  contextPlacement?: 'top' | 'composer'
  layout?: 'standalone' | 'embedded'
  appearance?: 'default' | 'seamless'
  composerMode?: 'internal' | 'host'
  composerTarget?: string | HTMLElement | null
  showTaskList?: boolean
  interactionPlacement?: 'composer' | 'inline'
  errorPlacement?: 'top' | 'inline'
  showComposer?: boolean
  scrollMode?: 'internal' | 'host'
  connectionMode?: 'live' | 'history' | 'manual'
  autoLoad?: boolean
  density?: FastPlanDensity
  compactBreakpoint?: number
  /** Optional messageId -> feedback map restored from host persistence. */
  initialFeedback?: Record<string, number>
  /** Live runtime owned by the host. When present, component unmount only detaches UI listeners. */
  runtime?: any
}>(), { apiBase: '', wsBase: '', executionMode: 'auto', confirmationPolicy: 'legacy', maxReconnectAttempts: undefined,
  baseReconnectDelayMs: undefined, uploadBase: '', adminBase: '', downloadBase: '',
  retainConnectionOnSessionSwitch: false, placeholder: '输入任务或问题…',
  composerDisabled: false, composerReadonly: false, composerActions: () => [], composerAttachments: () => [], planAttachments: () => [],
  sendByCtrlEnter: false, showActions: true, actions: () => ({}), autoConnect: true, theme: 'light', themeTokens: () => ({}),
  showContext: false, contextItems: () => [], contextPlacement: 'top', layout: 'standalone', appearance: 'default', composerMode: 'internal', composerTarget: null, showTaskList: true, interactionPlacement: 'composer', errorPlacement: 'top',
  showComposer: undefined, scrollMode: undefined, connectionMode: undefined, autoLoad: true, density: 'auto', compactBreakpoint: 700, initialFeedback: () => ({}) })

const emit = defineEmits<{
  (event: 'state-change', value: any): void
  (event: 'loading-change', value: boolean): void
  (event: 'message-sent', value: any): void
  (event: 'plan-completed', value: any): void
  (event: 'execution-completed', value: any): void
  (event: 'human-request', value: any): void
  (event: 'composer-action', value: any): void
  (event: 'composer-files', value: any): void
  (event: 'attachment-remove', attachment: any, index: number): void
  (event: 'attachment-action', value: any): void
  (event: 'message-action', value: any): void
  (event: 'context-action', value: any): void
  (event: 'context-option-select', value: any): void
  (event: 'node-select', value: any): void
  (event: 'error', value: any): void
}>()
defineSlots<{
  'top-bar'(props: { runtime: any; state: any }): any
  avatar(props: { message: any }): any
  header(props: { message: any }): any
  'message-card'(props: { messages: any[]; runtime: any; state: any }): any
  'result-card'(props: { result: any; files: any[]; images: any[]; charts: any[]; state: any }): any
  'historical-planning-card'(props: { plan: any; state: any }): any
  'attachment-input'(props: { runtime: any; state: any }): any
  composer(props: { runtime: any; state: any; modelValue: string; updateModelValue: (value: string) => void; submit: (value: any) => Promise<void> }): any
  'action-bar'(props: { message: any; actions: any[]; trigger: (action: any) => any }): any
  action(props: { action: any; message: any; trigger: (action: any) => any }): any
  'action-icon'(props: { action?: any; message: any; iconName: string; active: boolean; copied: boolean }): any
  'action-menu'(props: { action: any; message: any; placement: 'direct' | 'overflow'; triggerItem: (item: any) => any; close: () => void }): any
  'context-item'(props: { item: any; select: (item: any, event: MouseEvent | KeyboardEvent) => void }): any
  error(props: { error: any }): any
}>()

const conversationRoot = ref<HTMLElement | null>(null)
const composerTargetElement = ref<HTMLElement | null>(null)
let composerTargetObserver: MutationObserver | null = null

function resolveComposerTarget() {
  composerTargetObserver?.disconnect()
  composerTargetObserver = null
  const target = props.composerTarget
  if (typeof HTMLElement !== 'undefined' && target instanceof HTMLElement) {
    composerTargetElement.value = target
    return
  }
  if (typeof target !== 'string' || typeof document === 'undefined') {
    composerTargetElement.value = null
    return
  }
  const findTarget = () => {
    const element = document.querySelector<HTMLElement>(target)
    composerTargetElement.value = element
    if (element) {
      composerTargetObserver?.disconnect()
      composerTargetObserver = null
    }
    return !!element
  }
  if (findTarget() || typeof MutationObserver === 'undefined' || !document.body) return
  composerTargetObserver = new MutationObserver(findTarget)
  composerTargetObserver.observe(document.body, { childList: true, subtree: true })
}

watch([() => props.composerMode, () => props.composerTarget], resolveComposerTarget, { immediate: true, flush: 'post' })
const observedComposerTarget = computed(() => props.composerMode === 'host'
  ? (composerTargetElement.value || conversationRoot.value)
  : conversationRoot.value)
const contentDensity = useResponsiveDensity(conversationRoot, () => props.density, () => props.compactBreakpoint)
const composerDensity = useResponsiveDensity(observedComposerTarget, () => props.density, () => props.compactBreakpoint)

const ownsRuntime = !props.runtime
const runtime: FastPlanExecutionController = props.runtime || createFastPlanExecutionController({ sessionId: props.sessionId, apiBase: props.apiBase, wsBase: props.wsBase,
  access: props.access,
  model: props.model, executionMode: props.executionMode, confirmationPolicy: props.confirmationPolicy,
  maxReconnectAttempts: props.maxReconnectAttempts, baseReconnectDelayMs: props.baseReconnectDelayMs,
  retainConnectionOnSessionSwitch: props.retainConnectionOnSessionSwitch,
  // The backend persists the authoritative user turn received over WebSocket.
  // A second REST write here produced two adjacent user rows for every prompt.
  persistUserMessages: false })
provide(FASTPLAN_ACCESS_KEY, props.access)
watch(runtime.loading, loading => emit('loading-change', loading), { flush: 'sync' })
const removeRuntimeListeners = [
  runtime.on('state-change', (payload: any) => emit('state-change', payload)),
  runtime.on('message-sent', (payload: any) => emit('message-sent', payload)),
  runtime.on('plan-completed', (payload: any) => emit('plan-completed', payload)),
  runtime.on('execution-completed', (payload: any) => emit('execution-completed', payload)),
  runtime.on('human-request', (payload: any) => emit('human-request', payload)),
  runtime.on('error', (payload: any) => emit('error', payload)),
]

const input = ref(''); const gateFiles = ref<any[]>([]); const choiceState = ref('idle')
const turnViews = reactive<Record<string, string>>({})
const expandedProcessMessages = reactive<Record<string, boolean>>({})
const activeView = computed({
  get: () => turnViews[String(runtime.state.activeUserMessageId || lastUserMessageId.value)] || 'list',
  set: value => { turnViews[String(runtime.state.activeUserMessageId || lastUserMessageId.value)] = value },
})
const planningPanel = ref<any>(null)
const restoredSession = ref(false)
const planningModel = computed(() => {
  const config = typeof props.model === 'string' ? { model: props.model } : (props.model || {})
  const id = config.model || config.id || ''
  return { id, name: config.name || id || 'FastPlan', avatar: config.avatar || null }
})
const planExecution = computed(() => adaptFastPlanTaskPlanExecution(runtime.state))
/** 赞/踩状态（规划结果不在宿主 messages 表，本地维护并随 entry 落盘） */
const messageFeedback = reactive<Record<string, number>>({ ...(props.initialFeedback || {}) })
watch(() => props.initialFeedback, (value) => {
  if (!value || typeof value !== 'object') return
  for (const [id, feedback] of Object.entries(value)) {
    const next = Number(feedback)
    if (next === 1 || next === -1 || next === 0) messageFeedback[id] = next
  }
}, { deep: true })
const adaptedMessages = computed(() => adaptFastPlanMessages(runtime.state.messages, {
  modelId: typeof props.model === 'string' ? props.model : props.model?.model,
  modelName: planningModel.value.name,
  modelAvatar: planningModel.value.avatar,
  executionHistory: runtime.state.executionHistory,
  currentTreeId: runtime.state.currentTreeId,
  finalResult: runtime.state.finalResult,
  resultFiles: runtime.state.resultFiles,
}))
const messages = computed(() => withFastPlanAssistantShell(adaptedMessages.value, planExecution.value, planningModel.value, runtime.state.executionHistory)
  .map((message: any) => {
    const feedback = messageFeedback[String(message.id)]
    return feedback === undefined ? message : { ...message, feedback }
  }))
const deferredProcessMessageIds = computed(() => collectDeferredFastPlanMessageIds(messages.value))
function isDeferredProcessMessage(message: any) {
  return deferredProcessMessageIds.value.has(String(message?.id))
}
function isProcessMessageCollapsed(message: any) {
  return isDeferredProcessMessage(message) && !expandedProcessMessages[String(message?.id)]
}
function setProcessMessageExpanded(message: any, expanded: boolean) {
  expandedProcessMessages[String(message?.id)] = expanded
}
const runtimeErrorText = computed(() => String(runtime.state.error?.message || runtime.state.error?.error || '').trim())
const errorAlreadyInMessages = computed(() => {
  const target = runtimeErrorText.value
  if (!target) return false
  return adaptedMessages.value.some((message: any) => message.status === 'error'
    && message.parts?.some((part: any) => part.type === 'text' && String(part.text || '').trim() === target))
})
// Protocol errors are also persisted as an assistant error message. Render the
// standalone alert only for controller/connection errors that have no message.
const showStandaloneError = computed(() => !!runtime.state.error && !errorAlreadyInMessages.value)
// Keep historical/streaming plan messages bounded. ChatMarkdownRenderer keeps
// the full payload available through its explicit "show full content" action.
const maxDisplayChars = 15_000
// Results are selected by their user-message anchor, never by the last reply
// in the whole session. This also keeps previous attachments visible while sending.
const resultsByMessage = computed(() => {
  const result = new Map<string, any>()
  const snapshots = [...runtime.state.executionHistory]
  const anchor = runtime.state.activeUserMessageId || lastUserMessageId.value
  if (planExecution.value) snapshots.push({ ...runtime.state, anchorMessageId: anchor })
  for (const state of snapshots) {
    const start = messages.value.findIndex((message: any) => String(message.id) === String(state.anchorMessageId))
    if (start < 0 || !state.finalResult) continue
    const next = messages.value.findIndex((message: any, index: number) => index > start && message.role === 'user')
    const replies = messages.value.slice(start + 1, next < 0 ? undefined : next)
    const id = findFastPlanFinalAssistantMessageId(replies) || [...replies].reverse().find((message: any) => message.role === 'assistant')?.id
    if (!id) continue
    const sources = [state.finalResult, ...Object.values(state.executionState || {}).flatMap((node: any) => [node?.output, node?.streamContent, node?.logs])]
    const images = extractFastPlanImages(sources), files = state.resultFiles || []
    const charts = splitFastPlanFinalAnswer({ kind: 'final_answer', content: String(state.finalResult.content || '') })
      .flatMap(part => part.type === 'chart' ? [part.option] : [])
    if (images.length || charts.length || files.length || state.finalResult.degraded?.length)
      result.set(id, { state, finalResult: state.finalResult, images, charts, files })
  }
  return result
})
function resultForMessage(message: any) {
  const result = resultsByMessage.value.get(message.id)
  return result ? [result] : []
}
const composerPlan = computed<any>(() => planExecution.value || { status: 'planning', phases: [] })
const showComposerTaskList = computed(() => props.showTaskList && !!planExecution.value && !!runtime.state.plannedTree)
const confirmationModel = computed<any>(() => {
  const request = runtime.state.humanRequestQueue?.[0] || runtime.state.humanRequest
  if (request) {
    const interaction = request.interaction || {}
    const interactionKind = interaction.kind || (interaction.fields?.length ? 'input' : 'legacy')
    const rawOptions = Array.isArray(interaction.options) && interaction.options.length
      ? interaction.options
      : (Array.isArray(request.options) && request.options.length ? request.options : (interactionKind === 'input' ? [] : ['continue', 'abort']))
    const options = rawOptions.map((option: any, index: number) => typeof option === 'string'
      ? { value: option, label: option, tone: /abort|cancel|stop|终止|取消/i.test(option) ? 'danger' : 'normal' }
      : { value: String(option.value ?? option.id ?? option.action ?? index), label: option.label || option.title || option.value || `选项 ${index + 1}`,
          detail: option.detail || option.description, tone: option.tone || (/abort|cancel|stop|终止|取消/i.test(String(option.value || option.label || '')) ? 'danger' : 'normal') })
    return { id: `human:${request.tree_id || runtime.state.currentTreeId}:${request.node_id || ''}`,
      kind: 'human', interactionKind, title: request.title || (interactionKind === 'input' ? '需要补充信息' : '需要人工确认'),
      question: request.question || request.prompt || request.message || '', options, fields: interaction.fields || [],
      submitLabel: interaction.submit_label || '提交并继续', cancelLabel: interaction.cancel_label || '取消任务',
      validationError: interaction.validation_error || '' }
  }
  if (runtime.state.isPreviewing && choiceState.value !== 'cancelled' && choiceState.value !== 'confirmed') {
    const editing = choiceState.value === 'editing'
    return { id: `plan:${runtime.state.plannedTree?.id || runtime.state.plannedTree?.tree_id || 'current'}:${editing ? 'editing' : 'ready'}`,
      kind: 'plan', title: editing ? '确认修改后的规划' : '确认执行当前规划',
      question: editing ? '完成步骤调整后，可在此确认并开始执行。' : '规划已生成，确认后 FastPlan 才会开始执行。',
      options: [
        { value: 'confirm', label: editing ? '确认修改并执行' : '确认并执行', detail: '开始执行当前任务树' },
        ...(editing ? [] : [{ value: 'edit', label: '修改规划', detail: '返回规划内容调整任务步骤' }]),
        { value: 'cancel', label: '取消任务', detail: '不执行当前规划', tone: 'danger' },
      ] }
  }
  if (props.executionMode === 'step' && runtime.state.isPaused && runtime.state.lastCompletedNodeId) {
    const completed = runtime.state.executionState?.[runtime.state.lastCompletedNodeId]
    const failed = completed?.status === 'failed'
    return { id: `step:${runtime.state.currentTreeId}:${runtime.state.lastCompletedNodeId}`,
      kind: 'step', title: failed ? '当前步骤执行失败' : '当前步骤已完成',
      question: completed?.name ? `“${completed.name}”${failed ? '执行失败' : '已完成'}，是否继续执行下一步？` : `当前步骤${failed ? '执行失败' : '已完成'}，是否继续执行下一步？`,
      options: [
        { value: 'continue', label: '继续下一步', detail: '确认后继续执行后续任务' },
        { value: 'stop', label: '终止执行', detail: '停止当前规划的剩余任务', tone: 'danger' },
      ] }
  }
  return null
})
const planExecutions = computed(() => adaptFastPlanExecutionHistory(runtime.state))
const lastUserMessageId = computed(() => [...messages.value].reverse().find(message => message.role === 'user')?.id)
function planForMessage(message: any) {
  if (message.role !== 'user') return null
  const historical = planExecutions.value.find(entry => String(entry.anchorMessageId) === String(message.id))?.plan
  if (historical) return historical
  return message.id === lastUserMessageId.value ? planExecution.value : null
}
function isActivePlanMessage(message: any) {
  return message.role === 'user' && String(message.id) === String(runtime.state.activeUserMessageId || lastUserMessageId.value) && !!planExecution.value
}
function userMessageForTurn(turn: any) {
  return turn?.messages?.find((message: any) => message.role === 'user') || null
}
function planForTurn(turn: any) {
  const userMessage = userMessageForTurn(turn)
  return userMessage ? planForMessage(userMessage) : null
}
function isActivePlanTurn(turn: any) {
  const userMessage = userMessageForTurn(turn)
  return !!userMessage && isActivePlanMessage(userMessage)
}
function stateForTurn(turn: any): any {
  if (isActivePlanTurn(turn)) return runtime.state
  const user = userMessageForTurn(turn)
  return runtime.state.executionHistory.find(item => String(item.anchorMessageId) === String(user?.id))
}
function phaseForTurn(turn: any) {
  const state = stateForTurn(turn)
  if (!state?.plannedTree) return 'idle'
  return isActivePlanTurn(turn) ? phase.value : 'finished'
}
function viewForTurn(turn: any) { return turnViews[String(userMessageForTurn(turn)?.id)] || 'list' }
function setTurnView(turn: any, value: string) { turnViews[String(userMessageForTurn(turn)?.id)] = value }
function selectTurnNode(turn: any, node: any) {
  const state = stateForTurn(turn)
  emit('node-select', { ...node, treeId: state?.currentTreeId || state?.treeId, anchorMessageId: userMessageForTurn(turn)?.id, state })
}
const activeAssistantActionsReady = computed(() => !runtime.state.isPlanning && (!runtime.state.plannedTree || runtime.state.treeCompleted))
function assistantActionsReady(_message: any, context: MessageActionContext) {
  const activeUser = messages.value.find((message: any) => isActivePlanMessage(message))
  if (!activeUser) return true
  const activeTurnId = activeUser.turnId || `turn-${activeUser.id}`
  return String(context.turnId) !== String(activeTurnId) || activeAssistantActionsReady.value
}
const phase = computed(() => {
  if (!runtime.state.plannedTree) return 'idle'
  if (runtime.state.isPreviewing) return 'plan'
  if (runtime.state.treeCompleted) return 'finished'
  return 'executing'
})
const downloadUrl = (url: string) => props.access ? url : resolveFastPlanDownloadUrl(url, props.downloadBase)
async function downloadFile(event: MouseEvent, file: any) {
  if (!props.access) return
  event.preventDefault()
  try { await downloadFastPlanFile(file.url, file.name || 'download', props.access) }
  catch { emit('error', new Error('FastPlan 文件下载失败，请检查服务连接')) }
}
function downloadLinkedFile(event: MouseEvent) {
  if (!props.access) return
  const anchor = (event.target as Element)?.closest?.('a[href]')
  const href = anchor?.getAttribute('href')
  if (!href) return
  try {
    const base = new URL(props.downloadBase || props.apiBase)
    const target = new URL(href, base)
    if (target.origin !== base.origin || !/^\/api\/(?:download\/|files\/|sessions\/[^/]+\/files\/)/.test(target.pathname)) return
    // Includes links restored inside Markdown and attachment cards.
    event.preventDefault(); event.stopPropagation()
    void downloadFile(event, { url: href, name: target.pathname.split('/').pop() || 'download' })
  } catch { /* External links retain their normal behavior. */ }
}
const fileExtension = (file: any) => String(file?.name || file?.url || '').split('?')[0].split('.').pop()?.toUpperCase() || 'FILE'
const fileKind = (file: any) => ({
  DOC: 'Word 文档', DOCX: 'Word 文档', PDF: 'PDF 文档', XLS: 'Excel 表格', XLSX: 'Excel 表格',
  PPT: 'PowerPoint 演示文稿', PPTX: 'PowerPoint 演示文稿', TXT: '文本文件', CSV: 'CSV 数据', ZIP: '压缩文件',
} as Record<string, string>)[fileExtension(file)] || '生成文件'
const fileSize = (size: unknown) => {
  const bytes = Number(size)
  if (!Number.isFinite(bytes) || bytes < 0) return ''
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}
const effectiveTheme = computed(() => props.theme === 'auto'
  ? (typeof window !== 'undefined' && window.matchMedia?.('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
  : props.theme)
const themeStyle = computed(() => createFastPlanThemeStyle(props.themeTokens))
const composerVisible = computed(() => props.composerMode === 'internal' && (props.showComposer ?? props.layout !== 'embedded'))
const effectiveScrollMode = computed(() => props.scrollMode || (props.layout === 'embedded' ? 'host' : 'internal'))
const effectiveConnectionMode = computed(() => props.connectionMode || (props.autoConnect ? 'live' : 'manual'))

async function sendChat(content: string, options: FastPlanSendOptions = {}) {
  const normalized = normalizeFastPlanSubmission(content)
  if (!normalized) return false
  if (props.beforeSend && !(await props.beforeSend(options.model || props.model, normalized))) return false
  if (componentDisposed) return false
  restoredSession.value = false
  return runtime.sendChat(normalized, { ...options, model: options.model || props.model })
}
async function submit(value: any) {
  const selected = Array.isArray(value?.attachments) ? value.attachments : props.composerAttachments
  const attachments = props.resolveComposerAttachments
    ? props.resolveComposerAttachments(selected)
    : resolveFastPlanComposerAttachments(selected)
  const content = normalizeFastPlanSubmission(value, input.value) || (attachments.length ? '请处理上传的文件' : '')
  if (!content) return
  await sendChat(content, { attachments }); input.value = ''
}
function updateInput(value: string) { input.value = value }
function handleComposerAction(value: any) { emit('composer-action', value) }
function handleComposerFiles(value: any) { emit('composer-files', value) }
function handleAttachmentRemove(attachment: any, index: number) { emit('attachment-remove', attachment, index) }
function handleAttachmentAction(value: any) { emit('attachment-action', value) }

const inlineEditId = ref<string | null>(null)
const inlineEditContent = ref('')

function messagePlainText(message: any) {
  if (!message) return ''
  if (typeof message.content === 'string' && message.content.trim()) return message.content
  return (message.parts || [])
    .filter((part: any) => part?.type === 'text')
    .map((part: any) => String(part.text || ''))
    .filter(Boolean)
    .join('\n\n')
}

function cancelInlineUserEdit() {
  inlineEditId.value = null
  inlineEditContent.value = ''
}

function submitInlineUserEdit() {
  const content = inlineEditContent.value.trim()
  if (!content || runtime.loading.value || runtime.state.isPlanning) return
  const messageId = inlineEditId.value
  cancelInlineUserEdit()
  emit('message-action', {
    action: 'resend',
    messageId,
    message: {
      protocolVersion: 1,
      id: messageId,
      role: 'user',
      createdAt: Date.now(),
      parts: [{ type: 'text', text: content, format: 'plain' }],
    },
  })
}

function handleMessageAction(value: any) {
  if (value?.action === 'edit' && value?.message?.role === 'user') {
    if (runtime.loading.value || runtime.state.isPlanning) return
    inlineEditId.value = String(value.messageId || value.message?.id || '')
    inlineEditContent.value = messagePlainText(value.message)
    return
  }
  if (value?.action === 'resend') {
    cancelInlineUserEdit()
  }
  if (value?.action === 'like' || value?.action === 'dislike') {
    const id = String(value.messageId || value.message?.id || '')
    if (id) {
      const current = Number(messageFeedback[id]) || 0
      const next = value.action === 'like'
        ? (current === 1 ? 0 : 1)
        : (current === -1 ? 0 : -1)
      messageFeedback[id] = next
      emit('message-action', { ...value, payload: { ...(value.payload || {}), feedback: next } })
      return
    }
  }
  emit('message-action', value)
}
function captureInteractionView() {
  const turn = runtime.state.activeUserMessageId
  const tree = runtime.state.plannedTree
  return () => !componentDisposed && runtime.state.activeUserMessageId === turn && runtime.state.plannedTree === tree
}
async function confirm(tree = runtime.state.plannedTree, attachments = [...props.planAttachments, ...gateFiles.value]) {
  const isCurrent = captureInteractionView()
  const result = await control({ type: 'confirm', tree, attachments })
  if (result !== false && isCurrent()) choiceState.value = 'confirmed'
  return result
}
function control(command: FastPlanInteractionCommand) {
  if (componentDisposed) return false
  return props.onControl ? props.onControl(command) : executeFastPlanInteraction(runtime, command)
}
function cancelPlan() {
  const isCurrent = captureInteractionView()
  const finish = (result: any) => {
    if (result !== false && isCurrent()) choiceState.value = 'cancelled'
    return result
  }
  const result = control({ type: 'cancel-plan' })
  return result instanceof Promise ? result.then(finish) : finish(result)
}
function reportControl(task: () => unknown) {
  // Also catch synchronous transport errors (e.g. an offline cancel).
  try { Promise.resolve(task()).catch(error => { if (!componentDisposed) emit('error', error) }) }
  catch (error) { if (!componentDisposed) emit('error', error) }
}
function choose(value: string) {
  if (value === 'confirm') reportControl(() => confirm())
  else if (value === 'cancel') reportControl(cancelPlan)
  else choiceState.value = 'editing'
}
async function handleConfirmationSelect(option: any) {
  try {
    if (confirmationModel.value?.kind === 'human') {
      await control({ type: 'human-response', response: option.action === 'submit'
        ? { action: 'submit', values: option.values || {} }
        : { value: option.value, action: option.action || option.value } })
      return
    }
    if (confirmationModel.value?.kind === 'step') {
      if (option.value === 'continue') await control({ type: 'resume' })
      else if (option.value === 'stop') await control({ type: 'cancel-execution' })
      return
    }
    if (option.value === 'edit') { choiceState.value = 'editing'; activeView.value = 'list'; return }
    if (option.value === 'cancel') { await cancelPlan(); return }
    if (option.value === 'confirm') {
      const errors = planningPanel.value?.validateTree?.() || []
      if (errors.length) throw Object.assign(new Error(errors.join('；')), { code: 'FASTPLAN_PLAN_INVALID', errors })
      await confirm(planningPanel.value?.getEditedTree?.() || runtime.state.plannedTree)
    }
  } catch (error) { if (!componentDisposed) emit('error', error) }
}
function planAction({ action, payload }: any) {
  const actions: Record<string, () => any> = { confirm: () => confirm(), cancel: cancelPlan, resume: () => control({ type: 'resume' }),
    stop: () => control({ type: 'cancel-execution' }), retry: () => { const failed: any = Object.values(runtime.state.executionState).find((node: any) => node.status === 'failed'); return failed && control({ type: 'retry-node', nodeId: failed.id }) },
    replan: async () => { const prompt = runtime.state.lastPrompt; if (await cancelPlan() !== false && !componentDisposed) return sendChat(prompt) },
    'human-response': () => control({ type: 'human-response', response: typeof payload === 'string' ? { value: payload, action: payload } : payload }) }
  reportControl(() => actions[action]?.())
}
function abortPlanningOrExecution() {
  const state = runtime.state
  reportControl(() => state.isPreviewing || state.isPlanning || !state.currentTreeId
    ? cancelPlan() : control({ type: 'cancel-execution' }))
}

watch(() => props.executionMode, mode => runtime.setExecutionMode(mode))
watch(() => props.confirmationPolicy, policy => {
  if (policy === 'legacy' || policy === 'strict') runtime.state.confirmationPolicy = policy
})
watch(() => runtime.state.plannedTree?.id || runtime.state.plannedTree?.tree_id, () => { choiceState.value = 'idle' })
watch([() => props.sessionId, effectiveConnectionMode, () => props.autoLoad], async ([id, mode, autoLoad]) => {
  choiceState.value = 'idle'
  restoredSession.value = false
  if (!id) {
    return
  }
  if (!autoLoad || mode === 'manual') {
    if (ownsRuntime) runtime.disconnect()
    return
  }
  try {
    const hasManagedSnapshot = !!props.runtime && runtime.state.sessionId === id && (
      runtime.loading.value
      || runtime.state.connectionStatus === 'connecting'
      || runtime.state.connectionStatus === 'reconnecting'
      || runtime.connected.value
      || runtime.state.messages.length > 0
      || !!runtime.state.plannedTree
      || runtime.state.isPlanning
      || runtime.state.isPreviewing
      || runtime.state.isPaused
      || runtime.state.executionHistory.length > 0
    )
    if (hasManagedSnapshot) {
      // The application-level manager kept this controller alive while the
      // renderer was absent. Reuse its reactive snapshot immediately instead
      // of clearing it through another loadSession round trip.
      if (mode === 'live' && !runtime.loading.value && !runtime.connected.value) runtime.connect(id)
      restoredSession.value = true
      emit('state-change', {
        event: { type: 'managed_snapshot', session_id: id },
        state: runtime.state,
      })
      return
    }
    if (mode === 'history') runtime.closeSession(id)
    await runtime.loadSession(id, { connect: mode === 'live' })
    if (mode === 'history') runtime.closeSession(id)
    restoredSession.value = true
  } catch (error) {
    emit('error', error)
  }
}, { immediate: true })
onBeforeUnmount(() => {
  componentDisposed = true
  composerTargetObserver?.disconnect()
  for (const remove of removeRuntimeListeners) remove?.()
  if (ownsRuntime) runtime.disconnect()
})
defineExpose({ runtime, state: runtime.state, sendChat, loadSession: runtime.loadSession, disconnect: runtime.disconnect,
  confirmPlan: confirm,
  cancelPlan, cancelExecution: () => control({ type: 'cancel-execution' }), pauseExecution: () => control({ type: 'pause' }),
  resumeExecution: () => control({ type: 'resume' }), retryNode: (nodeId: string) => control({ type: 'retry-node', nodeId }),
  skipNode: (nodeId: string) => control({ type: 'skip-node', nodeId }),
  respondHumanCheckpoint: (response: any, request?: any) => control({ type: 'human-response', response, request }),
  updateNodeOutput: (nodeId: string, output: any) => control({ type: 'edit-node-output', nodeId, output }),
  updateNextNodeConfig: (nodeId: string, changes: any) => control({ type: 'edit-node-config', nodeId, changes }),
  executeTemplate: (...args: Parameters<FastPlanExecutionController['executeTemplate']>) => componentDisposed ? Promise.resolve(false) : runtime.executeTemplate(...args),
  saveAsTemplate: (...args: Parameters<FastPlanExecutionController['saveAsTemplate']>) => componentDisposed ? Promise.resolve(false) : runtime.saveAsTemplate(...args) })
</script>

<template>
  <section ref="conversationRoot" class="fastplan-conversation" @click.capture="downloadLinkedFile" :class="`fastplan-conversation--${layout}`" :data-theme="effectiveTheme" :data-layout="layout" :data-density="contentDensity" :data-appearance="appearance === 'default' ? undefined : appearance" :style="themeStyle">
    <slot name="top-bar" :runtime="runtime" :state="runtime.state" />
    <div v-if="showStandaloneError && errorPlacement === 'top'" class="fastplan-conversation__error"><slot name="error" :error="runtime.state.error">{{ runtime.state.error.message || runtime.state.error.error || 'FastPlan 执行失败' }}</slot></div>
    <UntypedChatConversation v-model="input" :messages="messages"
      :show-context="showContext" :context-items="contextItems" :context-placement="contextPlacement"
      :show-composer="composerVisible" :scroll-mode="effectiveScrollMode" :auto-scroll="effectiveScrollMode === 'internal'"
      :appearance="appearance" :show-composer-auxiliary="false"
      :max-render-chars="maxDisplayChars"
      :theme="effectiveTheme" :style="themeStyle"
      :initializing="runtime.loading.value" :assistant-loading="runtime.state.isPlanning" :composer-loading="runtime.state.isPlanning"
      :composer-placeholder="placeholder" :composer-disabled="composerDisabled" :composer-readonly="composerReadonly"
      :composer-actions="composerActions" :composer-attachments="composerAttachments" :send-by-ctrl-enter="sendByCtrlEnter"
      :show-actions="showActions" :assistant-actions-ready="assistantActionsReady" :actions="actions" :resolve-actions="resolveActions"
      @submit="submit" @abort="abortPlanningOrExecution" @plan-action="planAction"
      @composer-action="handleComposerAction" @composer-files="handleComposerFiles"
      @attachment-remove="handleAttachmentRemove" @attachment-action="handleAttachmentAction" @message-action="handleMessageAction"
      @context-action="emit('context-action', $event)">
      <template v-if="$slots.avatar" #avatar="scope"><slot name="avatar" v-bind="scope" /></template>
      <template v-if="$slots.header" #header="scope"><slot name="header" v-bind="scope" /></template>
      <template #context-item="scope">
        <slot name="context-item" v-bind="scope"><FastPlanContextItem v-bind="scope" @option-select="emit('context-option-select', $event)" /></slot>
      </template>
      <template v-if="$slots['message-card']" #messages>
        <slot name="message-card" :messages="messages" :state="runtime.state" :runtime="runtime" />
      </template>
      <template v-if="$slots['action-bar']" #action-bar="scope">
        <slot name="action-bar" v-bind="scope" />
      </template>
      <template v-if="$slots.action" #action="scope">
        <slot name="action" v-bind="scope" />
      </template>
      <template v-if="$slots['action-icon']" #action-icon="scope">
        <slot name="action-icon" v-bind="scope" />
      </template>
      <template v-if="$slots['action-menu']" #action-menu="scope">
        <slot name="action-menu" v-bind="scope" />
      </template>
      <template #message-before-content="{ message, turn, isFirstAssistantInTurn }">
        <template v-if="message.role === 'assistant' && isFirstAssistantInTurn && stateForTurn(turn)">
          <slot v-if="!isActivePlanTurn(turn) && $slots['historical-planning-card']" name="historical-planning-card" :plan="planForTurn(turn)" :state="stateForTurn(turn)" />
          <FastPlanPlanningPanel v-else :key="userMessageForTurn(turn)?.id"
            :ref="(panel: any) => { if (isActivePlanTurn(turn)) planningPanel = panel }"
            :data-planning-turn="userMessageForTurn(turn)?.id"
            :state="stateForTurn(turn)" :phase="phaseForTurn(turn)" :prompt="stateForTurn(turn).lastPrompt"
            :theme="effectiveTheme"
            :density="contentDensity"
            :choice-state="isActivePlanTurn(turn) ? choiceState : 'confirmed'"
            :choice-options="isActivePlanTurn(turn) ? [{ label: '确认执行', key: 'y', value: 'confirm' }, { label: '修改计划', key: 'e', value: 'edit' }, { label: '取消', key: 'n', value: 'cancel' }] : []"
            :active-view="viewForTurn(turn)" :human-request="isActivePlanTurn(turn) ? (runtime.state.humanRequestQueue?.[0] || runtime.state.humanRequest) : null"
            :confirmed-history="stateForTurn(turn).confirmedHistory || []"
            :api-base="uploadBase || apiBase" :admin-base="adminBase || apiBase"
            @view-change="setTurnView(turn, $event)" @update:active-view="setTurnView(turn, $event)"
            @choice-pick="isActivePlanTurn(turn) && choose($event)" @node-select="selectTurnNode(turn, $event)"
            @confirm="isActivePlanTurn(turn) && reportControl(() => confirm($event))" @edit="isActivePlanTurn(turn) && (choiceState = 'editing')"
            @cancel="isActivePlanTurn(turn) && reportControl(cancelPlan)" @files-change="isActivePlanTurn(turn) && (gateFiles = $event)"
            @human-response="isActivePlanTurn(turn) && reportControl(() => control({ type: 'human-response', response: $event }))" />
        </template>
      </template>
      <template #message-text="{ part, message }">
        <FastPlanAnswer v-if="message.role === 'assistant' && message.metadata?.contentKind === 'final_answer'"
          kind="final_answer" :content="part.text" :theme="effectiveTheme" />
        <div v-else-if="part.format === 'plain' || message.role === 'user'" class="jcr-plain-text">{{ part.text }}</div>
        <pre v-else-if="part.format === 'json'" class="jcr-json">{{ part.text }}</pre>
        <button v-else-if="isProcessMessageCollapsed(message)" type="button"
          class="fastplan-conversation__process-summary" @click="setProcessMessageExpanded(message, true)">
          <span>{{ summarizeFastPlanProcessText(part.text) }}</span>
          <small>展开过程内容</small>
        </button>
        <div v-else-if="isDeferredProcessMessage(message)" class="fastplan-conversation__process-detail">
          <button type="button" @click="setProcessMessageExpanded(message, false)">收起过程内容</button>
          <ChatMarkdownRenderer :content="part.text" :theme="effectiveTheme" :max-chars="maxDisplayChars" />
        </div>
        <ChatMarkdownRenderer v-else :content="part.text" :theme="effectiveTheme" :max-chars="maxDisplayChars"
          :streaming="message.status === 'streaming'" />
      </template>
      <template #message-after-content="{ message }">
        <div
          v-if="message.role === 'user' && inlineEditId != null && String(inlineEditId) === String(message.id)"
          class="fastplan-inline-user-edit"
          @click.stop
        >
          <button class="fastplan-inline-edit-cancel" type="button" title="取消" @click="cancelInlineUserEdit">
            <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none">
              <path d="M18 6L6 18M6 6l12 12" />
            </svg>
          </button>
          <textarea
            v-model="inlineEditContent"
            class="fastplan-inline-edit-input"
            rows="2"
            autofocus
            @keydown.enter.exact.prevent="submitInlineUserEdit"
            @keydown.esc.prevent="cancelInlineUserEdit"
          />
          <button
            class="fastplan-inline-edit-submit"
            type="button"
            title="确认"
            :disabled="!inlineEditContent.trim() || runtime.loading.value || runtime.state.isPlanning"
            @click="submitInlineUserEdit"
          >
            <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2.4" fill="none">
              <polyline points="20 6 9 17 4 12" />
            </svg>
          </button>
        </div>
        <template v-for="result in resultForMessage(message)" :key="message.id">
        <slot
          name="result-card" :result="result.finalResult" :files="result.files" :images="result.images" :charts="result.charts" :state="result.state">
          <article class="fastplan-conversation__artifact-result">
            <div v-if="Array.isArray(result.finalResult?.degraded) && result.finalResult.degraded.length"
              class="fastplan-conversation__degraded">
              <div v-for="reason in result.finalResult.degraded" :key="reason">⚠ {{ reason }}</div>
            </div>
            <div v-if="result.images.length" class="fastplan-conversation__images">
              <a v-for="(image, index) in result.images" :key="`${image.src.length}-${index}`" :href="image.src" target="_blank" rel="noopener">
                <img :src="image.src" :alt="`FastPlan 结果图片 ${Number(index) + 1}`" loading="lazy" />
              </a>
            </div>
            <div v-if="result.files.length" class="fastplan-conversation__files">
              <a v-for="file in result.files" :key="file.url" :href="downloadUrl(file.url)" @click="downloadFile($event, file)" target="_blank" rel="noopener"
                :aria-label="`下载 ${file.name}`">
                <span class="fastplan-conversation__file-icon" :data-kind="fileExtension(file)">{{ fileExtension(file) }}</span>
                <span class="fastplan-conversation__file-info">
                  <strong>{{ file.name }}</strong>
                  <small>{{ fileKind(file) }}<template v-if="fileSize(file.size)"> · {{ fileSize(file.size) }}</template></small>
                </span>
                <span class="fastplan-conversation__download-action">
                  <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3v12m0 0 5-5m-5 5-5-5M5 21h14" /></svg>
                  下载
                </span>
              </a>
            </div>
          </article>
        </slot>
        </template>
      </template>
      <template #composer-before>
        <div v-if="composerMode === 'internal' && interactionPlacement !== 'inline' && (showComposerTaskList || $slots['attachment-input'])" class="fastplan-conversation__composer-before">
          <FastPlanTaskList v-if="showComposerTaskList" :plan="composerPlan" :confirmation="confirmationModel" :density="contentDensity"
            :initially-collapsed="restoredSession" @confirmation-select="handleConfirmationSelect" />
          <slot name="attachment-input" :runtime="runtime" :state="runtime.state" />
        </div>
      </template>
      <template v-if="$slots.composer" #composer>
        <slot name="composer" :runtime="runtime" :state="runtime.state" :model-value="input"
          :update-model-value="updateInput" :submit="submit" />
      </template>
    </UntypedChatConversation>
    <div v-if="showStandaloneError && errorPlacement === 'inline'" class="fastplan-conversation__error fastplan-conversation__error--inline"><slot name="error" :error="runtime.state.error">{{ runtime.state.error.message || runtime.state.error.error || 'FastPlan 执行失败' }}</slot></div>
    <Teleport v-if="composerMode === 'host' && composerTargetElement && (showComposerTaskList || $slots['attachment-input'])" :to="composerTargetElement">
      <div class="fastplan-conversation__host-composer-contribution" :data-density="composerDensity">
        <FastPlanTaskList v-if="showComposerTaskList" :plan="composerPlan" :confirmation="confirmationModel" :density="composerDensity"
          :initially-collapsed="restoredSession" @confirmation-select="handleConfirmationSelect" />
        <slot name="attachment-input" :runtime="runtime" :state="runtime.state" />
      </div>
    </Teleport>
    <div v-else-if="composerMode === 'host' && (showComposerTaskList || $slots['attachment-input'])" class="fastplan-conversation__inline-interaction" data-composer-fallback="inline" :data-density="contentDensity">
      <FastPlanTaskList v-if="showComposerTaskList" :plan="composerPlan" :confirmation="confirmationModel" :density="contentDensity"
        :initially-collapsed="restoredSession" @confirmation-select="handleConfirmationSelect" />
      <slot name="attachment-input" :runtime="runtime" :state="runtime.state" />
    </div>
    <div v-else-if="composerMode === 'internal' && interactionPlacement === 'inline' && (showComposerTaskList || $slots['attachment-input'])" class="fastplan-conversation__inline-interaction" :data-density="contentDensity">
      <FastPlanTaskList v-if="showComposerTaskList" :plan="composerPlan" :confirmation="confirmationModel" :density="contentDensity"
        :initially-collapsed="restoredSession" @confirmation-select="handleConfirmationSelect" />
      <slot name="attachment-input" :runtime="runtime" :state="runtime.state" />
    </div>
  </section>
</template>
