<script setup lang="ts">
import { computed, inject, provide, ref } from 'vue'
import { conversationLoadingKey } from './loadingContext'
import ChatComposer from './ChatComposer.vue'
import ChatContextBar from './ChatContextBar.vue'
import ChatMessagePanel from './ChatMessagePanel.vue'
import { resolveChatRendererMessages } from './messages'
import type {
  ActionResolver, AssistantActionsReady, ChatComposerAction, ChatComposerActionEvent, ChatComposerAttachment,
  ChatComposerAttachmentActionEvent, ChatComposerFileSelectionEvent, ChatComposerSelector,
  ChatComposerSelectorChangeEvent, ChatComposerSubmitEvent, ChatContextActionEvent, ChatContextItem, ChatContextPlacement,
  ChatMessage, ChatRendererAdapters, ChatRendererMessages, ImagePart, LoadingState, MessageActionEvent,
  MessageActionsConfig, PaginationState, ToolRendererRegistry,
} from './types'

const props = withDefaults(defineProps<{
  messages?: ChatMessage[]
  modelValue?: string
  contextItems?: ChatContextItem[]
  contextPlacement?: ChatContextPlacement
  showContext?: boolean
  showComposer?: boolean
  showComposerAuxiliary?: boolean
  scrollMode?: 'internal' | 'host'
  appearance?: 'default' | 'seamless'
  initializing?: boolean
  contentLoading?: boolean
  assistantLoading?: boolean
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
  adapters?: ChatRendererAdapters
  toolRenderers?: ToolRendererRegistry
  labels?: Partial<ChatRendererMessages>
  locale?: string
  theme?: 'light' | 'dark' | 'auto'
  allowHtml?: boolean
  maxRenderChars?: number
  markdownAutoFoldThreshold?: number
  markdownShowCodeRowNumber?: boolean
  autoScroll?: boolean
}>(), {
  messages: () => [], modelValue: '', contextItems: () => [], contextPlacement: 'top', showContext: true,
  showComposer: true, showComposerAuxiliary: false, scrollMode: 'internal', appearance: 'default', initializing: false, assistantLoading: false, composerLoading: false,
  composerDisabled: false, composerReadonly: false, composerPlaceholder: undefined,
  composerActions: () => [], composerSelectors: () => [], composerAttachments: () => [], composerMaxHeight: 200, composerAutoResize: true, sendByCtrlEnter: false,
  loadingState: () => ({}), paginationState: () => ({}), showActions: true, assistantActionsReady: true, actions: () => ({}),
  toolRenderers: () => ({}), labels: () => ({}), theme: 'auto', allowHtml: false,
  maxRenderChars: 15000, markdownAutoFoldThreshold: 10000, markdownShowCodeRowNumber: true, autoScroll: true,
})

const ancestorLoading = inject(conversationLoadingKey, computed(() => false))
const ownLoading = computed(() => !!(props.contentLoading || props.initializing))
const showLoading = computed(() => ownLoading.value && !ancestorLoading.value)
provide(conversationLoadingKey, computed(() => ancestorLoading.value || ownLoading.value))

const effectiveTheme = computed<'light' | 'dark'>(() => {
  if (props.theme !== 'auto') return props.theme
  if (typeof window !== 'undefined' && window.matchMedia?.('(prefers-color-scheme: dark)').matches) return 'dark'
  return 'light'
})
const messagePanel = ref<InstanceType<typeof ChatMessagePanel> | null>(null)
const text = computed(() => resolveChatRendererMessages(props.locale, props.labels))

const emit = defineEmits<{
  (event: 'update:modelValue', value: string): void
  (event: 'submit', value: ChatComposerSubmitEvent): void
  (event: 'abort', nativeEvent: MouseEvent): void
  (event: 'composer-action', value: ChatComposerActionEvent): void
  (event: 'composer-files', value: ChatComposerFileSelectionEvent): void
  (event: 'composer-selector-change', value: ChatComposerSelectorChangeEvent): void
  (event: 'context-action', value: ChatContextActionEvent): void
  (event: 'attachment-remove', attachment: ChatComposerAttachment, index: number): void
  (event: 'attachment-action', value: ChatComposerAttachmentActionEvent): void
  (event: 'message-action', value: MessageActionEvent): void
  (event: 'load-older'): void
  (event: 'load-all'): void
  (event: 'recover'): void
  (event: 'link-click', href: string, nativeEvent: MouseEvent): void
  (event: 'image-preview', images: ImagePart[], activeIndex: number): void
  (event: 'show-full-content', message: ChatMessage): void
  (event: 'container-ready', element: HTMLElement): void
}>()

