<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Check, ChevronDown, ChevronRight, Circle, ListTree, LoaderCircle, MessageSquareWarning, X } from 'lucide-vue-next'
import type { FastPlanTaskPlanExecution, FastPlanTaskPlanNode } from 'planflow-runtime'

type ConfirmationOption = { value: string; label: string; detail?: string; tone?: 'normal' | 'danger'; action?: string; values?: Record<string, unknown> }
type ConfirmationField = { key: string; label: string; type?: string; required?: boolean; placeholder?: string; options?: Array<{ value: string; label: string }> }
type ConfirmationModel = { id?: string; kind?: string; interactionKind?: string; title?: string; question?: string; options: ConfirmationOption[]; fields?: ConfirmationField[]; submitLabel?: string; cancelLabel?: string; validationError?: string }

const props = withDefaults(defineProps<{
  plan: FastPlanTaskPlanExecution
  confirmation?: ConfirmationModel | null
  initiallyCollapsed?: boolean
  density?: 'comfortable' | 'compact'
}>(), { confirmation: null, initiallyCollapsed: false, density: 'comfortable' })
const emit = defineEmits<{ (event: 'confirmation-select', value: ConfirmationOption): void }>()

const expanded = ref(!props.initiallyCollapsed && props.plan?.status !== 'completed')
const activePanel = ref<'steps' | 'confirmation'>('steps')
const selectedIndex = ref(0)
const formValues = ref<Record<string, unknown>>({})
const inputFields = computed(() => props.confirmation?.fields || [])
const isInputConfirmation = computed(() => props.confirmation?.interactionKind === 'input')
const hasConfirmation = computed(() => !!props.confirmation && (isInputConfirmation.value || !!props.confirmation.options?.length))
const missingRequiredFields = computed(() => inputFields.value.filter(field => {
  if (!field.required) return false
  const value = formValues.value[field.key]
  return value === undefined || value === null || (typeof value === 'string' && !value.trim())
}))
const nodes = computed(() => (props.plan?.phases || []).flatMap(phase => phase.nodes || []))
const runningTaskNames = computed(() => {
  const titles = nodes.value
    .filter(node => node.status === 'running')
    .map(node => String(node.title || '').trim())
    .filter(Boolean)
  return titles
})
const progress = computed(() => {
  const total = nodes.value.length
  const success = nodes.value.filter(node => node.status === 'completed' || node.status === 'skipped').length
  const failed = nodes.value.filter(node => node.status === 'failed').length
  return { total, success, failed, successWidth: total ? success / total * 100 : 0, failedWidth: total ? failed / total * 100 : 0 }
})
const taskListTitle = computed(() => {
  if (progress.value.total > 0 && progress.value.success === progress.value.total && progress.value.failed === 0) return '全部完成'
  if (runningTaskNames.value.length) return runningTaskNames.value.join('、')
  return '任务列表'
})
const displayedTaskListTitle = computed(() => {
  if (!runningTaskNames.value.length) return taskListTitle.value
  const characters = Array.from(taskListTitle.value)
  return characters.length > 15 ? `${characters.slice(0, 15).join('')}…` : taskListTitle.value
})

watch(() => props.plan?.id, () => { expanded.value = true; activePanel.value = 'steps' })
watch(() => props.initiallyCollapsed, value => {
  if (value && !hasConfirmation.value) { expanded.value = false; activePanel.value = 'steps' }
})
watch(() => props.plan?.status, status => {
  if (hasConfirmation.value) return
  if (status === 'completed' || status === 'cancelled') expanded.value = false
  else if (status === 'executing' || status === 'paused' || status === 'failed') { expanded.value = true; activePanel.value = 'steps' }
}, { immediate: true })
watch(() => props.confirmation?.id, () => {
  if (!hasConfirmation.value) return
  selectedIndex.value = 0
  formValues.value = {}
  expanded.value = true
  activePanel.value = 'confirmation'
}, { immediate: true })
watch(hasConfirmation, value => {
  if (value) { expanded.value = true; activePanel.value = 'confirmation' }
  else if (activePanel.value === 'confirmation') activePanel.value = 'steps'
})

