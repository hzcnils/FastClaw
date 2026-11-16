<script setup lang="ts">
import { onMounted, ref, watch, computed } from 'vue'
import { useSettingsStore } from '@/stores/settings'
import { useThemeStore, type SimpleThemeId } from '@/stores/theme'
import { useAppConfigStore } from '@/stores/appConfig'

// 引入进阶功能组件
import DataSettingsView from '@/views/DataSettingsView.vue'
import DefaultModelView from '@/views/DefaultModelView.vue'
import AboutusView from '@/views/AboutusView.vue'
import WebSearchSettings from '@/views/settings/WebSearchSettings.vue'
import { useWebSearchStore } from '@/stores/websearch'
import MarketView from '@/views/MarketView.vue';
import PersonalAssistantPanel from '@/views/assistant/PersonalAssistantPanel.vue'
import NetworkServiceSettings from '@/views/settings/NetworkServiceSettings.vue'
import BusinessConfigSettings from '@/views/settings/BusinessConfigSettings.vue'
import SvgIcon from '@/components/svgIcon/index.vue'
import {
  loadBranding,
  appName,
  appLogo,
  chatModelIcon,
  setAppName,
  setAppLogo,
  setChatModelIcon,
  resetAppLogo,
  resetChatModelIcon,
  fileToDataUrl,
  DEFAULT_APP_NAME,
} from '@/api/branding'
import logoImg from '@/assets/logo.webp'

const emit = defineEmits<{
  (e: 'close'): void
}>()

const settings = useSettingsStore()
const themeStore = useThemeStore()
const appConfigStore = useAppConfigStore()
const activeMenu = ref('settings')
const ws = useWebSearchStore()
watch(() => ws.settingsTab, (v) => { if (v) { activeMenu.value = v; ws.settingsTab = null } }, { immediate: true })

// Provide default values so the UI doesn't break if loadSettings fails (e.g. in browser)
onMounted(async () => {
  settings.settings = {
    theme: 'light',
    language: 'zh-CN',
    font_size: '14',
    tab_session_restore: 'unfinished',
    temperature: '0.7',
    top_p: '1.0',
    max_tokens: '2000',
    chunk_strategy: 'semantic',
    embedding_model: 'text-embedding-3-small',
    system_tray: 'false',
    ...settings.settings
  }
  await settings.loadSettings()
  themeStore.initTheme()
  loadBranding()   // 加载自定义系统名称/图标
})

const update = (key: string, value: string) => settings.updateSetting(key, value)


function onFontSizeChange(e: Event) {
  const input = e.target as HTMLInputElement
  const val = Number(input.value)
  if (!isNaN(val) && val >= 11 && val <= 24) {
    update('font_size', String(val))
  } else {
    // 恢复为上一个合法值
    input.value = String(settings.settings.font_size || 14)
  }
}

function incrementFontSize() {
  const current = Number(settings.settings.font_size || 14)
  if (current < 24) update('font_size', String(current + 1))
}

function decrementFontSize() {
  const current = Number(settings.settings.font_size || 14)
  if (current > 11) update('font_size', String(current - 1))
}

function parseRetryCount(raw: string | undefined): number {
  const n = Number(raw)
  if (!Number.isInteger(n) || n < 0) return 5
  return n
}

function onRetryCountChange(e: Event) {
  const input = e.target as HTMLInputElement
  const val = Number(input.value)
  // 仅允许非负整数（含 0）
  if (Number.isInteger(val) && val >= 0) {
    update('retry_count', String(val))
  } else {
    input.value = String(parseRetryCount(settings.settings.retry_count))
  }
}

function incrementRetryCount() {
  const current = parseRetryCount(settings.settings.retry_count)
  update('retry_count', String(current + 1))
}

function decrementRetryCount() {
  const current = parseRetryCount(settings.settings.retry_count)
  if (current > 0) update('retry_count', String(current - 1))
}

