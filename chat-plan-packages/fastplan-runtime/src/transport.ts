import type { FastPlanAccess } from './access'

export interface FastPlanWebSocketTransportOptions {
  wsBase?: string
  access?: FastPlanAccess
  socketPath?: (sessionId: string) => string
  WebSocket?: typeof WebSocket
  heartbeatMs?: number
  authTimeoutMs?: number
  maxReconnectAttempts?: number
  baseReconnectDelayMs?: number
  shouldReconnect?: (sessionId: string) => boolean
  onMessage: (message: any, sessionId: string) => void
  onConnectionChange?: (sessionId: string, connected: boolean, detail?: FastPlanConnectionChange) => void
  onError?: (error: unknown, sessionId: string) => void
}

export interface FastPlanConnectionChange {
  status: 'idle' | 'connecting' | 'connected' | 'reconnecting' | 'failed'
  attempt: number
  maxAttempts: number
  closeCode?: number
}

type Subscriber = { options: FastPlanWebSocketTransportOptions }
type SharedConnection = {
  id: string
  key: string
  pool: Map<string, SharedConnection>
  subscribers: Set<Subscriber>
  Socket: typeof WebSocket
  socket?: WebSocket
  ready: boolean
  opening: boolean
  attempts: number
  heartbeat?: ReturnType<typeof setInterval>
  retry?: ReturnType<typeof setTimeout>
  auth?: ReturnType<typeof setTimeout>
}

// FastPlanAccess 代表同一个父应用/服务身份；其创建的控制器共享 session 连接。
const poolsByScope = new WeakMap<object, Map<string, SharedConnection>>()

function poolFor(scope: object) {
  let pool = poolsByScope.get(scope)
  if (!pool) {
    pool = new Map()
    poolsByScope.set(scope, pool)
  }
  return pool
}

function maxAttempts(c: SharedConnection) {
  return Math.max(0, ...[...c.subscribers].map(item => item.options.maxReconnectAttempts ?? 50))
}

function status(c: SharedConnection, value: FastPlanConnectionChange['status'], closeCode?: number) {
  const detail = { status: value, attempt: c.attempts, maxAttempts: maxAttempts(c), closeCode }
  for (const subscriber of [...c.subscribers]) {
    subscriber.options.onConnectionChange?.(c.id, value === 'connected', detail)
  }
}

function report(c: SharedConnection, message: string) {
  const error = Object.assign(new Error(message), { code: 'FASTPLAN_CONNECTION_ERROR', recoverable: true })
  for (const subscriber of [...c.subscribers]) subscriber.options.onError?.(error, c.id)
}

function clearTimers(c: SharedConnection) {
  clearInterval(c.heartbeat)
  clearTimeout(c.retry)
  clearTimeout(c.auth)
  c.heartbeat = undefined
  c.retry = undefined
  c.auth = undefined
}

function wantsReconnect(c: SharedConnection) {
  return [...c.subscribers].some(item => item.options.shouldReconnect?.(c.id))
}

function fail(c: SharedConnection, code?: number) {
  if (c.pool.get(c.key) !== c) return
  clearTimers(c)
  c.ready = false
  const reconnect = wantsReconnect(c)
  const limit = maxAttempts(c)
  if (!reconnect || c.attempts >= limit || c.subscribers.size === 0) {
    c.pool.delete(c.key)
    status(c, reconnect && c.subscribers.size ? 'failed' : 'idle', code)
    return
  }
  c.attempts += 1
  status(c, 'reconnecting', code)
  const delay = Math.min(
    Math.min(...[...c.subscribers].map(item => item.options.baseReconnectDelayMs ?? 1000))
      * Math.pow(1.5, c.attempts - 1),
    15000,
  )
  c.retry = setTimeout(() => {
    if (c.pool.get(c.key) === c) open(c)
  }, delay)
}

