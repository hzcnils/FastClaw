import type { Component } from 'vue'

export const CHAT_PROTOCOL_VERSION = 1 as const
export const CHAT_WORKSPACE_PROTOCOL_VERSION = 1 as const

export type ChatRole = 'user' | 'assistant' | 'system' | 'tool'
export type ChatMessageStatus = 'pending' | 'streaming' | 'complete' | 'error'

export interface TextPart {
  type: 'text'
  text: string
  format?: 'plain' | 'markdown' | 'json'
}

export interface ReasoningPart {
  type: 'reasoning'
  text: string
  label?: string
  status?: 'streaming' | 'complete'
}

export interface ImagePart {
  type: 'image'
  src: string
  alt?: string
  name?: string
}

export interface AttachmentPart {
  type: 'attachment'
  id?: string
  name: string
  url?: string
  size?: number
  mimeType?: string
  metadata?: Record<string, unknown>
}

export interface ToolCallPart {
  type: 'tool-call'
  id: string
  name: string
  input?: unknown
  output?: unknown
  status?: 'pending' | 'running' | 'complete' | 'error'
  error?: string
}

export interface NoticePart {
  type: 'notice'
  text: string
  tone?: 'neutral' | 'info' | 'success' | 'warning' | 'error'
}

export interface CustomPart {
  type: 'custom'
  kind: string
  data: unknown
}

export interface ChatTokenUsageContextPart {
  id: string
  label: string
  value: number
  color?: string
}

/** Host-normalized token usage data for the reusable usage summary component. */
export interface ChatTokenUsageData {
  round?: number | null
  step?: number | null
  model?: string
  timestamp?: string
  estimated?: boolean
  inputTokens: number
  outputTokens: number
  cumulativeInputTokens?: number | null
  cumulativeOutputTokens?: number | null
  cacheReadTokens?: number | null
  cacheMissTokens?: number | null
  firstTokenMs?: number | null
  outputTokensPerSecond?: number | null
  llmDurationMs?: number | null
  toolDurationMs?: number | null
  totalDurationMs?: number | null
  steps?: number | null
  contextParts?: ChatTokenUsageContextPart[]
}

export interface ChatTokenUsageLabels {
  roundPrefix: string
  roundSuffix: string
  inputTokens: string
  outputTokens: string
  cache: string
  cacheHitRate: string
  cacheUnavailable: string
  currentRound: string
  cumulative: string
  performance: string
  firstToken: string
  llm: string
  tool: string
  endToEnd: string
  stepsSuffix: string
  contextComposition: string
  contextUnit: string
  expandDetails: string
  collapseDetails: string
}

export interface ChatTokenUsageFormatters {
  compact?: (value: number) => string
  exact?: (value: number) => string
  duration?: (milliseconds: number) => string
  rate?: (value: number) => string
}

export type ChatPart = TextPart | ReasoningPart | ImagePart | AttachmentPart | ToolCallPart | NoticePart | CustomPart

export interface ChatModelInfo {
  id?: string
  name?: string
  avatar?: string
}

export type ChatErrorCategory =
  | 'quota'
  | 'authentication'
  | 'permission'
  | 'account-restricted'
  | 'model-unavailable'
  | 'gateway'
  | 'rate-limit'
  | 'timeout'
  | 'server'
  | 'network'
  | 'unknown'

export interface ChatErrorAction {
  id: string
  label: string
  kind?: 'primary' | 'secondary'
}

/** Structured error data. When omitted, the renderer classifies text parts. */
export interface ChatMessageError {
  raw: string
  statusCode?: number
  code?: string
  category?: ChatErrorCategory
  message?: string
  actions?: ChatErrorAction[]
}

export interface ChatRetryProgress {
  attempt: number
  total: number
  label?: string
}

export interface ChatMessage {
  protocolVersion?: typeof CHAT_PROTOCOL_VERSION
  id: string
  role: ChatRole
  createdAt: string | number | Date
  model?: ChatModelInfo
  status?: ChatMessageStatus
  error?: ChatMessageError
  retry?: ChatRetryProgress
  parts: ChatPart[]
  feedback?: -1 | 0 | 1
  turnId?: string
  /** Optional derived answers, such as host-provided translations. */
  translations?: ChatMessage[]
  metadata?: Record<string, unknown>
}

