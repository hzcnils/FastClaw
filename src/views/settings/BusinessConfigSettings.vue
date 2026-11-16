<script setup lang="ts">
import { onMounted, ref, reactive, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppConfigStore } from '@/stores/appConfig'
import { ALL_MENU_IDS, ALL_SETTING_KEYS, DEFAULT_FRONTEND_CONFIG } from '@/types/app-config'
import { useToast } from '@/composables/useToast'

const store = useAppConfigStore()
const toast = useToast()
const { t } = useI18n()

// 菜单标签映射（支持中英文切换）
const menuLabelMap = computed<Record<string, string>>(() => ({
  chat: t('sidebar.smartChat'),
  kb: t('sidebar.knowledge'),
  models: t('sidebar.modelManage'),
  alltool: t('sidebar.allTools'),
  skills: t('sidebar.skillsManage'),
  mcp: t('sidebar.mcpService'),
  wsp: t('sidebar.wspManage'),
  dashboard: t('sidebar.callDashboard'),
}))

// 设置页标签映射（支持中英文切换）
const settingLabelMap = computed<Record<string, string>>(() => ({
  settings: t('settings.parameterSettings'),
  defaultModel: t('settings.defaultModel'),
  networkService: t('settings.networkService'),
  data: t('settings.dataSettings'),
  personalAssistant: t('settings.personalAssistant'),
  pluginMarket: t('settings.pluginMarket'),
  webSearch: t('settings.webSearch'),
  aboutus: t('settings.aboutus'),
  businessConfig: t('settings.businessConfigTitle'),
}))

const menuToggles = reactive<Record<string, boolean>>({})
const settingToggles = reactive<Record<string, boolean>>({})
const saving = ref(false)

onMounted(async () => {
  if (!store.loaded) await store.loadConfig()
  for (const id of ALL_MENU_IDS) menuToggles[id] = store.isMenuVisible(id)
  for (const key of ALL_SETTING_KEYS) settingToggles[key] = store.isSettingVisible(key)
})

async function handleSave() {
  saving.value = true
  try {
    const menus: Record<string, boolean> = {}
    for (const id of ALL_MENU_IDS) {
      if (menuToggles[id] !== DEFAULT_FRONTEND_CONFIG.menus[id]) menus[id] = menuToggles[id]
    }
    const settings: Record<string, boolean> = {}
    for (const key of ALL_SETTING_KEYS) {
      if (settingToggles[key] !== DEFAULT_FRONTEND_CONFIG.settings[key]) settings[key] = settingToggles[key]
    }
    await store.saveBusinessConfig(menus, settings)
    toast.success(t('settings.businessConfig.saveSuccess'))
  } catch (e) {
    console.error('[BusinessConfig] 保存失败:', e)
    toast.error(t('settings.businessConfig.saveFailedRetry'))
  } finally {
    saving.value = false
  }
}

function toggleAll(toggles: Record<string, boolean>, keys: readonly string[], val: boolean) {
  for (const k of keys) toggles[k] = val
}
</script>

