// FastClaw · 应用配置 Store
// 统一从 exe 同目录 config.json（Rust ConfigManager）读取全部配置。
// 所有默认值定义在 @/config/constants.ts，本文件不定义任何硬编码值。

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { DEFAULT_FRONTEND_CONFIG } from '@/types/app-config'
import {
  HTTP_SERVER_DEFAULTS,
  SERVICE_URL_DEFAULTS,
  REMOTE_SERVER_DEFAULTS,
  KNOWLEDGE_SERVICE_DEFAULTS,
  BRANDING_DEFAULTS,
  WEBVIEW2_DEFAULTS,
} from '@/config/constants'

async function invokeCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<T>(command, args)
}

// ─── 重新导出（供其他模块统一从 appConfig 导入）──────────────

export {
  SERVICE_LABELS,
  REMOTE_SERVER_LABELS,
  REMOTE_SERVER_KEYS,
} from '@/config/constants'

// ─── 类型定义 ────────────────────────────────────────────────

export interface HttpServerConfig {
  host: string
  port_start: number
  port_end: number
}

export interface RemoteServersConfig {
  download_api_base_url: string
  download_api_base_url_private: string
  model_sync_url: string
  fastci_base_url: string
  fastci_model_download_url: string
  independent_model_data: boolean
  independent_model_api_url: string
}

export interface BrandingConfig {
  company_name: string
  app_name: string
  app_logo: string
  chat_model_icon: string
}

export interface KnowledgeServicesConfig {
  vector_backend: string
  vector_url: string
  vector_api_key_env: string
  embedding_url: string
  embedding_model: string
  embedding_api_key_env: string
  reranker_url: string
  reranker_model: string
  reranker_api_key_env: string
}

export type WebView2NetworkProbeConfig =
  | { type: 'tcp'; target: string }
  | { type: 'dns'; host: string }
  | { type: 'http'; url: string }

export interface WebView2Config {
  min_version: string
  online_installer_url: string
  /** 无法直连互联网时提供的内网下载地址 */
  offline_installer_url: string
  network_timeout_secs: number
  internet_probes: WebView2NetworkProbeConfig[]
}

export interface AppConfig {
  version: number
  http_server: HttpServerConfig
  services: Record<string, string>
  menus: Record<string, boolean>
  settings: Record<string, boolean>
  remote_servers: RemoteServersConfig
  knowledge: KnowledgeServicesConfig
  branding: BrandingConfig
  webview2: WebView2Config
}

// ─── 默认值（全部从 constants.ts 导入，此处仅组装结构）───────

export const DEFAULTS: AppConfig = {
  version: 6,
  http_server: { ...HTTP_SERVER_DEFAULTS },
  services: { ...SERVICE_URL_DEFAULTS },
  menus: { ...DEFAULT_FRONTEND_CONFIG.menus },
  settings: { ...DEFAULT_FRONTEND_CONFIG.settings },
  remote_servers: { ...REMOTE_SERVER_DEFAULTS },
  knowledge: { ...KNOWLEDGE_SERVICE_DEFAULTS },
  branding: { ...BRANDING_DEFAULTS },
  webview2: {
    ...WEBVIEW2_DEFAULTS,
    internet_probes: WEBVIEW2_DEFAULTS.internet_probes.map(probe => ({ ...probe })),
  },
}

// ─── Store ──────────────────────────────────────────────────

