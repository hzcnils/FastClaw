import { computed, reactive, ref } from 'vue'
import type { ApiError, ApiResult, InvokeAdapter, InvokeOptions } from './types'

export class TauriIpcError extends Error {
  constructor(
    public readonly code: string,
    message: string,
    public readonly details?: unknown,
    public readonly transport = false,
  ) {
    super(message)
    this.name = 'TauriIpcError'
  }
}

const tauriInvoke: InvokeAdapter = async <T>(command: string, args?: Record<string, unknown>) => {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<T>(command, args)
}
let invokeAdapter: InvokeAdapter = tauriInvoke

/** Allows browser previews and tests to replace the Tauri transport. */
export function setInvokeAdapter(adapter: InvokeAdapter): void {
  invokeAdapter = adapter
}

export function resetInvokeAdapter(): void {
  invokeAdapter = tauriInvoke
}

export async function invokeRaw<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  return invokeAdapter<T>(command, args)
}

export async function invokeApi<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  let result: ApiResult<T>
  try {
    result = await invokeAdapter<ApiResult<T>>(command, args)
  } catch (cause) {
    if (cause instanceof TauriIpcError) throw cause
    throw new TauriIpcError(
      'IPC_ERROR',
      cause instanceof Error ? cause.message : String(cause),
      cause,
      true,
    )
  }

  if (!result || typeof result.success !== 'boolean') {
    throw new TauriIpcError('INVALID_RESPONSE', 'Rust 接口返回格式无效', result)
  }
  if (!result.success) {
    const error: ApiError = result.error ?? {
      code: 'BUSINESS_ERROR',
      message: '操作失败',
    }
    throw new TauriIpcError(error.code, error.message, error.details)
  }
  return result.data as T
}

/** Shared loading/error state for callers that need declarative UI feedback. */
export function useTauriApi() {
  const pending = reactive(new Map<string, number>())
  const error = ref<TauriIpcError | null>(null)
  const loading = computed(() => pending.size > 0)

  const isLoading = (key: string) => (pending.get(key) ?? 0) > 0

  async function execute<T>(options: InvokeOptions): Promise<T> {
    const key = options.loadingKey ?? options.command
    pending.set(key, (pending.get(key) ?? 0) + 1)
    error.value = null
    try {
      return await invokeApi<T>(options.command, options.args)
    } catch (cause) {
      error.value = cause instanceof TauriIpcError
        ? cause
        : new TauriIpcError('UNKNOWN_ERROR', String(cause), cause)
      throw error.value
    } finally {
      const count = (pending.get(key) ?? 1) - 1
      if (count > 0) pending.set(key, count)
      else pending.delete(key)
    }
  }

  return { loading, error, execute, isLoading }
}
