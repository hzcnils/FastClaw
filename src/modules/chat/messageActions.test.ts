// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { reactive } from 'vue'
import { flushPromises } from '@vue/test-utils'
import type { Message } from '@/types'
import { createChatTabState } from './tabState'
import { MessageActionService, type MessageActionOptions } from './messageActions'
import { createChatContext, createChatContextBinding } from './contextBinding'
import { createMessageActionBinding } from './messageActionBinding'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).reverse().forEach(fn => fn()))
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (reason: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no }); return { promise, resolve, reject }
}
function fixture() {
  const owner = createChatTabState('actions'); cleanup.push(() => owner.release())
  const store = reactive({ currentConversationId: 'a' as string | null,
    messages: [{ id: 'one', conversation_id: 'a', content: 'source', is_favorite: 0, is_liked: 0 },
      { id: 'two', conversation_id: 'a', content: 'second', is_favorite: 0, is_liked: 0 }] as Message[] })
  const ipc = deferred<unknown>(), invoke = vi.fn((_command: string, _args?: Record<string, unknown>) => ipc.promise)
  const options: MessageActionOptions = { getInvoke: vi.fn(async () => invoke), canStart: vi.fn(() => true),
    isVisible: vi.fn(() => true), report: vi.fn() }
  const target = { owner, store }, service = new MessageActionService()
  const favorite = (value = true) => service.favorite(target, 'one', value, options)
  const feedback = (value = 1) => service.feedback(target, 'one', value, options)
  const translate = (lang = 'en') => service.translate(target, 'one', lang, 'model', options)
  return { owner, store, target, options, ipc, invoke, service, favorite, feedback, translate }
}