// 托盘开关：同时写 DB 并实时切换运行时托盘状态
async function onSystemTrayChange(enabled: boolean) {
  settings.updateSetting('system_tray', enabled ? '1' : '0')
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('set_system_tray_enabled', { enabled })
  } catch (e) {
    console.warn('[Tray] set_system_tray_enabled failed:', e)
  }
}

// ─── 系统品牌（名称 + 图标） ────────────────────────────────

function onAppNameChange(e: Event) {
  setAppName((e.target as HTMLInputElement).value.trim())
}

const logoInputRef = ref<HTMLInputElement | null>(null)
function pickLogo() {
  logoInputRef.value?.click()
}
async function onLogoFileChange(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0]
  if (!file || !file.type.startsWith('image/')) return
  try {
    const dataUrl = await fileToDataUrl(file)
    await setAppLogo(dataUrl)
  } catch { /* ignore */ }
  finally {
    if (logoInputRef.value) logoInputRef.value.value = ''
  }
}
function onResetLogo() {
  resetAppLogo()
}

const chatIconInputRef = ref<HTMLInputElement | null>(null)
function pickChatIcon() {
  chatIconInputRef.value?.click()
}
async function onChatIconFileChange(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0]
  if (!file || !file.type.startsWith('image/')) return
  try {
    const dataUrl = await fileToDataUrl(file)
    await setChatModelIcon(dataUrl)
  } catch { /* ignore */ }
  finally {
    if (chatIconInputRef.value) chatIconInputRef.value.value = ''
  }
}
function onResetChatIcon() {
  resetChatModelIcon()
}

// ─── 设置页导航项（配置驱动） ───
const ALL_NAV_ITEMS = [
  { key: 'settings',        labelKey: 'settings.parameterSettings', icon: 'nav-settings'  },
  { key: 'defaultModel',    labelKey: 'settings.defaultModel',       icon: 'nav-model'     },
  { key: 'networkService', labelKey: 'settings.networkService',     icon: 'nav-network'   },
  { key: 'data',            labelKey: 'settings.dataSettings',       icon: 'nav-data'      },
  { key: 'personalAssistant',   labelKey: 'settings.personalAssistant', icon: 'nav-assistant' },
  { key: 'pluginMarket',    labelKey: 'settings.pluginMarket',       icon: 'nav-plugin'    },
  { key: 'webSearch',       labelKey: 'chat.webSearch',              icon: 'nav-websearch' },
  { key: 'businessConfig', labelKey: 'settings.businessConfigTitle',  icon: 'nav-settings' },
  { key: 'aboutus',         labelKey: 'settings.aboutus',            icon: 'nav-about'     },
]

const navItems = computed(() =>
  ALL_NAV_ITEMS.filter(item => appConfigStore.isSettingVisible(item.key))
)

// 确保 activeMenu 始终指向一个可见的设置页（兜底安全）
watch(navItems, (items) => {
  if (items.length > 0 && !items.some(item => item.key === activeMenu.value)) {
    activeMenu.value = items[0].key
  }
}, { immediate: true })

const currentThemeValue = ref('light')
const currentLanguageValue = ref('zh-CN')

watch(() => settings.settings.language, (lang) => {
  currentLanguageValue.value = lang || 'zh-CN'
}, { immediate: true })

watch(() => themeStore.currentPreset.id, (themeId) => {
  if (themeId === 'simple-light') {
    currentThemeValue.value = 'light'
    settings.settings.theme = 'light'
  } else if (themeId === 'simple-dark') {
    currentThemeValue.value = 'dark'
    settings.settings.theme = 'dark'
  } else {
    currentThemeValue.value = 'system'
    settings.settings.theme = 'system'
  }
}, { immediate: true })

function handleThemeChange(value: string) {
  currentThemeValue.value = value
  update('theme', value)

  if (value === 'light') {
    themeStore.beginDraft()
    themeStore.selectTheme('simple-light' as SimpleThemeId)
    themeStore.confirmDraft()
  } else if (value === 'dark') {
    themeStore.beginDraft()
    themeStore.selectTheme('simple-dark' as SimpleThemeId)
    themeStore.confirmDraft()
  }
}

