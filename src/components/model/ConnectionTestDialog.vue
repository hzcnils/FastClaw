<script setup lang="ts">
import { ref, computed, watch, nextTick, onBeforeUnmount } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useI18n } from 'vue-i18n'
import type { ConnTestLogEntry } from '@/types/index'
import { testConnection, updateProvider } from '@/modules/models/api'
import type { TestConnectionResult } from '@/modules/models/types/contracts'

const { t } = useI18n()

// ─── Props & Emits ───
const props = defineProps<{
  visible: boolean
  providerId: string
  providerCode: string
  baseUrl: string
  apiKey: string
}>()

const emit = defineEmits<{
  'update:visible': [value: boolean]
  saved: [apiKey: string]
}>()

// ─── Reactive state ───
const testing = ref(false)
const logs = ref<ConnTestLogEntry[]>([])
const testResult = ref<{ success: boolean; latency_ms: number; message: string } | null>(null)
const showFullKey = ref(false)
const logContainerRef = ref<HTMLElement | null>(null)
const testPath = ref('/models')

let unlistenFns: UnlistenFn[] = []

// ─── Computed ───
const hasEmptyUrl = computed(() => !props.baseUrl?.trim())
const canTest = computed(() => !hasEmptyUrl.value && !testing.value)

const fullTestUrl = computed(() => {
  const base = (props.baseUrl || '').replace(/\/+$/, '')
  const path = testPath.value || '/models'
  if (!base) return ''
  return base + (path.startsWith('/') ? path : '/' + path)
})

const startButtonText = computed(() => {
  if (testing.value) return t('modelManage.testDialogTesting')
  if (testResult.value) return t('modelManage.testDialogRetest')
  return t('modelManage.testDialogStartTest')
})

const displayKey = computed(() => {
  if (!props.apiKey) return ''
  if (showFullKey.value) return props.apiKey
  return getMaskedKey(props.apiKey)
})

// ─── Functions ───
function getMaskedKey(key: string): string {
  if (!key) return ''
  if (key.length <= 8) return '***'
  return key.substring(0, 4) + '****' + key.substring(key.length - 4)
}

function toggleShowKey() {
  showFullKey.value = !showFullKey.value
}

function scrollToBottom() {
  nextTick(() => {
    if (logContainerRef.value) {
      logContainerRef.value.scrollTop = logContainerRef.value.scrollHeight
    }
  })
}

function cleanupListeners() {
  unlistenFns.forEach(fn => fn())
  unlistenFns = []
}

async function startTest() {
  testing.value = true
  logs.value = []
  testResult.value = null

  // 先注册事件监听器（防止竞态条件）
  try {
    const unlistenLog = await listen<ConnTestLogEntry>('conn-test-log', (event) => {
      logs.value.push(event.payload)
      scrollToBottom()
    })
    unlistenFns.push(unlistenLog)
  } catch (e) {
    console.warn('Failed to listen conn-test-log:', e)
  }

  try {
    const r: TestConnectionResult = await testConnection({
      providerCode: props.providerCode,
      testApiUrl: fullTestUrl.value,
      testApiKey: props.apiKey,
    })

    testResult.value = {
      success: r.success,
      latency_ms: r.latency_ms,
      message: r.message,
    }

    if (r.success && props.apiKey) {
      // 自动保存 API 密钥
      try {
        await updateProvider(props.providerId, { api_key_ref: props.apiKey })
      } catch (saveErr) {
        console.warn('Auto-save API key failed:', saveErr)
      }
      emit('saved', props.apiKey)
    }
  } catch (e: any) {
    // Tauri invoke 本身抛出异常（非预期情况）
    logs.value.push({
      level: 'error',
      message: `[错误] 调用失败: ${e.message || String(e)}`,
      timestamp_ms: Date.now(),
    })
    testResult.value = {
      success: false,
      latency_ms: 0,
      message: e.message || String(e),
    }
  } finally {
    testing.value = false
    cleanupListeners()
  }
}

function handleClose() {
  cleanupListeners()
  resetState()
  emit('update:visible', false)
}

function resetState() {
  testResult.value = null
  logs.value = []
  showFullKey.value = false
  testPath.value = '/models'
}

// 监听弹窗打开 → 重置状态，URL 有效时自动开始测试
let autoTestTimer: ReturnType<typeof setTimeout> | null = null

watch(
  () => props.visible,
  (val) => {
    if (val) {
      resetState()
      // URL 有效时，弹窗渲染后延时自动开始测试
      if (!hasEmptyUrl.value) {
        autoTestTimer = setTimeout(() => {
          autoTestTimer = null
          startTest()
        }, 400)
      }
    } else {
      if (autoTestTimer) {
        clearTimeout(autoTestTimer)
        autoTestTimer = null
      }
    }
  }
)

