<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { useI18n } from 'vue-i18n'
import SvgIcon from "@/components/svgIcon/index.vue"
import SearchClearButton from '@/components/common/SearchClearButton.vue'
import { deleteModels, fetchRemoteModels as loadRemoteModels, saveRemoteModels } from '@/modules/models/api'
import type { RemoteModel } from '@/modules/models/types/contracts'

const { t } = useI18n()

const props = defineProps<{
  visible: boolean
  providerCode: string
  providerName: string
}>()

const emit = defineEmits<{
  (e: 'update:visible', value: boolean): void
  (e: 'saved'): void
}>()

const remoteModels = ref<RemoteModel[]>([])
const loadingRemoteModels = ref(false)
const selectedRemoteModelIds = ref<Set<string>>(new Set())
const remoteSearchQuery = ref('')
const remoteModelTab = ref('all')
const tabs = [
  { label: t('modelManage.all'), value: 'all' },
  { label: t('modelManage.chat'), value: 'chat' },
  { label: t('modelManage.image'), value: 'image' },
  { label: t('modelManage.tts'), value: 'tts' },
  { label: t('modelManage.tool'), value: 'tool' },
  { label: t('modelManage.embedding'), value: 'embedding' },
  { label: t('modelManage.rerank'), value: 'rerank' },
]

const filteredRemoteModels = computed(() => {
  let list = remoteModels.value
  if (remoteSearchQuery.value) {
    const q = remoteSearchQuery.value.toLowerCase()
    list = list.filter(m =>
      (m.model_name || m.id || '').toLowerCase().includes(q) ||
      (m.model_id || '').toLowerCase().includes(q)
    )
  }
  if (remoteModelTab.value !== 'all') {
    list = list.filter(m => matchModelTab(m, remoteModelTab.value))
  }
  return list
})

/** 根据 model_label 标签数据匹配 tab */
function matchModelTab(m: RemoteModel, tab: string): boolean {
  const label = m.model_label || ''
  const labels = splitLabels(label)

  // tab → 对应标签关键词（中文一对一映射）
  const tabLabelMap: Record<string, string> = {
    chat: '推理',
    image: '视觉',
    tts: '语音',
    tool: '工具',
    embedding: '嵌入',
    rerank: '重排',
  }

  const keyword = tabLabelMap[tab]
  return keyword ? labels.includes(keyword) : false
}

async function fetchRemoteModels() {
  loadingRemoteModels.value = true
  remoteModels.value = []
  selectedRemoteModelIds.value.clear()
  remoteSearchQuery.value = ''
  remoteModelTab.value = 'all'

  try {
    const res = await loadRemoteModels(props.providerCode)
    remoteModels.value = res || []
    // 自动选中已存在的模型
    res?.forEach(m => {
      if (m._exists) selectedRemoteModelIds.value.add(m.id)
    })
  } catch (e: any) {
    console.warn('Fetch remote models failed:', e)
    ElMessage.error(t('modelManage.fetchRemoteFail', { msg: e.message || String(e) }))
  } finally {
    loadingRemoteModels.value = false
  }
}

watch(() => props.visible, (val) => {
  if (val && props.providerCode) {
    fetchRemoteModels()
  }
})

function toggleRemoteModel(id: string) {
  if (selectedRemoteModelIds.value.has(id)) {
    selectedRemoteModelIds.value.delete(id)
  } else {
    selectedRemoteModelIds.value.add(id)
  }
}

async function saveSelectedModels() {
  if (!props.providerCode) return

  const selectedIds = selectedRemoteModelIds.value

  // 需要保存的模型（当前选中）
  const modelsToSave = remoteModels.value
    .filter(m => selectedIds.has(m.id))
    .map(m => ({ ...m, is_enabled: true }))

  // 需要删除的模型（之前存在但当前未选中）
  const modelsToDelete = remoteModels.value
    .filter(m => m._exists && !selectedIds.has(m.id))
    .map(m => m.id)

  try {
    // 增量保存
    if (modelsToSave.length > 0) {
      await saveRemoteModels(props.providerCode, modelsToSave)
    }
    // 删除取消勾选的已有模型
    if (modelsToDelete.length > 0) {
      await deleteModels(modelsToDelete)
    }
    emit('saved')
    close()
    ElMessage.success(t('common.saveSuccess'))
  } catch (e: any) {
    console.error('Save models failed:', e)
    ElMessage.error(t('modelManage.saveModelFail', { msg: e.message || String(e) }))
  }
}

