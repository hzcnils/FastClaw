import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fastPlanRequest, publicFastPlanModel, type FastPlanAccess } from './access'
import { createFastPlanWebSocketTransport } from './transport'
import { createFastPlanExecutionController } from './executionController'
import { createFastPlanServiceConnection } from './serviceConnection'
import { FastPlanFetchEventSource } from './fetchEventSource'
import { uploadFastPlanFile } from './upload'
import { createFastPlanClient } from './client'

class Socket {
  static OPEN = 1; static CONNECTING = 0; static instances: Socket[] = []
  readyState = 0; sent: any[] = []; onopen: any; onmessage: any; onclose: any; onerror: any
  constructor(public url: string) { Socket.instances.push(this) }
  open() { this.readyState = 1; this.onopen?.({}) }
  message(value: any) { this.onmessage?.({ data: JSON.stringify(value) }) }
  send(text: string) { this.sent.push(JSON.parse(text)) }
  close(code = 1000) { this.readyState = 3; this.onclose?.({ code }) }
}
const connection = (token = 'parent-secret') => ({ wsBase: 'ws://localhost:8080', apiBase: 'http://localhost:8080', parentToken: token })
const model = (key = 'model-secret') => ({ provider: 'openai_compatible', model: 'test-model', base_url: 'http://models.test/v1', auth_type: 'bearer' as const, api_key: key })
const access = (): FastPlanAccess => ({ getConnection: vi.fn(async () => connection()), resolveModel: vi.fn(async () => model()) })
const flush = async () => { for (let i = 0; i < 10; i++) await Promise.resolve() }

