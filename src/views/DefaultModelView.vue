<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import ModelSelector from '@/components/chat/ModelSelector.vue'
import { listProviderSummaries } from '@/modules/models/api'

// 默认助手模型配置
const assistantModel = ref<string | undefined>()
let defaultModelResizeObserver: ResizeObserver | null = null

function updateDefaultModelTriggerWidth() {
  const wrappers = document.querySelectorAll('.default-model-select')
  let width = 0
  wrappers.forEach((el) => {
    width = Math.max(width, el.getBoundingClientRect().width)
  })
  document.documentElement.style.setProperty('--default-model-trigger-width', `${width}px`)
}

/** 下拉高度限制在设置弹窗内：按触发器上下剩余空间取可用高度 */
function constrainDefaultModelDropdown(visible: boolean) {
  if (!visible) return

  const apply = () => {
    const modal = document.querySelector('.settings-modal') as HTMLElement | null
    const poppers = Array.from(document.querySelectorAll('.default-model-popper')) as HTMLElement[]
    // 取当前可见的下拉（display 非 none / 未 aria-hidden）
    const popper =
      poppers.find((el) => el.style.display !== 'none' && el.getAttribute('aria-hidden') !== 'true')
      || poppers[poppers.length - 1]
    if (!modal || !popper) return

    const triggerEl =
      (document.querySelector('.default-model-select .el-select__wrapper[aria-expanded="true"]') as HTMLElement | null)
      || (document.querySelector('.default-model-select .el-select__wrapper.is-focused') as HTMLElement | null)
    if (!triggerEl) return

    const modalRect = modal.getBoundingClientRect()
    const triggerRect = triggerEl.getBoundingClientRect()
    const gap = 8
    const headerH = 54
    const spaceBelow = Math.floor(modalRect.bottom - triggerRect.bottom - gap)
    const spaceAbove = Math.floor(triggerRect.top - modalRect.top - gap)
    const usablePanel = Math.max(120, Math.min(Math.max(spaceBelow, spaceAbove), 300))
    const wrapMax = Math.max(80, usablePanel - headerH)

    popper.style.maxHeight = `${usablePanel}px`
    popper.style.overflow = 'hidden'

    const dropdown = popper.querySelector('.el-select-dropdown') as HTMLElement | null
    if (dropdown) {
      dropdown.style.maxHeight = `${usablePanel}px`
      dropdown.style.overflow = 'hidden'
    }

    popper.querySelectorAll<HTMLElement>(
      '.el-select-dropdown__wrap, .el-scrollbar__wrap, .el-scrollbar',
    ).forEach((el) => {
      el.style.maxHeight = `${wrapMax}px`
    })
  }

  nextTick(() => {
    requestAnimationFrame(() => {
      apply()
      // Element Plus 定位/翻转后再压一次，避免被内联高度覆盖
      setTimeout(apply, 50)
    })
  })
}

// 默认翻译模型配置
const translationModel = ref<string | undefined>()

// 话题命名模型配置
const namingModel = ref<string | undefined>()

const settingsLoaded = ref(false)

/** 与智能对话一致：供应商 baseUrl/apiKey 完整性 */
const BUILTIN_PROVIDER_CODES = new Set(['HUIZHI', 'PRIVATE', 'LOCAL'])
const providerConfigMap = ref<Record<string, { id: string; baseUrl: string; hasApiKey: boolean }>>({})

function isVendorIncomplete(providerCode?: string): boolean {
  if (!providerCode || BUILTIN_PROVIDER_CODES.has(providerCode)) return false
  const cfg = providerConfigMap.value[providerCode]
  if (!cfg) return false
  return !cfg.baseUrl || !cfg.hasApiKey
}

async function loadProviderConfigs() {
  try {
    const providers = await listProviderSummaries()
    const map: Record<string, { id: string; baseUrl: string; hasApiKey: boolean }> = {}
    for (const p of providers) {
      const code = p.provider_code
      if (!code) continue
      map[code] = {
        id: p.id || '',
        baseUrl: p.base_url || '',
        hasApiKey: p.has_api_key,
      }
    }
    providerConfigMap.value = map
  } catch (e) {
    console.warn('list_provider_summaries failed', e)
  }
}

const updateModel = async (key: string, value: string) => {
  try {
    await invoke('update_setting_with_category', { key, value, category: 'defaultmodel' })
  } catch (e) {
    console.error('Failed to update model:', e)
  }
}

watch(assistantModel, (val: string | undefined) => {
  if (val && settingsLoaded.value) {
    updateModel('default_assistant_model', val)
  }
})

