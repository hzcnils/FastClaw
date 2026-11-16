// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { reactive } from 'vue'
import { flushPromises } from '@vue/test-utils'
import type { Message } from '@/types'
import { createChatTabState } from './tabState'
import { createChatContext, createChatContextBinding } from './contextBinding'
import { createQualityFeedbackBinding } from './qualityFeedbackBinding'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).reverse().forEach(fn => fn()))
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (reason?: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no }); return { promise, resolve, reject }
}
function fixture() {
  const owner = createChatTabState('quality'); cleanup.push(() => owner.release())
  const store = reactive({ currentConversationId: 'a' as string | null, messages: [{ id: 'one', conversation_id: 'a',
    content: 'answer', trace_source: { result: { show: true, isSubmit: false, loading: false, rate: 5,
      content: 'feedback', isQuery: true } } }] as Message[] })
  const context = createChatContext({ instanceId: 'quality', path: '/chat/quality' }, owner, store)
  const binding = createChatContextBinding<typeof store>(); binding.activate(context); cleanup.push(binding.dispose)
  const dialog = deferred<string>(), ipc = deferred<unknown>()
  const invoke = vi.fn((_cmd: string, _args?: Record<string, unknown>) => ipc.promise)
  const dependencies = { prompt: vi.fn(() => dialog.promise), getInvoke: vi.fn(async () => invoke), report: vi.fn() }
  const actions = createQualityFeedbackBinding(binding, dependencies)
  const trace = store.messages[0].trace_source!.result!
  return { owner, store, context, binding, dialog, ipc, invoke, dependencies, actions, trace }
}

