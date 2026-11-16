<script setup lang="ts">
import ChatMessageItem from './ChatMessageItem.vue'
import type { ChatMessage, ChatRendererAdapters, ChatRendererMessages, ImagePart, MessageActionEvent, ToolRendererRegistry } from './types'

withDefaults(defineProps<{
  messages: ChatMessage[]
  labels: ChatRendererMessages
  adapters?: ChatRendererAdapters
  toolRenderers?: ToolRendererRegistry
  allowHtml?: boolean
  maxRenderChars?: number
  markdownAutoFoldThreshold?: number
  markdownShowCodeRowNumber?: boolean
  theme?: 'light' | 'dark'
}>(), { toolRenderers: () => ({}), allowHtml: false, maxRenderChars: 15000, markdownAutoFoldThreshold: 10000, markdownShowCodeRowNumber: true, theme: 'light' })

defineEmits<{
  (event: 'message-action', value: MessageActionEvent): void
  (event: 'link-click', href: string, nativeEvent: MouseEvent): void
  (event: 'image-preview', images: ImagePart[], activeIndex: number): void
  (event: 'show-full-content', message: ChatMessage): void
}>()
</script>

<template>
  <details class="jcr-tool-group">
    <summary>⌘ {{ messages.length }} tool calls</summary>
    <div class="jcr-tool-group__content">
      <ChatMessageItem
        v-for="message in messages" :key="message.id" :message="message" :actions="[]" :messages="labels"
        :show-avatar="false" :show-model-header="false"
        :adapters="adapters" :tool-renderers="toolRenderers" :allow-html="allowHtml"
        :max-render-chars="maxRenderChars" :theme="theme"
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
      </ChatMessageItem>
    </div>
  </details>
</template>