watch(translationModel, (val: string | undefined) => {
  if (val && settingsLoaded.value) {
    updateModel('default_translation_model', val)
  }
})

watch(namingModel, (val: string | undefined) => {
  if (val && settingsLoaded.value) {
    updateModel('topic_naming_model', val)
  }
})

onMounted(async () => {
  await loadProviderConfigs()
  try {
    const rows: any[] = await invoke('get_settings', { category: 'defaultmodel' })
    const found = { assistant: false, translation: false, naming: false }
    for (const r of rows) {
      if (r.key === 'default_assistant_model') {
        assistantModel.value = r.value
        found.assistant = true
      } else if (r.key === 'default_translation_model') {
        translationModel.value = r.value
        found.translation = true
      } else if (r.key === 'topic_naming_model') {
        namingModel.value = r.value
        found.naming = true
      }
    }
    // 未配置的项重置为 undefined，避免 ModelSelector 自动选中的值被误认为已配置
    if (!found.assistant) assistantModel.value = undefined
    if (!found.translation) translationModel.value = undefined
    if (!found.naming) namingModel.value = undefined
  } catch (e) {
    console.error('Failed to load settings:', e)
  }
  setTimeout(() => {
    settingsLoaded.value = true
  }, 50);

  updateDefaultModelTriggerWidth()
  defaultModelResizeObserver = new ResizeObserver(updateDefaultModelTriggerWidth)
  document.querySelectorAll('.default-model-select').forEach((el) => {
    defaultModelResizeObserver.observe(el)
  })
})

