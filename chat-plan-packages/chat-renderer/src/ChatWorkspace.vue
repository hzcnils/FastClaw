<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import ChatAuxiliaryPanel from './ChatAuxiliaryPanel.vue'
import ChatConversation from './ChatConversation.vue'
import ChatQuestionNavigator from './ChatQuestionNavigator.vue'
import ChatTimeline from './ChatTimeline.vue'
import type {
  ActionResolver, AssistantActionsReady, ChatAuxiliaryItem, ChatAuxiliaryPlacement, ChatComposerAction,
  ChatComposerActionEvent, ChatComposerAttachment, ChatComposerAttachmentActionEvent,
  ChatComposerFileSelectionEvent, ChatComposerSelector, ChatComposerSelectorChangeEvent,
  ChatComposerSubmitEvent, ChatContextActionEvent, ChatContextItem, ChatContextPlacement,
  ChatEmbeddedNavigationChange, ChatEmbeddedNavigationItem, ChatMessage, ChatQuestionNavigationItem,
  ChatRendererAdapters, ChatRendererMessages, ChatStreamError, ChatStreamEvent,
  ChatThemeTokens, ChatTimelineEntry, ChatVariantChangeEvent, ChatComparisonModeChangeEvent,
  ChatWorkspaceActionEvent, ChatWorkspaceActionResultEvent, ChatWorkspaceAdapter, ChatWorkspacePlugins, ImagePart, LoadingState,
  MessageActionEvent, MessageActionsConfig, NormalizedChatInput, PaginationState, ToolRendererRegistry,
} from './types'
import {
  applyChatStreamEvent, createChatThemeStyle, createPendingChatTurn, messagesToTimelineEntries,
  questionItemsFromTimeline, updateTimelineComparisonMode, updateTimelineVariant,
} from './workspace'
import { resolveChatRendererMessages } from './messages'

const props = withDefaults(defineProps<{
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
  contentLoading?: boolean
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
  auxiliaryScope?: 'workspace' | 'content'
  metadata?: Record<string, unknown>
}>(), {
  defaultTimeline: () => [], messages: () => [], plugins: () => ({}), modelValue: '', contextItems: () => [],
  contextPlacement: 'top', showContext: true, showComposer: true, scrollMode: 'internal', appearance: 'default',
  initializing: false, composerLoading: false, composerDisabled: false, composerReadonly: false,
  composerPlaceholder: undefined, composerActions: () => [], composerSelectors: () => [], composerAttachments: () => [], composerMaxHeight: 200, composerAutoResize: true, sendByCtrlEnter: false,
  loadingState: () => ({}), paginationState: () => ({}), showActions: true, assistantActionsReady: true,
  actions: () => ({}), toolRenderers: () => ({}), labels: () => ({}), theme: 'auto', themeTokens: () => ({}),
  allowHtml: false, maxRenderChars: 15000, markdownAutoFoldThreshold: 10000, markdownShowCodeRowNumber: true, autoScroll: true, showScrollToBottom: false, showQuestionNavigator: false,
  questionNavigatorCollapsed: false, auxiliaryItems: () => [], auxiliaryPlacement: 'side', auxiliaryScope: 'workspace', metadata: () => ({}),
})

const emit = defineEmits<{
  (event: 'update:timeline', value: ChatTimelineEntry[]): void
  (event: 'update:modelValue', value: string): void
  (event: 'update:activeAuxiliaryId', value: string): void
  (event: 'workspace-action', value: ChatWorkspaceActionEvent): void
  (event: 'action-result', value: ChatWorkspaceActionResultEvent): void
  (event: 'stream-event', value: ChatStreamEvent): void
  (event: 'submit', value: ChatComposerSubmitEvent): void
  (event: 'abort', nativeEvent: MouseEvent): void
  (event: 'composer-files', value: ChatComposerFileSelectionEvent): void
  (event: 'composer-selector-change', value: ChatComposerSelectorChangeEvent): void
  (event: 'message-action', value: MessageActionEvent): void
  (event: 'variant-change', value: ChatVariantChangeEvent): void
  (event: 'comparison-mode-change', value: ChatComparisonModeChangeEvent): void
  (event: 'active-entry-change', entryId: string): void
  (event: 'error', value: ChatStreamError): void
  (event: 'load-older'): void
  (event: 'load-all'): void
  (event: 'recover'): void
  (event: 'link-click', href: string, nativeEvent: MouseEvent): void
  (event: 'image-preview', images: ImagePart[], activeIndex: number): void
  (event: 'show-full-content', message: ChatMessage): void
  (event: 'container-ready', element: HTMLElement): void
}>()

