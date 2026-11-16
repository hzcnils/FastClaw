<script setup lang="ts">
/**
 * 远程规划过程卡：与 App 对齐，接上 PlanPreview（清单 / 时间轴 / DAG）。
 * StatusCard 的 tabs 只 emit view-change，必须本地持有 activeView 才会切换。
 */
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { FastPlanPlanningStatusCard, FastPlanPlanPreview } from 'planflow-planning-ui'
import { remotePlanLiveBySession } from './remotePlanRuntime'

const props = defineProps<{
  runtime?: Record<string, any> | null
  sessionId?: string | null
}>()

const state = computed(() => {
  const sid = String(props.sessionId || '').trim()
  if (sid && remotePlanLiveBySession[sid]) return remotePlanLiveBySession[sid]
  return props.runtime || {}
})

const nowMs = ref(Date.now())
const activeView = ref<'list' | 'timeline' | 'dag'>('list')
let clockTimer: ReturnType<typeof setInterval> | null = null

const hasContent = computed(() => {
  const s = state.value
  return !!(
    s.planningLogs?.length
    || s.routeFlow?.length
    || s.validationFlow?.length
    || s.plannedTree
    || s.planTree
    || s.isPlanning
    || s.planningError
    || s.thinkingStream
  )
})

const planTree = computed(() => state.value.plannedTree || state.value.planTree || null)

const previewPhase = computed(() => {
  const s = state.value
  if (s.treeCompleted || s.executionStatus === 'completed') return 'finished'
  if (s.isPreviewing && !Object.keys(s.executionState || {}).length) return 'plan'
  if (planTree.value) return 'executing'
  return 'idle'
})

const showPlanPreview = computed(() =>
  !!(planTree.value && (state.value.isPreviewing || previewPhase.value !== 'idle')),
)

const hasRunningTask = computed(() =>
  Object.values(state.value.executionState || {}).some((node: any) => node?.status === 'running'),
)

watch(hasRunningTask, (running) => {
  if (clockTimer) {
    clearInterval(clockTimer)
    clockTimer = null
  }
  nowMs.value = Date.now()
  if (running) clockTimer = setInterval(() => { nowMs.value = Date.now() }, 1000)
}, { immediate: true })

onBeforeUnmount(() => {
  if (clockTimer) clearInterval(clockTimer)
})

function onViewChange(view: string) {
  if (view === 'list' || view === 'timeline' || view === 'dag') activeView.value = view
}
</script>

<template>
  <div v-if="hasContent" class="fp-thinking-card">
    <FastPlanPlanningStatusCard
      density="comfortable"
      :logs="state.planningLogs || []"
      :is-planning="!!state.isPlanning"
      :planning-error="state.planningError || null"
      :is-previewing="!!state.isPreviewing"
      :route-flow="state.routeFlow || []"
      :validation-flow="state.validationFlow || []"
      :execution-state="state.executionState || {}"
      :plan-tree="planTree"
      :thinking-stream="state.thinkingStream || ''"
      :tree-completed="!!state.treeCompleted"
      :now-ms="nowMs"
      :active-view="activeView"
      @view-change="onViewChange"
    >
      <FastPlanPlanPreview
        v-if="showPlanPreview && planTree"
        :tree="planTree"
        :phase="previewPhase"
        :execution-state="state.executionState || {}"
        :execution-status="state.executionStatus || null"
        :execution-mode="state.executionMode || 'auto'"
        :presentation="state.presentation || null"
        :user-input="state.lastPrompt || state.prompt || ''"
        :now-ms="nowMs"
        density="comfortable"
        choice-state="confirmed"
        :active-view="activeView"
        @update:active-view="onViewChange"
      />
    </FastPlanPlanningStatusCard>
  </div>
</template>

<style scoped>
.fp-thinking-card {
  width: 100%;
  min-width: 0;
}
</style>
