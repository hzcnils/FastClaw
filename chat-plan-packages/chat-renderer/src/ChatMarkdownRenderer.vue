<script setup lang="ts">
import { computed, useId } from 'vue'
import DOMPurify from 'dompurify'
import { MdPreview } from 'md-editor-v3'
import { safeUrl } from './core'
import { wrapMarkdownTables } from './markdownTables'
import { ensureMarkdownExtensions } from './markdownExtensions'

ensureMarkdownExtensions()

const props = withDefaults(defineProps<{
  content?: string
  allowHtml?: boolean
  maxChars?: number
  autoFoldThreshold?: number
  showCodeRowNumber?: boolean
  theme?: 'light' | 'dark'
  streaming?: boolean
}>(), {
  content: '',
  allowHtml: false,
  maxChars: 15000,
  autoFoldThreshold: 10000,
  showCodeRowNumber: true,
  theme: 'light',
  streaming: false,
})

const emit = defineEmits<{
  (event: 'link-click', href: string, nativeEvent: MouseEvent): void
  (event: 'show-full-content'): void
}>()

const previewId = `jcr-markdown-${useId().replace(/[^a-zA-Z0-9_-]/g, '')}`
// MdPreview enables ECharts by default, even without an editorExtensions registration.
// This shared renderer also displays task details; only the final-answer renderer owns charts.
const truncated = computed(() => props.content.length > props.maxChars)
const renderedContent = computed(() => truncated.value
  ? props.content.slice(0, props.maxChars)
  : props.content)
const markdownSyntax = /(^|\n)\s{0,3}(#{1,6}\s|[-+*]\s|\d+\.\s|>\s|```|~~~)|[`*_~[\]<>|]|!\[|https?:\/\//
const renderAsPlainText = computed(() => props.streaming
  || !markdownSyntax.test(renderedContent.value))

function sanitize(html: string): string {
  const cleaned = DOMPurify.sanitize(html, {
    // MdPreview's built-in controls (including the code collapse indicator)
    // use sanitized inline SVG icons.
    USE_PROFILES: { html: true, svg: true, svgFilters: true },
    ALLOW_DATA_ATTR: true,
    // md-editor-v3 uses this marker to render the generated code line numbers.
    ADD_ATTR: ['rn-wrapper'],
    FORBID_TAGS: ['script', 'style', 'iframe', 'object', 'embed', 'form'],
    // Raw HTML remains sanitized. When explicitly enabled, inline styles are
    // stripped so message content cannot override the host interface.
    FORBID_ATTR: props.allowHtml ? ['style', 'srcdoc'] : ['srcdoc'],
  })
  return wrapMarkdownTables(cleaned)
}

function handleClick(event: MouseEvent) {
  const link = (event.target as HTMLElement).closest<HTMLAnchorElement>('a[href]')
  if (!link) return
  const href = safeUrl(link.getAttribute('href') || '')
  if (!href) {
    event.preventDefault()
    return
  }
  link.rel = 'noopener noreferrer nofollow'
  emit('link-click', href, event)
}
</script>

<template>
  <div class="jcr-markdown" @click="handleClick">
    <div>
      <div v-if="renderAsPlainText" class="jcr-markdown__plain">{{ renderedContent }}</div>
      <MdPreview
        v-else
        :id="previewId"
        :model-value="renderedContent"
        :theme="theme"
        preview-theme="github"
        code-theme="atom"
        :sanitize="sanitize"
        :no-katex="streaming"
        :no-mermaid="streaming"
        :no-echarts="true"
        :no-img-zoom-in="false"
        :auto-fold-threshold="autoFoldThreshold"
        :show-code-row-number="showCodeRowNumber"
      />
    </div>
    <button v-if="truncated" class="jcr-show-full" type="button" @click="$emit('show-full-content')">
      <slot name="show-full-label">Show full content</slot>
    </button>
  </div>
</template>

<style scoped>
.jcr-markdown__plain {
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}
</style>