export const useAppConfigStore = defineStore('appConfig', () => {
  const config = ref<AppConfig>({ ...DEFAULTS })
  const loaded = ref(false)

  // ─── 派生状态：菜单/设置页可见性 ──────────────────────────

  const menuVisibility = computed<Record<string, boolean>>(() => {
    const result = { ...DEFAULT_FRONTEND_CONFIG.menus }
    const cfg = config.value.menus
    if (cfg && typeof cfg === 'object') {
      for (const key of Object.keys(result)) {
        if (key in cfg) {
          const val = cfg[key]
          result[key] = Boolean(val)
        }
      }
    }
    return result
  })

  const settingsVisibility = computed<Record<string, boolean>>(() => {
    const result = { ...DEFAULT_FRONTEND_CONFIG.settings }
    const cfg = config.value.settings
    if (cfg && typeof cfg === 'object') {
      for (const key of Object.keys(result)) {
        if (key in cfg) {
          const val = cfg[key]
          result[key] = Boolean(val)
        }
      }
    }
    return result
  })

  function isMenuVisible(menuId: string): boolean {
    return menuVisibility.value[menuId] !== false
  }

  function isSettingVisible(settingKey: string): boolean {
    return settingsVisibility.value[settingKey] !== false
  }

  const hasAnyVisibleSettings = computed(() =>
    Object.values(settingsVisibility.value).some(v => v !== false)
  )

  // ─── 配置加载与保存 ──────────────────────────────────────

  async function loadConfig() {
    try {
      const cfg = await invokeCommand<AppConfig>('get_app_config')
      config.value = cfg
      loaded.value = true
      console.info('[AppConfig] 配置加载成功')
    } catch (e) {
      console.warn('[AppConfig] 加载配置失败，使用默认值:', e)
      config.value = { ...DEFAULTS }
      loaded.value = true
    }
  }

  async function saveBusinessConfig(
    menus: Record<string, boolean>,
    settings: Record<string, boolean>,
  ) {
    const updated = { ...config.value }
    updated.menus = { ...menus }
    updated.settings = { ...settings }
    await invokeCommand('update_app_config', { config: updated })
    config.value = updated
    console.info('[AppConfig] 业务配置已保存')
  }

  async function updateField(dotPath: string, value: string) {
    try {
      await invokeCommand('update_app_config_field', { dotPath, value })
      setByDotPath(config.value, dotPath, value)
    } catch (e) {
      console.error(`[AppConfig] 更新 ${dotPath} 失败:`, e)
      throw e
    }
  }

  function serviceUrl(name: string): string {
    return config.value.services[name] ?? DEFAULTS.services[name] ?? ''
  }

  async function resetServiceUrl(name: string) {
    try {
      await invokeCommand('reset_service_url', { serviceName: name })
      await loadConfig()
    } catch (e) {
      console.error(`[AppConfig] 重置 ${name} 失败:`, e)
    }
  }

  function remoteServerUrl(key: string): string {
    const val = (config.value.remote_servers as unknown as Record<string, unknown>)[key]
      ?? (DEFAULTS.remote_servers as unknown as Record<string, unknown>)[key]
    if (val == null) return ''
    return typeof val === 'boolean' ? String(val) : String(val)
  }

  async function resetRemoteServerUrl(key: string) {
    try {
      await invokeCommand('reset_remote_server_url', { serverKey: key })
      await loadConfig()
    } catch (e) {
      console.error(`[AppConfig] 重置远程服务器 ${key} 失败:`, e)
    }
  }

  return {
    config,
    loaded,
    loadConfig,
    saveBusinessConfig,
    updateField,
    serviceUrl,
    resetServiceUrl,
    remoteServerUrl,
    resetRemoteServerUrl,
    menuVisibility,
    settingsVisibility,
    isMenuVisible,
    isSettingVisible,
    hasAnyVisibleSettings,
  }
})

// ─── 工具函数 ────────────────────────────────────────────────

function setByDotPath(obj: any, dotPath: string, value: string) {
  const segments = dotPath.split('.')
  let current = obj
  for (let i = 0; i < segments.length - 1; i++) {
    if (!(segments[i] in current)) {
      current[segments[i]] = {}
    }
    current = current[segments[i]]
  }
  const lastKey = segments[segments.length - 1]
  try {
    current[lastKey] = JSON.parse(value)
  } catch {
    current[lastKey] = value
  }
}
