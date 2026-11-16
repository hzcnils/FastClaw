<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import ChatMessageList from './components/ChatMessageList.vue'
import ChatPartPluginRenderer from './components/ChatPartPluginRenderer.vue'
import { usePinnedScroll } from './composables/usePinnedScroll'
import ChatModelVariants from './ChatModelVariants.vue'
import RendererBoundary from './RendererBoundary.vue'
import { resolveChatRendererMessages } from './messages'
import { lastCompletedAssistantMessageId, questionItemsFromTimeline } from './workspace'
import type {
  ActionResolver, AssistantActionsReady, ChatComparisonModeChangeEvent, ChatEmbeddedTimelineEntry, ChatMessage,
  ChatQuestionNavigationItem, ChatRendererAdapters, ChatRendererMessages, ChatTimelineEntry, ChatVariantChangeEvent,
  ChatWorkspacePlugins, ImagePart, LoadingState, MessageActionEvent, MessageActionsConfig,
  PaginationState, ToolRendererRegistry,
} from './types'

const props = withDefaults(defineProps<{
  entries?: ChatTimelineEntry[]
  plugins?: ChatWorkspacePlugins
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
  showScrollToBottom?: boolean
  scrollMode?: 'internal' | 'host'
  appearance?: 'default' | 'seamless'
  navigationItems?: ChatQuestionNavigationItem[]
}>(), {
  entries: () => [], plugins: () => ({}), loadingState: () => ({}), paginationState: () => ({}),
  showActions: true, assistantActionsReady: true, actions: () => ({}), toolRenderers: () => ({}),
  labels: () => ({}), theme: 'auto', allowHtml: false, maxRenderChars: 15000, markdownAutoFoldThreshold: 10000, markdownShowCodeRowNumber: true,
  autoScroll: true, showScrollToBottom: false, scrollMode: 'internal', appearance: 'default',
})

const emit = defineEmits<{
  (event: 'message-action', value: MessageActionEvent): void
  (event: 'variant-change', value: ChatVariantChangeEvent): void
  (event: 'comparison-mode-change', value: ChatComparisonModeChangeEvent): void
  (event: 'embedded-action', payload: unknown, entry: ChatEmbeddedTimelineEntry): void
  (event: 'active-entry-change', entryId: string): void
  (event: 'active-navigation-change', itemId: string): void
  (event: 'load-older'): void
  (event: 'load-all'): void
  (event: 'recover'): void
  (event: 'link-click', href: string, nativeEvent: MouseEvent): void
  (event: 'image-preview', images: ImagePart[], activeIndex: number): void
  (event: 'show-full-content', message: ChatMessage): void
  (event: 'container-ready', element: HTMLElement): void
}>()

type MessageRun = { kind: 'message-run'; id: string; entries: Extract<ChatTimelineEntry, { kind: 'message' }>[] }
type TimelineBlock = MessageRun | Extract<ChatTimelineEntry, { kind: 'turn' | 'embedded' }>

