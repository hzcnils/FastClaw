<!-- 规划树执行前的文件上传引导面板
     读取 tree.metadata.input_slots,提示用户上传每个所需文件,传齐后才放行执行。
     若树没有 input_slots 声明,则不拦截(直接显示"无需上传文件,可执行")。 -->
<script setup>
import { ref, computed, inject } from 'vue'
import { Upload, Loader2, Check, FileText, Table, AlertCircle } from 'lucide-vue-next'
import { uploadFastPlanFile, FASTPLAN_ACCESS_KEY } from 'planflow-runtime'
const access = inject(FASTPLAN_ACCESS_KEY, undefined)

const props = defineProps({
  tree: { type: Object, required: true },
  apiBase: { type: String, default: '' },
})
const emit = defineEmits(['ready', 'files-change'])

const API_BASE = props.apiBase

// 从树读输入槽
const slots = computed(() => {
  const declared = props.tree?.metadata?.input_slots
  if (Array.isArray(declared) && declared.length) {
    return declared.map(s => ({
      key: `${s.node_id}.${s.arg_key}`,
      label: s.label || s.arg_key,
      accept: (s.accept || []).map(e => e.toLowerCase()),
    }))
  }
  return []
})

// 每个槽对应已上传文件 { [slotKey]: { name, path } }
const uploaded = ref({})
const uploadingKey = ref(null)
const errorKey = ref(null)

const allReady = computed(() =>
  slots.value.length === 0 || slots.value.every(s => uploaded.value[s.key])
)

function iconFor(accept) {
  if (accept.some(e => ['xlsx', 'xls'].includes(e))) return Table
  return FileText
}
function acceptAttr(accept) {
  return accept.map(e => '.' + e).join(',')
}

async function onPick(slot, e) {
  const file = e.target.files?.[0]
  if (!file) return
  uploadingKey.value = slot.key
  errorKey.value = null
  try {
    const { promise } = uploadFastPlanFile(file, { apiBase: API_BASE, access })
    const { path } = await promise
    uploaded.value[slot.key] = { name: file.name, path }
    emitFiles()
  } catch (err) {
    errorKey.value = slot.key
  } finally {
    uploadingKey.value = null
    e.target.value = ''
  }
}

function emitFiles() {
  // 收集所有已上传文件 [{ name, path }] 给父组件注入树
  const files = Object.values(uploaded.value)
  emit('files-change', files)
  emit('ready', allReady.value)
}

defineExpose({
  getFiles: () => Object.values(uploaded.value),
  isReady: () => allReady.value,
})
</script>

<template>
  <div v-if="slots.length" class="fpp-ui bg-white border border-gray-200 rounded-xl p-4 space-y-3">
    <div class="flex items-center gap-2 text-sm font-semibold text-gray-800">
      <Upload :size="16" class="text-blue-600" />
      执行前请上传以下文件
    </div>
    <div class="space-y-2">
      <div v-for="slot in slots" :key="slot.key"
           class="flex items-center gap-3 p-3 rounded-lg border transition-colors"
           :class="uploaded[slot.key] ? 'border-green-200 bg-green-50/50' : 'border-gray-200 bg-gray-50'">
        <component :is="iconFor(slot.accept)" :size="18"
                   :class="uploaded[slot.key] ? 'text-green-600' : 'text-gray-400'" />
        <div class="flex-1 min-w-0">
          <div class="text-sm font-medium text-gray-800">{{ slot.label }}</div>
          <div class="text-xs text-gray-400 truncate">
            {{ uploaded[slot.key] ? uploaded[slot.key].name : '支持 ' + slot.accept.join(' / ') }}
          </div>
        </div>
        <Loader2 v-if="uploadingKey === slot.key" :size="16" class="animate-spin text-blue-500" />
        <Check v-else-if="uploaded[slot.key]" :size="18" class="text-green-600" />
        <label v-else class="cursor-pointer px-3 py-1.5 text-xs font-medium text-blue-600 border border-blue-200 rounded-lg hover:bg-blue-50 transition-colors">
          上传
          <input type="file" :accept="acceptAttr(slot.accept)" class="hidden" @change="(e) => onPick(slot, e)" />
        </label>
      </div>
    </div>
    <div v-if="errorKey" class="text-xs text-red-500 flex items-center gap-1">
      <AlertCircle :size="12" /> 上传失败,请重试
    </div>
    <div v-if="!allReady" class="text-xs text-amber-600 flex items-center gap-1">
      <AlertCircle :size="12" /> 还有文件未上传,上传齐全后方可执行
    </div>
  </div>
</template>
