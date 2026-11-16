<script setup>
import { computed, ref } from 'vue'
import { ChevronLeft, ChevronRight, GitBranch, Zap } from 'lucide-vue-next'
import GraphView from './GraphView.vue'
import { treeToDAG, getDAGStats } from '../utils/treeToDAG'

const props = defineProps({
  tree: { type: Object, default: null },
  executionState: { type: Object, default: () => ({}) },
  isCollapsed: { type: Boolean, default: false },
})

const emit = defineEmits(['toggle', 'node-click'])

const dagData = computed(() => {
  if (!props.tree) return { nodes: [], edges: [], groups: [] }
  return treeToDAG(props.tree, props.executionState)
})

const stats = computed(() => getDAGStats(dagData.value.nodes))

// 检测并发执行（多个节点同时 running）
const hasParallel = computed(() => stats.value.running > 1)

// 按阶段统计运行/完成情况
const phaseStats = computed(() => {
  if (!props.tree?.root?.children) return []
  return props.tree.root.children.map(phase => {
    const tasks = phase.children || []
    const total = tasks.length
    const completed = tasks.filter(t => props.executionState[t.id]?.status === 'completed').length
    const running = tasks.filter(t => props.executionState[t.id]?.status === 'running').length
    const failed = tasks.filter(t => props.executionState[t.id]?.status === 'failed').length
    return { id: phase.id, name: phase.name, total, completed, running, failed }
  })
})
</script>

<template>
  <div
    class="fpp-ui border-r border-gray-200 bg-gray-50/80 flex flex-col flex-shrink-0 transition-all duration-300 hidden lg:flex"
    :class="isCollapsed ? 'w-10' : 'w-64'"
  >
    <!-- Header -->
    <div class="h-14 flex items-center justify-between px-2.5 border-b border-gray-100 flex-shrink-0">
      <div v-if="!isCollapsed" class="flex items-center gap-2 overflow-hidden">
        <GitBranch :size="14" class="text-blue-600 flex-shrink-0" />
        <span class="text-xs font-bold text-gray-700 truncate">执行进度</span>
        <!-- 并发标识 -->
        <span v-if="hasParallel" class="flex items-center gap-0.5 text-[10px] text-amber-600 bg-amber-50 border border-amber-200 px-1.5 py-0.5 rounded-full font-medium flex-shrink-0">
          <Zap :size="9" /> 并发
        </span>
      </div>
      <button
        @click="emit('toggle')"
        class="p-1.5 hover:bg-gray-200 rounded-lg transition-colors text-gray-400 hover:text-gray-600 flex-shrink-0"
        :title="isCollapsed ? '展开执行 DAG' : '折叠面板'"
      >
        <ChevronLeft v-if="!isCollapsed" :size="14" />
        <GitBranch v-else :size="14" class="text-blue-500" />
      </button>
    </div>

    <!-- 折叠时：竖排节点状态圆点 -->
    <div v-if="isCollapsed" class="flex-1 flex flex-col items-center py-3 gap-1.5 overflow-y-auto">
      <template v-for="ps in phaseStats" :key="ps.id">
        <div
          class="w-4 h-4 rounded-full border-2 border-white shadow-sm flex-shrink-0"
          :class="
            ps.failed   > 0 ? 'bg-red-400'   :
            ps.running  > 0 ? 'bg-blue-400 animate-pulse' :
            ps.completed === ps.total && ps.total > 0 ? 'bg-green-400' :
            'bg-gray-300'
          "
          :title="ps.name"
        />
      </template>
    </div>

    <!-- DAG 图 -->
    <div v-if="!isCollapsed" class="flex-1 overflow-hidden min-h-0 relative">
      <GraphView
        v-if="dagData.nodes.length > 0"
        :nodes="dagData.nodes"
        :edges="dagData.edges"
        :groups="dagData.groups"
        :executionState="executionState"
        :animated="true"
        @node-click="emit('node-click', $event)"
      />
      <div v-else class="flex items-center justify-center h-full text-xs text-gray-400 p-4 text-center leading-relaxed">
        等待任务<br>规划中...
      </div>
    </div>

    <!-- 进度由粘性条展示，此处仅保留状态徽章 -->
    <div v-if="!isCollapsed && stats.total > 0" class="p-2 border-t border-gray-100 flex-shrink-0 space-y-1">
      <!-- 状态徽章行 -->
      <div class="flex items-center gap-2 text-[10px]">
        <span v-if="stats.running > 0" class="flex items-center gap-1 text-blue-700 bg-blue-50 border border-blue-100 px-2 py-0.5 rounded-full">
          <span class="w-1.5 h-1.5 rounded-full bg-blue-500 animate-pulse inline-block"></span>
          {{ stats.running }} 执行中
        </span>
        <span v-if="hasParallel" class="flex items-center gap-0.5 text-amber-600 bg-amber-50 border border-amber-100 px-2 py-0.5 rounded-full">
          <Zap :size="9" /> 并发
        </span>
        <span v-if="stats.failed > 0" class="text-red-500 bg-red-50 border border-red-100 px-2 py-0.5 rounded-full">
          {{ stats.failed }} 失败
        </span>
      </div>
    </div>
  </div>

</template>
