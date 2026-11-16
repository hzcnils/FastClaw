/** Browser-only capture: caller supplies the owning UI lifetime and attachment insertion. */
export async function captureBrowserScreenshot(options: {
  isCurrent: () => boolean
  capture: () => Promise<MediaStream>
  grabFrame: (track: MediaStreamTrack) => Promise<ImageBitmap>
  encode: (bitmap: ImageBitmap) => string
  accept: (dataUrl: string) => void
  onError: (error: unknown) => void
}) {
  let stream: MediaStream | undefined
  let bitmap: ImageBitmap | undefined
  try {
    if (!options.isCurrent()) return
    stream = await options.capture()
    if (!options.isCurrent()) return
    const track = stream.getVideoTracks()[0]
    if (!track) throw new Error('Screenshot stream has no video track')
    bitmap = await options.grabFrame(track)
    if (!options.isCurrent()) return
    const dataUrl = options.encode(bitmap)
    if (options.isCurrent()) options.accept(dataUrl)
  } catch (error) {
    if (options.isCurrent()) options.onError(error)
  } finally {
    // Permission can resolve after switching away. Always stop every track, including on failure.
    for (const track of stream?.getTracks() || []) {
      try { track.stop() } catch { /* Continue releasing the remaining tracks. */ }
    }
    try { bitmap?.close() } catch { /* Already closed or unavailable. */ }
  }
}