const root = ref<HTMLElement | null>(null)
const text = computed(() => resolveChatRendererMessages(props.locale, props.labels))
const effectiveTheme = computed<'light' | 'dark'>(() => {
  if (props.theme !== 'auto') return props.theme
  return typeof window !== 'undefined' && window.matchMedia?.('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
})
const blocks = computed<TimelineBlock[]>(() => {
  const result: TimelineBlock[] = []
  let run: MessageRun | undefined
  for (const entry of props.entries) {
    if (entry.kind !== 'message') {
      run = undefined
      result.push(entry)
      continue
    }
    if (!run) {
      run = { kind: 'message-run', id: entry.id, entries: [] }
      result.push(run)
    }
    run.entries.push(entry)
  }
  return result
})
const lastAssistantId = computed(() => lastCompletedAssistantMessageId(props.entries))
const navigationItems = computed(() => props.navigationItems ?? questionItemsFromTimeline(props.entries))
let activeEntryId = ''
let activeNavigationId = ''

function entryElement(entryId: string): HTMLElement | undefined {
  const node = root.value
  if (!node) return undefined
  const direct = [...node.querySelectorAll<HTMLElement>('[data-entry-id]')].find(item => item.dataset.entryId === entryId)
  if (direct) return direct
  const entry = props.entries.find(item => item.id === entryId)
  if (entry?.kind === 'message') {
    return [...node.querySelectorAll<HTMLElement>('[data-message-id]')].find(item => item.dataset.messageId === entry.message.id)
  }
  return undefined
}

function navigationTarget(item: ChatQuestionNavigationItem): HTMLElement | undefined {
  const entry = entryElement(item.entryId)
  if (!item.targetMessageId) return entry
  return [...(entry?.querySelectorAll<HTMLElement>('[data-message-id]') || [])]
    .find(element => element.dataset.messageId === item.targetMessageId)
}

function detectActiveEntry() {
  const node = root.value
  if (!node || !navigationItems.value.length) return
  const top = node.getBoundingClientRect().top + 96
  let current = navigationItems.value[0]
  let best = Number.POSITIVE_INFINITY
  for (const item of navigationItems.value) {
    const target = navigationTarget(item)
    if (!target) continue
    const distance = Math.abs(target.getBoundingClientRect().top - top)
    if (distance < best) { best = distance; current = item }
  }
  if (current.id !== activeNavigationId) {
    activeNavigationId = current.id
    emit('active-navigation-change', current.id)
  }
  if (current.entryId !== activeEntryId) {
    activeEntryId = current.entryId
    emit('active-entry-change', current.entryId)
  }
}

const { userScrolledUp, handleScroll, scrollToBottom } = usePinnedScroll({
  root,
  firstKey: () => props.entries[0]?.id,
  lastKey: () => props.entries[props.entries.length - 1]?.id,
  contentKey: () => props.entries,
  autoScroll: () => props.autoScroll,
  canLoadOlder: () => !!props.paginationState.hasMore && !props.paginationState.loading,
  onLoadOlder: () => emit('load-older'),
  onReady: element => {
    emit('container-ready', element)
    nextTick(detectActiveEntry)
  },
  onScroll: detectActiveEntry,
})

watch(navigationItems, async () => {
  await nextTick()
  detectActiveEntry()
}, { deep: true })

async function scrollToEntry(entryId: string, behavior: ScrollBehavior = 'smooth') {
  await nextTick()
  const target = entryElement(entryId)
  if (!target) return false
  target.scrollIntoView({ behavior, block: 'start' })
  activeEntryId = entryId
  emit('active-entry-change', entryId)
  return true
}

async function scrollToNavigationItem(item: ChatQuestionNavigationItem, behavior: ScrollBehavior = 'smooth') {
  await nextTick()
  const target = navigationTarget(item)
  if (!target) return false
  target.scrollIntoView({ behavior, block: 'start' })
  activeNavigationId = item.id
  emit('active-navigation-change', item.id)
  if (activeEntryId !== item.entryId) {
    activeEntryId = item.entryId
    emit('active-entry-change', item.entryId)
  }
  return true
}

defineExpose({ scrollToBottom, scrollToEntry, scrollToNavigationItem, element: root })
</script>

<template>
  <section ref="root" class="jcr-timeline" :data-theme="effectiveTheme" :data-scroll-mode="scrollMode"
    :data-appearance="appearance === 'default' ? undefined : appearance" :lang="locale" @scroll.passive="handleScroll">
    <slot v-if="loadingState.initial && !entries.length" name="loading" :state="loadingState">
      <div class="jcr-state" role="status"><span class="jcr-spinner" />{{ loadingState.label || text.loading }}</div>
    </slot>
    <slot v-else-if="!entries.length" name="empty">
      <div class="jcr-state jcr-state--empty">{{ text.empty }}</div>
    </slot>
    <template v-else>
      <div v-if="paginationState.hasMore" class="jcr-pagination">
        <button type="button" :disabled="paginationState.loading" @click="emit('load-older')"><span v-if="paginationState.loading" class="jcr-spinner" />{{ text.loadOlder }}</button>
        <button type="button" :disabled="paginationState.loadingAll" @click="emit('load-all')">{{ text.loadAll }}</button>
        <small v-if="paginationState.total">{{ entries.length }} / {{ paginationState.total }}</small>
      </div>

      <template v-for="block in blocks" :key="block.id">
        <slot name="entry" :entry="block">
          <ChatMessageList v-if="block.kind === 'message-run'" :messages="block.entries.map(item => item.message)"
            :loading-state="loadingState" :show-actions="showActions" :assistant-actions-ready="assistantActionsReady"
            :actions="actions" :resolve-actions="resolveActions" :adapters="adapters" :tool-renderers="toolRenderers"
            :labels="text" :theme="effectiveTheme" :allow-html="allowHtml" :max-render-chars="maxRenderChars"
            :markdown-auto-fold-threshold="markdownAutoFoldThreshold"
            :markdown-show-code-row-number="markdownShowCodeRowNumber"
            :last-completed-assistant-id="lastAssistantId" @message-action="emit('message-action', $event)"
            @link-click="(href, event) => emit('link-click', href, event)"
            @image-preview="(images, index) => emit('image-preview', images, index)"
            @show-full-content="emit('show-full-content', $event)">
            <template v-if="$slots.avatar" #avatar="scope"><slot name="avatar" v-bind="scope" /></template>
            <template v-if="$slots.header" #header="scope"><slot name="header" v-bind="scope" /></template>
            <template v-if="$slots['after-content']" #after-content="scope"><slot name="after-content" v-bind="scope" /></template>
            <template v-if="$slots['action-icon']" #action-icon="scope"><slot name="action-icon" v-bind="scope" /></template>
            <template #custom="scope"><slot name="custom" v-bind="scope"><ChatPartPluginRenderer v-bind="scope" :plugins="plugins" :labels="text" /></slot></template>
          </ChatMessageList>

          <ChatModelVariants v-else-if="block.kind === 'turn'" :turn="block" :plugins="plugins" :loading-state="loadingState"
            :show-actions="showActions" :assistant-actions-ready="assistantActionsReady" :actions="actions"
            :resolve-actions="resolveActions" :adapters="adapters" :tool-renderers="toolRenderers" :labels="text"
            :theme="effectiveTheme" :allow-html="allowHtml" :max-render-chars="maxRenderChars"
            :markdown-auto-fold-threshold="markdownAutoFoldThreshold"
            :markdown-show-code-row-number="markdownShowCodeRowNumber"
            :last-completed-assistant-id="lastAssistantId" @variant-change="emit('variant-change', $event)"
            @comparison-mode-change="emit('comparison-mode-change', $event)" @message-action="emit('message-action', $event as MessageActionEvent)"
            @link-click="(href, event) => emit('link-click', href, event)"
            @image-preview="(images, index) => emit('image-preview', images as ImagePart[], index)"
            @show-full-content="emit('show-full-content', $event as ChatMessage)">
            <template v-if="$slots.avatar" #avatar="scope"><slot name="avatar" v-bind="scope" /></template>
            <template v-if="$slots.header" #header="scope"><slot name="header" v-bind="scope" /></template>
            <template v-if="$slots['after-content']" #after-content="scope"><slot name="after-content" v-bind="scope" /></template>
            <template v-if="$slots['action-icon']" #action-icon="scope"><slot name="action-icon" v-bind="scope" /></template>
            <template #custom="scope"><slot name="custom" v-bind="scope" /></template>
          </ChatModelVariants>

          <article v-else class="jcr-timeline__embedded" :data-entry-id="block.id" :data-status="block.status">
            <slot name="embedded" :entry="block" :renderer="plugins.timelineRenderers?.[block.renderer]">
              <RendererBoundary v-if="plugins.timelineRenderers?.[block.renderer]" :reset-key="block.id">
                <component :is="plugins.timelineRenderers[block.renderer]" :entry="block" :data="block.data" :status="block.status"
                  @action="emit('embedded-action', $event, block)">
                  <template v-if="$slots['action-icon']" #action-icon="scope"><slot name="action-icon" v-bind="scope" /></template>
                </component>
                <template #fallback="scope"><slot name="renderer-error" v-bind="scope" :entry="block" /></template>
              </RendererBoundary>
              <slot v-else name="missing-renderer" :entry="block">
                <p class="jcr-fallback">{{ text.missingRenderer }}: {{ block.renderer }}</p>
              </slot>
            </slot>
          </article>
        </slot>
      </template>

      <slot v-if="loadingState.generating" name="generating" :state="loadingState">
        <div class="jcr-generating" role="status"><span class="jcr-spinner" />{{ loadingState.label || text.generating }}</div>
      </slot>
      <slot v-if="loadingState.stalled" name="stalled" :state="loadingState">
        <div class="jcr-stalled" role="alert"><span>{{ text.stalled }}</span><button type="button" @click="emit('recover')">{{ text.recover }}</button></div>
      </slot>
    </template>
    <button v-if="showScrollToBottom && userScrolledUp" class="jcr-scroll-to-bottom" type="button"
      :title="text.scrollToBottom" :aria-label="text.scrollToBottom" @click="scrollToBottom()">
      <slot name="scroll-to-bottom-icon">
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3v14m0 0 6-6m-6 6-6-6M5 21h14" /></svg>
      </slot>
    </button>
  </section>
</template>
