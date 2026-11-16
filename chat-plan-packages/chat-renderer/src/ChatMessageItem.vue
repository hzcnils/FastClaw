<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import ChatMarkdownRenderer from './ChatMarkdownRenderer.vue'
import RendererBoundary from './RendererBoundary.vue'
import ChatActionIcon from './ChatActionIcon.vue'
import ChatFileIcon from './ChatFileIcon.vue'
import ChatMessageHeader from './ChatMessageHeader.vue'
import ChatMessageError from './ChatMessageError.vue'
import { calculateDirectActionCount, safeUrl, textFromMessage } from './core'
import type {
  AttachmentPart, ChatMessage, ChatRendererAdapters, ChatRendererMessages, ImagePart,
  ResolvedMessageAction, ToolRendererRegistry, MessageActionEvent, MessageActionMenuItem,
} from './types'

const props = withDefaults(defineProps<{
  message: ChatMessage
  actions?: ResolvedMessageAction[]
  messages: ChatRendererMessages
  adapters?: ChatRendererAdapters
  toolRenderers?: ToolRendererRegistry
  allowHtml?: boolean
  maxRenderChars?: number
  markdownAutoFoldThreshold?: number
  markdownShowCodeRowNumber?: boolean
  theme?: 'light' | 'dark'
  showModelHeader?: boolean
  showAvatar?: boolean
}>(), {
  actions: () => [], toolRenderers: () => ({}), allowHtml: false, maxRenderChars: 15000, markdownAutoFoldThreshold: 10000, markdownShowCodeRowNumber: true,
  theme: 'light', showModelHeader: true, showAvatar: true,
})

const emit = defineEmits<{
  (event: 'message-action', value: MessageActionEvent): void
  (event: 'link-click', href: string, nativeEvent: MouseEvent): void
  (event: 'image-preview', images: ImagePart[], activeIndex: number): void
  (event: 'show-full-content', message: ChatMessage): void
}>()

const copied = ref(false)
const confirming = ref<string | null>(null)
const reasoningOpen = ref<Record<number, boolean>>({})
const actionsRoot = ref<HTMLElement | null>(null)
const directActionCount = ref(5)
let actionResizeObserver: ResizeObserver | null = null
const visibleActions = computed(() => props.message.status === 'error' ? [] : props.actions)
const safeAvatar = computed(() => props.message.model?.avatar ? safeUrl(props.message.model.avatar, 'image') : null)
const hasDedicatedStreamingPart = computed(() => props.message.parts.some(part =>
  (part.type === 'reasoning' && part.status === 'streaming')
  || (part.type === 'tool-call' && part.status === 'running'),
) || Boolean(props.message.retry))
const primaryActions = computed(() => props.message.role === 'user'
  ? visibleActions.value
  : visibleActions.value.slice(0, directActionCount.value))
const overflowActions = computed(() => props.message.role === 'user'
  ? []
  : visibleActions.value.slice(directActionCount.value))

function errorSource() {
  return props.message.error || textFromMessage(props.message)
}

function emitErrorAction(value: { action: { id: string }; error: unknown }) {
  emit('message-action', {
    action: 'error-action',
    messageId: props.message.id,
    turnId: props.message.turnId,
    message: props.message,
    payload: { actionId: value.action.id, error: value.error },
  })
}

function isActionActive(action: ResolvedMessageAction) {
  return action.name === 'like'
    ? props.message.feedback === 1
    : action.name === 'dislike' && props.message.feedback === -1
}

function actionText(action: ResolvedMessageAction) {
  if (action.tooltip) return action.tooltip
  if (action.name === 'copy' && copied.value) return props.messages.copied
  return props.messages[action.name as keyof ChatRendererMessages] || action.label
}

function needsActionDivider(action: ResolvedMessageAction, index: number) {
  if (index === 0) return false
  if (action.dividerBefore) return true
  if (action.name !== 'like' && action.name !== 'dislike') return false
  const previous = visibleActions.value[index - 1]
  return previous?.name !== 'like' && previous?.name !== 'dislike'
}

