<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { Circle, CheckCircle2, Loader2, AlertCircle } from 'lucide-vue-next'
import type { DAGNode, DAGEdge } from './treeToDAG'
import type { ExecutionNodeState } from './session'

interface ContextMenu {
  show: boolean
  x: number
  y: number
  node: DAGNode | null
}

const props = defineProps<{
  nodes: DAGNode[]
  edges: DAGEdge[]
  executionState: Record<string, ExecutionNodeState>
  animated?: boolean
}>()

const emit = defineEmits<{
  'node-click': [node: DAGNode]
  'node-context-menu': [payload: { action: string; node: DAGNode }]
}>()

const containerRef = ref<HTMLDivElement | null>(null)
const contextMenu = ref<ContextMenu>({ show: false, x: 0, y: 0, node: null })

const nodePositions = computed(() => {
  const positions: Record<string, { x: number; y: number; width: number; height: number }> = {}
  const levelGroups: Record<number, DAGNode[]> = {}

  for (const node of props.nodes) {
    const level = node.level || 0
    if (!levelGroups[level]) levelGroups[level] = []
    levelGroups[level].push(node)
  }

  const nodeWidth = 160
  const nodeHeight = 60
  const levelGap = 120
  const nodeGap = 80
  const levels = Object.keys(levelGroups).map(Number).sort((a, b) => a - b)

  for (const level of levels) {
    const nodesInLevel = levelGroups[level]
    const totalWidth = nodesInLevel.length * nodeWidth + (nodesInLevel.length - 1) * nodeGap
    const startX = -totalWidth / 2
    nodesInLevel.forEach((node, index) => {
      positions[node.id] = {
        x: startX + index * (nodeWidth + nodeGap) + nodeWidth / 2,
        y: level * levelGap,
        width: nodeWidth,
        height: nodeHeight,
      }
    })
  }
  return positions
})

const viewBox = computed(() => {
  const positions = Object.values(nodePositions.value)
  if (positions.length === 0) return '-200 -50 400 300'
  const xs = positions.map(p => p.x)
  const ys = positions.map(p => p.y)
  const minX = Math.min(...xs) - 100
  const maxX = Math.max(...xs) + 100
  const minY = Math.min(...ys) - 50
  const maxY = Math.max(...ys) + 100
  return `${minX} ${minY} ${maxX - minX} ${maxY - minY}`
})

const edgePaths = computed(() => {
  return props.edges.map(edge => {
    const from = nodePositions.value[edge.from]
    const to = nodePositions.value[edge.to]
    if (!from || !to) return null
    const startX = from.x
    const startY = from.y + from.height / 2
    const endX = to.x
    const endY = to.y - to.height / 2
    const midY = (startY + endY) / 2
    return {
      edge,
      path: `M ${startX} ${startY} C ${startX} ${midY}, ${endX} ${midY}, ${endX} ${endY}`,
      isActive: props.executionState[edge.from]?.status === 'completed',
    }
  }).filter((p): p is NonNullable<typeof p> => p !== null)
})

function getNodeStatus(nodeId: string) {
  return props.executionState[nodeId]?.status || 'pending'
}

function getNodeClass(nodeId: string) {
  const status = getNodeStatus(nodeId)
  switch (status) {
    case 'running': return 'node-running'
    case 'completed': return 'node-completed'
    case 'failed': return 'node-failed'
    case 'skipped': return 'node-skipped'
    default: return 'node-pending'
  }
}

function getStatusIcon(nodeId: string) {
  const status = getNodeStatus(nodeId)
  switch (status) {
    case 'running': return Loader2
    case 'completed': return CheckCircle2
    case 'failed': return AlertCircle
    default: return Circle
  }
}

function handleNodeClick(node: DAGNode) {
  emit('node-click', node)
}

function handleContextMenu(e: MouseEvent, node: DAGNode) {
  e.preventDefault()
  const rect = containerRef.value?.getBoundingClientRect()
  contextMenu.value = { show: true, x: e.clientX - (rect?.left || 0), y: e.clientY - (rect?.top || 0), node }
}

function closeContextMenu() {
  contextMenu.value.show = false
}
</script>

