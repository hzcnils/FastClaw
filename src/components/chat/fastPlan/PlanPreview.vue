<script setup lang="ts">
import { ref, computed } from 'vue'
import { Play, CheckCircle2, Brain, User, Wrench, GripVertical, ChevronDown, ChevronRight, Layers, X, Trash2, Plus, GitBranch, List } from 'lucide-vue-next'
import GraphView from './GraphView.vue'
import { treeToDAG } from './treeToDAG'
import type { TaskTree, TaskTreeNode } from './treeToDAG'
import { useDialog } from '@/composables/useDialog'

const { confirm: confirmDialog } = useDialog()

const props = defineProps<{
  tree: TaskTree
  executionMode?: 'auto' | 'manual' | 'step'
}>()

const emit = defineEmits<{ 'confirm': [tree: TaskTree]; 'cancel': [] }>()

const editableTree = ref<TaskTree>(JSON.parse(JSON.stringify(props.tree)))
const viewMode = ref<'timeline' | 'dag'>('timeline')
const dagData = computed(() => treeToDAG(editableTree.value))
const collapsedPhases = ref<Record<string, boolean>>({})

interface NodeTypeOption {
  id: string
  label: string
  icon: unknown
  defaultConfig: Record<string, unknown>
}

const nodeTypeOptions: NodeTypeOption[] = [
  { id: 'LlmTask', label: 'AI 推理', icon: Brain, defaultConfig: { prompt_template: '' } },
  { id: 'ToolTask', label: '工具调用', icon: Wrench, defaultConfig: { tool_name: '', arguments: {} } },
  { id: 'HumanCheckpoint', label: '人工确认', icon: User, defaultConfig: { prompt: '', options: ['continue', 'abort'] } },
]

function switchNodeType(task: TaskTreeNode, newTypeId: string) {
  const cfg = nodeTypeOptions.find(t => t.id === newTypeId)
  if (cfg) (task as any).node_type = { type: newTypeId, config: { ...cfg.defaultConfig } }
}

const editingTypeId = ref<string | null>(null)
const toggleTypeEdit = (taskId: string) => { editingTypeId.value = editingTypeId.value === taskId ? null : taskId }

const deletePhase = async (idx: number) => {
  if (await confirmDialog('确定要删除这个阶段吗？')) editableTree.value.root.children!.splice(idx, 1)
}
const deleteTask = (phase: TaskTreeNode, idx: number) => { phase.children!.splice(idx, 1) }

const addPhase = () => {
  const id = `phase_${Date.now()}`
  if (!editableTree.value.root.children) editableTree.value.root.children = []
  editableTree.value.root.children.push({ id, name: '新阶段', node_type: { type: 'Phase' }, children: [] })
  collapsedPhases.value[id] = false
}
const addTask = (phase: TaskTreeNode) => {
  if (!phase.children) phase.children = []
  phase.children.push({ id: `task_${Date.now()}`, name: '新任务', node_type: { type: 'LlmTask', config: { prompt_template: '请输入任务描述...' } } })
}

// Drag & Drop
const draggedItem = ref<TaskTreeNode | null>(null)
const draggedType = ref<string | null>(null)
const dragOverItem = ref<TaskTreeNode | null>(null)
const draggedSourceList = ref<TaskTreeNode[] | null>(null)

