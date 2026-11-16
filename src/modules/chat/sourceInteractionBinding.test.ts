// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { reactive, ref } from 'vue'
import { flushPromises } from '@vue/test-utils'
import type { Message } from '@/types'
import { createChatTabState } from './tabState'
import { createChatContext, createChatContextBinding } from './contextBinding'
import { createSourceInteractionBinding } from './sourceInteractionBinding'
import { useTraceSourceActions } from '@/components/chat/TraceSource.vue'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).reverse().forEach(fn => fn()))
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (reason?: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no }); return { promise, resolve, reject }
}
function fixture() {
  const owner = createChatTabState(crypto.randomUUID()); cleanup.push(() => owner.release())
  const store = reactive({ currentConversationId: 'a' as string | null, messages: [{ id: 'one', conversation_id: 'a', content: 'answer',
    trace_source: { local: { show: false, data: [{ snippet: 'source' }] } },
    web_hits: [{ title: 'source', url: 'https://example.com/a', snippet: 'text' }], web_search_query: 'query',
  }, { id: 'two', conversation_id: 'a', content: 'second', web_hits: [] }] as Message[] })
  const context = createChatContext({ instanceId: owner.instanceId, path: '/chat/a' }, owner, store)
  const binding = createChatContextBinding<typeof store>(); binding.activate(context); cleanup.push(binding.dispose)
  const ticks: (() => void)[] = [], gate = deferred<unknown>(), displayed = ref(true)
  const invoke = vi.fn((_cmd: string, _args?: Record<string, unknown>) => gate.promise)
  const dependencies = { traceActions: useTraceSourceActions, schedule: (fn: () => void) => ticks.push(fn), scroll: vi.fn(),
    getInvoke: vi.fn(async () => invoke as typeof invoke | null), fallback: vi.fn(), linkFailed: vi.fn() }
  const api = createSourceInteractionBinding(binding, dependencies)
  const link = (url = 'https://example.com/a') => api.captureLink('one', url, () => displayed.value)
  return { owner, store, context, binding, ticks, gate, displayed, invoke, dependencies, api, link }
}

describe('captured source selections', () => {
  it('uses actual trace toggles, retains selection on deactivate, and does not enable the disabled aside', () => {
    const f = fixture(); f.api.actions.value!.trace('local', 'one')
    expect(f.store.messages[0].trace_source!.local!.show).toBe(true)
    expect(f.owner.state.currentTraceMessageId.value).toBe('one'); expect(f.owner.state.rightPanel.value).toBe('none')
    f.ticks[0](); expect(f.dependencies.scroll).toHaveBeenCalledOnce()
    f.api.actions.value!.trace('result', 'one')
    expect(f.store.messages[0].trace_source!.result).toMatchObject({ show: true, loading: false, isSubmit: false })
    f.binding.deactivate(); f.ticks[1](); expect(f.dependencies.scroll).toHaveBeenCalledOnce()
    expect(f.owner.state.currentTraceMessageId.value).toBe('one')
  })
  it('rejects missing targets, cross-conversation IDs, and unknown trace types', () => {
    const f = fixture(); f.api.actions.value!.trace('unknown', 'one'); f.api.actions.value!.trace('local', 'missing')
    f.store.messages[0].conversation_id = 'b'; f.api.actions.value!.trace('local', 'one'); f.api.actions.value!.webSearch('one')
    expect(f.owner.state.currentTraceMessageId.value).toBeNull(); expect(f.api.webPanel.value).toBeNull()
    expect(f.ticks).toHaveLength(0)
  })
  it('rejects old actions after leaving and returning to the same Tab', () => {
    const f = fixture(), old = f.api.actions.value!
    f.binding.deactivate(); f.binding.activate(f.context); old.trace('local', 'one'); old.webSearch('one')
    expect(f.store.messages[0].trace_source!.local!.show).toBe(false); expect(f.api.webPanel.value).toBeNull()
  })
  it('only scrolls the latest trace action and rejects a replaced message target', () => {
    const f = fixture(); f.api.actions.value!.trace('local', 'one'); f.api.actions.value!.trace('local', 'one')
    f.ticks[0](); expect(f.dependencies.scroll).not.toHaveBeenCalled()
    f.store.messages[0] = { ...f.store.messages[0] }; f.ticks[1](); expect(f.dependencies.scroll).not.toHaveBeenCalled()
  })
  it('keeps panel state on deactivate, restores current hits on return, and rejects old close events', () => {
    const f = fixture(); f.api.actions.value!.webSearch('one'); const old = f.api.webPanel.value!
    expect(old).toMatchObject({ messageId: 'one', query: 'query', hits: f.store.messages[0].web_hits })
    f.binding.deactivate(); expect(f.api.webPanel.value).toBeNull()
    expect(f.owner.state.rightPanel.value).toBe('websearch')
    f.store.messages[0].web_hits = []; f.binding.activate(f.context); old.close()
    expect(f.api.webPanel.value!.hits).toEqual([]); f.api.webPanel.value!.close(); expect(f.api.webPanel.value).toBeNull()
  })
  it('does not let a stale close dismiss another target or a freshly reopened panel', () => {
    const f = fixture(); f.api.actions.value!.webSearch('one'); const old = f.api.webPanel.value!
    f.api.actions.value!.webSearch('two'); old.close(); expect(f.api.webPanel.value!.messageId).toBe('two')
    f.api.actions.value!.webSearch('one'); old.close(); expect(f.api.webPanel.value!.messageId).toBe('one')
    const current = f.api.webPanel.value!; current.close(); f.api.actions.value!.webSearch('one'); current.close()
    expect(f.api.webPanel.value!.messageId).toBe('one')
  })
  it('hides deleted targets and does not leave an empty aside', () => {
    const f = fixture(); f.api.actions.value!.webSearch('one'); f.store.messages = []
    expect(f.api.webPanel.value).toBeNull()
  })
})

