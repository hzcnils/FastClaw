<script setup lang="ts">
import { ref, onMounted, computed, watch, onActivated } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useKnowledgeBaseStore } from '@/stores/index'
import { chunkToolApi } from '@/api/knowledge'
import type { KnowledgeBase, CreateKnowledgeBaseRequest, AlgorithmInfo } from '@/types/knowledge'
import { useThemeStore } from '@/stores/theme';
import { ElMessage } from 'element-plus';
import { pinyin } from 'pinyin-pro'
import { useFastClawDownloadDriver as useDownloadStore } from '@/integrations/download-center'
import FastKgInstallDialog from '@/components/fastkg/FastKgInstallDialog.vue'
import SearchClearButton from '@/components/common/SearchClearButton.vue'

const themeStore = useThemeStore()

const bgStyle = computed(() => {
  const id = themeStore.currentPreset.id
  if (id == 'simple-dark' || id.startsWith('dark-')) {
    return { 'background-color': 'rgb(255 255 255 / 15%)' }
  } else if (id == 'scene-graphite-cyan') {
    return { 'background-color': '#dfdfe0' }
  } else if (id.startsWith('scene-')) {
    return { 'background-color': 'rgba(255,255,255,0.15)' }
  }
  return {}
})

// 弹窗背景使用主题色 --theme-dailog-color
const modalStyle = computed(() => {
  const id = themeStore.currentPreset.id
  const r: Record<string, string> = {}
  r['background-color'] = 'var(--theme-dailog-color)'
  if (id === 'simple-dark' || id.startsWith('dark-') || (id.startsWith('scene-') && id !== 'scene-graphite-cyan')) {
    r['border'] = '1px solid rgba(255,255,255,0.15)'
    r['color'] = '#fff'
  } else if (id === 'scene-graphite-cyan') {
    r['border'] = '1px solid rgba(0,0,0,0.10)'
    r['color'] = '#1f2937'
  } else if (id === 'simple-light') {
    r['border'] = '1px solid #e5e7eb'
    r['color'] = '#1f2937'
  }
  return r
})

const router = useRouter()
const kbStore = useKnowledgeBaseStore()
const { t } = useI18n()
const downloadStore = useDownloadStore()

const searchQuery = ref('')
const statusFilter = ref<string>('')
const showCreateModal = ref(false)
const showDeleteModal = ref(false)
const deleteTarget = ref<KnowledgeBase | null>(null)
const isCreating = ref(false)
const isDeleting = ref(false)
const showDownloadDialog = ref(false)
const kgFilesExist = ref(false)
const initialized = ref(false)

// 只有服务文件存在且服务已就绪时才允许创建知识库
const canCreate = computed(() => kgFilesExist.value && kgServiceReady.value)

// 切片算法下拉
const algorithms = ref<AlgorithmInfo[]>([])
const algorithmsLoading = ref(false)

async function loadAlgorithms() {
  algorithmsLoading.value = true
  try {
    const response = await chunkToolApi.listAlgorithms()
    algorithms.value = response || []
  } catch (e) {
    console.error('加载切片算法列表失败:', e)
  } finally {
    algorithmsLoading.value = false
  }
}

interface ExtendedCreateRequest extends CreateKnowledgeBaseRequest {
  chunk_size?: number
  chunk_overlap?: number
  folder_path?: string | null
  index_type?: string
  auto_sync?: boolean
  default_algorithm?: string
  user_id?: string
  chunking_config?: any
}

const createForm = ref<ExtendedCreateRequest>({
  name: '', code: '', description: '',
  embedding_model: 'bge-m3', embedding_dimension: 1024,
  is_public: false, folder_path: '', index_type: 'hnsw',
  auto_sync: true, default_algorithm: 'recursive',
  user_id: 'admin', chunking_config: {},
  chunk_size: 500, chunk_overlap: 50,
})

// 根据选择的模型自动设置向量维度
watch(() => createForm.value.embedding_model, (newModel) => {
  if (newModel === 'bge-m3' || newModel === 'bge-large-zh-v1.5') {
    createForm.value.embedding_dimension = 1024;
  } else if (newModel === 'm3e-base') {
    createForm.value.embedding_dimension = 768;
  } else if (newModel === 'bge-small-zh' || newModel === 'all-MiniLM-L6-v2') {
    createForm.value.embedding_dimension = 384;
  } else if (newModel === 'text-embedding-ada-002') {
    createForm.value.embedding_dimension = 1536;
  }
});

// 记录上次自动生成的编码，避免覆盖用户手动输入
const lastAutoCode = ref('')

// 知识库名称 → 自动填充拼音编码（仅在编码为空时自动填充，手动输入后不再覆盖）
watch(() => createForm.value.name, (newName) => {
  if (!newName || !newName.trim()) {
    createForm.value.code = ''
    lastAutoCode.value = ''
    return
  }
  // 编码已被用户手动修改 → 不再自动覆盖
  if (createForm.value.code && createForm.value.code !== lastAutoCode.value) return

  const py = pinyin(newName.trim(), { toneType: 'none' })
  const code = py.replace(/\s+/g, '') + String(Math.floor(Math.random() * 900) + 100)
  createForm.value.code = code
  lastAutoCode.value = code
})

const filteredKnowledgeBases = computed(() => {
  return kbStore.knowledgeBases.filter((kb) => {
    const matchSearch = !searchQuery.value ||
      kb.name.toLowerCase().includes(searchQuery.value.toLowerCase()) ||
      kb.code.toLowerCase().includes(searchQuery.value.toLowerCase())
    const matchStatus = !statusFilter.value || kb.status === statusFilter.value
    return matchSearch && matchStatus
  })
})

const stats = computed(() => ({
  total: kbStore.knowledgeBases.length,
  active: kbStore.knowledgeBases.filter(kb => kb.status === 'active').length,
  files: kbStore.totalFiles,
  chunks: kbStore.totalChunks,
}))

async function handleCreate() {
  if (!createForm.value.name || !createForm.value.code) {
    ElMessage.warning(t('knowledge.requiredFieldsHint') || '请填写知识库名称和编码')
    return
  }
  isCreating.value = true
  try {
    const payload = {
      ...createForm.value,
      chunking_config: {
        chunk_size: createForm.value.chunk_size,
        chunk_overlap: createForm.value.chunk_overlap,
        separator: ['\n\n', '\n', ' ', ''],
        algorithm: createForm.value.default_algorithm || 'recursive',
      },
    } as any
    await kbStore.createKnowledgeBase(payload)
    showCreateModal.value = false
    resetCreateForm()
  } catch (e: any) {
    console.error('创建失败', e)
    alert(e.message || t('knowledge.createFailed'))
  } finally {
    isCreating.value = false
  }
}

