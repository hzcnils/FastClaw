<script setup>
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { AlertCircle, Check, Circle, Loader2, Minus, Pause, UserRound } from 'lucide-vue-next'
import { buildPlanningChecklist } from '../utils/planningChecklistAdapter'
import { formatDuration } from '../utils/duration'

const props = defineProps({
  prompt: { type: String, default: '' },
  planTree: { type: Object, default: null },
  executionState: { type: Object, default: () => ({}) },
  validationFlow: { type: Array, default: () => [] },
  logs: { type: Array, default: () => [] },
  presentation: { type: Object, default: null },
  isPlanning: { type: Boolean, default: false },
  isPreviewing: { type: Boolean, default: false },
  isPaused: { type: Boolean, default: false },
  treeCompleted: { type: Boolean, default: false },
  humanRequest: { type: Object, default: null },
  planDurationMs: { type: Number, default: null },
  planStartedAt: { type: Number, default: null },
})

const clock = ref(Date.now())
let timer = null
onMounted(() => { timer = window.setInterval(() => { clock.value = Date.now() }, 1000) })
onUnmounted(() => { if (timer) window.clearInterval(timer) })

const checklist = computed(() => buildPlanningChecklist({
  prompt: props.prompt,
  planTree: props.planTree,
  executionState: props.executionState,
  validationFlow: props.validationFlow,
  logs: props.logs,
  presentation: props.presentation,
  isPlanning: props.isPlanning,
  isPreviewing: props.isPreviewing,
  isPaused: props.isPaused,
  treeCompleted: props.treeCompleted,
  humanRequest: props.humanRequest,
  planDurationMs: props.planDurationMs,
  planStartedAt: props.planStartedAt,
}))

function elapsed(row) {
  if (Number.isFinite(Number(row.durationMs)) && Number(row.durationMs) > 0) return Number(row.durationMs)
  if (row.startedAt && ['running', 'paused', 'waiting_human'].includes(row.status)) {
    return Math.max(0, clock.value - Number(row.startedAt))
  }
  return null
}

function trailingText(row) {
  const duration = elapsed(row)
  if (duration !== null) return formatDuration(duration, { live: !(Number(row.durationMs) > 0) })
  if (row.status === 'pending') return '等待中'
  if (row.status === 'failed') return '失败'
  if (row.status === 'skipped') return '已跳过'
  if (row.status === 'paused') return '已暂停'
  if (row.status === 'waiting_human') return '等待确认'
  return row.status === 'running' ? '进行中' : ''
}

function rowClass(status) {
  return {
    completed: 'is-completed', running: 'is-running', failed: 'is-failed',
    skipped: 'is-skipped', paused: 'is-paused', waiting_human: 'is-waiting', pending: 'is-pending',
  }[status] || 'is-pending'
}
</script>

<template>
  <section class="fp-checklist" :data-mode="checklist.mode">
    <p v-if="prompt" class="fp-checklist__prompt">{{ prompt }}</p>

    <div class="fp-checklist__rows">
      <div v-for="(row, index) in checklist.rows" :key="row.id"
        class="fp-checklist__row" :class="[rowClass(row.status), {
          'is-task': row.kind === 'task',
          'is-task-start': index === checklist.planRows.length && checklist.taskRows.length,
        }]">
        <span class="fp-checklist__icon" aria-hidden="true">
          <Check v-if="row.status === 'completed'" :size="15" />
          <Loader2 v-else-if="row.status === 'running'" :size="14" class="fp-checklist__spin" />
          <AlertCircle v-else-if="row.status === 'failed'" :size="14" />
          <Minus v-else-if="row.status === 'skipped'" :size="14" />
          <Pause v-else-if="row.status === 'paused'" :size="14" />
          <UserRound v-else-if="row.status === 'waiting_human'" :size="14" />
          <Circle v-else :size="10" />
        </span>
        <div class="fp-checklist__copy">
          <div class="fp-checklist__title">{{ row.title }}</div>
          <div v-if="row.detail" class="fp-checklist__detail">{{ row.detail }}</div>
        </div>
        <span class="fp-checklist__time">{{ trailingText(row) }}</span>
      </div>
    </div>

    <div v-if="checklist.rows.length" class="fp-checklist__status">{{ checklist.footer }}</div>
  </section>
</template>

<style scoped>
.fp-checklist { min-width: 0; color: var(--jcr-fg, #1f2937); }
.fp-checklist__prompt { margin: 0 0 18px; color: color-mix(in srgb,var(--jcr-fg,#1f2937) 72%,var(--jcr-muted,#6b7280)); font-size: 13px; line-height: 1.7; overflow-wrap: anywhere; }
.fp-checklist__rows { display: grid; gap: 10px; }
.fp-checklist__row { position: relative; display: grid; grid-template-columns: 18px minmax(0,1fr) auto; align-items: start; column-gap: 7px; min-width: 0; padding: 0; }
.fp-checklist__row.is-task-start { margin-top: 5px; padding-top: 5px; }
.fp-checklist__row.is-task + .fp-checklist__row.is-task { margin-top: 1px; padding-top: 11px; }
.fp-checklist__row.is-task + .fp-checklist__row.is-task::before { position: absolute; top: 0; right: 0; left: 25px; border-top: 1px solid color-mix(in srgb,var(--jcr-border,#e5e7eb) 58%,transparent); content: ''; }
.fp-checklist__icon { display: inline-flex; width: 18px; height: 22px; align-items: center; justify-content: center; color: var(--jcr-muted,#6b7280); }
.fp-checklist__row.is-completed .fp-checklist__icon { color: #15803d; }
.fp-checklist__row.is-running .fp-checklist__icon { color: #4f46e5; }
.fp-checklist__row.is-failed .fp-checklist__icon { color: #dc2626; }
.fp-checklist__row.is-paused .fp-checklist__icon,.fp-checklist__row.is-waiting .fp-checklist__icon { color: #d97706; }
.fp-checklist__row.is-skipped { opacity: .72; }
.fp-checklist__copy { min-width: 0; }
.fp-checklist__title { min-width: 0; color: var(--jcr-fg,#1f2937); font-size: 13px; line-height: 22px; overflow-wrap: anywhere; }
.fp-checklist__detail { margin-top: 1px; color: var(--jcr-muted,#6b7280); font-size: 12px; line-height: 1.65; overflow-wrap: anywhere; }
.fp-checklist__time { min-width: 52px; padding-left: 12px; color: var(--jcr-muted,#6b7280); font: 11px/22px ui-monospace,SFMono-Regular,Menlo,monospace; text-align: right; white-space: nowrap; }
.fp-checklist__status { margin-top: 14px; color: var(--jcr-muted,#6b7280); font-size: 11px; }
.fp-checklist__spin { animation: fp-checklist-spin .9s linear infinite; }
@keyframes fp-checklist-spin { to { transform: rotate(360deg); } }
@media (max-width:640px) {
  .fp-checklist__prompt { margin-bottom: 14px; }
  .fp-checklist__time { min-width: 42px; padding-left: 6px; }
}
@media (prefers-reduced-motion:reduce) { .fp-checklist__spin { animation: none; } }
</style>