const messagesText = computed(() => resolveChatRendererMessages(props.locale, props.labels))

let idCounter = 0
function createId(prefix: string) {
  const randomId = typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function' ? crypto.randomUUID() : `${Date.now()}-${++idCounter}`
  return `${prefix}-${randomId}`
}

const initialTimeline = () => (props.timeline !== undefined || props.defaultTimeline.length > 0)
  ? [...(props.timeline || props.defaultTimeline)]
  : messagesToTimelineEntries(props.messages)
const draftTimeline = ref<ChatTimelineEntry[]>(initialTimeline())
watch(() => props.timeline, value => { if (value) draftTimeline.value = [...value] }, { deep: true })
watch(() => props.messages, value => {
  if (props.timeline === undefined && !props.defaultTimeline.length) draftTimeline.value = messagesToTimelineEntries(value)
}, { deep: true })

const timeline = computed(() => draftTimeline.value)
const themeStyle = computed(() => createChatThemeStyle(props.themeTokens))
const embeddedNavigation = ref<Record<string, ChatEmbeddedNavigationItem[]>>({})
const questionItems = computed<ChatQuestionNavigationItem[]>(() => {
  const result: ChatQuestionNavigationItem[] = []
  for (const entry of timeline.value) {
    const contributed = entry.kind === 'embedded' ? embeddedNavigation.value[entry.id] : undefined
    if (contributed?.length) {
      for (const item of contributed) {
        const label = item.label.trim()
        if (!label) continue
        result.push({
          ...item,
          id: `question:${entry.id}:${item.id}`,
          entryId: entry.id,
          label: label.length > 80 ? `${label.slice(0, 79)}…` : label,
          index: result.length,
        })
      }
      continue
    }
    for (const item of questionItemsFromTimeline([entry])) result.push({ ...item, index: result.length })
  }
  return result
})
const activeEntryId = ref('')
const activeQuestionId = ref('')
const closedAuxiliaryIds = ref<string[]>([])
const displayedAuxiliaryItems = computed(() => props.auxiliaryItems.filter(item => item.visible !== false && !closedAuxiliaryIds.value.includes(item.id)))
const visibleAuxiliary = computed(() => displayedAuxiliaryItems.value.length > 0)
const effectiveActiveAuxiliaryId = computed(() => props.activeAuxiliaryId || displayedAuxiliaryItems.value[0]?.id)
watch(() => props.activeAuxiliaryId, activeId => {
  if (activeId) closedAuxiliaryIds.value = closedAuxiliaryIds.value.filter(id => id !== activeId)
})
watch(timeline, entries => {
  const validIds = new Set(entries.filter(entry => entry.kind === 'embedded').map(entry => entry.id))
  const next = Object.fromEntries(Object.entries(embeddedNavigation.value).filter(([entryId]) => validIds.has(entryId)))
  if (Object.keys(next).length !== Object.keys(embeddedNavigation.value).length) embeddedNavigation.value = next
}, { deep: true })
const timelineComponent = ref<InstanceType<typeof ChatTimeline> | null>(null)
const adapterLoading = ref(false)
let activeController: AbortController | undefined
let activeContext: { entryId: string; turnId: string; messageId: string } | undefined

function commit(next: ChatTimelineEntry[]) {
  draftTimeline.value = next
  emit('update:timeline', next)
}

function workspaceAction(action: ChatWorkspaceActionEvent['action'], scope: ChatWorkspaceActionEvent['scope'], details: Partial<ChatWorkspaceActionEvent> = {}) {
  const event: ChatWorkspaceActionEvent = {
    protocolVersion: 1, action, scope, conversationId: props.conversationId, metadata: props.metadata, ...details,
  }
  emit('workspace-action', event)
  if (props.adapter?.executeAction && action !== 'submit' && action !== 'abort') {
    Promise.resolve(props.adapter.executeAction(event))
      .then(result => emit('action-result', {
        event,
        result: result || { status: 'success' },
      }))
      .catch(error => emit('action-result', {
        event,
        result: { status: 'error', error: reportError(error) },
      }))
  }
  return event
}