</script>
<template>
  <Teleport to="body">
    <Transition name="theme-modal">
      <div class="settings-overlay">
        <div class="settings-modal">
          <!-- 关闭按钮 -->
          <button class="btn-close" @click="emit('close')" title="关闭设置">
            <svg viewBox="0 0 24 24" width="1.25em" height="1.25em" stroke="currentColor" stroke-width="2" fill="none">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>

          <!-- 设置侧边栏 -->
          <div class="settings-sidebar">
            <div class="sidebar-header">
              <h2>{{ $t('settings.title') }}</h2>
            </div>

            <div class="sidebar-nav">
              <div v-for="item in navItems" :key="item.key" class="nav-item" :class="{ active: activeMenu === item.key }"
                @click="activeMenu = item.key">
                <SvgIcon :icon-name="item.icon" />
                {{ $t(item.labelKey) }}
              </div>
            </div>
          </div>

          <!-- 设置主内容 -->
          <div class="settings-main-container">
            <div v-if="activeMenu === 'settings'" class="settings-content">
              <div class="settings-card">
                <div class="card-header">
                  <span class="card-title">{{ $t('settings.parameterSettings') }}</span>
                </div>
                <div class="settings-groups">
                  <section class="group">
                    <!-- <h3>{{ $t('settings.general') }}</h3> -->
                    <!-- <div class="setting-row">
                      <label>{{ $t('settings.theme') }}</label>
                      <el-select :model-value="currentThemeValue" @change="handleThemeChange" style="flex: 1;">
                        <el-option value="light" :label="$t('settings.light')" />
                        <el-option value="dark" :label="$t('settings.dark')" />
                        <el-option value="system" :label="$t('settings.followSystem')" />
                      </el-select>
                    </div> -->
                    <div class="setting-row">
                      <label>{{ $t('settings.language') }}</label>
                      <el-select v-model="currentLanguageValue" @change="(val: string) => { update('language', val) }"
                        style="flex: 1;">
                        <el-option value="zh-CN" label="简体中文" />
                        <el-option value="en" label="English" />
                      </el-select>
                    </div>
                    <div class="setting-row">
                      <label>{{ $t('settings.sendShortcut') }}</label>
                      <el-select :model-value="settings.settings.send_shortcut || 'enter'"
                        @change="(v: string) => update('send_shortcut', v)" style="flex: 1;">
                        <el-option value="enter" :label="$t('settings.sendByEnter')" />
                        <el-option value="alt_s" :label="$t('settings.sendByAltS')" />
                      </el-select>
                    </div>
                    <div class="setting-row">
                      <label>{{ $t('settings.fontSize') }}</label>
                      <div class="number-input-wrap">
                        <input type="number" :value="settings.settings.font_size || 14" min="11" max="24"
                          @change="onFontSizeChange" />
                        <div class="number-arrows">
                          <button class="number-arrow number-arrow-up" type="button" @click="incrementFontSize">
                            <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                              <polyline points="18 15 12 9 6 15"></polyline>
                            </svg>
                          </button>
                          <button class="number-arrow number-arrow-down" type="button" @click="decrementFontSize">
                            <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                              <polyline points="6 9 12 15 18 9"></polyline>
                            </svg>
                          </button>
                        </div>
                      </div>
                    </div>
                    <div class="setting-row">
                      <label>{{ $t('settings.retryCount') }}</label>
                      <div class="number-input-wrap">
                        <input type="number" :value="settings.settings.retry_count ?? 5" min="0" step="1"
                          @change="onRetryCountChange" />
                        <div class="number-arrows">
                          <button class="number-arrow number-arrow-up" type="button" @click="incrementRetryCount">
                            <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                              <polyline points="18 15 12 9 6 15"></polyline>
                            </svg>
                          </button>
                          <button class="number-arrow number-arrow-down" type="button" @click="decrementRetryCount">
                            <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                              <polyline points="6 9 12 15 18 9"></polyline>
                            </svg>
                          </button>
                        </div>
                      </div>
                    </div>
                    <div class="setting-row">
                      <label>{{ $t('settings.tabSessionRestore') }}</label>
                      <el-select
                        :model-value="settings.settings.tab_session_restore || 'unfinished'"
                        @change="(v: string) => update('tab_session_restore', v)"
                        style="flex: 1;"
                      >
                        <el-option value="unfinished" :label="$t('settings.tabSessionRestoreUnfinished')" />
                        <el-option value="all" :label="$t('settings.tabSessionRestoreAll')" />
                      </el-select>
                    </div>
                  </section>

                  <section class="group">
                    <!-- <h3>{{ $t('settings.networkEnv') }}</h3> -->
                    <div class="setting-row">
                      <label>{{ $t('settings.systemTray') }}</label>
                      <label class="toggle-switch small">
                        <input type="checkbox" :checked="settings.settings.system_tray === '1'"
                          @change="onSystemTrayChange(($event.target).checked)">
                        <span class="slider"></span>
                      </label>
                    </div>
                  </section>
                   <!-- 系统信息 -->
                  <!-- <section class="group">
                    <h3>{{ $t('settings.systemInfo') }}</h3>
                    <div class="setting-row">
                      <label>{{ $t('settings.systemName') }}</label>
                      <input type="text" :value="appName" :placeholder="DEFAULT_APP_NAME"
                        @change="onAppNameChange" style="flex: 1;" />
                    </div>
                    <div class="setting-row">
                      <label>{{ $t('settings.systemLogo') }}</label>
                      <div style="flex: 1; display: flex; align-items: center; gap: 12px;">
                        <img :src="appLogo || logoImg" alt="logo"
                          style="width: 36px; height: 36px; border-radius: 8px; object-fit: contain; background: var(--bg-secondary);" />
                        <input ref="logoInputRef" type="file" accept="image/*" style="display: none"
                          @change="onLogoFileChange" />
                        <button class="setting-btn" @click="pickLogo">{{ $t('settings.changeIcon') }}</button>
                        <button v-if="appLogo" class="setting-btn setting-btn--text" @click="onResetLogo">{{ $t('settings.restoreDefault') }}</button>
                      </div>
                    </div>
                    <div class="setting-row">
                      <label>{{ $t('settings.chatModelIcon') }}</label>
                      <div style="flex: 1; display: flex; align-items: center; gap: 12px;">
                        <img :src="chatModelIcon || logoImg" alt="model-icon"
                          style="width: 36px; height: 36px; border-radius: 8px; object-fit: contain; background: var(--bg-secondary);" />
                        <input ref="chatIconInputRef" type="file" accept="image/*" style="display: none"
                          @change="onChatIconFileChange" />
                        <button class="setting-btn" @click="pickChatIcon">{{ $t('settings.changeIcon') }}</button>
                        <button v-if="chatModelIcon" class="setting-btn setting-btn--text" @click="onResetChatIcon">{{ $t('settings.restoreDefault') }}</button>
                      </div>
                    </div>
                  </section> -->
                </div>
              </div>
            </div>

            <!-- 设置子页 -->
            <DefaultModelView v-else-if="activeMenu === 'defaultModel'" class="embedded-view" />
            <DataSettingsView v-else-if="activeMenu === 'data'" class="embedded-view" />
            <NetworkServiceSettings v-else-if="activeMenu === 'networkService'" class="embedded-view" />
            <WebSearchSettings v-else-if="activeMenu === 'webSearch'" class="embedded-view" />
            <AboutusView v-else-if="activeMenu === 'aboutus'" class="embedded-view" />
            <MarketView v-else-if="activeMenu === 'pluginMarket'" class="embedded-view" />
            <PersonalAssistantPanel v-else-if="activeMenu === 'personalAssistant'" class="embedded-view" />
            <BusinessConfigSettings v-else-if="activeMenu === 'businessConfig'" class="embedded-view" />
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
<style lang="scss" scoped>
/* 主题风格过渡动画 */
.theme-modal-enter-active,
.theme-modal-leave-active {
  transition: opacity 0.2s ease;
}