function selectAll() {
  filteredRemoteModels.value.forEach(m => selectedRemoteModelIds.value.add(m.id))
}

function deselectAll() {
  selectedRemoteModelIds.value.clear()
}

const isAllSelected = computed(() =>
  filteredRemoteModels.value.length > 0 &&
  filteredRemoteModels.value.every(m => selectedRemoteModelIds.value.has(m.id))
)

function splitLabels(label: string): string[] {
  if (!label) return []
  return label.split(/[,，]/).map(s => s.trim()).filter(s => s.length > 0)
}

// ─── 标签语义配色（与 ModelCard 一致） ───
const CHIP_PALETTES = ['blue', 'purple', 'orange', 'green', 'teal', 'rose', 'indigo'] as const

const CHIP_KEYWORDS: Array<[RegExp, (typeof CHIP_PALETTES)[number]]> = [
  [/对话|chat|dialog/i, 'blue'],
  [/视觉|多模态|图像|vision|image|multimodal|vl/i, 'purple'],
  [/推理|思考|reason|think/i, 'orange'],
  [/工具|函数|tool|function/i, 'green'],
  [/嵌入|embed/i, 'teal'],
  [/重排|rerank/i, 'rose'],
  [/语音|voice|audio|tts|stt|asr/i, 'indigo'],
]

function chipClass(text: string): string {
  for (const [re, palette] of CHIP_KEYWORDS) {
    if (re.test(text)) return `m-chip-${palette}`
  }
  let hash = 0
  for (let i = 0; i < text.length; i++) hash = (hash * 31 + text.charCodeAt(i)) >>> 0
  return `m-chip-${CHIP_PALETTES[hash % CHIP_PALETTES.length]}`
}

function close() {
  emit('update:visible', false)
}
</script>

