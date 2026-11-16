<script setup>
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { planI18n as vPlanI18n, providePlanningI18n } from './i18n'
import PlanningStatusCard from './components/PlanningStatusCard.vue'
import PlanPreview from './components/PlanPreview.vue'
import PlanFileGate from './components/PlanFileGate.vue'
import ManualReviewPanel from './components/ManualReviewPanel.vue'
import { groupConfirmedHistoryByNode } from './utils/taskDisplay'

const props = defineProps({
  state: { type: Object, required: true },
  phase: { type: String, default: 'idle' },
  prompt: { type: String, default: '' },
  choiceState: { type: String, default: 'idle' },
  choiceOptions: { type: Array, default: () => [] },
  activeView: { type: String, default: 'list' },
  humanRequest: { type: Object, default: null },
  confirmedHistory: { type: Array, default: () => [] },
  apiBase: { type: String, default: '' },
  parentToken: { type: String, default: undefined },
  getConnection: { type: Function, default: undefined },
  adminBase: { type: String, default: '' },
  theme: { type: String, default: 'light', validator: value => ['light', 'dark'].includes(value) },
  locale: { type: String, default: 'zh-CN' },
  labels: { type: Object, default: () => ({}) },
  density: { type: String, default: 'comfortable', validator: value => ['comfortable', 'compact'].includes(value) },
})
const planningText = providePlanningI18n(computed(() => props.locale), computed(() => props.labels))
const emit = defineEmits(['view-change', 'choice-pick', 'replan', 'confirm', 'edit', 'cancel', 'files-change', 'human-response', 'node-select', 'update:activeView'])
const preview = ref(null)
const nowMs = ref(Date.now())
const effectiveActiveView = computed(() => props.density === 'compact' ? 'list' : props.activeView)
const hasRunningTask = computed(() => Object.values(props.state.executionState || {}).some(node => node?.status === 'running'))
const unmatchedConfirmedHistory = computed(() => {
  if (effectiveActiveView.value === 'dag') return props.confirmedHistory
  return groupConfirmedHistoryByNode(props.state.plannedTree, props.confirmedHistory).unmatched
})
let clockTimer = null

watch(hasRunningTask, running => {
  if (clockTimer) { clearInterval(clockTimer); clockTimer = null }
  nowMs.value = Date.now()
  if (running) clockTimer = setInterval(() => { nowMs.value = Date.now() }, 1000)
}, { immediate: true })

onBeforeUnmount(() => { if (clockTimer) clearInterval(clockTimer) })

const visible = () => (props.state.routeFlow?.length || props.state.validationFlow?.length || props.state.planningLogs?.length || props.state.presentation || (props.state.isPlanning && !props.state.taskBubbles?.length)) || (props.state.plannedTree && (props.state.isPreviewing || props.phase !== 'idle'))
defineExpose({
  getEditedTree: () => preview.value?.getEditedTree?.(),
  validateTree: () => preview.value?.validateTree?.() || [],
  setViewMode: value => preview.value?.setViewMode?.(value),
})
</script>

<template>
  <div v-if="visible()" v-plan-i18n="planningText" class="fpp-ui" :data-density="density">
    <PlanningStatusCard :logs="state.planningLogs || []" :is-planning="state.isPlanning"
      :planning-error="state.planningError"
      :is-previewing="state.isPreviewing" :presentation="state.presentation" :prompt-preview="prompt"
      :thinking-stream="state.thinkingStream" :thinking-steps="state.thinkingSteps || []" :route-flow="state.routeFlow || []"
      :plan-phases="state.plannedTree?.root?.children?.length || 0"
      :plan-tasks="(state.plannedTree?.root?.children || []).reduce((sum, item) => sum + (item.children?.length || 0), 0)"
      :plan-tree="state.plannedTree" :validation-flow="state.validationFlow || []" :execution-state="state.executionState || {}"
      :is-paused="state.isPaused" :tree-completed="state.treeCompleted" :human-request="humanRequest"
      :now-ms="nowMs"
      :theme="theme"
      :density="density"
      :active-view="effectiveActiveView" @view-change="emit('view-change', $event)">
      <template v-if="state.plannedTree && (state.isPreviewing || phase !== 'idle')">
        <PlanFileGate v-if="phase === 'plan' && state.plannedTree?.metadata?.input_slots?.length" :tree="state.plannedTree" :api-base="apiBase" @files-change="emit('files-change', $event)" />
        <PlanPreview ref="preview" :tree="state.plannedTree" :phase="phase" :execution-state="state.executionState || {}"
          :execution-status="state.executionStatus"
          :execution-mode="state.executionMode" :presentation="state.presentation" :user-input="prompt"
          :now-ms="nowMs"
          :theme="theme"
          :density="density"
          :confirmed-history="confirmedHistory"
          :api-base="apiBase" :admin-base="adminBase"
          :choice-state="choiceState" :choice-options="choiceOptions" :active-view="effectiveActiveView"
          @update:active-view="emit('update:activeView', $event)" @choice-pick="emit('choice-pick', $event)"
          @node-select="emit('node-select', $event)"
          @replan="emit('replan')" @confirm="emit('confirm', $event)" @edit="emit('edit')" @cancel="emit('cancel')" />
      </template>
    </PlanningStatusCard>
    <ManualReviewPanel v-if="phase !== 'plan' && unmatchedConfirmedHistory.length" :confirmed-history="unmatchedConfirmedHistory" />
  </div>
</template>