function attachmentRemove(attachment: ChatComposerAttachment, index: number) {
  emit('attachment-remove', attachment, index)
}

defineExpose({
  scrollToBottom: (behavior?: ScrollBehavior) => messagePanel.value?.scrollToBottom(behavior),
  messagePanel,
})
</script>

<template>
  <section class="jcr-conversation" :data-context-placement="contextPlacement" :data-theme="effectiveTheme" :data-scroll-mode="scrollMode" :data-appearance="appearance === 'default' ? undefined : appearance">
    <ChatContextBar v-if="showContext && contextPlacement === 'top'" :items="contextItems" placement="top" :theme="effectiveTheme"
      @context-action="$emit('context-action', $event)">
      <template v-if="$slots['context-start']" #start><slot name="context-start" /></template>
      <template v-if="$slots['context-item']" #item="scope"><slot name="context-item" v-bind="scope" /></template>
      <template v-if="$slots['context-end']" #end><slot name="context-end" /></template>
    </ChatContextBar>

    <div class="jcr-conversation__content" :aria-busy="contentLoading || initializing">
    <div v-if="showLoading" class="jcr-conversation__loading" role="status" :aria-label="text.loading">
      <span class="jcr-spinner" aria-hidden="true" />
    </div>
    <main class="jcr-conversation__main">
      <slot name="before-messages" />
      <slot v-if="initializing" name="initialization-placeholder" />
      <slot v-else name="messages">
        <ChatMessagePanel ref="messagePanel" :messages="messages" :loading-state="loadingState" :pagination-state="paginationState"
          :appearance="appearance"
          :show-actions="showActions" :assistant-actions-ready="assistantActionsReady" :actions="actions" :resolve-actions="resolveActions" :adapters="adapters"
          :tool-renderers="toolRenderers" :labels="text" :locale="locale" :theme="theme" :allow-html="allowHtml"
          :max-render-chars="maxRenderChars" :auto-scroll="autoScroll" @message-action="$emit('message-action', $event)"
          :markdown-auto-fold-threshold="markdownAutoFoldThreshold"
          :markdown-show-code-row-number="markdownShowCodeRowNumber"
          @load-older="$emit('load-older')" @load-all="$emit('load-all')" @recover="$emit('recover')"
          @link-click="(href, event) => $emit('link-click', href, event)"
          @image-preview="(images, index) => $emit('image-preview', images, index)"
          @show-full-content="$emit('show-full-content', $event)" @container-ready="$emit('container-ready', $event)">
          <template v-if="$slots.avatar" #avatar="scope"><slot name="avatar" v-bind="scope" /></template>
          <template v-if="$slots.header" #header="scope"><slot name="header" v-bind="scope" /></template>
          <template v-if="$slots['message-content']" #content="scope"><slot name="message-content" v-bind="scope" /></template>
          <template v-if="$slots['message-text']" #text="scope"><slot name="message-text" v-bind="scope" /></template>
          <template v-if="$slots['message-reasoning']" #reasoning="scope"><slot name="message-reasoning" v-bind="scope" /></template>
          <template v-if="$slots.attachment" #attachment="scope"><slot name="attachment" v-bind="scope" /></template>
          <template v-if="$slots.tool" #tool="scope"><slot name="tool" v-bind="scope" /></template>
          <template v-if="$slots.custom" #custom="scope"><slot name="custom" v-bind="scope" /></template>
          <template v-if="$slots['message-before-content']" #before-content="scope"><slot name="message-before-content" v-bind="scope" /></template>
          <template v-if="$slots['message-after-content']" #after-content="scope"><slot name="message-after-content" v-bind="scope" /></template>
          <template v-if="$slots['message-after']" #message-after="scope"><slot name="message-after" v-bind="scope" /></template>
          <template v-if="$slots['action-bar']" #action-bar="scope"><slot name="action-bar" v-bind="scope" /></template>
          <template v-if="$slots.action" #action="scope"><slot name="action" v-bind="scope" /></template>
          <template v-if="$slots['action-icon']" #action-icon="scope"><slot name="action-icon" v-bind="scope" /></template>
          <template v-if="$slots['action-menu']" #action-menu="scope"><slot name="action-menu" v-bind="scope" /></template>
          <template v-if="$slots['message-loading']" #loading="scope"><slot name="message-loading" v-bind="scope" /></template>
          <template v-if="$slots['message-empty']" #empty><slot name="message-empty" /></template>
          <template v-if="$slots['message-generating']" #generating="scope"><slot name="message-generating" v-bind="scope" /></template>
          <template v-if="$slots['message-stalled']" #stalled="scope"><slot name="message-stalled" v-bind="scope" /></template>
        </ChatMessagePanel>
      </slot>
      <slot v-if="assistantLoading" name="assistant-loading" />
      <slot name="after-messages" />
    </main>
    <slot name="content-side" />
    </div>

    <div v-if="showComposerAuxiliary" class="jcr-conversation__auxiliary">
      <slot name="planning-panel" />
      <slot name="composer-before" />
    </div>

    <footer v-if="showComposer" class="jcr-conversation__footer">
      <slot name="planning-panel" />
      <ChatContextBar v-if="showContext && contextPlacement === 'composer'" :items="contextItems" placement="composer" :theme="effectiveTheme"
        @context-action="$emit('context-action', $event)">
        <template v-if="$slots['context-start']" #start><slot name="context-start" /></template>
        <template v-if="$slots['context-item']" #item="scope"><slot name="context-item" v-bind="scope" /></template>
        <template v-if="$slots['context-end']" #end><slot name="context-end" /></template>
      </ChatContextBar>
      <slot v-if="!showComposerAuxiliary" name="composer-before" />
      <slot name="composer">
        <ChatComposer :model-value="modelValue" :placeholder="composerPlaceholder" :loading="composerLoading" :theme="effectiveTheme"
          :disabled="composerDisabled" :readonly="composerReadonly" :actions="composerActions"
          :selectors="composerSelectors" :attachments="composerAttachments" :max-height="composerMaxHeight"
          :auto-resize="composerAutoResize" :send-by-ctrl-enter="sendByCtrlEnter" :labels="text" :locale="locale"
          @update:model-value="$emit('update:modelValue', $event)" @submit="$emit('submit', $event)"
          @abort="$emit('abort', $event)" @composer-action="$emit('composer-action', $event)"
          @files-selected="$emit('composer-files', $event)" @selector-change="$emit('composer-selector-change', $event)"
          @attachment-action="$emit('attachment-action', $event)"
          @attachment-remove="attachmentRemove">
          <template v-if="$slots['composer-inside-top']" #inside-top="scope">
            <slot name="composer-inside-top" v-bind="scope" />
          </template>
          <template v-if="$slots['composer-attachment']" #attachment="scope">
            <slot name="composer-attachment" v-bind="scope" />
          </template>
          <template v-if="$slots['composer-selectors']" #selectors="scope">
            <slot name="composer-selectors" v-bind="scope" />
          </template>
          <template v-if="$slots['composer-input']" #input="scope">
            <slot name="composer-input" v-bind="scope" />
          </template>
          <template v-if="$slots['composer-action']" #action="scope">
            <slot name="composer-action" v-bind="scope" />
          </template>
          <template v-if="$slots['composer-action-icon']" #action-icon="scope">
            <slot name="composer-action-icon" v-bind="scope" />
          </template>
          <template v-if="$slots['composer-toolbar-start']" #toolbar-start="scope">
            <slot name="composer-toolbar-start" v-bind="scope" />
          </template>
          <template v-if="$slots['composer-toolbar-end']" #toolbar-end="scope">
            <slot name="composer-toolbar-end" v-bind="scope" />
          </template>
          <template v-if="$slots['composer-submit-icon']" #submit-icon="scope">
            <slot name="composer-submit-icon" v-bind="scope" />
          </template>
          <template v-if="$slots['composer-abort-icon']" #abort-icon="scope">
            <slot name="composer-abort-icon" v-bind="scope" />
          </template>
        </ChatComposer>
      </slot>
      <slot name="composer-after" />
    </footer>
    <slot name="overlays" />
  </section>
</template>
