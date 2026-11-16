<script setup lang="ts">
import { computed, ref, watch, nextTick } from 'vue'
import {
  CheckCircle2, Loader2, AlertCircle, ChevronDown, ChevronRight,
  Terminal, Play, Check, Pause, SkipForward, RefreshCw, Ban
} from 'lucide-vue-next'
import TaskItem from './TaskItem.vue'
import HumanCard from './HumanCard.vue'
import type { TaskTree, TaskTreeNode } from './treeToDAG'
import type { ExecutionNodeState, HumanRequest } from './session'

const props = defineProps<{
  tree: TaskTree
  executionState: Record<string, ExecutionNodeState>
  currentHumanRequest: HumanRequest | null
  isPaused: boolean
  executionMode: 'auto' | 'manual' | 'step'
  stepConfirmNodeId?: string | null
}>()

const emit = defineEmits<{
  'retry': [nodeId: string]
  'human-response': [data: Record<string, unknown>]
  'skip-node': [nodeId: string]
  'pause': []
  'resume': []
  'select-node': [nodeId: string]
}>()

const expandedTasks = ref<Record<string, boolean>>({})
const collapsedPhases = ref<Record<string, boolean>>({})

watch(() => props.executionState, (newState) => {
  Object.values(newState).forEach(task => {
    if (task.status === 'running') expandedTasks.value[task.id] = true
  })
  phases.value.forEach(phase => {
    if (newState[phase.id]?.status === 'running') collapsedPhases.value[phase.id] = false
  })
}, { deep: true })

watch(() => props.currentHumanRequest, (req) => {
  if (req) {
    nextTick(() => {
      const el = document.getElementById(`human-req-${req.node_id}`)
      if (el) el.scrollIntoView({ behavior: 'smooth', block: 'center' })
    })
  }
})

const phases = computed<TaskTreeNode[]>(() => props.tree?.root?.children || [])

function getNodeStatus(nodeId: string) {
  return props.executionState[nodeId]?.status || 'pending'
}

function getPhaseStatus(phaseId: string) {
  const children = (props.tree.root.children || []).find(p => p.id === phaseId)?.children || []
  if (children.some(c => getNodeStatus(c.id) === 'running')) return 'running'
  if (props.currentHumanRequest && children.some(c => c.id === props.currentHumanRequest!.node_id)) return 'running'
  const status = getNodeStatus(phaseId)
  if (status === 'completed' && children.length > 0 && children.every(c => getNodeStatus(c.id) === 'pending')) return 'pending'
  return status || 'pending'
}

function getNodeState(nodeId: string) { return props.executionState[nodeId] }
function getNodeStateStrict(nodeId: string): ExecutionNodeState { return props.executionState[nodeId] as ExecutionNodeState }
const isPhaseCompleted = (id: string) => getPhaseStatus(id) === 'completed'
const isPhaseRunning = (id: string) => getPhaseStatus(id) === 'running'
const isPhaseFailed = (id: string) => getPhaseStatus(id) === 'failed'
const isPhasePending = (id: string) => getPhaseStatus(id) === 'pending'

function formatDuration(ms: number | null | undefined) {
  if (!ms) return ''
  return (ms / 1000).toFixed(1) + 's'
}

function getPhaseDuration(phaseId: string) {
  const children = (props.tree.root.children || []).find(p => p.id === phaseId)?.children || []
  if (children.length === 0) {
    const st = getNodeState(phaseId)
    if (st?.startTime && st?.endTime) return formatDuration(st.endTime - st.startTime)
    return ''
  }
  let total = 0, has = false
  children.forEach(t => {
    const st = getNodeState(t.id)
    if (st?.startTime && st?.endTime) { total += (st.endTime - st.startTime); has = true }
  })
  return has ? formatDuration(total) : ''
}