onUnmounted(() => {
  defaultModelResizeObserver?.disconnect()
  defaultModelResizeObserver = null
})
</script>
<template>
  <div class="model-view-container">
    <div class="model-content">
      <!-- 默认助手模型卡片 -->
      <div class="model-card">
        <div class="card-header">
          <div class="header-left">
            <span class="header-icon">
             <svg xmlns="http://www.w3.org/2000/svg" width="20" height="19.427" viewBox="0 0 20 19.427">
             <path fill="var(--text-primary)" id="路径_5212" data-name="路径 5212" d="M11.632,1.217a1.678,1.678,0,0,0-3.207-.065,1.577,1.577,0,0,0,.962,1.861c.127.064.385.192.385.32V4.807h.513V3.462a.55.55,0,0,1,.384-.45,1.475,1.475,0,0,0,.963-1.795Zm5.157,6.577A2.955,2.955,0,0,0,13.833,4.84H6.242A2.955,2.955,0,0,0,3.286,7.794v4.872A2.955,2.955,0,0,0,6.242,15.62h7.591a2.955,2.955,0,0,0,2.956-2.954ZM12.362,9.026a.671.671,0,0,1-.352-.705.808.808,0,0,1,.879-.588q.47.059.47.823V8.9a.19.19,0,0,1-.178.193,1.9,1.9,0,0,1-.818-.07ZM7.4,12.609a.586.586,0,0,1-.536-.826L8.344,8.489a.782.782,0,0,1,.713-.462h.482a.782.782,0,0,1,.753.572l.993,3.574a.391.391,0,0,1-.377.495h-.368a.391.391,0,0,1-.374-.278L9.328,9.621a.1.1,0,0,0-.183-.014l-.88,1.864a.1.1,0,0,0,.088.139h.753a.2.2,0,0,1,.172.287l-.157.3a.782.782,0,0,1-.69.416H7.4Zm10.415-.417A2.147,2.147,0,0,0,20,10.078a2.087,2.087,0,0,0-2.181-1.99Zm-15.638,0v-4.1A2.151,2.151,0,0,0,0,10.2a2.175,2.175,0,0,0,2.181,1.988Zm10.225.535a.586.586,0,0,1-.585-.63l.148-2a.586.586,0,0,1,.585-.543h.055a.586.586,0,0,1,.584.63l-.148,2a.586.586,0,0,1-.584.543h-.055ZM5.415,17.688h9.17a.769.769,0,0,1,.87.87.769.769,0,0,1-.87.87H5.415a.769.769,0,0,1-.87-.87.769.769,0,0,1,.87-.87Z"/>
             </svg>
            </span>
            <span class="card-title">{{ $t('defaultModel.assistantModel') }}</span>
          </div>
        </div>
        <div class="card-body">
          <div class="model-select-row">
            <div class="assistant-model-select default-model-select">
              <ModelSelector
                v-model="assistantModel"
                popper-class="main-chat-select-popper default-model-popper"
                icon-tags
                show-incomplete-filter
                :is-vendor-incomplete="isVendorIncomplete"
                @visible-change="constrainDefaultModelDropdown"
              />
            </div>
          </div>
          <div class="model-desc">{{ $t('defaultModel.assistantModelDesc') }}</div>
        </div>
      </div>

      <!-- 话题命名模型卡片 -->
      <div class="model-card">
        <div class="card-header">
          <div class="header-left">
            <span class="header-icon">
              <svg xmlns="http://www.w3.org/2000/svg" width="20" height="19.546" viewBox="0 0 20 19.546">
              <path fill="var(--text-primary)" id="联合_23" data-name="联合 23" d="M0,9.092A7.273,7.273,0,0,1,7.274,1.819H10.91V3.638H7.274A5.454,5.454,0,0,0,1.819,9.092c0,3.282,2.238,5.423,7.273,7.708V14.547H10.91a5.454,5.454,0,0,0,5.454-5.455h1.819a7.273,7.273,0,0,1-7.272,7.272v3.182C6.365,17.727,0,15,0,9.092Zm12.027.019a1,1,0,1,1,1,1A1,1,0,0,1,12.027,9.111Zm-4,0a1,1,0,1,1,1,1A1,1,0,0,1,8.026,9.111Zm-4,0a1,1,0,1,1,1,1A1,1,0,0,1,4.027,9.111ZM15.94,6.994l-.224-.514A3.972,3.972,0,0,0,13.7,4.433l-.69-.307a.483.483,0,0,1,0-.876l.652-.29A3.97,3.97,0,0,0,15.707.846l.23-.554a.46.46,0,0,1,.856,0l.23.554a3.968,3.968,0,0,0,2.046,2.115l.652.29a.483.483,0,0,1,0,.876l-.69.307A3.972,3.972,0,0,0,17.014,6.48l-.224.514a.459.459,0,0,1-.85,0Z"/>
              </svg>
            </span>
            <span class="card-title">{{ $t('defaultModel.namingModel') }}</span>
          </div>
        </div>
        <div class="card-body">
          <div class="model-select-row">
            <div class="assistant-model-select default-model-select">
              <ModelSelector
                v-model="namingModel"
                popper-class="main-chat-select-popper default-model-popper"
                icon-tags
                show-incomplete-filter
                :is-vendor-incomplete="isVendorIncomplete"
                @visible-change="constrainDefaultModelDropdown"
              />
            </div>
          </div>
          <div class="model-desc">{{ $t('defaultModel.namingModelDesc') }}</div>
        </div>
      </div>

      <!-- 默认翻译模型卡片 -->
      <div class="model-card">
        <div class="card-header">
          <div class="header-left">
            <span class="header-icon">
              <svg xmlns="http://www.w3.org/2000/svg" width="20" height="21.371" viewBox="0 0 20 21.371">
              <path fill="var(--text-primary)" id="联合_21" data-name="联合 21" d="M7.531,21.371a.822.822,0,0,1-.811-.689l-.011-.133V12.241H.962a.685.685,0,0,1-.685-.685V.685A.685.685,0,0,1,.962,0H11.833a.684.684,0,0,1,.685.685v8.5h5.988a.822.822,0,0,1,.81.689l.011.133V20.549a.822.822,0,0,1-.689.811l-.133.011Zm.821-1.643h9.331v-8.9H12.518v.732a.685.685,0,0,1-.685.685H8.353Zm-6.43-9.133h8.953V1.643H1.922Zm2.306,8.793a4.6,4.6,0,0,1-2.939-1.51A4.548,4.548,0,0,1,.06,15.33l-.027-.313L0,14.333l1.367-.067.034.685a3.134,3.134,0,0,0,.9,2,3.231,3.231,0,0,0,1.816,1.033l.271.041.681.079-.157,1.36Zm6.156-1.021,1.971-5.447h1.384l1.913,5.445H14.474l-.411-1.148H11.914L11.5,18.367Zm1.825-2.061h1.56l-.735-2.326H13ZM5.841,9.377V6.983H4.477v.5H3.531V4.227H5.813V2.863l.947,0c.13.018.195.036.195.055A.2.2,0,0,1,6.926,3a1.5,1.5,0,0,0-.083.529v.7H9.265V7.4H8.235V6.983H6.843V9.377Zm1-3.146H8.235V5.006H6.843Zm-2.367,0H5.841V5.006H4.477Zm14.155.213L18.6,5.758a3.137,3.137,0,0,0-.9-2,3.23,3.23,0,0,0-1.818-1.033l-.27-.041-.681-.078.157-1.361.681.079a4.6,4.6,0,0,1,2.939,1.508A4.561,4.561,0,0,1,19.94,5.379l.027.312L20,6.377l-1.368.068Z"/>
             </svg>
            </span>
            <span class="card-title">{{ $t('defaultModel.translationModel') }}</span>
          </div>
        </div>
        <div class="card-body">
          <div class="model-select-row">
            <div class="assistant-model-select default-model-select">
              <ModelSelector
                v-model="translationModel"
                popper-class="main-chat-select-popper default-model-popper"
                icon-tags
                show-incomplete-filter
                :is-vendor-incomplete="isVendorIncomplete"
                @visible-change="constrainDefaultModelDropdown"
              />
            </div>
          </div>
          <div class="model-desc">{{ $t('defaultModel.translationModelDesc') }}</div>
        </div>
      </div>


    </div>
  </div>
