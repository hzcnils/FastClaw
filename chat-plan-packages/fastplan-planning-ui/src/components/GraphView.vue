<script setup>
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue';
import { Circle, CheckCircle2, Loader2, AlertCircle, Brain, Wrench, User, Bot, ChevronRight } from 'lucide-vue-next';
import { ChatModelIcon } from 'chat-renderer';

const props = defineProps({
  nodes: { type: Array, default: () => [] },
  edges: { type: Array, default: () => [] },
  groups: { type: Array, default: () => [] },
  executionState: { type: Object, default: () => ({}) },
  animated: { type: Boolean, default: true }
});

const emit = defineEmits(['node-click', 'node-context-menu']);

const svgRef = ref(null);
const containerRef = ref(null);
const contextMenuRef = ref(null);
/** 窄屏（App / 小窗）：收紧间距与 viewBox，节点在 meet 缩放下更大更易读 */
const compact = ref(false);
let compactRo = null;

// 右键菜单状态（viewport 坐标，Teleport 到 body 避免被 overflow 裁切）
const contextMenu = ref({ show: false, x: 0, y: 0, node: null });

// 节点本地位置覆盖（拖拽用）
const localPositions = ref({});

// 拖拽状态
let dragging = null;

// ─── 坐标转换 ──────────────────────────────────────────────
const screenToSvg = (screenX, screenY) => {
  const svg = svgRef.value;
  if (!svg) return { x: 0, y: 0 };
  const pt = svg.createSVGPoint();
  pt.x = screenX;
  pt.y = screenY;
  const svgPt = pt.matrixTransform(svg.getScreenCTM().inverse());
  return { x: svgPt.x, y: svgPt.y };
};

// ─── 节点基础位置（按层级分组布局）──────────────────────────
const basePositions = computed(() => {
  const positions = {};
  const levelGroups = {};

  for (const node of props.nodes) {
    const level = node.level || 0;
    if (!levelGroups[level]) levelGroups[level] = [];
    levelGroups[level].push(node);
  }

  // 窄屏：节点略放大 + 间距收紧 → viewBox 更窄 → meet 后屏幕上更大
  const nodeWidth = compact.value ? 200 : 170;
  const nodeHeight = compact.value ? 72 : 60;
  const levelGap = compact.value ? 120 : 140;
  const nodeGap = compact.value ? 28 : 100;

  const levels = Object.keys(levelGroups).map(Number).sort((a, b) => a - b);
  for (const level of levels) {
    const nodesInLevel = levelGroups[level];
    const totalWidth = nodesInLevel.length * nodeWidth + (nodesInLevel.length - 1) * nodeGap;
    const startX = -totalWidth / 2;
    nodesInLevel.forEach((node, index) => {
      positions[node.id] = {
        x: startX + index * (nodeWidth + nodeGap) + nodeWidth / 2,
        y: level * levelGap,
        width: nodeWidth,
        height: nodeHeight
      };
    });
  }
  return positions;
});

// 合并本地位置覆盖
const nodePositions = computed(() => {
  const result = {};
  for (const id of Object.keys(basePositions.value)) {
    const base = basePositions.value[id];
    const local = localPositions.value[id];
    result[id] = local
      ? { ...base, x: local.x, y: local.y }
      : { ...base };
  }
  return result;
});

// 当节点列表变化时，清空本地位置覆盖
watch(() => props.nodes, () => { localPositions.value = {}; });

// ─── 阶段区域框（groups → 计算边界矩形）────────────────────
const NODE_W = computed(() => (compact.value ? 200 : 170));
const NODE_H = computed(() => (compact.value ? 72 : 60));

const groupBounds = computed(() => {
  const nw = NODE_W.value;
  const nh = NODE_H.value;
  const pad = compact.value ? 14 : 24;
  const labelH = compact.value ? 22 : 28;
  return (props.groups || []).map(group => {
    const positions = group.taskIds.map(id => nodePositions.value[id]).filter(Boolean);
    if (!positions.length) return null;
    const xs  = positions.map(p => p.x - nw / 2);
    const ys  = positions.map(p => p.y - nh / 2);
    const xs2 = positions.map(p => p.x + nw / 2);
    const ys2 = positions.map(p => p.y + nh / 2);
    return {
      ...group,
      x: Math.min(...xs)  - pad,
      y: Math.min(...ys)  - labelH - pad / 2,
      w: Math.max(...xs2) - Math.min(...xs) + pad * 2,
      h: Math.max(...ys2) - Math.min(...ys) + labelH + pad * 1.5,
    };
  }).filter(Boolean);
});