export type ChatTimelineEntryStatus = 'pending' | 'active' | 'complete' | 'error'
export type ChatTurnComparisonMode = 'single' | 'tabs' | 'split'

export interface ChatMessageTimelineEntry {
  protocolVersion?: typeof CHAT_WORKSPACE_PROTOCOL_VERSION
  kind: 'message'
  id: string
  message: ChatMessage
}

export interface ChatTurnTimelineEntry {
  protocolVersion?: typeof CHAT_WORKSPACE_PROTOCOL_VERSION
  kind: 'turn'
  id: string
  user: ChatMessage
  alternatives: ChatMessage[]
  activeId?: string
  comparisonMode?: ChatTurnComparisonMode
  metadata?: Record<string, unknown>
}

export interface ChatEmbeddedTimelineEntry {
  protocolVersion?: typeof CHAT_WORKSPACE_PROTOCOL_VERSION
  kind: 'embedded'
  id: string
  renderer: string
  data: unknown
  status?: ChatTimelineEntryStatus
  metadata?: Record<string, unknown>
}

/**
 * Mixed conversation timeline consumed by the future ChatWorkspace component.
 * Host-specific persistence fields must be normalized before entering this type.
 */
export type ChatTimelineEntry = ChatMessageTimelineEntry | ChatTurnTimelineEntry | ChatEmbeddedTimelineEntry

export interface ChatTimelineSubmission {
  entry: ChatTimelineEntry
  /** Replace an active embedded entry instead of appending another instance. */
  replaceEntryId?: string
}

export interface NormalizedChatInput {
  id?: string
  text: string
  attachments?: ChatComposerAttachment[]
  modelId?: string
  contextItems?: ChatContextItem[]
  metadata?: Record<string, unknown>
}

export interface ChatActionContext {
  conversationId?: string
  entryId?: string
  turnId?: string
  messageId?: string
  signal?: AbortSignal
  metadata?: Record<string, unknown>
}

export interface ChatStreamError {
  message: string
  code?: string
  recoverable?: boolean
  details?: unknown
}

export type ChatStreamEvent =
  | { type: 'message-start'; message: ChatMessage }
  | { type: 'reasoning-delta'; messageId?: string; delta: string }
  | { type: 'text-delta'; messageId?: string; delta: string }
  | { type: 'part'; messageId?: string; part: ChatPart }
  | { type: 'attachments-ready'; messageId?: string; attachments: AttachmentPart[] }
  | { type: 'message-complete'; messageId?: string; message?: ChatMessage }
  | { type: 'error'; error: ChatStreamError }

export type ChatWorkspaceActionScope = 'workspace' | 'timeline' | 'message' | 'composer' | 'context' | 'auxiliary'

export type ChatWorkspaceActionName = MessageActionName
  | 'error-action'
  | 'submit' | 'abort' | 'new-chat'
  | 'model-change' | 'context-change' | 'composer-action' | 'attachment-remove'
  | 'load-older' | 'load-all' | 'recover'
  | 'prompt-request' | 'clear-conversation' | 'export-conversation'
  | 'toggle-thinking' | 'toggle-planning' | 'voice' | 'open-auxiliary'
  | 'variant-change' | 'comparison-mode-change' | 'navigate' | 'auxiliary-change' | 'attachment-retry'

export interface ChatWorkspaceActionEvent {
  protocolVersion?: typeof CHAT_WORKSPACE_PROTOCOL_VERSION
  action: ChatWorkspaceActionName
  scope: ChatWorkspaceActionScope
  conversationId?: string
  entryId?: string
  turnId?: string
  messageId?: string
  actionId?: string
  payload?: unknown
  nativeEvent?: Event
  metadata?: Record<string, unknown>
}

export interface ChatActionResult {
  status: 'success' | 'cancelled' | 'error'
  data?: unknown
  error?: ChatStreamError
}

export interface ChatWorkspaceActionResultEvent {
  event: ChatWorkspaceActionEvent
  result: ChatActionResult
}

