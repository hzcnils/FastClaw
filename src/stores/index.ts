import { acceptHMRUpdate, defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { invoke, isTauri } from '@tauri-apps/api/core'
import { matchingLocalKnowledgeBase, uniqueSelections } from '@/utils/localKnowledgeBatch'
import { knowledgeBaseApi, kgServiceReady, ensureKgReady } from '@/api/knowledge'
import type { LocalKnowledgeBase, LocalKbSource } from '@/types/knowledge'
import type { FilesystemSelection } from '@/components/chat/FilesystemBrowserModal.vue'

const LOCAL_KB_STORAGE_KEY = 'fastclaw-local-kb'

export const useKnowledgeBaseStore = defineStore('knowledgeBase', () => {
  const loading = ref(false)
  const error = ref<string | null>(null)
  const knowledgeBases = ref<any[]>([])
  const currentKnowledgeBase = ref<any>(null)
  let knowledgeCatalogLoadedAt = 0
  let knowledgeCatalogPromise: Promise<void> | null = null

  // ─── 本地知识库（多源模型） ───────────────────
  const localKnowledgeBases = ref<LocalKnowledgeBase[]>([])
  const localKbScanStates = ref<Record<string, { loading: boolean; truncated: boolean; errors: string[] }>>({})
  const sourceScans = new Map<string, Promise<{ files: string[]; errors: { path: string; message: string }[]; truncated: boolean }>>()

  function importLocalKnowledgeBatch(selections: FilesystemSelection[]) {
    const sources = uniqueSelections(selections)
    if (!sources.length) return null
    return matchingLocalKnowledgeBase(localKnowledgeBases.value, sources) || createLocalKnowledgeBase('', sources)
  }

  async function scanLocalKb(kb: LocalKnowledgeBase) {
    const signature = JSON.stringify(kb.sources)
    const key = kb.id + signature
    const pending = sourceScans.get(key)
    if (pending) return pending
    localKbScanStates.value[kb.id] = { loading: true, truncated: false, errors: [] }
    const task = invoke<{ files: string[]; errors: { path: string; message: string }[]; truncated: boolean }>(
      'list_local_source_files', { sources: kb.sources.map(s => ({ path: s.path, recursive: !!s.recursive })) },
    ).then(result => {
      if (localKnowledgeBases.value.includes(kb) && JSON.stringify(kb.sources) === signature) {
        kb.fileCount = result.files.length
        kb.exists = result.errors.length === 0
        localKbScanStates.value[kb.id] = { loading: false, truncated: result.truncated, errors: result.errors.map(e => `${e.path}: ${e.message}`) }
      }
      return result
    }).catch(error => {
      if (localKnowledgeBases.value.includes(kb) && JSON.stringify(kb.sources) === signature) {
        localKbScanStates.value[kb.id] = { loading: false, truncated: false, errors: [String(error)] }
      }
      throw error
    }).finally(() => { sourceScans.delete(key) })
    sourceScans.set(key, task)
    return task
  }

  function loadLocalKnowledgeBases() {
    try {
      const raw = localStorage.getItem(LOCAL_KB_STORAGE_KEY)
      if (raw) {
        const data = JSON.parse(raw)
        // 向后兼容迁移：旧格式 { path, recursive } → 新格式 { sources }
        localKnowledgeBases.value = data.map((item: any) => {
          if (!item.sources && item.path) {
            return {
              id: item.id,
              name: item.name,
              type: 'local' as const,
              enabled: item.enabled ?? true,
              sources: [{
                path: item.path,
                entryType: 'directory' as const,
                recursive: item.recursive || false,
              }],
              fileCount: item.fileCount,
              exists: item.exists,
            }
          }
          // 确保 sources 中的 entryType 存在（兼容迁移过程中的中间态）
          if (item.sources) {
            item.sources = item.sources.map((s: any) => ({
              ...s,
              entryType: s.entryType || 'directory',
            }))
          }
          return item
        })
      }
    } catch { localKnowledgeBases.value = [] }
  }

  function persistLocalKnowledgeBases() {
    try {
      localStorage.setItem(LOCAL_KB_STORAGE_KEY, JSON.stringify(localKnowledgeBases.value))
    } catch { /* localStorage 不可用 */ }
  }

  /** 从 FilesystemBrowserModal 的选中结果创建新 KB */
  function createLocalKnowledgeBase(name: string, selections: FilesystemSelection[]) {
    const kb: LocalKnowledgeBase = {
      id: crypto.randomUUID(),
      name: name || tDefaultName(selections),
      type: 'local',
      enabled: true,
      sources: selections.map(s => ({
        path: s.path,
        entryType: s.type === 'directory' ? 'directory' as const : 'file' as const,
        recursive: false,
      })),
    }
    localKnowledgeBases.value.push(kb)
    persistLocalKnowledgeBases()
    return kb
  }

  function tDefaultName(selections: FilesystemSelection[]): string {
    if (selections.length === 0) return '本地知识库'
    if (selections.length === 1) return selections[0].name
    return selections[0].name + ` 等${selections.length}项`
  }

  /** 向已有 KB 追加 sources（去重） */
  function addSourcesToLocalKb(kbId: string, selections: FilesystemSelection[]) {
    const kb = localKnowledgeBases.value.find(k => k.id === kbId)
    if (!kb) return
    for (const s of selections) {
      if (kb.sources.some(src => src.path === s.path)) continue
      kb.sources.push({
        path: s.path,
        entryType: s.type === 'directory' ? 'directory' as const : 'file' as const,
        recursive: false,
      })
    }
    persistLocalKnowledgeBases()
  }

  /** 移除 KB 中的单个 source；若无 source 则移除整个 KB */
  function removeLocalKbSource(kbId: string, sourcePath: string) {
    const kb = localKnowledgeBases.value.find(k => k.id === kbId)
    if (!kb) return
    kb.sources = kb.sources.filter(s => s.path !== sourcePath)
    if (kb.sources.length === 0) {
      removeLocalKnowledgeBase(kbId)
    } else {
      persistLocalKnowledgeBases()
    }
  }

  function removeLocalKnowledgeBase(id: string) {
    localKnowledgeBases.value = localKnowledgeBases.value.filter(k => k.id !== id)
    delete localKbScanStates.value[id]
    persistLocalKnowledgeBases()
  }

  function updateLocalKnowledgeBase(id: string, patch: Partial<Pick<LocalKnowledgeBase, 'name' | 'enabled'>>) {
    const kb = localKnowledgeBases.value.find(k => k.id === id)
    if (kb) {
      Object.assign(kb, patch)
      persistLocalKnowledgeBases()
    }
  }

  /** 更新单个 source 的属性（如 toggle recursive） */
  function updateLocalKbSource(kbId: string, sourcePath: string, patch: Partial<LocalKbSource>) {
    const kb = localKnowledgeBases.value.find(k => k.id === kbId)
    if (!kb) return
    const src = kb.sources.find(s => s.path === sourcePath)
    if (src) {
      Object.assign(src, patch)
      persistLocalKnowledgeBases()
    }
  }

  async function validateLocalKbs() {
    if (!isTauri()) return
    for (const kb of [...localKnowledgeBases.value]) {
      try { await scanLocalKb(kb) } catch { /* surfaced in the selector */ }
    }
  }

  /** Counts and previews use the same bounded, deduplicated scan. */
  async function getLocalKbFileList(kbId: string): Promise<string[]> {
    const kb = localKnowledgeBases.value.find(k => k.id === kbId)
    if (!kb || !isTauri()) return []
    return (await scanLocalKb(kb)).files
  }

  // 统一列表：远程在前，本地在后
  const allKnowledgeBases = computed(() => {
    const remote = knowledgeBases.value.map(kb => ({ ...kb, _type: 'remote' as const }))
    const local = localKnowledgeBases.value.map(kb => ({ ...kb, _type: 'local' as const }))
    return [...remote, ...local]
  })

  // 选中 ID 对应的本地 KB 路径（扁平化所有 source）
  function getEnabledLocalPaths(ids: string[]): { path: string; recursive: boolean }[] {
    return localKnowledgeBases.value
      .filter(k => ids.includes(k.id) && k.enabled)
      .flatMap(k => k.sources.map(s => ({
        path: s.path,
        recursive: s.entryType === 'directory' ? !!s.recursive : false,
      })))
  }

  const activeKnowledgeBases = computed(() => knowledgeBases.value.filter(kb => kb.status === 'active'))
  const totalFiles = computed(() => knowledgeBases.value.reduce((sum, kb) => sum + (kb.file_count || 0), 0))
  const totalChunks = computed(() => knowledgeBases.value.reduce((sum, kb) => sum + (kb.chunk_count || 0), 0))

  async function checkKgService() {
    await ensureKgReady({ startIfNeeded: false })
  }

  async function fetchKnowledgeBases() {
    if (!kgServiceReady.value) return
    loading.value = true
    error.value = null
    try {
      // 动态导入，避免 keep-alive / Pinia 仍持有直连 :3000 的旧 axios 实例
      const { knowledgeBaseApi: api } = await import('@/api/knowledge')
      const result = await api.list({ "user_id": 'admin' })
      knowledgeBases.value = Array.isArray(result) ? result : []
      knowledgeCatalogLoadedAt = Date.now()
    } catch (e) {
      error.value = e instanceof Error ? e.message : '获取知识库列表失败'
      console.warn('获取知识库列表失败:', error.value)
    } finally {
      loading.value = false
    }
  }

  async function ensureKnowledgeBases(force = false, maxAgeMs = 60_000) {
    if (!force && knowledgeCatalogLoadedAt > 0 && Date.now() - knowledgeCatalogLoadedAt < maxAgeMs) return
    if (knowledgeCatalogPromise) return knowledgeCatalogPromise

    knowledgeCatalogPromise = (async () => {
      // 已就绪时直接复用应用级状态，避免列表恢复时再次执行约 4 秒的深度健康检查。
      if (!kgServiceReady.value) {
        await ensureKgReady()
      }
      if (kgServiceReady.value) await fetchKnowledgeBases()
    })().finally(() => {
      knowledgeCatalogPromise = null
    })
    return knowledgeCatalogPromise
  }

  async function fetchKnowledgeBase(id: string) {
    if (!kgServiceReady.value) return null
    loading.value = true
    try {
      currentKnowledgeBase.value = await knowledgeBaseApi.get(id)
    } catch (e) {
      error.value = e instanceof Error ? e.message : '获取知识库详情失败'
    } finally {
      loading.value = false
    }
    return currentKnowledgeBase.value
  }

  async function createKnowledgeBase(data: any) {
    if (!kgServiceReady.value) throw new Error('知识库服务未就绪')
    loading.value = true
    try {
      const newKb = await knowledgeBaseApi.create(data)
      knowledgeBases.value.push(newKb)
      return newKb
    } catch (e) {
      error.value = e instanceof Error ? e.message : '创建知识库失败'
      throw e
    } finally {
      loading.value = false
    }
  }

  async function updateKnowledgeBase(id: string, data: any) {
    if (!kgServiceReady.value) throw new Error('知识库服务未就绪')
    loading.value = true
    try {
      const updated = await knowledgeBaseApi.update(id, data)
      const idx = knowledgeBases.value.findIndex(kb => kb.id === id)
      if (idx !== -1) knowledgeBases.value[idx] = updated
      if (currentKnowledgeBase.value?.id === id) currentKnowledgeBase.value = updated
      return updated
    } catch (e) {
      error.value = e instanceof Error ? e.message : '更新知识库失败'
      throw e
    } finally {
      loading.value = false
    }
  }

  async function deleteKnowledgeBase(id: string) {
    if (!kgServiceReady.value) throw new Error('知识库服务未就绪')
    loading.value = true
    try {
      await knowledgeBaseApi.delete(id)
      knowledgeBases.value = knowledgeBases.value.filter(kb => kb.id !== id)
      if (currentKnowledgeBase.value?.id === id) currentKnowledgeBase.value = null
    } catch (e) {
      error.value = e instanceof Error ? e.message : '删除知识库失败'
      throw e
    } finally {
      loading.value = false
    }
  }

  function clearError() {
    error.value = null
  }

  // 初始化：加载 localStorage 中的本地 KB
  loadLocalKnowledgeBases()

  return {
    knowledgeBases,
    localKnowledgeBases,
    currentKnowledgeBase,
    loading,
    error,
    kgServiceReady,
    activeKnowledgeBases,
    totalFiles,
    totalChunks,
    allKnowledgeBases,
    checkKgService,
    fetchKnowledgeBases,
    ensureKnowledgeBases,
    fetchKnowledgeBase,
    createKnowledgeBase,
    updateKnowledgeBase,
    deleteKnowledgeBase,
    clearError,
    createLocalKnowledgeBase,
    importLocalKnowledgeBatch,
    localKbScanStates,
    addSourcesToLocalKb,
    removeLocalKbSource,
    removeLocalKnowledgeBase,
    updateLocalKnowledgeBase,
    updateLocalKbSource,
    validateLocalKbs,
    getLocalKbFileList,
    getEnabledLocalPaths,
  }
})

if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useKnowledgeBaseStore, import.meta.hot))
}