<template>
  <div ref="containerRef" class="graph-container w-full h-full bg-gray-50 rounded-xl overflow-hidden" @click="closeContextMenu">
    <svg :viewBox="viewBox" class="w-full h-full" preserveAspectRatio="xMidYMid meet">
      <defs>
        <marker id="arrowhead" markerWidth="10" markerHeight="7" refX="9" refY="3.5" orient="auto">
          <polygon points="0 0, 10 3.5, 0 7" fill="#9ca3af" />
        </marker>
        <marker id="arrowhead-active" markerWidth="10" markerHeight="7" refX="9" refY="3.5" orient="auto">
          <polygon points="0 0, 10 3.5, 0 7" fill="#14b8a6" />
        </marker>
        <filter id="glow">
          <feGaussianBlur stdDeviation="2" result="coloredBlur"/>
          <feMerge><feMergeNode in="coloredBlur"/><feMergeNode in="SourceGraphic"/></feMerge>
        </filter>
      </defs>
      <g class="edges">
        <path v-for="edgePath in edgePaths" :key="`${edgePath.edge.from}-${edgePath.edge.to}`"
          :d="edgePath.path" :class="['edge', { 'edge-active': edgePath.isActive }]"
          :marker-end="edgePath.isActive ? 'url(#arrowhead-active)' : 'url(#arrowhead)'"
          fill="none" stroke-width="2" />
      </g>
      <g class="nodes">
        <g v-for="node in nodes" :key="node.id"
          :transform="`translate(${nodePositions[node.id]?.x || 0}, ${nodePositions[node.id]?.y || 0})`"
          class="node-group cursor-pointer"
          @click="handleNodeClick(node)"
          @contextmenu="handleContextMenu($event, node)">
          <rect :x="-80" :y="-25" width="160" height="50" rx="8" :class="['node-rect', getNodeClass(node.id)]" />
          <foreignObject x="-75" y="-18" width="24" height="36">
            <div class="flex items-center justify-center h-full">
              <component :is="getStatusIcon(node.id)" :size="18"
                :class="[
                  getNodeStatus(node.id) === 'running' ? 'animate-spin text-teal-500' : '',
                  getNodeStatus(node.id) === 'completed' ? 'text-green-500' : '',
                  getNodeStatus(node.id) === 'failed' ? 'text-red-500' : '',
                  getNodeStatus(node.id) === 'pending' ? 'text-gray-400' : '',
                ]" />
            </div>
          </foreignObject>
          <text x="0" y="0" text-anchor="middle" dominant-baseline="middle" class="node-text">
            {{ node.name?.substring(0, 18) || node.id }}
          </text>
          <text x="0" y="15" text-anchor="middle" class="node-id">{{ node.id }}</text>
        </g>
      </g>
    </svg>

    <div class="absolute bottom-4 left-4 bg-white/90 backdrop-blur rounded-lg p-3 text-sm shadow-sm border border-gray-100">
      <div class="font-medium text-gray-700 mb-2">图例</div>
      <div class="space-y-1.5">
        <div class="flex items-center gap-2"><div class="w-3 h-3 rounded bg-gray-200"></div><span class="text-gray-500">等待中</span></div>
        <div class="flex items-center gap-2"><div class="w-3 h-3 rounded bg-teal-500"></div><span class="text-gray-500">执行中</span></div>
        <div class="flex items-center gap-2"><div class="w-3 h-3 rounded bg-green-500"></div><span class="text-gray-500">已完成</span></div>
        <div class="flex items-center gap-2"><div class="w-3 h-3 rounded bg-red-500"></div><span class="text-gray-500">失败</span></div>
      </div>
    </div>

    <div v-if="contextMenu.show" class="absolute bg-white border border-gray-200 rounded-lg shadow-xl z-50 py-1 w-40 text-sm"
      :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }" @mouseleave="closeContextMenu">
      <button @click="contextMenu.node && emit('node-click', contextMenu.node); closeContextMenu()"
        class="w-full text-left px-3 py-2 hover:bg-gray-50 text-gray-700 flex items-center gap-2">📋 查看详情</button>
      <button @click="contextMenu.node && emit('node-context-menu', { action: 'edit', node: contextMenu.node }); closeContextMenu()"
        class="w-full text-left px-3 py-2 hover:bg-gray-50 text-gray-700 flex items-center gap-2">✏️ 编辑节点</button>
      <button @click="contextMenu.node && emit('node-context-menu', { action: 'skip', node: contextMenu.node }); closeContextMenu()"
        class="w-full text-left px-3 py-2 hover:bg-gray-50 text-gray-700 flex items-center gap-2">⏭ 跳过节点</button>
      <hr class="my-1 border-gray-100">
      <button @click="contextMenu.node && emit('node-context-menu', { action: 'delete', node: contextMenu.node }); closeContextMenu()"
        class="w-full text-left px-3 py-2 hover:bg-red-50 text-red-600 flex items-center gap-2">🗑 删除节点</button>
    </div>
  </div>
</template>

<style scoped>
.graph-container { position: relative; min-height: 300px; }
.edge { stroke: #d1d5db; transition: stroke 0.3s ease; }
.edge-active { stroke: #14b8a6; stroke-width: 2.5; stroke-dasharray: 5 5; animation: flow 0.5s linear infinite; }
.node-rect { fill: #f9fafb; stroke: #e5e7eb; stroke-width: 2; transition: all 0.3s ease; }
.node-rect.node-pending { fill: #f9fafb; stroke: #e5e7eb; }
.node-rect.node-running { fill: #f0fdfa; stroke: #14b8a6; filter: url(#glow); }
.node-rect.node-completed { fill: #f0fdf4; stroke: #22c55e; }
.node-rect.node-failed { fill: #fef2f2; stroke: #ef4444; }
.node-rect.node-skipped { fill: #f9fafb; stroke: #9ca3af; stroke-dasharray: 4 2; }
.node-group:hover .node-rect { stroke-width: 3; }
.node-text { font-size: 12px; font-weight: 500; fill: #374151; }
.node-id { font-size: 9px; fill: #9ca3af; }
@keyframes flow { from { stroke-dashoffset: 20; } to { stroke-dashoffset: 0; } }
</style>