function nodeSequence(node: FastPlanTaskPlanNode, nodeIndex: number) {
  return node.sequence || `${nodeIndex + 1}`
}
function nodeStatusIcon(node: FastPlanTaskPlanNode) {
  if (node.status === 'running') return LoaderCircle
  if (node.status === 'completed' || node.status === 'skipped') return Check
  if (node.status === 'failed') return X
  return Circle
}
function showSteps() {
  if (activePanel.value === 'steps') expanded.value = !expanded.value
  else { activePanel.value = 'steps'; expanded.value = true }
}
function showConfirmation() {
  if (!hasConfirmation.value) return
  if (activePanel.value === 'confirmation') expanded.value = !expanded.value
  else { activePanel.value = 'confirmation'; expanded.value = true }
}
function selectOption(option: ConfirmationOption) { emit('confirmation-select', option) }
function submitInput() {
  if (missingRequiredFields.value.length) return
  selectOption({ value: 'submit', label: props.confirmation?.submitLabel || '提交并继续', action: 'submit', values: { ...formValues.value } })
}
function cancelInput() {
  selectOption({ value: 'cancel', label: props.confirmation?.cancelLabel || '取消任务', action: 'cancel' })
}
function onKeydown(event: KeyboardEvent) {
  if (!expanded.value || activePanel.value !== 'confirmation' || !hasConfirmation.value) return
  if (isInputConfirmation.value) return
  const target = event.target as HTMLElement | null
  if (['input', 'textarea', 'select'].includes(target?.tagName?.toLowerCase() || '') || target?.isContentEditable) return
  const options = props.confirmation?.options || []
  if (event.key === 'ArrowDown') { event.preventDefault(); selectedIndex.value = (selectedIndex.value + 1) % options.length }
  else if (event.key === 'ArrowUp') { event.preventDefault(); selectedIndex.value = (selectedIndex.value - 1 + options.length) % options.length }
  else if (event.key === 'Enter') { event.preventDefault(); selectOption(options[selectedIndex.value]) }
  else if (event.key === 'Escape') { event.preventDefault(); expanded.value = false }
  else if (/^[1-9]$/.test(event.key)) {
    const index = Number(event.key) - 1
    if (options[index]) { event.preventDefault(); selectedIndex.value = index; selectOption(options[index]) }
  }
}
onMounted(() => document.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => document.removeEventListener('keydown', onKeydown))
</script>

