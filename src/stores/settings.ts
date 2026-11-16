// FastClaw · 设置状态
import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import i18n from '@/i18n'
import {
  DEFAULT_TAB_SESSION_RESTORE,
  TAB_SESSION_RESTORE_KEY,
  TAB_SESSION_SNAPSHOT_KEY,
  type TabSessionRestoreMode,
  type TabSessionSnapshot,
  mirrorRestoreModeToLocalStorage,
  mirrorSnapshotToLocalStorage,
  normalizeRestoreMode,
  parseSnapshot,
  readSnapshotFromLocalStorage,
} from '@/utils/tabSession'

const FONT_SIZE_KEY = 'fastclaw-font-size'

function applyFontSize(value: string | undefined) {
  if (typeof document === 'undefined') return
  const px = Math.min(Math.max(parseInt(value || '') || 14, 10), 27)
  document.documentElement.style.setProperty('--font-size-base', `${px}px`)
  document.documentElement.style.fontSize = `${px}px`
}

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<Record<string, string>>({
    language: 'zh-CN',
    font_size: '14',
    retry_count: '5',
    theme: 'light',
    [TAB_SESSION_RESTORE_KEY]: DEFAULT_TAB_SESSION_RESTORE,
  })

  let settingsLoaded = false
  let settingsLoadPromise: Promise<void> | null = null

  async function performLoadSettings() {
    try {
      const rows: any[] = await invoke('get_settings', {})
      for (const r of rows) {
        let val = r.value
        if (typeof val === 'string' && val.startsWith('"') && val.endsWith('"')) {
          val = val.slice(1, -1)
        }
        settings.value[r.key] = val
      }
    } catch (err) {
      console.warn('Tauri invoke not available in browser environment', err)
    }
    // 数据库无数据时使用默认值
    if (!settings.value['theme']) settings.value['theme'] = 'light'
    if (!settings.value['language']) settings.value['language'] = 'zh-CN'
    if (!settings.value['font_size']) settings.value['font_size'] = '14'
    if (settings.value['retry_count'] === undefined || settings.value['retry_count'] === '') {
      settings.value['retry_count'] = '5'
    }
    if (!settings.value[TAB_SESSION_RESTORE_KEY]) {
      settings.value[TAB_SESSION_RESTORE_KEY] = DEFAULT_TAB_SESSION_RESTORE
    }
    localStorage.setItem(FONT_SIZE_KEY, settings.value['font_size'])
    applyFontSize(settings.value['font_size'])
    mirrorRestoreModeToLocalStorage(normalizeRestoreMode(settings.value[TAB_SESSION_RESTORE_KEY]))

    // 会话快照：DB 优先，否则用本地镜像/旧版 tabs
    const fromDb = parseSnapshot(settings.value[TAB_SESSION_SNAPSHOT_KEY])
    const fromLs = readSnapshotFromLocalStorage()
    const best =
      fromDb && fromLs
        ? (fromDb.savedAt >= fromLs.savedAt ? fromDb : fromLs)
        : (fromDb || fromLs)
    if (best) {
      settings.value[TAB_SESSION_SNAPSHOT_KEY] = JSON.stringify(best)
      mirrorSnapshotToLocalStorage(best)
    }
  }

  async function loadSettings(force = false) {
    if (!force && settingsLoaded) return
    if (settingsLoadPromise) return settingsLoadPromise
    settingsLoadPromise = performLoadSettings()
      .then(() => { settingsLoaded = true })
      .finally(() => { settingsLoadPromise = null })
    return settingsLoadPromise
  }

  async function updateSetting(key: string, value: string) {
    try {
      await invoke('update_setting', { key, value })
    } catch (err) {
      console.warn('Tauri invoke not available in browser environment', err)
    }
    settings.value[key] = value
    if (key === 'font_size') {
      localStorage.setItem(FONT_SIZE_KEY, value)
      // 通知所有 iframe 字体大小变更
      document.querySelectorAll('iframe').forEach(iframe => {
        try { iframe.contentWindow?.postMessage({ type: 'FONT_SIZE_CHANGE', fontSize: value + 'px' }, '*') } catch (_) {}
      })
    }
    if (key === 'language') {
      localStorage.setItem('fastclaw-language', value)
      i18n.global.locale.value = value as any
      try {
        const { useMenuStore } = await import('@/stores/menu')
        useMenuStore().syncLocale(String(value || ''))
      } catch { /* ignore */ }
      // 通知所有 iframe 切换语言
      document.querySelectorAll('iframe').forEach(iframe => {
        try { iframe.contentWindow?.postMessage({ type: 'LANGUAGE_CHANGE', lang: value }, '*') } catch (_) {}
      })
    }
    if (key === TAB_SESSION_RESTORE_KEY) {
      mirrorRestoreModeToLocalStorage(normalizeRestoreMode(value))
    }
    if (key === TAB_SESSION_SNAPSHOT_KEY) {
      const snap = parseSnapshot(value)
      if (snap) mirrorSnapshotToLocalStorage(snap)
    }
  }

  function getTabSessionRestoreMode(): TabSessionRestoreMode {
    return normalizeRestoreMode(settings.value[TAB_SESSION_RESTORE_KEY])
  }

  /** 持久化标签会话（与基础参数同库 settings 表） */
  async function saveTabSessionSnapshot(snapshot: TabSessionSnapshot) {
    const value = JSON.stringify(snapshot)
    settings.value[TAB_SESSION_SNAPSHOT_KEY] = value
    mirrorSnapshotToLocalStorage(snapshot)
    try {
      await invoke('update_setting', { key: TAB_SESSION_SNAPSHOT_KEY, value })
    } catch (err) {
      console.warn('saveTabSessionSnapshot failed', err)
    }
  }

  function getTabSessionSnapshot(): TabSessionSnapshot | null {
    return parseSnapshot(settings.value[TAB_SESSION_SNAPSHOT_KEY]) || readSnapshotFromLocalStorage()
  }

  watch(
    () => settings.value['font_size'],
    (val) => applyFontSize(val),
  )

  return {
    settings,
    loadSettings,
    updateSetting,
    getTabSessionRestoreMode,
    saveTabSessionSnapshot,
    getTabSessionSnapshot,
  }
})
