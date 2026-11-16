<script setup lang="ts">
import { onMounted, ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  useAppConfigStore,
  SERVICE_LABELS,
  DEFAULTS,
  REMOTE_SERVER_KEYS,
} from '@/stores/appConfig'
const store = useAppConfigStore()
const { t } = useI18n()

// 本地服务名称列表
const serviceNames = Object.keys(SERVICE_LABELS)

// 本地服务标签映射（支持中英文切换）
const serviceLabelMap = computed<Record<string, string>>(() => ({
  fastplan: t('settings.fastplanService'),
  fastkg: t('settings.fastkgService'),
  shimmy: t('settings.shimmyService'),
  pinchtab: t('settings.pinchtabService'),
  // workflow: t('settings.workflowService'),
  qdrant: t('settings.qdrantService'),
  llama_server: t('settings.llamaEmbedService'),
}))

// 远程服务器标签映射（支持中英文切换）
const remoteServerLabelMap = computed<Record<string, string>>(() => ({
  download_api_base_url: t('settings.downloadApiBaseUrl'),
  download_api_base_url_private: t('settings.downloadApiBaseUrlPrivate'),
  model_sync_url: t('settings.modelSyncUrl'),
  fastci_base_url: t('settings.fastciBaseUrl'),
  fastci_model_download_url: t('settings.fastciModelDownloadUrl'),
  independent_model_data: t('settings.independentModelData'),
  independent_model_api_url: t('settings.independentModelApiUrl'),
}))

// 本地编辑状态（延迟提交，用户修改后即时保存）
const editingUrls = ref<Record<string, string>>({})
const saving = ref<Record<string, boolean>>({})

// 远程服务器编辑状态
const editingRemoteUrls = ref<Record<string, string>>({})
const savingRemote = ref<Record<string, boolean>>({})
// 远程地址都显示。独立模型开关单独渲染，其地址只在打开开关后出现。默认值均为空。
const urlKeys = REMOTE_SERVER_KEYS.filter(
  k => k !== 'independent_model_data' && k !== 'independent_model_api_url'
)

onMounted(async () => {
  if (!store.loaded) {
    await store.loadConfig()
  }
  // 初始化本地服务编辑值
  for (const name of serviceNames) {
    editingUrls.value[name] = store.serviceUrl(name)
  }
  // 初始化远程服务器编辑值
  for (const key of REMOTE_SERVER_KEYS) {
    editingRemoteUrls.value[key] = store.remoteServerUrl(key)
  }
})

// ─── 本地服务操作 ──────────────────────────────────────────

/** 修改服务 URL（即时保存到配置文件和运行时） */
async function onUrlChange(name: string, newUrl: string) {
  saving.value[name] = true
  try {
    await store.updateField(`services.${name}`, JSON.stringify(newUrl))
    editingUrls.value[name] = newUrl
  } catch (e) {
    // 恢复旧值
    editingUrls.value[name] = store.serviceUrl(name)
  } finally {
    saving.value[name] = false
  }
}

/** 重置为默认值 */
async function onReset(name: string) {
  saving.value[name] = true
  try {
    await store.resetServiceUrl(name)
    editingUrls.value[name] = DEFAULTS.services[name]
  } catch (e) {
    console.error(`重置 ${name} 失败:`, e)
  } finally {
    saving.value[name] = false
  }
}

/** 全部恢复默认 */
async function onResetAll() {
  for (const name of serviceNames) {
    saving.value[name] = true
    try {
      await store.resetServiceUrl(name)
      editingUrls.value[name] = DEFAULTS.services[name]
    } catch (e) {
      console.error(`重置 ${name} 失败:`, e)
    } finally {
      saving.value[name] = false
    }
  }
}

// ─── 远程服务器操作 ────────────────────────────────────────

/** 修改远程服务器 URL */
async function onRemoteUrlChange(key: string, newUrl: string) {
  savingRemote.value[key] = true
  try {
    await store.updateField(`remote_servers.${key}`, JSON.stringify(newUrl))
    editingRemoteUrls.value[key] = newUrl
  } catch (e) {
    editingRemoteUrls.value[key] = store.remoteServerUrl(key)
  } finally {
    savingRemote.value[key] = false
  }
}

/** 重置单个远程服务器 URL */
async function onRemoteReset(key: string) {
  savingRemote.value[key] = true
  try {
    await store.resetRemoteServerUrl(key)
    editingRemoteUrls.value[key] = getRemoteDefault(key)
  } catch (e) {
    console.error(`重置远程服务器 ${key} 失败:`, e)
  } finally {
    savingRemote.value[key] = false
  }
}

