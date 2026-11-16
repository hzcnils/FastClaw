import { invoke } from '@tauri-apps/api/core'
import { isHiddenReleaseProvider } from '@/utils/provider'
import { listProviderSummaries } from './api'

export interface ChatModelCatalogSnapshot {
  providers: Awaited<ReturnType<typeof listProviderSummaries>>
  tree: any[]
}

const CACHE_TTL_MS = 60_000
let snapshot: ChatModelCatalogSnapshot | null = null
let loadedAt = 0
let catalogRevision = 0
let appliedCatalogRevision = -1
let loadPromise: Promise<ChatModelCatalogSnapshot> | null = null

/** Cheap activation-time check; avoids remapping a still-current shared tree. */
export function isChatModelCatalogStale(now = Date.now()) {
  return !snapshot
    || appliedCatalogRevision !== catalogRevision
    || now - loadedAt >= CACHE_TTL_MS
}

/** Shared by every chat tab so model/provider IPC never scales with tab count. */
export function loadChatModelCatalog(force = false): Promise<ChatModelCatalogSnapshot> {
  if (force) invalidateChatModelCatalog()
  if (!force && snapshot && appliedCatalogRevision === catalogRevision && Date.now() - loadedAt < CACHE_TTL_MS) {
    return Promise.resolve(snapshot)
  }
  if (loadPromise) return loadPromise

  const requestRevision = catalogRevision
  let currentPromise!: Promise<ChatModelCatalogSnapshot>
  currentPromise = Promise.all([
    listProviderSummaries().catch((error) => {
      console.warn('list_provider_summaries failed', error)
      return []
    }),
    invoke<any[]>('list_models_tree').then(result => result || []),
  ])
    .then(async ([providers, tree]) => {
      // 模型管理写入若发生在请求期间，丢弃旧结果并串行补取最新快照。
      if (requestRevision !== catalogRevision) {
        if (loadPromise === currentPromise) loadPromise = null
        return loadChatModelCatalog()
      }
      const internetTree = (tree || []).filter((group: any) => !isHiddenReleaseProvider(
        group?.providerCode || group?.provider_code,
        group?.platformId,
      ))
      snapshot = { providers, tree: internetTree }
      loadedAt = Date.now()
      appliedCatalogRevision = requestRevision
      return snapshot
    })
    .finally(() => {
      if (loadPromise === currentPromise) loadPromise = null
    })

  loadPromise = currentPromise
  return currentPromise
}

export function invalidateChatModelCatalog() {
  catalogRevision += 1
  loadedAt = 0
}

/** Notify every cached chat view after a successful model-management write. */
export function notifyChatModelCatalogChanged(detail?: Record<string, unknown>) {
  invalidateChatModelCatalog()
  if (typeof document !== 'undefined') {
    document.dispatchEvent(new CustomEvent('fastclaw:model-updated', { detail }))
  }
}