function open(c: SharedConnection) {
  if (c.opening) return
  const first = c.subscribers.values().next().value as Subscriber | undefined
  if (!first) return
  c.opening = true
  clearTimers(c)
  status(c, c.attempts ? 'reconnecting' : 'connecting')

  const attach = (base: string, token?: string) => {
    if (c.pool.get(c.key) !== c || c.subscribers.size === 0) return
    const path = first.options.socketPath?.(c.id) || `/ws/session/${encodeURIComponent(c.id)}`
    const socket = new c.Socket(`${base.replace(/\/$/, '')}${path}`)
    c.opening = false
    c.socket = socket
    const markReady = () => {
      clearTimeout(c.auth)
      c.auth = undefined
      c.ready = true
      c.attempts = 0
      status(c, 'connected')
      const interval = Math.min(...[...c.subscribers].map(item => item.options.heartbeatMs ?? 30000))
      c.heartbeat = setInterval(() => {
        if (c.ready && socket.readyState === c.Socket.OPEN) socket.send(JSON.stringify({ type: 'ping' }))
      }, interval)
    }
    socket.onopen = () => {
      if (c.pool.get(c.key) !== c) return
      if (!first.options.access) {
        markReady()
        return
      }
      socket.send(JSON.stringify({ type: 'authenticate', token }))
      token = undefined
      const authTimeout = Math.max(...[...c.subscribers].map(item => item.options.authTimeoutMs ?? 5000))
      c.auth = setTimeout(() => {
        report(c, 'FastPlan 鉴权超时')
        socket.close(1008, 'Authentication timeout')
      }, authTimeout)
    }
    socket.onmessage = event => {
      if (c.pool.get(c.key) !== c) return
      try {
        const message = JSON.parse(event.data)
        if (!c.ready) {
          if (message.type === 'authenticated') markReady()
          else {
            report(c, 'FastPlan 鉴权失败')
            socket.close(1008, 'Authentication required')
          }
          return
        }
        if (message.type !== 'authenticated') {
          for (const subscriber of [...c.subscribers]) subscriber.options.onMessage(message, c.id)
        }
      } catch {
        report(c, 'FastPlan 消息格式无效')
        if (!c.ready) socket.close(1008, 'Invalid authentication frame')
      }
    }
    socket.onerror = () => {
      if (c.pool.get(c.key) === c) report(c, 'FastPlan WebSocket 连接异常')
    }
    socket.onclose = event => fail(c, event.code)
  }

  if (first.options.access) {
    first.options.access.getConnection().then(info => {
      if (!info.parentToken) throw new Error('Missing token')
      attach(info.wsBase, info.parentToken)
    }).catch(() => {
      if (c.pool.get(c.key) !== c) return
      c.opening = false
      report(c, 'FastPlan 尚未就绪或连接信息不可用')
      fail(c)
    })
  } else {
    attach(first.options.wsBase || '')
  }
}

/** 同 host/session 的多个控制器共享一条物理 WebSocket。 */
export function createFastPlanWebSocketTransport(options: FastPlanWebSocketTransportOptions) {
  const Socket = options.WebSocket || (globalThis as any).WebSocket as typeof WebSocket | undefined
  if (!Socket) {
    const unavailable = () => { throw new Error('FastPlan WebSocket is unavailable in this environment; inject options.WebSocket or load with connect: false') }
    return { connect: unavailable, close: () => {}, closeAll: () => {}, send: () => false,
      has: () => false, state: () => undefined, sessions: () => [] as string[] }
  }
  const SocketCtor: typeof WebSocket = Socket
  // Managed hosts pass one shared FastPlanAccess instance, which is the safe
  // identity boundary for pooling. Standalone callers have no such boundary
  // and therefore retain the historical per-factory isolation.
  const scope = (options.access || {}) as object
  const pool = poolFor(scope)
  const subscriber: Subscriber = { options }
  const owned = new Map<string, SharedConnection>()

  function connectionKey(id: string) {
    const path = options.socketPath?.(id) || `/ws/session/${encodeURIComponent(id)}`
    return `${options.access ? 'access' : options.wsBase || ''}|${path}`
  }

  function connect(id: string) {
    const key = connectionKey(id)
    let connection = pool.get(key)
    if (!connection) {
      connection = { id, key, pool, subscribers: new Set(), Socket: SocketCtor, ready: false, opening: false, attempts: 0 }
      pool.set(key, connection)
    }
    const c: SharedConnection = connection
    c.subscribers.add(subscriber)
    owned.set(id, c)
    if (!c.socket || (c.socket.readyState !== SocketCtor.OPEN && c.socket.readyState !== SocketCtor.CONNECTING)) open(c)
    else if (c.ready) options.onConnectionChange?.(id, true, { status: 'connected', attempt: 0, maxAttempts: maxAttempts(c) })
  }

  function close(id: string) {
    const c = owned.get(id)
    if (!c) return
    owned.delete(id)
    c.subscribers.delete(subscriber)
    const remainingMax = c.subscribers.size ? maxAttempts(c) : (options.maxReconnectAttempts ?? 50)
    options.onConnectionChange?.(id, false, { status: 'idle', attempt: 0, maxAttempts: remainingMax })
    if (c.subscribers.size) return
    c.pool.delete(c.key)
    clearTimers(c)
    c.ready = false
    c.socket?.close()
  }

  function send(id: string | null | undefined, payload: unknown) {
    const c = id ? owned.get(id) : undefined
    if (!c?.ready || c.pool.get(c.key) !== c || c.socket?.readyState !== SocketCtor.OPEN) return false
    c.socket.send(JSON.stringify(payload))
    return true
  }

  return {
    connect,
    close,
    closeAll: () => { for (const id of [...owned.keys()]) close(id) },
    send,
    has: (id: string) => {
      const c = owned.get(id)
      return !!c && c.pool.get(c.key) === c
    },
    state: (id: string) => {
      const c = owned.get(id)
      if (!c || c.pool.get(c.key) !== c) return undefined
      return c.socket?.readyState
    },
    sessions: () => [...owned].filter(([, c]) => c.pool.get(c.key) === c).map(([id]) => id),
  }
}