.theme-modal-enter-active .settings-modal,
.theme-modal-leave-active .settings-modal {
  transition: transform 0.2s ease, opacity 0.2s ease;
}

.theme-modal-enter-from,
.theme-modal-leave-to {
  opacity: 0;
}

.theme-modal-enter-from .settings-modal,
.theme-modal-leave-to .settings-modal {
  opacity: 0;
  transform: translateY(-8px) scale(0.98);
}

.settings-overlay {
  position: fixed;
  inset: 0;
  z-index: 2000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: rgba(15, 23, 42, 0.15);
  backdrop-filter: blur(0px);
  overflow: hidden;
}

.settings-modal {
  width: 90vw;
  max-width: 1100px;
  height: 85vh;
  max-height: 800px;
  display: flex;
  overflow: hidden;
  position: relative;
  color: var(--text-primary);
  background: linear-gradient(135deg, rgba(255, 255, 255, 0.06), transparent), var(--theme-dailog-color);
  border: 1px solid var(--border-color);
  border-radius: 18px;
  box-shadow: 0 24px 70px rgba(15, 23, 42, 0.24);
}

.btn-close {
  position: absolute;
  top: 16px;
  right: 16px;
  width: 32px;
  height: 32px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--text-secondary);
  background: transparent;
  border: none;
  border-radius: 8px;
  cursor: pointer;
  z-index: 100;
  transition: all 0.18s ease;

  &:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }
}

