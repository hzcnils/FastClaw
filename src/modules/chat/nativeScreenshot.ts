import type { CaptureMetadata } from '@/types/screenshot'
import { beginChatScreenshotRequest } from '@/utils/chatScreenshotEventBridge'

export async function startNativeScreenshot(options: {
  isCurrent: () => boolean
  prepareWindow: () => Promise<boolean>
  capture: (requestId: string, inline: boolean) => Promise<CaptureMetadata>
  runWindow: (requestId: string) => Promise<void>
  showInline: (metadata: CaptureMetadata, confirm: (dataUrl: string) => Promise<void>, cancel: () => Promise<void>) => Promise<boolean>
  confirm: (dataUrl: string) => void
  cleanup: (requestId: string) => Promise<void>
  onError: (error: unknown) => void
}) {
  if (!options.isCurrent()) return
  let requestId = ''
  let startupInFlight: Promise<void> | undefined
  const request = beginChatScreenshotRequest({
    confirm: dataUrl => { if (options.isCurrent()) options.confirm(dataUrl) },
    async cleanup() {
      // Teardown cannot release shared resources while startup is still capturing/showing.
      await startupInFlight?.catch(() => {})
      await options.cleanup(requestId)
    },
  })
  if (!request) return 'busy' as const
  requestId = request.id
  let handedOff = false
  startupInFlight = (async () => {
    if (!await request.ready()) throw new Error('Screenshot listeners unavailable')
    if (!request.isPending() || !options.isCurrent()) return
    const standalone = await options.prepareWindow()
    if (!request.isPending() || !options.isCurrent()) return
    const metadata = await options.capture(request.id, !standalone)
    if (metadata.request_id !== request.id) throw new Error('Screenshot request identity mismatch')
    if (!request.isPending() || !options.isCurrent()) return
    if (standalone) await options.runWindow(request.id)
    else if (!await options.showInline(metadata, request.confirm, request.cancel)) return
    handedOff = true
  })()
  try {
    await startupInFlight
  } catch (error) {
    if (options.isCurrent()) options.onError(error)
  } finally {
    if (!handedOff) await request.cancel()
  }
}