/** Host adapter for ordinary chat only. FastPlan networking remains inside planflow-runtime. */
export interface ChatWorkspaceAdapter {
  /**
   * Optionally route a submission to an embedded timeline renderer instead of
   * creating the ordinary optimistic chat turn. Returning no entry preserves
   * the existing send-stream behavior.
   */
  createTimelineEntry?(
    input: NormalizedChatInput,
    context: ChatActionContext,
  ): Promise<ChatTimelineEntry | ChatTimelineSubmission | undefined>
    | ChatTimelineEntry
    | ChatTimelineSubmission
    | undefined
  send(input: NormalizedChatInput, context: ChatActionContext): AsyncIterable<ChatStreamEvent>
  abort?(context: ChatActionContext): Promise<void> | void
  executeAction?(event: ChatWorkspaceActionEvent): Promise<ChatActionResult | void> | ChatActionResult | void
}

export interface ChatWorkspacePlugins {
  timelineRenderers?: Record<string, Component>
  partRenderers?: Record<string, Component>
  auxiliaryPanels?: Record<string, Component>
}

/** Public input contract for the opt-in workspace component. */
export interface ChatWorkspaceProps {
  timeline?: ChatTimelineEntry[]
  defaultTimeline?: ChatTimelineEntry[]
  messages?: ChatMessage[]
  adapter?: ChatWorkspaceAdapter
  plugins?: ChatWorkspacePlugins
  conversationId?: string
  modelValue?: string
  selectedModelId?: string
  contextItems?: ChatContextItem[]
  contextPlacement?: ChatContextPlacement
  showContext?: boolean
  showComposer?: boolean
  scrollMode?: 'internal' | 'host'
  appearance?: 'default' | 'seamless'
  initializing?: boolean
  composerLoading?: boolean
  composerDisabled?: boolean
  composerReadonly?: boolean
  composerPlaceholder?: string
  composerActions?: ChatComposerAction[]
  composerSelectors?: ChatComposerSelector[]
  composerAttachments?: ChatComposerAttachment[]
  composerMaxHeight?: number
  composerAutoResize?: boolean
  sendByCtrlEnter?: boolean
  loadingState?: LoadingState
  paginationState?: PaginationState
  showActions?: boolean
  assistantActionsReady?: AssistantActionsReady
  actions?: MessageActionsConfig
  resolveActions?: ActionResolver
  rendererAdapters?: ChatRendererAdapters
  toolRenderers?: ToolRendererRegistry
  labels?: Partial<ChatRendererMessages>
  locale?: string
  theme?: 'light' | 'dark' | 'auto'
  themeTokens?: ChatThemeTokens
  allowHtml?: boolean
  maxRenderChars?: number
  markdownAutoFoldThreshold?: number
  markdownShowCodeRowNumber?: boolean
  autoScroll?: boolean
  showScrollToBottom?: boolean
  showQuestionNavigator?: boolean
  questionNavigatorCollapsed?: boolean
  auxiliaryItems?: ChatAuxiliaryItem[]
  activeAuxiliaryId?: string
  auxiliaryPlacement?: ChatAuxiliaryPlacement
  /** Whether a side panel shares the whole workspace or only the scrollable content above the composer. */
  auxiliaryScope?: 'workspace' | 'content'
  metadata?: Record<string, unknown>
}

export interface ChatThemeTokens {
  background?: string
  foreground?: string
  muted?: string
  border?: string
  primary?: string
  userBackground?: string
  assistantBackground?: string
  danger?: string
  codeBackground?: string
  surface?: string
  subtleBackground?: string
  radius?: string
  fontSize?: string
  gap?: string
  bubbleMaxWidth?: string
  auxiliaryWidth?: string
}

export interface ChatQuestionNavigationItem {
  id: string
  entryId: string
  label: string
  index: number
  /** Optional message target rendered inside an embedded timeline entry. */
  targetMessageId?: string
  disabled?: boolean
  metadata?: Record<string, unknown>
}

/** Navigation contribution emitted by an embedded timeline renderer. */
export interface ChatEmbeddedNavigationItem {
  id: string
  label: string
  targetMessageId?: string
  disabled?: boolean
  metadata?: Record<string, unknown>
}

export interface ChatEmbeddedNavigationChange {
  type: 'navigation-change'
  items: ChatEmbeddedNavigationItem[]
}

export interface ChatQuestionNavigateEvent {
  item: ChatQuestionNavigationItem
  nativeEvent: MouseEvent | KeyboardEvent
}

export interface ChatVariantChangeEvent {
  turnId: string
  activeId: string
  previousActiveId?: string
}

