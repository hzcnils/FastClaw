import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useFastPlanServiceStatusStore } from './fastPlanServiceStatus'

describe('fastPlanServiceStatus', () => {
  beforeEach(() => setActivePinia(createPinia()))

  it('shares one readiness probe between concurrent tab requests', async () => {
    let resolve!: () => void
    const probe = vi.fn(() => new Promise<void>(done => { resolve = done }))
    const store = useFastPlanServiceStatusStore()
    const first = store.ensureReady({ probe })
    const second = store.ensureReady({ probe })

    expect(probe).toHaveBeenCalledTimes(1)
    expect(store.status).toBe('checking')
    resolve()
    await expect(Promise.all([first, second])).resolves.toEqual([true, true])
    expect(store.presentationStatus).toBe('connected')
  })

  it('leaves connecting state after a bounded timeout', async () => {
    vi.useFakeTimers()
    const store = useFastPlanServiceStatusStore()
    const result = store.ensureReady({ probe: () => new Promise(() => {}), timeoutMs: 50 })
    await vi.advanceTimersByTimeAsync(50)
    await expect(result).resolves.toBe(false)
    expect(store.status).toBe('failed')
    expect(store.presentationStatus).toBe('disconnected')
    vi.useRealTimers()
  })
})
