<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { DEFAULT_CONFIG } from '@/config/index'
import {
  resolveFastKgDownloadSource,
  type FastKgDownloadSource,
} from '@/api/fastKgDownloadConnectivity'
import { useFastClawDownloadDriver } from '@/integrations/download-center'

const props = defineProps<{ modelValue: boolean }>()
const emit = defineEmits<{
  (event: 'update:modelValue', value: boolean): void
  (event: 'installed'): void
  (event: 'install-failed', error: unknown): void
}>()

const { t } = useI18n()
const downloadStore = useFastClawDownloadDriver()
const uploadProgress = ref(0)
const uploadMessage = ref('')
const isUploading = ref(false)
const isStartingDownload = ref(false)
const isCheckingConnectivity = ref(false)
const downloadSource = ref<FastKgDownloadSource | null>(null)
let connectivityCheckId = 0

const visible = computed({
  get: () => props.modelValue,
  set: (value) => emit('update:modelValue', value),
})

const isFastKgDownloadActive = computed(() => {
  return downloadStore.isKgDownloadActive
})

async function checkDownloadConnectivity(): Promise<FastKgDownloadSource | null> {
  const checkId = ++connectivityCheckId
  isCheckingConnectivity.value = true
  try {
    const result = await resolveFastKgDownloadSource({
      internetCheckUrl: String(DEFAULT_CONFIG.CONNECTIVITY_CHECK_URL || ''),
      publicBaseUrl: String(DEFAULT_CONFIG.DOWNLOAD_API_BASE_URL || ''),
      privateBaseUrl: String(DEFAULT_CONFIG.DOWNLOAD_API_BASE_URL_PRIVATE || ''),
    })
    if (checkId !== connectivityCheckId) return downloadSource.value
    downloadSource.value = result.source
    return result.source
  } finally {
    if (checkId === connectivityCheckId) isCheckingConnectivity.value = false
  }
}

watch(visible, (open) => {
  if (open) void checkDownloadConnectivity()
}, { immediate: true })

async function verifyInstallation() {
  const { invoke } = await import('@tauri-apps/api/core')
  const status = await invoke<{ all_exist: boolean; missing_files?: string[] }>('check_kg_service_files')
  if (!status.all_exist) {
    throw new Error(`安装包文件不完整：${status.missing_files?.join('、') || '缺少必要文件'}`)
  }
}

async function completeInstallation() {
  await verifyInstallation()
  visible.value = false
  emit('installed')
}

async function handleOnlineDownload() {
  if (isStartingDownload.value || isFastKgDownloadActive.value) return
  isStartingDownload.value = true
  try {
    const source = await checkDownloadConnectivity()
    if (!source) {
      ElMessage.warning(t('knowledge.downloadSourcesUnavailable'))
      return
    }
    visible.value = false
    await downloadStore.startKgDownload(source.baseUrl, {
      onSuccess: completeInstallation,
    })
  } catch (error: any) {
    emit('install-failed', error)
    ElMessage.error(t('knowledge.downloadFailed', { msg: error?.message || String(error) }))
  } finally {
    isStartingDownload.value = false
  }
}

async function handleOfflineUpload() {
  if (isUploading.value || isFastKgDownloadActive.value) return
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const selected = await open({
      multiple: false,
      filters: [{ name: 'ZIP', extensions: ['zip'] }],
    })
    if (!selected) return

    const filePath = typeof selected === 'string' ? selected : (selected as any).path
    isUploading.value = true
    uploadProgress.value = 0
    uploadMessage.value = ''

    let unlistenUpload: (() => void) | null = null
    try {
      const { listen } = await import('@tauri-apps/api/event')
      unlistenUpload = await listen<{ phase: string; progress: number; message: string }>(
        'kg-upload-progress',
        (event) => {
          uploadProgress.value = event.payload.progress
          uploadMessage.value = event.payload.message
        },
      )
    } catch {
      // 浏览器预览环境没有 Tauri 事件通道。
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke('upload_fastkg_service', { filePath })
      await completeInstallation()
    } finally {
      unlistenUpload?.()
    }
  } catch (error: any) {
    emit('install-failed', error)
    ElMessage.error(t('knowledge.uploadFailed', { msg: error?.message || String(error) }))
  } finally {
    isUploading.value = false
  }
}
</script>

