// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { reactive } from 'vue'
import { flushPromises } from '@vue/test-utils'
import type { Message } from '@/types'
import { createChatTabState } from './tabState'
import { createChatContext, createChatContextBinding } from './contextBinding'
import { DeletedMessageLedger, MessageDeletionService, MessageDeletionBusyError, MessageDeletionUnavailableError } from './messageDeletion'
import { createMessageDeletionBinding } from './messageDeletionBinding'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).reverse().forEach(fn => fn()))
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (reason: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no }); return { promise, resolve, reject }
}
const message = (id = 'm', conversation_id = 'a') => ({ id, conversation_id, role: 'assistant', group_id: 'g', content: 'body', status: 'completed' } as Message)
function fixture() {
  const owner = createChatTabState('delete'), service = new MessageDeletionService(); cleanup.push(() => owner.release())
  const gate = deferred<unknown>(), invoke = vi.fn((_command: string, _args?: Record<string, unknown>) => gate.promise)
  const options = { getInvoke: vi.fn(async (): Promise<typeof invoke | null> => invoke), busy: vi.fn(() => false), commit: vi.fn() }
  const store = reactive({ currentConversationId: 'a' as string | null, messages: [message()],
    deleteMessage(id: string, canStart = () => true) { return service.remove({ owner, store }, id, { ...options, canStart }) },
  })
  const context = createChatContext({ instanceId: owner.instanceId, path: '/chat/a' }, owner, store)
  const binding = createChatContextBinding<typeof store>(); binding.activate(context); cleanup.push(binding.dispose)
  const report = vi.fn(), actions = createMessageDeletionBinding(binding, report)
  return { owner, service, gate, invoke, options, store, context, binding, report, actions }
}
describe('message deletion ownership', () => {
  it('removes the unique original local question alias only when backend confirms question deletion', async () => {
    const f = fixture(); f.store.messages.push({ ...message('temporary-question'), role: 'user' })
    const task = f.store.deleteMessage('m'); f.gate.resolve(['m', 'database-question']); await task
    expect(f.options.commit).toHaveBeenCalledWith('a', ['m', 'database-question', 'temporary-question'])
  })
  it('does not remove question aliases when a sibling survives or the local group is ambiguous', async () => {
    const f = fixture(); f.store.messages.push({ ...message('temporary-question'), role: 'user' })
    f.invoke.mockResolvedValueOnce(['m']); await f.store.deleteMessage('m')
    expect(f.options.commit).toHaveBeenLastCalledWith('a', ['m'])
    f.store.messages.push({ ...message('duplicate-question'), role: 'user' })
    f.invoke.mockResolvedValueOnce(['m', 'database-question']); await f.store.deleteMessage('m')
    expect(f.options.commit).toHaveBeenLastCalledWith('a', ['m', 'database-question'])
  })
  it('sends a captured target, commits only returned IDs, then releases the conversation lock', async () => {
    const f = fixture(), task = f.store.deleteMessage('m'); expect(f.service.isPending('a')).toBe(true)
    await flushPromises(); expect(f.invoke).toHaveBeenCalledWith('delete_message_turn', { conversationId: 'a', messageId: 'm', expectedContent: 'body' })
    f.gate.resolve(['m', 'u']); expect(await task).toBe(true)
    expect(f.options.commit).toHaveBeenCalledWith('a', ['m', 'u']); expect(f.service.isPending('a')).toBe(false)
  })
  it.each(['missing', 'wrong-conversation', 'no-conversation'])('rejects %s without IPC', async kind => {
    const f = fixture()
    if (kind === 'missing') f.store.messages = []
    if (kind === 'wrong-conversation') f.store.messages[0].conversation_id = 'b'
    if (kind === 'no-conversation') f.store.currentConversationId = null
    expect(await f.store.deleteMessage('m')).toBe(false); expect(f.invoke).not.toHaveBeenCalled()
  })
  it('blocks execution before and after transport preparation', async () => {
    const f = fixture(); f.options.busy.mockReturnValueOnce(true)
    await expect(f.store.deleteMessage('m')).rejects.toBeInstanceOf(MessageDeletionBusyError)
    const transport = deferred<typeof f.invoke | null>(); f.options.getInvoke.mockReturnValueOnce(transport.promise)
    const task = f.store.deleteMessage('m'); f.options.busy.mockReturnValue(true); transport.resolve(f.invoke)
    await expect(task).rejects.toBeInstanceOf(MessageDeletionBusyError); expect(f.invoke).not.toHaveBeenCalled()
  })
  it('keeps messages unchanged on persistence failure and allows retry', async () => {
    const f = fixture(); f.invoke.mockRejectedValueOnce(new Error('database'))
    await expect(f.store.deleteMessage('m')).rejects.toThrow('database'); expect(f.options.commit).not.toHaveBeenCalled()
    expect(f.store.messages).toHaveLength(1); expect(f.service.isPending('a')).toBe(false)
    f.invoke.mockResolvedValueOnce(['m']); expect(await f.store.deleteMessage('m')).toBe(true)
  })
  it('does not delete in a browser-only environment', async () => {
    const f = fixture(); f.options.getInvoke.mockResolvedValueOnce(null)
    await expect(f.store.deleteMessage('m')).rejects.toBeInstanceOf(MessageDeletionUnavailableError)
    expect(f.options.commit).not.toHaveBeenCalled()
  })
  it.each([null, [], ['other'], ['m', 42]])('rejects invalid backend receipt %j', async receipt => {
    const f = fixture(), task = f.store.deleteMessage('m'); f.gate.resolve(receipt)
    await expect(task).rejects.toThrow('删除结果无效'); expect(f.options.commit).not.toHaveBeenCalled()
  })
  it('deduplicates a conversation even across another owner', async () => {
    const f = fixture(), task = f.store.deleteMessage('m'); await flushPromises()
    const other = createChatTabState('other'); cleanup.push(() => other.release())
    expect(await f.service.remove({ owner: other, store: f.store }, 'm', { ...f.options, canStart: () => true })).toBe(false)
    f.gate.resolve(['m']); await task; expect(f.invoke).toHaveBeenCalledOnce()
  })
  it.each(['hide', 'release', 'conversation', 'content'])('accepted delete commits only its original conversation after %s', async change => {
    const f = fixture(), task = f.actions.value!.remove('m'); await flushPromises()
    if (change === 'hide') f.binding.deactivate()
    if (change === 'release') f.owner.release()
    if (change === 'conversation') f.store.currentConversationId = 'b'
    if (change === 'content') f.store.messages[0].content = 'new'
    await task; expect(f.service.isPending('a')).toBe(true)
    f.gate.resolve(['m']); await flushPromises(); expect(f.options.commit).toHaveBeenCalledWith('a', ['m'])
    expect(f.report).not.toHaveBeenCalled(); expect(f.service.isPending('a')).toBe(false)
  })
  it('invalid preparation releases its effective lock and old finally cannot erase replacement lock', async () => {
    const f = fixture(), transport = deferred<typeof f.invoke | null>(); f.options.getInvoke.mockReturnValueOnce(transport.promise)
    const old = f.actions.value!.remove('m'); f.binding.deactivate(); await old
    expect(f.service.isPending('a')).toBe(false); f.binding.activate(f.context)
    const current = f.actions.value!.remove('m'); await flushPromises(); transport.resolve(f.invoke); await flushPromises()
    expect(f.invoke).toHaveBeenCalledOnce(); expect(f.service.isPending('a')).toBe(true)
    f.gate.resolve(['m']); await current; expect(f.service.isPending('a')).toBe(false)
  })
  it('reports failures only in the original visible view', async () => {
    const f = fixture(); f.options.busy.mockReturnValueOnce(true); await f.actions.value!.remove('m')
    expect(f.report).toHaveBeenCalledWith('busy', expect.any(Error)); f.report.mockClear()
    const task = f.actions.value!.remove('m'); await flushPromises(); f.binding.deactivate(); await task
    f.gate.reject(new Error('late')); await flushPromises(); expect(f.report).not.toHaveBeenCalled()
  })
})

describe('successful deletion ledger', () => {
  it('updates all aliases in place and preserves other conversations with reused IDs', () => {
    const ledger = new DeletedMessageLedger(), a = [message(), message('keep')], b = [message('m','b')], copy = [...a]
    ledger.commit('a', ['m'], [a, a, copy, b])
    expect(a.map(m => m.id)).toEqual(['keep']); expect(copy.map(m => m.id)).toEqual(['keep']); expect(b).toHaveLength(1)
  })
  it('filters late loads and prepared snapshots without removing newly appended unrelated messages', () => {
    const ledger = new DeletedMessageLedger(), old = [message(), message('u')]
    ledger.commit('a', ['m','u'], []); old.push(message('new'))
    expect(ledger.prune('a', old)).toBe(old); expect(old.map(m => m.id)).toEqual(['new'])
  })
})
