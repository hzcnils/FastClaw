// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { reactive, ref } from 'vue'
import { flushPromises } from '@vue/test-utils'
import type { Message } from '@/types'
import type { ExportControl } from '@/utils/exportControl'
import { createChatTabState } from './tabState'
import { createChatContext, createChatContextBinding } from './contextBinding'
import { createMessageExportBinding, type MessageExportFormat } from './messageExportBinding'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).reverse().forEach(fn => fn()))
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (reason?: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no }); return { promise, resolve, reject }
}
function fixture() {
  const owner = createChatTabState('export'); cleanup.push(() => owner.release())
  const store = reactive({ currentConversationId: 'a' as string | null, messages: [{ id: 'one', conversation_id: 'a', content: 'original', status: 'completed' }] as Message[] })
  const context = createChatContext({ instanceId: owner.instanceId, path: '/chat/a' }, owner, store)
  const binding = createChatContextBinding<typeof store>(); binding.activate(context); cleanup.push(binding.dispose)
  const gate = deferred<boolean>(), close = vi.fn(), displayed = ref(true)
  const dependencies = { run: vi.fn((_content: string, _format: MessageExportFormat, _control: ExportControl) => gate.promise),
    loading: vi.fn(() => close), report: vi.fn() }
  const api = createMessageExportBinding(binding, dependencies)
  return { owner, store, context, binding, gate, close, displayed, dependencies, api }
}
describe('message export ownership', () => {
  it.each(['md', 'docx', 'pdf'] as const)('exports a captured %s snapshot and reports success to its view', async format => {
    const f = fixture(), task = f.api.actions.value!.export('one', format)
    expect(f.dependencies.run.mock.calls[0].slice(0, 2)).toEqual(['original', format])
    expect(f.dependencies.run.mock.calls[0][2].isCurrent()).toBe(true)
    f.gate.resolve(true); await task; expect(f.dependencies.report).toHaveBeenCalledWith('success', format)
    expect(f.close).toHaveBeenCalledTimes(format === 'md' ? 0 : 1)
  })
  it.each(['tab', 'conversation', 'owner', 'release', 'remove', 'edit', 'revert', 'stream', 'display'] as const)(
    'invalidates pending export on %s, promptly closes only its loading notice and ignores late errors', async change => {
      const f = fixture(), run = f.api.captureExport('one', () => f.displayed.value), task = run('docx')
      const control = f.dependencies.run.mock.calls[0][2]
      if (change === 'tab') { f.binding.deactivate(); f.binding.activate(f.context) }
      if (change === 'conversation') f.store.currentConversationId = 'b'
      if (change === 'owner') f.owner.invalidateContext()
      if (change === 'release') f.owner.release()
      if (change === 'remove') f.store.messages = []
      if (change === 'edit' || change === 'revert') f.store.messages[0].content = 'new'
      if (change === 'revert') f.store.messages[0].content = 'original'
      if (change === 'stream') f.store.messages[0].status = 'streaming'
      if (change === 'display') { f.displayed.value = false; f.displayed.value = true }
      await task; expect(control.isCurrent()).toBe(false); expect(f.close).toHaveBeenCalledOnce()
      f.gate.reject('late'); await flushPromises(); expect(f.dependencies.report).not.toHaveBeenCalled()
    })
  it('blocks old callbacks after returning and rejects missing messages or unknown formats', async () => {
    const f = fixture(), old = f.api.actions.value!, run = f.api.captureExport('one', () => true)
    f.binding.deactivate(); f.binding.activate(f.context); await old.export('one', 'pdf'); await run('pdf')
    await f.api.actions.value!.export('missing', 'md'); await f.api.actions.value!.export('one', 'exe')
    expect(f.dependencies.run).not.toHaveBeenCalled()
  })
  it.each(['empty', 'streaming'] as const)('reports %s content without opening a save dialog', async kind => {
    const f = fixture()
    if (kind === 'empty') f.store.messages[0].content = ' '
    else f.store.messages[0].status = 'streaming'
    await f.api.actions.value!.export('one', 'md')
    expect(f.dependencies.report).toHaveBeenCalledWith(kind); expect(f.dependencies.run).not.toHaveBeenCalled()
  })
  it('deduplicates all formats and both legacy/workspace entry points while pending', async () => {
    const f = fixture(), task = f.api.actions.value!.export('one', 'docx')
    await f.api.captureExport('one', () => true)('pdf'); await f.api.actions.value!.export('one', 'md')
    expect(f.dependencies.run).toHaveBeenCalledOnce(); f.gate.resolve(false); await task
    expect(f.dependencies.report).not.toHaveBeenCalled(); expect(f.close).toHaveBeenCalledOnce()
  })
  it('reports errors in the original view, releases the lock and allows retry', async () => {
    const f = fixture(), error = new Error('write failed'), task = f.api.actions.value!.export('one', 'pdf')
    f.gate.reject(error); await task
    expect(f.dependencies.report).toHaveBeenCalledWith('error', 'pdf', error); expect(f.close).toHaveBeenCalledOnce()
    f.dependencies.run.mockResolvedValue(true); await f.api.actions.value!.export('one', 'pdf')
    expect(f.dependencies.run).toHaveBeenCalledTimes(2)
  })
  it('does not let stale cleanup clear a new request lock', async () => {
    const f = fixture(), old = f.api.actions.value!.export('one', 'pdf')
    f.binding.deactivate(); f.binding.activate(f.context)
    const current = f.api.actions.value!.export('one', 'pdf'); await old
    await f.api.actions.value!.export('one', 'pdf'); expect(f.dependencies.run).toHaveBeenCalledTimes(2)
    f.gate.resolve(true); await current; expect(f.dependencies.report).toHaveBeenCalledTimes(1)
  })
})