const allTasks = computed(() => phases.value.flatMap(p => p.children || []))
const totalTasks = computed(() => allTasks.value.length)
const completedTasks = computed(() => allTasks.value.filter(t => getNodeStatus(t.id) === 'completed').length)
const progressPercent = computed(() => totalTasks.value ? Math.round((completedTasks.value / totalTasks.value) * 100) : 0)

const nextPendingNodeId = computed(() => {
  for (const phase of phases.value) {
    for (const task of (phase.children || [])) {
      if (getNodeStatus(task.id) === 'pending') return task.id
    }
  }
  return null
})
</script>

<template>
  <div class="relative pl-0 space-y-6 pb-4">
    <div class="relative pl-3">
      <div class="absolute left-[29px] top-6 bottom-0 w-0.5 bg-gray-200 z-0"></div>
      <div v-if="phases.length === 0" class="text-center py-8 text-gray-400">暂无执行计划</div>

      <div v-for="(phase, index) in phases" :key="phase.id" class="relative z-10 group mb-8 last:mb-0">
        <div class="flex items-start gap-3 p-3 rounded-xl transition-colors duration-300"
          :class="[isPhaseRunning(phase.id) ? 'bg-blue-50/50' : 'hover:bg-gray-50/50']">
          <div class="relative z-10 w-8 h-8 rounded-full flex items-center justify-center transition-all duration-300 flex-shrink-0" @click="collapsedPhases[phase.id] = !collapsedPhases[phase.id]">
            <div v-if="isPhaseCompleted(phase.id)" class="w-6 h-6 rounded-full bg-green-500 text-white flex items-center justify-center shadow-sm cursor-pointer">
              <Check :size="14" stroke-width="3" />
            </div>
            <div v-else-if="isPhaseRunning(phase.id)" class="w-6 h-6 rounded-full bg-blue-50 border-2 border-blue-500 flex items-center justify-center shadow-md animate-pulse cursor-pointer">
              <div class="w-2 h-2 rounded-full bg-blue-500"></div>
            </div>
            <div v-else-if="isPhaseFailed(phase.id)" class="w-6 h-6 rounded-full bg-red-50 border border-red-200 text-red-500 flex items-center justify-center cursor-pointer">
              <AlertCircle :size="14" />
            </div>
            <div v-else class="w-6 h-6 rounded-full border-2 border-gray-200 bg-white flex items-center justify-center text-sm font-bold text-gray-400 cursor-pointer hover:border-gray-300">
              {{ index + 1 }}
            </div>
          </div>

          <div class="flex-1 min-w-0 pt-1">
            <div class="flex items-center justify-between mb-2 cursor-pointer p-1 rounded-lg -ml-1 transition-colors"
              @click="collapsedPhases[phase.id] = !collapsedPhases[phase.id]">
              <div class="flex items-center gap-3">
                <h3 class="font-bold transition-colors text-lg"
                  :class="[isPhaseRunning(phase.id) ? 'text-gray-900' : isPhasePending(phase.id) ? 'text-gray-400' : 'text-gray-700']">
                  {{ phase.name }}
                </h3>
                <span v-if="isPhaseRunning(phase.id)" class="text-[10px] px-2 py-0.5 rounded-full bg-blue-50 text-blue-600 font-medium border border-blue-100 flex items-center gap-1">
                  <Loader2 :size="10" class="animate-spin" />生成中...
                </span>
                <span v-if="(isPhaseCompleted(phase.id) || isPhaseFailed(phase.id)) && getPhaseDuration(phase.id)" class="text-[10px] text-gray-400 bg-gray-50 px-1.5 py-0.5 rounded">
                  {{ getPhaseDuration(phase.id) }}
                </span>
              </div>
              <component :is="collapsedPhases[phase.id] ? ChevronRight : ChevronDown" class="text-gray-300 w-4 h-4" />
            </div>

            <div v-show="!collapsedPhases[phase.id]" class="space-y-3 pl-1">
              <div v-for="task in (phase.children || [])" :key="task.id" class="relative">
                <div v-if="executionMode === 'step' && isPaused && nextPendingNodeId === task.id && getNodeStatus(task.id) === 'pending'"
                  class="bg-violet-50 border border-violet-200 rounded-xl p-4 mb-2">
                  <div class="flex items-center gap-2 mb-2">
                    <Pause :size="14" class="text-violet-600" />
                    <span class="font-medium text-violet-800">等待确认</span>
                  </div>
                  <p class="text-sm text-violet-600 mb-3">下一步将执行: {{ task.name }}</p>
                  <div class="flex items-center gap-2">
                    <button @click="$emit('resume')" class="flex items-center gap-1.5 px-3 py-1.5 bg-violet-600 text-white text-sm font-medium rounded-lg hover:bg-violet-700 transition-colors">
                      <Play :size="12" /> 执行此步
                    </button>
                    <button @click="$emit('skip-node', task.id)" class="flex items-center gap-1.5 px-3 py-1.5 border border-violet-200 text-violet-600 text-sm font-medium rounded-lg hover:bg-violet-50 transition-colors">
                      <SkipForward :size="12" /> 跳过
                    </button>
                  </div>
                </div>

                <div v-if="getNodeState(task.id)" class="transition-all duration-300">
                  <div v-if="currentHumanRequest && currentHumanRequest.node_id === task.id" :id="`human-req-${task.id}`" class="mb-3">
                    <HumanCard :request="currentHumanRequest" @response="$emit('human-response', $event)" />
                  </div>
                  <div class="group/task">
                    <div v-if="getNodeStatus(task.id) === 'running'" class="text-sm text-blue-500 mb-1 flex items-center justify-between gap-1 font-medium">
                      <div class="flex items-center gap-1 animate-pulse">
                        <Terminal :size="12" /> 正在执行: {{ getNodeState(task.id)?.name || task.name }}
                      </div>
                      <div class="flex items-center gap-1 opacity-0 group-hover/task:opacity-100 transition-opacity">
                        <button @click="$emit('skip-node', task.id)" class="p-1 text-gray-400 hover:text-violet-600 hover:bg-violet-50 rounded transition-colors" title="跳过">
                          <SkipForward :size="12" />
                        </button>
                        <button @click="$emit('pause')" class="p-1 text-gray-400 hover:text-amber-600 hover:bg-amber-50 rounded transition-colors" title="暂停">
                          <Pause :size="12" />
                        </button>
                      </div>
                    </div>
                    <div v-if="getNodeStatus(task.id) === 'skipped'" class="flex items-center gap-2 text-sm text-gray-400 py-2">
                      <Ban :size="12" /> <span>{{ task.name }} — 已跳过</span>
                    </div>
                    <div v-if="getNodeStatus(task.id) !== 'skipped'" @click="$emit('select-node', task.id)" class="cursor-pointer">
                      <TaskItem v-if="getNodeState(task.id)" :task="getNodeStateStrict(task.id)" mode="embedded" class="border-0 !bg-transparent p-0"
                        :force-collapsed="getNodeStatus(task.id) === 'completed' && !expandedTasks[task.id + '_manual']"
                        @retry="$emit('retry', task.id)" />
                    </div>
                    <div v-if="getNodeStatus(task.id) === 'failed'" class="flex items-center gap-2 mt-1">
                      <button @click="$emit('retry', task.id)" class="flex items-center gap-1 px-2 py-1 text-[10px] bg-red-50 text-red-600 rounded hover:bg-red-100 transition-colors">
                        <RefreshCw :size="10" /> 重试
                      </button>
                      <button @click="$emit('skip-node', task.id)" class="flex items-center gap-1 px-2 py-1 text-[10px] bg-gray-50 text-gray-500 rounded hover:bg-gray-100 transition-colors">
                        <SkipForward :size="10" /> 跳过
                      </button>
                    </div>
                  </div>
                </div>
                <div v-else class="pl-2 py-1 text-sm text-gray-300">{{ task.name }}</div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.left-\[29px\] { left: 29px; }
</style>
