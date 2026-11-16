// FastClaw · 统一配置入口
// - 本地服务/远程服务器 URL：运行时从 config.json 动态获取
// - 默认值回退：从 constants.ts 导入
// - 搜索 API 等：从 constants.ts 导出
//
// 向后兼容：保留 DEFAULT_CONFIG 代理对象

import { useAppConfigStore } from '@/stores/appConfig'
import {
  SERVICE_URL_DEFAULTS,
  REMOTE_SERVER_DEFAULTS,
  CONNECTIVITY_CHECK_URL,
  CONNECTIVITY_CHECK_URL_FALLBACK,
} from '@/config/constants'

// ─── 重新导出常量（兼容旧导入路径）──────────────────────────

export {
  CONNECTIVITY_CHECK_URL,
  CONNECTIVITY_CHECK_URL_FALLBACK,
}

// ─── 动态获取器 ──────────────────────────────────────────────

function getStore() {
  try {
    return useAppConfigStore()
  } catch {
    return null
  }
}

export function getFastPlanUrl() {
  const store = getStore()
  // 无论是 DEV 还是 PROD，都通过 Rust HTTP 代理（127.0.0.1:5179）转发到 FastPlan
  // 这样可以确保 SSE 和其他 API 行为一致，避免 Vite 代理对 SSE 支持问题
  // 代理端口由 Tauri 动态分配，默认 5179
  return 'http://127.0.0.1:5179/FastPlan'
}

export function getFastKgUrl() {
  const store = getStore()
  return store?.serviceUrl('fastkg') ?? SERVICE_URL_DEFAULTS.fastkg
}

export function getServiceUrl(name) {
  const store = getStore()
  return store?.serviceUrl(name) ?? SERVICE_URL_DEFAULTS[name] ?? ''
}

export function getRemoteServerUrl(key) {
  const store = getStore()
  return store?.remoteServerUrl(key) ?? REMOTE_SERVER_DEFAULTS[key] ?? ''
}

// ─── 向后兼容：DEFAULT_CONFIG 代理对象 ───────────────────────

export const DEFAULT_CONFIG = new Proxy({}, {
  get(_, prop) {
    const store = getStore()
    switch (prop) {
      case 'FastPlan':
        return getFastPlanUrl()
      case 'WORKFLOW_URL':
        return getServiceUrl('workflow')
      case 'DOWNLOAD_API_BASE_URL':
        return getRemoteServerUrl('download_api_base_url')
      case 'DOWNLOAD_API_BASE_URL_PRIVATE':
        return getRemoteServerUrl('download_api_base_url_private')
      case 'CONNECTIVITY_CHECK_URL':
        return CONNECTIVITY_CHECK_URL
      case 'MODEL_SYNC_URL':
        return getRemoteServerUrl('model_sync_url')
      case 'FASTCI_BASE_URL':
        return getRemoteServerUrl('fastci_base_url')
      case 'FASTCI_MODEL_DOWNLOAD_URL':
        return getRemoteServerUrl('fastci_model_download_url')
      case 'KB_SEARCH_API_URL':
        return `${getFastKgUrl()}/api/v1/search`
      case 'QUICK_UPLOAD_API_URL':
        return `${getFastKgUrl()}/api/v1/documents/quick-upload`
      default:
        return undefined
    }
  }
})