function recalculateActionCount() {
  if (props.message.role === 'user') {
    directActionCount.value = visibleActions.value.length
    return
  }
  const width = actionsRoot.value?.clientWidth || 0
  directActionCount.value = calculateDirectActionCount(visibleActions.value, width)
}

function closeMenus() {
  actionsRoot.value?.querySelectorAll<HTMLDetailsElement>('details[open]').forEach(details => details.removeAttribute('open'))
}

function keepOnlyCurrentMenu(event: Event) {
  const current = event.currentTarget as HTMLDetailsElement
  if (!current.open) return
  actionsRoot.value?.querySelectorAll<HTMLDetailsElement>('details[open]').forEach(details => {
    if (details !== current) details.removeAttribute('open')
  })
}

function onOutsidePointerDown(event: PointerEvent) {
  if (!actionsRoot.value?.contains(event.target as Node)) closeMenus()
}

function onActionsKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') { closeMenus(); (event.target as HTMLElement)?.blur() }
}

function formattedDate() {
  if (props.adapters?.formatDate) return props.adapters.formatDate(props.message.createdAt, props.message)
  try { return new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' }).format(new Date(props.message.createdAt)) }
  catch { return '' }
}

function dateTimeAttribute() {
  try { return new Date(props.message.createdAt).toISOString() } catch { return undefined }
}

async function copyMessage() {
  const text = textFromMessage(props.message)
  if (!text) return
  let success = false
  if (props.adapters?.clipboard) success = await props.adapters.clipboard(text)
  else if (typeof navigator !== 'undefined' && navigator.clipboard) {
    try { await navigator.clipboard.writeText(text); success = true } catch { success = false }
  }
  if (success) { copied.value = true; window.setTimeout(() => { copied.value = false }, 1600) }
}

async function triggerAction(action: ResolvedMessageAction, menuItem?: MessageActionMenuItem) {
  if (action.disabled) return
  if (!menuItem && action.requireConfirm && confirming.value !== action.actionId) { confirming.value = action.actionId; return }
  confirming.value = null
  if (!menuItem && action.name === 'copy') await copyMessage()
  const selection = typeof window !== 'undefined' ? window.getSelection()?.toString() || undefined : undefined
  emit('message-action', {
    action: action.actionId,
    messageId: props.message.id,
    turnId: props.message.turnId,
    message: props.message,
    selectedText: selection,
    payload: menuItem?.payload ?? action.payload,
    menuItemId: menuItem?.id,
    menuItem,
  })
}

async function triggerAndClose(action: ResolvedMessageAction) {
  const wasConfirming = confirming.value === action.actionId
  await triggerAction(action)
  if (!action.requireConfirm || wasConfirming) closeMenus()
}

async function triggerMenuItem(action: ResolvedMessageAction, menuItem: MessageActionMenuItem) {
  if (menuItem.disabled) return
  await triggerAction(action, menuItem)
  closeMenus()
}

function triggerItemFor(action: ResolvedMessageAction) {
  return (menuItem: MessageActionMenuItem) => triggerMenuItem(action, menuItem)
}

function imageMarkdown(part: ImagePart): string {
  const source = safeUrl(part.src, 'image')
  if (!source) return ''
  const normalizedSource = source.startsWith('data:image/')
    ? source.replace(/\s+/g, '')
    : source.replace(/\s/g, '%20').replace(/\(/g, '%28').replace(/\)/g, '%29')
  const alt = (part.alt || part.name || '')
    .replace(/\\/g, '\\\\')
    .replace(/\[/g, '\\[')
    .replace(/\]/g, '\\]')
  return `![${alt}](${normalizedSource})`
}

function handleLink(href: string, event: MouseEvent) {
  if (props.adapters?.link) { event.preventDefault(); props.adapters.link(href, event) }
  emit('link-click', href, event)
}

function referenceAttachment(part: AttachmentPart, partIndex: number, nativeEvent: MouseEvent) {
  emit('message-action', {
    action: 'custom',
    messageId: props.message.id,
    turnId: props.message.turnId,
    message: props.message,
    payload: { type: 'attachment-reference', attachment: part, partIndex },
  })
  nativeEvent.stopPropagation()
}

function pretty(value: unknown) {
  if (typeof value === 'string') return value
  try { return JSON.stringify(value, null, 2) } catch { return String(value ?? '') }
}

function translationLabel(message: ChatMessage, index: number) {
  return String(message.metadata?.label || message.metadata?.language || message.model?.name || `Translation ${index + 1}`)
}

onMounted(() => {
  recalculateActionCount()
  document.addEventListener('pointerdown', onOutsidePointerDown)
})
onBeforeUnmount(() => {
  actionResizeObserver?.disconnect()
  document.removeEventListener('pointerdown', onOutsidePointerDown)
})
watch(actionsRoot, node => {
  actionResizeObserver?.disconnect()
  actionResizeObserver = null
  if (typeof ResizeObserver !== 'undefined' && node) {
    actionResizeObserver = new ResizeObserver(recalculateActionCount)
    actionResizeObserver.observe(node)
  }
  recalculateActionCount()
}, { flush: 'post' })
watch(visibleActions, async () => { await nextTick(); recalculateActionCount() }, { deep: true })
</script>

<template>
  <article class="jcr-message" :class="[`jcr-message--${message.role}`, `jcr-message--${message.status || 'complete'}`]" :data-message-id="message.id">
    <slot v-if="showAvatar" name="avatar" :message="message">
      <div class="jcr-avatar" aria-hidden="true">
        <img v-if="safeAvatar" :src="safeAvatar" alt="" />
        <svg v-else-if="message.role === 'user'" class="jcr-user-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><circle cx="12" cy="8" r="4" /><path d="M4 21a8 8 0 0 1 16 0" /></svg>
        <span v-else>{{ message.role === 'assistant' ? 'AI' : '•' }}</span>
      </div>
    </slot>
    <div class="jcr-message__main">
      <slot v-if="message.role === 'assistant' && showModelHeader" name="header" :message="message">
        <ChatMessageHeader :message="message" :adapters="adapters" />
      </slot>

      <slot name="before-content" :message="message" />

      <div class="jcr-message__content">
        <ChatMessageError v-if="message.status === 'error'" :error="errorSource()" @error-action="emitErrorAction" />
        <div v-else-if="message.status === 'streaming' && message.retry" class="jcr-message-retry" role="status">
          <span class="jcr-message-retry__spinner" aria-hidden="true" />
          <span>{{ message.retry.label || '正在重试中…' }}{{ message.retry.attempt }}/{{ message.retry.total }}</span>
        </div>
        <slot name="content" :message="message" :parts="message.parts">
          <template v-if="message.status !== 'error' && !message.retry" v-for="(part, index) in message.parts" :key="`${part.type}-${index}`">
            <slot v-if="part.type === 'text'" name="text" :part="part" :message="message" :part-index="index">
              <div v-if="part.format === 'plain' || message.role === 'user'" class="jcr-plain-text">{{ part.text }}</div>
              <pre v-else-if="part.format === 'json'" class="jcr-json">{{ pretty(part.text) }}</pre>
              <ChatMarkdownRenderer
                v-else
                :content="part.text" :allow-html="allowHtml" :max-chars="maxRenderChars" :theme="theme"
                :auto-fold-threshold="markdownAutoFoldThreshold"
                :show-code-row-number="markdownShowCodeRowNumber"
                :streaming="message.status === 'streaming'"
                @link-click="handleLink" @show-full-content="$emit('show-full-content', message)"
              ><template #show-full-label>{{ messages.showFullContent }}</template></ChatMarkdownRenderer>
            </slot>

            <slot v-else-if="part.type === 'reasoning'" name="reasoning" :part="part" :message="message" :part-index="index">
              <details class="jcr-reasoning" :open="reasoningOpen[index]" @toggle="reasoningOpen[index] = ($event.target as HTMLDetailsElement).open">
                <summary>{{ part.label || messages.reasoning }}<span v-if="part.status === 'streaming'" class="jcr-live-dot" /></summary>
                <ChatMarkdownRenderer :content="part.text" :allow-html="allowHtml" :max-chars="maxRenderChars" :theme="theme"
                  :auto-fold-threshold="markdownAutoFoldThreshold"
                  :show-code-row-number="markdownShowCodeRowNumber"
                  :streaming="part.status === 'streaming' || message.status === 'streaming'" @link-click="handleLink" />
              </details>
            </slot>

          <ChatMarkdownRenderer
            v-else-if="part.type === 'image' && safeUrl(part.src, 'image')"
            class="jcr-markdown--image"
            :content="imageMarkdown(part)"
            :allow-html="false"
            :max-chars="imageMarkdown(part).length"
            :auto-fold-threshold="markdownAutoFoldThreshold"
            :show-code-row-number="markdownShowCodeRowNumber"
            :theme="theme"
          />

          <slot v-else-if="part.type === 'attachment'" name="attachment" :part="part" :message="message">
            <div class="jcr-attachment" :title="part.name">
              <a v-if="part.url && safeUrl(part.url)" class="jcr-attachment__main" :href="safeUrl(part.url)!"
                rel="noopener noreferrer" @click="handleLink(part.url!, $event)">
                <ChatFileIcon :name="part.name" :mime-type="part.mimeType" generic />
                <strong>{{ part.name }}</strong>
              </a>
              <span v-else class="jcr-attachment__main">
                <ChatFileIcon :name="part.name" :mime-type="part.mimeType" generic />
                <strong>{{ part.name }}</strong>
              </span>
              <button v-if="message.role === 'user'" type="button" class="jcr-attachment__reference"
                :title="messages.attachmentReference" :aria-label="`${messages.attachmentReference}: ${part.name}`"
                @click="referenceAttachment(part, index, $event)">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  <path d="M9 17H6.75A3.75 3.75 0 0 1 3 13.25V8.75A3.75 3.75 0 0 1 6.75 5h7.5A3.75 3.75 0 0 1 18 8.75V10" />
                  <path d="m14 15 3-3 3 3M17 12v7" />
                </svg>
              </button>
            </div>
          </slot>

          <RendererBoundary v-else-if="part.type === 'tool-call'" :reset-key="`${part.id}-${part.status}`">
            <slot name="tool" :part="part" :message="message">
              <component v-if="toolRenderers[part.name]" :is="toolRenderers[part.name]" :part="part" :message="message" />
              <details v-else class="jcr-tool" :open="part.status === 'running' || part.status === 'error'">
                <summary><span>⌘</span><strong>{{ part.name }}</strong><span class="jcr-tool__status">{{ part.status || 'complete' }}</span></summary>
                <div v-if="part.input !== undefined"><small>{{ messages.toolInput }}</small><pre>{{ pretty(part.input) }}</pre></div>
                <div v-if="part.output !== undefined || part.error"><small>{{ messages.toolOutput }}</small><pre :class="{ 'jcr-error': part.error }">{{ part.error || pretty(part.output) }}</pre></div>
              </details>
            </slot>
            <template #fallback><div class="jcr-fallback">{{ messages.unknownContent }}: {{ part.name }}</div></template>
          </RendererBoundary>

          <div v-else-if="part.type === 'notice'" class="jcr-notice" :data-tone="part.tone || 'neutral'">{{ part.text }}</div>
          <RendererBoundary v-else-if="part.type === 'custom'" :reset-key="`${part.kind}-${index}`">
            <slot name="custom" :part="part" :message="message"><div class="jcr-fallback">{{ messages.unknownContent }}: {{ part.kind }}</div></slot>
            <template #fallback><div class="jcr-fallback">{{ messages.unknownContent }}: {{ part.kind }}</div></template>
          </RendererBoundary>
            <div v-else class="jcr-fallback">{{ messages.unknownContent }}</div>
          </template>
          <span v-if="message.status === 'streaming' && !hasDedicatedStreamingPart" class="jcr-streaming" role="status" :aria-label="messages.generating"><i /><i /><i /></span>
        </slot>
      </div>

      <slot name="after-content" :message="message" />

      <div v-if="message.translations?.length" class="jcr-translations">
        <details v-for="(translation, translationIndex) in message.translations" :key="translation.id" class="jcr-translation" open>
          <summary>{{ translationLabel(translation, translationIndex) }}<span v-if="translation.status === 'streaming'" class="jcr-live-dot" /></summary>
          <template v-for="(part, partIndex) in translation.parts" :key="`${part.type}-${partIndex}`">
            <div v-if="part.type === 'text' && part.format === 'plain'" class="jcr-plain-text">{{ part.text }}</div>
            <ChatMarkdownRenderer
              v-else-if="part.type === 'text'"
              :content="part.text" :allow-html="allowHtml" :max-chars="maxRenderChars" :theme="theme"
              :auto-fold-threshold="markdownAutoFoldThreshold"
              :show-code-row-number="markdownShowCodeRowNumber"
              :streaming="translation.status === 'streaming'"
              @link-click="handleLink"
            />
            <details v-else-if="part.type === 'reasoning'" class="jcr-reasoning">
              <summary>{{ part.label || messages.reasoning }}</summary>
              <ChatMarkdownRenderer :content="part.text" :allow-html="allowHtml" :max-chars="maxRenderChars" :theme="theme"
                :auto-fold-threshold="markdownAutoFoldThreshold"
                :show-code-row-number="markdownShowCodeRowNumber"
                :streaming="part.status === 'streaming' || translation.status === 'streaming'" @link-click="handleLink" />
            </details>
          </template>
        </details>
      </div>

      <div v-if="(message.role === 'user' && formattedDate()) || visibleActions.length" class="jcr-message__meta">
        <time v-if="message.role === 'user' && formattedDate()" class="jcr-message__time" :datetime="dateTimeAttribute()">{{ formattedDate() }}</time>
        <slot name="action-bar" :message="message" :actions="visibleActions" :trigger="triggerAction">
        <div v-if="visibleActions.length" ref="actionsRoot" class="jcr-actions" @keydown="onActionsKeydown">
          <template v-for="(action, index) in primaryActions" :key="action.actionId">
            <span v-if="needsActionDivider(action, index)" class="jcr-action-divider" aria-hidden="true" />
            <slot name="action" :action="action" :message="message" :trigger="triggerAction">
              <span v-if="confirming === action.actionId" class="jcr-confirm">
                <span>{{ actionText(action) }}?</span>
                <button type="button" @click="triggerAndClose(action)">{{ messages.confirm }}</button>
                <button type="button" @click="confirming = null">{{ messages.cancel }}</button>
              </span>
              <details v-else-if="!action.disabled && (action.menuItems?.length || (action.customMenu && $slots['action-menu']))" class="jcr-action-menu" @toggle="keepOnlyCurrentMenu">
                <summary class="jcr-action" :class="{ 'jcr-action--active': isActionActive(action) }" :title="actionText(action)" :aria-label="actionText(action)" aria-haspopup="menu">
                  <slot name="action-icon" :action="action" :message="message" :icon-name="action.loading ? 'loader' : action.name" :active="isActionActive(action)" :copied="false">
                    <ChatActionIcon :name="action.loading ? 'loader' : action.name" :active="isActionActive(action)" :custom-icon="action.loading ? undefined : action.icon" />
                  </slot>
                </summary>
                <div class="jcr-action-menu__popup" :class="{ 'jcr-action-menu__popup--custom': action.customMenu }" role="menu" :aria-label="actionText(action)">
                  <slot v-if="action.customMenu" name="action-menu" :action="action" :message="message" placement="direct" :trigger-item="triggerItemFor(action)" :close="closeMenus">
                    <button v-for="item in action.menuItems" :key="item.id" type="button" role="menuitem" :class="{ 'jcr-action-menu__danger': item.danger }" :disabled="item.disabled" @click="triggerMenuItem(action, item)">{{ item.label }}</button>
                  </slot>
                  <template v-else>
                    <button v-for="item in action.menuItems" :key="item.id" type="button" role="menuitem" :class="{ 'jcr-action-menu__danger': item.danger }" :disabled="item.disabled" @click="triggerMenuItem(action, item)">{{ item.label }}</button>
                  </template>
                </div>
              </details>
              <button v-else type="button" class="jcr-action" :class="{ 'jcr-action--danger': action.danger, 'jcr-action--active': isActionActive(action), 'jcr-action--negative': action.name === 'dislike' && isActionActive(action) }" :disabled="action.disabled" :title="actionText(action)" :aria-label="actionText(action)" :aria-pressed="action.name === 'like' ? message.feedback === 1 : action.name === 'dislike' ? message.feedback === -1 : undefined" @click="triggerAndClose(action)">
                <slot name="action-icon" :action="action" :message="message" :icon-name="action.loading ? 'loader' : action.name" :active="isActionActive(action)" :copied="action.name === 'copy' && copied">
                  <ChatActionIcon :name="action.loading ? 'loader' : action.name" :copied="action.name === 'copy' && copied" :active="isActionActive(action)" :custom-icon="action.loading ? undefined : action.icon" />
                </slot>
              </button>
            </slot>
          </template>
          <details v-if="message.role !== 'user' && overflowActions.length" class="jcr-more" @toggle="keepOnlyCurrentMenu">
            <summary class="jcr-action" :title="messages.more" :aria-label="messages.more">
              <slot name="action-icon" :action="undefined" :message="message" icon-name="custom" :active="false" :copied="false"><ChatActionIcon name="custom" /></slot>
            </summary>
            <div class="jcr-more__menu" role="menu">
              <template v-for="action in overflowActions" :key="action.actionId">
                <slot name="action" :action="action" :message="message" :trigger="triggerAction">
                  <span v-if="confirming === action.actionId" class="jcr-confirm">
                    <span>{{ actionText(action) }}?</span>
                    <button type="button" @click="triggerAndClose(action)">{{ messages.confirm }}</button>
                    <button type="button" @click="confirming = null">{{ messages.cancel }}</button>
                  </span>
                  <div v-else-if="!action.disabled && (action.menuItems?.length || (action.customMenu && $slots['action-menu']))" class="jcr-more__group">
                    <span class="jcr-more__group-label">
                      <slot name="action-icon" :action="action" :message="message" :icon-name="action.name" :active="isActionActive(action)" :copied="false"><ChatActionIcon :name="action.name" :custom-icon="action.icon" /></slot>{{ actionText(action) }}
                    </span>
                    <slot v-if="action.customMenu" name="action-menu" :action="action" :message="message" placement="overflow" :trigger-item="triggerItemFor(action)" :close="closeMenus">
                      <button v-for="item in action.menuItems" :key="item.id" type="button" role="menuitem" :class="{ 'jcr-action-menu__danger': item.danger }" :disabled="item.disabled" @click="triggerMenuItem(action, item)">{{ item.label }}</button>
                    </slot>
                    <template v-else>
                      <button v-for="item in action.menuItems" :key="item.id" type="button" role="menuitem" :class="{ 'jcr-action-menu__danger': item.danger }" :disabled="item.disabled" @click="triggerMenuItem(action, item)">{{ item.label }}</button>
                    </template>
                  </div>
                  <button v-else type="button" role="menuitem" class="jcr-more__item" :class="{ 'jcr-action--danger': action.danger, 'jcr-action--active': isActionActive(action) }" :disabled="action.disabled" @click="triggerAndClose(action)">
                    <slot name="action-icon" :action="action" :message="message" :icon-name="action.loading ? 'loader' : action.name" :active="isActionActive(action)" :copied="action.name === 'copy' && copied">
                      <ChatActionIcon :name="action.loading ? 'loader' : action.name" :active="isActionActive(action)" :custom-icon="action.loading ? undefined : action.icon" />
                    </slot><span>{{ action.name === 'copy' && copied ? messages.copied : (messages[action.name as keyof ChatRendererMessages] || action.label) }}</span>
                  </button>
                </slot>
              </template>
            </div>
          </details>
        </div>
        </slot>
      </div>
    </div>
  </article>
</template>
