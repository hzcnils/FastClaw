export interface DownloadCenterUiMessages {
  title: string
  empty: string
  active: string
  pause: string
  resume: string
  cancel: string
  retry: string
  remove: string
  close: string
  statusPending: string
  statusDownloading: string
  statusPausing: string
  statusPaused: string
  statusVerifying: string
  statusInstalling: string
  statusSuccess: string
  statusFailed: string
  statusCancelled: string
  [messageKey: string]: string
}

export interface DownloadCenterUiConfig {
  locale: string
  title: string
  emptyText: string
  messages: Record<string, { downloadCenter: DownloadCenterUiMessages }>
}

export type DownloadCenterUiConfigInput = Partial<Omit<DownloadCenterUiConfig, 'messages'>> & {
  messages?: Record<string, { downloadCenter?: Partial<DownloadCenterUiMessages> }>
}

export const defaultDownloadCenterUiConfig: DownloadCenterUiConfig = {
  locale: 'zh-CN',
  title: '下载中心',
  emptyText: '暂无下载任务',
  messages: {
    'zh-CN': { downloadCenter: {
      downloaded: '已下载',
      title: '下载中心', empty: '暂无下载任务', active: '{n} 个任务进行中', pause: '暂停', resume: '继续', cancel: '取消', retry: '重试', remove: '删除记录', close: '关闭',
      statusPending: '等待中', statusDownloading: '下载中', statusPausing: '正在暂停', statusPaused: '已暂停', statusVerifying: '校验中', statusInstalling: '安装中', statusSuccess: '已完成', statusFailed: '失败', statusCancelled: '已取消',
      'download.queued': '等待下载', 'download.downloading': '正在下载', 'download.pausing': '正在暂停', 'download.paused': '已暂停', 'download.verifying': '正在校验', 'download.extracting': '正在解压', 'download.installing': '正在安装', 'download.completed': '下载完成', 'download.cancelled': '已取消', 'download.restartPaused': '应用已重启，请手动继续', 'download.failed': '下载失败',
    } },
    en: { downloadCenter: {
      downloaded: 'Downloaded',
      title: 'Downloads', empty: 'No download tasks', active: '{n} active', pause: 'Pause', resume: 'Resume', cancel: 'Cancel', retry: 'Retry', remove: 'Remove', close: 'Close',
      statusPending: 'Pending', statusDownloading: 'Downloading', statusPausing: 'Pausing', statusPaused: 'Paused', statusVerifying: 'Verifying', statusInstalling: 'Installing', statusSuccess: 'Completed', statusFailed: 'Failed', statusCancelled: 'Cancelled',
      'download.queued': 'Queued', 'download.downloading': 'Downloading', 'download.pausing': 'Pausing', 'download.paused': 'Paused', 'download.verifying': 'Verifying', 'download.extracting': 'Extracting', 'download.installing': 'Installing', 'download.completed': 'Completed', 'download.cancelled': 'Cancelled', 'download.restartPaused': 'Application restarted; resume manually', 'download.failed': 'Download failed',
    } },
  },
}

export function resolveDownloadCenterUiConfig(input: DownloadCenterUiConfigInput = {}, base: DownloadCenterUiConfig = defaultDownloadCenterUiConfig): DownloadCenterUiConfig {
  const messages = { ...base.messages }
  for (const [locale, value] of Object.entries(input.messages ?? {})) {
    const fallback = messages[locale]?.downloadCenter ?? messages['zh-CN'].downloadCenter
    messages[locale] = { downloadCenter: { ...fallback, ...value.downloadCenter } as DownloadCenterUiMessages }
  }
  return { ...base, ...input, messages } as DownloadCenterUiConfig
}

export function createDownloadCenterText(config: DownloadCenterUiConfig) { return (key: keyof DownloadCenterUiMessages | string, params?: Record<string, string | number>): string => {
  const values = config.messages[config.locale]?.downloadCenter ?? config.messages['zh-CN'].downloadCenter
  if (key === 'title') return config.title || values.title
  if (key === 'empty') return config.emptyText || values.empty
  const template = values[key as keyof DownloadCenterUiMessages] ?? String(key)
  return Object.entries(params ?? {}).reduce((value, [name, replacement]) => value.replaceAll(`{${name}}`, String(replacement)), template)
} }
