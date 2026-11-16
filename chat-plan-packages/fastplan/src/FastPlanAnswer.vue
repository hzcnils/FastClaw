<script setup lang="ts">
import { computed } from 'vue'
import { ChatMarkdownRenderer } from 'chat-renderer'
import { FastPlanEchartsBlock } from 'planflow-planning-ui'
import { splitFastPlanFinalAnswer } from 'planflow-runtime'

const props = withDefaults(defineProps<{ kind: 'final_answer'; content: string; theme?: 'light' | 'dark' }>(), { theme: 'light' })
const parts = computed(() => splitFastPlanFinalAnswer({ kind: props.kind, content: props.content }))
</script>

<template>
  <div class="fastplan-answer">
    <template v-for="(part, index) in parts" :key="index">
      <FastPlanEchartsBlock v-if="part.type === 'chart'" :option="part.option" :source="part.source" :theme="theme" />
      <ChatMarkdownRenderer v-else :content="part.text" :theme="theme" :max-chars="Number.MAX_SAFE_INTEGER" />
    </template>
  </div>
</template>