describe('quality feedback UI preparation and accepted persistence', () => {
  it('submits the captured target using the real backend argument names', async () => {
    const f = fixture(), task = f.actions.value!.request('one'); await flushPromises()
    f.dialog.resolve('  useful feedback  '); await flushPromises()
    expect(f.invoke).toHaveBeenCalledWith('submit_quality_evaluation', { messageId: 'one', conversationId: 'a',
      score: 0, content: 'useful feedback', expectedContent: 'answer' })
    f.ipc.resolve(null); expect(await task).toBe(true)
    expect(f.dependencies.report).toHaveBeenCalledWith('success')
  })
  it.each(['tab', 'conversation', 'reset', 'release', 'remove', 'content', 'content-revert'] as const)(
    'expires the prompt on %s and handles its late rejection', async reason => {
      const f = fixture(), task = f.actions.value!.request('one'); await flushPromises()
      if (reason === 'tab') f.binding.deactivate()
      if (reason === 'conversation') f.store.currentConversationId = 'b'
      if (reason === 'reset') f.owner.invalidateContext()
      if (reason === 'release') f.owner.release()
      if (reason === 'remove') f.store.messages = []
      if (reason === 'content' || reason === 'content-revert') f.store.messages[0].content = 'changed'
      if (reason === 'content-revert') f.store.messages[0].content = 'answer'
      expect(await task).toBe(false)
      f.dialog.reject(new Error('late')); await flushPromises()
      expect(f.invoke).not.toHaveBeenCalled(); expect(f.dependencies.report).not.toHaveBeenCalled()
    })
  it('does not revive old callbacks after returning to the same Tab', async () => {
    const f = fixture(), old = f.actions.value!
    f.binding.deactivate(); f.binding.activate(f.context)
    expect(await old.submit('one', 5, 'feedback')).toBe(false)
    expect(await old.request('one')).toBe(false)
    expect(f.dependencies.prompt).not.toHaveBeenCalled(); expect(f.invoke).not.toHaveBeenCalled()
  })
  it('prevents duplicate prompts and inline submissions across presentation rebuilds', async () => {
    const f = fixture(), task = f.actions.value!.request('one'); await flushPromises()
    expect(await f.actions.value!.request('one')).toBe(false)
    expect(await f.actions.value!.submit('one', 5, 'feedback')).toBe(false)
    f.dialog.resolve('feedback'); await flushPromises()
    f.binding.deactivate(); f.binding.activate(f.context)
    expect(await f.actions.value!.submit('one', 5, 'feedback')).toBe(false)
    expect(f.dependencies.prompt).toHaveBeenCalledTimes(1); expect(f.invoke).toHaveBeenCalledTimes(1)
    f.ipc.resolve(null); await task
  })
  it('cleans up preparation loading and releases the lock without waiting for invoke import', async () => {
    const f = fixture(), get = deferred<typeof f.invoke>(); f.dependencies.getInvoke.mockImplementation(() => get.promise)
    const task = f.actions.value!.submit('one', 5, 'feedback'); await flushPromises()
    expect(f.trace.loading).toBe(true)
    f.binding.deactivate(); expect(await task).toBe(false); expect(f.trace.loading).toBe(false)
    get.resolve(f.invoke); await flushPromises(); expect(f.invoke).not.toHaveBeenCalled()
    f.binding.activate(f.context)
    const fresh = f.actions.value!.submit('one', 5, 'feedback'); await flushPromises()
    f.ipc.resolve(null); expect(await fresh).toBe(true)
  })
  it('completes accepted feedback on the original message after deactivation, without a misplaced toast', async () => {
    const f = fixture(), task = f.actions.value!.submit('one', 5, 'feedback'); await flushPromises()
    expect(f.trace.loading).toBe(true)
    f.binding.deactivate(); f.ipc.resolve(null); expect(await task).toBe(true)
    expect(f.trace.loading).toBe(false); expect(f.trace.isSubmit).toBe(true)
    expect(f.dependencies.report).not.toHaveBeenCalled()
  })
  it.each(['context', 'form', 'draft'] as const)('does not mark a changed %s as submitted by an old request', async change => {
    const f = fixture(), task = f.actions.value!.submit('one', 5, 'feedback'); await flushPromises()
    if (change === 'context') { f.owner.invalidateContext(); f.store.currentConversationId = 'b' }
    if (change === 'form') f.store.messages[0].trace_source!.result = { ...f.trace, loading: false }
    if (change === 'draft') f.trace.content = 'new feedback'
    f.ipc.resolve(null); await task
    expect(f.store.messages[0].trace_source!.result!.isSubmit).toBe(false)
    expect(f.trace.loading).toBe(false)
  })
  it('keeps an inline draft after failure and permits retry', async () => {
    const f = fixture(), task = f.actions.value!.submit('one', 5, 'feedback'); await flushPromises()
    f.ipc.reject(new Error('failed')); expect(await task).toBe(false)
    expect(f.trace).toMatchObject({ loading: false, isSubmit: false, content: 'feedback', rate: 5 })
    expect(f.dependencies.report).toHaveBeenCalledWith('error')
    f.invoke.mockResolvedValue(null)
    expect(await f.actions.value!.submit('one', 5, 'feedback')).toBe(true)
  })
  it.each([-1, 11, NaN, Infinity])('rejects invalid score %s without IPC', async score => {
    const f = fixture(); expect(await f.actions.value!.submit('one', score, 'feedback')).toBe(false)
    expect(f.dependencies.getInvoke).not.toHaveBeenCalled(); expect(f.trace.loading).toBe(false)
  })
  it('rejects empty unscored feedback and permits score-only feedback', async () => {
    const f = fixture(); expect(await f.actions.value!.submit('one', 0, '  ')).toBe(false)
    const task = f.actions.value!.submit('one', 8, ''); await flushPromises()
    expect(f.invoke.mock.calls[0][1]).toMatchObject({ score: 8, content: '' })
    f.ipc.resolve(null); expect(await task).toBe(true)
  })
  it('ignores cancellation and allows another prompt without reporting an error', async () => {
    const f = fixture(), task = f.actions.value!.request('one'); await flushPromises()
    f.dialog.reject('cancel'); expect(await task).toBe(false)
    expect(f.dependencies.report).not.toHaveBeenCalled()
    f.dependencies.prompt.mockResolvedValue('second'); f.invoke.mockResolvedValue(null)
    expect(await f.actions.value!.request('one')).toBe(true)
  })
  it('reports missing transport and reliably clears loading', async () => {
    const f = fixture(); f.dependencies.getInvoke.mockResolvedValue(null as any)
    expect(await f.actions.value!.submit('one', 5, 'feedback')).toBe(false)
    expect(f.trace.loading).toBe(false); expect(f.dependencies.report).toHaveBeenCalledWith('unavailable')
  })
})
