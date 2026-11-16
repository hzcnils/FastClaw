import { beforeEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

beforeEach(() => {
  vi.resetModules()
  vi.restoreAllMocks()
  invoke.mockReset().mockResolvedValue(undefined)
  localStorage.clear()
})

describe('FastPlan early resume bootstrap', () => {
  it('uploads saved credentials immediately and coalesces entry/mounted calls', async () => {
    const { provideFastPlanResume, FASTPLAN_RESUME_KEY } = await import('./fastPlanResume')
    const data = { pid: 123, process_started_at: 456, parent_token: 'test-only-token' }
    localStorage.setItem(FASTPLAN_RESUME_KEY, JSON.stringify(data))
    const first = provideFastPlanResume()
    expect(invoke).toHaveBeenCalledWith('provide_fastplan_resume_data', { data })
    expect(provideFastPlanResume()).toBe(first)
    await first
    await provideFastPlanResume()
    expect(invoke).toHaveBeenCalledTimes(1)
  })

  it.each(['missing', 'malformed', 'unavailable'])('releases the startup gate when storage is %s', async (mode) => {
    const { provideFastPlanResume, FASTPLAN_RESUME_KEY } = await import('./fastPlanResume')
    if (mode === 'malformed') localStorage.setItem(FASTPLAN_RESUME_KEY, '{')
    if (mode === 'unavailable') vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => { throw new Error('denied') })
    await provideFastPlanResume()
    expect(invoke).toHaveBeenCalledExactlyOnceWith('provide_fastplan_resume_data', { data: null })
  })

  it('falls back to an empty resume when the saved data is rejected', async () => {
    const { provideFastPlanResume } = await import('./fastPlanResume')
    invoke.mockRejectedValueOnce(new Error('invalid data'))
    await provideFastPlanResume()
    expect(invoke).toHaveBeenCalledTimes(2)
    expect(invoke).toHaveBeenLastCalledWith('provide_fastplan_resume_data', { data: null })
  })

  it('allows a later mounted call to retry after IPC was unavailable', async () => {
    const { provideFastPlanResume } = await import('./fastPlanResume')
    invoke.mockRejectedValueOnce(new Error('not ready')).mockRejectedValueOnce(new Error('not ready'))
    await provideFastPlanResume()
    await provideFastPlanResume()
    expect(invoke).toHaveBeenCalledTimes(3)
  })
})