export interface ChatComparisonModeChangeEvent {
  turnId: string
  mode: ChatTurnComparisonMode
}

export type ChatAuxiliaryPlacement = 'inline' | 'side' | 'overlay'

export interface ChatAuxiliaryItem {
  id: string
  renderer: string
  title?: string
  data: unknown
  visible?: boolean
  closable?: boolean
  metadata?: Record<string, unknown>
}

export interface ChatTimelineRendererProps {
  entry: ChatEmbeddedTimelineEntry
  data: unknown
  status?: ChatTimelineEntryStatus
}

export interface ChatComposerAttachmentActionEvent {
  action: 'remove' | 'retry' | 'preview' | 'custom'
  attachment: ChatComposerAttachment
  index: number
  nativeEvent?: Event
  payload?: unknown
}

export type MessageActionName =
  | 'copy' | 'edit' | 'resend' | 'regenerate' | 'quote' | 'tts'
  | 'translate' | 'favorite' | 'edit-as-doc' | 'switch-model' | 'trace-source'
  | 'export' | 'feedback' | 'like' | 'dislike'
  | 'branch' | 'delete' | 'custom'

/** Resolver-only marker: render a divider before the next visible action. */
export const MESSAGE_ACTION_DIVIDER = '__divider__' as const
export type MessageActionResolverEntry = MessageActionName | typeof MESSAGE_ACTION_DIVIDER

export type ChatActionIconName = MessageActionName | 'copied' | 'chevron-down' | 'loader'

export interface MessageActionMenuItem {
  id: string
  label: string
  disabled?: boolean
  danger?: boolean
  payload?: unknown
}

export interface MessageActionConfig {
  enabled?: boolean
  visible?: boolean
  disabled?: boolean
  /** Show an in-progress indicator and prevent duplicate invocation. */
  loading?: boolean
  label?: string
  /** Text glyph override. Built-in actions use the package SVG icon set. */
  icon?: string
  tooltip?: string
  order?: number
  danger?: boolean
  requireConfirm?: boolean
  actionId?: string
  payload?: unknown
  menuItems?: MessageActionMenuItem[]
  /** Use the action-menu slot for this action's menu body. */
  customMenu?: boolean
  /** Render a visual separator immediately before this action. */
  dividerBefore?: boolean
}

export interface ResolvedMessageAction extends MessageActionConfig {
  name: MessageActionName
  actionId: string
  visible: boolean
  disabled: boolean
  label: string
  order: number
}

export interface MessageActionContext {
  index: number
  turnId?: string
  isLastMessage: boolean
  isLastCompletedAssistantTurn: boolean
  isGenerating: boolean
  hasStableId: boolean
}

/**
 * Resolve the final actions for a message. A name array is rendered in the
 * exact returned order; a resolved-action array is sorted by each item.order.
 */
export type ActionResolver = (
  message: ChatMessage,
  context: MessageActionContext,
) => MessageActionResolverEntry[] | ResolvedMessageAction[]

export interface MessageActionEvent {
  action: string
  messageId: string
  turnId?: string
  message: ChatMessage
  selectedText?: string
  payload?: unknown
  menuItemId?: string
  menuItem?: MessageActionMenuItem
}

export interface PaginationState {
  hasMore?: boolean
  loading?: boolean
  loadingAll?: boolean
  total?: number
}

export interface LoadingState {
  initial?: boolean
  generating?: boolean
  stalled?: boolean
  label?: string
}

export interface ChatRendererMessages {
  empty: string
  loading: string
  generating: string
  stalled: string
  recover: string
  loadOlder: string
  loadAll: string
  copy: string
  copied: string
  edit: string
  resend: string
  regenerate: string
  quote: string
  tts: string
  translate: string
  favorite: string
  'edit-as-doc': string
  'switch-model': string
  'trace-source': string
  export: string
  feedback: string
  like: string
  dislike: string
  branch: string
  delete: string
  confirm: string
  cancel: string
  reasoning: string
  toolInput: string
  toolOutput: string
  unknownContent: string
  showFullContent: string
  newMessages: string
  scrollToBottom: string
  more: string
  questionDirectory: string
  openQuestionDirectory: string
  closeQuestionDirectory: string
  modelVariants: string
  tabs: string
  split: string
  missingRenderer: string
  close: string
  attachmentIdle: string
  attachmentUploading: string
  attachmentProcessing: string
  attachmentComplete: string
  attachmentError: string
  attachmentReference: string
}