<template>
  <div class="biz-config">
    <!-- Card 1: Menu Visibility -->
    <div class="cfg-card">
      <div class="card-head">
        <div class="card-title">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="9" y1="3" x2="9" y2="21"/></svg>
          <span>{{ $t('settings.businessConfig.menuVisibility') }}</span>
        </div>
        <div class="card-actions">
          <button class="link" @click="toggleAll(menuToggles, ALL_MENU_IDS, true)">{{ $t('settings.businessConfig.selectAll') }}</button>
          <button class="link" @click="toggleAll(menuToggles, ALL_MENU_IDS, false)">{{ $t('settings.businessConfig.deselectAll') }}</button>
        </div>
      </div>
      <p class="card-desc">{{ $t('settings.businessConfig.menuVisibilityDesc') }}</p>
      <div class="chip-row">
        <label v-for="id in ALL_MENU_IDS" :key="id" class="chip" :class="{ off: !menuToggles[id] }" @click="menuToggles[id] = !menuToggles[id]">
          <span class="chip-dot" :class="{ on: menuToggles[id] }" />
          {{ menuLabelMap[id] || id }}
        </label>
      </div>
    </div>

    <!-- Card 2: Settings Page Visibility -->
    <div class="cfg-card">
      <div class="card-head">
        <div class="card-title">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
          <span>{{ $t('settings.businessConfig.settingsVisibility') }}</span>
        </div>
        <div class="card-actions">
          <button class="link" @click="toggleAll(settingToggles, ALL_SETTING_KEYS, true)">{{ $t('settings.businessConfig.selectAll') }}</button>
          <button class="link" @click="toggleAll(settingToggles, ALL_SETTING_KEYS, false)">{{ $t('settings.businessConfig.deselectAll') }}</button>
        </div>
      </div>
      <p class="card-desc">{{ $t('settings.businessConfig.settingsVisibilityDesc') }}</p>
      <div class="chip-row">
        <label v-for="key in ALL_SETTING_KEYS" :key="key" class="chip" :class="{ off: !settingToggles[key] }" @click="settingToggles[key] = !settingToggles[key]">
          <span class="chip-dot" :class="{ on: settingToggles[key] }" />
          {{ settingLabelMap[key] || key }}
        </label>
      </div>
    </div>

    <!-- Save -->
    <div class="save-bar">
      <el-button class="save-bar-btn" type="primary" size="small" :loading="saving" @click="handleSave">{{ $t('settings.saveConfig') }}</el-button>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.biz-config {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding-top: 2px;
}

// ─── Card ──────────────────────────────────────────────────

.cfg-card {
  border: 1px solid var(--border-color, #e4e7ed);
  border-radius: 10px;
  padding: 14px 16px 12px;
  // background: var(--bg-primary, #fff);
}

.card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 6px;
}

.card-title {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: var(--font-size-base);
  font-weight: 600;
  color: var(--text-primary);
}

.card-actions {
  display: flex;
  gap: 10px;

  .link {
    background: none;
    border: none;
    font-size: var(--font-size-sm);
    color: var(--color-primary, #409eff);
    cursor: pointer;
    padding: 0;
    &:hover { text-decoration: underline; }
  }
}

.card-desc {
  margin: 0 0 10px 23px;
  font-size: var(--font-size-sm);
  color: var(--text-secondary, #909399);
  line-height: 1.5;

  code {
    font-size: var(--font-size-sm);
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--bg-hover, rgba(0,0,0,0.04));
  }
}

// ─── Chips ─────────────────────────────────────────────────

.chip-row {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 4px 10px;
  border-radius: 14px;
  font-size: var(--font-size-sm);
  color: var(--text-primary);
  background: var(--bg-hover, rgba(0,0,0,0.03));
  border: 1px solid transparent;
  cursor: pointer;
  user-select: none;
  transition: all 0.15s;

  &:hover {
    border-color: var(--color-primary, rgba(64,158,255,0.25));
  }

  &.off {
    color: var(--text-placeholder, #c0c4cc);
    background: transparent;
    border-color: var(--border-color, #e4e7ed);
    &:hover {
      color: var(--text-secondary, #909399);
      border-color: var(--text-secondary, #c0c4cc);
    }
  }
}

.chip-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #c0c4cc;
  flex-shrink: 0;
  transition: background 0.2s;

  &.on {
    background: var(--color-primary, #409eff);
    box-shadow: 0 0 3px rgba(64,158,255,0.35);
  }
}

// ─── Save bar ──────────────────────────────────────────────

.save-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding-top: 2px;
}
.save-bar-btn
{

  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 16px 16px;
  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);
  border: 1px solid var(--btn-primary-border, transparent);
  border-radius: 4px;
  cursor: pointer;
  font-size: var(--font-size-md);
  font-weight: 500;
  transition: all 0.2s;

  &:hover {
    background: var(--btn-primary-hover-bg, var(--color-primary));
    color: var(--btn-primary-hover-text, #fff);
    border-color: var(--btn-primary-hover-border, transparent);
  }
}
</style>