<template>
  <section class="jcr-plan-tasks" :data-attention="hasConfirmation" :data-expanded="expanded && activePanel === 'steps'" :data-density="density">
    <div class="jcr-plan-tasks__header">
      <button type="button" class="jcr-plan-tasks__summary" :aria-expanded="expanded && activePanel === 'steps'" @click="showSteps">
        <component :is="expanded && activePanel === 'steps' ? ChevronDown : ChevronRight" class="jcr-plan-tasks__toggle-icon" :size="15" />
        <span class="jcr-plan-tasks__title" :title="taskListTitle">{{ displayedTaskListTitle }}</span>
        <span v-if="progress.total" class="jcr-plan-tasks__count">{{ progress.success }}/{{ progress.total }}</span>
        <span class="jcr-plan-tasks__progress" aria-label="任务执行进度">
          <span class="jcr-plan-tasks__progress-success" :style="{ width: `${progress.successWidth}%` }"></span>
          <span class="jcr-plan-tasks__progress-failed" :style="{ left: `${progress.successWidth}%`, width: `${progress.failedWidth}%` }"></span>
        </span>
      </button>
      <div class="jcr-plan-tasks__switches" aria-label="底部信息类型">
        <button type="button" :class="{ 'is-active': activePanel === 'steps' && expanded }" title="任务步骤" @click="showSteps"><ListTree :size="16" /></button>
        <button type="button" class="jcr-plan-tasks__confirm-switch" :class="{ 'is-active': activePanel === 'confirmation' && expanded, 'has-alert': hasConfirmation }" :disabled="!hasConfirmation" title="确认表单" @click="showConfirmation">
          <MessageSquareWarning :size="16" /><i v-if="hasConfirmation"></i>
        </button>
      </div>
    </div>

    <div v-show="expanded && activePanel === 'steps'" class="jcr-plan-tasks__body">
      <div v-for="(node, nodeIndex) in nodes" :key="node.id" class="jcr-plan-tasks__node" :data-status="node.status">
        <component :is="nodeStatusIcon(node)" class="jcr-plan-tasks__status-icon" :size="14" :stroke-width="2.25" aria-hidden="true" />
        <span class="jcr-plan-tasks__sequence">{{ nodeSequence(node, nodeIndex) }}.</span>
        <span class="jcr-plan-tasks__node-title">{{ node.title }}</span>
      </div>
    </div>

    <div v-if="hasConfirmation" v-show="expanded && activePanel === 'confirmation'" class="jcr-plan-confirm" role="group" aria-label="等待人工确认">
      <strong>{{ confirmation?.title || '需要您的确认' }}</strong>
      <p v-if="confirmation?.question">{{ confirmation.question }}</p>
      <form v-if="isInputConfirmation" class="jcr-plan-confirm__form" @submit.prevent="submitInput">
        <label v-for="field in inputFields" :key="field.key" class="jcr-plan-confirm__field">
          <span>{{ field.label }}<i v-if="field.required">*</i></span>
          <textarea v-if="field.type === 'textarea'" v-model="formValues[field.key]" :placeholder="field.placeholder" rows="2"></textarea>
          <select v-else-if="field.type === 'select'" v-model="formValues[field.key]">
            <option value="" disabled>请选择</option>
            <option v-for="option in field.options || []" :key="option.value" :value="option.value">{{ option.label }}</option>
          </select>
          <input v-else v-model="formValues[field.key]" :type="['date', 'number'].includes(field.type || '') ? field.type : 'text'" :placeholder="field.placeholder" />
        </label>
        <p v-if="confirmation?.validationError" class="jcr-plan-confirm__error">{{ confirmation.validationError }}</p>
        <p v-else-if="missingRequiredFields.length" class="jcr-plan-confirm__required">请填写：{{ missingRequiredFields.map(field => field.label).join('、') }}</p>
        <div class="jcr-plan-confirm__actions">
          <button type="button" class="is-danger" @click="cancelInput">{{ confirmation?.cancelLabel || '取消任务' }}</button>
          <button type="submit" class="is-primary" :disabled="missingRequiredFields.length > 0">{{ confirmation?.submitLabel || '提交并继续' }}</button>
        </div>
      </form>
      <div v-else class="jcr-plan-confirm__options">
        <button v-for="(option, index) in confirmation?.options" :key="option.value" type="button"
          :class="{ 'is-selected': selectedIndex === index, 'is-danger': option.tone === 'danger' }"
          @mouseenter="selectedIndex = index" @focus="selectedIndex = index" @click="selectOption(option)">
          <span>{{ index + 1 }}.</span><b>{{ option.label }}</b><small v-if="option.detail">{{ option.detail }}</small>
        </button>
      </div>
      <small v-if="!isInputConfirmation" class="jcr-plan-confirm__hint">↑↓ 选择 · Enter 确认 · Esc 收起</small>
    </div>
  </section>
</template>