export const useLLMStore = defineStore('llmCompat', () => {
  const models = ref<any[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)
  const chatModels = computed(() => models.value.filter(m => m.model_type === 'chat' && m.is_active))
  const embeddingModels = computed(() => models.value.filter(m => m.model_type === 'embedding' && m.is_active))
  const rerankModels = computed(() => models.value.filter(m => m.model_type === 'rerank' && m.is_active))
  async function fetchModels() {
    models.value = []
  }
  return { models, loading, error, chatModels, embeddingModels, rerankModels, fetchModels }
})

export const useTaskStore = defineStore('taskCompat', () => {
  const tasks = ref<any[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)
  const runningTasks = computed(() => tasks.value.filter(t => t.status === 'running'))
  const pendingTasks = computed(() => tasks.value.filter(t => t.status === 'pending'))
  const failedTasks = computed(() => tasks.value.filter(t => t.status === 'failed'))
  async function fetchTasks() {
    tasks.value = []
  }
  async function retryTask() {
    return null
  }
  return { tasks, loading, error, runningTasks, pendingTasks, failedTasks, fetchTasks, retryTask }
})

export const useStatsStore = defineStore('statsCompat', () => {
  const stats = ref<any>(null)
  const loading = ref(false)
  async function fetchStats() {
    stats.value = null
  }
  return { stats, loading, fetchStats }
})

