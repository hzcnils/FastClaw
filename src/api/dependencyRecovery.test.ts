import { afterEach, describe, expect, it, vi } from 'vitest'
const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))
import { waitForDependencyReady } from './dependencyRecovery'

afterEach(() => { vi.useRealTimers(); invoke.mockReset() })
describe('dependency recovery waits for backend state', () => {
  it('waits for a dependency and respects cancellation without starting or replaying requests', async () => {
    vi.useFakeTimers()
    const controller = new AbortController()
    invoke.mockResolvedValue([{ id: 'fastplan', desired_state: 'running', recovery_state: 'starting' }])
    const pending = waitForDependencyReady('fastplan', controller.signal)
    const rejected = expect(pending).rejects.toThrow('已取消等待服务')
    await vi.advanceTimersByTimeAsync(0)
    controller.abort()
    await vi.advanceTimersByTimeAsync(500)
    await rejected
    expect(invoke).toHaveBeenCalledExactlyOnceWith('get_dependency_recovery_status')
  })

  it('waits through backoff until the backend reports healthy', async () => {
    vi.useFakeTimers()
    invoke.mockResolvedValueOnce([{ id: 'fastplan', desired_state: 'running', recovery_state: 'backoff' }])
      .mockResolvedValueOnce([{ id: 'fastplan', desired_state: 'running', recovery_state: 'healthy' }])
    const pending = waitForDependencyReady('fastplan')
    await vi.advanceTimersByTimeAsync(500)
    await expect(pending).resolves.toBeUndefined()
    expect(invoke).toHaveBeenCalledTimes(2)
    expect(invoke).toHaveBeenCalledWith('get_dependency_recovery_status')
  })

  it('does not start services or reset the budget when stopped', async () => {
    invoke.mockResolvedValue([{ id: 'fastkg', desired_state: 'stopped', recovery_state: 'idle' }])
    await expect(waitForDependencyReady('fastkg')).rejects.toThrow('服务已停止')
    expect(invoke).toHaveBeenCalledTimes(1)
  })

  it('surfaces upstream permanent failure instead of waiting forever', async () => {
    invoke.mockResolvedValue([
      { id: 'fastkg', desired_state: 'running', recovery_state: 'waiting_dependency' },
      { id: 'qdrant', desired_state: 'running', recovery_state: 'failed', last_error: '服务文件未安装' },
    ])
    await expect(waitForDependencyReady('fastkg')).rejects.toThrow('服务文件未安装')
  })
})