function reportError(value: unknown): ChatStreamError {
  const error: ChatStreamError = value && typeof value === 'object' && 'message' in value
    ? { message: String((value as { message: unknown }).message), details: value }
    : { message: String(value || 'Unknown chat adapter error'), details: value }
  emit('error', error)
  return error
}

async function submit(value: ChatComposerSubmitEvent) {
  emit('submit', value)
  const input: NormalizedChatInput = {
    id: createId('input'), text: value.text.trim(), attachments: value.attachments,
    modelId: props.selectedModelId, contextItems: props.contextItems, metadata: props.metadata,
  }
  workspaceAction('submit', 'composer', { payload: input, nativeEvent: value.nativeEvent })
  if (!props.adapter || adapterLoading.value) return

  const turnId = createId('turn')
  const userMessageId = createId('temporary-user')
  const assistantMessageId = createId('temporary-assistant')
  const submissionContext = {
    conversationId: props.conversationId, entryId: turnId, turnId, messageId: assistantMessageId,
    metadata: props.metadata,
  }
  if (props.adapter.createTimelineEntry) {
    adapterLoading.value = true
    try {
      const entry = await props.adapter.createTimelineEntry(input, submissionContext)
      if (entry) {
        const submission = 'entry' in entry ? entry : { entry }
        const next = submission.replaceEntryId
          ? timeline.value.map(item => item.id === submission.replaceEntryId ? submission.entry : item)
          : [...timeline.value, submission.entry]
        commit(next)
        emit('update:modelValue', '')
        return
      }
    } catch (cause) {
      reportError(cause)
      return
    } finally {
      adapterLoading.value = false
    }
  }
  commit([...timeline.value, createPendingChatTurn(input, { turnId, userMessageId, assistantMessageId })])
  emit('update:modelValue', '')
  activeController = new AbortController()
  activeContext = { entryId: turnId, turnId, messageId: assistantMessageId }
  adapterLoading.value = true
  let terminal = false
  try {
    const stream = props.adapter.send(input, {
      ...submissionContext, signal: activeController.signal,
    })
    for await (const event of stream) {
      emit('stream-event', event)
      commit(applyChatStreamEvent(timeline.value, event, activeContext))
      if (event.type === 'message-start') activeContext.messageId = event.message.id
      if (event.type === 'message-complete' || event.type === 'error') terminal = true
    }
    if (!terminal && activeContext) {
      const complete: ChatStreamEvent = { type: 'message-complete', messageId: activeContext.messageId }
      emit('stream-event', complete)
      commit(applyChatStreamEvent(timeline.value, complete, activeContext))
    }
  } catch (cause) {
    if (!activeController.signal.aborted && activeContext) {
      const errorEvent: ChatStreamEvent = { type: 'error', error: reportError(cause) }
      emit('stream-event', errorEvent)
      commit(applyChatStreamEvent(timeline.value, errorEvent, activeContext))
    }
  } finally {
    adapterLoading.value = false
    activeController = undefined
    activeContext = undefined
  }
}

async function abort(nativeEvent: MouseEvent) {
  emit('abort', nativeEvent)
  workspaceAction('abort', 'composer', { nativeEvent, entryId: activeContext?.entryId, turnId: activeContext?.turnId, messageId: activeContext?.messageId })
  activeController?.abort()
  if (props.adapter?.abort && activeContext) {
    try { await props.adapter.abort({ conversationId: props.conversationId, ...activeContext, metadata: props.metadata }) }
    catch (error) { reportError(error) }
  }
}

function messageAction(value: MessageActionEvent) {
  emit('message-action', value)
  workspaceAction(value.action as ChatWorkspaceActionEvent['action'], 'message', {
    messageId: value.messageId, turnId: value.turnId, actionId: value.action, payload: value,
  })
}