const groupStatusClass = (status) => {
  if (status === 'running')   return 'group-running';
  if (status === 'completed') return 'group-completed';
  if (status === 'failed')    return 'group-failed';
  return 'group-pending';
};

// ─── ViewBox ────────────────────────────────────────────────
const viewBox = computed(() => {
  const positions = Object.values(nodePositions.value);
  if (positions.length === 0) return '-200 -50 400 300';
  const padX = compact.value ? 36 : 120;
  const padYTop = compact.value ? 36 : 60;
  const padYBot = compact.value ? 48 : 100;
  const xs = positions.map(p => p.x);
  const ys = positions.map(p => p.y);
  let minX = Math.min(...xs) - padX;
  let maxX = Math.max(...xs) + padX;
  let minY = Math.min(...ys) - padYTop;
  let maxY = Math.max(...ys) + padYBot;
  // 扩展 viewBox 以容纳 group 区域框
  for (const g of groupBounds.value) {
    minX = Math.min(minX, g.x - 10);
    maxX = Math.max(maxX, g.x + g.w + 10);
    minY = Math.min(minY, g.y - 10);
    maxY = Math.max(maxY, g.y + g.h + 10);
  }
  return `${minX} ${minY} ${maxX - minX} ${maxY - minY}`;
});

// ─── 边路径 ─────────────────────────────────────────────────
const edgePaths = computed(() => {
  return props.edges.map(edge => {
    const from = nodePositions.value[edge.from];
    const to = nodePositions.value[edge.to];
    if (!from || !to) return null;
    const startX = from.x;
    const startY = from.y + from.height / 2;
    const endX = to.x;
    const endY = to.y - to.height / 2;
    const midY = (startY + endY) / 2;
    return {
      edge,
      path: `M ${startX} ${startY} C ${startX} ${midY}, ${endX} ${midY}, ${endX} ${endY}`,
      isActive: isEdgeActive(edge),
      isDependency: edge.type === 'dependency'
    };
  }).filter(Boolean);
});

const isEdgeActive = (edge) => props.executionState[edge.from]?.status === 'completed';

// ─── 节点状态 ───────────────────────────────────────────────
const getNodeStatus = (nodeId) => props.executionState[nodeId]?.status || 'pending';

const getNodeClass = (node) => {
  const status = getNodeStatus(node.id);
  if (status === 'running') return 'node-running';
  if (status === 'completed') return 'node-completed';
  if (status === 'failed') return 'node-failed';
  if (status === 'skipped') return 'node-skipped';
  return 'node-pending';
};

const getStatusIcon = (nodeId) => {
  const status = getNodeStatus(nodeId);
  if (status === 'running') return Loader2;
  if (status === 'completed') return CheckCircle2;
  if (status === 'failed') return AlertCircle;
  return Circle;
};

// ─── 节点类型颜色（PRD: 按 nodeType 区分颜色）────────────────
const getTypeAccentColor = (nodeType) => {
  switch (nodeType) {
    case 'Phase': return '#6b7280';
    case 'LlmTask': return '#0d9488';
    case 'ToolTask': return '#d97706';
    case 'HumanCheckpoint': return '#2563eb';
    case 'AgentTask': return '#7c3aed';
    case 'special': return '#10b981';
    default: return '#6b7280';
  }
};

const getTypeIcon = (nodeType) => {
  switch (nodeType) {
    case 'LlmTask': return Brain;
    case 'ToolTask': return Wrench;
    case 'HumanCheckpoint': return User;
    case 'AgentTask': return Bot;
    default: return null;
  }
};

const getTypeLabel = (nodeType) => {
  const map = { LlmTask: 'AI', ToolTask: '工具', HumanCheckpoint: '人工', AgentTask: 'Agent', Phase: '阶段' };
  return map[nodeType] || '';
};