</template>
<style lang="scss" scoped>
.model-view-container {
  display: flex;
  height: 100%;
  // background: var(--bg-primary);
  overflow: hidden;
}

.model-content {
  flex: 1;
  overflow-y: auto;
}

.model-card {
  border: 1px solid var(--border-color);
  border-radius: 8px;
  margin-bottom: 15px;
}

.card-header {
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-color);

  .header-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .header-icon {
    font-size: var(--font-size-lg);
  }

  .card-title {
    font-size: var(--font-size-base) ;
    font-weight: 600;
    color: var(--text-primary);
  }
}

.card-body {
  padding: 20px;
}

.model-select-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.assistant-model-select {
  flex: 1;
}

.model-select {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  font-size: var(--font-size-base);
  color: var(--text-primary);
  background: var(--bg-primary);
  cursor: pointer;

  &:hover {
    background: #f5f5f5;
  }

  &:focus {
    outline: none;
    border-color: var(--color-primary);
  }

  option {
    font-size: var(--font-size-base);
    font-weight: normal;
  }

  optgroup {
    font-size: var(--font-size-sm);
    font-weight: normal !important;
  }
}

.btn-settings {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 16px;
  background: transparent;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  font-size: var(--font-size-md);
  color: var(--text-primary);
  cursor: pointer;

  &:hover {
    background: var(--bg-hover);
  }
}

.model-desc {
  font-size: var(--font-size-md);
  color: var(--text-secondary);
  line-height: 1.5;
}
</style>

<style lang="scss">
/* 设置页默认助手模型触发器尺寸 */
.assistant-model-select {
  .model-selector {
    width: 100% !important;
  }

  .el-select__wrapper {
    padding: 7px 12px;
    height: 40px !important;
    border: 1px solid var(--border-color);
    border-radius: 6px;
    background: transparent !important;
    font-size: var(--font-size-base);
    box-shadow: none !important;
    display: flex;
    align-items: center;
  }

  .el-select__prefix,
  .el-select__selection,
  .el-select__selected-item {
    display: flex;
    align-items: center;
  }

  .el-select__selected-item {
    padding-left: 0 !important;
    line-height: 1.4;
  }

  .el-select__selection {
    gap: 8px;
  }

  .model-selector-prefix {
    display: inline-flex;
    height: 24px;
    align-items: center;
    justify-content: center;
    line-height: 1;
  }

  .model-selector-prefix .el-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    line-height: 1;
    transform: translateY(1px);
  }

  .model-selector-prefix img,
  .model-selector-prefix svg {
    display: block;
  }
}

/* 情景主题（除雾灰调）：触发器已选文字白色 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .assistant-model-select .el-select__placeholder,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .assistant-model-select .el-select__selected-item {
  color: #fff !important;
}

/*
 * 下拉视觉复用智能对话 main-chat-select-popper；
 * 宽度锁定；高度由 JS 按设置弹窗剩余空间写入 inline，以下为兜底上限。
 */
.default-model-popper {
  width: var(--default-model-trigger-width, 100%) !important;
  min-width: var(--default-model-trigger-width, 100%) !important;
  max-width: var(--default-model-trigger-width, 100%) !important;
  z-index: 2100 !important;
  max-height: 280px !important;
  overflow: hidden !important;
}

.default-model-popper .el-select-dropdown {
  max-height: inherit !important;
  overflow: hidden !important;
  display: flex !important;
  flex-direction: column !important;
}

.default-model-popper .el-select-dropdown__header {
  flex-shrink: 0;
}

.default-model-popper .el-scrollbar,
.default-model-popper .el-select-dropdown__wrap,
.default-model-popper .el-scrollbar__wrap,
.default-model-popper.main-chat-select-popper .el-select-dropdown__wrap,
.default-model-popper.main-chat-select-popper .el-scrollbar__wrap {
  max-height: 220px !important;
}
</style>
