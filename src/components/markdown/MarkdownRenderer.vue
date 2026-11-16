<script setup lang="ts">
import MarkdownViewer from '@/components/markdown/MarkdownViewer.vue'

defineProps<{ content: string }>()
</script>

<template>
  <MarkdownViewer :content="content" />
</template>