// ─── 工具 badge 解析 ─────────────────────────────────────────
const getNodeTools = (node) => {
  if (node.nodeType === 'AgentTask' && node.assignedTools?.length) {
    return node.assignedTools.map(tool => {
      if (tool.includes(':')) {
        return { name: tool.split(':').pop(), type: 'mcp' }
      }
      if (['xlsx', 'docx', 'pptx', 'pdf', 'weather'].includes(tool)) {
        return { name: tool, type: 'skill' }
      }
      return { name: tool, type: 'http' }
    })
  }
  if (node.nodeType === 'LlmTask') {
    return [{ name: 'LLM', type: 'llm' }]
  }
  return []
};

const getToolBadgeColor = (type) => {
  const map = { skill: '#92400E', mcp: '#5B21B6', http: '#065F46', llm: '#1E40AF' }
  return map[type] || '#374151'
};

const getToolBadgeBg = (type) => {
  const map = { skill: '#FEF3C7', mcp: '#EDE9FE', http: '#D1FAE5', llm: '#DBEAFE' }
  return map[type] || '#F3F4F6'
};

// ─── 拖拽（鼠标 + 触摸；foreignObject 默认不拦截，否则手机点不到节点） ───
function clientPoint(e) {
  if (e.touches?.length) return { x: e.touches[0].clientX, y: e.touches[0].clientY }
  if (e.changedTouches?.length) return { x: e.changedTouches[0].clientX, y: e.changedTouches[0].clientY }
  return { x: e.clientX, y: e.clientY }
}

const onNodePointerDown = (e, node) => {
  if (e.type === 'mousedown' && e.button !== 0) return
  e.preventDefault()
  e.stopPropagation()
  const { x, y } = clientPoint(e)
  const svgPos = screenToSvg(x, y)
  const nodePos = nodePositions.value[node.id] || { x: 0, y: 0 }
  dragging = {
    nodeId: node.id,
    offsetX: svgPos.x - nodePos.x,
    offsetY: svgPos.y - nodePos.y,
  }
}

const onPointerMove = (e) => {
  if (!dragging) return
  if (e.cancelable) e.preventDefault()
  const { x, y } = clientPoint(e)
  const svgPos = screenToSvg(x, y)
  localPositions.value = {
    ...localPositions.value,
    [dragging.nodeId]: {
      x: svgPos.x - dragging.offsetX,
      y: svgPos.y - dragging.offsetY,
    },
  }
}

const onPointerUp = () => { dragging = null }

// ─── 右键菜单 ────────────────────────────────────────────────
const handleContextMenu = (e, node) => {
  e.preventDefault()
  e.stopPropagation()
  contextMenu.value = {
    show: true,
    x: e.clientX,
    y: e.clientY,
    node,
  }
  nextTick(() => {
    const el = contextMenuRef.value
    if (!el) return
    const r = el.getBoundingClientRect()
    const pad = 8
    let x = contextMenu.value.x
    let y = contextMenu.value.y
    if (r.bottom > window.innerHeight - pad) y = Math.max(pad, window.innerHeight - r.height - pad)
    if (r.right > window.innerWidth - pad) x = Math.max(pad, window.innerWidth - r.width - pad)
    if (x !== contextMenu.value.x || y !== contextMenu.value.y) {
      contextMenu.value = { ...contextMenu.value, x, y }
    }
  })
}

const closeContextMenu = () => { contextMenu.value.show = false }

const onContextMenuAction = (action) => {
  if (contextMenu.value.node) {
    if (action === 'detail') {
      emit('node-click', contextMenu.value.node)
    } else {
      emit('node-context-menu', { action, node: contextMenu.value.node })
    }
  }
  closeContextMenu()
}

// 展开/折叠 AgentTask 子流程
const expandedAgents = ref({})
const toggleAgentExpand = (e, nodeId) => {
  e.stopPropagation()
  expandedAgents.value[nodeId] = !expandedAgents.value[nodeId]
}

/** 节点绘制盒（随 compact 放大，避免窄屏 meet 后字过小） */
const nodeBox = computed(() => (
  compact.value
    ? {
        halfW: 108,
        top: -34,
        bodyH: 68,
        toolsH: 86,
        iconX: -98,
        titleX: -72,
        titleW: 148,
        titleH: 48,
        toolsY: 28,
        statusX: 78,
        iconSize: 16,
        statusSize: 18,
        nameLen: 22,
      }
    : {
        halfW: 95,
        top: -27,
        bodyH: 54,
        toolsH: 70,
        iconX: -86,
        titleX: -62,
        titleW: 120,
        titleH: 40,
        toolsY: 22,
        statusX: 65,
        iconSize: 14,
        statusSize: 16,
        nameLen: 18,
      }
))

