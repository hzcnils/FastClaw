// FastClaw · 系统品牌（公司名 + 名称 + 图标）共享状态
// - 持久化到 config.json → branding 节（可随文件分发到其他电脑）
// - 任意组件读取：import { displayCompanyName, appLogo, displayName } from '@/api/branding'
//   标题栏 logo 用法：:src="appLogo || 默认logo"，名称用法：{{ displayName }}

import { ref, computed } from 'vue'
import { BRANDING_DEFAULTS } from '@/config/constants'

export const DEFAULT_APP_NAME = 'FastClaw'
export const DEFAULT_COMPANY_NAME = BRANDING_DEFAULTS.company_name

/** 公司名称；所有公司署名统一读取此值 */
export const companyName = ref<string>(DEFAULT_COMPANY_NAME)
/** 自定义系统名称（空 = 用默认 FastClaw） */
export const appName = ref<string>('')
/** 自定义系统图标（data URL；空 = 用打包内置 logo） */
export const appLogo = ref<string>('')
/** 自定义对话模型图标（data URL；空 = 用默认模型图标） */
export const chatModelIcon = ref<string>('')

/** 实际展示用名称（带默认回退） */
export const displayName = computed(() => appName.value?.trim() || DEFAULT_APP_NAME)
/** 实际展示用公司名称（带默认回退） */
export const displayCompanyName = computed(
  () => companyName.value?.trim() || DEFAULT_COMPANY_NAME,
)
/** 关于页等位置共用的版权文案 */
export const displayCopyright = computed(
  () => `Copyright © 2024 ${displayName.value} · ${displayCompanyName.value}`,
)

let loaded = false

type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>
async function getInvoke(): Promise<InvokeFn | null> {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    return invoke as InvokeFn
  } catch {
    return null
  }
}

/** 读取 config.json 中的自定义名称/图标（幂等） */
export async function loadBranding(): Promise<void> {
  if (loaded) return
  loaded = true
  try {
    const { useAppConfigStore } = await import('@/stores/appConfig')
    const store = useAppConfigStore()
    if (!store.loaded) await store.loadConfig()
    const b = store.config.branding
    if (b) {
      companyName.value = b.company_name || DEFAULT_COMPANY_NAME
      if (b.app_name) appName.value = b.app_name
      if (b.app_logo) appLogo.value = b.app_logo
      if (b.chat_model_icon) chatModelIcon.value = b.chat_model_icon
    }
  } catch {
    // 非 Pinia 环境或加载失败 → 保持默认空值
  }
}

/** 保存公司名称到 config.json */
export async function setCompanyName(name: string): Promise<void> {
  companyName.value = name
  try {
    const { useAppConfigStore } = await import('@/stores/appConfig')
    const store = useAppConfigStore()
    await store.updateField('branding.company_name', JSON.stringify(name))
  } catch (e) {
    console.warn('persist company_name to config.json failed', e)
  }
}

/** 保存名称到 config.json */
export async function setAppName(name: string): Promise<void> {
  appName.value = name
  try {
    const { useAppConfigStore } = await import('@/stores/appConfig')
    const store = useAppConfigStore()
    await store.updateField('branding.app_name', JSON.stringify(name))
  } catch (e) {
    console.warn('persist app_name to config.json failed', e)
  }
}

/** 保存图标到 config.json（dataUrl 为空 = 恢复默认） */
export async function setAppLogo(dataUrl: string): Promise<void> {
  appLogo.value = dataUrl
  try {
    const { useAppConfigStore } = await import('@/stores/appConfig')
    const store = useAppConfigStore()
    await store.updateField('branding.app_logo', JSON.stringify(dataUrl))
  } catch (e) {
    console.warn('persist app_logo to config.json failed', e)
  }
}

/** 保存对话模型图标到 config.json */
export async function setChatModelIcon(dataUrl: string): Promise<void> {
  chatModelIcon.value = dataUrl
  try {
    const { useAppConfigStore } = await import('@/stores/appConfig')
    const store = useAppConfigStore()
    await store.updateField('branding.chat_model_icon', JSON.stringify(dataUrl))
  } catch (e) {
    console.warn('persist chat_model_icon to config.json failed', e)
  }
}

export async function resetAppName(): Promise<void> {
  return setAppName('')
}
export async function resetCompanyName(): Promise<void> {
  return setCompanyName(DEFAULT_COMPANY_NAME)
}
export async function resetAppLogo(): Promise<void> {
  return setAppLogo('')
}
export async function resetChatModelIcon(): Promise<void> {
  return setChatModelIcon('')
}

/** 把图片文件读成 data URL（供上传图标用） */
export function fileToDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result || ''))
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(file)
  })
}