function resetCreateForm() {
  createForm.value = {
    name: '', code: '', description: '',
    embedding_model: 'bge-m3', embedding_dimension: 1024,
    is_public: false, folder_path: '', index_type: 'hnsw',
    auto_sync: true, default_algorithm: 'recursive',
    user_id: 'admin', chunking_config: {},
    chunk_size: 500, chunk_overlap: 50,
  }
}

function incrementDim() { createForm.value.embedding_dimension++ }
function decrementDim() { if (createForm.value.embedding_dimension > 1) createForm.value.embedding_dimension-- }
function incrementChunkSize() { createForm.value.chunk_size++ }
function decrementChunkSize() { if (createForm.value.chunk_size > 0) createForm.value.chunk_size-- }
function incrementOverlap() { createForm.value.chunk_overlap++ }
function decrementOverlap() { if (createForm.value.chunk_overlap > 0) createForm.value.chunk_overlap-- }

function openDeleteModal(kb: KnowledgeBase) {
  deleteTarget.value = kb
  showDeleteModal.value = true
}

async function handleDelete() {
  if (!deleteTarget.value) return
  isDeleting.value = true
  try {
    await kbStore.deleteKnowledgeBase(deleteTarget.value.id)
    showDeleteModal.value = false
    deleteTarget.value = null
  } catch (e: any) {
    alert(e.message || t('knowledge.deleteFailed'))
  } finally {
    isDeleting.value = false
  }
}

function formatSize(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}

function formatDate(dateStr: string): string {
  return new Date(dateStr).toLocaleDateString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit' })
}

function getStatusBadge(status: string) {
  const map: Record<string, { cls: string; text: string }> = {
    active: { cls: 'status-active', text: t('knowledge.running') },
    inactive: { cls: 'status-inactive', text: t('knowledge.stopped') },
    building: { cls: 'status-building', text: t('knowledge.building') },
  }
  return map[status] || { cls: 'status-inactive', text: status }
}

// 启动状态使用 api/knowledge.ts 模块级共享变量
import { kgStartingPhase, kgStarting, startKgService, kgServiceReady, ensureKgReady, kgStartBlocked, kgLastStartError } from '@/api/knowledge'

/** 重试时动态加载 API，避开 keep-alive 里仍直连 :3000 的旧 axios */
async function retryLoadKnowledgeBases() {
  kbStore.clearError()
  kbStore.loading = true
  try {
    const api = await import('@/api/knowledge')
    api.kgLastStartError.value = ''
    if (!api.kgServiceReady.value) {
      await api.ensureKgReady({ force: true, startIfNeeded: true })
    }
    if (!api.kgServiceReady.value) {
      return
    }
    const result = await api.knowledgeBaseApi.list({ user_id: 'admin' })
    kbStore.knowledgeBases = Array.isArray(result) ? result : []
  } catch (e) {
    kbStore.error = e instanceof Error ? e.message : '获取知识库列表失败'
    console.warn('获取知识库列表失败:', kbStore.error)
  } finally {
    kbStore.loading = false
  }
}

async function checkKgServiceFiles() {
  // 网络连通性状态由 App.vue onMounted 时通过 checkConnectivity() 初始化
  // 此处只需检查 KG 服务文件是否存在
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const status: any = await invoke('check_kg_service_files')
    kgFilesExist.value = status.all_exist
    if (!status.all_exist) {
      showDownloadDialog.value = true
    } else {
      try {
        await ensureKgReady()
      } catch (e) {
        console.warn('启动知识库服务未完成:', e)
      }
      if (kgServiceReady.value) await kbStore.ensureKnowledgeBases()
    }
  } catch (e) {
    console.warn('检查 KG 服务文件失败:', e)
  } finally {
    initialized.value = true
  }
}

function openCreateModal() {
  if (!canCreate.value) return
  resetCreateForm()
  showCreateModal.value = true
  loadAlgorithms()
}

async function handleBrowseFolder() {
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择本地存储路径',
    })
    if (selected && typeof selected === 'string') {
      createForm.value.folder_path = selected
    }
  } catch (e) {
    console.error('打开文件夹选择失败:', e)
  }
}

async function handleKgInstalled() {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const status = await invoke<{ all_exist: boolean }>('check_kg_service_files')
    kgFilesExist.value = status.all_exist
    if (!status.all_exist) return
    await startKgService()
    if (kgServiceReady.value) await kbStore.fetchKnowledgeBases()
  } catch (error) {
    console.warn('FastKG 安装后的启动流程未完成:', error)
  }
}

const kgReplacingFiles = computed(() => {
  const tasks = downloadStore.tasks as Array<{
    resourceId?: string
    name?: string
    status?: string
    payload?: { appName?: string }
  }> | undefined
  if (!Array.isArray(tasks)) return false
  return tasks.some((t) => {
    const id = String(t?.resourceId || '').toLowerCase()
    const name = String(t?.payload?.appName || t?.name || '')
    return (id === 'fastkg' || name === 'FastKG') && String(t?.status || '') === 'installing'
  })
})

watch(kgReplacingFiles, async (replacing) => {
  kgStartBlocked.value = replacing
  if (!replacing) return
  kgServiceReady.value = false
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('stop_kg_for_update')
  } catch (e) {
    console.warn('更新前停止 FastKG 失败:', e)
  }
})

onMounted(async () => {
  await checkKgServiceFiles()
})

onActivated(async () => {
  // keep-alive 重新激活时优先复用应用级就绪状态和列表 TTL 缓存。
  if (kgFilesExist.value && !kgStartBlocked.value) {
    if (!kgServiceReady.value) {
      try {
        await ensureKgReady()
      } catch (e) {
        console.warn('激活时启动知识库服务未完成:', e)
      }
    }
    if (kgServiceReady.value) await kbStore.ensureKnowledgeBases()
  }
})

// 服务从「未就绪」变为就绪时自动拉列表，避免先闪未就绪再手动重试
watch(kgServiceReady, (ready, wasReady) => {
  if (ready && !wasReady && kgFilesExist.value) {
    void kbStore.ensureKnowledgeBases()
  }
})
</script>

