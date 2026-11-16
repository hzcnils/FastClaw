// @vitest-environment jsdom
import { createPinia, setActivePinia } from 'pinia'
import { effectScope } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { disposeChatStore, useChatStore, type AnswerChatStore } from './chat'
import { getChatTabState } from '@/modules/chat/tabState'
import type { Message } from '@/types'

const transport = vi.hoisted(() => ({ invoke: vi.fn(), gate: null as Promise<void> | null,
  handlers: new Map<string, Set<(event: { payload: any }) => unknown>>() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: transport.invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async (name, handler) => {
  const set = transport.handlers.get(name) || new Set(); transport.handlers.set(name, set); set.add(handler)
  if (name !== 'remote-session-event' && transport.gate) await transport.gate
  return () => set.delete(handler)
}) }))
type Store = AnswerChatStore & { loadMessages(id: string): Promise<void> }
function deferred<T>() { let resolve!: (value: T) => void, reject!: (error: Error) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no }); return { promise, resolve, reject } }
const emit = async (name: string, payload: any) => { for (const handler of [...(transport.handlers.get(name) || [])]) await handler({ payload }) }
const calls = (command: string) => transport.invoke.mock.calls.filter(([cmd]) => cmd === command)
let sequence = 0
const ids = new Set<string>()
const row = (role: Message['role'], id: string, conversationId: string): Message => ({ id, role, conversation_id: conversationId,
  group_id: 'group', content: role === 'user' ? '原问题' : '旧回答', status: 'completed', model_id: null,
  token_input: null, token_output: null, created_at: '' })
async function tab() {
  const id = `answer-retry-${++sequence}`, conversationId = `conversation-${id}`; ids.add(id)
  const chat = useChatStore(id) as unknown as Store
  await chat.loadMessages(conversationId)
  chat.messages.push(row('user', 'user', conversationId), row('assistant', 'old', conversationId))
  return { id, chat, conversationId, owner: getChatTabState(id) }
}
async function done(conversationId: string, extra: Record<string, unknown> = {}) {
  await emit('chat-stream-done', { conversation_id: conversationId, message_id: 'saved-new', content: '新回答', ...extra })
}

