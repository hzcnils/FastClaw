<script setup lang="ts">
import { computed } from 'vue'
import { Pause, Play, SkipForward, RefreshCw, Loader2, CheckCircle2, AlertCircle } from 'lucide-vue-next'
import type { ExecutionNodeState } from './session'

const props = defineProps<{
  executionState: Record<string, ExecutionNodeState>
  isPaused: boolean
  isPlanning: boolean
  executionMode: 'auto' | 'manual' | 'step'
}>()

const emit = defineEmits<{
  'pause': []
  'resume': []
  'skip-current': [nodeId: string]
  'replan': []
}>()

const tasks = computed(() => Object.values(props.executionState))
const total = computed(() => tasks.value.length)
const completed = computed(() => tasks.value.filter(t => t.status === 'completed').length)
const running = computed(() => tasks.value.filter(t => t.status === 'running').length)
const failed = computed(() => tasks.value.filter(t => t.status === 'failed').length)
const progress = computed(() => total.value > 0 ? Math.round((completed.value / total.value) * 100) : 0)
const currentRunning = computed(() => tasks.value.find(t => t.status === 'running'))
const isFinished = computed(() => !props.isPlanning && total.value > 0 && running.value === 0)

const modeLabel = computed(() => {
  switch (props.executionMode) {
    case 'auto': return '自动'
    case 'step': return '逐步'
    default: return '手动'
  }
})
</script>

<template>
  <div v-if="total > 0 && (isPlanning || !isFinished)"
    class="flex items-center justify-between gap-4 px-4 py-2.5 bg-white border border-gray-200 rounded-xl shadow-sm mb-4">
    <div class="flex items-center gap-3 min-w-0">
      <div class="relative w-12 h-12 flex-shrink-0">
        <svg class="w-12 h-12 -rotate-90" viewBox="0 0 36 36">
          <circle cx="18" cy="18" r="15" fill="none" stroke="#e5e7eb" stroke-width="3" />
          <circle cx="18" cy="18" r="15" fill="none" stroke="#14b8a6" stroke-width="3" stroke-linecap="round"
            :stroke-dasharray="`${progress * 0.942} 94.2`" class="transition-all duration-500" />
        </svg>
        <span class="absolute inset-0 flex items-center justify-center text-[9px] font-bold text-gray-600">{{ progress }}%</span>
      </div>
      <div class="min-w-0">
        <div class="flex items-center gap-2">
          <span class="font-medium text-gray-800">
            <template v-if="isPaused">已暂停</template>
            <template v-else-if="running > 0">执行中</template>
            <template v-else>等待中</template>
          </span>
          <span class="text-[10px] px-1.5 py-0.5 rounded bg-gray-100 text-gray-500 font-medium">{{ modeLabel }}模式</span>
        </div>
        <div class="flex items-center gap-2 text-[11px] text-gray-500 mt-0.5">
          <span class="flex items-center gap-0.5"><CheckCircle2 :size="10" class="text-green-500" /> {{ completed }}</span>
          <span v-if="running > 0" class="flex items-center gap-0.5"><Loader2 :size="10" class="text-teal-500 animate-spin" /> {{ running }}</span>
          <span v-if="failed > 0" class="flex items-center gap-0.5"><AlertCircle :size="10" class="text-red-500" /> {{ failed }}</span>
          <span class="text-gray-400">/ {{ total }} 任务</span>
        </div>
      </div>
    </div>
    <div class="flex items-center gap-1.5">
      <button v-if="!isPaused && running > 0" @click="$emit('pause')"
        class="p-2 text-gray-500 hover:text-amber-600 hover:bg-amber-50 rounded-lg transition-colors" title="暂停全部">
        <Pause :size="16" />
      </button>
      <button v-if="isPaused" @click="$emit('resume')"
        class="p-2 text-amber-600 bg-amber-50 hover:bg-amber-100 rounded-lg transition-colors" title="恢复执行">
        <Play :size="16" />
      </button>
      <button v-if="currentRunning" @click="$emit('skip-current', currentRunning.id)"
        class="p-2 text-gray-500 hover:text-violet-600 hover:bg-violet-50 rounded-lg transition-colors" title="跳过当前节点">
        <SkipForward :size="16" />
      </button>
      <button @click="$emit('replan')"
        class="p-2 text-gray-500 hover:text-teal-600 hover:bg-teal-50 rounded-lg transition-colors" title="重新规划">
        <RefreshCw :size="16" />
      </button>
    </div>
  </div>
</template>