/** 全部远程服务器恢复默认 */
async function onRemoteResetAll() {
  const rs: Record<string, string> = DEFAULTS.remote_servers as unknown as Record<string, string>
  for (const key of REMOTE_SERVER_KEYS) {
    savingRemote.value[key] = true
    try {
      await store.resetRemoteServerUrl(key)
      editingRemoteUrls.value[key] = rs[key] ?? ''
    } catch (e) {
      console.error(`重置远程服务器 ${key} 失败:`, e)
    } finally {
      savingRemote.value[key] = false
    }
  }
}

/** 获取 input 事件的值（避免模板中使用 as 断言） */
function getInputVal(event: Event): string {
  return (event.target as HTMLInputElement).value
}

/** 获取远程服务器默认值 */
function getRemoteDefault(key: string): string {
  const rs: Record<string, string> = DEFAULTS.remote_servers as unknown as Record<string, string>
  return rs[key] ?? ''
}

/** 修改 toggle 类型配置（布尔值） */
async function onRemoteToggleChange(key: string, val: boolean) {
  savingRemote.value[key] = true
  try {
    await store.updateField(`remote_servers.${key}`, JSON.stringify(val))
    editingRemoteUrls.value[key] = String(val)
  } catch (e) {
    editingRemoteUrls.value[key] = String(store.remoteServerUrl(key) === 'true')
  } finally {
    savingRemote.value[key] = false
  }
}
</script>

<template>
  <div class="network-service-settings">
    <div class="settings-card">
      <div class="card-header">
        <span class="card-title">{{ $t('settings.networkService') }}</span>
        <span class="card-subtitle">{{ $t('settings.networkServiceDesc') }}</span>
      </div>

      <div class="settings-groups">
        <!-- Local Services -->
        <section class="group">
          <h3>{{ $t('settings.localServices') }}</h3>
          <div class="service-list">
            <div v-for="name in serviceNames" :key="name" class="service-row">
              <label class="service-label">{{ serviceLabelMap[name] || name }}</label>
              <div class="service-input-wrap">
                <input
                  type="text"
                  class="service-input"
                  :value="editingUrls[name]"
                  @change="onUrlChange(name, getInputVal($event))"
                  :disabled="saving[name]"
                  :placeholder="DEFAULTS.services[name]"
                />
                <button
                  class="btn-reset"
                  @click="onReset(name)"
                  :disabled="saving[name]"
                  :title="$t('settings.resetToDefault')"
                >
                  <span v-if="saving[name]" class="spinner"></span>
                  <span v-else>↺</span>
                </button>
              </div>
            </div>
          </div>
        </section>

        <!-- Remote Servers -->
        <section class="group">
          <h3>{{ $t('settings.remoteServerAddress') }}</h3>
          <div class="hint-tip">
            {{ $t('settings.remoteServerAddressHint') }}
          </div>

          <!-- Independent Model Data Switch -->
          <div class="service-row">
            <label class="service-label">{{ remoteServerLabelMap['independent_model_data'] }}</label>
            <div class="service-input-wrap">
              <el-switch
                :model-value="editingRemoteUrls['independent_model_data'] === 'true'"
                @update:model-value="onRemoteToggleChange('independent_model_data', $event)"
                :disabled="savingRemote['independent_model_data']"
              />
            </div>
          </div>

          <!-- Independent Model API URL (shown when switch is on) -->
          <div v-if="editingRemoteUrls['independent_model_data'] === 'true'" class="service-row">
            <label class="service-label">{{ remoteServerLabelMap['independent_model_api_url'] }}</label>
            <div class="service-input-wrap">
              <input
                type="text"
                class="service-input"
                :value="editingRemoteUrls['independent_model_api_url']"
                @change="onRemoteUrlChange('independent_model_api_url', getInputVal($event))"
                :disabled="savingRemote['independent_model_api_url']"
                :placeholder="getRemoteDefault('independent_model_api_url')"
              />
              <button
                class="btn-reset"
                @click="onRemoteReset('independent_model_api_url')"
                :disabled="savingRemote['independent_model_api_url']"
                :title="$t('settings.resetToDefault')"
              >
                <span v-if="savingRemote['independent_model_api_url']" class="spinner"></span>
                <span v-else>↺</span>
              </button>
            </div>
          </div>

          <div class="service-list">
            <div v-for="key in urlKeys" :key="key" class="service-row">
              <label class="service-label">{{ remoteServerLabelMap[key] || key }}</label>
              <div class="service-input-wrap">
                <input
                  type="text"
                  class="service-input"
                  :value="editingRemoteUrls[key]"
                  @change="onRemoteUrlChange(key, getInputVal($event))"
                  :disabled="savingRemote[key]"
                  :placeholder="getRemoteDefault(key)"
                />
                <button
                  class="btn-reset"
                  @click="onRemoteReset(key)"
                  :disabled="savingRemote[key]"
                  :title="$t('settings.resetToDefault')"
                >
                  <span v-if="savingRemote[key]" class="spinner"></span>
                  <span v-else>↺</span>
                </button>
              </div>
            </div>
          </div>
        </section>

        <!-- Bottom Actions -->
        <section class="group actions">
          <button class="btn-reset-all" @click="onRemoteResetAll">
            {{ $t('settings.resetRemoteAddressToDefault') }}
          </button>
          <button class="btn-reset-all" @click="onResetAll">
            {{ $t('settings.resetAllToDefault') }}
          </button>
        </section>
      </div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.network-service-settings {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.settings-card {
  flex: 1;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--border-color, rgba(255, 255, 255, 0.08));
  border-radius: 14px;
  overflow: hidden;
  min-height: 0;
}

