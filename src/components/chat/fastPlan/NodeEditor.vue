<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { X, Brain, Wrench, User, Plus, Trash2, Save } from 'lucide-vue-next'
import type { TaskTreeNode } from './treeToDAG'

interface NodeType {
  id: string
  label: string
  icon: unknown
  color: string
  defaultConfig: Record<string, unknown>
}

const props = defineProps<{ node: TaskTreeNode | null; visible: boolean }>()
const emit = defineEmits<{ 'close': []; 'save': [node: TaskTreeNode] }>()

const editData = ref<TaskTreeNode | null>(null)
const dialogVisible = ref(false)

const nodeTypes: NodeType[] = [
  { id: 'LlmTask', label: 'AI 推理', icon: Brain, color: 'text-teal-600', defaultConfig: { prompt_template: '' } },
  { id: 'ToolTask', label: '工具调用', icon: Wrench, color: 'text-amber-600', defaultConfig: { tool_name: '', arguments: {} } },
  { id: 'HumanCheckpoint', label: '人工确认', icon: User, color: 'text-blue-600', defaultConfig: { prompt: '', options: ['continue', 'abort'] } },
]

watch(() => props.node, (newNode) => {
  if (newNode) editData.value = JSON.parse(JSON.stringify(newNode))
}, { immediate: true, deep: true })

watch(() => props.visible, (newVal) => { dialogVisible.value = newVal })

const currentType = computed(() => (editData.value?.node_type as any)?.type || 'LlmTask')

function switchType(typeId: string) {
  if (!editData.value) return
  const typeCfg = nodeTypes.find(t => t.id === typeId)
  if (!typeCfg) return
  ;(editData.value as any).node_type = { type: typeId, config: { ...typeCfg.defaultConfig } }
}

const argEntries = computed<[string, unknown][]>(() => {
  if (currentType.value !== 'ToolTask') return []
  const args = (editData.value?.node_type as any)?.config?.arguments
  if (!args) return []
  return Object.entries(args)
})

function addArg() {
  if (!editData.value?.node_type) return
  const config = (editData.value.node_type as any).config
  if (!config) return
  if (!config.arguments) config.arguments = {}
  config.arguments[`arg_${Date.now()}`] = ''
}

function removeArg(key: string) {
  const args = (editData.value?.node_type as any)?.config?.arguments
  if (args) delete args[key]
}

function updateArgKey(oldKey: string, newKey: string) {
  const args = (editData.value?.node_type as any)?.config?.arguments
  if (!args) return
  const val = args[oldKey]
  delete args[oldKey]
  args[newKey] = val
}

function handleSave() {
  if (editData.value) emit('save', editData.value)
}
</script>

<template>
  <el-dialog v-model="dialogVisible" title="编辑节点" width="40%" destroy-on-close align-center @close="$emit('close')">
    <div class="w-full max-h-[80vh] flex flex-col overflow-hidden">
      <div class="flex-1 overflow-y-auto p-4 space-y-4">
        <div>
          <label class="font-medium text-gray-600 mb-1.5 block">节点名称</label>
          <input v-if="editData" v-model="editData.name"
            class="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-2 focus:ring-teal-500/20 focus:border-teal-400 outline-none transition-all"
            placeholder="请输入节点名称" />
        </div>
        <div>
          <label class="font-medium text-gray-600 mb-1.5 block">节点类型</label>
          <div class="flex gap-2">
            <button v-for="nt in nodeTypes" :key="nt.id" @click="switchType(nt.id)"
              class="flex-1 flex items-center justify-center gap-1.5 px-3 py-2.5 rounded-lg border font-medium transition-all"
              :class="currentType === nt.id ? 'border-teal-300 bg-teal-50 text-teal-700 shadow-sm' : 'border-gray-200 text-gray-600 hover:bg-gray-50'">
              <component :is="nt.icon" :size="14" /> {{ nt.label }}
            </button>
          </div>
        </div>
        <div v-if="editData && currentType === 'LlmTask'">
          <label class="font-medium text-gray-600 mb-1.5 block">Prompt 模板</label>
          <textarea v-model="(editData.node_type as any).config.prompt_template" rows="4"
            class="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-2 focus:ring-teal-500/20 focus:border-teal-400 outline-none resize-none transition-all"
            placeholder="请输入任务指令..." />
        </div>
        <div v-if="editData && currentType === 'HumanCheckpoint'" class="space-y-3">
          <div>
            <label class="font-medium text-gray-600 mb-1.5 block">确认提示语</label>
            <textarea v-model="(editData.node_type as any).config.prompt" rows="2"
              class="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-2 focus:ring-teal-500/20 focus:border-teal-400 outline-none resize-none transition-all"
              placeholder="请输入提示信息..." />
          </div>
          <div>
            <label class="font-medium text-gray-600 mb-1.5 block">选项 (逗号分隔)</label>
            <input :value="((editData.node_type as any).config.options || []).join(', ')"
              @input="(editData!.node_type as any).config.options = ($event.target as HTMLInputElement).value.split(',').map((s: string) => s.trim()).filter(Boolean)"
              class="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-2 focus:ring-teal-500/20 focus:border-teal-400 outline-none transition-all"
              placeholder="continue, abort" />
          </div>
        </div>
        <div v-if="editData && currentType === 'ToolTask'" class="space-y-3">
          <div>
            <label class="font-medium text-gray-600 mb-1.5 block">工具名称</label>
            <input v-model="(editData.node_type as any).config.tool_name"
              class="w-full px-3 py-2 border border-gray-200 rounded-lg focus:ring-2 focus:ring-teal-500/20 focus:border-teal-400 outline-none transition-all"
              placeholder="e.g. web_search" />
          </div>
          <div>
            <div class="flex items-center justify-between mb-1.5">
              <label class="font-medium text-gray-600">参数 (Key-Value)</label>
              <button @click="addArg" class="text-teal-600 hover:text-teal-700 flex items-center gap-0.5">
                <Plus :size="14" /> 添加
              </button>
            </div>
            <div class="space-y-2">
              <div v-for="[key, val] in argEntries" :key="key" class="flex items-center gap-2">
                <input :value="key" @change="updateArgKey(key, ($event.target as HTMLInputElement).value)"
                  class="flex-1 px-2 py-1.5 border border-gray-200 rounded focus:border-teal-400 outline-none" placeholder="key" />
                <input :value="String(val)" @input="(editData!.node_type as any).config.arguments[key] = ($event.target as HTMLInputElement).value"
                  class="flex-1 px-2 py-1.5 border border-gray-200 rounded focus:border-teal-400 outline-none" placeholder="value" />
                <button @click="removeArg(key)" class="p-1 text-gray-400 hover:text-red-500 transition-colors">
                  <Trash2 :size="16" />
                </button>
              </div>
              <div v-if="argEntries.length === 0" class="text-gray-400 text-center py-2">暂无参数</div>
            </div>
          </div>
        </div>
      </div>
    </div>
    <template #footer>
      <el-button @click="$emit('close')">取消</el-button>
      <el-button type="primary" @click="handleSave">保存</el-button>
    </template>
  </el-dialog>
</template>