<template>
  <div v-if="visible" class="overlay">
    <div class="dialog large-dialog">
      <div class="dialog-header">
        <h3>{{ $t('modelManage.providerModelTitle', { name: providerName }) }}</h3>
        <button class="close-btn" @click="close">
          <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </div>
      <div class="model-search-bar">
        <div class="search-input-wrap">
          <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none"
            class="search-icon">
            <circle cx="11" cy="11" r="8"></circle>
            <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
          </svg>
          <input type="text" v-model="remoteSearchQuery" :placeholder="$t('modelManage.searchModelPlaceholder')"
            :class="{ 'has-clear': remoteSearchQuery }" />
          <SearchClearButton v-if="remoteSearchQuery" overlay @clear="remoteSearchQuery = ''" />
        </div>
        <button class="btn-refresh" @click="fetchRemoteModels"><svg viewBox="0 0 24 24" width="16" height="16"
            stroke="currentColor" stroke-width="2" fill="none">
            <polyline points="23 4 23 10 17 10"></polyline>
            <polyline points="1 20 1 14 7 14"></polyline>
            <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
          </svg></button>
      </div>
      <div class="model-tabs">
        <template v-for="tab in tabs" :key="tab.value">
          <span class="tab" :class="{ active: remoteModelTab === tab.value }" @click="remoteModelTab = tab.value">
            {{ tab.label }}
          </span>
        </template>
      </div>

      <div v-if="filteredRemoteModels.length > 0" class="select-bar"
        style="display:flex;align-items:center;justify-content:space-between;padding:8px 24px;">
        <button class="btn-select-all" @click="isAllSelected ? deselectAll() : selectAll()"
          style="font-size:var(--font-size-sm);cursor:pointer;border:1px solid var(--border-color);border-radius:4px;padding:4px 12px;background:transparent;color:var(--text-primary);">
          {{ isAllSelected ? $t('modelManage.deselectAll') : $t('modelManage.selectAll') }}
        </button>
        <span style="font-size:var(--font-size-sm);color:var(--text-secondary);">
          {{ $t('modelManage.selectedCount', { n: selectedRemoteModelIds.size }) }}
        </span>
      </div>

      <div v-if="loadingRemoteModels" class="empty-state">
        <p>{{ $t('modelManage.loadingModels') }}</p>
      </div>
      <div v-else-if="filteredRemoteModels.length > 0" class="remote-model-list"
        style="flex: 1; overflow-y: auto; padding: 16px 24px;">
        <div v-for="m in filteredRemoteModels" :key="m.id" class="remote-model-item"
          :class="{ selected: selectedRemoteModelIds.has(m.id) }" @click="toggleRemoteModel(m.id)"
          style="display: flex; align-items: center; gap: 12px; padding: 12px; border: 1px solid var(--border-color); border-radius: 8px; margin-bottom: 8px; cursor: pointer;"
          :style="selectedRemoteModelIds.has(m.id) ? 'border-color: var(--color-primary); background: var(--bg-hover);' : ''">
          <div class="m-checkbox"
            style="width: 18px; height: 18px; border: 1px solid var(--border-color); border-radius: 4px; display: flex; align-items: center; justify-content: center;"
            :style="selectedRemoteModelIds.has(m.id) ? 'border-color: var(--color-primary); background: var(--color-primary);' : ''">
            <svg v-if="selectedRemoteModelIds.has(m.id)" viewBox="0 0 24 24" width="14" height="14" stroke="white"
              stroke-width="2" fill="none">
              <polyline points="20 6 9 17 4 12"></polyline>
            </svg>
          </div>
          <div class="m-info" style="flex:1;">
            <span class="m-title"
              style="font-weight: 500; font-size: var(--font-size-base); color: var(--text-primary);">
              {{ m.model_name || m.id }}</span>
          </div>
          <span v-if="m.model_label" class="m-labels">
            <span v-for="tag in splitLabels(m.model_label)" :key="tag" class="m-chip" :class="chipClass(tag)">
              <el-icon :size="14"><svg-icon :iconName="tag" /></el-icon>
              {{ tag }}
            </span>
          </span>
        </div>
      </div>
      <div v-else class="empty-state">
        <div class="empty-icon">
          <svg viewBox="0 0 24 24" width="48" height="48" stroke="currentColor" stroke-width="1" fill="none">
            <path
              d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z">
            </path>
            <polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline>
            <line x1="12" y1="22.08" x2="12" y2="12"></line>
          </svg>
        </div>
        <p>{{ $t('modelManage.noModelsFound') }}</p>
      </div>

      <div class="actions model-picker-actions" v-if="filteredRemoteModels.length > 0">
        <button class="btn-cancel" @click="close">{{ $t('common.cancel') }}</button>
        <button class="btn-primary" @click="saveSelectedModels" :disabled="selectedRemoteModelIds.size === 0"
          :style="{ opacity: selectedRemoteModelIds.size === 0 ? 0.5 : 1 }">
          {{ $t('common.save') }}
        </button>
      </div>
    </div>
  </div>
</template>
<style src="./dialog-common.css"></style>
<style scoped>
.large-dialog {
  height: 90vh;
}

.model-search-bar {
  padding: 15px 20px 0px 20px;
  display: flex;
  gap: 8px;
  align-items: center;
}

.search-input-wrap {
  position: relative;
  flex: 1;
  display: flex;
  align-items: center;
  gap: 8px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 6px 12px;
}

.search-input-wrap input {
  border: none;
  outline: none;
  width: 100%;
  background: transparent;
  color: var(--text-primary);
}

.search-input-wrap input.has-clear {
  padding-right: 28px;
}

.search-icon {
  color: var(--text-secondary);
}

