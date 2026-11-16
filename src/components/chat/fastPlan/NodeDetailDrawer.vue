<script setup lang="ts">
import { computed } from 'vue'
import {
  X, Brain, Wrench, User, Circle, CheckCircle2, Loader2, AlertCircle,
  Clock, SkipForward, RefreshCw, Play, Edit3, ChevronRight, Ban
} from 'lucide-vue-next'
import type { TaskTree, TaskTreeNode } from './treeToDAG'
import type { ExecutionNodeState } from './session'

const props = defineProps<{
  nodeId: string | null
  tree: TaskTree | null
  executionState: Record<string, ExecutionNodeState>
  isPaused: boolean
}>()

const emit = defineEmits<{
  'close': []
  'retry': [nodeId: string]
  'skip': [nodeId: string]
  'resume': []
  'edit': [nodeId: string]
}>()

const nodeDef = computed<TaskTreeNode | null>(() => {
  if (!props.nodeId || !props.tree?.root?.children) return null
  for (const phase of props.tree.root.children) {
    if (phase.id === props.nodeId) return phase
    for (const task of (phase.children || [])) {
      if (task.id === props.nodeId) return task
    }
  }
  return null
})

const nodeState = computed(() => props.nodeId ? props.executionState[props.nodeId] || null : null)
const status = computed(() => nodeState.value?.status || 'pending')
const isRunning = computed(() => status.value === 'running')
const isCompleted = computed(() => status.value === 'completed')
const isFailed = computed(() => status.value === 'failed')
const isSkipped = computed(() => status.value === 'skipped')
const isPending = computed(() => status.value === 'pending')

const nodeType = computed(() => (nodeDef.value?.node_type as any)?.type || 'Unknown')
const nodeTypeLabel = computed(() => {
  switch (nodeType.value) {
    case 'LlmTask': return 'AI 推理'
    case 'ToolTask': return '工具调用'
    case 'HumanCheckpoint': return '人工确认'
    case 'Phase': return '阶段'
    default: return '任务'
  }
})
const nodeTypeIcon = computed(() => {
  switch (nodeType.value) {
    case 'LlmTask': return Brain
    case 'ToolTask': return Wrench
    case 'HumanCheckpoint': return User
    default: return Circle
  }
})

const statusConfig = computed(() => {
  if (isRunning.value) return { label: '执行中', color: 'text-teal-600', bg: 'bg-teal-50', border: 'border-teal-200', icon: Loader2, spin: true }
  if (isCompleted.value) return { label: '已完成', color: 'text-green-600', bg: 'bg-green-50', border: 'border-green-200', icon: CheckCircle2, spin: false }
  if (isFailed.value) return { label: '失败', color: 'text-red-600', bg: 'bg-red-50', border: 'border-red-200', icon: AlertCircle, spin: false }
  if (isSkipped.value) return { label: '已跳过', color: 'text-gray-500', bg: 'bg-gray-50', border: 'border-gray-200', icon: Ban, spin: false }
  return { label: '等待中', color: 'text-gray-500', bg: 'bg-gray-50', border: 'border-gray-200', icon: Circle, spin: false }
})

