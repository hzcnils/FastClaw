<script setup>
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { Check, ChevronDown, ChevronRight, Circle, LoaderCircle, Minus, X } from 'lucide-vue-next'
import { ChatMarkdownRenderer, ChatModelIcon } from 'chat-renderer'
import { userFacingTaskError } from 'planflow-runtime'
import { cleanLine, collectExecutionNames, extractArgs, extractCompletedNodeOutput, nodeTokens, resolveNodeElapsed } from '../../utils/blockParser'
import { resolveNodeModel } from '../../utils/nodeModel'
import { formatDuration } from '../../utils/duration'

const props = defineProps({
  state: { type: Object, default: () => ({ status: 'pending', logs: [] }) },
  treeNode: { type: Object, default: null },
  seq: { type: String, default: '' },
  nowMs: { type: Number, default: null },
  isLastTask: { type: Boolean, default: false },
  theme: { type: String, default: 'light', validator: value => ['light', 'dark'].includes(value) },
  density: { type: String, default: 'comfortable', validator: value => ['comfortable', 'compact'].includes(value) },
})

const outputBox = ref(null)
const outputExpanded = ref(false)
const followOutput = ref(true)
const localNowMs = ref(Date.now())
let localClockTimer = null
const status = computed(() => props.state?.status || 'pending')
const nodeType = computed(() => String(props.treeNode?.node_type?.type || props.state?.nodeType || '').toLowerCase())
const isLlm = computed(() => nodeType.value.includes('llm') || nodeType.value.includes('agent'))
const isStreamingLlm = computed(() => nodeType.value.includes('llm') || nodeType.value.includes('model'))
const title = computed(() => props.treeNode?.name || props.state?.name || '未命名任务')
const config = computed(() => props.treeNode?.node_type?.config || {})
const model = computed(() => resolveNodeModel(props.treeNode, props.state))
const names = computed(() => collectExecutionNames(props.state?.logs || [], props.treeNode))
const executionName = computed(() => names.value.join('、') || (isLlm.value ? 'LLM' : '任务执行器'))
const firstToolCall = computed(() => (props.state?.logs || []).find(log => log?.type === 'tool_call'))
const executionKind = computed(() => {
  const actualName = String(firstToolCall.value?.toolName || names.value[0] || '').toLowerCase()
  const rawType = nodeType.value
  const configKind = String(config.value.kind || config.value.type || config.value.executor_type || '').toLowerCase()
  if (actualName) {
    if (actualName.includes(':') || /(^|[-_])mcp($|[-_])/.test(actualName) || configKind.includes('mcp')) return 'MCP'
    if (configKind.includes('skill') || rawType.includes('skill')) return '技能'
    return '接口'
  }
  if (rawType.includes('agent')) return 'Agent'  // 执行者类型（非工具分类，与 counts 口径无关）
  if (rawType.includes('mcp') || configKind.includes('mcp')) return 'MCP'
  if (rawType.includes('skill') || configKind.includes('skill')) return '技能'
  if (rawType.includes('tool') || rawType.includes('http')) return '接口'
  if (rawType.includes('llm') || rawType.includes('model')) return '模型'
  return '执行器'
})
const argumentText = computed(() => {
  // 优先取结构化入参（tool_call_started 事件 arguments 字段），再回退到调用文本解析。
  const actual = firstToolCall.value?.arguments
    ? extractArgs(typeof firstToolCall.value.arguments === 'string' ? firstToolCall.value.arguments : JSON.stringify(firstToolCall.value.arguments))
    : extractArgs(firstToolCall.value?.content || '')
  if (actual) return cleanLine(actual, 100)
  // 尚未收到调用事件时使用 Tool/MCP 配置中的真实参数。
  const value = config.value.arguments ?? config.value.args ?? config.value.input
  if (value != null && value !== '') {
    const configured = extractArgs(typeof value === 'string' ? value : JSON.stringify(value))
    if (configured) return cleanLine(configured, 100)
  }
  // 最后才回退到任务树中的用户可读输入说明。
  const displayInput = props.treeNode?.metadata?.display_input_name
  if (displayInput != null && String(displayInput).trim()) return cleanLine(String(displayInput), 100)
  return ''
})
const tokens = computed(() => nodeTokens(props.state || {}, JSON.stringify(config.value || {})))
const tokenText = computed(() => {
  return `↑${compactNumber(tokens.value.input)}  ↓${compactNumber(tokens.value.output)} tokens`
})
const hasExternalClock = computed(() => Number.isFinite(props.nowMs))
const effectiveNowMs = computed(() => hasExternalClock.value ? props.nowMs : localNowMs.value)
const elapsed = computed(() => resolveNodeElapsed(props.state || {}, effectiveNowMs.value))
const durationText = computed(() => {
  if (elapsed.value.durationMs == null) return ''
  return formatDuration(elapsed.value.durationMs, { live: elapsed.value.live })
})
const streamingContent = computed(() => isStreamingLlm.value ? String(props.state?.streamContent || '') : '')
const completedOutput = computed(() => extractCompletedNodeOutput(props.state || {}))
const detailContent = computed(() => {
  if (isStreamingLlm.value && streamingContent.value) return streamingContent.value
  return completedOutput.value
})
const degradedReasons = computed(() => {
  const values = Array.isArray(props.state?.degraded) ? props.state.degraded : []
  return [...new Set(values.map(value => String(value || '').trim()).filter(Boolean))]
})
const errorText = computed(() => userFacingTaskError(props.state?.error || props.state?.errorMessage || ''))
const toolName = computed(() => names.value[0] || (isLlm.value ? 'llm' : ''))
const canToggleDetails = computed(() => !!detailContent.value || degradedReasons.value.length > 0)
const autoExpandLastLlm = computed(() => props.isLastTask && isStreamingLlm.value)
const statusIcon = computed(() => ({
  running: LoaderCircle,
  completed: Check,
  failed: X,
  skipped: Minus,
  pending: Circle,
})[status.value] || Circle)

