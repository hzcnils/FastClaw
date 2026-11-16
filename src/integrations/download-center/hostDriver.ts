import type { DownloadTask, StartDownloadInput } from 'tauri-download-center-vue'
import { getHostDownloadRuntime } from './runtime'
import { isFastKgDownloadTaskActive } from './taskState'

type CompletionCallback = () => Promise<void> | void
const completionCallbacks = new Map<string, CompletionCallback>()
type Deferred = { resolve:(task:DownloadTask)=>void;reject:(error:Error)=>void;silent:boolean }
const deferredTasks = new Map<string, Deferred>()

async function invokeHost<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<T>(command, args)
}

async function startApp(
  appName: 'FastKG' | 'File2Md',
  apiBaseUrl: string,
  options?: { onSuccess?: CompletionCallback; silent?: boolean; targetPath?: string; install?: boolean },
) {
  const runtime = getHostDownloadRuntime()
  const input = await invokeHost<StartDownloadInput>('prepare_app_download', {
    apiBaseUrl, appName, silent: options?.silent ?? false, targetPath: options?.targetPath, install: options?.install,
  })
  if (options?.onSuccess) completionCallbacks.set(input.resourceId, options.onSuccess)
  return startAndWait(input, options?.silent ?? false)
}

async function startAndWait(input: StartDownloadInput, silent = false): Promise<DownloadTask> {
  const completion = new Promise<DownloadTask>((resolve, reject) => deferredTasks.set(input.resourceId, { resolve, reject, silent }))
  try { await getHostDownloadRuntime().start(input) }
  catch (error) { deferredTasks.delete(input.resourceId); throw error }
  return completion
}

/** Host lifecycle hook passed to DownloadCenterVuePlugin. */
export async function handleFastClawDownloadCompleted(task: DownloadTask): Promise<void> {
  try {
    if (task.resourceType === 'fastclaw-app') {
      const appName = String(task.payload?.appName ?? task.name)
      const version = task.payload?.version != null ? String(task.payload.version) : undefined
      await invokeHost('complete_app_download', { appName, version })
    } else if (task.resourceType === 'wsp-model') {
      await invokeHost('complete_wsp_download', { modelId: task.resourceId, targetPath: String(task.payload?.targetPath ?? '') })
    } else return
    const callback = completionCallbacks.get(task.resourceId)
    completionCallbacks.delete(task.resourceId)
    await callback?.()
    deferredTasks.get(task.resourceId)?.resolve(task)
  } catch (cause) {
    deferredTasks.get(task.resourceId)?.reject(cause instanceof Error ? cause : new Error(String(cause)))
  } finally {
    completionCallbacks.delete(task.resourceId)
    deferredTasks.delete(task.resourceId)
  }
}

export function handleFastClawDownloadFailed(task: DownloadTask): void {
  completionCallbacks.delete(task.resourceId)
  const deferred = deferredTasks.get(task.resourceId)
  if (deferred) {
    if (deferred.silent) deferred.resolve(task)
    else deferred.reject(new Error(task.error?.message ?? task.messageKey ?? 'Download failed'))
  }
  deferredTasks.delete(task.resourceId)
}

export function handleFastClawDownloadUpdated(task: DownloadTask): void {
  if (task.status === 'cancelled') handleFastClawDownloadFailed(task)
}

/**
 * FastClaw-only resource driver facade. Generic task state is owned by the
 * package runtime; this adapter only resolves FastClaw metadata and completion
 * side effects.
 */
export function useFastClawDownloadDriver() {
  const runtime = getHostDownloadRuntime()
  return {
    get tasks() { return runtime.tasks.value },
    get hasActiveTasks() { return runtime.activeCount.value > 0 },
    get isKgDownloadActive() { return runtime.tasks.value.some(isFastKgDownloadTaskActive) },
    openPanel: runtime.openPanel,
    startKgDownload: (
      apiBaseUrl: string,
      options?: { onSuccess?: CompletionCallback; silent?: boolean },
    ) => startApp('FastKG', apiBaseUrl, options),
    startFile2MdDownload: (apiBaseUrl: string, callbacks?: { onSuccess?: CompletionCallback; silent?: boolean }) => startApp('File2Md', apiBaseUrl, callbacks),
    async startWspDownload(modelId: string | number, modelName: string) {
      const input = await invokeHost<StartDownloadInput>('prepare_wsp_download', { modelId: String(modelId), modelName })
      return startAndWait(input)
    },
    cancelTask: runtime.cancel,
    retryTask: runtime.retry,
    removeTask: runtime.remove,
  }
}
