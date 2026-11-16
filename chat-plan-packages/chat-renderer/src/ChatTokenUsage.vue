<script setup lang="ts">
import { computed, ref } from 'vue'
import type {
  ChatTokenUsageData,
  ChatTokenUsageFormatters,
  ChatTokenUsageLabels,
} from './types'

const props = withDefaults(defineProps<{
  usage: ChatTokenUsageData
  labels?: Partial<ChatTokenUsageLabels>
  formatters?: ChatTokenUsageFormatters
  defaultExpanded?: boolean
  showProgress?: boolean
  locale?: string
}>(), {
  defaultExpanded: false,
  showProgress: false,
  locale: 'zh-CN',
})

const emit = defineEmits<{
  toggle: [expanded: boolean]
  'update:expanded': [expanded: boolean]
}>()

const defaultLabels: ChatTokenUsageLabels = {
  roundPrefix: '第 ',
  roundSuffix: '',
  inputTokens: '输入 Token',
  outputTokens: '输出 Token',
  cache: '缓存',
  cacheHitRate: '缓存命中率',
  cacheUnavailable: '缓存命中率不可用',
  currentRound: '当前轮次 Token：',
  cumulative: '累计',
  performance: '性能：',
  firstToken: '首个 Token',
  llm: 'LLM',
  tool: '工具',
  endToEnd: '端到端',
  stepsSuffix: ' 步',
  contextComposition: '上下文构成：',
  contextUnit: '字符',
  expandDetails: '展开 Token 用量详情',
  collapseDetails: '收起 Token 用量详情',
}

const englishLabels: ChatTokenUsageLabels = {
  roundPrefix: 'Round ', roundSuffix: '', inputTokens: 'Input tokens', outputTokens: 'Output tokens',
  cache: 'Cache', cacheHitRate: 'Cache hit rate', cacheUnavailable: 'Cache hit rate unavailable',
  currentRound: 'Current round tokens:', cumulative: 'Cumulative', performance: 'Performance:',
  firstToken: 'First token', llm: 'LLM', tool: 'Tools', endToEnd: 'End to end', stepsSuffix: ' steps',
  contextComposition: 'Context composition:', contextUnit: 'characters',
  expandDetails: 'Expand token usage details', collapseDetails: 'Collapse token usage details',
}

const text = computed<ChatTokenUsageLabels>(() => ({ ...(props.locale?.toLowerCase().startsWith('en') ? englishLabels : defaultLabels), ...props.labels }))
const expanded = ref(props.defaultExpanded)

const finiteNumber = (value: number | null | undefined) => Number.isFinite(value) ? Number(value) : 0
const defaultCompact = (value: number) => {
  const number = finiteNumber(value)
  const absolute = Math.abs(number)
  if (absolute < 1_000) return String(Math.round(number))
  if (absolute < 1_000_000) {
    const rounded = Math.round((number / 1_000) * 10) / 10
    return rounded >= 1_000
      ? `${Number((rounded / 1_000).toFixed(2))}M`
      : `${Number(rounded.toFixed(1))}k`
  }
  return `${Number((number / 1_000_000).toFixed(2))}M`
}
const defaultExact = (value: number) => finiteNumber(value).toLocaleString(props.locale)
const defaultDuration = (milliseconds: number) => `${Number((finiteNumber(milliseconds) / 1_000).toFixed(1))}s`
const defaultRate = (value: number) => String(Math.round(finiteNumber(value)))

const compact = (value: number) => props.formatters?.compact?.(value) ?? defaultCompact(value)
const exact = (value: number) => props.formatters?.exact?.(value) ?? defaultExact(value)
const duration = (value: number) => props.formatters?.duration?.(value) ?? defaultDuration(value)
const rate = (value: number) => props.formatters?.rate?.(value) ?? defaultRate(value)

const cacheHitRatio = computed(() => {
  if (props.usage.cacheMissTokens == null) return null
  const read = finiteNumber(props.usage.cacheReadTokens)
  const total = read + finiteNumber(props.usage.cacheMissTokens)
  return total > 0 ? read / total : null
})

