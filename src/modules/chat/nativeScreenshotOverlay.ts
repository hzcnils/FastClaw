import { computed, shallowRef } from 'vue'
import type { CaptureMetadata, ScreenshotReply } from '@/types/screenshot'

type OverlayData = CaptureMetadata & { monitorImages: string[] }
export function createNativeScreenshotOverlay(ports: {
  take: (requestId: string) => Promise<OverlayData>
  show: (data: OverlayData) => Promise<void>
  hide: () => Promise<void>
  reply: (confirm: boolean, payload: ScreenshotReply) => Promise<void>
}) {
  const data = shallowRef<OverlayData | null>(null)
  type Frame = { id: string; loading: Promise<void>; finishing?: Promise<void>; cancelled: boolean }
  const active = shallowRef<Frame | null>(null)
  let disposed = false
  async function finish(frame: Frame, dataUrl?: string) {
    if (active.value !== frame) return
    if (frame.finishing) return frame.finishing
    frame.cancelled = true
    frame.finishing = (async () => {
      await frame.loading
      data.value = null
      // Hide must precede terminal reply, which releases the main-window request lock.
      try { await ports.hide() }
      finally {
        try { await ports.reply(dataUrl !== undefined, { requestId: frame.id, ...(dataUrl !== undefined ? { dataUrl } : {}) }) }
        finally { if (active.value === frame) active.value = null }
      }
    })()
    return frame.finishing
  }
  async function run(requestId: string) {
    if (disposed || active.value || !requestId) return
    const frame: Frame = { id: requestId, loading: Promise.resolve(), cancelled: false }
    active.value = frame
    let failed = false
    frame.loading = (async () => {
      try {
        const result = await ports.take(requestId)
        if (result.request_id !== requestId) throw new Error('Screenshot request identity mismatch')
        if (frame.cancelled || disposed) return
        data.value = result
        await ports.show(result)
      } catch { failed = true }
    })()
    await frame.loading
    if (failed || disposed) await finish(frame)
  }
  const events = computed(() => {
    const frame = active.value
    return { confirm: (url: string) => frame ? finish(frame, url) : Promise.resolve(),
      cancel: () => frame ? finish(frame) : Promise.resolve() }
  })
  async function cancel() { if (active.value) await finish(active.value); else await ports.hide() }
  async function dispose() { disposed = true; await cancel() }
  return { data, events, run, cancel, dispose }
}
