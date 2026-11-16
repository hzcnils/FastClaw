import { invoke } from '@tauri-apps/api/core'
import { tempDir } from '@tauri-apps/api/path'
import { writeFile } from '@tauri-apps/plugin-fs'
import { useAppConfigStore } from '@/stores/appConfig'
import { useFastClawDownloadDriver } from '@/integrations/download-center'

let installation: Promise<void> | null = null
function ensureFile2Md() {
  if (!installation) {
    installation = (async () => {
      // Older hosts may not expose this probe; OCR retains its backend fallback.
      const exists = await invoke<boolean>('check_file2md_exists').catch(() => true)
      if (exists) return
      const apiBaseUrl = useAppConfigStore().config.remote_servers.download_api_base_url
      const task = await useFastClawDownloadDriver().startFile2MdDownload(apiBaseUrl, { silent: true })
      if (task.status !== 'success') throw new Error(task.status === 'cancelled' ? '下载已取消，OCR插件未安装' : 'OCR插件下载失败')
    })().finally(() => { installation = null })
  }
  return installation
}

/** Cancelling one attachment does not cancel an installation shared by other Tabs. */
function untilAborted<T>(promise: Promise<T>, signal: AbortSignal): Promise<T> {
  return new Promise((resolve, reject) => {
    const abort = () => reject(new DOMException('Attachment removed', 'AbortError'))
    signal.addEventListener('abort', abort, { once: true })
    if (signal.aborted) abort()
    promise.then(resolve, reject).finally(() => signal.removeEventListener('abort', abort))
  })
}

export async function recognizeImageText(file: File | undefined, previewUrl: string, signal: AbortSignal): Promise<string> {
  await untilAborted(ensureFile2Md(), signal)
  signal.throwIfAborted()
  if (!file && !previewUrl) throw new Error('图片数据不可用')
  const blob = file || await fetch(previewUrl, { signal }).then(response => response.blob())
  if (!blob) throw new Error('图片数据不可用')
  const bytes = new Uint8Array(await blob.arrayBuffer())
  signal.throwIfAborted()
  const directory = await tempDir()
  signal.throwIfAborted()
  const path = `${directory}fastclaw_ocr_${crypto.randomUUID()}.${blob.type === 'image/jpeg' ? 'jpg' : 'png'}`
  try {
    await writeFile(path, bytes)
    signal.throwIfAborted()
    // IPC cannot be aborted; attachmentTasks checks the owner again on completion.
    return await invoke<string>('file2md_ocr_by_path', { imagePath: path })
  } finally {
    try { await invoke('file2md_cleanup_temp', { path }) } catch { /* best effort */ }
  }
}
