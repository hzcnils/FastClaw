import { describe, expect, it, vi } from 'vitest'
import { createKgServiceCoordinator } from './kgServiceCoordinator'

describe('createKgServiceCoordinator', () => {
  it('deduplicates concurrent health checks', async () => {
    let resolveProbe!: (ready: boolean) => void
    const probe = vi.fn(() => new Promise<boolean>((resolve) => {
      resolveProbe = resolve
    }))
    const coordinator = createKgServiceCoordinator()

    const first = coordinator.check(probe)
    const second = coordinator.check(probe)
    expect(probe).toHaveBeenCalledTimes(1)

    resolveProbe(true)
    await expect(Promise.all([first, second])).resolves.toEqual([true, true])
    expect(coordinator.state.value).toBe('ready')
  })

  it('reuses a fresh ready result without another probe', async () => {
    const probe = vi.fn().mockResolvedValue(true)
    const coordinator = createKgServiceCoordinator(30_000)

    await expect(coordinator.check(probe)).resolves.toBe(true)
    await expect(coordinator.check(probe, false)).resolves.toBe(true)
    expect(probe).toHaveBeenCalledTimes(1)
  })

  it('deduplicates concurrent ensure operations', async () => {
    let resolveEnsure!: (ready: boolean) => void
    const task = vi.fn(() => new Promise<boolean>((resolve) => {
      resolveEnsure = resolve
    }))
    const coordinator = createKgServiceCoordinator()

    const first = coordinator.ensure(task)
    const second = coordinator.ensure(task)
    expect(task).toHaveBeenCalledTimes(1)

    resolveEnsure(false)
    await expect(Promise.all([first, second])).resolves.toEqual([false, false])
    expect(coordinator.state.value).toBe('unavailable')
  })
})