const duration = computed(() => {
  if (!nodeState.value?.startTime) return null
  const end = nodeState.value.endTime || Date.now()
  const ms = end - nodeState.value.startTime
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(1)}s`
})

const nodePrompt = computed(() => {
  const config = (nodeDef.value?.node_type as any)?.config
  if (!config) return ''
  return config.prompt_template || config.prompt || config.tool_name || ''
})

const toolArgs = computed(() => {
  if (nodeType.value !== 'ToolTask') return null
  return (nodeDef.value?.node_type as any)?.config?.arguments || null
})

const output = computed(() => nodeState.value?.output || nodeState.value?.streamContent || '')

const parentPhase = computed<TaskTreeNode | null>(() => {
  if (!props.tree?.root?.children) return null
  for (const phase of props.tree.root.children) {
    if (phase.id === props.nodeId) return null
    if ((phase.children || []).some(t => t.id === props.nodeId)) return phase
  }
  return null
})

const neighbors = computed(() => {
  if (!parentPhase.value) return { prev: null, next: null }
  const tasks = parentPhase.value.children || []
  const idx = tasks.findIndex(t => t.id === props.nodeId)
  return {
    prev: idx > 0 ? tasks[idx - 1] : null,
    next: idx < tasks.length - 1 ? tasks[idx + 1] : null,
  }
})
</script>

<template>
  <transition enter-active-class="transition-all duration-200 ease-out" leave-active-class="transition-all duration-150 ease-in"
    enter-from-class="translate-x-full opacity-0" leave-to-class="translate-x-full opacity-0">
    <div v-if="nodeId && nodeDef" class="w-full bg-white flex flex-col h-full overflow-hidden flex-shrink-0">
      <div class="flex-1 overflow-y-auto p-4 space-y-4">
        <div>
          <div class="flex items-start gap-3">
            <div class="p-2 rounded-lg" :class="statusConfig.bg">
              <component :is="nodeTypeIcon" :size="18" :class="statusConfig.color" />
            </div>
            <div class="flex-1 min-w-0">
              <h4 class="font-bold text-lg text-gray-900">{{ nodeDef.name }}</h4>
              <div class="flex items-center gap-2 mt-1">
                <span class="text-[10px] font-medium px-2 py-0.5 rounded-full border"
                  :class="[statusConfig.bg, statusConfig.color, statusConfig.border]">
                  <component :is="statusConfig.icon" :size="10" :class="{ 'animate-spin': statusConfig.spin }" class="inline mr-0.5" />
                  {{ statusConfig.label }}
                </span>
                <span class="text-[10px] px-1.5 py-0.5 bg-gray-100 text-gray-500 rounded font-medium">{{ nodeTypeLabel }}</span>
              </div>
            </div>
          </div>
        </div>
        <div v-if="duration" class="flex items-center gap-2 text-sm text-gray-500 bg-gray-50 rounded-lg px-3 py-2">
          <Clock :size="12" />
          <span>耗时: {{ duration }}</span>
          <template v-if="nodeState?.startTime">
            <span class="text-gray-300">|</span>
            <span>开始: {{ new Date(nodeState.startTime).toLocaleTimeString('zh-CN') }}</span>
          </template>
        </div>
        <div v-if="nodePrompt">
          <h5 class="text-xs font-bold uppercase tracking-wider text-gray-500 mb-2">输入 / 描述</h5>
          <div class="text-gray-700 bg-gray-50 rounded-lg p-3 border border-gray-100 whitespace-pre-wrap break-words max-h-40 overflow-y-auto">{{ nodePrompt }}</div>
        </div>
        <div v-if="toolArgs">
          <h5 class="text-xs font-bold uppercase tracking-wider text-gray-500 mb-2">工具参数</h5>
          <pre class="text-gray-600 bg-gray-50 rounded-lg p-3 overflow-x-auto max-h-32 overflow-y-auto text-sm">{{ JSON.stringify(toolArgs, null, 2) }}</pre>
        </div>
        <div v-if="output">
          <h5 class="text-xs font-bold uppercase tracking-wider text-gray-500 mb-2">输出</h5>
          <div class="text-gray-700 bg-gray-50 rounded-lg p-3 border border-gray-100 whitespace-pre-wrap break-words max-h-60 overflow-y-auto">{{ output }}</div>
        </div>
        <div v-if="nodeState?.error">
          <h5 class="text-xs font-bold uppercase tracking-wider text-gray-500 mb-2">错误信息</h5>
          <div class="text-red-600 bg-red-50 rounded-lg p-3 border border-red-100">{{ nodeState.error }}</div>
        </div>
        <div v-if="parentPhase">
          <h5 class="text-xs font-bold uppercase tracking-wider text-gray-500 mb-2">所属阶段</h5>
          <div class="text-gray-600 bg-gray-50 rounded-lg px-3 py-2">{{ parentPhase.name }}</div>
        </div>
        <div v-if="neighbors.prev || neighbors.next">
          <h5 class="text-xs font-bold uppercase tracking-wider text-gray-500 mb-2">依赖</h5>
          <div class="space-y-1.5">
            <div v-if="neighbors.prev" class="flex items-center gap-2 bg-gray-50 rounded-lg px-3 py-2">
              <ChevronRight :size="12" class="rotate-180" /> 前置: {{ neighbors.prev.name }}
            </div>
            <div v-if="neighbors.next" class="flex items-center gap-2 bg-gray-50 rounded-lg px-3 py-2">
              <ChevronRight :size="12" class="text-gray-400" /> 后续: {{ neighbors.next.name }}
            </div>
          </div>
        </div>
      </div>
      <div class="p-3 border-t border-gray-100 flex items-center gap-2">
        <button v-if="isPending || isPaused" @click="$emit('resume')"
          class="flex-1 flex items-center justify-center gap-1.5 px-3 py-2 bg-teal-600 text-white text-sm font-medium rounded-lg hover:bg-teal-700 transition-colors">
          <Play :size="14" /> 执行
        </button>
        <button v-if="isFailed" @click="nodeId && $emit('retry', nodeId)"
          class="flex-1 flex items-center justify-center gap-1.5 px-3 py-2 bg-amber-500 text-white text-sm font-medium rounded-lg hover:bg-amber-600 transition-colors">
          <RefreshCw :size="14" /> 重试
        </button>
        <button v-if="isPending || isRunning" @click="nodeId && $emit('skip', nodeId)"
          class="flex items-center justify-center gap-1.5 px-3 py-2 border border-gray-200 text-gray-600 text-sm font-medium rounded-lg hover:bg-gray-50 transition-colors">
          <SkipForward :size="14" /> 跳过
        </button>
        <button @click="nodeId && $emit('edit', nodeId)"
          class="flex items-center justify-center gap-1.5 px-3 py-2 border border-gray-200 text-gray-600 text-sm font-medium rounded-lg hover:bg-gray-50 transition-colors">
          <Edit3 :size="14" /> 编辑
        </button>
      </div>
    </div>
  </transition>
</template>