onBeforeUnmount(() => {
  if (autoTestTimer) {
    clearTimeout(autoTestTimer)
    autoTestTimer = null
  }
  cleanupListeners()
})
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="overlay">
      <div class="dialog test-connection-dialog">
        <!-- 头部 -->
        <div class="dialog-header">
          <h3>{{ t('modelManage.testDialogTitle') }}</h3>
          <button class="close-btn" @click="handleClose">
            <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>

        <!-- 信息展示区 -->
        <div class="test-info-section">
          <div class="info-row">
            <span class="info-label">{{ t('modelManage.testDialogUrlLabel') }}</span>
            <span class="info-value" :class="{ 'empty-warning': hasEmptyUrl }">
              {{ props.baseUrl || t('modelManage.testDialogUrlEmpty') }}
            </span>
          </div>
          <div class="info-row">
            <span class="info-label">{{ t('modelManage.testDialogKeyLabel') }}</span>
            <div class="info-value-with-toggle">
              <span class="info-value" :class="{ 'empty-warning': !props.apiKey }">
                {{ displayKey || t('modelManage.testDialogKeyEmpty') }}
              </span>
              <button v-if="props.apiKey" class="btn-toggle-key" type="button" @click="toggleShowKey"
                :title="showFullKey ? t('modelManage.testDialogHideKey') : t('modelManage.testDialogShowKey')">
                <svg v-if="showFullKey" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor"
                  stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
                  <path
                    d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24">
                  </path>
                  <line x1="1" y1="1" x2="23" y2="23"></line>
                </svg>
                <svg v-else viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2"
                  fill="none" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M1 12 C1 12 5 4 12 4 C19 4 23 12 23 12 C23 12 19 20 12 20 C5 20 1 12 1 12 Z"></path>
                  <circle cx="12" cy="12" r="3"></circle>
                </svg>
              </button>
            </div>
          </div>
          <div class="info-row">
            <span class="info-label">{{ t('modelManage.testDialogPathLabel') }}</span>
            <input class="test-path-input" v-model="testPath" :disabled="testing" placeholder="/models" />
          </div>
        </div>

        <!-- 警告横幅：URL 为空时显示 -->
        <div v-if="hasEmptyUrl && !testing" class="warning-banner">
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="#faad14" stroke-width="2">
            <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path>
            <line x1="12" y1="9" x2="12" y2="13"></line>
            <line x1="12" y1="17" x2="12.01" y2="17"></line>
          </svg>
          <span>{{ t('modelManage.testDialogIncompleteInfo') }}</span>
        </div>

        <!-- 日志输出区域 -->
        <div class="test-log-area" ref="logContainerRef">
          <div v-if="logs.length === 0 && !testing" class="log-placeholder">
            {{ t('modelManage.testDialogStartTest') }}
          </div>
          <div v-for="(log, idx) in logs" :key="idx" class="log-entry" :class="`log-${log.level}`">
            <span class="log-message">{{ log.message }}</span>
          </div>
          <div v-if="testing && !testResult" class="log-loading">
            <span class="spinner"></span>
          </div>
        </div>

        <!-- 结果横幅 -->
        <div v-if="testResult" class="result-banner" :class="testResult.success ? 'result-success' : 'result-fail'">
          <svg v-if="testResult.success" viewBox="0 0 24 24" width="18" height="18" stroke="#52c41a" stroke-width="2"
            fill="none">
            <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
            <polyline points="22 4 12 14.01 9 11.01"></polyline>
          </svg>
          <svg v-else viewBox="0 0 24 24" width="18" height="18" stroke="#f5222d" stroke-width="2" fill="none">
            <circle cx="12" cy="12" r="10"></circle>
            <line x1="15" y1="9" x2="9" y2="15"></line>
            <line x1="9" y1="9" x2="15" y2="15"></line>
          </svg>
          <span>{{ testResult.success ? t('modelManage.testDialogSuccess') : testResult.message }}</span>
        </div>

        <!-- 操作按钮 -->
        <div class="actions">
          <button class="btn-cancel" @click="handleClose">
            {{ t('modelManage.testDialogClose') }}
          </button>
          <button class="btn-start" @click="startTest" :disabled="!canTest">
            <svg v-if="testing" class="spinner-icon" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor"
              stroke-width="2" fill="none">
              <circle cx="12" cy="12" r="10" stroke-opacity="0.25"></circle>
              <path d="M12 2a10 10 0 0 1 10 10" stroke-linecap="round"></path>
            </svg>
            {{ startButtonText }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style lang="scss" scoped>
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 200;
}