watch(status, value => {
  if (value === 'running') {
    outputExpanded.value = autoExpandLastLlm.value
    followOutput.value = true
  } else outputExpanded.value = false
}, { immediate: true })

watch(autoExpandLastLlm, value => {
  if (status.value === 'running' && value) outputExpanded.value = true
})

watch([status, hasExternalClock], ([value, external]) => {
  if (localClockTimer) { clearInterval(localClockTimer); localClockTimer = null }
  if (value === 'running' && !external) {
    localNowMs.value = Date.now()
    localClockTimer = setInterval(() => { localNowMs.value = Date.now() }, 1000)
  }
}, { immediate: true })

watch(detailContent, async () => {
  await nextTick()
  if (outputBox.value && followOutput.value) outputBox.value.scrollTop = outputBox.value.scrollHeight
})

onBeforeUnmount(() => {
  if (localClockTimer) clearInterval(localClockTimer)
})

function toggleOutput() {
  outputExpanded.value = !outputExpanded.value
  if (outputExpanded.value) {
    followOutput.value = true
    nextTick(() => { if (outputBox.value) outputBox.value.scrollTop = outputBox.value.scrollHeight })
  }
}

function handleOutputScroll() {
  const element = outputBox.value
  if (!element) return
  followOutput.value = element.scrollHeight - element.scrollTop - element.clientHeight < 20
}

function compactNumber(value) {
  const number = Number(value || 0)
  if (number >= 1000000) return `${(number / 1000000).toFixed(number >= 10000000 ? 0 : 1)}m`
  if (number >= 1000) return `${(number / 1000).toFixed(number >= 10000 ? 0 : 1)}k`
  return String(Math.round(number))
}

</script>

<template>
  <article class="fp-task" :data-status="status" :data-density="density">
    <div class="fp-task__title-row">
      <span class="fp-task__status" :data-status="status" aria-hidden="true">
        <component :is="statusIcon" :size="13" :stroke-width="2.25" />
      </span>
      <ChatModelIcon v-if="density === 'comfortable' && isStreamingLlm" class="fpp-model-icon" :name="model.name" :model-id="model.id" :size="14" />
      <span class="fp-task__prefix">执行任务{{ seq }}（<strong>{{ title }}</strong>）</span>
      <span v-if="density === 'compact' && toolName" class="fp-task__tool">{{ toolName }}</span>
      <button v-if="density === 'compact' && canToggleDetails" type="button" class="fp-task__toggle" :aria-expanded="outputExpanded" :title="outputExpanded ? '折叠详情' : '展开详情'" @click="toggleOutput">
        <component :is="outputExpanded ? ChevronDown : ChevronRight" :size="15" />
      </button>
    </div>
    <p v-if="errorText" class="fp-task__error">{{ errorText }}</p>
    <div v-if="density === 'comfortable' || (canToggleDetails && outputExpanded)" class="fp-task__execution-row">
      <span class="fp-task__executor">执行{{ executionKind }}：{{ executionName }}</span>
      <span v-if="argumentText" class="fp-task__args" :title="argumentText">{{ argumentText }}</span>
      <span v-if="durationText" class="fp-task__duration">{{ durationText }}</span>
      <span class="fp-task__tokens">{{ tokenText }}</span>
      <span v-if="density === 'comfortable'" class="fp-task__spacer"></span>
      <button v-if="density === 'comfortable' && canToggleDetails" type="button" class="fp-task__toggle" :aria-expanded="outputExpanded" :title="outputExpanded ? '折叠详情' : '展开详情'" @click="toggleOutput">
        <component :is="outputExpanded ? ChevronDown : ChevronRight" :size="15" />
      </button>
    </div>
    <div v-if="canToggleDetails && outputExpanded" ref="outputBox" class="fp-task__details" @scroll.passive="handleOutputScroll">
      <div v-if="degradedReasons.length" class="fp-task__degraded">
        <div v-for="reason in degradedReasons" :key="reason" class="fp-task__degraded-item">⚠ {{ reason }}</div>
      </div>
      <section v-if="detailContent" class="fp-task__detail-section">
        <ChatMarkdownRenderer class="fp-task__detail-content" :content="detailContent" :max-chars="100000" :theme="theme" />
      </section>
    </div>
  </article>