const onDragStart = (e: DragEvent, item: TaskTreeNode, type: string, src: TaskTreeNode[] | null = null) => {
  draggedItem.value = item; draggedType.value = type
  if (type === 'task' && src) draggedSourceList.value = src
  if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move'
  setTimeout(() => { if (e.target) (e.target as HTMLElement).classList.add('opacity-50') }, 0)
}
const onDragEnd = (e: DragEvent) => {
  if (e.target) (e.target as HTMLElement).classList.remove('opacity-50')
  draggedItem.value = null; draggedType.value = null; dragOverItem.value = null; draggedSourceList.value = null
}
const onDragOver = (e: DragEvent, item: TaskTreeNode, type: string) => {
  if (draggedType.value === 'task' && type === 'phase') { e.preventDefault(); dragOverItem.value = item; return }
  if (draggedType.value !== type || draggedItem.value === item) return
  e.preventDefault(); dragOverItem.value = item
}
const onDrop = (e: DragEvent, target: TaskTreeNode, type: string, list: TaskTreeNode[]) => {
  e.preventDefault()
  if (draggedType.value === 'task' && type === 'task') {
    const s = draggedSourceList.value; if (!s) return
    const fi = s.indexOf(draggedItem.value!), ti = list.indexOf(target)
    if (fi > -1 && ti > -1) { const [m] = s.splice(fi, 1); list.splice(ti, 0, m) }
  } else if (draggedType.value === 'phase' && type === 'phase') {
    const fi = list.indexOf(draggedItem.value!), ti = list.indexOf(target)
    if (fi > -1 && ti > -1) { const [m] = list.splice(fi, 1); list.splice(ti, 0, m) }
  } else if (draggedType.value === 'task' && type === 'phase') {
    const s = draggedSourceList.value
    if (s && target.children) {
      const fi = s.indexOf(draggedItem.value!)
      if (fi > -1) { const [m] = s.splice(fi, 1); target.children.push(m); collapsedPhases.value[target.id] = false }
    }
  }
}

const getNodeTypeStr = (t: TaskTreeNode) => (t.node_type as any)?.type || 'Unknown'
const getNodeIcon = (t: TaskTreeNode) => {
  const tp = getNodeTypeStr(t)
  return tp === 'HumanCheckpoint' ? User : tp === 'LlmTask' ? Brain : tp === 'ToolTask' ? Wrench : CheckCircle2
}
const getNodeLabel = (t: TaskTreeNode) => {
  return ({ HumanCheckpoint: '人工确认', LlmTask: 'AI 推理', ToolTask: '工具调用', Phase: '阶段' } as Record<string, string>)[getNodeTypeStr(t)] || '任务'
}
const getTaskDetailKey = (t: TaskTreeNode) => {
  const tp = getNodeTypeStr(t)
  return tp === 'LlmTask' ? 'prompt_template' : tp === 'HumanCheckpoint' ? 'prompt' : 'tool_name'
}

const totalPhases = computed(() => editableTree.value.root?.children?.length || 0)
const totalTasks = computed(() => (editableTree.value.root?.children || []).reduce((s, p) => s + (p.children?.length || 0), 0))

const handleDagNodeClick = (node: { id: string }) => {
  for (const p of (editableTree.value.root?.children || [])) {
    if (p.id === node.id || (p.children || []).some(t => t.id === node.id)) {
      collapsedPhases.value[p.id] = false
      viewMode.value = 'timeline'
      return
    }
  }
}
</script>

