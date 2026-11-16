// FastClaw · 对话页 MCP 工具开关（与 WebSearch 弹层共享模式）
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface McpServerSummary {
  id: string
  name: string
  status: string
  tool_count: number
  description?: string
}

export interface McpChatConfig {
  enabled: boolean
  /** 空数组 = 使用全部已连接 Server */
  serverIds: string[]
}

const DEFAULT: McpChatConfig = {
  enabled: true,
  serverIds: [],
}

export const useMcpChatStore = defineStore('mcpChat', () => {
  const cfg = ref<McpChatConfig>({ ...DEFAULT })
  const loaded = ref(false)
  const servers = ref<McpServerSummary[]>([])
  const loadingServers = ref(false)
  const errorCode = ref<string | null>(null)
  let configLoadPromise: Promise<void> | null = null
  let serversLoadPromise: Promise<void> | null = null
  let serversLoadedAt = 0

  const enabled = computed({
    get: () => cfg.value.enabled,
    set: (v) => { cfg.value.enabled = v; save() },
  })

  const connectedServers = computed(() =>
    servers.value.filter((s) => s.status === 'Running' && s.tool_count > 0),
  )

  const activeToolCount = computed(() =>
    connectedServers.value.reduce((sum, s) => sum + s.tool_count, 0),
  )

  const hasReadyServers = computed(() => connectedServers.value.length > 0)

  function isServerSelected(id: string): boolean {
    if (cfg.value.serverIds.length === 0) return true
    return cfg.value.serverIds.includes(id)
  }

  function toggleServer(id: string) {
    const connected = connectedServers.value.map((s) => s.id)
    if (connected.length === 0) return

    let next = [...cfg.value.serverIds]
    if (next.length === 0) {
      next = connected.filter((sid) => sid !== id)
    } else if (next.includes(id)) {
      next = next.filter((sid) => sid !== id)
    } else {
      next.push(id)
    }

    if (next.length === 0 || next.length >= connected.length) {
      cfg.value.serverIds = []
    } else {
      cfg.value.serverIds = next
    }
    save()
  }

  async function load() {
    if (loaded.value) return
    if (configLoadPromise) return configLoadPromise
    configLoadPromise = (async () => {
      try {
        const rows: any[] = await invoke('get_settings', { category: 'mcp_chat' })
        const row = rows.find((r) => r.key === 'config')
        if (row?.value) cfg.value = { ...DEFAULT, ...JSON.parse(row.value) }
      } catch (e) {
        console.warn('[mcpChat] load skipped', e)
      }
      loaded.value = true
    })().finally(() => { configLoadPromise = null })
    return configLoadPromise
  }

  async function save() {
    try {
      await invoke('update_setting_with_category', {
        key: 'config',
        value: JSON.stringify(cfg.value),
        category: 'mcp_chat',
      })
    } catch (e) {
      console.warn('[mcpChat] save skipped', e)
    }
  }

  async function refreshServers() {
    if (serversLoadPromise) return serversLoadPromise
    serversLoadPromise = performRefreshServers()
      .finally(() => { serversLoadPromise = null })
    return serversLoadPromise
  }

  async function performRefreshServers() {
    loadingServers.value = true
    errorCode.value = null
    try {
      const list = await invoke<McpServerSummary[]>('list_mcp_servers')
      servers.value = (list || []).map((s) => ({
        id: s.id,
        name: s.name || s.id,
        status: normalizeStatus(s.status),
        tool_count: s.tool_count ?? 0,
        description: s.description,
      }))
      serversLoadedAt = Date.now()
    } catch (e) {
      console.warn('[mcpChat] refreshServers failed', e)
      servers.value = []
      errorCode.value = normalizeErrorCode(e)
    } finally {
      loadingServers.value = false
    }
  }

  async function ensureServersLoaded(maxAgeMs = 30_000) {
    if (servers.value.length > 0 && Date.now() - serversLoadedAt < maxAgeMs) return
    return refreshServers()
  }

  return {
    cfg,
    enabled,
    loaded,
    servers,
    connectedServers,
    activeToolCount,
    hasReadyServers,
    loadingServers,
    errorCode,
    load,
    save,
    refreshServers,
    ensureServersLoaded,
    isServerSelected,
    toggleServer,
  }
})

function normalizeStatus(status: unknown): string {
  if (typeof status === 'string') {
    if (status === 'running' || status === 'Running') return 'Running'
    if (status.startsWith('Error')) return 'Error'
    return status
  }
  if (status && typeof status === 'object' && 'Error' in (status as object)) return 'Error'
  return 'Stopped'
}

function normalizeErrorCode(error: unknown): string {
  const message = String(error || '').toLowerCase()
  if (message.includes('mcp manager unavailable')) return 'managerUnavailable'
  return 'loadFailed'
}
