<script setup lang="ts">
import WebSearchSources from '@/components/chat/WebSearchSources.vue'
import type { CustomPart } from 'chat-renderer'
import { FastPlanEchartsBlock } from 'planflow-planning-ui'
import PlanThinkingCard from './PlanThinkingCard.vue'

defineProps<{ part: CustomPart; data: any }>()
</script>

<template>
  <WebSearchSources v-if="part.kind === 'fastclaw-web-search' && data?.hits?.length"
    :hits="data.hits" :query="data.query" :message-id="data.messageId" />
  <div v-else-if="part.kind === 'fastclaw-web-search'" class="fc-custom-card">正在联网搜索{{ data?.query ? `：${data.query}` : '' }}…</div>
  <div v-else-if="part.kind === 'fastclaw-skill-used'" class="fc-skill-badge">🔧 Skill · {{ data?.name }}</div>
  <div v-else-if="part.kind === 'fastclaw-echarts' && data?.option" class="fc-echarts">
    <FastPlanEchartsBlock :option="data.option" :source="data.source || ''" theme="light" />
  </div>
  <PlanThinkingCard
    v-else-if="part.kind === 'fastclaw-plan'"
    :runtime="data?.runtime"
    :session-id="data?.sessionId"
  />
</template>

<style scoped>
.fc-custom-card { padding:8px 10px; border:1px solid var(--border-color); border-radius:8px; color:var(--text-secondary); background:var(--bg-secondary); }
.fc-skill-badge { display:inline-flex; margin-top:6px; padding:3px 8px; border-radius:999px; color:var(--color-primary); background:color-mix(in srgb,var(--color-primary) 10%,transparent); font-size:12px; }
.fc-echarts { width:100%; margin:8px 0 12px; overflow:hidden; border-radius:12px; border:1px solid var(--border-color, #eef2f7); background:var(--bg-secondary, #f8fafc); }
.fc-echarts :deep(.fpp-echarts-block) { width:100%; padding:8px 4px 4px; box-sizing:border-box; }
.fc-echarts :deep(.fpp-echarts-block__canvas) { width:100% !important; min-height:280px; }
</style>