<template>
  <div class="kb-page">
    <!-- 统计卡片 -->
    <div class="stats-row">
      <div class="stat-card" :style="bgStyle">
        <div class="stat-icon si-blue">
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2">
            <path
              d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4">
            </path>
          </svg>
        </div>
        <!-- <div class="stat-info"> -->
          <span class="stat-label">{{ $t('chat.totalKb') }}</span>
          <span class="stat-value">{{ stats.total }}</span>
        <!-- </div> -->
      </div>
      <div class="stat-card" :style="bgStyle">
        <div class="stat-icon si-green">
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
            <polyline points="22 4 12 14.01 9 11.01"></polyline>
          </svg>
        </div>
        <!-- <div class="stat-info"> -->
          <span class="stat-label">{{ $t('chat.running') }}</span>
          <span class="stat-value">{{ stats.active }}</span>
        <!-- </div> -->
      </div>
      <div class="stat-card" :style="bgStyle">
        <div class="stat-icon si-indigo">
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
          </svg>
        </div>
        <!-- <div class="stat-info"> -->
          <span class="stat-label">{{ $t('chat.totalFiles') }}</span>
          <span class="stat-value">{{ stats.files.toLocaleString() }}</span>
        <!-- </div> -->
      </div>
      <div class="stat-card" :style="bgStyle">
        <div class="stat-icon si-amber">
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="3" y="3" width="7" height="7"></rect>
            <rect x="14" y="3" width="7" height="7"></rect>
            <rect x="3" y="14" width="7" height="7"></rect>
            <rect x="14" y="14" width="7" height="7"></rect>
          </svg>
        </div>
        <!-- <div class="stat-info"> -->
          <span class="stat-label">{{ $t('chat.totalChunks') }}</span>
          <span class="stat-value">{{ stats.chunks.toLocaleString() }}</span>
        <!-- </div> -->
      </div>
    </div>

    <!-- 操作栏  :style="bgStyle"  -->
    <div class="toolbar" >
      <div class="toolbar-left">
        <!-- <div class="search-box">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="11" cy="11" r="8"></circle>
            <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
          </svg>
          <input v-model="searchQuery" type="text" :placeholder="$t('chat.searchKb')" />
        </div> -->
        <el-input v-model="searchQuery" :placeholder="$t('chat.searchKb')" style="max-width: 300px;">
          <template #prefix>
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8" />
              <line x1="21" y1="21" x2="16.65" y2="16.65" />
            </svg>
          </template>
          <template #suffix>
            <SearchClearButton v-if="searchQuery" @clear="searchQuery = ''" />
          </template>
        </el-input>
        <el-select v-model="statusFilter" :placeholder="$t('knowledge.filterStatusPlaceholder')" style="max-width: 200px; ">
          <el-option value="" :label="$t('knowledge.allStatus')"></el-option>
          <el-option value="active" :label="$t('knowledge.running')"></el-option>
          <el-option value="inactive" :label="$t('knowledge.stopped')"></el-option>
          <el-option value="building" :label="$t('knowledge.building')"></el-option>
        </el-select>
        <!-- <select v-model="statusFilter" class="status-select">
          <option value="">{{ $t('knowledge.allStatus') }}</option>
          <option value="active">{{ $t('knowledge.running') }}</option>
          <option value="inactive">{{ $t('knowledge.stopped') }}</option>
          <option value="building">{{ $t('knowledge.building') }}</option>
        </select> -->
      </div>
      <button v-if="!kgFilesExist" class="btn-install-service" @click="() => showDownloadDialog = true">
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
          <polyline points="7 10 12 15 17 10"></polyline>
          <line x1="12" y1="15" x2="12" y2="3"></line>
        </svg>
        {{ $t('knowledge.installServiceBtn') }}
      </button>
      <el-tooltip :content="$t('knowledge.createDisabledTip')" :disabled="canCreate" placement="top">
        <span>
          <button class="btn-create" :disabled="!canCreate" @click="openCreateModal">
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="12" y1="5" x2="12" y2="19"></line>
              <line x1="5" y1="12" x2="19" y2="12"></line>
            </svg>
            {{ $t('knowledge.createKb') }}
          </button>
        </span>
      </el-tooltip>
    </div>

    <!-- KG 服务不可用提示横幅 -->
    <el-alert v-if="initialized && !canCreate && !kgStarting"
      :title="!kgFilesExist ? $t('knowledge.kgNotInstalledBanner') : $t('knowledge.kgNotReadyBanner')" type="warning"
      show-icon :closable="false" style="margin: 0 10px 10px; border-radius: 8px;" />

    <div v-if="kgStarting" class="loading-state">
      <div class="spinner"></div>
      <span>{{ kgStartingPhase ? t(`knowledge.${kgStartingPhase}`) : t('knowledge.startingService') }}</span>
    </div>

    <!-- 加载状态 -->
    <div v-else-if="kbStore.loading" class="loading-state">
      <div class="spinner"></div>
      <span>{{ $t('common.loading') }}</span>
    </div>

    <!-- 服务未就绪 -->
    <div v-else-if="kgFilesExist && !kgServiceReady" class="empty-state">
      <div class="empty-icon" style="color: #faad14;">
        <svg viewBox="0 0 24 24" width="48" height="48" fill="none" stroke="currentColor" stroke-width="1.5">
          <circle cx="12" cy="12" r="10"></circle>
          <line x1="12" y1="8" x2="12" y2="12"></line>
          <line x1="12" y1="16" x2="12.01" y2="16"></line>
        </svg>
      </div>
      <h3>{{ $t('knowledge.serviceNotReady') }}</h3>
      <p>{{ kgLastStartError || $t('knowledge.serviceNotReadyDesc') }}</p>
      <button class="btn-create"
        @click="retryLoadKnowledgeBases">{{
          $t('knowledge.retry') }}</button>
    </div>

    <!-- 错误状态 -->
    <div v-else-if="kbStore.error" class="empty-state">
      <div class="empty-icon" style="color: #ff4d4f;">
        <svg viewBox="0 0 24 24" width="48" height="48" fill="none" stroke="currentColor" stroke-width="1.5">
          <circle cx="12" cy="12" r="10"></circle>
          <line x1="15" y1="9" x2="9" y2="15"></line>
          <line x1="9" y1="9" x2="15" y2="15"></line>
        </svg>
      </div>
      <h3>{{ $t('knowledge.loadFailed') }}</h3>
      <p>{{ kbStore.error }}</p>
      <button class="btn-create" @click="retryLoadKnowledgeBases">{{ $t('knowledge.retry') }}</button>
    </div>

    <!-- 空状态 -->
    <div v-else-if="filteredKnowledgeBases.length === 0" class="empty-state">
      <div class="empty-icon">
        <svg viewBox="0 0 24 24" width="48" height="48" fill="none" stroke="currentColor" stroke-width="1.5">
          <path
            d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4">
          </path>
        </svg>
      </div>
      <h3>{{ $t('chat.noKb') }}</h3>
      <p>{{ canCreate ? $t('chat.createFirstKb') : '' }}</p>
      <button v-if="canCreate" class="btn-create" @click="openCreateModal">
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="12" y1="5" x2="12" y2="19"></line>
          <line x1="5" y1="12" x2="19" y2="12"></line>
        </svg>
        {{ $t('knowledge.createKb') }}
      </button>
    </div>

    <!-- 知识库卡片网格 -->
    <div v-else class="kb-grid">
      <el-scrollbar height="100%" always view-style="padding: 0 10px; margin-top: 4px; background: transparent;">
        <el-row :gutter="10">
          <el-col v-for="kb in filteredKnowledgeBases" :key="kb.id" :xs="24" :sm="12" :md="8" :lg="8" :xl="4">
            <div class="kb-card" @click="router.push(`/knowledge-bases/${kb.id}`)" :style="bgStyle">
              <div class="kb-card-header">
                <div class="kb-avatar">{{ kb.name?.charAt(0).toUpperCase() }}</div>
                <div class="kb-title-area">
                  <div class="kb-name-row">
                    <el-tooltip :content="kb.name" placement="top" :disabled="true">
                      <h3 class="kb-name">{{ kb.name }}</h3>
                    </el-tooltip>
                    <span class="kb-status" :class="getStatusBadge(kb.status).cls">{{ getStatusBadge(kb.status).text
                    }}</span>
                  </div>
                  <el-tooltip :content="kb.code" placement="top" :disabled="true" >
                    <span class="kb-code">
                      <svg class="kb-code-icon" viewBox="0 0 24 24" width="14" height="14" fill="none"
                        stroke="currentColor" stroke-width="2">
                        <polyline points="16 18 22 12 16 6"></polyline>
                        <polyline points="8 6 2 12 8 18"></polyline>
                      </svg>
                      <span class="kb-code-label">{{ $t('knowledge.kbCode') }}</span>
                      
                      {{ kb.code }}
                    </span>
                  </el-tooltip>
                </div>
              </div>
              <!-- <el-tooltip :content="kb.description" placement="top" :show-after="200" :disabled="!kb.description" > -->
              <el-tooltip :content="kb.description" placement="top" :show-after="200"  :disabled="true">
                <p class="kb-desc">{{ kb.description || $t('knowledge.noDesc') }}</p>
              </el-tooltip>
              <div class="kb-stats">
                <div class="kb-stat-item">
                  <svg class="kb-stat-icon" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor"
                    stroke-width="2">
                    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
                  </svg>
                  <span class="kb-stat-lbl">{{ $t('knowledge.files') }}：</span>
                  <span class="kb-stat-val">{{ kb.file_count }}</span>
                </div>
                <div class="kb-stat-item">
                  <svg class="kb-stat-icon" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor"
                    stroke-width="2">
                    <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"></path>
                  </svg>
                  <span class="kb-stat-lbl">{{ $t('knowledge.chunks') }}：</span>
                  <span class="kb-stat-val">{{ kb.chunk_count }}</span>
                </div>
                <div class="kb-stat-item">
                  <svg class="kb-stat-icon" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor"
                    stroke-width="2">
                    <path d="M4 4h16v16H4z" fill="none" stroke="currentColor" stroke-width="2" />
                    <path d="M4 4h4v4H4z" fill="currentColor" />
                    <line x1="8" y1="8" x2="8" y2="4" />
                    <line x1="11" y1="8" x2="11" y2="4" />
                    <line x1="14" y1="8" x2="14" y2="4" />
                  </svg>
                  <span class="kb-stat-lbl">{{ $t('knowledge.size') }}：</span>
                  <span class="kb-stat-val">{{ formatSize(kb.total_size || 0) }}</span>
                </div>
              </div>
              <div class="kb-card-footer">
                <span class="kb-date">{{ formatDate(kb.updated_at) }}</span>
                <button class="kb-del-btn" @click.stop="openDeleteModal(kb)" :title="$t('knowledge.deleteKb')">
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2">
                    <polyline points="3 6 5 6 21 6"></polyline>
                    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                  </svg>
                </button>
              </div>
            </div>
          </el-col>
        </el-row>
      </el-scrollbar>
    </div>

    <!-- 创建知识库弹窗 -->
    <div v-if="showCreateModal" class="modal-overlay">
      <div class="modal-box" :style="modalStyle" :key="themeStore.currentPreset.id">
        <div class="modal-header">
          <h3>{{ $t('knowledge.createKbTitle') }}</h3>
          <button class="modal-close" @click="showCreateModal = false">
            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
        <div class="modal-body">
          <div class="form-row">
            <div class="form-group">
              <label>{{ $t('knowledge.formName') }} <span class="req">*</span></label>
              <input v-model="createForm.name" type="text" :placeholder="$t('knowledge.namePh')" />
            </div>
            <div class="form-group">
              <label>{{ $t('knowledge.formCode') }} <span class="req">*</span></label>
              <input v-model="createForm.code" type="text" :placeholder="$t('knowledge.codePh')" />
            </div>
          </div>
          <div class="form-group">
            <label>{{ $t('knowledge.formDesc') }}</label>
            <textarea v-model="createForm.description" rows="2" :placeholder="$t('knowledge.formDescPh')"></textarea>
          </div>
          <div class="form-divider"></div>
          <div class="form-row">
            <div class="form-group">
              <label>{{ $t('knowledge.formModel') }}</label>
              <el-select v-model="createForm.embedding_model" teleported popper-class="kb-create-popper">
                <el-option value="bge-m3" label="bge-m3 (推荐, 1024维)" />
                <el-option value="bge-large-zh-v1.5" label="bge-large-zh-v1.5 (1024维)" />
                <el-option value="m3e-base" label="m3e-base (768维)" />
                <el-option value="bge-small-zh" label="bge-small-zh (384维)" />
                <el-option value="all-MiniLM-L6-v2" label="all-MiniLM-L6-v2 (384维)" />
                <el-option value="text-embedding-ada-002" label="OpenAI Ada 002 (1536维)" />
              </el-select>
            </div>
            <div class="form-group">
              <label>{{ $t('knowledge.formDim') }}</label>
              <div class="number-input-wrap">
                <input v-model.number="createForm.embedding_dimension" type="number" />
                <div class="number-arrows">
                  <button class="number-arrow number-arrow-up" type="button" @click="incrementDim">
                    <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                      <polyline points="18 15 12 9 6 15"></polyline>
                    </svg>
                  </button>
                  <button class="number-arrow number-arrow-down" type="button" @click="decrementDim">
                    <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                      <polyline points="6 9 12 15 18 9"></polyline>
                    </svg>
                  </button>
                </div>
              </div>
            </div>
          </div>
          <div class="form-group">
            <label>{{ $t('knowledge.localPath') }}</label>
            <div class="path-input-row">
              <input v-model="createForm.folder_path" type="text" placeholder="/data/knowledge_base"
                class="mono path-input" />
              <button type="button" class="btn-browse" @click="handleBrowseFolder">{{ $t('knowledge.browse') }}</button>
            </div>
            <span class="form-hint">{{ $t('knowledge.localPathHint') }}</span>
          </div>
          <div class="form-row">
            <div class="form-group">
              <label>{{ $t('knowledge.indexType') }}</label>
              <input v-model="createForm.index_type" type="text" placeholder="hnsw" />
            </div>
            <div class="form-group">
              <label>{{ $t('knowledge.chunkAlgorithm') }}</label>
              <el-select v-model="createForm.default_algorithm" :disabled="algorithmsLoading"
                :placeholder="$t('knowledge.chunkAlgorithmPlaceholder')" teleported popper-class="kb-create-popper">
                <el-option v-for="alg in algorithms" :key="alg.code" :value="alg.code"
                  :label="alg.name + '（' + alg.code + '）'" />
              </el-select>
            </div>
          </div>
          <div class="form-section">
            <span class="form-section-title">{{ $t('knowledge.chunkParamTitle') }}</span>
            <div class="form-row">
              <div class="form-group">
                <label>{{ $t('knowledge.chunkSizeLabel') }}</label>
                <div class="number-input-wrap">
                  <input v-model.number="createForm.chunk_size" type="number" placeholder="500" />
                  <div class="number-arrows">
                    <button class="number-arrow number-arrow-up" type="button" @click="incrementChunkSize">
                      <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="18 15 12 9 6 15"></polyline>
                      </svg>
                    </button>
                    <button class="number-arrow number-arrow-down" type="button" @click="decrementChunkSize">
                      <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="6 9 12 15 18 9"></polyline>
                      </svg>
                    </button>
                  </div>
                </div>
              </div>
              <div class="form-group">
                <label>{{ $t('knowledge.overlapSize') }}</label>
                <div class="number-input-wrap">
                  <input v-model.number="createForm.chunk_overlap" type="number" placeholder="50" />
                  <div class="number-arrows">
                    <button class="number-arrow number-arrow-up" type="button" @click="incrementOverlap">
                      <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="18 15 12 9 6 15"></polyline>
                      </svg>
                    </button>
                    <button class="number-arrow number-arrow-down" type="button" @click="decrementOverlap">
                      <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="6 9 12 15 18 9"></polyline>
                      </svg>
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>
          <div class="form-switches">
            <label class="switch-label">
              <input type="checkbox" v-model="createForm.is_public" />
              <span class="switch-track"></span>
              <span>{{ $t('knowledge.publicKb') }}</span>
            </label>
            <label class="switch-label">
              <input type="checkbox" v-model="createForm.auto_sync" />
              <span class="switch-track"></span>
              <span>{{ $t('knowledge.autoSync') }}</span>
            </label>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn-secondary" @click="showCreateModal = false">{{ $t('knowledge.cancel') }}</button>
          <button class="btn-create" :disabled="isCreating" @click="handleCreate">
            {{ isCreating ? $t('knowledge.creating') : $t('knowledge.confirmCreate') }}
          </button>
        </div>
      </div>
    </div>

    <FastKgInstallDialog v-model="showDownloadDialog" @installed="handleKgInstalled" />

    <!-- 删除确认弹窗 -->
    <div v-if="showDeleteModal" class="modal-overlay">
      <div class="modal-box modal-sm" :style="modalStyle">
        <div class="modal-header">
          <h3>{{ $t('knowledge.confirmDeleteTitle') }}</h3>
          <button class="modal-close" @click="showDeleteModal = false">
            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
        <div class="modal-body" style="text-align: center; padding: 24px 20px;">
          <div class="delete-icon-wrap">
            <svg viewBox="0 0 24 24" width="28" height="28" fill="none" stroke="#ff4d4f" stroke-width="2">
              <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path>
              <line x1="12" y1="9" x2="12" y2="13"></line>
              <line x1="12" y1="17" x2="12.01" y2="17"></line>
            </svg>
          </div>
          <h4 style="margin: 12px 0 6px; font-size: var(--font-size-lg);">{{ $t('knowledge.confirmDeleteMsg') }}</h4>
          <p style="color: var(--text-secondary); font-size: var(--font-size-md);">{{
            $t('knowledge.confirmDeleteDetail', { name: deleteTarget?.name }) }}</p>
        </div>
        <div class="modal-footer">
          <button class="btn-secondary" @click="showDeleteModal = false">{{ $t('knowledge.cancel') }}</button>
          <button class="btn-danger" :disabled="isDeleting" @click="handleDelete">
            {{ isDeleting ? $t('knowledge.deleting') : $t('knowledge.confirmDelete') }}
          </button>
        </div>
      </div>
    </div>

  </div>
