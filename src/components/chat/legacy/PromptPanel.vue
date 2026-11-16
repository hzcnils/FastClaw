<script setup lang="ts">
// 提示词面板：我的提示词 + 广场浏览 + 搜索/分类筛选
import { ref, onMounted, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElInput, ElSelect, ElOption, ElButton, ElIcon, ElTabs, ElTabPane, ElEmpty } from 'element-plus'
import { Plus, Search, Close, Download, Check } from '@element-plus/icons-vue'
import type { PromptTemplate } from '@/types'
import { useThemeStore } from '@/stores/theme'

const { t } = useI18n()

const themeStore = useThemeStore()

// 弹窗背景样式随主题变化
const modalStyle = computed(() => {
  const id = themeStore.currentPreset.id
  const p = themeStore.currentPreset.palette
  const r: Record<string, string> = {}
  const bg = p?.modalBg || p?.bgPrimary || '#ffffff'
  if (id === 'simple-dark' || id.startsWith('dark-') || (id.startsWith('scene-') && id !== 'scene-graphite-cyan')) {
    r['background-color'] = bg
    r['border'] = '1px solid rgba(255,255,255,0.15)'
    r['color'] = '#fff'
  } else if (id === 'scene-graphite-cyan') {
    r['background-color'] = bg
    r['border'] = '1px solid var(--border-color)'
    r['color'] = 'var(--text-primary)'
  } else {
    r['background-color'] = bg
    r['border'] = '1px solid var(--border-color)'
    r['color'] = 'var(--text-primary)'
  }
  return r
})

const emit = defineEmits<{
  'apply': [content: string]
  'close': []
}>()

type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>
let _invoke: InvokeFn | null = null
async function getInvoke(): Promise<InvokeFn | null> {
  if (_invoke) return _invoke
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    _invoke = invoke as InvokeFn
    return _invoke
  } catch { return null }
}

const activeTab = ref<'mine' | 'square'>('mine')
const searchStr = ref('')
const selectedPromptId = ref('')
const allPrompts = ref<PromptTemplate[]>([])
const myPrompts = ref<PromptTemplate[]>([])
const loading = ref(false)

const quickTitles = computed(() => {
  const list = activeTab.value === 'mine' ? myPrompts.value : allPrompts.value
  return list.map(p => ({ id: p.id, title: p.title }))
})

const filteredPrompts = computed(() => {
  const list = activeTab.value === 'mine' ? myPrompts.value : allPrompts.value
  const kw = searchStr.value.trim().toLowerCase()
  return list.filter(p => {
    const matchKw = !kw || p.title.toLowerCase().includes(kw) || (p.description || '').toLowerCase().includes(kw)
    const matchPrompt = !selectedPromptId.value || p.id === selectedPromptId.value
    return matchKw && matchPrompt
  })
})

async function loadAll() {
  const invoke = await getInvoke()
  if (!invoke) return
  loading.value = true
  try {
    const [all, my] = await Promise.all([
      invoke<PromptTemplate[]>('list_prompts', {}),
      invoke<PromptTemplate[]>('list_my_prompts', {}),
    ])
    allPrompts.value = all || []
    myPrompts.value = my || []
  } catch (e) {
    console.warn('load prompts failed', e)
  } finally {
    loading.value = false
  }
}

async function applyPrompt(p: PromptTemplate) {
  const invoke = await getInvoke()
  let content = p.content
  if (invoke) {
    try {
      content = await invoke<string>('apply_prompt', { promptId: p.id })
    } catch { /* use local content */ }
  }
  emit('apply', content)
}

async function downloadPrompt(p: PromptTemplate) {
  const invoke = await getInvoke()
  if (!invoke) return
  try {
    await invoke('download_prompt', { promptId: p.id })
    ElMessage.success(t('chat.promptCreateSuccess'))
    await loadAll()
  } catch (e) {
    ElMessage.error(t('chat.promptCreateFail'))
  }
}

const isInMine = (p: PromptTemplate) => myPrompts.value.some(m => m.id === p.id)

// 新建提示词
const createDialogVisible = ref(false)
const createForm = ref({ title: '', content: '', description: '', category: '' })
const createLoading = ref(false)
async function submitCreate() {
  if (!createForm.value.title.trim() || !createForm.value.content.trim()) {
    ElMessage.warning(t('chat.promptTitleContentRequired'))
    return
  }
  const invoke = await getInvoke()
  if (!invoke) return
  createLoading.value = true
  try {
    await invoke('create_prompt', {
      title: createForm.value.title.trim(),
      content: createForm.value.content.trim(),
      description: createForm.value.description.trim() || null,
      category: createForm.value.category.trim() || null,
    })
    ElMessage.success(t('chat.promptCreateOk'))
    createDialogVisible.value = false
    createForm.value = { title: '', content: '', description: '', category: '' }
    await loadAll()
  } catch (e) {
    ElMessage.error(t('chat.promptCreateError'))
  } finally {
    createLoading.value = false
  }
}

