import type { FastPlanConfirmationPolicy, FastPlanExecutionController, FastPlanExecutionMode, FastPlanModelConfig, FastPlanRuntimeState } from 'planflow-runtime'
import type { FastPlanAccess } from 'planflow-runtime'
import type { FastPlanInteractionCommand } from './interaction'
import type {
  ActionResolver,
  ChatComposerAction,
  ChatComposerActionEvent,
  ChatComposerAttachment,
  ChatComposerAttachmentActionEvent,
  ChatComposerFileSelectionEvent,
  ChatComposerRegionScope,
  ChatComposerSelectorConfig,
  ChatComposerSelectorChangeEvent,
  ChatComposerSubmitEvent,
  ChatEmbeddedNavigationItem,
  ChatRendererMessages,
  ChatMessage,
  MessageActionEvent,
  MessageActionMenuItem,
  MessageActionsConfig,
  ResolvedMessageAction,
} from 'chat-renderer'
import type { FastPlanPlanningMessages } from 'planflow-planning-ui'

export type FastPlanThemeMode = 'light' | 'dark' | 'auto'
export type FastPlanDensity = 'auto' | 'comfortable' | 'compact'
export type FastPlanComposerSelector = ChatComposerSelectorConfig

export interface FastPlanThemeTokens {
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
}

export interface FastPlanConversationProps {
  access?: FastPlanAccess
  beforeSend?: (selection: unknown, prompt: string) => Promise<boolean>
  onControl?: (command: FastPlanInteractionCommand) => Promise<boolean>
  sessionId: string
  apiBase?: string
  wsBase?: string
  parentToken?: string
  getConnection?: () => Promise<{ wsBase: string; apiBase: string; parentToken: string }>
  model?: string | FastPlanModelConfig
  executionMode?: FastPlanExecutionMode
  confirmationPolicy?: FastPlanConfirmationPolicy
  maxReconnectAttempts?: number
  baseReconnectDelayMs?: number
  uploadBase?: string
  adminBase?: string
  downloadBase?: string
  retainConnectionOnSessionSwitch?: boolean
  placeholder?: string
  composerDisabled?: boolean
  composerReadonly?: boolean
  composerActions?: ChatComposerAction[]
  composerSelectors?: FastPlanComposerSelector[]
  composerSelectorPlacement?: 'composer' | 'header' | 'hidden'
  composerAttachments?: ChatComposerAttachment[]
  composerMaxHeight?: number
  composerAutoResize?: boolean
  sendByCtrlEnter?: boolean
  showActions?: boolean
  actions?: MessageActionsConfig
  resolveActions?: ActionResolver
  autoConnect?: boolean
  theme?: FastPlanThemeMode
  themeTokens?: FastPlanThemeTokens
  showContext?: boolean
  contextItems?: Array<Record<string, unknown>>
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
  /** Built-in locale. Defaults to zh-CN; English is selected with en-US. */
  locale?: string
  /** Overrides chat-renderer's generic language pack. */
  labels?: Partial<ChatRendererMessages>
  /** Overrides FastPlan planning controls' language pack. */
  planningLabels?: Partial<FastPlanPlanningMessages>
  /** Responsive information density. Auto observes the rendered content width. */
  density?: FastPlanDensity
  /** Width at or below which auto density becomes compact. Defaults to 700px. */
  compactBreakpoint?: number
}

export interface FastPlanConversationEventMap {
  'state-change': unknown
  'loading-change': boolean
  'message-sent': unknown
  'plan-completed': unknown
  'execution-completed': unknown
  'human-request': unknown
  'composer-action': ChatComposerActionEvent
  'composer-files': ChatComposerFileSelectionEvent
  'composer-selector-change': ChatComposerSelectorChangeEvent
  'attachment-remove': { attachment: ChatComposerAttachment; index: number }
  'attachment-action': ChatComposerAttachmentActionEvent
  'message-action': MessageActionEvent
  'context-action': unknown
  'context-option-select': unknown
  'node-select': unknown
  'navigation-change': ChatEmbeddedNavigationItem[]
  error: unknown
}

export type FastPlanConversationEmits = {
  'state-change': [value: FastPlanConversationEventMap['state-change']]
  'loading-change': [value: FastPlanConversationEventMap['loading-change']]
  'message-sent': [value: FastPlanConversationEventMap['message-sent']]
  'plan-completed': [value: FastPlanConversationEventMap['plan-completed']]
  'execution-completed': [value: FastPlanConversationEventMap['execution-completed']]
  'human-request': [value: FastPlanConversationEventMap['human-request']]
  'composer-action': [value: FastPlanConversationEventMap['composer-action']]
  'composer-files': [value: FastPlanConversationEventMap['composer-files']]
  'composer-selector-change': [value: FastPlanConversationEventMap['composer-selector-change']]
  'attachment-remove': [attachment: ChatComposerAttachment, index: number]
  'attachment-action': [value: FastPlanConversationEventMap['attachment-action']]
  'message-action': [value: FastPlanConversationEventMap['message-action']]
  'context-action': [value: FastPlanConversationEventMap['context-action']]
  'context-option-select': [value: FastPlanConversationEventMap['context-option-select']]
  'node-select': [value: FastPlanConversationEventMap['node-select']]
  'navigation-change': [value: FastPlanConversationEventMap['navigation-change']]
  error: [value: FastPlanConversationEventMap['error']]
}

