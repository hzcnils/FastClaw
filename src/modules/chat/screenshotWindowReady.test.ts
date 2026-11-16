// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
const { events, unlisten, emitMock, existing } = vi.hoisted(() => ({
  events: new Map<string, (event: { payload: unknown }) => void>(), unlisten: vi.fn(),
  emitMock: vi.fn(async (_name: string, _payload: unknown) => {}), existing: vi.fn(async () => ({})),
}))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async (name: string, fn: (event: { payload: unknown }) => void) => { events.set(name, fn); return unlisten }), emit: emitMock }))
vi.mock('@tauri-apps/api/webviewWindow', () => ({ WebviewWindow: { getByLabel: existing } }))
import { ensureScreenshotWindow } from '@/utils/screenshotWindow'
beforeEach(() => { vi.useFakeTimers(); vi.stubGlobal('__TAURI_INTERNALS__', {}); events.clear(); vi.clearAllMocks() })
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals() })
describe('screenshot window readiness handshake', () => {
  it('accepts a matching probe reply and clears the timer/listener', async () => {
    emitMock.mockImplementation(async (_name, token) => { events.get('screenshot-ready')!({ payload: token }) })
    expect(await ensureScreenshotWindow()).toEqual({ existed: true, ok: true })
    expect(unlisten).toHaveBeenCalledOnce(); expect(vi.getTimerCount()).toBe(0)
  })
  it('ignores a stale token and times out into the fallback', async () => {
    emitMock.mockImplementation(async () => { events.get('screenshot-ready')!({ payload: 'old' }) })
    const pending = ensureScreenshotWindow(); await vi.waitFor(() => expect(emitMock).toHaveBeenCalledOnce())
    await vi.advanceTimersByTimeAsync(2000); expect(await pending).toEqual({ existed: true, ok: false })
    expect(unlisten).toHaveBeenCalledOnce(); expect(vi.getTimerCount()).toBe(0)
  })
  it('accepts the cold window ready notification after a probe and rolls back on emit failure', async () => {
    emitMock.mockImplementation(async () => { events.get('screenshot-ready')!({ payload: null }) })
    expect((await ensureScreenshotWindow()).ok).toBe(true)
    emitMock.mockRejectedValueOnce(new Error('failed'))
    expect((await ensureScreenshotWindow()).ok).toBe(false); expect(unlisten).toHaveBeenCalledTimes(2)
    expect(vi.getTimerCount()).toBe(0)
  })
})