<template>
  <div class="bg-white rounded-2xl shadow-xl border border-gray-200 overflow-hidden flex flex-col max-h-[80vh] w-full">
    <div class="p-5 border-b border-gray-100 bg-gray-50 sticky top-0 z-10">
      <div class="flex items-center justify-between mb-3">
        <div>
          <h2 class="text-lg font-bold text-gray-800 flex items-center gap-2">
            <Layers class="text-teal-600" :size="20" /> 任务规划预览
          </h2>
          <p class="text-sm text-gray-500 mt-1">已规划 {{ totalPhases }} 个阶段 · {{ totalTasks }} 个任务，您可以编辑后确认执行</p>
        </div>
        <div class="flex gap-3">
          <button @click="$emit('cancel')" class="px-4 py-2 text-gray-600 hover:bg-gray-200 rounded-lg transition-colors flex items-center gap-1">
            <X :size="16" /> 取消
          </button>
          <button @click="$emit('confirm', editableTree)" class="px-5 py-2 bg-teal-600 text-white font-medium rounded-lg hover:bg-teal-700 shadow-md transition-all flex items-center gap-2 active:scale-95">
            <Play :size="16" fill="currentColor" /> 确认执行
          </button>
        </div>
      </div>
      <div class="flex items-center gap-1 bg-gray-200/60 rounded-lg p-0.5 w-fit">
        <button @click="viewMode = 'timeline'" class="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium transition-all" :class="viewMode === 'timeline' ? 'bg-white text-gray-800 shadow-sm' : 'text-gray-500 hover:text-gray-700'">
          <List :size="14" /> 时间轴
        </button>
        <button @click="viewMode = 'dag'" class="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium transition-all" :class="viewMode === 'dag' ? 'bg-white text-gray-800 shadow-sm' : 'text-gray-500 hover:text-gray-700'">
          <GitBranch :size="14" /> DAG 图
        </button>
      </div>
    </div>

    <div class="flex-1 overflow-y-auto bg-gray-50/50 relative">
      <div v-if="viewMode === 'dag'" class="p-4 h-[500px]">
        <GraphView :nodes="dagData.nodes" :edges="dagData.edges" :executionState="{}" @node-click="handleDagNodeClick" />
      </div>
      <div v-else class="p-6">
        <div class="max-w-3xl mx-auto relative pl-4 pb-20">
          <div class="absolute left-[27px] top-4 bottom-4 w-0.5 bg-gray-200 z-0"></div>

          <div v-for="(phase, pIdx) in editableTree.root.children" :key="phase.id"
            class="mb-6 relative z-10 transition-all duration-200" draggable="true"
            @dragstart="onDragStart($event, phase, 'phase')" @dragend="onDragEnd"
            @dragover="onDragOver($event, phase, 'phase')" @drop="onDrop($event, phase, 'phase', editableTree.root.children!)">

            <div class="flex items-center gap-4 mb-3 group select-none">
              <div class="w-14 h-14 rounded-full bg-white border-4 border-gray-100 flex items-center justify-center shadow-sm z-10 cursor-pointer" @click="collapsedPhases[phase.id] = !collapsedPhases[phase.id]">
                <span class="font-bold text-teal-600 text-lg">{{ pIdx + 1 }}</span>
              </div>
              <div class="flex-1 bg-white p-4 rounded-xl border border-gray-200 shadow-sm hover:shadow-md hover:border-teal-200 transition-all">
                <div class="flex items-center justify-between gap-3">
                  <input v-model="phase.name" class="flex-1 font-bold text-lg text-gray-800 bg-transparent border-b-2 border-transparent hover:border-gray-200 focus:border-teal-500 focus:outline-none transition-colors px-1 py-0.5" placeholder="阶段名称" />
                  <div class="flex items-center gap-2">
                    <button @click="deletePhase(pIdx)" class="p-1.5 text-gray-400 hover:text-red-500 hover:bg-red-50 rounded-lg transition-colors opacity-0 group-hover:opacity-100" title="删除阶段">
                      <Trash2 :size="16" />
                    </button>
                    <button @click="collapsedPhases[phase.id] = !collapsedPhases[phase.id]" class="p-1 text-gray-400 hover:bg-gray-100 rounded-lg">
                      <component :is="collapsedPhases[phase.id] ? ChevronRight : ChevronDown" :size="20" />
                    </button>
                  </div>
                </div>
              </div>
            </div>

            <transition enter-active-class="transition-all duration-300 ease-out" leave-active-class="transition-all duration-200 ease-in"
              enter-from-class="opacity-0 -translate-y-2" leave-to-class="opacity-0 -translate-y-2">
              <div v-if="!collapsedPhases[phase.id]" class="pl-[3.5rem] space-y-3">
                <div v-for="(task, tIdx) in phase.children" :key="task.id"
                  class="group/task relative bg-white border border-gray-200 p-4 rounded-xl hover:shadow-md transition-all hover:border-teal-300 flex gap-3 items-start"
                  draggable="true" @dragstart.stop="onDragStart($event, task, 'task', phase.children!)" @dragend.stop="onDragEnd"
                  @dragover.stop="onDragOver($event, task, 'task')" @drop.stop="onDrop($event, task, 'task', phase.children!)">
                  <div class="mt-1 cursor-grab active:cursor-grabbing text-gray-300 hover:text-gray-500"><GripVertical :size="16" /></div>
                  <div class="relative">
                    <div class="mt-0.5 p-2 bg-gray-50 rounded-lg text-gray-500 group-hover/task:text-teal-600 group-hover/task:bg-teal-50 transition-colors cursor-pointer" @click="toggleTypeEdit(task.id)">
                      <component :is="getNodeIcon(task)" :size="18" />
                    </div>
                    <div v-if="editingTypeId === task.id" class="absolute top-full left-0 mt-1 bg-white border border-gray-200 rounded-lg shadow-xl z-50 py-1 w-36">
                      <button v-for="nt in nodeTypeOptions" :key="nt.id" @click.stop="switchNodeType(task, nt.id); editingTypeId = null"
                        class="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-teal-50 transition-colors text-left"
                        :class="getNodeTypeStr(task) === nt.id ? 'text-teal-700 bg-teal-50 font-medium' : 'text-gray-600'">
                        <component :is="nt.icon" :size="14" /> {{ nt.label }}
                      </button>
                    </div>
                  </div>
                  <div class="flex-1 min-w-0">
                    <div class="flex items-center justify-between mb-2">
                      <div class="flex items-center gap-2 flex-1">
                        <span class="text-[10px] font-bold px-1.5 py-0.5 bg-gray-100 text-gray-500 rounded uppercase tracking-wider">{{ getNodeLabel(task) }}</span>
                        <input v-model="task.name" class="font-medium text-gray-800 bg-transparent border-b border-transparent hover:border-gray-300 focus:border-teal-500 focus:outline-none transition-colors w-full px-1" placeholder="任务名称" />
                      </div>
                      <button @click="deleteTask(phase, tIdx)" class="text-gray-300 hover:text-red-500 transition-colors p-1 opacity-0 group-hover/task:opacity-100" title="删除任务">
                        <Trash2 :size="14" />
                      </button>
                    </div>
                    <div class="relative w-full">
                      <textarea v-if="['LlmTask', 'HumanCheckpoint'].includes(getNodeTypeStr(task))"
                        v-model="(task.node_type as any).config[getTaskDetailKey(task)]" rows="2" placeholder="任务指令/描述..."
                        class="w-full text-sm text-gray-600 bg-gray-50 border border-transparent hover:border-gray-300 focus:border-teal-500 rounded-lg p-2 focus:outline-none resize-none transition-all focus:bg-white"></textarea>
                      <div v-else-if="getNodeTypeStr(task) === 'ToolTask'" class="space-y-2">
                        <input v-model="(task.node_type as any).config.tool_name" class="w-full text-sm text-gray-600 font-mono bg-gray-50 border border-transparent hover:border-gray-300 focus:border-teal-500 rounded-lg p-2 focus:outline-none transition-all" placeholder="工具名称 (e.g. web_search)" />
                      </div>
                    </div>
                  </div>
                </div>
                <button @click="addTask(phase)" class="ml-6 flex items-center gap-2 text-sm text-gray-400 hover:text-teal-600 px-3 py-2 rounded-lg hover:bg-teal-50 border border-dashed border-gray-200 hover:border-teal-200 w-[calc(100%-1.5rem)] transition-all group/add">
                  <div class="w-6 h-6 rounded-full bg-gray-100 flex items-center justify-center group-hover/add:bg-teal-100 group-hover/add:text-teal-600 transition-colors"><Plus :size="14" /></div>
                  <span>添加新任务</span>
                </button>
              </div>
            </transition>
          </div>

          <div class="pl-1 mb-8">
            <button @click="addPhase" class="flex items-center gap-3 w-full p-4 rounded-xl border-2 border-dashed border-gray-200 hover:border-teal-300 hover:bg-teal-50/30 text-gray-400 hover:text-teal-600 transition-all group">
              <div class="w-12 h-12 rounded-full bg-gray-100 border-2 border-white flex items-center justify-center group-hover:bg-white group-hover:border-teal-200 transition-colors shadow-sm"><Plus :size="24" /></div>
              <span class="font-bold">添加新阶段</span>
            </button>
          </div>

          <div class="flex items-center gap-4 pl-1 pb-4 opacity-50">
            <div class="w-12 h-12 rounded-full bg-gray-100 flex items-center justify-center z-10 border-4 border-white shadow-sm"><CheckCircle2 class="text-gray-400" :size="20" /></div>
            <span class="text-gray-400 font-medium">规划结束</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
