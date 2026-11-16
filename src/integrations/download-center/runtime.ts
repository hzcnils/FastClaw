import type { DownloadRuntime } from 'tauri-download-center-vue'

let runtime: DownloadRuntime | undefined

export function setHostDownloadRuntime(value: DownloadRuntime): void { runtime = value }
export function getHostDownloadRuntime(): DownloadRuntime {
  if (!runtime) throw new Error('Host download runtime has not been initialized')
  return runtime
}
