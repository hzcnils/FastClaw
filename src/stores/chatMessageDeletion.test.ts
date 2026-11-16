// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises } from '@vue/test-utils'
import type { Message } from '@/types'
import { useChatStore, disposeChatStore, type PreparedConversationMessages } from './chat'
import { useExecutionActivityStore } from './executionActivity'

const transport = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: transport.invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }))
interface Store {
  currentConversationId: string | null; messages: Message[]
  loadMessages(id: string): Promise<void>
  prepareMessages(id: string): Promise<PreparedConversationMessages | null>
  commitPreparedMessages(prepared: PreparedConversationMessages): void
  loadMoreMessages(id: string): Promise<void>
  deleteMessage(id: string, canStart?: () => boolean): Promise<boolean>
  sendMessage(...args: unknown[]): Promise<boolean>
  addModelAnswer(...args: unknown[]): Promise<boolean>
}
let sequence = 0
const tabs: string[] = []
function tab() { const id = `delete-tab-${++sequence}`; tabs.push(id); return useChatStore(id) as unknown as Store }
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (reason: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no }); return { promise, resolve, reject }
}
beforeEach(() => { setActivePinia(createPinia()); sessionStorage.clear(); transport.invoke.mockReset() })
afterEach(async () => { for (const id of tabs.splice(0)) await disposeChatStore(id) })
async function fixture() {
  const conversationId = `delete-conversation-${++sequence}`, nextId = `${conversationId}-next`
  const message = (id: string, conv = conversationId) => ({ id, conversation_id: conv, role: id === 'u' ? 'user' : 'assistant', content: 'body', group_id: 'g', status: 'completed', created_at: '2026-09-24T00:00:00Z', model_id: null, token_input: null, token_output: null } as Message)
  const gate = deferred<string[]>()
  transport.invoke.mockImplementation(async (cmd, args) => {
    if (cmd === 'get_conversations') return []
    if (cmd === 'get_messages') return [message('m', args.conversationId)]
    if (cmd === 'delete_message_turn') return gate.promise
    return undefined
  })
  const store = tab(); await store.loadMessages(conversationId)
  return { store, gate, conversationId, nextId, message }
}
describe('actual Store deletion cache integration', () => {
  it('removes both persisted and temporary question IDs for a confirmed last-answer deletion', async () => {
    const f = await fixture(); f.store.messages.unshift({ ...f.message('temporary-user'), role: 'user' })
    const task = f.store.deleteMessage('m'); f.gate.resolve(['m', 'database-user']); await task
    expect(f.store.messages).toHaveLength(0)
    f.store.commitPreparedMessages({ conversationId: f.conversationId, messages: [{ ...f.message('database-user'), role: 'user' }] })
    expect(f.store.messages).toHaveLength(0)
  })
  it('filters a late initial database read after deletion commits before that read returns', async () => {
    const f = await fixture(), cold = f.conversationId + '-cold', read = deferred<Message[]>()
    f.store.currentConversationId = cold; f.store.messages = [f.message('m', cold)]
    transport.invoke.mockImplementation(cmd => cmd === 'get_messages' ? read.promise : cmd === 'delete_message_turn' ? f.gate.promise : Promise.resolve([]))
    const other = tab(), loading = other.prepareMessages(cold); await flushPromises()
    const deleting = f.store.deleteMessage('m'); f.gate.resolve(['m']); await deleting
    read.resolve([f.message('m', cold), f.message('keep', cold)]); const prepared = await loading
    other.commitPreparedMessages(prepared!); expect(other.messages.map(m => m.id)).toEqual(['keep'])
  })
  it('does not optimistically remove messages when the database rejects', async () => {
    const f = await fixture(), task = f.store.deleteMessage('m'); await flushPromises()
    expect(f.store.messages.map(m => m.id)).toEqual(['m']); f.gate.reject(new Error('db'))
    await expect(task).rejects.toThrow('db'); expect(f.store.messages.map(m => m.id)).toEqual(['m'])
  })
  it('commits to source caches and other open aliases without deleting a reused ID in the newly selected conversation', async () => {
    const f = await fixture(), alias = tab(); await alias.loadMessages(f.conversationId)
    const original = f.store.messages, task = f.store.deleteMessage('m'); await flushPromises()
    await f.store.loadMessages(f.nextId); f.gate.resolve(['m']); expect(await task).toBe(true)
    expect(original).toHaveLength(0); expect(alias.messages).toHaveLength(0); expect(f.store.messages).toHaveLength(1)
    await f.store.loadMessages(f.conversationId); expect(f.store.messages).toHaveLength(0)
  })
  it('filters stale prepared snapshots at the real commit point', async () => {
    const f = await fixture(), prepared = { conversationId: f.conversationId, messages: [f.message('m'), f.message('u')] }
    const task = f.store.deleteMessage('m'); f.gate.resolve(['m','u']); await task
    f.store.commitPreparedMessages(prepared); expect(f.store.messages).toHaveLength(0)
  })
  it('filters a late pagination response using database-returned IDs outside the loaded page', async () => {
    const f = await fixture(), older = deferred<Message[]>()
    transport.invoke.mockImplementation((cmd, args) => {
      if (cmd === 'get_messages_page') return older.promise
      if (cmd === 'delete_message_turn') return f.gate.promise
      if (cmd === 'get_messages') return Promise.resolve([f.message('m', args.conversationId)])
      return Promise.resolve([])
    })
    const page = f.store.loadMoreMessages(f.conversationId); await flushPromises()
    const task = f.store.deleteMessage('m'); f.gate.resolve(['m','u']); await task
    older.resolve([f.message('u'), f.message('keep')]); await page
    expect(f.store.messages.map(m => m.id)).toEqual(['keep'])
  })
  it('blocks ordinary send and answer retry during accepted deletion', async () => {
    const f = await fixture(), task = f.store.deleteMessage('m'); await flushPromises()
    expect(await f.store.sendMessage('new','model')).toBe(false)
    expect(await f.store.addModelAnswer('g','body','model')).toBe(false)
    expect(transport.invoke.mock.calls.filter(([cmd]) => cmd === 'delete_message_turn')).toHaveLength(1)
    f.gate.resolve(['m']); await task
  })
  it('rejects manual planning waits without stopping execution', async () => {
    const f = await fixture(); useExecutionActivityStore().update(f.conversationId,'/chat/test','fastplan',{ phase: 'awaiting_step_confirmation', protected: true })
    await expect(f.store.deleteMessage('m')).rejects.toThrow('会话正在执行')
    expect(transport.invoke.mock.calls.some(([cmd]) => cmd === 'delete_message_turn' || cmd === 'stop_stream')).toBe(false)
  })
})
