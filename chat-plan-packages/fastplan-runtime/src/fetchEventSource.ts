import { fastPlanRequest, type FastPlanAccess } from './access'

/** Minimal EventSource-compatible fetch stream, with header auth and abortable reconnect. */
export class FastPlanFetchEventSource extends EventTarget {
  readyState = 0
  private controller = new AbortController()
  private retry?: ReturnType<typeof setTimeout>
  private lastId = ''
  constructor(private url: string, private access: FastPlanAccess, private request?: typeof fetch) {
    super()
    void this.connect()
  }
  close() { this.readyState = 2; clearTimeout(this.retry); this.controller.abort() }
  private async connect() {
    try {
      const response = await fastPlanRequest(this.url, { signal: this.controller.signal,
        headers: { Accept: 'text/event-stream', ...(this.lastId ? { 'Last-Event-ID': this.lastId } : {}) } }, this.access, this.request)
      if (this.readyState === 2) return
      if (!response.ok || !response.body || !response.headers.get('Content-Type')?.includes('text/event-stream')) throw new Error('SSE unavailable')
      this.readyState = 1; this.dispatchEvent(new Event('open'))
      const reader = response.body.getReader()
      const decoder = new TextDecoder()
      let buffer = ''; let data: string[] = []; let name = ''; let skipLf = false
      const line = (value: string) => {
        if (!value) {
          if (data.length) this.dispatchEvent(new MessageEvent(name || 'message', { data: data.join('\n'), lastEventId: this.lastId }))
          data = []; name = ''; return
        }
        const colon = value.indexOf(':')
        const field = colon < 0 ? value : value.slice(0, colon)
        const content = colon < 0 ? '' : value.slice(colon + 1).replace(/^ /, '')
        if (field === 'data') data.push(content)
        if (field === 'event') name = content
        if (field === 'id' && !content.includes('\0')) this.lastId = content
      }
      try {
        while (!this.controller.signal.aborted) {
          const chunk = await reader.read()
          if (chunk.done) break
          for (const char of decoder.decode(chunk.value, { stream: true })) {
            if (skipLf && char === '\n') { skipLf = false; continue }
            skipLf = false
            if (char === '\r' || char === '\n') { line(buffer); buffer = ''; skipLf = char === '\r' }
            else buffer += char
          }
        }
      } finally { await reader.cancel().catch(() => {}); reader.releaseLock() }
    } catch { /* Only a generic event is exposed; never include request headers. */ }
    if (this.controller.signal.aborted) return
    this.readyState = 0; this.dispatchEvent(new Event('error'))
    this.retry = setTimeout(() => { void this.connect() }, 3000)
  }
}