async function deletePrompt(p: PromptTemplate) {
  const invoke = await getInvoke()
  if (!invoke) return
  try {
    await invoke('delete_prompt', { id: p.id })
    ElMessage.success(t('chat.promptDeleteSuccess'))
    await loadAll()
  } catch (e: any) {
    ElMessage.error(e?.message || t('chat.promptDeleteFail'))
  }
}

onMounted(loadAll)
</script>

<template>
  <div class="prompt-panel">
    <!-- 搜索 + 新建 -->
    <div class="panel-toolbar">
      <el-input v-model="searchStr" :placeholder="t('chat.promptSearch')" size="small" clearable :prefix-icon="Search"
        style="flex: 1;" />
      <el-button size="small" type="primary" :icon="Plus" @click="createDialogVisible = true">{{ t('chat.promptCreate') }}</el-button>
    </div>

    <!-- 标题快捷筛选 -->
    <div class="panel-cats" v-if="quickTitles.length">
      <span :class="['cat-tag', !selectedPromptId ? 'active' : '']" @click="selectedPromptId = ''">{{ t('chat.promptAll') }}</span>
      <span v-for="t in quickTitles" :key="t.id" :class="['cat-tag', selectedPromptId === t.id ? 'active' : '']"
        @click="selectedPromptId = t.id">{{ t.title }}</span>
    </div>

    <!-- Tabs：我的 / 广场 -->
    <el-tabs v-model="activeTab" class="panel-tabs">
      <el-tab-pane :label="t('chat.promptMine')" name="mine" />
      <el-tab-pane :label="t('chat.promptSquare')" name="square" />
    </el-tabs>

    <!-- 列表 -->
    <div class="prompt-list" v-loading="loading">
      <el-empty v-if="!filteredPrompts.length" :description="t('chat.promptEmpty')" :image-size="60" />
      <div v-for="p in filteredPrompts" :key="p.id" class="prompt-item">
        <div class="prompt-meta">
          <span class="prompt-title">{{ p.title }}</span>
          <span v-if="p.category" class="prompt-cat">{{ p.category }}</span>
        </div>
        <div v-if="p.description" class="prompt-desc">{{ p.description }}</div>
        <div class="prompt-actions">
          <el-button size="small" type="primary" text @click="applyPrompt(p)">{{ t('chat.promptUse') }}</el-button>
          <el-button v-if="activeTab === 'square' && !isInMine(p)" size="small" text :icon="Download"
            @click="downloadPrompt(p)">{{ t('chat.promptAdd') }}</el-button>
          <el-button v-if="activeTab === 'square' && isInMine(p)" size="small" text :icon="Check"
            disabled>{{ t('chat.promptAdded') }}</el-button>
          <el-button v-if="p.source === 'user'" size="small" text type="danger" @click="deletePrompt(p)">{{ t('chat.promptDelete') }}</el-button>
        </div>
      </div>
    </div>

    <!-- 新建弹窗 -->
    <div v-if="createDialogVisible" class="modal-overlay" @click.self="createDialogVisible = false">
      <div class="modal-box" :style="modalStyle" :key="themeStore.currentPreset.id">
        <div class="modal-header">
          <h3>{{ t('chat.promptCreateTitle') }}</h3>
          <button class="modal-close" @click="createDialogVisible = false">
            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
        <div class="modal-body">
          <div class="form-item">
            <label>{{ t('chat.promptTitleLabel') }}</label>
            <input v-model="createForm.title" :placeholder="t('chat.promptTitlePlaceholder')" />
          </div>
          <div class="form-item">
            <label>{{ t('chat.promptCategoryLabel') }}</label>
            <input v-model="createForm.category" :placeholder="t('chat.promptCategoryPlaceholder')" />
          </div>
          <div class="form-item">
            <label>{{ t('chat.promptDescLabel') }}</label>
            <input v-model="createForm.description" :placeholder="t('chat.promptDescPlaceholder')" />
          </div>
          <div class="form-item">
            <label>{{ t('chat.promptContentLabel') }}</label>
            <textarea v-model="createForm.content" rows="6" :placeholder="t('chat.promptContentPlaceholder')"></textarea>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn-secondary" @click="createDialogVisible = false">{{ t('chat.promptCancel') }}</button>
          <button class="btn-primary" :disabled="createLoading" @click="submitCreate">
            {{ createLoading ? t('chat.promptCreating') : t('chat.promptSubmit') }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.prompt-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.panel-toolbar {
  display: flex;
  gap: 8px;
  padding: 0 0 10px;
}

.panel-cats {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: 10px;
}

.cat-tag {
  padding: 2px 10px;
  border-radius: 12px;
  border: 1px solid var(--border-color);
  font-size: 12px;
  cursor: pointer;
  color: var(--text-secondary);
  background: var(--bg-primary);

  &.active {
    // border-color: var(--color-primary);
    // color: var(--color-primary);
    // background: color-mix(in srgb, var(--color-primary) 8%, transparent);
    //  border-color: var(--color-primary);
    border-color:var(--color-primary);
    color: #fff;
    background: var(--color-primary);
  }

  &:hover {
    border-color:var(--color-primary);
    background:var(--color-primary);
    color: #fff;
  }
}

.panel-tabs {
  flex-shrink: 0;
}

.prompt-list {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-top: 4px;
}

.prompt-item {
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 10px 12px;
  background: var(--bg-primary);

  &:hover {
    border-color: var(--color-primary);
  }
}

.prompt-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 4px;
}

