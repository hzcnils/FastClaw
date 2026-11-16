import { computed, ref, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { ElMessage } from 'element-plus'
import { useI18n } from 'vue-i18n'
import { DEFAULT_CONFIG } from '@/config/index'
import { ensureOnlineBeforeDownload } from '@/api/connectivity'
import { useFastClawDownloadDriver } from '@/integrations/download-center'

const FASTKG_VERSION_POLL_MS = 5 * 60 * 1000
export type FastKgUpdateMode = 'online' | 'offline'

export function useFastKgUpdate(options: {
  /** FastKG 是否已安装（依赖服务状态） */
  isInstalled: Ref<boolean> | (() => boolean)
  /** 更新成功后重启并刷新依赖状态 */
  onAfterUpdate: () => Promise<void>
}) {
  const { t } = useI18n()
  const downloadStore = useFastClawDownloadDriver()

  const localVersion = ref('')
  const remoteVersion = ref('')
  const remoteAvailable = ref(true)
  const hasUpdate = ref(false)
  const dialogVisible = ref(false)
  const dialogMode = ref<FastKgUpdateMode>('online')
  const updateStarting = ref(false)
  const isUploading = ref(false)
  const uploadProgress = ref(0)
  const uploadMessage = ref('')

  let versionPollTimer: ReturnType<typeof setInterval> | null = null

  function resolveInstalled(): boolean {
    const src = options.isInstalled
    return typeof src === 'function' ? src() : !!src.value
  }

  const showOnlineUpdateBtn = computed(
    () => resolveInstalled() && remoteAvailable.value && hasUpdate.value,
  )

  const showOfflineUploadBtn = computed(
    () => resolveInstalled() && !remoteAvailable.value,
  )

  const kgDownloadInProgress = computed(() => {
    if (updateStarting.value || isUploading.value) return true
    return downloadStore.isKgDownloadActive
  })

  // 覆盖前停止由下载组件的宿主 InstallHook 等待执行，不能依赖页面状态 watch。

  async function checkFastKgVersion() {
    if (!resolveInstalled()) {
      hasUpdate.value = false
      remoteAvailable.value = true
      return
    }
    try {
      const result = await invoke<{
        local_version: string
        remote_version: string
        has_update: boolean
        remote_available?: boolean
      }>('check_fastkg_version', {
        apiBaseUrl: DEFAULT_CONFIG.DOWNLOAD_API_BASE_URL,
      })
      localVersion.value = result.local_version || ''
      remoteVersion.value = result.remote_version || ''
      remoteAvailable.value = result.remote_available !== false
      hasUpdate.value = !!result.has_update
    } catch (e) {
      console.warn('[WSP] 检查 FastKG 版本失败:', e)
      remoteAvailable.value = false
      hasUpdate.value = false
    }
  }

  function startVersionPoll() {
    stopVersionPoll()
    versionPollTimer = setInterval(() => {
      void checkFastKgVersion()
    }, FASTKG_VERSION_POLL_MS)
  }

  function stopVersionPoll() {
    if (versionPollTimer) {
      clearInterval(versionPollTimer)
      versionPollTimer = null
    }
  }

  /** 进入页 / 激活 / 切到依赖 Tab：立即检查并（重新）启动轮询 */
  function onEnterWspPage() {
    void checkFastKgVersion()
    startVersionPoll()
  }

  function onLeaveWspPage() {
    stopVersionPoll()
  }

  function openOnlineUpdateDialog() {
    if (kgDownloadInProgress.value || !showOnlineUpdateBtn.value) return
    dialogMode.value = 'online'
    dialogVisible.value = true
  }

  function openOfflineUploadDialog() {
    if (kgDownloadInProgress.value || !showOfflineUploadBtn.value) return
    dialogMode.value = 'offline'
    dialogVisible.value = true
  }

  async function confirmOnlineUpdate() {
    if (kgDownloadInProgress.value) return
    dialogVisible.value = false

    const actuallyOnline = await ensureOnlineBeforeDownload()
    if (!actuallyOnline) return

    updateStarting.value = true
    downloadStore.openPanel()
    try {
      await downloadStore.startKgDownload(DEFAULT_CONFIG.DOWNLOAD_API_BASE_URL, {
        onSuccess: async () => {
          hasUpdate.value = false
          dialogVisible.value = false
          await options.onAfterUpdate()
          await checkFastKgVersion()
        },
      })
    } catch (e: any) {
      console.error('[WSP] FastKG 更新失败:', e)
      ElMessage.error(t('wsp.fastkgUpdateFailed', { msg: e?.message || String(e) }))
      try {
        await options.onAfterUpdate()
      } catch (restartErr) {
        console.warn('[WSP] 更新失败后重启 FastKG 失败:', restartErr)
      }
    } finally {
      updateStarting.value = false
    }
  }

  async function confirmOfflineUpload() {
    if (kgDownloadInProgress.value) return

    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: 'ZIP 压缩包', extensions: ['zip'] }],
      })
      if (!selected) return

      const filePath = typeof selected === 'string' ? selected : (selected as { path?: string }).path
      if (!filePath) return

      isUploading.value = true
      uploadProgress.value = 0
      uploadMessage.value = ''

      let unlistenUpload: (() => void) | null = null
      try {
        unlistenUpload = await listen<{ phase: string; progress: number; message: string }>(
          'kg-upload-progress',
          (event) => {
            uploadProgress.value = event.payload.progress
            uploadMessage.value = event.payload.message
          },
        )
      } catch { /* non-Tauri */ }

      try {
        await invoke('upload_fastkg_service', { filePath })
        dialogVisible.value = false
        await options.onAfterUpdate()
        await checkFastKgVersion()
        ElMessage.success(t('wsp.fastkgUploadUpdateSuccess'))
      } finally {
        unlistenUpload?.()
      }
    } catch (e: any) {
      console.error('[WSP] FastKG 离线上传更新失败:', e)
      ElMessage.error(t('wsp.fastkgUploadUpdateFailed', { msg: e?.message || String(e) }))
      try {
        await options.onAfterUpdate()
      } catch (restartErr) {
        console.warn('[WSP] 上传失败后重启 FastKG 失败:', restartErr)
      }
    } finally {
      isUploading.value = false
      uploadProgress.value = 0
      uploadMessage.value = ''
    }
  }

  return {
    localVersion,
    remoteVersion,
    remoteAvailable,
    hasUpdate,
    dialogVisible,
    dialogMode,
    isUploading,
    uploadProgress,
    uploadMessage,
    showOnlineUpdateBtn,
    showOfflineUploadBtn,
    kgDownloadInProgress,
    checkFastKgVersion,
    onEnterWspPage,
    onLeaveWspPage,
    openOnlineUpdateDialog,
    openOfflineUploadDialog,
    confirmOnlineUpdate,
    confirmOfflineUpload,
  }
}
