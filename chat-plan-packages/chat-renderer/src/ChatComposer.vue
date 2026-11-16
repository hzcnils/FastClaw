<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { resolveComposerActions } from './core'
import ChatComposerIcon from './ChatComposerIcon.vue'
import ChatComposerSelector from './ChatComposerSelector.vue'
import ChatFileIcon from './ChatFileIcon.vue'
import { resolveChatRendererMessages } from './messages'
import type {
  ChatComposerAction, ChatComposerActionEvent, ChatComposerAttachment, ChatComposerAttachmentActionEvent,
  ChatComposerFileSelectionEvent, ChatComposerSelector as ChatComposerSelectorConfig,
  ChatComposerRegionScope, ChatComposerSelectorChangeEvent, ChatComposerSubmitEvent, ChatRendererMessages,
} from './types'

const props = withDefaults(defineProps<{
  modelValue?: string
  placeholder?: string
  disabled?: boolean
  readonly?: boolean
  loading?: boolean
  actions?: ChatComposerAction[]
  selectors?: ChatComposerSelectorConfig[]
  attachments?: ChatComposerAttachment[]
  maxLength?: number
  maxHeight?: number
  autoResize?: boolean
  sendByCtrlEnter?: boolean
  clearOnSubmit?: boolean
  submitLabel?: string
  stopLabel?: string
  locale?: string
  theme?: 'light' | 'dark'
  labels?: Partial<ChatRendererMessages>
}>(), {
  modelValue: '', placeholder: undefined, disabled: false, readonly: false, loading: false,
  actions: () => [], selectors: () => [], attachments: () => [], maxLength: 100000, maxHeight: 200, autoResize: true,
  sendByCtrlEnter: false, clearOnSubmit: false, submitLabel: undefined, stopLabel: undefined, locale: 'zh-CN', theme: 'light', labels: () => ({}),
})

const emit = defineEmits<{
  (event: 'update:modelValue', value: string): void
  (event: 'submit', value: ChatComposerSubmitEvent): void
  (event: 'abort', nativeEvent: MouseEvent): void
  (event: 'composer-action', value: ChatComposerActionEvent): void
  (event: 'files-selected', value: ChatComposerFileSelectionEvent): void
  (event: 'selector-change', value: ChatComposerSelectorChangeEvent): void
  (event: 'attachment-remove', attachment: ChatComposerAttachment, index: number): void
  (event: 'attachment-action', value: ChatComposerAttachmentActionEvent): void
  (event: 'focus', nativeEvent: FocusEvent): void
  (event: 'blur', nativeEvent: FocusEvent): void
}>()

const textarea = ref<HTMLTextAreaElement | null>(null)
const fileInputs = new Map<string, HTMLInputElement>()
const composing = ref(false)
const visibleActions = computed(() => resolveComposerActions(props.actions))
const startActions = computed(() => visibleActions.value.filter(action => (action.position || 'start') === 'start'))
const endActions = computed(() => visibleActions.value.filter(action => action.position === 'end'))
const canSubmit = computed(() => !props.disabled && !props.readonly && (!!props.modelValue.trim() || props.attachments.length > 0))
const messages = computed(() => resolveChatRendererMessages(props.locale, props.labels))
const effectivePlaceholder = computed(() => props.placeholder || (props.locale?.toLowerCase().startsWith('en') ? 'Type a message…' : '输入消息…'))
const effectiveSubmitLabel = computed(() => props.submitLabel || (props.locale?.toLowerCase().startsWith('en') ? 'Send' : '发送'))
const effectiveStopLabel = computed(() => props.stopLabel || (props.locale?.toLowerCase().startsWith('en') ? 'Stop' : '停止'))

function attachmentStatus(attachment: ChatComposerAttachment) {
  const status = attachment.status || 'idle'
  const key = `attachment${status.charAt(0).toUpperCase()}${status.slice(1)}` as keyof ChatRendererMessages
  return messages.value[key]
}

