<script setup lang="ts">
import { computed } from 'vue'
import RendererBoundary from '../RendererBoundary.vue'
import { defaultMessages } from '../messages'
import type { ChatMessage, ChatRendererMessages, ChatWorkspacePlugins, CustomPart } from '../types'

const props = withDefaults(defineProps<{
  part: CustomPart
  message: ChatMessage
  plugins?: ChatWorkspacePlugins
  labels?: Partial<ChatRendererMessages>
}>(), { plugins: () => ({}), labels: () => ({}) })

const renderer = computed(() => props.plugins.partRenderers?.[props.part.kind])
const text = computed(() => ({ ...defaultMessages, ...props.labels }))
</script>

<template>
  <RendererBoundary v-if="renderer" :reset-key="`${message.id}:${part.kind}`">
    <component :is="renderer" :part="part" :message="message" :data="part.data" />
    <template #fallback><p class="jcr-fallback">{{ text.missingRenderer }}: {{ part.kind }}</p></template>
  </RendererBoundary>
  <p v-else class="jcr-fallback">{{ text.unknownContent }}: {{ part.kind }}</p>
</template>