function variantChange(value: ChatVariantChangeEvent) {
  commit(updateTimelineVariant(timeline.value, value.turnId, value.activeId))
  emit('variant-change', value)
  workspaceAction('variant-change', 'timeline', { entryId: value.turnId, turnId: value.turnId, payload: value })
}

function comparisonModeChange(value: ChatComparisonModeChangeEvent) {
  commit(updateTimelineComparisonMode(timeline.value, value.turnId, value.mode))
  emit('comparison-mode-change', value)
  workspaceAction('comparison-mode-change', 'timeline', { entryId: value.turnId, turnId: value.turnId, payload: value })
}

function composerAction(value: ChatComposerActionEvent) {
  workspaceAction('composer-action', 'composer', { actionId: value.action, payload: value, nativeEvent: value.nativeEvent })
}

function composerFiles(value: ChatComposerFileSelectionEvent) {
  emit('composer-files', value)
  workspaceAction('composer-action', 'composer', { actionId: value.action, payload: value, nativeEvent: value.nativeEvent })
}

function composerSelectorChange(value: ChatComposerSelectorChangeEvent) {
  emit('composer-selector-change', value)
  workspaceAction('context-change', 'composer', { actionId: value.selector.id, payload: value, nativeEvent: value.nativeEvent })
}

function contextAction(value: ChatContextActionEvent) {
  workspaceAction('context-change', 'context', { actionId: value.action, payload: value, nativeEvent: value.nativeEvent })
}

function attachmentAction(value: ChatComposerAttachmentActionEvent) {
  workspaceAction(value.action === 'retry' ? 'attachment-retry' : 'attachment-remove', 'composer', {
    actionId: value.action, payload: value, nativeEvent: value.nativeEvent,
  })
}

function setActiveEntry(entryId: string) {
  activeEntryId.value = entryId
  emit('active-entry-change', entryId)
}

function setActiveNavigation(itemId: string) {
  activeQuestionId.value = itemId
}

function selectAuxiliary(activeId: string) {
  emit('update:activeAuxiliaryId', activeId)
  workspaceAction('auxiliary-change', 'auxiliary', { actionId: activeId, payload: { activeId } })
}

function closeAuxiliary(item: ChatAuxiliaryItem) {
  closedAuxiliaryIds.value = [...closedAuxiliaryIds.value, item.id]
  const next = displayedAuxiliaryItems.value[0]?.id || ''
  emit('update:activeAuxiliaryId', next)
  workspaceAction('auxiliary-change', 'auxiliary', { actionId: item.id, payload: { action: 'close', item, activeId: next } })
}

function embeddedAction(payload: unknown, entry: Extract<ChatTimelineEntry, { kind: 'embedded' }>) {
  const change = payload as Partial<ChatEmbeddedNavigationChange> | null
  if (change?.type === 'navigation-change' && Array.isArray(change.items)) {
    embeddedNavigation.value = {
      ...embeddedNavigation.value,
      [entry.id]: change.items.flatMap(item => {
        if (!item || typeof item !== 'object') return []
        const id = String(item.id || '').trim()
        const label = String(item.label || '').trim()
        if (!id || !label) return []
        return [{ ...item, id, label,
          targetMessageId: item.targetMessageId == null ? undefined : String(item.targetMessageId) }]
      }),
    }
  }
  workspaceAction('custom', 'timeline', { entryId: entry.id, payload })
}

function activateQuestion(questionId: string) {
  if (!questionItems.value.some(item => item.id === questionId)) return false
  activeQuestionId.value = questionId
  return true
}

function activateLatestQuestion() {
  const item = questionItems.value[questionItems.value.length - 1]
  return item ? activateQuestion(item.id) : false
}

function scrollToQuestion(questionId: string, behavior?: ScrollBehavior) {
  const item = questionItems.value.find(candidate => candidate.id === questionId)
  return item ? timelineComponent.value?.scrollToNavigationItem(item, behavior) : false
}

function scrollToLatestQuestion(behavior?: ScrollBehavior) {
  const item = questionItems.value[questionItems.value.length - 1]
  return item ? timelineComponent.value?.scrollToNavigationItem(item, behavior) : false
}

function linkClick(href: string, event: MouseEvent) {
  emit('link-click', href, event)
}

function imagePreview(images: ImagePart[], index: number) {
  emit('image-preview', images, index)
}