const contextParts = computed(() => props.usage.contextParts ?? [])
const contextTotal = computed(() => contextParts.value.reduce((sum, part) => sum + finiteNumber(part.value), 0))
const hasPerformance = computed(() => [
  props.usage.firstTokenMs,
  props.usage.outputTokensPerSecond,
  props.usage.llmDurationMs,
  props.usage.toolDurationMs,
  props.usage.totalDurationMs,
  props.usage.steps,
].some(value => value != null))

function toggle() {
  expanded.value = !expanded.value
  emit('toggle', expanded.value)
  emit('update:expanded', expanded.value)
}
</script>

<template>
  <div class="jcr-token-usage">
    <button
      type="button"
      class="jcr-token-usage__summary"
      :aria-expanded="expanded"
      :aria-label="expanded ? text.collapseDetails : text.expandDetails"
      :title="expanded ? text.collapseDetails : text.expandDetails"
      @click="toggle"
    >
      <svg
        class="jcr-token-usage__chevron"
        :class="{ 'is-expanded': expanded }"
        viewBox="0 0 16 16"
        aria-hidden="true"
      >
        <path d="m6 3.5 4.5 4.5L6 12.5" />
      </svg>
      <span v-if="usage.round != null" :title="`${text.roundPrefix}${usage.round}${text.roundSuffix}`">
        {{ text.roundPrefix }}{{ usage.round }}{{ text.roundSuffix }}
      </span>
      <span :title="text.inputTokens">↑<template v-if="usage.estimated">≈</template>{{ compact(usage.inputTokens) }}</span>
      <span :title="text.outputTokens">↓<template v-if="usage.estimated">≈</template>{{ compact(usage.outputTokens) }}</span>
      <span
        v-if="cacheHitRatio != null"
        :title="`${text.cacheHitRate}: ${(cacheHitRatio * 100).toFixed(1)}%`"
      >· {{ text.cache }} {{ Math.round(cacheHitRatio * 100) }}%</span>
      <span
        v-else-if="usage.cacheReadTokens != null"
        :title="`${text.cache} ${exact(usage.cacheReadTokens)} tokens (${text.cacheUnavailable})`"
      >· {{ text.cache }} {{ compact(usage.cacheReadTokens) }}</span>
      <span v-if="usage.timestamp" class="jcr-token-usage__muted">· {{ usage.timestamp }}</span>
    </button>

    <div v-show="expanded" class="jcr-token-usage__details">
      <div class="jcr-token-usage__flow jcr-token-usage__numbers">
        <span v-if="usage.model" class="jcr-token-usage__heading">{{ usage.model }}</span>
        <span>{{ text.currentRound }}</span>
        <span :title="text.inputTokens">↑<template v-if="usage.estimated">≈</template>{{ exact(usage.inputTokens) }}</span>
        <span :title="text.outputTokens">· ↓<template v-if="usage.estimated">≈</template>{{ exact(usage.outputTokens) }}</span>
        <span v-if="usage.cacheReadTokens != null">· {{ text.cache }} {{ exact(usage.cacheReadTokens) }}</span>
        <span v-if="usage.cumulativeInputTokens != null" :title="`${text.cumulative} ${text.inputTokens}`">· {{ text.cumulative }} ↑{{ exact(usage.cumulativeInputTokens) }}</span>
        <span v-if="usage.cumulativeOutputTokens != null" :title="`${text.cumulative} ${text.outputTokens}`">· ↓{{ exact(usage.cumulativeOutputTokens) }}</span>
      </div>

      <div v-if="hasPerformance" class="jcr-token-usage__flow jcr-token-usage__numbers">
        <span class="jcr-token-usage__heading">{{ text.performance }}</span>
        <span v-if="usage.firstTokenMs != null" :title="`${usage.firstTokenMs}ms`">{{ text.firstToken }} {{ duration(usage.firstTokenMs) }}</span>
        <span v-if="usage.outputTokensPerSecond != null">{{ rate(usage.outputTokensPerSecond) }} tok/s</span>
        <span v-if="usage.llmDurationMs != null" :title="`${usage.llmDurationMs}ms`">{{ text.llm }} {{ duration(usage.llmDurationMs) }}</span>
        <span v-if="usage.toolDurationMs != null" :title="`${usage.toolDurationMs}ms`">{{ text.tool }} {{ duration(usage.toolDurationMs) }}</span>
        <span v-if="usage.totalDurationMs != null" :title="`${usage.totalDurationMs}ms`">{{ text.endToEnd }} {{ duration(usage.totalDurationMs) }}</span>
        <span v-if="usage.steps != null">{{ usage.steps }}{{ text.stepsSuffix }}</span>
      </div>

      <div v-if="contextParts.length" class="jcr-token-usage__context">
        <div class="jcr-token-usage__flow">
          <span class="jcr-token-usage__heading">{{ text.contextComposition }}</span>
          <span
            v-for="part in contextParts"
            :key="part.id"
            class="jcr-token-usage__legend-item jcr-token-usage__numbers"
            :title="`${part.label}: ${exact(part.value)} ${text.contextUnit}`"
          >
            <span class="jcr-token-usage__swatch" :style="{ backgroundColor: part.color }" />
            <span>{{ part.label }} {{ compact(part.value) }}</span>
          </span>
        </div>
        <div v-if="showProgress && contextTotal > 0" class="jcr-token-usage__progress" aria-hidden="true">
          <span
            v-for="part in contextParts.filter(item => item.value > 0)"
            :key="part.id"
            :style="{ width: `${(part.value / contextTotal) * 100}%`, backgroundColor: part.color }"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.jcr-token-usage {
  display: flex;
  box-sizing: border-box;
  flex-direction: column;
  gap: .5rem;
  padding: .375rem .75rem;
  border: 1px solid color-mix(in srgb, var(--jcr-border, #e5e7eb) 72%, transparent);
  border-radius: .5rem;
  color: var(--jcr-muted, #4b5563);
  background: color-mix(in srgb, var(--jcr-bg, #fff) 72%, var(--jcr-border, #e5e7eb));
  font-size: 11px;
  line-height: 1.25rem;
  user-select: none;
}
.jcr-token-usage__summary,
.jcr-token-usage__flow {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  column-gap: .375rem;
  row-gap: .125rem;
}
.jcr-token-usage__summary {
  width: 100%;
  padding: 0;
  border: 0;
  border-radius: .25rem;
  color: inherit;
  background: transparent;
  font: inherit;
  text-align: left;
  cursor: pointer;
  transition: color .15s ease;
}
.jcr-token-usage__summary:hover { color: var(--jcr-fg, #1f2937); }
.jcr-token-usage__summary:focus-visible { outline: 2px solid rgb(96 165 250 / .6); outline-offset: 2px; }
.jcr-token-usage__chevron {
  width: .75rem;
  height: .75rem;
  flex: 0 0 auto;
  color: var(--jcr-muted, #6b7280);
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
  transition: transform .2s ease;
}
.jcr-token-usage__chevron.is-expanded { transform: rotate(90deg); }
.jcr-token-usage__details { display: flex; flex-direction: column; gap: .5rem; }
.jcr-token-usage__numbers { font-variant-numeric: tabular-nums; }
.jcr-token-usage__heading { color: var(--jcr-fg, #374151); font-weight: 500; }
.jcr-token-usage__muted { opacity: .6; font-variant-numeric: tabular-nums; }
.jcr-token-usage__context { display: flex; flex-direction: column; gap: .375rem; }
.jcr-token-usage__context .jcr-token-usage__flow { column-gap: .75rem; row-gap: .25rem; }
.jcr-token-usage__legend-item { display: inline-flex; align-items: center; gap: .375rem; white-space: nowrap; }
.jcr-token-usage__swatch { width: .625rem; height: .625rem; flex: 0 0 auto; border-radius: 2px; }
.jcr-token-usage__progress { display: flex; width: 100%; height: .375rem; overflow: hidden; border-radius: 999px; background: var(--jcr-border, #e5e7eb); }
.jcr-token-usage__progress > span { height: 100%; }
</style>