function syncCompact() {
  const w = containerRef.value?.clientWidth || 0
  const next = w > 0 && w < 520
  if (next !== compact.value) {
    compact.value = next
    localPositions.value = {}
  }
}

// 全局抬起/移动（触摸需挂 document，且 touchmove 不能 passive）
onMounted(() => {
  document.addEventListener('mouseup', onPointerUp)
  document.addEventListener('touchend', onPointerUp)
  document.addEventListener('touchcancel', onPointerUp)
  document.addEventListener('mousemove', onPointerMove)
  document.addEventListener('touchmove', onPointerMove, { passive: false })
  syncCompact()
  if (typeof ResizeObserver !== 'undefined' && containerRef.value) {
    compactRo = new ResizeObserver(() => syncCompact())
    compactRo.observe(containerRef.value)
  }
})
onUnmounted(() => {
  document.removeEventListener('mouseup', onPointerUp)
  document.removeEventListener('touchend', onPointerUp)
  document.removeEventListener('touchcancel', onPointerUp)
  document.removeEventListener('mousemove', onPointerMove)
  document.removeEventListener('touchmove', onPointerMove)
  if (compactRo) {
    compactRo.disconnect()
    compactRo = null
  }
})
</script>

<template>
  <div
    ref="containerRef"
    class="fpp-ui graph-container w-full h-full bg-gray-50 rounded-xl overflow-hidden relative"
    :data-compact="compact ? '1' : '0'"
    @click="closeContextMenu"
  >    <svg
      ref="svgRef"
      :viewBox="viewBox"
      class="w-full h-full dag-svg"
      preserveAspectRatio="xMidYMid meet"
      @mousemove="onPointerMove"
      @mouseup="onPointerUp"
      @touchmove.prevent="onPointerMove"
      @touchend="onPointerUp"
    >
      <defs>
        <marker id="arrowhead" markerWidth="10" markerHeight="7" refX="9" refY="3.5" orient="auto">
          <polygon points="0 0, 10 3.5, 0 7" fill="#9ca3af" />
        </marker>
        <marker id="arrowhead-active" markerWidth="10" markerHeight="7" refX="9" refY="3.5" orient="auto">
          <polygon points="0 0, 10 3.5, 0 7" fill="#14b8a6" />
        </marker>
        <marker id="arrowhead-dep" markerWidth="10" markerHeight="7" refX="9" refY="3.5" orient="auto">
          <polygon points="0 0, 10 3.5, 0 7" fill="#7c3aed" />
        </marker>
        <filter id="glow">
          <feGaussianBlur stdDeviation="2" result="coloredBlur"/>
          <feMerge>
            <feMergeNode in="coloredBlur"/>
            <feMergeNode in="SourceGraphic"/>
          </feMerge>
        </filter>
        <filter id="shadow">
          <feDropShadow dx="0" dy="2" stdDeviation="3" flood-opacity="0.08"/>
        </filter>
      </defs>

      <!-- 阶段区域框（Phase Groups，在边和节点之后渲染层级最底） -->
      <g class="phase-groups">
        <g v-for="g in groupBounds" :key="g.id">
          <rect
            :x="g.x" :y="g.y" :width="g.w" :height="g.h"
            rx="12"
            class="group-rect"
            :class="groupStatusClass(g.status)"
          />
          <!-- Phase 标签背景（白底遮住边框） -->
          <rect :x="g.x + 8" :y="g.y - 2" :width="g.name.length * 7 + 16" height="20" rx="4" fill="white" />
          <text :x="g.x + 16" :y="g.y + 12" class="group-label">{{ g.name }}</text>
        </g>
      </g>

      <!-- 边 -->
      <g class="edges">
        <path
          v-for="edgePath in edgePaths"
          :key="`${edgePath.edge.from}-${edgePath.edge.to}`"
          :d="edgePath.path"
          :class="[
            'edge',
            { 'edge-active': edgePath.isActive },
            { 'edge-dependency': edgePath.isDependency && !edgePath.isActive }
          ]"
          :marker-end="edgePath.isActive ? 'url(#arrowhead-active)' : edgePath.isDependency ? 'url(#arrowhead-dep)' : 'url(#arrowhead)'"
          fill="none"
          stroke-width="2"
        />
      </g>

      <!-- 节点 -->
      <g class="nodes">
        <g
          v-for="node in nodes"
          :key="node.id"
          :transform="`translate(${nodePositions[node.id]?.x || 0}, ${nodePositions[node.id]?.y || 0})`"
          class="node-group"
          @mousedown="onNodePointerDown($event, node)"
          @touchstart.prevent="onNodePointerDown($event, node)"
          @click.stop="emit('node-click', node)"
          @contextmenu="handleContextMenu($event, node)"
        >
          <!-- 节点背景 -->
          <rect
            :x="-nodeBox.halfW"
            :y="nodeBox.top"
            :width="nodeBox.halfW * 2"
            :height="getNodeTools(node).length ? nodeBox.toolsH : nodeBox.bodyH"
            rx="10"
            :class="['node-rect', getNodeClass(node)]"
            filter="url(#shadow)"
          />

          <!-- 左侧类型色条 -->
          <rect
            :x="-nodeBox.halfW"
            :y="nodeBox.top"
            width="4"
            :height="getNodeTools(node).length ? nodeBox.toolsH : nodeBox.bodyH"
            rx="2"
            :fill="getTypeAccentColor(node.nodeType)"
            opacity="0.8"
          />

          <!-- 类型图标区域 -->
          <foreignObject :x="nodeBox.iconX" y="-20" width="22" height="40">
            <div class="flex items-center justify-center h-full">
              <ChatModelIcon
                v-if="node.nodeType === 'LlmTask'"
                class="fpp-model-icon"
                :name="node.model?.name"
                :model-id="node.model?.id"
                :size="nodeBox.iconSize"
              />
              <component
                v-else-if="getTypeIcon(node.nodeType)"
                :is="getTypeIcon(node.nodeType)"
                :size="nodeBox.iconSize"
                :style="{ color: getTypeAccentColor(node.nodeType) }"
              />
            </div>
          </foreignObject>

          <!-- 节点标题区：用 foreignObject+HTML，避免部分 WebView SVG <text> 中文不绘制 -->
          <foreignObject :x="nodeBox.titleX" y="-22" :width="nodeBox.titleW" :height="nodeBox.titleH">
            <div class="node-label-block" xmlns="http://www.w3.org/1999/xhtml">
              <div class="node-label-title">{{ node.name?.substring(0, nodeBox.nameLen) || node.id }}</div>
              <div class="node-label-type" :style="{ color: getTypeAccentColor(node.nodeType) }">
                {{ getTypeLabel(node.nodeType) }}
              </div>
            </div>
          </foreignObject>

          <!-- 工具 badge 行 -->
          <foreignObject v-if="getNodeTools(node).length" :x="nodeBox.iconX" :y="nodeBox.toolsY" :width="nodeBox.halfW * 2 - 20" height="22">
            <div class="flex items-center gap-1 px-1 overflow-hidden">
              <template v-for="(tool, ti) in getNodeTools(node).slice(0, 3)" :key="ti">
                <span class="inline-flex items-center px-1.5 py-0.5 rounded text-[9px] font-medium whitespace-nowrap"
                  :style="{ background: getToolBadgeBg(tool.type), color: getToolBadgeColor(tool.type) }">
                  {{ tool.name }}
                </span>
              </template>
              <span v-if="getNodeTools(node).length > 3" class="text-[9px] text-gray-400 font-medium">
                +{{ getNodeTools(node).length - 3 }}
              </span>
            </div>
          </foreignObject>

          <!-- 状态图标（右侧） -->
          <foreignObject :x="nodeBox.statusX" y="-18" width="24" height="36">
            <div class="flex items-center justify-center h-full">
              <component
                :is="getStatusIcon(node.id)"
                :size="nodeBox.statusSize"
                :class="[
                  getNodeStatus(node.id) === 'running' ? 'animate-spin' : '',
                ]"
                :style="{ color:
                  getNodeStatus(node.id) === 'running'  ? '#0d9488' :
                  getNodeStatus(node.id) === 'completed'? '#5B6AF0' :
                  getNodeStatus(node.id) === 'failed'   ? '#ef4444' : '#9ca3af'
                }"
              />
            </div>
          </foreignObject>

          <!-- AgentTask：展开/折叠按钮 -->
          <foreignObject v-if="node.hasSubTasks" x="35" y="14" width="20" height="16">
            <div class="dag-hit flex items-center justify-center h-full cursor-pointer" @click.stop="toggleAgentExpand($event, node.id)">
              <ChevronRight
                :size="12"
                class="text-violet-500 transition-transform"
                :style="{ transform: expandedAgents[node.id] ? 'rotate(90deg)' : 'rotate(0deg)' }"
              />
            </div>
          </foreignObject>
        </g>
      </g>
    </svg>

    <!-- 图例（App 用 .dag-legend 改成底部流式条，避免浮层挡节点） -->
    <div class="dag-legend absolute bottom-4 left-4 bg-white/90 backdrop-blur rounded-lg p-3 text-xs shadow-sm border border-gray-100">
      <div class="dag-legend__title font-medium text-gray-700 mb-2">图例</div>
      <div class="space-y-1.5">
        <div class="flex items-center gap-2"><div class="w-3 h-3 rounded bg-gray-200"></div><span class="text-gray-500">等待中</span></div>
        <div class="flex items-center gap-2"><div class="w-3 h-3 rounded bg-blue-500"></div><span class="text-gray-500">执行中</span></div>
        <div class="flex items-center gap-2"><div class="w-3 h-3 rounded" style="background:#5B6AF0"></div><span class="text-gray-500">已完成</span></div>
        <div class="flex items-center gap-2"><div class="w-3 h-3 rounded bg-red-500"></div><span class="text-gray-500">失败</span></div>
      </div>
      <div class="border-t border-gray-100 mt-2 pt-2 space-y-1.5">
        <div class="flex items-center gap-2"><div class="w-3 h-1.5 rounded" style="background:#0d9488"></div><span class="text-gray-500">AI 推理</span></div>
        <div class="flex items-center gap-2"><div class="w-3 h-1.5 rounded" style="background:#d97706"></div><span class="text-gray-500">工具调用</span></div>
        <div class="flex items-center gap-2"><div class="w-3 h-1.5 rounded" style="background:#2563eb"></div><span class="text-gray-500">人工确认</span></div>
        <div class="flex items-center gap-2"><div class="w-3 h-1.5 rounded" style="background:#7c3aed"></div><span class="text-gray-500">Agent</span></div>
      </div>
    </div>

    <!-- 操作提示 -->
    <div class="absolute top-3 right-3 text-[11px] text-gray-400 bg-white/80 backdrop-blur rounded px-2 py-1">
      拖拽移动 · 右键编辑
    </div>

    <!-- 右键菜单：Teleport 到 body，避免 graph-container overflow 裁切 -->
    <Teleport to="body">
      <div
        v-if="contextMenu.show"
        ref="contextMenuRef"
        class="fpp-ui fpp-graph-context-menu bg-white border border-gray-200 rounded-xl shadow-2xl py-1.5 w-44 text-xs overflow-hidden"
        :style="{ position: 'fixed', left: contextMenu.x + 'px', top: contextMenu.y + 'px', zIndex: 5000 }"
        @mouseleave="closeContextMenu"
      >
        <div class="px-3 py-1.5 text-[10px] font-bold text-gray-400 uppercase tracking-wider border-b border-gray-100 mb-1">
          {{ contextMenu.node?.name?.substring(0, 16) }}
        </div>
        <button @click="onContextMenuAction('detail')" class="w-full text-left px-3 py-2 hover:bg-gray-50 text-gray-700 flex items-center gap-2">
          📋 查看详情
        </button>
        <button @click="onContextMenuAction('edit')" class="w-full text-left px-3 py-2 hover:bg-blue-50 text-blue-700 flex items-center gap-2">
          ✏️ 编辑节点
        </button>
        <button @click="onContextMenuAction('add-after')" class="w-full text-left px-3 py-2 hover:bg-blue-50 text-blue-700 flex items-center gap-2">
          ➕ 在后方插入
        </button>
        <button
          v-if="contextMenu.node?.nodeType !== 'Phase'"
          @click="onContextMenuAction('skip')"
          class="w-full text-left px-3 py-2 hover:bg-gray-50 text-gray-700 flex items-center gap-2"
        >
          ⏭ 跳过节点
        </button>
        <hr class="my-1 border-gray-100">
        <button @click="onContextMenuAction('delete')" class="w-full text-left px-3 py-2 hover:bg-red-50 text-red-600 flex items-center gap-2">
          🗑 删除节点
        </button>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.graph-container {
  position: relative;
  min-height: 300px;
  cursor: default;
  touch-action: none;
}
.dag-svg {
  touch-action: none;
  user-select: none;
  -webkit-user-select: none;
}