// 全局 UI Store
export const useUIStore = defineStore('ui', () => {
  const sidebarCollapsed = ref(false);
  const theme = ref<'dark' | 'light'>('dark');
  const toasts = ref<Array<{
    id: number;
    type: 'success' | 'error' | 'warning' | 'info';
    message: string;
    duration?: number;
  }>>([]);

  let toastId = 0;

  function toggleSidebar() {
    sidebarCollapsed.value = !sidebarCollapsed.value;
  }

  function setTheme(newTheme: 'dark' | 'light') {
    theme.value = newTheme;
    document.documentElement.classList.remove('dark', 'light');
    document.documentElement.classList.add(newTheme);
    localStorage.setItem('theme', newTheme);
  }

  function initTheme() {
    const savedTheme = localStorage.getItem('theme') as 'dark' | 'light' | null;
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    setTheme(savedTheme || (prefersDark ? 'dark' : 'light'));
  }

  function showToast(
    type: 'success' | 'error' | 'warning' | 'info',
    message: string,
    duration = 3000
  ) {
    const id = ++toastId;
    toasts.value.push({ id, type, message, duration });

    if (duration > 0) {
      setTimeout(() => {
        removeToast(id);
      }, duration);
    }

    return id;
  }

  function removeToast(id: number) {
    const index = toasts.value.findIndex((t) => t.id === id);
    if (index !== -1) {
      toasts.value.splice(index, 1);
    }
  }

  return {
    sidebarCollapsed,
    theme,
    toasts,
    toggleSidebar,
    setTheme,
    initTheme,
    showToast,
    removeToast,
  };
});
