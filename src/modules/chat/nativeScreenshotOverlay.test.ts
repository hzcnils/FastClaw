import { describe, expect, it, vi } from 'vitest'
import { createNativeScreenshotOverlay } from './nativeScreenshotOverlay'

function data(id: string) { return { request_id: id, width: 1, height: 1, offset_x: 0, offset_y: 0, monitors: [], windows: [], monitorImages: [] } }
function fixture() {
  const ports = { take: vi.fn(async (id: string) => data(id)), show: vi.fn(async () => {}), hide: vi.fn(async () => {}), reply: vi.fn(async () => {}) }
  return { ports, overlay: createNativeScreenshotOverlay(ports) }
}
describe('native overlay request lifetime', () => {
  it('does not take prewarm pixels; takes only an explicit request and hides before replying', async () => {
    const { ports, overlay } = fixture(); expect(ports.take).not.toHaveBeenCalled()
    await overlay.run('one'); expect(overlay.data.value?.request_id).toBe('one')
    await overlay.events.value.confirm('image')
    expect(ports.reply).toHaveBeenCalledExactlyOnceWith(true, { requestId: 'one', dataUrl: 'image' })
    expect(ports.hide.mock.invocationCallOrder[0]).toBeLessThan(ports.reply.mock.invocationCallOrder[0])
    expect(overlay.data.value).toBeNull()
  })
  it('ignores old child emits after a new request and consumes duplicate terminal actions once', async () => {
    const { ports, overlay } = fixture(); await overlay.run('one'); const old = overlay.events.value
    await Promise.all([old.confirm('first'), old.cancel(), old.confirm('duplicate')])
    expect(ports.reply).toHaveBeenCalledOnce(); await overlay.run('two'); await old.cancel(); await old.confirm('old')
    expect(ports.reply).toHaveBeenCalledOnce(); expect(overlay.data.value?.request_id).toBe('two'); await overlay.cancel()
    expect(ports.reply).toHaveBeenLastCalledWith(false, { requestId: 'two' })
  })
  it('serializes close during pending metadata without showing it late', async () => {
    const { ports, overlay } = fixture(); let resolve!: (value: ReturnType<typeof data>) => void
    ports.take.mockReturnValue(new Promise(done => { resolve = done }))
    const running = overlay.run('one'), closing = overlay.cancel(); resolve(data('one')); await Promise.all([running, closing])
    expect(ports.show).not.toHaveBeenCalled(); expect(ports.reply).toHaveBeenCalledExactlyOnceWith(false, { requestId: 'one' })
  })
  it('waits for in-flight show before hiding on OS close', async () => {
    const { ports, overlay } = fixture(); let resolve!: () => void
    ports.show.mockReturnValue(new Promise(done => { resolve = done }))
    const running = overlay.run('one'); await Promise.resolve(); const closing = overlay.cancel()
    expect(ports.hide).not.toHaveBeenCalled(); resolve(); await Promise.all([running, closing])
    expect(ports.hide).toHaveBeenCalledOnce(); expect(ports.reply).toHaveBeenCalledOnce()
  })
  it.each(['take', 'show', 'identity'] as const)('cancels and hides after %s failure', async stage => {
    const { ports, overlay } = fixture()
    if (stage === 'take') ports.take.mockRejectedValue(new Error('failed'))
    if (stage === 'show') ports.show.mockRejectedValue(new Error('failed'))
    if (stage === 'identity') ports.take.mockResolvedValue(data('other'))
    await overlay.run('one'); expect(ports.reply).toHaveBeenCalledExactlyOnceWith(false, { requestId: 'one' })
    expect(ports.hide).toHaveBeenCalledOnce(); expect(overlay.data.value).toBeNull()
  })
  it('still replies if hiding fails and prevents work after disposal', async () => {
    const { ports, overlay } = fixture(); await overlay.run('one'); ports.hide.mockRejectedValue(new Error('hide failed'))
    await expect(overlay.dispose()).rejects.toThrow('hide failed')
    expect(ports.reply).toHaveBeenCalledExactlyOnceWith(false, { requestId: 'one' })
    await overlay.run('two'); expect(ports.take).toHaveBeenCalledOnce()
  })
})