describe('ordinary answer retry uses the source group and a per-stream lease', () => {
  beforeEach(() => {
    setActivePinia(createPinia()); sessionStorage.clear(); transport.gate = null
    transport.invoke.mockReset().mockImplementation(async command => {
      if (command === 'get_messages' || command === 'get_conversations') return []
      if (command === 'save_assistant_message') return 'saved-error'
    })
  })
  afterEach(async () => {
    transport.gate = null
    for (const id of ids) await disposeChatStore(id)
    ids.clear()
    await vi.waitFor(() => expect([...transport.handlers].filter(([name]) => name !== 'remote-session-event')
      .reduce((sum, [, set]) => sum + set.size, 0)).toBe(0))
  })

  it('keeps the old answer until a successful persisted done event, even after caller disposal', async () => {
    const c = await tab(), scope = effectScope()
    await scope.run(() => c.chat.regenerateMessage('old', 'model'))!
    scope.stop()
    expect(calls('delete_message')).toHaveLength(0)
    expect(c.chat.isLoading).toBe(true)
    expect(calls('send_message_stream')[0][1]).toMatchObject({ conversationId: c.conversationId, content: '原问题', groupId: 'group', modelId: 'model', skipUserMessage: true })
    const mirror = await tab(); await mirror.chat.loadMessages(c.conversationId)
    expect(mirror.chat.messages).toBe(c.chat.messages)
    await done(c.conversationId)
    expect(calls('delete_message')).toHaveLength(1)
    expect(calls('delete_message')[0][1]).toEqual({ id: 'old' })
    expect(c.chat.messages.map(message => message.id)).toEqual(['user', 'saved-new'])
    expect(c.chat.isLoading).toBe(false)
    expect(mirror.chat.messages.at(-1)?.content).toBe('新回答')
  })

  it.each(['error', 'stopped', 'missing-id', 'invoke-error', 'close'] as const)('preserves the old answer after %s', async outcome => {
    const c = await tab()
    if (outcome === 'invoke-error') transport.invoke.mockImplementation(async command => {
      if (command === 'send_message_stream') throw new Error('send failed')
      if (command === 'save_assistant_message') return 'saved-error'
    })
    await c.chat.regenerateMessage('old', 'model')
    if (outcome === 'error') await emit('chat-stream-error', { conversation_id: c.conversationId, error: 'failed' })
    if (outcome === 'stopped') await done(c.conversationId, { stopped: true })
    if (outcome === 'missing-id') await done(c.conversationId, { message_id: undefined })
    if (outcome === 'close') { await disposeChatStore(c.id); await done(c.conversationId) }
    expect(calls('delete_message')).toHaveLength(0)
    expect(c.chat.messages.some(message => message.id === 'old')).toBe(true)
    expect(c.chat.isLoading).toBe(false)
    if (outcome === 'stopped') expect(c.chat.messages.at(-1)?.status).toBe('canceled')
    if (outcome === 'missing-id') expect(c.chat.messages.at(-1)?.id).toBeTruthy()
  })

  it('retains both successful answers if deleting the old row fails, without repeating generation', async () => {
    const c = await tab()
    transport.invoke.mockImplementation(async command => { if (command === 'delete_message') throw new Error('disk error') })
    await c.chat.regenerateMessage('old', 'model'); await done(c.conversationId)
    expect(c.chat.messages.map(message => message.id)).toEqual(['user', 'old', 'saved-new'])
    expect(c.owner.state.chatSubmissionError.value).toContain('旧回答清理失败')
    expect(calls('send_message_stream')).toHaveLength(1)
  })

  it('does not let a late send rejection overwrite a successful terminal awaiting replacement cleanup', async () => {
    const c = await tab(), sending = deferred<void>(), deleting = deferred<void>()
    transport.invoke.mockImplementation(async command => command === 'send_message_stream' ? sending.promise
      : command === 'delete_message' ? deleting.promise : undefined)
    const task = c.chat.regenerateMessage('old', 'model')
    await vi.waitFor(() => expect(calls('send_message_stream')).toHaveLength(1))
    const completing = done(c.conversationId)
    await vi.waitFor(() => expect(calls('delete_message')).toHaveLength(1))
    sending.reject(new Error('late IPC failure')); await task
    expect(calls('save_assistant_message')).toHaveLength(0)
    expect(c.chat.messages.at(-1)).toMatchObject({ status: 'completed', content: '新回答' })
    deleting.resolve(); await completing
    expect(c.chat.messages.map(message => message.id)).toEqual(['user', 'saved-new'])
    expect(c.chat.isLoading).toBe(false)
  })

  it('persists only one terminal when duplicate error callbacks overlap', async () => {
    const c = await tab(), persistence = deferred<string>()
    transport.invoke.mockImplementation(async command => command === 'save_assistant_message' ? persistence.promise : undefined)
    await c.chat.regenerateMessage('old', 'model')
    const payload = { conversation_id: c.conversationId, error: 'failure' }
    const first = emit('chat-stream-error', payload), second = emit('chat-stream-error', payload)
    await vi.waitFor(() => expect(calls('save_assistant_message')).toHaveLength(1))
    persistence.resolve('saved-error'); await Promise.all([first, second])
    expect(calls('save_assistant_message')).toHaveLength(1)
    expect(c.chat.messages.at(-1)?.status).toBe('error')
    expect(c.chat.isLoading).toBe(false)
  })

  it('cancels replacement cleanup promptly and does not let its late result affect a new stream', async () => {
    const c = await tab(), gate = deferred<any>()
    transport.invoke.mockImplementation(async command => command === 'delete_message' ? gate.promise : undefined)
    await c.chat.regenerateMessage('old', 'model')
    const completing = done(c.conversationId)
    await vi.waitFor(() => expect(calls('delete_message')).toHaveLength(1))
    await c.chat.stopStream(); await completing
    await c.chat.addModelAnswer('group', '原问题', 'next-model')
    gate.resolve(undefined); await Promise.resolve()
    expect(c.chat.isLoading).toBe(true)
    expect(c.chat.messages.some(message => message.id === 'old')).toBe(true)
    await emit('chat-stream-token', { conversation_id: c.conversationId, token: 'current' })
    expect(c.chat.messages.at(-1)?.content).toBe('current')
  })

  it('pairs by group rather than the nearest question and rejects missing/edited groups and busy calls', async () => {
    const c = await tab()
    c.chat.messages.splice(1, 0, { ...row('user', 'unrelated', c.conversationId), group_id: 'unrelated', content: '别的问题' })
    expect(await c.chat.addModelAnswer('absent', '原问题', 'model')).toBe(false)
    expect(await c.chat.addModelAnswer('group', 'edited', 'model')).toBe(false)
    expect(await c.chat.regenerateMessage('missing', 'model')).toBe(false)
    await c.chat.regenerateMessage('old', 'model')
    expect(calls('send_message_stream')[0][1].content).toBe('原问题')
    expect(await c.chat.regenerateMessage('old', 'model')).toBe(false)
    expect(calls('send_message_stream')).toHaveLength(1)
    expect(c.chat.messages.filter(message => message.role === 'user')).toHaveLength(2)
  })

  it('stale listener registration cannot restart an old stream sharing the same message array', async () => {
    const c = await tab(), gate = deferred<void>()
    transport.gate = gate.promise
    const old = c.chat.regenerateMessage('old', 'model')
    await vi.waitFor(() => expect(transport.handlers.get('chat-stream-reset')?.size).toBeGreaterThan(0))
    await c.chat.stopStream()
    transport.gate = null
    await c.chat.addModelAnswer('group', '原问题', 'new-model')
    gate.resolve(); await old
    expect(calls('send_message_stream')).toHaveLength(1)
    expect(calls('send_message_stream')[0][1].modelId).toBe('new-model')
    await emit('chat-stream-token', { conversation_id: c.conversationId, token: 'new token' })
    expect(c.chat.messages.at(-1)?.content).toBe('new token')
    expect(c.chat.isLoading).toBe(true)
  })

  it('waits for a stop in flight before dispatching a newly registered stream', async () => {
    const c = await tab(), registration = deferred<void>(), stopping = deferred<void>()
    transport.gate = registration.promise
    transport.invoke.mockImplementation(async command => command === 'stop_stream' ? stopping.promise : undefined)
    const sending = c.chat.regenerateMessage('old', 'model')
    await vi.waitFor(() => expect(transport.handlers.get('chat-stream-reset')?.size).toBeGreaterThan(0))
    const stop = c.chat.stopStream()
    transport.gate = null; registration.resolve()
    await vi.waitFor(() => expect(calls('stop_stream')).toHaveLength(1))
    expect(calls('send_message_stream')).toHaveLength(0)
    stopping.resolve(); await stop; await sending
    expect(calls('send_message_stream')).toHaveLength(0)
    expect(c.chat.isLoading).toBe(false)
  })

  it('old error persistence cannot clear a newer stream or rewrite its message', async () => {
    const c = await tab(), gate = deferred<any>()
    transport.invoke.mockImplementation(async command => command === 'save_assistant_message' ? gate.promise : undefined)
    await c.chat.regenerateMessage('old', 'model')
    const failing = emit('chat-stream-error', { conversation_id: c.conversationId, error: 'first failed' })
    await vi.waitFor(() => expect(calls('save_assistant_message')).toHaveLength(1))
    await c.chat.stopStream(); await failing
    await c.chat.addModelAnswer('group', '原问题', 'next')
    gate.resolve('late-error-id'); await Promise.resolve()
    expect(c.chat.isLoading).toBe(true)
    expect(c.chat.messages.some(message => message.id === 'late-error-id')).toBe(false)
    await emit('chat-stream-token', { conversation_id: c.conversationId, token: 'next' })
    expect(c.chat.messages.at(-1)?.content).toBe('next')
  })

})
