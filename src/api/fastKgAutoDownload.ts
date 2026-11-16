import { invoke } from '@tauri-apps/api/core'
import { DEFAULT_CONFIG } from '@/config/index'
import { useFastClawDownloadDriver } from '@/integrations/download-center'

let bootstrapStarted = false

/**
 * Public-source bootstrap: keep the repository free of FastKG executables and
 * fetch the supported package on first desktop startup instead.
 */
export async function ensureFastKgPackageDownloaded(): Promise<void> {
  if (bootstrapStarted) return
  bootstrapStarted = true

  try {
    const status = await invoke<{ all_exist: boolean }>('check_kg_service_files')
    if (status.all_exist) return

    const download = useFastClawDownloadDriver()
    if (download.isKgDownloadActive) return

    const publicBaseUrl = String(DEFAULT_CONFIG.DOWNLOAD_API_BASE_URL || '').trim()
    if (!publicBaseUrl) {
      console.warn('[FastKG] 未配置公开下载地址，跳过自动下载')
      return
    }

    // Do not block application startup. The download center persists progress
    // and the existing completion hook validates and installs the package.
    void download.startKgDownload(publicBaseUrl, { silent: true }).catch((error) => {
      console.warn('[FastKG] 自动下载失败，可在依赖服务页重试:', error)
    })
  } catch {
    // Browser preview/non-Tauri environments have no native command channel.
  }
}