.settings-sidebar {
  width: 240px;
  // background: var(--bg-sidebar, #f9f9f9);
  border-right: 1px solid var(--chat-divider-color) !important;
  padding: 24px 16px 0 16px;
  display: flex;
  flex-direction: column;
  flex-shrink: 0;

  .sidebar-nav {
    flex: 1;
    overflow-y: auto;
    padding-bottom: 24px;
    padding-right: 4px;

    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-thumb, rgba(0, 0, 0, 0.15)) transparent;

    &::-webkit-scrollbar { width: 6px; }
    &::-webkit-scrollbar-track { background: transparent; }
    &::-webkit-scrollbar-thumb {
      background: var(--scrollbar-thumb, rgba(0, 0, 0, 0.15));
      border-radius: 3px;
    }
    &::-webkit-scrollbar-thumb:hover {
      background: var(--scrollbar-thumb-hover, rgba(0, 0, 0, 0.25));
    }
  }

  .sidebar-header {
    margin-bottom: 24px;
    padding: 0 8px;

    h2 {
      font-size: var(--font-size-xl);
      font-weight: 600;
      margin: 0;
      color: var(--text-primary);
    }
  }

  .nav-group {
    font-size: var(--font-size-sm);
    font-weight: 600;
    color: var(--text-secondary);
    margin: 8px 8px 4px 8px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: 8px;
    font-size: var(--font-size-base);
    color: var(--text-primary);
    cursor: pointer;
    transition: all 0.2s;
    margin-bottom: 2px;

    svg {
      color: var(--text-secondary);
    }

    &:hover {
      background: var(--bg-hover, rgba(0, 0, 0, 0.05));
    }

    &.active {
      background: var(--bg-active, #e6f7ff);
      color: var(--color-primary, #1677ff);
      font-weight: 500;

      svg {
        color: var(--color-primary, #1677ff);
      }
    }
  }
}

.settings-main-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  // background: var(--bg-primary);
  position: relative;
  padding: 32px 48px 20px 28px;
}

.settings-content {
  flex: 1;
  overflow-y: auto;

  &::-webkit-scrollbar {
    width: 6px;
  }

  &::-webkit-scrollbar-thumb {
    background: var(--border-color);
    border-radius: 8px;
  }
}

.embedded-view {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;

  &::-webkit-scrollbar {
    width: 6px;
  }

  &::-webkit-scrollbar-thumb {
    background: var(--border-color);
    border-radius: 8px;
  }
}

.settings-card {
  border: 1px solid var(--border-color);
  border-radius: 12px;
  overflow: hidden;
}

.settings-card .card-header {
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-color);

  .card-title {
    font-size: var(--font-size-base);
    font-weight: 600;
    color: var(--text-primary);
  }
}

