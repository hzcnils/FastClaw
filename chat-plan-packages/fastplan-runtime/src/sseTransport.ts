import type { FastPlanAccess } from './access'
import { FastPlanFetchEventSource } from './fetchEventSource'

export interface FastPlanEventSource {
  addEventListener(type: string, listener: (event: any) => void): void
  close(): void
  readonly readyState: number
}

export interface FastPlanSseTransportOptions {
  access?: FastPlanAccess
  fetch?: typeof fetch
  apiBase?: string
  EventSource?: typeof EventSource
  /** Explicit host identity for standalone integrations that want connection sharing. */
  poolScope?: object
  onConnectionChange?: (sessionId: string, connected: boolean) => void
}

type Owner = {
  options: FastPlanSseTransportOptions
  listeners: Map<string, Set<(event: any) => void>>
}

type SharedSource = {
  key: string
  sessionId: string
  pool: Map<string, SharedSource>
  source: FastPlanEventSource
  owners: Set<Owner>
  boundTypes: Set<string>
}

const poolsByScope = new WeakMap<object, Map<string, SharedSource>>()

function poolFor(scope: object) {
  let pool = poolsByScope.get(scope)
  if (!pool) {
    pool = new Map()
    poolsByScope.set(scope, pool)
  }
  return pool
}

function bindType(shared: SharedSource, type: string) {
  if (shared.boundTypes.has(type)) return
  shared.boundTypes.add(type)
  shared.source.addEventListener(type, (event: any) => {
    for (const owner of [...shared.owners]) {
      if (type === 'open') owner.options.onConnectionChange?.(shared.sessionId, true)
      else if (type === 'error') owner.options.onConnectionChange?.(shared.sessionId, false)
      for (const listener of owner.listeners.get(type) || []) listener(event)
    }
  })
}

/** 每个 host/session 只创建一个物理 SSE，各控制器保留独立的事件订阅。 */
export function createFastPlanSseTransport(options: FastPlanSseTransportOptions = {}) {
  const Source = options.EventSource || (globalThis as any).EventSource as typeof EventSource | undefined
  const base = (options.apiBase || '').replace(/\/$/, '')
  // Pool managed-host connections by their shared access object. Without an
  // access identity, keep standalone/test transports isolated.
  const scope = (options.access || options.poolScope || {}) as object
  const pool = poolFor(scope)
  const owner: Owner = { options, listeners: new Map() }
  let shared: SharedSource | null = null
  let proxy: FastPlanEventSource | null = null

  function url(id: string) {
    return `${options.access ? '' : base}/api/sessions/${encodeURIComponent(id)}/events?include_fine_log=true`
  }

  function close() {
    if (!shared) return
    const previous = shared
    shared = null
    proxy = null
    previous.owners.delete(owner)
    owner.options.onConnectionChange?.(previous.sessionId, false)
    owner.listeners.clear()
    if (previous.owners.size) return
    previous.pool.delete(previous.key)
    previous.source.close()
  }

  function open(id: string) {
    if (!Source && !options.access) throw new Error('FastPlan EventSource is unavailable in this environment; inject options.EventSource or load with connect: false')
    if (shared?.sessionId === id && proxy) return proxy
    close()
    const key = `${options.access ? 'access' : base}|${id}`
    let next = pool.get(key)
    if (!next) {
      const source = options.access
        ? new FastPlanFetchEventSource(url(id), options.access, options.fetch)
        : new Source!(url(id))
      next = { key, sessionId: id, pool, source, owners: new Set(), boundTypes: new Set() }
      pool.set(key, next)
    }
    shared = next
    shared.owners.add(owner)
    bindType(shared, 'open')
    bindType(shared, 'error')
    proxy = {
      addEventListener(type: string, listener: (event: any) => void) {
        if (!shared || shared.sessionId !== id) return
        const listeners = owner.listeners.get(type) || new Set<(event: any) => void>()
        listeners.add(listener)
        owner.listeners.set(type, listeners)
        bindType(shared, type)
      },
      close,
      get readyState() { return shared?.source.readyState ?? 2 },
    }
    return proxy
  }

  return { open, close, current: () => proxy, sessionId: () => shared?.sessionId || null, url }
}