function formatFileSize(size?: number) {
  if (size === undefined || !Number.isFinite(size)) return ''
  if (size < 1024) return `${size} B`
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(size < 10 * 1024 ? 1 : 0)} KB`
  return `${(size / 1024 / 1024).toFixed(1)} MB`
}

function attachmentAction(action: ChatComposerAttachmentActionEvent['action'], attachment: ChatComposerAttachment, index: number, nativeEvent?: Event) {
  emit('attachment-action', { action, attachment, index, nativeEvent })
  if (action === 'remove') emit('attachment-remove', attachment, index)
}

async function resize() {
  await nextTick()
  if (!textarea.value) return
  if (!props.autoResize) {
    textarea.value.style.height = ''
    textarea.value.style.overflowY = 'auto'
    return
  }
  textarea.value.style.height = 'auto'
  textarea.value.style.height = `${Math.min(textarea.value.scrollHeight, props.maxHeight)}px`
  textarea.value.style.overflowY = textarea.value.scrollHeight > props.maxHeight ? 'auto' : 'hidden'
}

function syntheticEvent(type: string): Event {
  return typeof Event === 'function' ? new Event(type) : { type } as Event
}

function submit(nativeEvent?: Event) {
  if (!canSubmit.value || props.loading || composing.value) return
  emit('submit', { text: props.modelValue, attachments: [...props.attachments], nativeEvent: nativeEvent || syntheticEvent('submit') })
  if (props.clearOnSubmit) emit('update:modelValue', '')
}

function abort(nativeEvent?: MouseEvent) {
  const event = nativeEvent || (typeof MouseEvent === 'function'
    ? new MouseEvent('click')
    : syntheticEvent('click') as MouseEvent)
  emit('abort', event)
}

function onKeydown(event: KeyboardEvent) {
  if (composing.value || event.key !== 'Enter') return
  const shouldSubmit = props.sendByCtrlEnter ? (event.ctrlKey || event.metaKey) : !event.shiftKey
  if (!shouldSubmit) return
  event.preventDefault()
  submit(event)
}

function triggerAction(config: ChatComposerAction, nativeEvent: MouseEvent) {
  if (config.disabled) return
  if (config.kind === 'file') {
    fileInputs.get(config.id)?.click()
    return
  }
  emit('composer-action', { action: config.id, config, nativeEvent, payload: config.payload })
}

function setFileInput(actionId: string, element: unknown) {
  if (element instanceof HTMLInputElement) fileInputs.set(actionId, element)
  else fileInputs.delete(actionId)
}

function filesSelected(config: ChatComposerAction, nativeEvent: Event) {
  const input = nativeEvent.target as HTMLInputElement
  const files = [...(input.files || [])]
  if (files.length) emit('files-selected', { action: config.id, config, files, nativeEvent })
  input.value = ''
}

const regionScope = computed<ChatComposerRegionScope>(() => ({
  value: props.modelValue,
  attachments: props.attachments,
  selectors: props.selectors,
  actions: visibleActions.value,
  disabled: props.disabled,
  readonly: props.readonly,
  loading: props.loading,
  canSubmit: canSubmit.value && !props.loading,
  update: value => emit('update:modelValue', value),
  submit,
  abort,
  triggerAction,
}))

watch([() => props.modelValue, () => props.maxHeight, () => props.autoResize], resize)
onMounted(resize)
defineExpose({ focus: () => textarea.value?.focus(), element: textarea })
</script>

<template>
  <section class="jcr-composer" :data-theme="theme" :class="{ 'is-disabled': disabled, 'is-loading': loading }">
    <slot name="above" />
    <form class="jcr-composer__box" @submit.prevent="submit">
      <slot name="inside-top" v-bind="regionScope" />
      <div v-if="selectors.length || $slots.selectors" class="jcr-composer__selectors" data-composer-region="selectors">
        <slot name="selectors" v-bind="regionScope">
          <ChatComposerSelector v-for="selector in selectors" :key="selector.id" :selector="selector"
            @change="$emit('selector-change', $event)" />
        </slot>
      </div>
      <div v-if="attachments.length" class="jcr-composer__attachments" data-composer-region="attachments">
        <template v-for="(attachment, index) in attachments" :key="attachment.id">
          <slot name="attachment" v-bind="regionScope" :attachment="attachment" :index="index"
            :remove="(event?: Event) => attachmentAction('remove', attachment, index, event)"
            :retry="(event?: Event) => attachmentAction('retry', attachment, index, event)"
            :preview="(event?: Event) => attachmentAction('preview', attachment, index, event)">
            <div class="jcr-composer-attachment" :class="{ 'jcr-composer-attachment--image': attachment.kind === 'image' }"
              :data-status="attachment.status || 'idle'"
              :title="attachment.error || attachmentStatus(attachment)"
              :aria-busy="attachment.status === 'uploading' || attachment.status === 'processing'">
              <span class="jcr-composer-attachment__visual" :title="attachment.name">
                <img v-if="attachment.kind === 'image' && attachment.previewUrl" :src="attachment.previewUrl" :alt="attachment.name" />
                <ChatFileIcon v-else :name="attachment.name" :mime-type="attachment.mimeType" />
                <i v-if="attachment.status === 'uploading' || attachment.status === 'processing'"
                  class="jcr-composer-attachment__spinner" role="status" :aria-label="attachmentStatus(attachment)" />
                <i v-else-if="attachment.kind === 'image' && attachment.status === 'complete'"
                  class="jcr-composer-attachment__status jcr-composer-attachment__status--complete" :aria-label="attachmentStatus(attachment)">✓</i>
                <i v-else-if="attachment.kind === 'image' && attachment.status === 'error'"
                  class="jcr-composer-attachment__status jcr-composer-attachment__status--error" :aria-label="attachment.error || attachmentStatus(attachment)">!</i>
              </span>
              <span v-if="attachment.kind !== 'image'" class="jcr-composer-attachment__info">
                <strong :title="attachment.name">{{ attachment.name }}</strong>
                <small :class="{ 'jcr-error': attachment.status === 'error' }">
                  {{ attachment.status === 'error' ? (attachment.error || attachmentStatus(attachment)) : (formatFileSize(attachment.size) || attachmentStatus(attachment)) }}
                </small>
                <i v-if="attachment.status === 'uploading' || attachment.status === 'processing'" class="jcr-composer-attachment__progress"
                  role="progressbar" :aria-valuenow="attachment.progress" aria-valuemin="0" aria-valuemax="100">
                  <i :style="{ width: `${Math.max(0, Math.min(100, attachment.progress || 0))}%` }" />
                </i>
              </span>
              <button v-if="attachment.kind !== 'image' && attachment.status === 'error'" class="jcr-composer-attachment__retry" type="button"
                :aria-label="`${messages.recover} ${attachment.name}`" @click="attachmentAction('retry', attachment, index, $event)">↻</button>
              <button class="jcr-composer-attachment__remove" type="button" :aria-label="`${messages.close} ${attachment.name}`"
                @click="attachmentAction('remove', attachment, index, $event)">×</button>
            </div>
          </slot>
        </template>
      </div>
      <div class="jcr-composer__input-control" data-composer-region="input">
        <slot name="input" v-bind="regionScope">
          <textarea ref="textarea" :value="modelValue" :placeholder="effectivePlaceholder" :disabled="disabled" :readonly="readonly"
            :maxlength="maxLength" rows="1" @input="$emit('update:modelValue', ($event.target as HTMLTextAreaElement).value)"
            @keydown="onKeydown" @compositionstart="composing = true" @compositionend="composing = false"
            @focus="$emit('focus', $event)" @blur="$emit('blur', $event)" />
        </slot>
      </div>
      <footer class="jcr-composer__toolbar" data-composer-region="toolbar">
        <div class="jcr-composer__actions">
          <template v-for="action in startActions" :key="action.id">
            <slot name="action" v-bind="regionScope" :action="action" :trigger="triggerAction">
              <button type="button" :disabled="action.disabled" :title="action.tooltip || action.label"
                :aria-label="action.label" :aria-pressed="action.active || undefined" @click="triggerAction(action, $event)">
                <slot name="action-icon" v-bind="regionScope" :action="action">
                  <ChatComposerIcon :name="action.id" :custom-icon="action.icon" />
                </slot><span class="jcr-sr-only">{{ action.label }}</span>
              </button>
            </slot>
            <input v-if="action.kind === 'file'" :ref="element => setFileInput(action.id, element)" class="jcr-sr-only"
              type="file" tabindex="-1" :accept="action.accept" :multiple="action.multiple" @change="filesSelected(action, $event)" />
          </template>
          <slot name="toolbar-start" v-bind="regionScope" />
        </div>
        <div class="jcr-composer__actions jcr-composer__actions--end">
          <slot name="toolbar-end" v-bind="regionScope" />
          <template v-for="action in endActions" :key="action.id">
            <slot name="action" v-bind="regionScope" :action="action" :trigger="triggerAction">
              <button type="button" :disabled="action.disabled" :title="action.tooltip || action.label"
                :aria-label="action.label" :aria-pressed="action.active || undefined" @click="triggerAction(action, $event)">
                <slot name="action-icon" v-bind="regionScope" :action="action">
                  <ChatComposerIcon :name="action.id" :custom-icon="action.icon" />
                </slot><span class="jcr-sr-only">{{ action.label }}</span>
              </button>
            </slot>
            <input v-if="action.kind === 'file'" :ref="element => setFileInput(action.id, element)" class="jcr-sr-only"
              type="file" tabindex="-1" :accept="action.accept" :multiple="action.multiple" @change="filesSelected(action, $event)" />
          </template>
          <button v-if="loading" type="button" class="jcr-composer__submit" :aria-label="effectiveStopLabel" @click="abort($event)">
            <slot name="abort-icon" v-bind="regionScope"><ChatComposerIcon name="stop" /></slot>
          </button>
          <button v-else type="submit" class="jcr-composer__submit" :disabled="!canSubmit" :aria-label="effectiveSubmitLabel">
            <slot name="submit-icon" v-bind="regionScope"><ChatComposerIcon name="send" /></slot>
          </button>
        </div>
      </footer>
    </form>
    <slot name="below" />
  </section>
</template>