export interface ChatRendererAdapters {
  clipboard?: (text: string) => boolean | Promise<boolean>
  imagePreview?: (images: ImagePart[], activeIndex: number) => void
  download?: (data: Blob | string, filename: string) => void | Promise<void>
  link?: (href: string, event: MouseEvent) => void
  formatDate?: (value: ChatMessage['createdAt'], message: ChatMessage) => string
}

export type ToolRendererRegistry = Record<string, Component>
export type MessageActionsConfig = Partial<Record<MessageActionName, boolean | MessageActionConfig>>

export type ChatContextPlacement = 'top' | 'composer'
export type ChatContextItemKind = 'document' | 'knowledge' | 'assistant' | 'model' | 'settings' | 'custom'

export interface ChatContextItem {
  id: string
  kind: ChatContextItemKind
  label: string
  value?: string
  icon?: string
  badge?: string | number
  visible?: boolean
  disabled?: boolean
  active?: boolean
  metadata?: Record<string, unknown>
}

export interface ChatContextActionEvent {
  action: 'select'
  item: ChatContextItem
  nativeEvent: MouseEvent | KeyboardEvent
}

export type ChatComposerAttachmentKind = 'image' | 'file' | 'task' | 'custom'
export type ChatComposerAttachmentStatus = 'idle' | 'uploading' | 'processing' | 'complete' | 'error'

export interface ChatComposerAttachment {
  id: string
  kind: ChatComposerAttachmentKind
  name: string
  url?: string
  previewUrl?: string
  mimeType?: string
  size?: number
  status?: ChatComposerAttachmentStatus
  progress?: number
  error?: string
  metadata?: Record<string, unknown>
}

export interface ChatComposerAction {
  id: string
  label: string
  kind?: 'button' | 'file'
  tooltip?: string
  icon?: string
  position?: 'start' | 'end'
  visible?: boolean
  disabled?: boolean
  active?: boolean
  order?: number
  accept?: string
  multiple?: boolean
  payload?: unknown
}

export interface ChatComposerActionEvent {
  action: string
  config: ChatComposerAction
  nativeEvent: MouseEvent
  payload?: unknown
}

export interface ChatComposerFileSelectionEvent {
  action: string
  config: ChatComposerAction
  files: File[]
  nativeEvent: Event
}

export interface ChatComposerSelectorOption {
  value: string
  label: string
  group?: string
  description?: string
  icon?: string
  disabled?: boolean
  metadata?: Record<string, unknown>
}

export interface ChatComposerSelector {
  id: string
  label: string
  value?: string | string[]
  options: ChatComposerSelectorOption[]
  placeholder?: string
  searchable?: boolean
  clearable?: boolean
  multiple?: boolean
  disabled?: boolean
  visible?: boolean
  maxVisibleValues?: number
  metadata?: Record<string, unknown>
}

/** Controls assistant action bars globally or for each individual message turn. */
export type AssistantActionsReady = boolean | ((
  message: ChatMessage,
  context: MessageActionContext,
) => boolean)

/** Public config alias that does not collide with the ChatComposerSelector component export. */
export type ChatComposerSelectorConfig = ChatComposerSelector

export interface ChatComposerSelectorChangeEvent {
  selector: ChatComposerSelector
  value: string | string[]
  selected: ChatComposerSelectorOption[]
  nativeEvent: MouseEvent | KeyboardEvent
}

export interface ChatComposerSubmitEvent {
  text: string
  attachments: ChatComposerAttachment[]
  nativeEvent: Event
}

/** Shared state and behavior exposed to every composer region slot. */
export interface ChatComposerRegionScope {
  value: string
  attachments: ChatComposerAttachment[]
  selectors: ChatComposerSelector[]
  actions: ChatComposerAction[]
  disabled: boolean
  readonly: boolean
  loading: boolean
  canSubmit: boolean
  update: (value: string) => void
  submit: (nativeEvent?: Event) => void
  abort: (nativeEvent?: MouseEvent) => void
  triggerAction: (action: ChatComposerAction, nativeEvent: MouseEvent) => void
}
