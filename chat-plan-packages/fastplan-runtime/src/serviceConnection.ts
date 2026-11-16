import { createFastPlanWebSocketTransport } from './transport'
import type { FastPlanAccess } from './access'

export type FastPlanServiceConnectionStatus = 'connecting' | 'connected' | 'disconnected'

export interface FastPlanServiceConnectionOptions {
  access?: FastPlanAccess
  wsBase?: string
  WebSocket?: typeof WebSocket
  reconnectDelayMs?: number
  maxReconnectAttempts?: number
  onStatusChange?: (status: FastPlanServiceConnectionStatus) => void
}

export interface FastPlanServiceConnection {
  connect: () => void
  dispose: () => void
  status: () => FastPlanServiceConnectionStatus
}

/** Probe the service-wide `/ws` endpoint used by the legacy welcome view. */
export function createFastPlanServiceConnection(
  options: FastPlanServiceConnectionOptions = {},
): FastPlanServiceConnection {
  if (options.access) {
    let current: FastPlanServiceConnectionStatus = 'disconnected'
    const transport = createFastPlanWebSocketTransport({ ...options, socketPath: () => '/ws',
      baseReconnectDelayMs: options.reconnectDelayMs ?? 3000, maxReconnectAttempts: options.maxReconnectAttempts ?? 10,
      shouldReconnect: () => true, onMessage: () => {},
      onConnectionChange: (_id, connected, detail) => {
        const next = connected ? 'connected' : detail?.status === 'failed' || detail?.status === 'idle' ? 'disconnected' : 'connecting'
        if (current !== next) { current = next; options.onStatusChange?.(next) }
      } })
    transport.connect('service')
    return { connect: () => transport.connect('service'), dispose: transport.closeAll, status: () => current }
  }
  const Socket = options.WebSocket || (globalThis as any).WebSocket as typeof WebSocket | undefined
  const base = (options.wsBase || '').replace(/\/$/, '')
  const reconnectDelayMs = options.reconnectDelayMs ?? 3000
  const maxReconnectAttempts = options.maxReconnectAttempts ?? 10
  let socket: WebSocket | null = null
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null
  let reconnectAttempts = 0
  let disposed = false
  let currentStatus: FastPlanServiceConnectionStatus = 'disconnected'

  function updateStatus(status: FastPlanServiceConnectionStatus) {
    if (currentStatus === status) return
    currentStatus = status
    options.onStatusChange?.(status)
  }

  function clearReconnectTimer() {
    if (reconnectTimer) clearTimeout(reconnectTimer)
    reconnectTimer = null
  }

  function scheduleReconnect() {
    if (disposed || reconnectTimer || reconnectAttempts >= maxReconnectAttempts) return
    reconnectAttempts += 1
    reconnectTimer = setTimeout(() => {
      reconnectTimer = null
      connect()
    }, reconnectDelayMs)
  }

  function connect() {
    if (disposed) return
    if (!Socket) {
      updateStatus('disconnected')
      return
    }
    if (socket?.readyState === Socket.OPEN || socket?.readyState === Socket.CONNECTING) return
    clearReconnectTimer()
    updateStatus('connecting')
    let nextSocket: WebSocket
    try {
      nextSocket = new Socket(`${base}/ws`)
    } catch {
      updateStatus('disconnected')
      scheduleReconnect()
      return
    }
    socket = nextSocket
    nextSocket.onopen = () => {
      if (disposed || socket !== nextSocket) return
      reconnectAttempts = 0
      updateStatus('connected')
    }
    nextSocket.onerror = () => {
      if (!disposed && socket === nextSocket) updateStatus('disconnected')
    }
    nextSocket.onclose = () => {
      if (socket !== nextSocket) return
      socket = null
      if (disposed) return
      updateStatus('disconnected')
      scheduleReconnect()
    }
  }

  function dispose() {
    if (disposed) return
    disposed = true
    clearReconnectTimer()
    const activeSocket = socket
    socket = null
    activeSocket?.close()
  }

  connect()
  return { connect, dispose, status: () => currentStatus }
}