/** Managed controls may complete asynchronously; standalone runtime results are preserved. */
type ManagedControl<T extends (...args: any[]) => any> = (...args: Parameters<T>) => ReturnType<T> | Promise<boolean>

export interface FastPlanConversationExpose {
  runtime: FastPlanExecutionController
  state: FastPlanRuntimeState
  sendChat: FastPlanExecutionController['sendChat']
  loadSession: FastPlanExecutionController['loadSession']
  disconnect: FastPlanExecutionController['disconnect']
  confirmPlan: FastPlanExecutionController['confirmPlan']
  cancelPlan: ManagedControl<FastPlanExecutionController['cancelPlan']>
  cancelExecution: ManagedControl<FastPlanExecutionController['cancelExecution']>
  pauseExecution: ManagedControl<FastPlanExecutionController['pauseExecution']>
  resumeExecution: ManagedControl<FastPlanExecutionController['resumeExecution']>
  retryNode: FastPlanExecutionController['retryNode']
  skipNode: ManagedControl<FastPlanExecutionController['skipNode']>
  respondHumanCheckpoint: FastPlanExecutionController['respondHumanCheckpoint']
  updateNodeOutput: ManagedControl<FastPlanExecutionController['updateNodeOutput']>
  updateNextNodeConfig: ManagedControl<FastPlanExecutionController['updateNextNodeConfig']>
  executeTemplate: FastPlanExecutionController['executeTemplate']
  saveAsTemplate: FastPlanExecutionController['saveAsTemplate']
}

export interface FastPlanConversationSlots {
  'top-bar'(props: {
    runtime: FastPlanExecutionController
    state: FastPlanRuntimeState
    selectors: FastPlanComposerSelector[]
    select: (value: ChatComposerSelectorChangeEvent) => void
  }): any
  avatar(props: { message: ChatMessage }): any
  header(props: { message: ChatMessage }): any
  'message-card'(props: { messages: unknown[]; runtime: FastPlanExecutionController; state: FastPlanRuntimeState }): any
  'result-card'(props: { result: unknown; files: unknown[]; images: unknown[]; charts: unknown[]; state: FastPlanRuntimeState }): any
  'historical-planning-card'(props: { plan: unknown; state: FastPlanRuntimeState }): any
  'attachment-input'(props: { runtime: FastPlanExecutionController; state: FastPlanRuntimeState }): any
  composer(props: {
    runtime: FastPlanExecutionController
    state: FastPlanRuntimeState
    modelValue: string
    updateModelValue: (value: string) => void
    submit: (value: ChatComposerSubmitEvent | string) => Promise<void>
  }): any
  'composer-inside-top'(props: ChatComposerRegionScope): any
  'composer-selectors'(props: ChatComposerRegionScope): any
  'composer-attachment'(props: ChatComposerRegionScope & {
    attachment: ChatComposerAttachment
    index: number
    remove: (event?: Event) => void
    retry: (event?: Event) => void
    preview: (event?: Event) => void
  }): any
  'composer-input'(props: ChatComposerRegionScope): any
  'composer-action'(props: ChatComposerRegionScope & {
    action: ChatComposerAction
    trigger: ChatComposerRegionScope['triggerAction']
  }): any
  'composer-action-icon'(props: ChatComposerRegionScope & { action: ChatComposerAction }): any
  'composer-toolbar-start'(props: ChatComposerRegionScope): any
  'composer-toolbar-end'(props: ChatComposerRegionScope): any
  'composer-submit-icon'(props: ChatComposerRegionScope): any
  'composer-abort-icon'(props: ChatComposerRegionScope): any
  'action-bar'(props: {
    message: ChatMessage
    actions: ResolvedMessageAction[]
    trigger: (action: ResolvedMessageAction) => Promise<void>
  }): any
  action(props: {
    action: ResolvedMessageAction
    message: ChatMessage
    trigger: (action: ResolvedMessageAction) => Promise<void>
  }): any
  'action-icon'(props: {
    action?: ResolvedMessageAction
    message: ChatMessage
    iconName: string
    active: boolean
    copied: boolean
  }): any
  'action-menu'(props: {
    action: ResolvedMessageAction
    message: ChatMessage
    placement: 'direct' | 'overflow'
    triggerItem: (item: MessageActionMenuItem) => unknown
    close: () => void
  }): any
  'context-item'(props: { item: Record<string, unknown>; select: (item: Record<string, unknown>, event: MouseEvent | KeyboardEvent) => void }): any
  error(props: { error: unknown }): any
}