</template>

<style lang="scss" scoped>
.kb-page {
  padding: 25px 10px;
  height: 100%;
  box-sizing: border-box;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  background: var(--bg-primary);
}

/* ===== 统计卡片 ===== */
.stats-row {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 16px;
  margin-bottom: 15px;
  padding: 0 10px;
  flex-shrink: 0;
}

.stat-card {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 9px 10px;
  min-width: 0;
  box-sizing: border-box;
  white-space: nowrap;
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  transition: box-shadow 0.2s;

  &:hover {
    box-shadow: 0 2px 12px rgba(0, 0, 0, 0.04);
  }
}

.stat-icon {
  width: 34px;
  height: 34px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.si-blue {
  background: #e6f4ff;
  color: #1677ff;
}

.si-green {
  background: #f6ffed;
  color: #52c41a;
}

.si-indigo {
  background: #f0f5ff;
  color: #2f54eb;
}

.si-amber {
  background: #fffbe6;
  color: #faad14;
}

.stat-info {
  display: flex;
  flex-direction: column;
}

.stat-value {
  font-size: var(--font-size-3xl);
  font-weight: 600;
  color: var(--text-primary);
  line-height: 1.2;
}

.stat-label {
  font-size: var(--font-size-lm);
  // color: var(--text-secondary, #6b7280);
  margin-top: 2px;
}

/* ===== 操作栏 ===== */
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 12px;
  margin: 0 auto;
  width: calc(100% - 20px);
  margin-bottom: 10px;
  // padding: 14px 18px;
  // background: var(--bg-primary);
  // border: 1px solid var(--border-color);
  border-radius: 8px;
  flex-shrink: 0;

  /* 右侧操作按钮不收缩，宽度不够时整组落到下一行 */
  > .btn-install-service,
  > .el-tooltip__trigger,
  > span {
    flex: 0 0 auto;
  }
}