.settings-groups {
  max-width: 100%;
  padding: 0 20px;
}

.group {
  margin: 10px 0 20px;

  h3 {
    font-size: var(--font-size-base);
    margin-bottom: 16px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border-color);
    color: var(--text-primary);
  }
}

.setting-row {
  display: flex;
  align-items: center;
  margin-bottom: 16px;
  gap: 50px;

  label {
    width: 140px;
    flex-shrink: 0;
    font-size: var(--font-size-base);
    color: var(--text-secondary);
    line-height: 1.35;
    word-break: break-all;
  }

  .btn-change-env {
    margin-left: 8px;
    padding: 6px 14px;
    border: 1px solid var(--border-color);
    border-radius: 6px;
    background: var(--bg-primary);
    color: var(--text-secondary);
    cursor: pointer;
    font-size: var(--font-size-md);
    transition: all 0.15s;

    &:hover {
      border-color: var(--color-primary);
      color: var(--color-primary);
    }
  }

  input,
  select {
    flex: 1;

    &:focus {
      border-color: var(--border-color);
      outline: none;
    }
  }

  :deep(.el-select) {
    flex: 1;
  }

  :deep(.el-select .el-select__wrapper) {
    height: 38px !important;
    min-height: unset !important;
    box-sizing: border-box;
    background: transparent !important;
    border: 1px solid var(--border-color) !important;
    box-shadow: none !important;
  }

  :deep(.el-select .el-select__placeholder) {
    color: var(--text-secondary);
  }

  input,
  select {
    height: 38px;
    box-sizing: border-box;
    padding: 8px 12px;
    border: 1px solid var(--border-color);
    border-radius: 6px;
    font-size: var(--font-size-base);
    background: transparent;
    color: var(--text-primary);
    transition: all 0.2s;
  }

  .number-input-wrap {
    flex: 1;
    position: relative;
    display: flex;
    align-items: center;

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

.toggle-switch {
  position: relative;
  display: inline-block;
  width: 36px;
  height: 20px;
  margin-left: auto;
  flex-shrink: 0;

  input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .slider {
    position: absolute;
    cursor: pointer;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: #ccc;
    transition: 0.3s;
    border-radius: 20px;

    &:before {
      position: absolute;
      content: "";
      height: 14px;
      width: 14px;
      left: 3px;
      bottom: 3px;
      background-color: white;
      transition: 0.3s;
      border-radius: 50%;
    }
  }

  input:checked+.slider {
    background-color: var(--color-primary);
  }

  input:checked+.slider:before {
    transform: translateX(16px);
  }

  &.small {
    width: 36px;
    height: 20px;

    .slider {
      border-radius: 20px;

      &:before {
        height: 14px;
        width: 14px;
      }
    }

    input:checked+.slider:before {
      transform: translateX(16px);
    }
  }
}

.setting-btn {
  height: 34px;
  padding: 0 14px;
  border-radius: 8px;
  font-size: var(--font-size-base);
  cursor: pointer;
  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);
  border: 1px solid var(--btn-primary-border, transparent);
  border-radius: 4px;

  &:hover {
    background: var(--btn-primary-hover-bg, var(--color-primary));
    color: var(--btn-primary-hover-text, #fff);
    border-color: var(--btn-primary-hover-border, transparent);

  }

  &--text {
    border-color: transparent;
    background: transparent;
    color: var(--text-secondary);
  }
}


</style>

<style>
html.dark select option {
  background: #000 !important;
  color: #fff !important;
}

html.light select option {
  background: var(--color-primary) !important;
  color: #fff !important;
}


</style>
