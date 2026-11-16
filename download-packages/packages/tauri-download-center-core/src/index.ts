export const DOWNLOAD_CENTER_IPC_PROTOCOL_VERSION = 1

export interface ApiError { code: string; message: string; details?: unknown }
export interface ApiResult<T> { success: boolean; data: T | null; error: ApiError | null }
export type InvokeTransport = <T>(command: string, args?: Record<string, unknown>) => Promise<T>
export interface DownloadClientOptions { invoke?: InvokeTransport; command?: string }
export type DownloadStatus = 'pending' | 'downloading' | 'pausing' | 'paused' | 'verifying' | 'installing' | 'success' | 'failed' | 'cancelled'
export type DownloadInstruction =
  | {type:'download';url:string;tempPath:string;resume?:boolean}
  | {type:'verifySha256';sha256:string}
  | {type:'extractZip';stagingPath:string;requiredFileSuffix?:string}
  | {type:'backupTarget';targetPath:string;backupPath?:string}
  | {type:'replaceTarget';source:'temp'|'staging';targetPath:string}
  | {type:'overlayTarget';source:'temp'|'staging';targetPath:string}
  | {type:'cleanup';targets:Array<'temp'|'staging'|'backup'>}
export interface DownloadExecutionPlan { instructions:DownloadInstruction[] }
export interface DownloadTask { id: string; resourceType: string; resourceId: string; name: string; status: DownloadStatus; phase: string; progress: number; downloadedBytes: number; totalBytes?: number; messageKey?: string; messageParams?: Record<string,string|number>; error?: ApiError; silent?: boolean; payload?: Record<string, unknown>; createdAt: string; updatedAt: string }
export interface StartDownloadInput { resourceType: string; resourceId: string; name: string; payload: Record<string, unknown>; silent?: boolean }

export class DownloadApiError extends Error {
  constructor(public readonly apiError: ApiError) { super(apiError.message); this.name = 'DownloadApiError' }
}

export function createDownloadClient(options: DownloadClientOptions = {}) {
  const transport: InvokeTransport = options.invoke ?? (async (command, args) => {
    const { invoke } = await import('@tauri-apps/api/core')
    return invoke(command, args)
  })
  const command = options.command ?? 'plugin:download-center|execute'
  return async function execute<T>(action: string, payload?: unknown): Promise<T> {
    const result = await transport<ApiResult<T>>(command, { request: { action, payload } })
    if (!result.success) {
      throw new DownloadApiError(result.error ?? { code: 'INTERNAL_ERROR', message: 'Empty plugin response' })
    }
    return result.data as T
  }
}

export interface DownloadStateSnapshot<T = unknown> { tasks: T[]; loading: boolean; error: ApiError | null }
export class DownloadState<T = unknown> {
  private value: DownloadStateSnapshot<T> = { tasks: [], loading: false, error: null }
  private readonly listeners = new Set<(value: DownloadStateSnapshot<T>) => void>()
  get snapshot(): Readonly<DownloadStateSnapshot<T>> { return this.value }
  subscribe(listener: (value: DownloadStateSnapshot<T>) => void): () => void { this.listeners.add(listener); listener(this.value); return () => this.listeners.delete(listener) }
  patch(patch: Partial<DownloadStateSnapshot<T>>): void { this.value = { ...this.value, ...patch }; this.listeners.forEach(listener => listener(this.value)) }
}
