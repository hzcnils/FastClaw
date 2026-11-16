export interface ApiError {
  code: string
  message: string
  details?: unknown
}

export interface ApiResult<T> {
  success: boolean
  data: T | null
  error: ApiError | null
}

export interface InvokeOptions {
  command: string
  args?: Record<string, unknown>
  loadingKey?: string
}

export type InvokeAdapter = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>

