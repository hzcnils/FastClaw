// FastClaw · 共享网络连通性状态
// App.vue onMounted 时调用 checkConnectivity() 初始化
// 所有组件通过 isOnline 读取当前状态

import { ref } from 'vue'
import { DEFAULT_CONFIG } from '@/config/index'

/** null=检测中, true=在线, false=离线 */
export const isOnline = ref<boolean | null>(null)

let checked = false
let listenersSetup = false

/** 将连通性状态持久化到数据库，确保下次启动时 get_internet_status 读到最新值 */
async function persistConnectivityState(connected: boolean) {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('update_setting', {
      key: 'internet_connected',
      value: connected ? 'true' : 'false',
    })
  } catch {
    // 持久化失败不影响主流程
  }
}

/** 监听系统网络状态变化（Tauri webview 支持 online/offline 事件） */
function setupNetworkListeners() {
  if (listenersSetup || typeof window === 'undefined') return
  listenersSetup = true

  window.addEventListener('online', () => {
    if (isOnline.value !== true) {
      recheckConnectivity()
    }
  })

  window.addEventListener('offline', () => {
    isOnline.value = false
    persistConnectivityState(false)
  })
}

export async function checkConnectivity() {
  if (checked) return
  checked = true
  isOnline.value = null

  setupNetworkListeners()

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const connected: boolean = await invoke('get_internet_status')
    isOnline.value = connected
  } catch {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const connected: boolean = await invoke('check_internet_connectivity', {
        url: DEFAULT_CONFIG.CONNECTIVITY_CHECK_URL,
      })
      isOnline.value = connected
      persistConnectivityState(connected)
    } catch {
      isOnline.value = false
      persistConnectivityState(false)
    }
  }
}

/** 手动重新检测（供"重新检测"按钮调用） */
export async function recheckConnectivity() {
  isOnline.value = null
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const connected: boolean = await invoke('check_internet_connectivity', {
      url: DEFAULT_CONFIG.CONNECTIVITY_CHECK_URL,
    })
    isOnline.value = connected
    persistConnectivityState(connected)
  } catch {
    isOnline.value = false
    persistConnectivityState(false)
  }
}

/** 下载前快速复检：确保当前真的能连网，避免断网后还显示"可在线下载" */
export async function ensureOnlineBeforeDownload(): Promise<boolean> {
  if (typeof navigator !== 'undefined' && !navigator.onLine) {
    isOnline.value = false
    persistConnectivityState(false)
    return false
  }
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const connected: boolean = await invoke('check_internet_connectivity', {
      url: DEFAULT_CONFIG.CONNECTIVITY_CHECK_URL,
    })
    isOnline.value = connected
    persistConnectivityState(connected)
    return connected
  } catch {
    isOnline.value = false
    persistConnectivityState(false)
    return false
  }
}
