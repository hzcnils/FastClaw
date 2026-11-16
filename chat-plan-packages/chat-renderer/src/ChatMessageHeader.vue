<script setup lang="ts">
import { computed } from 'vue'
import ChatModelIcon from './ChatModelIcon.vue'
import { safeUrl } from './core'
import type { ChatMessage, ChatRendererAdapters } from './types'

const props = withDefaults(defineProps<{
  message: ChatMessage
  adapters?: ChatRendererAdapters
  fallbackLabel?: string
}>(), { fallbackLabel: '' })

const safeAvatar = computed(() => props.message.model?.avatar
  ? safeUrl(props.message.model.avatar, 'image')
  : null)

function formattedDate() {
  if (props.adapters?.formatDate) return props.adapters.formatDate(props.message.createdAt, props.message)
  try { return new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' }).format(new Date(props.message.createdAt)) }
  catch { return '' }
}

function dateTimeAttribute() {
  try { return new Date(props.message.createdAt).toISOString() } catch { return undefined }
}
</script>

<template>
  <header class="jcr-message__header">
    <ChatModelIcon :name="message.model?.name" :model-id="message.model?.id" :avatar="safeAvatar" />
    <span>{{ message.model?.name || fallbackLabel || message.role }}</span>
    <time :datetime="dateTimeAttribute()">{{ formattedDate() }}</time>
  </header>
</template>
