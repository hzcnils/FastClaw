import type { App } from 'vue'
import DownloadCenterButton from './DownloadButton.vue'
import DownloadCenterPanel from './DownloadCenter.vue'
import DownloadTaskItem from './DownloadTaskItem.vue'
import { createDownloadRuntime, downloadRuntimeKey, type DownloadEventTransport, type DownloadRuntime, type DownloadRuntimeOptions } from './runtime'
import type { DownloadClientOptions } from 'tauri-download-center-core'
import { resolveDownloadCenterUiConfig, type DownloadCenterUiConfigInput } from './uiConfig'

export * from 'tauri-download-center-core'
export * from './uiConfig'
export * from './runtime'
export { DownloadCenterButton, DownloadCenterPanel, DownloadTaskItem }

export interface DownloadCenterVuePluginOptions extends DownloadCenterUiConfigInput {
  i18n?: { global?: { mergeLocaleMessage(locale: string, message: Record<string, unknown>): void }; mergeLocaleMessage?(locale: string, message: Record<string, unknown>): void }
  client?: DownloadClientOptions
  eventName?: string
  listen?: DownloadEventTransport
  onTaskUpdated?: DownloadRuntimeOptions['onTaskUpdated']
  onTaskCompleted?: DownloadRuntimeOptions['onTaskCompleted']
  onTaskFailed?: DownloadRuntimeOptions['onTaskFailed']
  runtimeInstance?: DownloadRuntime
}

export const DownloadCenterVuePlugin = {
  install(app: App, options: DownloadCenterVuePluginOptions = {}) {
    const { i18n, client, eventName, listen, onTaskUpdated, onTaskCompleted, onTaskFailed, runtimeInstance, ...uiOptions } = options
    const config = resolveDownloadCenterUiConfig(uiOptions)
    const runtime = runtimeInstance ?? createDownloadRuntime({ config, client, eventName, listen, onTaskUpdated, onTaskCompleted, onTaskFailed })
    if (runtimeInstance) runtime.configureUi(uiOptions)
    app.provide(downloadRuntimeKey, runtime)
    const target = i18n?.global ?? i18n
    Object.entries(runtime.config.messages).forEach(([locale, messages]) => target?.mergeLocaleMessage?.(locale, messages))
    app.component('DownloadCenterButton', DownloadCenterButton)
    app.component('DownloadCenterPanel', DownloadCenterPanel)
    app.component('DownloadTaskItem', DownloadTaskItem)
  },
}

export default DownloadCenterVuePlugin
