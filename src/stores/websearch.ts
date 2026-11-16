// FastClaw · 联网搜索状态（A 弹层 + B 设置页共享）
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface WebSearchConfig {
  enabled: boolean
  provider: string
  maxResults: number
  timeRange: 'none' | 'year' | 'month' | 'week'
  safeSearch: boolean
  inject: 'system' | 'cite'
  blacklist: string[]
  keys: { tavily?: string; exa?: string; bocha?: string }
}

const DEFAULT: WebSearchConfig = {
  enabled: false,
  provider: 'bocha',
  maxResults: 5,
  timeRange: 'year',
  safeSearch: true,
  inject: 'system',
  blacklist: [],
  keys: {},
}

// 服务商元数据（弹层/设置页共用）
export const PROVIDERS = [
  { id: 'bocha',   name: '博查',     icon: '博', color: '#5b5bf0', badge: 'key',  needs: 'bocha' },
  { id: 'tavily',  name: 'Tavily',  icon: 'T',  color: '#16a34a', badge: 'key',  needs: 'tavily' },
  { id: 'exa',    name: 'Exa',     icon: 'E',  color: '#1a56db', badge: 'key',  needs: 'exa' },
  { id: 'baidu',   name: 'Baidu',   icon: '百', color: '#2932e1', badge: 'free', needs: null },
] as const

export const useWebSearchStore = defineStore('websearch', () => {
  const cfg = ref<WebSearchConfig>({ ...DEFAULT })
  const settingsTab = ref<string | null>(null) // A 弹层点“更多设置”时置位，SettingsView 读取
  const loaded = ref(false)

  const enabled = computed({
    get: () => cfg.value.enabled,
    set: (v) => { cfg.value.enabled = v; save() },
  })
  const provider = computed({
    get: () => cfg.value.provider,
    set: (v) => { cfg.value.provider = v; save() },
  })

  // 服务商是否“就绪”（免费的恒就绪；需 Key 的看是否填了）
  const availableProviders = computed(() =>
    PROVIDERS.map((p) => ({
      ...p,
      ready: p.needs ? !!(cfg.value.keys as any)[p.needs] : true,
    })),
  )

  async function load() {
    if (loaded.value) return
    try {
      const rows: any[] = await invoke('get_settings', { category: 'websearch' })
      const row = rows.find((r) => r.key === 'config')
      if (row?.value) cfg.value = { ...DEFAULT, ...JSON.parse(row.value) }
    } catch (e) {
      console.warn('[websearch] load skipped', e)
    }
    // 迁移：已下线的 duckduckgo 自动切换
    if (cfg.value.provider === 'duckduckgo') {
      cfg.value.provider = DEFAULT.provider
      save()
    }
    loaded.value = true
  }

  async function save() {
    try {
      await invoke('update_setting_with_category', {
        key: 'config',
        value: JSON.stringify(cfg.value),
        category: 'websearch',
      })
    } catch (e) {
      console.warn('[websearch] save skipped', e)
    }
  }

  function patch(part: Partial<WebSearchConfig>) {
    cfg.value = { ...cfg.value, ...part }
    save()
  }

  async function test(query: string) {
    return await invoke<any[]>('web_search_test', { query, provider: cfg.value.provider })
  }

  return { cfg, enabled, provider, availableProviders, settingsTab, loaded, load, save, patch, test }
})
