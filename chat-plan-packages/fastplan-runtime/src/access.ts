import { throwIfFastPlanAborted, waitForFastPlanOperation } from './cancellation'

export interface FastPlanConnectionInfo { wsBase: string; apiBase: string; parentToken: string }
export interface FastPlanRequestModel {
  provider: string
  model: string
  base_url: string
  auth_type: 'bearer' | 'none'
  api_key?: string
}
/** Host callbacks only. Credentials must never be stored in timeline/model state. */
export interface FastPlanAccess {
  getConnection: () => Promise<FastPlanConnectionInfo>
  resolveModel: (selection: unknown) => Promise<FastPlanRequestModel>
}
export const FASTPLAN_ACCESS_KEY = 'planflow.parent-access'

export async function fastPlanRequest(url: string, init: RequestInit = {}, access?: FastPlanAccess, request: typeof fetch = fetch) {
  // Log only history GETs and fixed stage metadata; never log tokens, headers or response bodies.
  const historyPath = /\/api\/(?:v2\/)?sessions\/[^/?#]+(?:\/execution-state)?$/.test(url) && (!init.method || init.method === 'GET')
  const trace = (stage: string, details: Record<string, unknown> = {}) => {
    if (historyPath) console.info(`[FastPlanHistory] request.${stage}`, JSON.stringify({ path: url.replace(/^https?:\/\/[^/]+/, ''), ...details }))
  }
  let stage = 'begin'
  trace(stage, { managed: !!access })
  try {
    throwIfFastPlanAborted(init.signal)
    if (!access) {
      stage = 'fetch'
      trace(stage)
      const response = await request(url, init)
      trace('response', { status: response.status })
      return response
    }
    stage = 'connection'
    trace(stage)
    const connection = await waitForFastPlanOperation(access.getConnection(), init.signal)
    throwIfFastPlanAborted(init.signal)
    stage = 'validate'
    trace(stage, { hasApiBase: !!connection.apiBase, hasParentToken: !!connection.parentToken })
    const target = new URL(url, connection.apiBase)
    if (target.origin !== new URL(connection.apiBase).origin || target.username || target.password) {
      throw new Error('FastPlan 请求地址与当前托管服务不一致')
    }
    if (!connection.parentToken) throw new Error('FastPlan 连接令牌不可用')
    const headers = new Headers(init.headers)
    headers.set('Authorization', `Bearer ${connection.parentToken}`)
    stage = 'fetch'
    trace(stage, { origin: target.origin })
    const response = await request(target.href, { ...init, headers, redirect: 'error' })
    trace('response', { status: response.status })
    return response
  } catch (error) {
    trace('failed', { stage, errorType: error instanceof Error ? error.name : typeof error })
    throw error
  }
}

/** Allowlist display metadata for events and persistence, including old saved selections. */
export function publicFastPlanModel(value: unknown): string | Record<string, string> | undefined {
  if (typeof value === 'string') return value
  if (!value || typeof value !== 'object') return undefined
  const result: Record<string, string> = {}
  for (const key of ['id', 'model', 'name', 'displayName', 'provider']) {
    const field = (value as Record<string, unknown>)[key]
    if (typeof field === 'string') result[key] = field
  }
  return result
}

export async function downloadFastPlanFile(url: string, filename: string, access: FastPlanAccess) {
  const response = await fastPlanRequest(url, {}, access)
  if (!response.ok) throw new Error(`FastPlan 下载失败 (${response.status})`)
  const objectUrl = URL.createObjectURL(await response.blob())
  const anchor = document.createElement('a')
  anchor.href = objectUrl; anchor.download = filename; anchor.click()
  setTimeout(() => URL.revokeObjectURL(objectUrl), 1000)
}