.toolbar-left {
  display: flex;
  align-items: center;
  flex-wrap: nowrap;
  gap: 12px;
  /* 优先占满一行左侧；宽度不够时整块把右侧按钮挤到下一行 */
  flex: 1 1 420px;
  min-width: 0;

  // 搜索框样式
  :deep(.el-input__wrapper) {
    background: transparent !important;
    border: 1px solid var(--border-color) !important;
    border-radius: 6px;
    height: 36px !important;
    box-shadow: none !important;
  }

  :deep(.el-input__wrapper:hover),
  :deep(.el-input__wrapper.is-focus),
  :deep(.el-input__wrapper:focus),
  :deep(.el-input.is-focus .el-input__wrapper) {
    border-color: var(--border-color) !important;
    box-shadow: none !important;
    outline: none !important;
  }

  :deep(.el-input--prefix .el-input__wrapper.is-focus),
  :deep(.el-input--prefix.is-focus .el-input__wrapper) {
    border-color: var(--border-color) !important;
    box-shadow: none !important;
  }

  :deep(.el-input__inner) {
    color: var(--text-primary);
  }

  /* 仅搜索框（带 prefix）占位符：比正常文字略浅 */
  :deep(.el-input--prefix .el-input__inner::placeholder) {
    color: color-mix(in srgb, var(--text-primary) 55%, transparent);
    opacity: 1;
  }

  :deep(.el-input__prefix) {
    color: var(--text-secondary);
  }

  // 下拉框样式（和搜索框保持一致）
  :deep(.el-select__wrapper) {
    background: transparent !important;
    border: 1px solid var(--border-color) !important;
    border-radius: 6px !important;
    height: 36px !important;
    min-height: unset !important;
    box-shadow: none !important;
    box-sizing: border-box;
  }

  :deep(.el-select__wrapper:hover) {
    border-color: var(--border-color) !important;
  }

  :deep(.el-select__wrapper.is-focus) {
    border-color: var(--border-color) !important;
    box-shadow: 0 0 0 1px var(--border-color) !important;
  }

  // :deep(.el-select__placeholder) {
  //   color: var(--text-secondary) !important;
  // }
  // :deep(.el-select__selected-item) {
  //   color: var(--text-primary) !important;
  // }

}