describe('source link request ownership', () => {
  it('opens the captured URL using IPC and deduplicates in-flight row callbacks', async () => {
    const f = fixture(), task = f.link()(); await flushPromises(); await f.link()()
    expect(f.invoke).toHaveBeenCalledExactlyOnceWith('open_external', { url: 'https://example.com/a' })
    f.gate.resolve(null); await task; expect(f.dependencies.fallback).not.toHaveBeenCalled()
  })
  it.each(['javascript:alert(1)', 'file:///C:/private', 'data:text/html,x', 'mailto:a@b.com', 'not-a-url', '//example.com'])('rejects unsupported URL %s even if present in results', async url => {
    const f = fixture(); f.store.messages[0].web_hits![0].url = url
    await f.link(url)(); expect(f.dependencies.getInvoke).not.toHaveBeenCalled()
  })
  it('rejects URLs not in the current source and stale callbacks before transport loading', async () => {
    const f = fixture(); await f.link('https://example.com/unlisted')()
    const old = f.link(); f.binding.deactivate(); f.binding.activate(f.context); await old()
    expect(f.dependencies.getInvoke).not.toHaveBeenCalled()
  })
  it.each(['tab', 'conversation', 'reset', 'release', 'remove', 'replace', 'hit', 'display'] as const)(
    'cancels preparation on %s and consumes a late rejection without opening a link', async change => {
      const f = fixture(), transport = deferred<typeof f.invoke>(); f.dependencies.getInvoke.mockImplementation(() => transport.promise)
      const task = f.link()(); await flushPromises()
      if (change === 'tab') { f.binding.deactivate(); f.binding.activate(f.context) }
      if (change === 'conversation') f.store.currentConversationId = 'b'
      if (change === 'reset') f.owner.invalidateContext()
      if (change === 'release') f.owner.release()
      if (change === 'remove') f.store.messages = []
      if (change === 'replace') f.store.messages[0] = { ...f.store.messages[0] }
      if (change === 'hit') { f.store.messages[0].web_hits = []; f.store.messages[0].web_hits = [{ title: '', url: 'https://example.com/a', snippet: '' }] }
      if (change === 'display') { f.displayed.value = false; f.displayed.value = true }
      await task; transport.reject('late'); await flushPromises()
      expect(f.invoke).not.toHaveBeenCalled(); expect(f.dependencies.fallback).not.toHaveBeenCalled(); expect(f.dependencies.linkFailed).not.toHaveBeenCalled()
    })
  it('does not launch a delayed fallback after native IPC was accepted and the view left', async () => {
    const f = fixture(), task = f.link()(); await flushPromises(); f.binding.deactivate(); await task
    f.gate.reject('late native failure'); await flushPromises()
    expect(f.dependencies.fallback).not.toHaveBeenCalled(); expect(f.dependencies.linkFailed).not.toHaveBeenCalled()
  })
  it('falls back only while the original row remains visible and permits retry after completion', async () => {
    const f = fixture(), task = f.link()(); await flushPromises(); f.gate.reject('native failure'); await task
    expect(f.dependencies.fallback).toHaveBeenCalledExactlyOnceWith('https://example.com/a')
    f.invoke.mockResolvedValue(null); await f.link()(); expect(f.invoke).toHaveBeenCalledTimes(2)
  })
  it('supports browser-only transport and reports fallback errors in the originating view', async () => {
    const f = fixture(); f.dependencies.getInvoke.mockResolvedValue(null)
    f.dependencies.fallback.mockImplementationOnce(() => { throw new Error('blocked') })
    await f.link()(); expect(f.dependencies.linkFailed).toHaveBeenCalledOnce()
    await f.link()(); expect(f.dependencies.fallback).toHaveBeenCalledTimes(2)
  })
  it('does not let obsolete request cleanup remove a replacement lock', async () => {
    const f = fixture(), gate = deferred<typeof f.invoke>(); f.dependencies.getInvoke.mockImplementationOnce(() => gate.promise)
    const old = f.link()(); await flushPromises(); f.binding.deactivate(); f.binding.activate(f.context)
    const current = f.link()(); await old; await flushPromises(); await f.link()()
    expect(f.invoke).toHaveBeenCalledOnce(); gate.resolve(f.invoke); f.gate.resolve(null); await current
  })
})
