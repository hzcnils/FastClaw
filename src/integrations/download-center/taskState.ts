import type { DownloadTask } from 'tauri-download-center-vue'

const ACTIVE_DOWNLOAD_STATUSES = new Set<DownloadTask['status']>([
  'pending',
  'downloading',
  'pausing',
  'paused',
  'verifying',
  'installing',
])

export function isDownloadTaskActive(task: Pick<DownloadTask, 'status'>): boolean {
  return ACTIVE_DOWNLOAD_STATUSES.has(task.status)
}

export function isFastKgDownloadTask(task: Partial<DownloadTask>): boolean {
  const resourceId = String(task.resourceId ?? '').toLowerCase()
  const appName = String(task.payload?.appName ?? task.name ?? '').toLowerCase()
  return resourceId === 'fastkg' || appName === 'fastkg'
}

export function isFastKgDownloadTaskActive(task: Partial<DownloadTask>): boolean {
  return isFastKgDownloadTask(task) && !!task.status
    && ACTIVE_DOWNLOAD_STATUSES.has(task.status)
}