.node-group {
  cursor: grab;
}
.node-group:active {
  cursor: grabbing;
}

/* HTML 层默认不抢事件，拖拽落在 SVG rect 上；仅展开按钮可点 */
.graph-container :deep(foreignObject) {
  pointer-events: none;
}
.graph-container :deep(foreignObject .dag-hit) {
  pointer-events: auto;
}

.edge {
  stroke: #d1d5db;
  transition: stroke 0.3s ease;
}
.edge-active {
  stroke: #14b8a6;
  stroke-width: 2.5;
  stroke-dasharray: 5 5;
  animation: flow 0.5s linear infinite;
}
.edge-dependency {
  stroke: #7c3aed;
  stroke-dasharray: 4 2;
}

.node-rect {
  fill: #ffffff;
  stroke: #e5e7eb;
  stroke-width: 1.5;
  transition: all 0.25s ease;
}
.node-rect.node-pending  { fill: #f9fafb; stroke: #e5e7eb; }
.node-rect.node-running  { fill: #f0fdfa; stroke: #14b8a6; filter: url(#glow); }
.node-rect.node-completed{ fill: #EFF6FF; stroke: #5B6AF0; }
.node-rect.node-failed   { fill: #fef2f2; stroke: #ef4444; }
.node-rect.node-skipped  { fill: #f9fafb; stroke: #9ca3af; stroke-dasharray: 4 2; }

.node-group:hover .node-rect {
  stroke-width: 2.5;
  filter: url(#shadow);
}

.node-text {
  font-size: 12px;
  font-weight: 600;
  fill: #1f2937;
  pointer-events: none;
  user-select: none;
}
.node-type-label {
  font-size: 9px;
  font-weight: 500;
  pointer-events: none;
  user-select: none;
  opacity: 0.85;
}
.node-label-block {
  display: flex;
  flex-direction: column;
  justify-content: center;
  height: 100%;
  padding: 0 2px;
  overflow: hidden;
  pointer-events: none;
  user-select: none;
  box-sizing: border-box;
}
.node-label-title {
  font-size: 12px;
  font-weight: 600;
  line-height: 1.25;
  color: #1f2937;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.node-label-type {
  margin-top: 2px;
  font-size: 9px;
  font-weight: 500;
  line-height: 1.2;
  opacity: 0.85;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.graph-container[data-compact="1"] .node-label-title {
  font-size: 15px;
  line-height: 1.3;
}
.graph-container[data-compact="1"] .node-label-type {
  font-size: 12px;
  margin-top: 3px;
}
.graph-container[data-compact="1"] .group-label {
  font-size: 13px;
}

@keyframes flow {
  from { stroke-dashoffset: 20; }
  to   { stroke-dashoffset: 0;  }
}

/* ─── 阶段区域框样式 ───────────────────────────────────────── */
.group-rect {
  stroke-width: 1.5;
  fill: rgba(249,250,251,0.55);
  stroke: #d1d5db;
  transition: all 0.3s ease;
}
.group-running   { fill: rgba(240,253,250,0.65); stroke: #5eead4; }
.group-completed { fill: rgba(240,253,244,0.65); stroke: #86efac; }
.group-failed    { fill: rgba(254,242,242,0.65); stroke: #fca5a5; }

.group-label {
  font-size: 11px;
  font-weight: 700;
  fill: #6b7280;
  pointer-events: none;
  user-select: none;
  text-transform: uppercase;
  letter-spacing: 0.06em;
}
</style>