<style scoped>
.jcr-plan-tasks,.jcr-plan-tasks * { box-sizing:border-box; }
.jcr-plan-tasks { width:100%; min-width:0; color:var(--jcr-fg,#1f2937); background:transparent; font-size:inherit; }
.jcr-plan-tasks__header { display:flex; width:100%; min-width:0; align-items:center; gap:8px; }
.jcr-plan-tasks__summary { display:flex; min-width:0; flex:1; align-items:center; gap:10px; padding:7px 0; border:0; color:inherit; background:transparent; cursor:pointer; }
.jcr-plan-tasks__toggle-icon { flex:0 0 auto; }
.jcr-plan-tasks__title { min-width:0; max-width:42%; flex:0 1 auto; overflow:hidden; font-weight:600; text-overflow:ellipsis; white-space:nowrap; }
.jcr-plan-tasks__count { flex:0 0 auto; color:var(--jcr-muted,#6b7280); }
.jcr-plan-tasks__progress { position:relative; width:50%; max-width:50%; height:4px; min-width:40px; flex:0 1 50%; overflow:hidden; border-radius:999px; background:color-mix(in srgb,var(--jcr-muted,#6b7280) 16%,transparent); }
.jcr-plan-tasks__progress-success,.jcr-plan-tasks__progress-failed { position:absolute; top:0; bottom:0; transition:width .25s ease,left .25s ease; }
.jcr-plan-tasks__progress-success { left:0; background:#22c55e; }.jcr-plan-tasks__progress-failed { background:#ef4444; }
.jcr-plan-tasks__switches { display:flex; flex:0 0 auto; gap:2px; }
.jcr-plan-tasks__switches button { position:relative; display:inline-flex; width:28px; height:28px; align-items:center; justify-content:center; padding:0; border:0; border-radius:6px; color:var(--jcr-muted,#6b7280); background:transparent; cursor:pointer; }
.jcr-plan-tasks__switches button:hover,.jcr-plan-tasks__switches button.is-active { color:var(--jcr-fg,#1f2937); background:color-mix(in srgb,var(--jcr-border,#e5e7eb) 55%,transparent); }
.jcr-plan-tasks__switches button:disabled { opacity:.35; cursor:default; background:transparent; }
.jcr-plan-tasks__confirm-switch.has-alert { color:#dc2626; opacity:1; }
.jcr-plan-tasks__confirm-switch i { position:absolute; top:3px; right:3px; width:6px; height:6px; border-radius:999px; background:#dc2626; }
.jcr-plan-tasks__body { display:grid; gap:1px; max-height:220px; padding:7px 0 8px; overflow-y:auto; border-top:1px solid color-mix(in srgb,var(--jcr-border,#e5e7eb) 65%,transparent); }
.jcr-plan-tasks__node { display:flex; min-width:0; min-height:28px; align-items:center; gap:8px; padding:4px 7px; border-radius:7px; }
.jcr-plan-tasks__status-icon { width:14px; height:14px; flex:0 0 auto; color:#cbd5e1; }
.jcr-plan-tasks__node[data-status="running"] { background:color-mix(in srgb,var(--jcr-primary,#6366f1) 7%,transparent); }
.jcr-plan-tasks__node[data-status="running"] .jcr-plan-tasks__status-icon { color:var(--jcr-primary,#6366f1); animation:jcr-plan-task-spin .8s linear infinite; }
.jcr-plan-tasks__node[data-status="completed"] .jcr-plan-tasks__status-icon { color:#16a34a; }
.jcr-plan-tasks__node[data-status="failed"] .jcr-plan-tasks__status-icon { color:#dc2626; }
.jcr-plan-tasks__node[data-status="skipped"] .jcr-plan-tasks__status-icon { color:#94a3b8; }
.jcr-plan-tasks__sequence { flex:0 0 auto; color:var(--jcr-muted,#6b7280); font-family:ui-monospace,SFMono-Regular,Menlo,monospace; }
.jcr-plan-tasks__node-title { min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.jcr-plan-tasks__node[data-status="running"] .jcr-plan-tasks__sequence,
.jcr-plan-tasks__node[data-status="running"] .jcr-plan-tasks__node-title { color:transparent; background:linear-gradient(90deg,#334155 0%,#6366f1 35%,#93c5fd 50%,#6366f1 65%,#334155 100%); background-size:220% 100%; background-clip:text; -webkit-background-clip:text; animation:jcr-plan-task-text 1.8s linear infinite; }
.jcr-plan-tasks__node[data-status="running"] .jcr-plan-tasks__node-title { font-weight:600; }
.jcr-plan-tasks__node[data-status="failed"] .jcr-plan-tasks__node-title { color:#dc2626; }
.jcr-plan-confirm { margin-top:4px; padding:11px 12px; border:1px solid #ef4444; border-radius:9px; background:color-mix(in srgb,#ef4444 3%,var(--jcr-bg,#fff)); box-shadow:0 0 0 2px rgb(239 68 68 / .08); }
.jcr-plan-confirm > strong { display:block; color:#b91c1c; }.jcr-plan-confirm > p { margin:4px 0 8px; color:var(--jcr-muted,#6b7280); }
.jcr-plan-confirm__options { display:grid; gap:2px; }
.jcr-plan-confirm__options button { display:grid; grid-template-columns:20px auto minmax(0,1fr); align-items:center; gap:4px; width:100%; padding:6px 7px; border:0; border-radius:6px; color:inherit; background:transparent; text-align:left; cursor:pointer; }
.jcr-plan-confirm__options button.is-selected { background:color-mix(in srgb,var(--jcr-primary,#2563eb) 9%,transparent); }
.jcr-plan-confirm__options button.is-danger b { color:#dc2626; }.jcr-plan-confirm__options span { color:var(--jcr-muted,#6b7280); font-family:ui-monospace,SFMono-Regular,Menlo,monospace; }
.jcr-plan-confirm__options b { font-weight:600; }.jcr-plan-confirm__options small { min-width:0; overflow:hidden; color:var(--jcr-muted,#6b7280); text-overflow:ellipsis; white-space:nowrap; }
.jcr-plan-confirm__hint { display:block; margin-top:7px; color:var(--jcr-muted,#6b7280); }
.jcr-plan-confirm__form { display:grid; gap:9px; margin-top:9px; }
.jcr-plan-confirm__field { display:grid; gap:4px; }.jcr-plan-confirm__field > span { font-weight:600; }.jcr-plan-confirm__field i { margin-left:2px; color:#dc2626; font-style:normal; }
.jcr-plan-confirm__field input,.jcr-plan-confirm__field textarea,.jcr-plan-confirm__field select { width:100%; padding:7px 9px; border:1px solid var(--jcr-border,#d1d5db); border-radius:7px; color:var(--jcr-fg,#1f2937); background:var(--jcr-bg,#fff); outline:none; resize:vertical; }
.jcr-plan-confirm__field input:focus,.jcr-plan-confirm__field textarea:focus,.jcr-plan-confirm__field select:focus { border-color:var(--jcr-primary,#2563eb); box-shadow:0 0 0 2px color-mix(in srgb,var(--jcr-primary,#2563eb) 14%,transparent); }
.jcr-plan-confirm__required,.jcr-plan-confirm__error { margin:0; color:#b45309 !important; }.jcr-plan-confirm__error { color:#dc2626 !important; }
.jcr-plan-confirm__actions { display:flex; justify-content:flex-end; gap:7px; }.jcr-plan-confirm__actions button { padding:6px 10px; border:1px solid var(--jcr-border,#d1d5db); border-radius:7px; background:transparent; cursor:pointer; }
.jcr-plan-confirm__actions button.is-primary { border-color:var(--jcr-primary,#2563eb); color:#fff; background:var(--jcr-primary,#2563eb); }.jcr-plan-confirm__actions button.is-primary:disabled { opacity:.45; cursor:not-allowed; }.jcr-plan-confirm__actions button.is-danger { color:#dc2626; }
@keyframes jcr-plan-task-spin { to { transform:rotate(360deg); } }
@keyframes jcr-plan-task-text { to { background-position:-220% 0; } }
@media (prefers-reduced-motion:reduce) {
  .jcr-plan-tasks__node[data-status="running"] .jcr-plan-tasks__status-icon,
  .jcr-plan-tasks__node[data-status="running"] .jcr-plan-tasks__sequence,
  .jcr-plan-tasks__node[data-status="running"] .jcr-plan-tasks__node-title { animation:none; }
}
@media (max-width:640px) { .jcr-plan-confirm__options button { grid-template-columns:18px minmax(0,1fr); }.jcr-plan-confirm__options small { grid-column:2; white-space:normal; } }
</style>