.prompt-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
}

.prompt-cat {
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--bg-hover);
  color: var(--text-secondary);
}

.prompt-desc {
  font-size: 12px;
  color: var(--text-secondary);
  margin-bottom: 6px;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}

.prompt-actions {
  display: flex;
  gap: 1px;
}

/* 搜索框样式 - 背景透明，边框使用分割线颜色 */
:deep(.el-input__wrapper) {
  background: transparent !important;
  box-shadow: 0 0 0 1px var(--border-color) inset !important;
}

:deep(.el-input__inner) {
  background: transparent !important;
}

/* el-tabs 未选中标签文字颜色 */
:deep(.el-tabs__item) {
  color: var(--text-secondary);
}

/* ===== 新建提示词弹窗样式（与知识库创建弹窗一致） ===== */
.modal-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  background: rgba(15, 23, 42, 0.15);
  backdrop-filter: blur(0px);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}

.modal-box {
  border-radius: 18px;
  box-shadow: 0 24px 70px rgba(15, 23, 42, 0.24);
  width: 480px;
  max-width: 95%;
  max-height: 90vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 18px 24px;
  border-bottom: 1px solid var(--border-color);
  background: transparent;

  h3 {
    font-size: 17px;
    font-weight: 600;
    color: inherit;
  }
}

.modal-close {
  border: none;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  padding: 4px;
  border-radius: 6px;

  &:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }
}

.modal-body {
  padding: 15px 20px 10px 20px;
  overflow-y: auto;
  background: transparent;
  flex: 1;
}

.form-item {
  margin-bottom: 10px;

  label {
    display: block;
    font-size: 13px;
    font-weight: 500;
    margin-bottom: 6px;
    color: inherit;
  }

  input,
  textarea {
    width: 100%;
    padding: 8px 12px;
    border: 1px solid var(--border-color);
    border-radius: 6px;
    background: var(--bg-primary);
    color: var(--text-primary);
    font-size: 13px;
    transition: border-color 0.2s;

    &:focus {
      outline: none;
      border-color: var(--color-primary);
    }

    &::placeholder {
      color: var(--text-tertiary);
    }
  }

  textarea {
    resize: vertical;
    min-height: 120px;
  }
}

/* 黑暗主题下输入框背景透明 */
html.dark .modal-box .form-item input,
html.dark .modal-box .form-item textarea {
  background: transparent !important;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  background: transparent;
  padding: 18px 24px;
  border-top: 1px solid var(--border-color);
}

.btn-secondary {
  padding: 8px 18px;
  border: 1px solid var(--btn-secondary-border);
  border-radius: 4px;
  background: var(--btn-secondary-bg);
  font-size: var(--font-size-md);
  color: var(--btn-secondary-text);
  cursor: pointer;
  transition: all 0.18s ease;

  &:hover {
    background: var(--btn-secondary-hover-bg);
    color: var(--btn-secondary-hover-text);
    border-color: var(--btn-secondary-hover-border);
  }
}

.btn-primary {
  padding: 8px 18px;
  border: 1px solid var(--btn-primary-border);
  border-radius: 4px;
  background: var(--btn-primary-bg);
  font-size: var(--font-size-md);
  color: var(--btn-primary-text);
  cursor: pointer;
  transition: all 0.18s ease;

  &:hover:not(:disabled) {
    background: var(--btn-primary-hover-bg);
    color: var(--btn-primary-hover-text);
    border-color: var(--btn-primary-hover-border);
  }

  &:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
}
</style>