.search-box {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  flex: 1;
  max-width: 360px;
  transition: border-color 0.2s;

  &:focus-within {
    border-color: #4b6bfc;
    background: var(--bg-primary);
  }

  svg {
    color: var(--text-secondary);
    flex-shrink: 0;
  }

  input {
    border: none;
    background: transparent;
    outline: none;
    font-size: var(--font-size-md);
    color: var(--text-primary);
    width: 100%;

    &::placeholder {
      color: var(--text-secondary);
      opacity: 0.6;
    }
  }
}

.status-select {
  padding: 8px 12px;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  background: var(--bg-primary);
  font-size: var(--font-size-md);
  color: var(--text-primary);
  outline: none;
  cursor: pointer;

  &:focus {
    border-color: #4b6bfc;
  }
}

.btn-create {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 18px;
  background: var(--btn-primary-bg);
  color: var(--btn-primary-text);
  border: 1px solid var(--btn-primary-border);
  border-radius: 4px;
  font-size: var(--font-size-md);
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;

  &:hover:not(:disabled) {
    background: var(--btn-primary-hover-bg);
    color: var(--btn-primary-hover-text);
    border-color: var(--btn-primary-hover-border);
    transform: translateY(-1px);
    box-shadow: 0 2px 8px color-mix(in srgb, var(--btn-primary-hover-bg) 26%, transparent);
  }

  &:disabled {
    background: var(--btn-primary-disabled-bg);
    color: var(--btn-primary-disabled-text);
    border-color: var(--btn-primary-disabled-border);
    cursor: not-allowed;
    transform: none;
  }
}

.btn-install-service {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 18px;
  background: var(--color-warning, #e6a23c);
  color: #fff;
  border: 1px solid var(--color-warning, #e6a23c);
  border-radius: 5px;
  font-size: var(--font-size-md);
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;

  &:hover {
    background: #d18e1f;
    border-color: #d18e1f;
    transform: translateY(-1px);
    box-shadow: 0 2px 8px rgba(230, 162, 60, 0.3);
  }
}

/* ===== 加载/空状态 ===== */
.loading-state {
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: var(--text-secondary);
  font-size: var(--font-size-md);
}

.spinner {
  width: 32px;
  height: 32px;
  border: 3px solid var(--border-color, #e8e8e8);
  border-top-color: var(--color-primary);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 80px 0;
}

.empty-icon {
  color: #d9d9d9;
  margin-bottom: 16px;
}

.empty-state h3 {
  font-size: var(--font-size-lg);
  color: var(--text-primary);
  margin-bottom: 6px;
}

.empty-state p {
  font-size: var(--font-size-md);
  color: var(--text-secondary);
  margin-bottom: 20px;
}

/* ===== 知识库卡片网格 ===== */
.kb-grid {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  background: transparent;

  :deep(.el-scrollbar) {
    height: 100%;
  }
}

.kb-grid :deep(.el-scrollbar__wrap) {
  background: transparent !important;
}

.kb-grid :deep(.el-scrollbar__view) {
  background: transparent !important;
}

.kb-card {
  position: relative;
  display: flex;
  flex-direction: column;
  padding: 16px 18px 14px;
  border-radius: 16px;
  border: 1px solid rgba(99, 102, 241, 0.10);
  box-shadow: 0 1px 2px rgba(16, 24, 40, 0.04);
  transition: transform 0.18s ease, box-shadow 0.18s ease, border-color 0.18s ease;
  cursor: pointer;
  overflow: hidden;
  margin-bottom: 10px;

  &:hover {
    transform: translateY(-2px);
    box-shadow: 0 0px 7px rgba(16, 24, 40, 0.10);
    border-color: rgba(99, 102, 241, 0.28);
  }

  &.is-disabled {
    opacity: 0.6;
    filter: grayscale(0.25);
  }

  &.manage-mode {
    cursor: pointer;

    &.is-selected {
      border-color: var(--color-primary);
      box-shadow: 0 0 0 2px rgba(24, 144, 255, 0.18);
    }
  }
}





.kb-card-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 5px;

}

.kb-avatar {
  flex-shrink: 0;
  width: 42px;
  height: 42px;
  border-radius: 8px;
  background: #e6f4ff;
  color: #1677ff;
  font-size: var(--font-size-lg);
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;

}

.kb-title-area {
  flex: 1;
  min-width: 0;
}

.kb-name-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--font-size-xl);
}

