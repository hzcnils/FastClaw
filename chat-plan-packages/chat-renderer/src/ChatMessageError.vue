<script setup lang="ts">
import { computed } from 'vue'
import type { ChatErrorAction, ChatMessageError } from './types'
import { formatChatErrorRaw, resolveChatError } from './chatErrors'

const props = defineProps<{ error: ChatMessageError | string }>()
const emit = defineEmits<{
  (event: 'error-action', value: { action: ChatErrorAction; error: ReturnType<typeof resolveChatError> }): void
}>()
const resolved = computed(() => resolveChatError(props.error))
const displayRaw = computed(() => formatChatErrorRaw(resolved.value.raw || ''))
</script>

<template>
  <section class="jcr-message-error" role="alert" :data-error-category="resolved.category">
    <pre v-if="displayRaw" class="jcr-message-error__raw">{{ displayRaw }}</pre>
    <p v-if="resolved.message" class="jcr-message-error__hint">{{ resolved.message }}</p>
    <div v-if="resolved.actions.length" class="jcr-message-error__actions">
      <button v-for="action in resolved.actions" :key="action.id" type="button"
        class="jcr-message-error__action" :data-kind="action.kind || 'secondary'"
        @click="emit('error-action', { action, error: resolved })">
        {{ action.label }}
      </button>
    </div>
  </section>
</template>