.model-search-bar button:not(.skills-search-clear) {
  padding: 8px;
  background: transparent;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  cursor: pointer;
  color: var(--text-secondary);
  display: flex;
  align-items: center;
  justify-content: center;
}

.model-search-bar button:not(.skills-search-clear):hover {
  background: var(--bg-hover);
}

.model-tabs {
  padding: 0 24px;
  display: flex;
  gap: 24px;
  border-bottom: 1px solid var(--border-color);
}

.tab {
  padding: 12px 8px;
  font-size: var(--font-size-base);
  color: var(--text-secondary);
  cursor: pointer;
  position: relative;
  border-radius: 4px;
  transition: background 0.2s;
}

.tab:hover {
  background: var(--dropdown-hover-color);
}

.tab.active {
  color: var(--color-primary);
  font-weight: 500;
}

.tab.active::after {
  content: '';
  position: absolute;
  bottom: -1px;
  left: 0;
  right: 0;
  height: 2px;
  background: var(--color-primary);
}

.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: var(--text-secondary);
}

.empty-icon {
  opacity: 0.2;
  margin-bottom: 16px;
}

.m-labels {
  flex-shrink: 0;
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.m-chip {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  height: 22px;
  padding: 0 9px;
  font-size: 12px;
  font-weight: 500;
  line-height: 1;
  border-radius: 7px;
  border: 1px solid transparent;
  white-space: nowrap;
}

/* 语义配色（与 ModelCard 一致） */
.m-chip-blue   { color: #2563eb; background: #eff5ff; border-color: #c7dbff; }
.m-chip-purple { color: #7c3aed; background: #f5f1ff; border-color: #ddd0fb; }
.m-chip-orange { color: #ea7517; background: #fff5eb; border-color: #ffdcb8; }
.m-chip-green  { color: #15a34a; background: #ecfdf3; border-color: #b9eccb; }
.m-chip-teal   { color: #0d9488; background: #effcfa; border-color: #b3ece6; }
.m-chip-rose   { color: #e11d63; background: #fff1f6; border-color: #fbcfdf; }
.m-chip-indigo { color: #4f46e5; background: #eef0ff; border-color: #cdd2fb; }

.close-btn {
  width: 28px;
  height: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  border-radius: 5px;
  cursor: pointer;
  color: var(--text-secondary);
  padding: 0;
  transition: background-color 0.18s ease, color 0.18s ease;
}

.close-btn:hover {
  background: var(--dropdown-hover-color, var(--bg-hover)) !important;
  color: var(--text-primary);
}

.model-picker-actions {
  padding: 16px 24px;
  border-top: 1px solid var(--border-color);
  margin-top: auto;
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 15px;
}

.btn-cancel,
.btn-primary {
  display: inline-flex !important;
  align-items: center !important;
  justify-content: center !important;
  gap: 0 !important;
  width: 64px !important;
  height: 37px !important;
  min-width: 64px !important;
  min-height: 37px !important;
  padding: 0 !important;
  margin: 0 !important;
  box-sizing: border-box !important;
  border-radius: 4px;
  cursor: pointer;
  font-size: var(--font-size-md);
  font-weight: 500;
  transition: all 0.2s;
}

.btn-cancel {
  color: var(--btn-secondary-text) !important;
  background: var(--cancel-btn-color) !important;
  border: 1px solid var(--btn-secondary-border) !important;
}

.btn-cancel:hover {
  background: var(--cancel-btn-hover-color) !important;
  color: var(--btn-secondary-hover-text) !important;
  border-color: var(--btn-secondary-hover-border) !important;
}

.btn-primary {
  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);
  border: 1px solid var(--btn-primary-border, transparent);
}

.btn-primary:hover:not(:disabled) {
  background: var(--btn-primary-hover-bg, var(--color-primary));
  color: var(--btn-primary-hover-text, #fff);
  border-color: var(--btn-primary-hover-border, transparent);
}

.large-dialog .dialog-header {
  padding: 16px 24px;
  border-bottom: 1px solid var(--border-color);
  margin-bottom: 0;
}
</style>
