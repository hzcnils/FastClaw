import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const { handlers, removers } = vi.hoisted(() => ({
  handlers: new Map<string, (event: { payload: unknown }) => void>(),
  removers: [] as ReturnType<typeof vi.fn>[],
}))
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
    handlers.set(name, handler)
    const remove = vi.fn(); removers.push(remove); return remove
  }),
}))
import { listen } from '@tauri-apps/api/event'
import { beginChatScreenshotRequest, disposeChatScreenshotEventBridge } from './chatScreenshotEventBridge'

const callbacks = () => ({ confirm: vi.fn(), cleanup: vi.fn(async () => {}) })
const flush = async () => { for (let i = 0; i < 8; i++) await Promise.resolve() }
beforeEach(async () => { await disposeChatScreenshotEventBridge(); handlers.clear(); removers.length = 0; vi.clearAllMocks() })
afterEach(async () => { await disposeChatScreenshotEventBridge() })

describe('request-owned screenshot event bridge', () => {
  it('registers only once and keeps the native request exclusive through cleanup', async () => {
    let release!: () => void
    const cleanup = new Promise<void>(resolve => { release = resolve })
    const first = callbacks(); first.cleanup.mockReturnValue(cleanup)
    const a = beginChatScreenshotRequest(first)!; expect(await a.ready()).toBe(true)
    expect(beginChatScreenshotRequest(callbacks())).toBeNull()
    handlers.get('screenshot-confirm')!({ payload: { requestId: a.id, dataUrl: 'image' } }); await flush()
    expect(first.confirm).toHaveBeenCalledExactlyOnceWith('image')
    expect(beginChatScreenshotRequest(callbacks())).toBeNull()
    release(); await a.cancel()
    const b = beginChatScreenshotRequest(callbacks())!; expect(await b.ready()).toBe(true)
    expect(listen).toHaveBeenCalledTimes(2); await b.cancel()
  })
  it.each([undefined, 'legacy image', {}, { requestId: 'other', dataUrl: 'wrong' }, { requestId: 123 }])('ignores malformed or unowned event %j', async payload => {
    const cb = callbacks(), r = beginChatScreenshotRequest(cb)!; await r.ready()
    handlers.get('screenshot-confirm')!({ payload }); handlers.get('screenshot-cancel')!({ payload }); await flush()
    expect(cb.confirm).not.toHaveBeenCalled(); expect(cb.cleanup).not.toHaveBeenCalled()
  })
  it('consumes terminal events once and rejects old IDs after starting a new request', async () => {
    const first = callbacks(), a = beginChatScreenshotRequest(first)!; await a.ready()
    handlers.get('screenshot-confirm')!({ payload: { requestId: a.id, dataUrl: 'one' } })
    handlers.get('screenshot-confirm')!({ payload: { requestId: a.id, dataUrl: 'two' } })
    handlers.get('screenshot-cancel')!({ payload: { requestId: a.id } }); await a.cancel()
    expect(first.confirm).toHaveBeenCalledExactlyOnceWith('one'); expect(first.cleanup).toHaveBeenCalledOnce()
    const second = callbacks(), b = beginChatScreenshotRequest(second)!; await b.ready()
    handlers.get('screenshot-cancel')!({ payload: { requestId: a.id } }); await flush()
    expect(second.cleanup).not.toHaveBeenCalled()
    handlers.get('screenshot-cancel')!({ payload: { requestId: b.id } }); await b.cancel()
    expect(second.confirm).not.toHaveBeenCalled(); expect(second.cleanup).toHaveBeenCalledOnce()
  })
  it('runs cleanup when the captured consumer throws synchronously', async () => {
    const cb = callbacks(); cb.confirm.mockImplementation(() => { throw new Error('render failed') })
    const r = beginChatScreenshotRequest(cb)!; await r.ready()
    await expect(r.confirm('data')).rejects.toThrow('render failed')
    expect(cb.cleanup).toHaveBeenCalledOnce()
    expect(beginChatScreenshotRequest(callbacks())).not.toBeNull()
  })
  it('rolls back partial registration and allows a later retry', async () => {
    vi.mocked(listen).mockRejectedValueOnce(new Error('unavailable'))
    const r = beginChatScreenshotRequest(callbacks())!
    expect(await r.ready()).toBe(false); expect(await r.ready()).toBe(true); await r.cancel()
  })
  it('unlistens only on root disposal and settles an unfinished request', async () => {
    const cb = callbacks(), r = beginChatScreenshotRequest(cb)!; await r.ready()
    expect(removers).toHaveLength(2); removers.forEach(remove => expect(remove).not.toHaveBeenCalled())
    await disposeChatScreenshotEventBridge()
    removers.forEach(remove => expect(remove).toHaveBeenCalledOnce()); expect(cb.cleanup).toHaveBeenCalledOnce()
  })
})