.dialog {
  background: var(--theme-dailog-color);
  padding: 24px;
  border-radius: 18px;
  width: 520px;
  max-height: 85vh;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--border-color);
  box-shadow: 0 24px 70px rgba(15, 23, 42, 0.24);

  .dialog-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
    flex-shrink: 0;

    h3 {
      font-size: var(--font-size-lg);
      font-weight: 600;
      margin: 0;
      color: var(--text-primary);
    }

    .close-btn {
      background: none;
      border: none;
      cursor: pointer;
      color: var(--text-secondary);
      padding: 4px;
      border-radius: 4px;

      &:hover {
        color: var(--text-primary);
        background: var(--bg-hover);
      }

      &:disabled {
        opacity: 0.4;
        cursor: not-allowed;
      }
    }
  }
}

// ─── 信息展示区 ───
.test-info-section {
  flex-shrink: 0;
  margin-bottom: 12px;

  .info-row {
    display: flex;
    align-items: flex-start;
    margin-bottom: 8px;
    font-size: var(--font-size-base);

    .info-label {
      flex-shrink: 0;
      width: 70px;
      color: var(--text-secondary);
      line-height: 28px;
    }

    .info-value {
      flex: 1;
      color: var(--text-primary);
      word-break: break-all;
      font-family: 'SF Mono', Monaco, 'Cascadia Code', monospace;
      font-size: 12px;
      line-height: 28px;

      &.empty-warning {
        color: #f5222d;
      }
    }

    .test-path-input {
      height: 34px;
      flex: 1;
      padding: 5px 10px;
      border: 1px solid var(--border-color);
      border-radius: 6px;
      background: transparent;
      color: var(--text-primary);
      font-family: 'SF Mono', Monaco, 'Cascadia Code', monospace;
      font-size: 12px;
      outline: none;

      // &:focus {
      //   border-color: var(--color-primary);
      // }

      &:disabled {
        opacity: 0.6;
      }
    }

    .info-value-with-toggle {
      flex: 1;
      display: flex;
      align-items: center;
      gap: 8px;

      .info-value {
        flex: unset;
      }
    }

    .btn-toggle-key {
      background: none;
      border: none;
      cursor: pointer;
      color: var(--text-secondary);
      padding: 2px;
      border-radius: 4px;
      display: flex;
      align-items: center;

      &:hover {
        color: var(--text-primary);
        background: var(--bg-hover);
      }
    }
  }
}

// ─── 警告横幅 ───
.warning-banner {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
  margin-bottom: 12px;
  background: rgba(250, 173, 20, 0.08);
  border-left: 3px solid #faad14;
  border-radius: 6px;
  font-size: var(--font-size-base);
  color: #f5222d;
  flex-shrink: 0;
}

// ─── 日志区域 ───
.test-log-area {
  flex: 1;
  min-height: 120px;
  max-height: 280px;
  overflow-y: auto;
  background: var(--bg-hover);
  border-radius: 8px;
  padding: 12px;
  margin-bottom: 12px;
  font-family: 'SF Mono', Monaco, 'Cascadia Code', monospace;
  font-size: 12px;
  line-height: 1.7;

  .log-placeholder {
    color: var(--text-secondary);
    text-align: center;
    padding: 20px 0;
  }

  .log-entry {
    padding: 1px 0;

    &.log-info .log-message {
      color: var(--text-secondary);
    }

    &.log-success .log-message {
      color: #52c41a;
    }

    &.log-error .log-message {
      color: #f5222d;
    }

    &.log-warning .log-message {
      color: #faad14;
    }
  }

  .log-loading {
    display: flex;
    justify-content: center;
    padding: 8px;

    .spinner {
      width: 14px;
      height: 14px;
      border: 2px solid var(--border-color);
      border-top-color: var(--color-primary);
      border-radius: 50%;
      animation: spin 0.6s linear infinite;
    }
  }
}

// ─── 结果横幅 ───
.result-banner {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 14px;
  margin-bottom: 12px;
  border-radius: 8px;
  font-size: var(--font-size-base);
  flex-shrink: 0;

  &.result-success {
    background: rgba(82, 196, 26, 0.08);
    border: 1px solid rgba(82, 196, 26, 0.25);
    color: #c2cebb;
  }

  &.result-fail {
    background: rgba(245, 34, 45, 0.08);
    border: 1px solid rgba(245, 34, 45, 0.25);
    color: #f5222d;
  }
}

// ─── 操作按钮 ───
.actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  flex-shrink: 0;
}

.btn-cancel {
  padding: 8px 20px;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  background: var(--bg-primary);
  color: var(--text-primary);
  font-size: var(--font-size-base);
  cursor: pointer;

  &:hover:not(:disabled) {
    background: var(--bg-hover);
  }

  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
}

.btn-start {
  padding: 8px 20px;
  border: none;
  border-radius: 4px;
  background: var(--color-primary);
  color: #fff;
  font-size: var(--font-size-base);
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 6px;

  &:hover:not(:disabled) {
    opacity: 0.9;
  }

  &:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .spinner-icon {
    animation: spin 0.8s linear infinite;
  }
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