.kb-name {
  flex: 1;
  min-width: 0;
  font-size: var(--font-size-lg);
  font-weight: 600;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;

}

.kb-code {
  display: block;
  font-size: var(--font-size-md);
  color: var(--text-secondary);
  font-family: monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.kb-code-icon {
  display: inline-block;
  vertical-align: middle;
  margin-right: 4px;
  flex-shrink: 0;
}

.kb-code-label {
  color: var(--text-secondary);
  margin-right: 2px;
}

.kb-status {
  font-size: var(--font-size-sm);
  padding: 3px 10px;
  border-radius: 12px;
  font-weight: 500;
  flex-shrink: 0;
}

.status-active {
  background: #f6ffed;
  color: #52c41a;
}

.status-inactive {
  background: #f5f5f5;
  color: #999;
}

.status-building {
  background: #fff7e6;
  color: #fa8c16;
}

.kb-desc {
  font-size: var(--font-size-lm);
  color: var(--text-secondary);
  line-height: 1.5;
  margin-bottom: 5px;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  min-height: 20px;
  overflow: hidden;
  text-overflow: ellipsis;
  /* 单行显示核心 */
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;

}

.kb-stats {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;


  padding: 5px 0;
  /* 上下内边距（分割线距离内容） */
  border-top: 1px solid rgba(0, 0, 0, 0.04);
  /* 上分割线 */
  border-bottom: 1px solid rgba(0, 0, 0, 0.04);
  /* 下分割线 */
  margin-bottom: 5px;
  /* 下外边距（和下面元素的间距） */

  /* 左右外边距清零 */
  margin-left: 0;
  margin-right: 0;

}

.kb-stat-item {
  /* 水平布局 + 内部垂直居中 */
  display: flex;
  align-items: center;
  /* 整体靠左对齐 */
  justify-content: flex-start;

  .kb-stat-icon {
    width: 13px;
    height: 13px;
    margin-right: 4px;
    margin-top: 4px;
  }

  :deep([data-theme-id="simple-light"]) .kb-stat-icon,
  :deep([data-theme-id="scene-graphite-cyan"]) .kb-stat-icon {
    color: #000 !important;
  }

  :deep([data-theme-id="simple-dark"]) .kb-stat-icon,
  :deep([data-theme-id^="dark-"]) .kb-stat-icon,
  :deep([data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"])) .kb-stat-icon {
    color: #fff !important;
  }
}

.kb-stat-val {
  display: block;
  font-size: var(--font-size-lm); //--font-size-lg
  //font-weight: 700;
  // color: var(--text-primary);
  color: var(--text-secondary);
  /* 单行显示核心 */
  white-space: nowrap;
  // margin-left:5px ;
}

.kb-stat-lbl {
  display: block;
  font-size: var(--font-size-lm);
  color: var(--text-secondary);
  margin-top: 2px;
  /* 单行显示核心 */
  white-space: nowrap;
  // overflow: hidden;
  // text-overflow: ellipsis;
}

.kb-card-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.kb-date {
  font-size: var(--font-size-lm);
  color: var(--text-secondary);
}

.kb-del-btn {
  border: none;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  padding: 4px;
  border-radius: 6px;
  opacity: 0;
  transition: all 0.15s;

  .kb-card:hover & {
    opacity: 1;
  }

  &:hover {
    color: #ff4d4f;
    background: rgba(255, 77, 79, 0.06);
  }
}

/* ===== 弹窗 ===== */
.modal-overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  background: rgba(15, 23, 42, 0.05);
  /* ← 遮罩颜色和透明度在这里 */
  backdrop-filter: blur(0px);
  /* ← 模糊效果 */
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px 24px 24px 24px;
}

.modal-box {
  border: 1px solid var(--border-color);
  border-radius: 18px;
  box-shadow: 0 24px 70px rgba(15, 23, 42, 0.24);
  width: 660px;
  max-width: 95%;
  max-height: 90vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.modal-sm {
  width: 420px;
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
    color: var(--text-primary);
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
  padding: 10px 24px 5px 24px;
  overflow-y: auto;
  background: transparent;
  flex: 1;

  input,
  select,
  textarea {
    color: inherit;
  }
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  background: transparent;
  padding: 18px 20px;
  border-top: 1px solid var(--border-color);
}

.btn-secondary {
  padding: 8px 18px;
  border-radius: 4px;
  font-size: var(--font-size-md);
  cursor: pointer;
  transition: all 0.18s ease;
  /* 与主题选择弹窗「恢复/取消」一致 */
  color: var(--btn-secondary-text);
  background: var(--cancel-btn-color);
  border: 1px solid var(--btn-secondary-border);

  &:hover {
    background: var(--cancel-btn-hover-color);
    color: var(--btn-secondary-hover-text);
    border-color: var(--btn-secondary-hover-border);
  }
}

.btn-danger {
  padding: 8px 18px;
  border: 1px solid var(--btn-danger-border);
  border-radius: 4px;
  background: var(--btn-danger-bg);
  color: var(--btn-danger-text);
  font-size: var(--font-size-md);
  font-weight: 500;
  cursor: pointer;
  transition: all 0.18s ease;

  &:hover:not(:disabled) {
    background: var(--btn-danger-hover-bg);
    color: var(--btn-danger-hover-text);
    border-color: var(--btn-danger-hover-border);
  }

  &:disabled {
    background: var(--btn-danger-disabled-bg);
    color: var(--btn-danger-disabled-text);
    border-color: var(--btn-danger-disabled-border);
    cursor: not-allowed;
  }
}

.delete-icon-wrap {
  width: 56px;
  height: 56px;
  margin: 0 auto;
  border-radius: 50%;
  background: rgba(255, 77, 79, 0.06);
  display: flex;
  align-items: center;
  justify-content: center;
}

/* ===== 表单 ===== */
.form-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 14px;
}

.form-group {
  margin-bottom: 14px;

  label {
    display: block;
    font-size: var(--font-size-md);
    font-weight: 600;
    color: var(--text-secondary);
    margin-bottom: 6px;
  }

  .req {
    color: #f5222d;
  }

  input,
  select,
  textarea {
    width: 100%;
    padding: 10px 12px;
    border: 1px solid var(--border-color);
    border-radius: 6px;
    font-size: var(--font-size-base);
    color: var(--text-primary);
    background: var(--bg-hover);
    outline: none;
    transition: all 0.2s;
    box-sizing: border-box;

    &::placeholder {
      color: var(--text-secondary);
      opacity: 0.5;
    }
  }

  textarea {
    resize: none;
    font-family: inherit;
  }

  .mono {
    font-family: monospace;
    font-size: var(--font-size-sm);
  }

  .number-input-wrap {
    position: relative;
    display: flex;
    align-items: center;
    width: 100%;

    input[type="number"] {
      width: 100%;
      padding-right: 28px;
      -moz-appearance: textfield;

      &::-webkit-inner-spin-button,
      &::-webkit-outer-spin-button {
        -webkit-appearance: none;
        margin: 0;
      }
    }

    .number-arrows {
      position: absolute;
      right: 1px;
      top: 1px;
      bottom: 1px;
      width: 26px;
      display: flex;
      flex-direction: column;
      opacity: 0;
      transition: opacity 0.2s;
      border-radius: 0 5px 5px 0;
      overflow: hidden;
    }

    &:hover .number-arrows {
      opacity: 1;
    }

    .number-arrow {
      flex: 1;
      display: flex;
      align-items: center;
      justify-content: center;
      background: transparent;
      border: none;
      color: var(--text-primary);
      cursor: pointer;
      padding: 0;

      &:hover {
        background: rgba(128, 128, 128, 0.2);
      }

      svg {
        width: 12px;
        height: 12px;
      }
    }
  }
}

.form-hint {
  font-size: var(--font-size-xs);
  color: var(--text-secondary);
  margin-top: 4px;
  display: block;
}

.path-input-row {
  display: flex;
  gap: 8px;
  align-items: center;
}

.path-input {
  flex: 1;
}

.btn-browse {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 18px;
  background: var(--btn-primary-bg);
  color: var(--btn-primary-text);
  border: 1px solid var(--btn-primary-border);
  border-radius: 4px;
  font-size: var(--font-size-md);
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;

  &:hover:not(:disabled) {
    background: var(--btn-primary-hover-bg);
    color: var(--btn-primary-hover-text);
    border-color: var(--btn-primary-hover-border);
    transform: translateY(-1px);
    box-shadow: 0 2px 8px color-mix(in srgb, var(--btn-primary-hover-bg) 26%, transparent);
  }

  &:disabled {
    background: var(--btn-primary-disabled-bg);
    color: var(--btn-primary-disabled-text);
    border-color: var(--btn-primary-disabled-border);
    cursor: not-allowed;
    transform: none;
  }
}

.form-divider {
  height: 1px;
  background: var(--border-color);
  margin: 4px 0 14px;
}

.form-section {
  background: transparent;
  border: 1px solid var(--border-color);
  border-radius: 10px;
  padding: 14px;
  margin-bottom: 1px;
}

.form-section-title {
  display: block;
  font-size: var(--font-size-md);
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 12px;
}

.form-switches {
  display: flex;
  gap: 28px;
  padding-top: 4px;
}

.switch-label {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  font-size: var(--font-size-md);
  color: var(--text-primary);

  input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .switch-track {
    width: 36px;
    height: 20px;
    background: #ccc;
    border-radius: 10px;
    position: relative;
    transition: background 0.3s;

    &::before {
      content: '';
      position: absolute;
      left: 3px;
      bottom: 3px;
      width: 14px;
      height: 14px;
      background: white;
      border-radius: 50%;
      transition: transform 0.3s;
    }
  }

  input:checked+.switch-track {
    background: var(--color-primary);

    &::before {
      transform: translateX(16px);
    }
  }
}

/* 状态下拉框与搜索输入框高度一致 */
:deep(.el-select__wrapper) {
  height: 32px;
  box-sizing: border-box;
}

:deep(.el-select__wrapper) {
  background: transparent !important;
  border: 1px solid var(--border-color) !important;
  border-radius: 6px !important;
  //height: 36px !important;
  min-height: unset !important;
  box-shadow: none !important;
  box-sizing: border-box;

  &:hover {
    border-color: var(--text-secondary) !important;
  }
}

:deep(.el-select__placeholder) {
  color: var(--text-secondary) !important;
}

:deep(.el-select__selected-item) {
  color: var(--text-primary) !important;
}

:deep(.el-select__caret) {
  color: var(--text-secondary) !important;
}




/* 创建弹窗内下拉框高度与输入框一致 */
:deep(.modal-body .el-select__wrapper) {
  height: 38px !important;
  min-height: unset !important;
  box-sizing: border-box;
}

.modal-body input {
  height: 38px;
  box-sizing: border-box;
}

.modal-body textarea {
  height: 70px;
  box-sizing: border-box;
}

/* 创建弹窗内输入框与下拉框背景色统一（与设置页一致） */
:deep(.modal-body .el-select__wrapper),
:deep(.modal-body .el-input__wrapper) {
  background: transparent !important;
  border: 1px solid var(--border-color) !important;
  box-shadow: none !important;
}

.modal-body input,
.modal-body select,
.modal-body textarea {
  background: transparent !important;
}
</style>

<style>
/* 知识库创建弹窗下拉框层级高于弹窗 */
.kb-create-popper {
  z-index: 10000 !important;
}

/* 暗色主题：下拉框边框半透明白色 */
.dark .toolbar-left .el-select__wrapper {
  border-color: rgba(255, 255, 255, 0.20) !important;
}

.dark .toolbar-left .el-select__wrapper:hover {
  border-color: rgba(255, 255, 255, 0.40) !important;
}

.dark .toolbar-left .el-select__wrapper.is-focus {
  border-color: var(--color-primary) !important;
  box-shadow: 0 0 0 1px var(--color-primary) !important;
}

/* 移除所有按钮点击后的浏览器焦点粗边框 */
button:focus,
button:focus-visible {
  outline: none !important;
  box-shadow: none !important;
}
</style>