<template>
  <el-dialog
    v-model="visible"
    :title="$t('knowledge.installServiceTitle')"
    width="520px"
    align-center
    class="fastkg-install-dialog"
    :close-on-click-modal="false"
    :close-on-press-escape="!isUploading"
    :show-close="!isUploading"
  >
    <div class="connectivity-banner" :class="isCheckingConnectivity ? 'checking' : downloadSource ? 'online' : 'offline'">
      <span class="conn-dot"></span>
      <span v-if="isCheckingConnectivity">{{ $t('knowledge.checkingNetwork') }}</span>
      <span v-else-if="downloadSource?.type === 'public'">{{ $t('knowledge.networkPublicDownloadAvailable') }}</span>
      <span v-else-if="downloadSource?.type === 'private'">{{ $t('knowledge.networkPrivateDownloadAvailable') }}</span>
      <span v-else>{{ $t('knowledge.downloadSourcesUnavailable') }}</span>
      <button v-if="!isCheckingConnectivity" class="btn-recheck" @click="checkDownloadConnectivity">
        {{ $t('knowledge.recheckConnection') }}
      </button>
    </div>

    <p class="install-description">{{ $t('knowledge.serviceInstallDesc') }}</p>

    <template v-if="downloadSource && !isCheckingConnectivity && !isUploading">
      <div class="action-section">
        <div class="action-icon download-icon">
          <svg viewBox="0 0 24 24" width="32" height="32" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <polyline points="7 10 12 15 17 10" />
            <line x1="12" y1="15" x2="12" y2="3" />
          </svg>
        </div>
        <h4>{{ $t('knowledge.onlineInstallTitle') }}</h4>
        <p>{{ $t('knowledge.onlineInstallDesc') }}</p>
        <div v-if="isFastKgDownloadActive || isStartingDownload" class="download-redirect-tip">
          <span>{{ $t('knowledge.downloadInBackground') }}</span>
          <el-button text type="primary" size="small" @click="downloadStore.openPanel()">
            {{ $t('knowledge.viewProgress') }}
          </el-button>
        </div>
        <el-button v-else type="primary" class="install-button" @click="handleOnlineDownload">
          {{ $t('knowledge.startDownload') }}
        </el-button>
      </div>
      <div class="upload-alt">
        <span>{{ $t('knowledge.orUploadHint') }}</span>
        <button class="upload-alt-link" @click="handleOfflineUpload">{{ $t('knowledge.selectZipAndInstall') }}</button>
      </div>
    </template>

    <div v-if="!downloadSource && !isCheckingConnectivity && !isUploading" class="action-section">
      <div class="action-icon upload-icon">
        <svg viewBox="0 0 24 24" width="32" height="32" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
          <polyline points="17 8 12 3 7 8" />
          <line x1="12" y1="3" x2="12" y2="15" />
        </svg>
      </div>
      <h4>{{ $t('knowledge.offlineInstallTitle') }}</h4>
      <p>{{ $t('knowledge.offlineInstallDesc') }}</p>
      <code class="download-url">{{ DEFAULT_CONFIG.DOWNLOAD_API_BASE_URL }}</code>
      <el-button type="primary" class="install-button" @click="handleOfflineUpload">
        {{ $t('knowledge.selectZipAndInstall') }}
      </el-button>
    </div>

    <div v-if="isUploading" class="progress-section">
      <el-progress :percentage="uploadProgress" />
      <p>{{ uploadMessage || $t('knowledge.processing') }}</p>
    </div>

    <div v-if="isCheckingConnectivity && !isUploading" class="checking-section">
      <span class="spinner"></span>
      <span>{{ $t('knowledge.checkingNetwork') }}</span>
    </div>

    <template #footer>
      <el-button :disabled="isUploading" @click="visible = false">{{ $t('knowledge.installLater') }}</el-button>
    </template>
  </el-dialog>
</template>

<style scoped lang="scss">
.connectivity-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  border-radius: 8px;
  font-size: var(--font-size-md);
  font-weight: 500;

  &.online { background: #f6ffed; color: #52c41a; }
  &.offline { background: #fff7e6; color: #fa8c16; }
  &.checking { background: #f0f5ff; color: #2f54eb; }
}

.conn-dot { width: 8px; height: 8px; border-radius: 50%; background: currentColor; flex-shrink: 0; }
.btn-recheck { margin-left: auto; padding: 4px 12px; border: 1px solid currentColor; border-radius: 4px; background: transparent; color: inherit; cursor: pointer; }
.install-description { margin: 16px 0; color: var(--text-secondary); }
.action-section { text-align: center; padding: 20px; border: 2px dashed var(--border-color); border-radius: 12px; margin-top: 16px; }
.action-section p { color: var(--text-secondary); font-size: var(--font-size-sm); margin: 8px 0 16px; }
.action-icon { width: 56px; height: 56px; border-radius: 50%; display: flex; align-items: center; justify-content: center; margin: 0 auto 12px; }
.download-icon { background: #e6f4ff; color: #1677ff; }
.upload-icon { background: #f0f5ff; color: #2f54eb; }
.install-button { width: 100%; }
.upload-alt { text-align: center; padding-top: 12px; font-size: var(--font-size-sm); color: var(--text-secondary); }
.upload-alt-link { border: 0; background: none; padding: 0 0 0 4px; color: var(--color-primary); cursor: pointer; }
.upload-alt-link:hover { text-decoration: underline; }
.download-redirect-tip { display: flex; align-items: center; justify-content: center; gap: 8px; padding: 10px 16px; background: color-mix(in srgb, var(--color-primary) 8%, transparent); border: 1px dashed color-mix(in srgb, var(--color-primary) 40%, transparent); border-radius: 6px; color: var(--text-secondary); }
.download-url { display: block; margin: 0 0 16px; word-break: break-all; color: var(--text-secondary); }
.progress-section { padding: 24px 0 12px; text-align: center; color: var(--text-secondary); }
.checking-section { display: flex; flex-direction: column; align-items: center; gap: 12px; padding: 20px; color: var(--text-secondary); }
.spinner { width: 24px; height: 24px; border: 3px solid var(--border-color); border-top-color: var(--color-primary); border-radius: 50%; animation: spin 0.8s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
</style>
