import { computed, inject, reactive, readonly, ref, type ComputedRef, type InjectionKey, type Ref } from 'vue'
import { createDownloadClient, DownloadApiError, type ApiError, type DownloadClientOptions, type DownloadTask, type StartDownloadInput } from 'tauri-download-center-core'
import type { DownloadCenterUiConfig, DownloadCenterUiConfigInput } from './uiConfig'
import { createDownloadCenterText, resolveDownloadCenterUiConfig } from './uiConfig'

export type DownloadEventTransport = <T>(eventName: string, handler: (payload: T) => void) => Promise<() => void>

export interface DownloadRuntimeOptions {
  config: DownloadCenterUiConfig
  client?: DownloadClientOptions
  eventName?: string
  listen?: DownloadEventTransport
  onTaskUpdated?: (task: DownloadTask) => void | Promise<void>
  onTaskCompleted?: (task: DownloadTask) => void | Promise<void>
  onTaskFailed?: (task: DownloadTask) => void | Promise<void>
}

export interface DownloadRuntime {
  config: DownloadCenterUiConfig
  text: ReturnType<typeof createDownloadCenterText>
  configureUi(input: DownloadCenterUiConfigInput): void
  tasks: Readonly<Ref<readonly DownloadTask[]>>
  panelVisible: Ref<boolean>
  activeCount: ComputedRef<number>
  loading: Readonly<Ref<boolean>>
  error: Readonly<Ref<ApiError | null>>
  initialize(): Promise<void>
  start(input: StartDownloadInput): Promise<DownloadTask>
  pause(id: string): Promise<DownloadTask>
  resume(id: string): Promise<DownloadTask>
  cancel(id: string): Promise<DownloadTask>
  retry(id: string): Promise<DownloadTask>
  remove(id: string): Promise<void>
  openPanel(): void
  closePanel(): void
  dispose(): void
}

export const downloadRuntimeKey: InjectionKey<DownloadRuntime> = Symbol('download-center-runtime')

export function createDownloadRuntime(options: DownloadRuntimeOptions): DownloadRuntime {
  const config = reactive(options.config) as DownloadCenterUiConfig
  const execute = createDownloadClient(options.client)
  const tasks = ref<DownloadTask[]>([])
  const panelVisible = ref(false)
  const loading = ref(false)
  const error = ref<ApiError | null>(null)
  const activeCount = computed(() => tasks.value.filter(task => ['pending', 'downloading', 'pausing', 'verifying', 'installing'].includes(task.status)).length)
  let initialized = false
  let unlisten: (() => void) | undefined
  const upsert = (task: DownloadTask) => {
    const previous = tasks.value.find(item => item.id === task.id)
    const index = tasks.value.findIndex(item => item.id === task.id)
    if (index < 0) tasks.value.unshift(task); else tasks.value[index] = task
    void options.onTaskUpdated?.(task)
    if (previous?.status !== task.status && task.status === 'success') void options.onTaskCompleted?.(task)
    if (previous?.status !== task.status && task.status === 'failed') void options.onTaskFailed?.(task)
  }
  const run = async <T>(operation: () => Promise<T>) => {
    loading.value = true; error.value = null
    try { return await operation() }
    catch (cause) { error.value = cause instanceof DownloadApiError ? cause.apiError : { code: 'IPC_ERROR', message: cause instanceof Error ? cause.message : String(cause) }; throw cause }
    finally { loading.value = false }
  }
  const action = (name: string, id: string) => run(async () => { const task = await execute<DownloadTask>(name, { taskId: id }); upsert(task); return task })
  return {
    config, text: createDownloadCenterText(config), configureUi(input) { Object.assign(config, resolveDownloadCenterUiConfig(input, config)) }, tasks: readonly(tasks), panelVisible, activeCount, loading: readonly(loading), error: readonly(error),
    async initialize() {
      if (initialized) return
      tasks.value = await run(() => execute<DownloadTask[]>('list'))
      const eventName = options.eventName ?? 'download-task-updated'
      if (options.listen) unlisten = await options.listen<DownloadTask>(eventName, upsert)
      else {
        const { listen } = await import('@tauri-apps/api/event')
        unlisten = await listen<DownloadTask>(eventName, event => upsert(event.payload))
      }
      initialized = true
    },
    async start(input) { return run(async () => { const task = await execute<DownloadTask>('start', input); upsert(task); if (!input.silent) panelVisible.value = true; return task }) },
    pause: id => action('pause', id), resume: id => action('resume', id), cancel: id => action('cancel', id), retry: id => action('retry', id),
    async remove(id) { await run(() => execute<unknown>('remove', { taskId: id })); tasks.value = tasks.value.filter(task => task.id !== id) },
    openPanel() { panelVisible.value = true }, closePanel() { panelVisible.value = false },
    dispose() { unlisten?.(); unlisten = undefined; initialized = false },
  }
}

export function useDownloadRuntime(): DownloadRuntime {
  const runtime = inject(downloadRuntimeKey)
  if (!runtime) throw new Error('tauri-download-center-vue is not installed')
  return runtime
}
