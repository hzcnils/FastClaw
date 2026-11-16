import { afterEach, describe, expect, it, vi } from 'vitest'
import { startNativeScreenshot } from './nativeScreenshot'
import { beginChatScreenshotRequest, disposeChatScreenshotEventBridge } from '@/utils/chatScreenshotEventBridge'
import type { CaptureMetadata } from '@/types/screenshot'

const handlers = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>())
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async (event: string, fn: (event: { payload: unknown }) => void) => {
  handlers.set(event, fn); return vi.fn()
}) }))
afterEach(async () => { await disposeChatScreenshotEventBridge(); handlers.clear(); vi.clearAllMocks() })
const metadata = (id: string): CaptureMetadata => ({ request_id: id, width: 10, height: 10, offset_x: 0, offset_y: 0, monitors: [], windows: [] })
function options() {
  return { isCurrent: vi.fn(() => true), prepareWindow: vi.fn(async () => true), capture: vi.fn(async (id: string, _inline: boolean) => metadata(id)),
    runWindow: vi.fn(async (_id: string) => {}), showInline: vi.fn(async (_meta: CaptureMetadata, _confirm: (url: string) => Promise<void>, _cancel: () => Promise<void>) => true),
    confirm: vi.fn(), cleanup: vi.fn(async (_id: string) => {}), onError: vi.fn() }
}
async function settle(id: string, url?: string) {
  handlers.get(url ? 'screenshot-confirm' : 'screenshot-cancel')!({ payload: { requestId: id, ...(url ? { dataUrl: url } : {}) } })
  for (let i = 0; i < 10; i++) await Promise.resolve()
}
describe('native screenshot request workflow', () => {
  it('waits for readiness, captures once and echoes the same ID through the window', async () => {
    const o = options(); await startNativeScreenshot(o)
    const id = o.capture.mock.calls[0][0]
    expect(o.prepareWindow.mock.invocationCallOrder[0]).toBeLessThan(o.capture.mock.invocationCallOrder[0])
    expect(o.capture).toHaveBeenCalledWith(id, false); expect(o.runWindow).toHaveBeenCalledExactlyOnceWith(id)
    expect(o.showInline).not.toHaveBeenCalled(); await settle(id, 'data'); expect(o.confirm).toHaveBeenCalledWith('data')
    expect(o.cleanup).toHaveBeenCalledExactlyOnceWith(id)
  })
  it('does not overwrite shared pending pixels with concurrent requests', async () => {
    const first = options(), second = options(); await startNativeScreenshot(first)
    expect(await startNativeScreenshot(second)).toBe('busy'); expect(second.capture).not.toHaveBeenCalled()
    await settle(first.capture.mock.calls[0][0])
  })
  it('cleans up even when source UI is invalidated before confirm', async () => {
    const o = options(); await startNativeScreenshot(o); o.isCurrent.mockReturnValue(false)
    await settle(o.capture.mock.calls[0][0], 'late'); expect(o.confirm).not.toHaveBeenCalled(); expect(o.cleanup).toHaveBeenCalledOnce()
  })
  it('captures an inline fallback only after window preparation fails, and retains captured handlers', async () => {
    const o = options(); o.prepareWindow.mockResolvedValue(false); await startNativeScreenshot(o)
    const id = o.capture.mock.calls[0][0]; expect(o.capture).toHaveBeenCalledWith(id, true); expect(o.runWindow).not.toHaveBeenCalled()
    await o.showInline.mock.calls[0][1]('inline'); expect(o.confirm).toHaveBeenCalledWith('inline'); expect(o.cleanup).toHaveBeenCalledWith(id)
    await o.showInline.mock.calls[0][2](); expect(o.cleanup).toHaveBeenCalledOnce()
  })
  it('settles a fallback that became invisible while fetching pixels', async () => {
    const o = options(); o.prepareWindow.mockResolvedValue(false); o.showInline.mockResolvedValue(false)
    await startNativeScreenshot(o); expect(o.cleanup).toHaveBeenCalledOnce(); expect(o.confirm).not.toHaveBeenCalled()
  })
  it.each(['prepare', 'capture', 'run', 'inline', 'identity'] as const)('restores and releases after %s failure', async stage => {
    const o = options(), error = new Error('failed')
    if (stage === 'prepare') o.prepareWindow.mockRejectedValue(error)
    if (stage === 'capture') o.capture.mockRejectedValue(error)
    if (stage === 'run') o.runWindow.mockRejectedValue(error)
    if (stage === 'inline') { o.prepareWindow.mockResolvedValue(false); o.showInline.mockRejectedValue(error) }
    if (stage === 'identity') o.capture.mockResolvedValue(metadata('wrong'))
    await startNativeScreenshot(o); expect(o.cleanup).toHaveBeenCalledOnce(); expect(o.onError).toHaveBeenCalledOnce()
    expect(beginChatScreenshotRequest({ confirm: vi.fn(), cleanup: vi.fn() })).not.toBeNull()
  })
  it('does not display a capture that became stale while the backend was running', async () => {
    const o = options(); o.capture.mockImplementation(async id => { o.isCurrent.mockReturnValue(false); return metadata(id) })
    await startNativeScreenshot(o); expect(o.runWindow).not.toHaveBeenCalled(); expect(o.cleanup).toHaveBeenCalledOnce()
  })
  it('root disposal waits for an in-flight inline setup before cleanup and unlocking', async () => {
    const o = options(); o.prepareWindow.mockResolvedValue(false)
    let finish!: (shown: boolean) => void
    o.showInline.mockReturnValue(new Promise(resolve => { finish = resolve }))
    const startup = startNativeScreenshot(o)
    await vi.waitFor(() => expect(o.showInline).toHaveBeenCalledOnce())
    const disposing = disposeChatScreenshotEventBridge()
    await Promise.resolve(); expect(o.cleanup).not.toHaveBeenCalled()
    expect(beginChatScreenshotRequest({ confirm: vi.fn(), cleanup: vi.fn() })).toBeNull()
    finish(true); await Promise.all([startup, disposing]); expect(o.cleanup).toHaveBeenCalledOnce()
  })
})
