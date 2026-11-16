<script setup lang="ts">
import { computed, ref } from 'vue'
import ChatMessageList from './components/ChatMessageList.vue'
import { usePinnedScroll } from './composables/usePinnedScroll'
import { resolveChatRendererMessages } from './messages'
import type {
  ActionResolver, AssistantActionsReady, ChatMessage, ChatRendererAdapters, ChatRendererMessages, ImagePart,
  LoadingState, MessageActionEvent, MessageActionsConfig, PaginationState, ToolRendererRegistry,
} from './types'

const props = withDefaults(defineProps<{
  messages: ChatMessage[]
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
  appearance?: 'default' | 'seamless'
  lastCompletedAssistantId?: string
}>(), {
  loadingState: () => ({}), paginationState: () => ({}), showActions: true, assistantActionsReady: true,
  actions: () => ({}), toolRenderers: () => ({}), labels: () => ({}),
  locale: undefined, theme: 'auto', allowHtml: false, maxRenderChars: 15000, markdownAutoFoldThreshold: 10000, markdownShowCodeRowNumber: true, autoScroll: true, appearance: 'default',
  lastCompletedAssistantId: undefined,
})

const emit = defineEmits<{
  (event: 'message-action', value: MessageActionEvent): void
  (event: 'load-older'): void
  (event: 'load-all'): void
  (event: 'recover'): void
  (event: 'link-click', href: string, nativeEvent: MouseEvent): void
  (event: 'image-preview', images: ImagePart[], activeIndex: number): void
  (event: 'show-full-content', message: ChatMessage): void
  (event: 'container-ready', element: HTMLElement): void
}>()

const root = ref<HTMLElement | null>(null)
const messagesText = computed(() => resolveChatRendererMessages(props.locale, props.labels))
const effectiveTheme = computed<'light' | 'dark'>(() => {
  if (props.theme !== 'auto') return props.theme
  if (typeof window !== 'undefined' && window.matchMedia?.('(prefers-color-scheme: dark)').matches) return 'dark'
  return 'light'
})

const { hasNewMessages, handleScroll: onScroll, scrollToBottom } = usePinnedScroll({
  root,
  firstKey: () => props.messages[0]?.id,
  lastKey: () => props.messages[props.messages.length - 1]?.id,
  contentKey: () => props.messages[props.messages.length - 1]?.parts,
  autoScroll: () => props.autoScroll,
  canLoadOlder: () => !!props.paginationState.hasMore && !props.paginationState.loading,
  onLoadOlder: () => emit('load-older'),
  onReady: element => emit('container-ready', element),
})

defineExpose({ scrollToBottom, element: root })
</script>

<template>
  <section ref="root" class="jcr-panel" :data-theme="effectiveTheme" :data-appearance="appearance === 'default' ? undefined : appearance" :lang="locale" @scroll.passive="onScroll">
    <slot v-if="loadingState.initial && !messages.length" name="loading" :state="loadingState">
      <div class="jcr-state" role="status"><span class="jcr-spinner" />{{ loadingState.label || messagesText.loading }}</div>
    </slot>

    <slot v-else-if="!messages.length" name="empty">
      <div class="jcr-state jcr-state--empty">{{ messagesText.empty }}</div>
    </slot>

    <template v-else>
      <div v-if="paginationState.hasMore" class="jcr-pagination">
        <button type="button" :disabled="paginationState.loading" @click="$emit('load-older')">
          <span v-if="paginationState.loading" class="jcr-spinner" />{{ messagesText.loadOlder }}
        </button>
        <button type="button" :disabled="paginationState.loadingAll" @click="$emit('load-all')">{{ messagesText.loadAll }}</button>
        <small v-if="paginationState.total">{{ messages.length }} / {{ paginationState.total }}</small>
      </div>

      <ChatMessageList
        :messages="messages" :loading-state="loadingState" :show-actions="showActions"
        :assistant-actions-ready="assistantActionsReady" :actions="actions" :resolve-actions="resolveActions"
        :adapters="adapters" :tool-renderers="toolRenderers" :labels="messagesText" :theme="effectiveTheme"
        :allow-html="allowHtml" :max-render-chars="maxRenderChars" :last-completed-assistant-id="lastCompletedAssistantId"
        :markdown-auto-fold-threshold="markdownAutoFoldThreshold"
        :markdown-show-code-row-number="markdownShowCodeRowNumber"
        @message-action="$emit('message-action', $event)" @link-click="(href, event) => $emit('link-click', href, event)"
        @image-preview="(images, index) => $emit('image-preview', images, index)" @show-full-content="$emit('show-full-content', $event)"
      >
        <template v-if="$slots.avatar" #avatar="scope"><slot name="avatar" v-bind="scope" /></template>
        <template v-if="$slots.header" #header="scope"><slot name="header" v-bind="scope" /></template>
        <template v-if="$slots.content" #content="scope"><slot name="content" v-bind="scope" /></template>
        <template v-if="$slots.text" #text="scope"><slot name="text" v-bind="scope" /></template>
        <template v-if="$slots.reasoning" #reasoning="scope"><slot name="reasoning" v-bind="scope" /></template>
        <template v-if="$slots.attachment" #attachment="scope"><slot name="attachment" v-bind="scope" /></template>
        <template v-if="$slots.tool" #tool="scope"><slot name="tool" v-bind="scope" /></template>
        <template v-if="$slots.custom" #custom="scope"><slot name="custom" v-bind="scope" /></template>
        <template v-if="$slots['before-content']" #before-content="scope"><slot name="before-content" v-bind="scope" /></template>
        <template v-if="$slots['after-content']" #after-content="scope"><slot name="after-content" v-bind="scope" /></template>
        <template v-if="$slots['message-after']" #message-after="scope"><slot name="message-after" v-bind="scope" /></template>
        <template v-if="$slots['action-bar']" #action-bar="scope"><slot name="action-bar" v-bind="scope" /></template>
        <template v-if="$slots.action" #action="scope"><slot name="action" v-bind="scope" /></template>
        <template v-if="$slots['action-icon']" #action-icon="scope"><slot name="action-icon" v-bind="scope" /></template>
        <template v-if="$slots['action-menu']" #action-menu="scope"><slot name="action-menu" v-bind="scope" /></template>
      </ChatMessageList>

      <slot v-if="loadingState.generating" name="generating" :state="loadingState">
        <div class="jcr-generating" role="status"><span class="jcr-spinner" />{{ loadingState.label || messagesText.generating }}</div>
      </slot>
      <slot v-if="loadingState.stalled" name="stalled" :state="loadingState">
        <div class="jcr-stalled" role="alert"><span>{{ messagesText.stalled }}</span><button type="button" @click="$emit('recover')">{{ messagesText.recover }}</button></div>
      </slot>
    </template>

    <button v-if="hasNewMessages" class="jcr-new-messages" type="button" @click="scrollToBottom()">↓ {{ messagesText.newMessages }}</button>
  </section>
</template>