describe('application-owned message actions', () => {
  it('rolls back the original ID after reordering, preserving unrelated new fields', async () => {
    const f = fixture(), task = f.favorite(); await flushPromises()
    expect(f.store.messages[0].is_favorite).toBe(1)
    f.store.messages.reverse()
    f.store.messages[1] = { ...f.store.messages[1], content: 'edited', translation: 'new field' }
    f.ipc.reject(new Error('failed')); expect(await task).toBe(false)
    expect(f.store.messages[0]).toMatchObject({ id: 'two', is_favorite: 0 })
    expect(f.store.messages[1]).toMatchObject({ id: 'one', is_favorite: 0, content: 'edited', translation: 'new field' })
    expect(f.options.report).toHaveBeenCalledWith('favorite', expect.any(Error))
  })
  it.each(['favorite', 'feedback', 'translate'] as const)('does not write a %s result into another conversation with the same message ID', async action => {
    const f = fixture(), task = f[action](); await flushPromises()
    f.store.currentConversationId = 'b'
    f.store.messages = [{ id: 'one', conversation_id: 'b', content: 'other' }] as Message[]
    f.ipc.resolve('translated'); await task
    expect(f.store.messages[0]).toEqual({ id: 'one', conversation_id: 'b', content: 'other' })
    expect(f.options.report).not.toHaveBeenCalled()
  })
  it.each(['favorite', 'feedback', 'translate'] as const)('skips %s after UI expires while resolving invoke', async action => {
    const f = fixture(), get = deferred<typeof f.invoke>()
    f.options.getInvoke = () => get.promise
    const task = f[action](); await flushPromises()
    f.options.canStart = () => false
    get.resolve(f.invoke); await task
    expect(f.invoke).not.toHaveBeenCalled()
    expect(f.store.messages[0]).toMatchObject({ is_favorite: 0, is_liked: 0 })
    expect(f.store.messages[0].translating).toBeUndefined()
  })
  it.each(['favorite', 'feedback', 'translate'] as const)('completes accepted %s work in the original hidden Tab', async action => {
    const f = fixture(), task = f[action](); await flushPromises()
    f.options.canStart = () => false; f.options.isVisible = () => false
    f.ipc.resolve('translated'); await task
    if (action === 'favorite') expect(f.store.messages[0].is_favorite).toBe(1)
    if (action === 'feedback') expect(f.store.messages[0].is_liked).toBe(1)
    if (action === 'translate') expect(f.store.messages[0]).toMatchObject({ translation: 'translated', translating: false })
    expect(f.options.report).not.toHaveBeenCalled()
  })
  it.each(['invalidate', 'release', 'remove'] as const)('does not recreate or patch a target after %s', async action => {
    const f = fixture(), task = f.feedback(); await flushPromises()
    if (action === 'invalidate') f.owner.invalidateContext()
    if (action === 'release') f.owner.release()
    if (action === 'remove') f.store.messages.splice(0, 1)
    const before = JSON.stringify(f.store.messages)
    f.ipc.resolve(null); await task
    expect(JSON.stringify(f.store.messages)).toBe(before)
  })
  it('deduplicates identical pending intents and serializes like/dislike database writes', async () => {
    const f = fixture(), second = deferred<unknown>()
    f.invoke.mockImplementationOnce(() => f.ipc.promise).mockImplementationOnce(() => second.promise)
    const first = f.feedback(1); expect(f.feedback(1)).toBe(first)
    await flushPromises()
    const last = f.feedback(-1); expect(f.feedback(-1)).toBe(last)
    expect(f.invoke).toHaveBeenCalledTimes(1)
    f.ipc.resolve(null); await first; await flushPromises()
    expect(f.invoke.mock.calls.map(call => call[1]?.isLiked)).toEqual([1, -1])
    expect(f.store.messages[0].is_liked).toBe(1)
    second.resolve(null); await last; expect(f.store.messages[0].is_liked).toBe(-1)
  })
  it('serializes favorite cancellation after a failed request using the latest confirmed baseline', async () => {
    const f = fixture(), second = deferred<unknown>()
    f.invoke.mockImplementationOnce(() => f.ipc.promise).mockImplementationOnce(() => second.promise)
    const first = f.favorite(); await flushPromises(); const last = f.favorite(false)
    f.ipc.reject(new Error('failed')); await first; await flushPromises()
    expect(f.invoke.mock.calls.map(call => call[1]?.isFavorite)).toEqual([true, false])
    second.resolve(null); await last; expect(f.store.messages[0].is_favorite).toBe(0)
  })
  it('discards queued UI work on deactivation without cancelling accepted IPC', async () => {
    const f = fixture(), first = f.feedback(1); await flushPromises(); const second = f.feedback(-1)
    f.options.canStart = () => false; f.options.isVisible = () => false
    f.ipc.resolve(null); await Promise.all([first, second])
    expect(f.invoke).toHaveBeenCalledTimes(1); expect(f.store.messages[0].is_liked).toBe(1)
  })
  it('keeps unrelated message fields and different messages independent', async () => {
    const f = fixture(), tasks = [f.favorite(), f.feedback(), f.service.feedback(f.target, 'two', -1, f.options)]
    await flushPromises(); expect(f.invoke).toHaveBeenCalledTimes(3)
    f.ipc.resolve(null); await Promise.all(tasks)
    expect(f.store.messages[0]).toMatchObject({ is_liked: 1, is_favorite: 1 })
    expect(f.store.messages[1].is_liked).toBe(-1)
  })
  it.each(['favorite', 'feedback', 'translate'] as const)('consumes late %s failures without reporting in another UI', async action => {
    const f = fixture(), task = f[action](); await flushPromises(); f.options.isVisible = () => false
    f.ipc.reject({ message: 'late' }); await task
    expect(f.options.report).not.toHaveBeenCalled()
    if (action === 'translate') expect(f.store.messages[0].translating).toBe(false)
  })
  it('rejects invalid feedback and unknown targets without loading IPC', async () => {
    const f = fixture()
    expect(await f.feedback(9)).toBe(false)
    expect(await f.service.favorite(f.target, 'missing', true, f.options)).toBe(false)
    expect(f.options.getInvoke).not.toHaveBeenCalled()
  })
  it.each(['before', 'after'] as const)('does not translate changed content %s invoking the backend', async when => {
    const f = fixture(), get = deferred<typeof f.invoke>(); f.options.getInvoke = () => get.promise
    const task = f.translate(); await flushPromises()
    if (when === 'before') f.store.messages[0].content = 'edited'
    get.resolve(f.invoke); await flushPromises()
    if (when === 'after') f.store.messages[0].content = 'edited'
    f.ipc.resolve('old translation'); expect(await task).toBeNull()
    expect(f.store.messages[0].translation).toBeUndefined()
    if (when === 'before') expect(f.invoke).not.toHaveBeenCalled()
    else expect(f.store.messages[0].translating).toBe(false)
  })
  it('preserves the selected model/language snapshot and serializes language changes', async () => {
    const f = fixture(), second = deferred<unknown>()
    f.invoke.mockImplementationOnce(() => f.ipc.promise).mockImplementationOnce(() => second.promise)
    const first = f.translate('en'); await flushPromises(); const last = f.translate('zh')
    f.ipc.resolve('English'); await first; await flushPromises()
    expect(f.invoke.mock.calls.map(call => call[1])).toEqual([
      { messageId: 'one', targetLang: 'en', modelId: 'model', expectedContent: 'source', expectedConversationId: 'a' },
      { messageId: 'one', targetLang: 'zh', modelId: 'model', expectedContent: 'source', expectedConversationId: 'a' }])
    second.resolve('中文'); expect(await last).toBe('中文')
    expect(f.store.messages[0]).toMatchObject({ translation_lang: 'zh', translation: '中文', translating: false })
  })
  it('handles missing transport and rejects malformed translation results with loading cleanup', async () => {
    const f = fixture(); f.options.getInvoke = async () => null
    expect(await f.translate()).toBeNull(); expect(f.options.report).toHaveBeenCalledWith('unavailable')
    f.options.getInvoke = async () => f.invoke
    const task = f.translate(); await flushPromises(); f.ipc.resolve(null); expect(await task).toBeNull()
    expect(f.store.messages[0].translating).toBe(false)
    expect(f.options.report).toHaveBeenLastCalledWith('translate', expect.any(Error))
  })
  it('clears only its own translation spinner on owner invalidation, even after immutable feedback copies', async () => {
    const f = fixture(), task = f.translate(); await flushPromises()
    f.store.messages[0] = { ...f.store.messages[0], is_liked: 1 }
    expect(f.store.messages[0].translating).toBe(true)
    f.owner.invalidateContext()
    expect(f.store.messages[0].translating).toBe(false)
    expect(Object.getOwnPropertySymbols(f.store.messages[0])).toHaveLength(0)
    f.ipc.resolve('obsolete'); await task
    expect(f.store.messages[0].translation).toBeUndefined()
    expect(f.store.messages[0].is_liked).toBe(1)
  })
  it('clears the detached original cache without resetting another owner’s pending translation', async () => {
    const f = fixture(), task = f.translate(); await flushPromises()
    const original = f.store.messages
    f.store.currentConversationId = 'b'
    f.store.messages = [{ id: 'one', conversation_id: 'b', content: 'other', translating: true }] as Message[]
    f.ipc.resolve('obsolete'); await task
    expect(original[0].translating).toBe(false)
    expect(f.store.messages[0].translating).toBe(true)
  })
  it('starts fresh work after context rotation instead of deduplicating against an expired preparation', async () => {
    const f = fixture(), get = deferred<typeof f.invoke>()
    const context = createChatContext({ path: '/chat/actions', instanceId: 'actions' }, f.owner, f.store)
    const binding = createChatContextBinding<typeof f.store>(); cleanup.push(binding.dispose); binding.activate(context)
    const actions = createMessageActionBinding(binding, { getInvoke: () => get.promise, report: f.options.report })
    const old = actions.value!.like('one', 1); await flushPromises()
    binding.deactivate(); binding.activate(context)
    const fresh = actions.value!.like('one', 1)
    get.resolve(f.invoke); await old; await flushPromises()
    expect(f.invoke).toHaveBeenCalledTimes(1)
    f.ipc.resolve(null); expect(await fresh).toBe(true)
  })
})

describe('message action presentation binding', () => {
  it('rejects old callbacks after A → B → A while preserving accepted results', async () => {
    const f = fixture(), context = createChatContext({ path: '/chat/actions', instanceId: 'actions' }, f.owner, f.store)
    const binding = createChatContextBinding<typeof f.store>(); cleanup.push(binding.dispose); binding.activate(context)
    const actions = createMessageActionBinding(binding, f.options), first = actions.value!
    const task = first.like('one', 1); await flushPromises()
    binding.deactivate(); binding.activate(context)
    await first.favorite('one', true); await first.translate('one', 'en')
    expect(f.invoke).toHaveBeenCalledTimes(1)
    f.ipc.resolve(null); await task; expect(f.store.messages[0].is_liked).toBe(1)
    expect(actions.value).not.toBe(first)
  })
})