describe('managed FastPlan access', () => {
  beforeEach(() => { vi.useFakeTimers(); Socket.instances = [] })
  afterEach(() => { vi.clearAllTimers(); vi.useRealTimers() })
  it('blocks send, events and heartbeat until authentication acknowledgement', async () => {
    const message = vi.fn(); const changed = vi.fn()
    const transport = createFastPlanWebSocketTransport({ access: access(), WebSocket: Socket as any, onMessage: message, onConnectionChange: changed, heartbeatMs: 100 })
    transport.connect('s'); await flush(); const socket = Socket.instances[0]; socket.open()
    expect(socket.sent).toEqual([{ type: 'authenticate', token: 'parent-secret' }])
    expect(transport.send('s', { type: 'chat' })).toBe(false)
    vi.advanceTimersByTime(100)
    expect(socket.sent).toHaveLength(1)
    socket.message({ type: 'authenticated' })
    expect(changed).toHaveBeenLastCalledWith('s', true, expect.objectContaining({ status: 'connected' }))
    expect(message).not.toHaveBeenCalled()
    expect(transport.send('s', { type: 'chat' })).toBe(true)
    transport.closeAll()
  })
  it.each(['timeout', 'wrong-frame'])('closes on %s before exposing history', async reason => {
    const message = vi.fn()
    const transport = createFastPlanWebSocketTransport({ access: access(), WebSocket: Socket as any, onMessage: message })
    transport.connect('s'); await flush(); const socket = Socket.instances[0]; socket.open()
    if (reason === 'timeout') vi.advanceTimersByTime(5000)
    else socket.message({ type: 'history', messages: ['private'] })
    expect(socket.readyState).toBe(3); expect(message).not.toHaveBeenCalled(); expect(transport.has('s')).toBe(false)
  })
  it('authenticates again with the latest token and never replays chat', async () => {
    const host = access()
    const transport = createFastPlanWebSocketTransport({ access: host, WebSocket: Socket as any, onMessage: () => {}, shouldReconnect: () => true, baseReconnectDelayMs: 10 })
    transport.connect('s'); await flush(); const first = Socket.instances[0]; first.open(); first.message({ type: 'authenticated' })
    transport.send('s', { type: 'chat', content: 'once' })
    host.getConnection = vi.fn(async () => connection('new-parent-secret'))
    first.close(); await vi.advanceTimersByTimeAsync(10)
    const second = Socket.instances[1]; second.open(); second.message({ type: 'authenticated' })
    expect(second.sent).toEqual([{ type: 'authenticate', token: 'new-parent-secret' }]); transport.closeAll()
  })
  it('does not open a socket after a pending connection has been disposed', async () => {
    let resolve: any
    const host = access(); host.getConnection = () => new Promise(done => { resolve = done })
    const transport = createFastPlanWebSocketTransport({ access: host, WebSocket: Socket as any, onMessage: () => {} })
    transport.connect('s'); transport.closeAll(); resolve(connection()); await flush()
    expect(Socket.instances).toHaveLength(0)
  })
  it('authenticates the global welcome probe too', async () => {
    const probe = createFastPlanServiceConnection({ access: access(), WebSocket: Socket as any })
    await flush(); const socket = Socket.instances[0]; socket.open()
    expect(socket.url).toBe('ws://localhost:8080/ws'); expect(probe.status()).toBe('connecting')
    socket.message({ type: 'authenticated' }); expect(probe.status()).toBe('connected'); probe.dispose()
  })
  it('adds only a header to HTTP and refuses another origin or redirects', async () => {
    const request = vi.fn(async () => new Response('{}'))
    await fastPlanRequest('/api/sessions', {}, access(), request)
    const [url, init] = request.mock.calls[0] as any
    expect(url).toBe('http://localhost:8080/api/sessions')
    expect(init.headers.get('Authorization')).toBe('Bearer parent-secret'); expect(init.redirect).toBe('error')
    await createFastPlanClient({ apiBase: 'http://previous-instance.test', access: access(), fetch: request }).listSessions()
    expect((request.mock.calls[1] as any)[0]).toBe('http://localhost:8080/api/sessions')
    await expect(fastPlanRequest('http://other.test/api/sessions', {}, access(), request)).rejects.toThrow('不一致')
    expect(request).toHaveBeenCalledTimes(2)
  })
  it('sends fresh model config without credentials in events, state or saved messages', async () => {
    const host = access(); const emitted: any[] = []; const requests: any[] = []
    const request = vi.fn(async (url: any, init: any) => {
      requests.push({ url, body: init?.body })
      if (String(url).includes('/events')) return new Promise<Response>(() => {})
      return new Response('{}')
    })
    const controller = createFastPlanExecutionController({ sessionId: 's', apiBase: 'http://localhost:8080', access: host, WebSocket: Socket as any, fetch: request, onEvent: (name, payload) => emitted.push({ name, payload }) })
    controller.connect(); await flush(); const socket = Socket.instances[0]; socket.open(); socket.message({ type: 'authenticated' })
    await controller.sendChat('first', { model: { id: 'db-1', model: 'display' }, requestId: 'r1' })
    host.resolveModel = vi.fn(async () => model('second-key'))
    await controller.sendChat('second', { model: { id: 'db-2' }, requestId: 'r2' }); await flush()
    expect(socket.sent[1]).toMatchObject({ request_id: 'r1', model_config: model() })
    expect(socket.sent[1]).not.toHaveProperty('base_url'); expect(socket.sent[1]).not.toHaveProperty('model')
    expect(socket.sent[2].model_config.api_key).toBe('second-key')
    const publicData = JSON.stringify([emitted, controller.state, requests])
    for (const secret of ['model-secret', 'second-key', 'parent-secret', 'model_config']) expect(publicData).not.toContain(secret)
    controller.disconnect()
  })
  it('rejects missing selection without sending chat or falling back to defaults', async () => {
    const host = access(); host.resolveModel = async () => { throw new Error('missing model') }
    const controller = createFastPlanExecutionController({ sessionId: 's', access: host })
    await expect(controller.sendChat('task')).rejects.toMatchObject({ message: 'missing model' })
    expect(controller.state.isPlanning).toBe(false); expect(Socket.instances).toHaveLength(0)
  })
  it('keeps only model identification and display metadata', () => {
    expect(publicFastPlanModel({ id: 'm', model: 'name', apiKey: 'key', api_key: 'key', baseUrl: 'http://user:pass@host', parentToken: 'token' })).toEqual({ id: 'm', model: 'name' })
  })
  it('parses authenticated SSE with split CRLF and multiline data and stops on close', async () => {
    const bytes = new TextEncoder()
    const stream = new ReadableStream({ start(controller) {
      for (const chunk of ['event: thinking\r', '\ndata: first\r\ndata: second\r', '\n\r', '\n']) controller.enqueue(bytes.encode(chunk))
      controller.close()
    } })
    const request = vi.fn(async () => new Response(stream, { headers: { 'Content-Type': 'text/event-stream' } }))
    const source = new FastPlanFetchEventSource('/api/sessions/s/events', access(), request)
    const received: string[] = []
    source.addEventListener('thinking', event => received.push((event as MessageEvent).data))
    await flush(); await flush()
    expect(received).toEqual(['first\nsecond'])
    expect((request.mock.calls[0] as any)[1].headers.get('Authorization')).toBe('Bearer parent-secret')
    source.close(); await vi.advanceTimersByTimeAsync(4000); expect(request).toHaveBeenCalledTimes(1)
  })
  it('uploads with a header and cancels even while resolving the token', async () => {
    class Xhr {
      upload: any = {}; onload: any; onerror: any; onabort: any; status = 200; responseText = '{"path":"/tmp/file"}'
      headers: any = {}; sends = 0; url = ''
      open(_method: string, url: string) { this.url = url }
      setRequestHeader(key: string, value: string) { this.headers[key] = value }
      send() { this.sends++; this.onload?.() }
      abort() { this.onabort?.() }
    }
    const first = uploadFastPlanFile(new File(['text'], 'file'), { access: access(), XMLHttpRequest: Xhr as any })
    await first.promise
    expect((first.xhr as any).headers.Authorization).toBe('Bearer parent-secret')
    let resolve: any; const host = access(); host.getConnection = () => new Promise(done => { resolve = done })
    const second = uploadFastPlanFile(new File(['text'], 'file'), { access: host, XMLHttpRequest: Xhr as any })
    const rejected = expect(second.promise).rejects.toThrow('取消')
    second.xhr.abort(); resolve(connection()); await rejected; await flush()
    expect((second.xhr as any).sends).toBe(0)
  })
})
