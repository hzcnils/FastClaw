import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createFastPlanServiceConnection } from './serviceConnection'

class MockSocket {
  static OPEN = 1
  static CONNECTING = 0
  static instances: MockSocket[] = []
  readyState = MockSocket.CONNECTING
  onopen: (() => void) | null = null
  onerror: (() => void) | null = null
  onclose: (() => void) | null = null
  close = vi.fn(() => { this.readyState = 3; this.onclose?.() })

  constructor(public url: string) { MockSocket.instances.push(this) }
  open() { this.readyState = MockSocket.OPEN; this.onopen?.() }
  disconnect() { this.readyState = 3; this.onclose?.() }
}

describe('FastPlan service connection', () => {
  beforeEach(() => { vi.useFakeTimers(); MockSocket.instances = [] })
  afterEach(() => vi.useRealTimers())

  it('reports the global websocket state and reconnects', () => {
    const statuses: string[] = []
    const connection = createFastPlanServiceConnection({
      wsBase: 'ws://fastplan.test/',
      WebSocket: MockSocket as any,
      reconnectDelayMs: 100,
      onStatusChange: status => statuses.push(status),
    })
    expect(MockSocket.instances[0].url).toBe('ws://fastplan.test/ws')
    expect(connection.status()).toBe('connecting')
    MockSocket.instances[0].open()
    expect(connection.status()).toBe('connected')
    MockSocket.instances[0].disconnect()
    expect(connection.status()).toBe('disconnected')
    vi.advanceTimersByTime(100)
    expect(MockSocket.instances).toHaveLength(2)
    expect(statuses).toEqual(['connecting', 'connected', 'disconnected', 'connecting'])
  })

  it('closes the probe and cancels reconnect when disposed', () => {
    const connection = createFastPlanServiceConnection({
      WebSocket: MockSocket as any,
      reconnectDelayMs: 100,
    })
    const socket = MockSocket.instances[0]
    connection.dispose()
    expect(socket.close).toHaveBeenCalledOnce()
    vi.advanceTimersByTime(100)
    expect(MockSocket.instances).toHaveLength(1)
  })

  it('stops reconnecting at the legacy attempt limit', () => {
    createFastPlanServiceConnection({
      WebSocket: MockSocket as any,
      reconnectDelayMs: 100,
      maxReconnectAttempts: 2,
    })
    MockSocket.instances[0].disconnect()
    vi.advanceTimersByTime(100)
    MockSocket.instances[1].disconnect()
    vi.advanceTimersByTime(100)
    MockSocket.instances[2].disconnect()
    vi.advanceTimersByTime(100)
    expect(MockSocket.instances).toHaveLength(3)
    expect(vi.getTimerCount()).toBe(0)
  })
})