.card-header {
  flex-shrink: 0;
  padding: 20px 24px 12px;
  border-bottom: 1px solid var(--border-color, rgba(255, 255, 255, 0.06));

  .card-title {
    font-size: var(--font-size-lg);
    font-weight: 600;
    color: var(--text-primary);
    display: block;
  }

  .card-subtitle {
    font-size: var(--font-size-sm);
    color: var(--text-tertiary);
    margin-top: 4px;
    display: block;
  }
}

.settings-groups {
  flex: 1;
  overflow-y: auto;
  padding: 8px 24px 20px;
}

.group {
  margin-bottom: 8px;

  h3 {
    font-size: var(--font-size-md);
    font-weight: 600;
    color: var(--text-secondary);
    margin: 16px 0 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  &.actions {
    margin-top: 16px;
    padding-top: 16px;
    border-top: 1px solid var(--border-color, rgba(255, 255, 255, 0.06));
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }
}

.hint-tip {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  padding: 8px 12px;
  margin-bottom: 8px;
  font-size: var(--font-size-sm);
  color: var(--text-tertiary);
  background: var(--bg-tertiary, rgba(255, 255, 255, 0.03));
  border: 1px solid var(--border-color, rgba(255, 255, 255, 0.06));
  border-radius: 8px;
  line-height: 1.5;

  .hint-icon {
    flex-shrink: 0;
    font-size: var(--font-size-base);
  }

  strong {
    color: var(--text-secondary);
  }
}

.service-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.service-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-radius: 8px;
  transition: background 0.15s;

  &:hover {
    background: var(--bg-hover, rgba(255, 255, 255, 0.03));
  }
}

.service-label {
  font-size: var(--font-size-md);
  color: var(--text-primary);
  flex-shrink: 0;
  min-width: 180px;
}

.service-input-wrap {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
  max-width: 500px;
}

.service-input {
  flex: 1;
  height: 32px;
  padding: 0 12px;
  font-size: var(--font-size-sm);
  font-family: 'SF Mono', 'Cascadia Code', 'Consolas', monospace;
  color: var(--text-primary);
  // background: var(--bg-tertiary, rgba(255, 255, 255, 0.05));
  border: 1px solid var(--border-color, rgba(255, 255, 255, 0.1));
  border-radius: 6px;
  outline: none;
  transition: border-color 0.15s;

  &:focus {
    border-color: var(--accent-color, #3b82f6);
  }

  &:disabled {
    opacity: 0.5;
  }

  &::placeholder {
    color: var(--text-tertiary);
  }
}

.btn-reset {
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--font-size-base);
  color: var(--text-tertiary);
  background: transparent;
  border: 1px solid transparent;
  border-radius: 6px;
  cursor: pointer;
  flex-shrink: 0;
  transition: all 0.15s;

  &:hover:not(:disabled) {
    color: var(--accent-color, #3b82f6);
    background: var(--bg-hover, rgba(255, 255, 255, 0.05));
    border-color: var(--border-color, rgba(255, 255, 255, 0.1));
  }

  &:disabled {
    cursor: not-allowed;
  }
}

.btn-reset-all {
  padding: 8px 20px;
  font-size: var(--font-size-md);

  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);
  border: 1px solid var(--btn-primary-border, transparent);
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.15s;

  &:hover {
     background: var(--btn-primary-hover-bg, var(--color-primary));
    color: var(--btn-primary-hover-text, #fff);
    border-color: var(--btn-primary-hover-border, transparent);
  }
}

.spinner {
  display: inline-block;
  width: 12px;
  height: 12px;
  border: 2px solid var(--border-color, rgba(255, 255, 255, 0.2));
  border-top-color: var(--accent-color, #3b82f6);
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}
</style>