</template>

<style scoped>
.fp-task { min-width:0; padding:8px 8px 9px; border:1px solid color-mix(in srgb,var(--jcr-border,#e5e7eb) 80%,transparent); border-radius:8px; color:var(--jcr-fg,#1f2937); font-size:inherit; line-height:1.55; }
.fp-task + .fp-task { margin-top:6px; }
.fp-task__title-row,.fp-task__execution-row { display:flex; min-width:0; align-items:center; }
.fp-task__title-row { gap:8px; }
.fp-task__prefix { min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; font-size:11px; }
.fp-task__tool { flex-shrink:0; color:#94a3b8; font-size:0.85em; }
.fp-task__error { display:block; margin:2px 0 0 22px; color:#dc2626; font-size:inherit; line-height:1.55; white-space:normal; }
.fp-task__status { display:inline-flex; width:14px; height:14px; flex:0 0 14px; align-items:center; justify-content:center; color:#cbd5e1; }
.fp-task__status[data-status="running"] { color:#6366f1; animation:fp-task-spin .8s linear infinite; }
.fp-task__status[data-status="completed"] { color:#16a34a; }
.fp-task__status[data-status="failed"] { color:#dc2626; }
.fp-task__status[data-status="skipped"] { color:#94a3b8; }
.fp-task__prefix strong { font-weight:650; }
.fp-task__execution-row { gap:9px; margin-top:5px; padding-left:16px; color:var(--jcr-muted,#6b7280); white-space:nowrap; }
.fp-task__executor { flex:0 1 auto; overflow:hidden; color:color-mix(in srgb,var(--jcr-fg,#1f2937) 76%,var(--jcr-muted,#6b7280)); text-overflow:ellipsis; }
.fp-task__args { min-width:0; overflow:hidden; text-overflow:ellipsis; }
.fp-task__args { max-width:26%; }
.fp-task__tokens,.fp-task__duration { flex:0 0 auto; font-family:ui-monospace,SFMono-Regular,Menlo,monospace; font-size:9px; }
.fp-task__tokens { color:#6366f1; }
.fp-task__spacer { flex:1; }
.fp-task__toggle { display:inline-flex; width:22px; height:22px; flex:0 0 22px; align-items:center; justify-content:center; margin-left:auto; padding:0; border:0; border-radius:5px; color:var(--jcr-muted,#6b7280); background:transparent; cursor:pointer; }
.fp-task__toggle:hover { color:var(--jcr-fg,#1f2937); background:color-mix(in srgb,var(--jcr-border,#e5e7eb) 55%,transparent); }
.fp-task__details { max-height:155px; margin:7px 0 0 16px; padding:8px 10px; overflow-y:auto; border:1px solid color-mix(in srgb,var(--jcr-border,#e5e7eb) 78%,transparent); border-radius:7px; color:var(--jcr-muted,#6b7280); background:color-mix(in srgb,var(--jcr-muted,#6b7280) 4%,var(--jcr-bg,#fff)); font-size:10px; overscroll-behavior:contain; }
.fp-task__detail-section + .fp-task__detail-section { margin-top:8px; padding-top:8px; border-top:1px solid color-mix(in srgb,var(--jcr-border,#e5e7eb) 65%,transparent); }
.fp-task__degraded { margin-bottom:8px; padding:6px 8px; border:1px solid #f59e0b; border-radius:6px; color:#92400e; background:color-mix(in srgb,#fef3c7 55%,var(--jcr-bg,#fff)); font-size:10px; line-height:1.5; }
.fp-task__degraded-item + .fp-task__degraded-item { margin-top:3px; }
.fp-task__detail-content { overflow-wrap:anywhere; }
.fp-task__detail-content :deep(.jcr-markdown > div > :first-child) { margin-top:0; }.fp-task__detail-content :deep(.jcr-markdown > div > :last-child) { margin-bottom:0; }
.fp-task[data-density="compact"] { padding:6px 0; border:0; border-radius:0; }
.fp-task[data-density="compact"] + .fp-task { margin-top:2px; }
.fp-task[data-density="compact"] .fp-task__title-row,
.fp-task[data-density="compact"] .fp-task__execution-row { align-items:flex-start; }
.fp-task[data-density="compact"] .fp-task__prefix { flex:1; font-size:inherit; }
@keyframes fp-task-spin { to { transform:rotate(360deg); } }
.fp-task[data-density="compact"] .fp-task__execution-row { flex-wrap:wrap; white-space:normal; }
.fp-task[data-density="compact"] .fp-task__args { max-width:100%; }
@media (prefers-reduced-motion:reduce) { .fp-task__status { animation:none !important; } }
</style>
