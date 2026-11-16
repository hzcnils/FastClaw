import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createFastPlanWebSocketTransport } from './transport'

class MockSocket {
  static OPEN = 1
  static CONNECTING = 0
  static instances: MockSocket[] = []

  readyState = MockSocket.CONNECTING
  onopen: ((event: unknown) => void) | null = null
  onmessage: ((event: unknown) => void) | null = null
  onerror: ((event: unknown) => void) | null = null
  onclose: ((event: { code: number }) => void) | null = null

  constructor(public url: string) {
    MockSocket.instances.push(this)
  }

  open() {
    this.readyState = MockSocket.OPEN
    this.onopen?.({})
  }

  disconnect(code = 1006) {
    this.readyState = 3
    this.onclose?.({ code })
  }

  send() {}
  close() { this.readyState = 3 }
}

describe('FastPlan WebSocket reconnect timing', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    MockSocket.instances = []
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('reconnects after the configured backoff and resets attempts on success', () => {
    const changes: any[] = []
    const transport = createFastPlanWebSocketTransport({
      wsBase: 'ws://fastplan.test',
      WebSocket: MockSocket as any,
      maxReconnectAttempts: 3,
      baseReconnectDelayMs: 100,
      shouldReconnect: () => true,
      onMessage: () => {},
      onConnectionChange: (_sessionId, _connected, detail) => changes.push(detail),
    })

    transport.connect('session-1')
    MockSocket.instances[0].disconnect()
    expect(changes[changes.length - 1]).toMatchObject({ status: 'reconnecting', attempt: 1 })

    vi.advanceTimersByTime(99)
    expect(MockSocket.instances).toHaveLength(1)
    vi.advanceTimersByTime(1)
    expect(MockSocket.instances).toHaveLength(2)

    MockSocket.instances[1].open()
    expect(changes[changes.length - 1]).toMatchObject({ status: 'connected' })
    MockSocket.instances[1].disconnect()
    expect(changes[changes.length - 1]).toMatchObject({ status: 'reconnecting', attempt: 1 })
  })

  it('reports failure only after all configured reconnect attempts are exhausted', () => {
    const changes: any[] = []
    const transport = createFastPlanWebSocketTransport({
      wsBase: 'ws://fastplan.test',
      WebSocket: MockSocket as any,
      maxReconnectAttempts: 2,
      baseReconnectDelayMs: 100,
      shouldReconnect: () => true,
      onMessage: () => {},
      onConnectionChange: (_sessionId, _connected, detail) => changes.push(detail),
    })

    transport.connect('session-1')
    MockSocket.instances[0].disconnect()
    vi.advanceTimersByTime(100)
    MockSocket.instances[1].disconnect()
    vi.advanceTimersByTime(150)
    MockSocket.instances[2].disconnect()

    expect(MockSocket.instances).toHaveLength(3)
    expect(changes[changes.length - 1]).toMatchObject({ status: 'failed', attempt: 2, maxAttempts: 2 })
    expect(vi.getTimerCount()).toBe(0)
  })

  it('shares one managed socket per access/session and releases it by reference count', async () => {
    const access = {
      getConnection: vi.fn(async () => ({ wsBase: 'ws://fastplan.test', apiBase: 'http://fastplan.test', parentToken: 'token' })),
      resolveModel: vi.fn(),
    }
    const first = createFastPlanWebSocketTransport({ access: access as any, WebSocket: MockSocket as any, onMessage: () => {} })
    const second = createFastPlanWebSocketTransport({ access: access as any, WebSocket: MockSocket as any, onMessage: () => {} })

    first.connect('shared-session')
    second.connect('shared-session')
    await Promise.resolve()
    await Promise.resolve()
    expect(MockSocket.instances).toHaveLength(1)

    MockSocket.instances[0].open()
    first.close('shared-session')
    expect(MockSocket.instances[0].readyState).toBe(MockSocket.OPEN)
    second.close('shared-session')
    expect(MockSocket.instances[0].readyState).toBe(3)
  })
})
