import { beforeEach, expect, it, vi } from 'vitest'
import { createFastPlanSseTransport } from './sseTransport'

class MockEventSource {
  static instances: MockEventSource[] = []
  readyState = 0
  listeners = new Map<string, Set<(event: any) => void>>()
  close = vi.fn(() => { this.readyState = 2 })

  constructor(public url: string) {
    MockEventSource.instances.push(this)
  }

  addEventListener(type: string, listener: (event: any) => void) {
    const listeners = this.listeners.get(type) || new Set()
    listeners.add(listener)
    this.listeners.set(type, listeners)
  }

  emit(type: string, event: any = {}) {
    for (const listener of this.listeners.get(type) || []) listener(event)
  }
}

beforeEach(() => {
  MockEventSource.instances = []
})

it('shares one physical SSE per scope/session and releases it by reference count', () => {
  const scope = {}
  const firstMessage = vi.fn()
  const secondMessage = vi.fn()
  const first = createFastPlanSseTransport({ EventSource: MockEventSource as any, poolScope: scope })
  const second = createFastPlanSseTransport({ EventSource: MockEventSource as any, poolScope: scope })

  first.open('session-1').addEventListener('message', firstMessage)
  second.open('session-1').addEventListener('message', secondMessage)

  expect(MockEventSource.instances).toHaveLength(1)
  MockEventSource.instances[0].emit('message', { data: '{"type":"thinking"}' })
  expect(firstMessage).toHaveBeenCalledTimes(1)
  expect(secondMessage).toHaveBeenCalledTimes(1)

  first.close()
  expect(MockEventSource.instances[0].close).not.toHaveBeenCalled()
  second.close()
  expect(MockEventSource.instances[0].close).toHaveBeenCalledTimes(1)
})