defineExpose({
  timeline,
  send: submit,
  abort,
  scrollToBottom: (behavior?: ScrollBehavior) => timelineComponent.value?.scrollToBottom(behavior),
  scrollToEntry: (entryId: string, behavior?: ScrollBehavior) => timelineComponent.value?.scrollToEntry(entryId, behavior),
  activateQuestion,
  activateLatestQuestion,
  scrollToQuestion,
  scrollToLatestQuestion,
})
</script>

<template>
  <section class="jcr-workspace" :data-theme="theme" :data-auxiliary-placement="auxiliaryPlacement" :data-auxiliary-scope="auxiliaryScope" :style="themeStyle">
    <slot name="header" />
    <div class="jcr-workspace__body">
      <ChatConversation :model-value="modelValue" :context-items="contextItems" :context-placement="contextPlacement" :style="themeStyle"
        :show-context="showContext" :show-composer="showComposer" :scroll-mode="scrollMode" :appearance="appearance"
        :initializing="initializing" :content-loading="contentLoading" :composer-loading="composerLoading || adapterLoading" :composer-disabled="composerDisabled"
        :composer-readonly="composerReadonly" :composer-placeholder="composerPlaceholder" :composer-actions="composerActions"
        :composer-selectors="composerSelectors" :composer-attachments="composerAttachments" :composer-max-height="composerMaxHeight"
        :composer-auto-resize="composerAutoResize" :send-by-ctrl-enter="sendByCtrlEnter" :labels="messagesText" :locale="locale" :theme="theme"
        @update:model-value="emit('update:modelValue', $event)" @submit="submit" @abort="abort"
        @composer-action="composerAction" @composer-files="composerFiles" @composer-selector-change="composerSelectorChange"
        @context-action="contextAction" @attachment-action="attachmentAction">
        <template #messages>
          <ChatTimeline ref="timelineComponent" :entries="timeline" :plugins="plugins" :loading-state="loadingState" :style="themeStyle"
            :navigation-items="questionItems"
            :pagination-state="paginationState" :show-actions="showActions" :assistant-actions-ready="assistantActionsReady"
            :actions="actions" :resolve-actions="resolveActions" :adapters="rendererAdapters" :tool-renderers="toolRenderers"
            :labels="messagesText" :locale="locale" :theme="theme" :allow-html="allowHtml" :max-render-chars="maxRenderChars"
            :markdown-auto-fold-threshold="markdownAutoFoldThreshold"
            :markdown-show-code-row-number="markdownShowCodeRowNumber"
            :auto-scroll="autoScroll" :show-scroll-to-bottom="showScrollToBottom" :scroll-mode="scrollMode" :appearance="appearance"
            @message-action="messageAction" @variant-change="variantChange" @comparison-mode-change="comparisonModeChange"
            @embedded-action="embeddedAction"
            @active-entry-change="setActiveEntry" @active-navigation-change="setActiveNavigation"
            @load-older="emit('load-older')" @load-all="emit('load-all')"
            @recover="emit('recover')" @link-click="linkClick" @image-preview="imagePreview"
            @show-full-content="emit('show-full-content', $event)" @container-ready="emit('container-ready', $event)">
            <template v-if="$slots['message-avatar']" #avatar="scope"><slot name="message-avatar" v-bind="scope" /></template>
            <template v-if="$slots['message-header']" #header="scope"><slot name="message-header" v-bind="scope" /></template>
            <template v-if="$slots['message-after-content']" #after-content="scope"><slot name="message-after-content" v-bind="scope" /></template>
            <template v-if="$slots['message-action-icon']" #action-icon="scope"><slot name="message-action-icon" v-bind="scope" /></template>
            <template v-if="$slots.entry" #entry="scope"><slot name="entry" v-bind="scope" /></template>
            <template v-if="$slots.embedded" #embedded="scope"><slot name="embedded" v-bind="scope" /></template>
            <template v-if="$slots['missing-renderer']" #missing-renderer="scope"><slot name="missing-renderer" v-bind="scope" /></template>
            <template v-if="$slots['message-custom']" #custom="scope"><slot name="message-custom" v-bind="scope" /></template>
            <template v-if="$slots.empty" #empty><slot name="empty" /></template>
            <template v-if="$slots['scroll-to-bottom-icon']" #scroll-to-bottom-icon><slot name="scroll-to-bottom-icon" /></template>
          </ChatTimeline>
        </template>
        <template v-if="$slots['context-start']" #context-start><slot name="context-start" /></template>
        <template v-if="$slots['context-item']" #context-item="scope"><slot name="context-item" v-bind="scope" /></template>
        <template v-if="$slots['context-end']" #context-end><slot name="context-end" /></template>
        <template v-if="$slots['composer-before']" #composer-before><slot name="composer-before" /></template>
        <template v-if="$slots['composer-inside-top']" #composer-inside-top="scope"><slot name="composer-inside-top" v-bind="scope" /></template>
        <template v-if="$slots['composer-selectors']" #composer-selectors="scope"><slot name="composer-selectors" v-bind="scope" /></template>
        <template v-if="$slots['composer-attachment']" #composer-attachment="scope"><slot name="composer-attachment" v-bind="scope" /></template>
        <template v-if="$slots['composer-input']" #composer-input="scope"><slot name="composer-input" v-bind="scope" /></template>
        <template v-if="$slots['composer-action']" #composer-action="scope"><slot name="composer-action" v-bind="scope" /></template>
        <template v-if="$slots['composer-action-icon']" #composer-action-icon="scope"><slot name="composer-action-icon" v-bind="scope" /></template>
        <template v-if="$slots['composer-toolbar-start']" #composer-toolbar-start="scope"><slot name="composer-toolbar-start" v-bind="scope" /></template>
        <template v-if="$slots['composer-toolbar-end']" #composer-toolbar-end="scope"><slot name="composer-toolbar-end" v-bind="scope" /></template>
        <template v-if="$slots['composer-submit-icon']" #composer-submit-icon="scope"><slot name="composer-submit-icon" v-bind="scope" /></template>
        <template v-if="$slots['composer-abort-icon']" #composer-abort-icon="scope"><slot name="composer-abort-icon" v-bind="scope" /></template>
        <template v-if="$slots.composer" #composer><slot name="composer" /></template>
        <template v-if="$slots['composer-after']" #composer-after><slot name="composer-after" /></template>
        <template #content-side>
          <ChatAuxiliaryPanel v-if="visibleAuxiliary && auxiliaryScope === 'content'" :items="displayedAuxiliaryItems" :plugins="plugins" :style="themeStyle"
            :active-id="effectiveActiveAuxiliaryId" :placement="auxiliaryPlacement" :labels="messagesText"
            @update:active-id="selectAuxiliary" @close="closeAuxiliary"
            @action="(payload, item) => workspaceAction('custom', 'auxiliary', { actionId: item.id, payload })">
            <template v-if="$slots['auxiliary-panel']" #panel="scope"><slot name="auxiliary-panel" v-bind="scope" /></template>
          </ChatAuxiliaryPanel>
        </template>
      </ChatConversation>

      <ChatAuxiliaryPanel v-if="visibleAuxiliary && auxiliaryScope === 'workspace'" :items="displayedAuxiliaryItems" :plugins="plugins" :style="themeStyle"
        :active-id="effectiveActiveAuxiliaryId" :placement="auxiliaryPlacement" :labels="messagesText"
        @update:active-id="selectAuxiliary" @close="closeAuxiliary"
        @action="(payload, item) => workspaceAction('custom', 'auxiliary', { actionId: item.id, payload })">
        <template v-if="$slots['auxiliary-panel']" #panel="scope"><slot name="auxiliary-panel" v-bind="scope" /></template>
      </ChatAuxiliaryPanel>
    </div>

    <ChatQuestionNavigator v-if="showQuestionNavigator" :items="questionItems" :active-id="activeQuestionId" :style="themeStyle" :labels="messagesText" :locale="locale"
      :collapsed="questionNavigatorCollapsed" @navigate="event => { timelineComponent?.scrollToNavigationItem(event.item); workspaceAction('navigate', 'timeline', { entryId: event.item.entryId, messageId: event.item.targetMessageId, payload: event, nativeEvent: event.nativeEvent }) }" />
    <slot name="overlays" />
  </section>
</template>
