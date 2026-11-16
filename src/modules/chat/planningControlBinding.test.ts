// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import * as Vue from 'vue'
import { defineComponent, nextTick, reactive } from 'vue'
import { mount } from '@vue/test-utils'
import { compile } from '@vue/compiler-dom'
import view from '@/views/ChatView.vue?raw'
import type { Message } from '@/types'
import { createChatTabState } from './tabState'
import { createChatContext, createChatContextBinding } from './contextBinding'
import { createRemotePlanConfirmationBinding, createWorkspaceAbortBinding } from './planningControlBinding'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).reverse().forEach(fn => fn()))
function deferred() {
  let resolve!: (value?: unknown) => void, reject!: (error: unknown) => void
  const promise = new Promise<unknown>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}
function fixture() {
  const make = (id: string) => {
    const owner = createChatTabState(id); cleanup.push(() => owner.release())
    return createChatContext({ path: '/chat/' + id, instanceId: id }, owner,
      reactive({ currentConversationId: id as string | null, isLoading: true, stopStream: vi.fn(async () => {}),
        messages: [{ id: 'user-' + id, role: 'user', conversation_id: id, content: 'question' }] as Message[] }))
  }
  const a = make('a'), b = make('b'), binding = createChatContextBinding<typeof a.store>()
  binding.activate(a); cleanup.push(binding.dispose)
  const runtimes = reactive<Record<string, Record<string, any>>>({
    a: { isPreviewing: true, plannedTree: { id: 'tree-a' }, executionMode: 'manual' },
    b: { isPreviewing: true, plannedTree: { id: 'tree-b' }, executionMode: 'manual' },
  })
  const gate = deferred(), send = vi.fn(() => gate.promise), error = vi.fn()
  const runtime = (context: typeof a) => runtimes[context.store.currentConversationId || ''] || null
  const remote = createRemotePlanConfirmationBinding(binding, runtime, send, error)
  const stop = vi.fn(() => gate.promise), cancel = vi.fn()
  const abort = createWorkspaceAbortBinding(binding, { cancelPending: cancel, stopPlan: stop, reportError: error })
  const plan = (context = a, id = 'entry-a') => {
    context.owner.state.dialogueCharacter.value.taskPlan = true
    context.owner.state.fastPlanEntries.value = [{ id, kind: 'embedded', renderer: 'fastplan', status: 'active', data: {} }]
  }
  return { a, b, binding, runtimes, gate, send, error, remote, runtime, stop, cancel, abort, plan }
}
const confirm = { value: 'confirm', action: 'confirm' }, cancelOption = { value: 'cancel', action: 'cancel' }

describe('remote planning confirmation ownership', () => {
  it('sends the backend wait ID and rejects callbacks from a same-node earlier wait', async () => {
    const f = fixture()
    Object.assign(f.runtimes.a, { controlProtocol: 1, controlWaitId: 'wait-1' })
    const old = f.remote.panel.value!
    f.runtimes.a.controlWaitId = 'wait-2'
    await old.events['confirmation-select'](confirm); expect(f.send).not.toHaveBeenCalled()
    const work = f.remote.panel.value!.events['confirmation-select'](confirm)
    expect(f.send).toHaveBeenCalledExactlyOnceWith('a', 'confirm', 'wait-2'); f.gate.resolve(); await work
  })
  it('does not expose a protocol-v1 panel without an installed wait ID', () => {
    const f = fixture(); f.runtimes.a.controlProtocol = 1
    expect(f.remote.panel.value).toBeNull()
  })
  it('does not send a conflicting action after a protocol-v1 wait was accepted', async () => {
    const f = fixture(); Object.assign(f.runtimes.a, { controlProtocol: 1, controlWaitId: 'wait-1' })
    const select = f.remote.panel.value!.events['confirmation-select'], work = select(confirm)
    f.gate.resolve(); await work; await select(cancelOption)
    expect(f.send).toHaveBeenCalledOnce()
  })
  it('can retry a failed acknowledgement with the exact same wait ID', async () => {
    const f = fixture(); Object.assign(f.runtimes.a, { controlProtocol: 1, controlWaitId: 'wait-1' })
    const select = f.remote.panel.value!.events['confirmation-select'], work = select(confirm)
    f.gate.reject(new Error('ack lost')); await work
    f.send.mockResolvedValueOnce({ ok: true, duplicate: true }); await select(confirm)
    expect(f.send.mock.calls).toEqual([['a', 'confirm', 'wait-1'], ['a', 'confirm', 'wait-1']])
  })
  it('dispatches to the original conversation and coalesces pending/conflicting/repeated operations', async () => {
    const f = fixture(), p = f.remote.panel.value!
    const work = p.events['confirmation-select'](confirm)
    await p.events['confirmation-select'](confirm); await p.events['confirmation-select'](cancelOption)
    expect(f.send).toHaveBeenCalledExactlyOnceWith('a', 'confirm'); expect(f.remote.panel.value!.pending).toBe(true)
    f.gate.resolve(); await work; await p.events['confirmation-select'](confirm)
    expect(f.send).toHaveBeenCalledOnce(); expect(f.remote.panel.value!.pending).toBe(false)
    await p.events['confirmation-select'](cancelOption)
    expect(f.send).toHaveBeenLastCalledWith('a', 'cancel')
    await p.events['confirmation-select'](confirm); expect(f.send).toHaveBeenCalledTimes(2)
  })
  it.each(['switch', 'deactivate', 'dispose', 'invalidate', 'release', 'conversation', 'turn', 'tree', 'completed', 'local'] as const)(
    'rejects an old callback after %s', async reason => {
      const f = fixture(), select = f.remote.panel.value!.events['confirmation-select']
      if (reason === 'switch') { f.binding.activate(f.b); f.binding.activate(f.a) }
      if (reason === 'deactivate') f.binding.deactivate()
      if (reason === 'dispose') f.binding.dispose()
      if (reason === 'invalidate') f.a.owner.invalidateContext()
      if (reason === 'release') f.a.owner.release()
      if (reason === 'conversation') f.a.store.currentConversationId = 'b'
      if (reason === 'turn') f.a.store.messages.push({ ...f.a.store.messages[0], id: 'next-user' })
      if (reason === 'tree') f.runtimes.a.plannedTree.id = 'next-tree'
      if (reason === 'completed') f.runtimes.a.treeCompleted = true
      if (reason === 'local') f.plan()
      await select(confirm); expect(f.send).not.toHaveBeenCalled()
    })
  it('ignores forged/mismatched/absent actions and allows valid confirmation', async () => {
    const f = fixture(), select = f.remote.panel.value!.events['confirmation-select']
    await select({ action: 'delete', value: 'confirm' }); await select({ action: 'confirm' }); await select({ value: 'resume' })
    expect(f.send).not.toHaveBeenCalled()
    const work = select({ value: 'confirm' }); f.gate.resolve(); await work
    expect(f.send).toHaveBeenCalledExactlyOnceWith('a', 'confirm')
  })
  it('supports step continue/cancel and rejects the previous node after progress', async () => {
    const f = fixture()
    f.runtimes.a = { executionMode: 'step', isPaused: true, currentTreeId: 'tree-a', lastCompletedNodeId: 'n1' }
    const old = f.remote.panel.value!
    expect(old.confirmation.kind).toBe('step')
    const work = old.events['confirmation-select']({ value: 'continue', action: 'resume' }); f.gate.resolve(); await work
    f.runtimes.a.lastCompletedNodeId = 'n2'
    await old.events['confirmation-select']({ value: 'stop', action: 'cancel' })
    expect(f.send).toHaveBeenCalledExactlyOnceWith('a', 'resume')
    await f.remote.panel.value!.events['confirmation-select']({ value: 'stop', action: 'cancel' })
    expect(f.send).toHaveBeenLastCalledWith('a', 'cancel')
  })
  it('keeps a pending receipt across eviction/remount, without cancelling an accepted IPC', async () => {
    const f = fixture(), work = f.remote.panel.value!.events['confirmation-select'](confirm)
    f.binding.dispose()
    const binding = createChatContextBinding<typeof f.a.store>(); binding.activate(f.a); cleanup.push(binding.dispose)
    const restored = createRemotePlanConfirmationBinding(binding, f.runtime, f.send, f.error)
    expect(restored.panel.value!.pending).toBe(true)
    await restored.panel.value!.events['confirmation-select'](confirm); expect(f.send).toHaveBeenCalledOnce()
    f.gate.resolve(); await work
    await restored.panel.value!.events['confirmation-select'](confirm); expect(f.send).toHaveBeenCalledOnce()
    expect(f.a.owner.disposed).toBe(false)
  })
  it('allows retry after failure and only reports errors to the still-current wait', async () => {
    const f = fixture(), select = f.remote.panel.value!.events['confirmation-select']
    const work = select(confirm); f.gate.reject(new Error('offline')); await work
    expect(f.error).toHaveBeenCalledOnce(); expect(f.a.owner.state.remotePlanControl.value).toBeNull()
    f.send.mockResolvedValueOnce(undefined); await select(confirm)
    expect(f.send).toHaveBeenCalledTimes(2)
  })
  it.each(['switch', 'step', 'reset'] as const)('does not toast into another UI after %s', async reason => {
    const f = fixture(), work = f.remote.panel.value!.events['confirmation-select'](confirm)
    if (reason === 'switch') f.binding.activate(f.b)
    if (reason === 'step') f.runtimes.a.plannedTree.id = 'next'
    if (reason === 'reset') f.a.owner.resetSelectedConversation()
    f.gate.reject(new Error('late')); await work
    expect(f.error).not.toHaveBeenCalled(); expect(f.b.owner.state.remotePlanControl.value).toBeNull()
  })
  it('does not overwrite a newer wait receipt when the old request completes', async () => {
    const f = fixture(), first = f.remote.panel.value!.events['confirmation-select'](confirm)
    f.runtimes.a.plannedTree.id = 'next'
    const next = deferred(); f.send.mockReturnValueOnce(next.promise)
    const second = f.remote.panel.value!.events['confirmation-select'](confirm)
    const receipt = f.a.owner.state.remotePlanControl.value
    f.gate.resolve(); await first
    expect(f.a.owner.state.remotePlanControl.value).toBe(receipt); expect(receipt?.status).toBe('pending')
    next.resolve(); await second; expect(receipt?.status).toBe('sent')
  })
  it('does not use runtime object identity as a wait identity or reset receipts on progress updates', async () => {
    const f = fixture(), p = f.remote.panel.value!, work = p.events['confirmation-select'](confirm)
    f.runtimes.a = { ...f.runtimes.a, planningLogs: ['more'] }
    expect(f.remote.panel.value!.key).toBe(p.key)
    await f.remote.panel.value!.events['confirmation-select'](confirm); expect(f.send).toHaveBeenCalledOnce()
    f.gate.resolve(); await work
  })
  it('hides confirmation for no conversation, local planning, or a non-waiting runtime', () => {
    const f = fixture(); f.a.store.currentConversationId = null; expect(f.remote.panel.value).toBeNull()
    f.a.store.currentConversationId = 'a'; f.plan(); expect(f.remote.panel.value).toBeNull()
    f.a.owner.state.fastPlanEntries.value = []; f.runtimes.a.isPreviewing = false; expect(f.remote.panel.value).toBeNull()
  })
})

describe('workspace abort target ownership', () => {
  it('stops the fallback entry without requiring a live runtime or changing composer selection', async () => {
    const f = fixture(); f.plan(); const work = f.abort.value!()
    expect(f.stop).toHaveBeenCalledExactlyOnceWith('a', 'entry-a')
    expect(f.a.owner.state.activeFastPlanEntryId.value).toBeNull()
    f.binding.activate(f.b); f.gate.resolve(); await work
    expect(f.error).not.toHaveBeenCalled(); expect(f.a.store.stopStream).not.toHaveBeenCalled()
  })
  it('cancels pending submission on the captured owner before touching planning or streaming', async () => {
    const f = fixture(); f.plan(); f.a.owner.state.pendingAutoSend.value = true
    await f.abort.value!(); expect(f.cancel).toHaveBeenCalledExactlyOnceWith(f.a.owner)
    expect(f.stop).not.toHaveBeenCalled(); expect(f.a.store.stopStream).not.toHaveBeenCalled()
  })
  it('uses the active entry, not another entry, and falls back to ordinary stream when none exists', async () => {
    const f = fixture(); f.plan(); f.a.owner.state.fastPlanEntries.value.push({ id: 'selected', kind: 'embedded', renderer: 'fastplan', data: {} })
    f.a.owner.state.activeFastPlanEntryId.value = 'selected'; const work = f.abort.value!()
    expect(f.stop).toHaveBeenCalledWith('a', 'selected'); f.gate.resolve(); await work
    f.a.owner.state.fastPlanEntries.value = []; await f.abort.value!()
    expect(f.a.store.stopStream).toHaveBeenCalledExactlyOnceWith('a')
  })
  it.each(['switch', 'entry', 'turn', 'mode', 'pending', 'conversation', 'invalidate'] as const)(
    'does not stop the replacement operation after %s', async reason => {
      const f = fixture(); f.plan(); const abort = f.abort.value!
      if (reason === 'switch') f.binding.activate(f.b)
      if (reason === 'entry') f.plan(f.a, 'new-entry')
      if (reason === 'turn') f.a.store.messages.push({ ...f.a.store.messages[0], id: 'new-user' })
      if (reason === 'mode') f.a.owner.state.dialogueCharacter.value.taskPlan = false
      if (reason === 'pending') f.a.owner.state.pendingAutoSend.value = true
      if (reason === 'conversation') f.a.store.currentConversationId = 'b'
      if (reason === 'invalidate') f.a.owner.invalidateContext()
      await abort(); expect(f.stop).not.toHaveBeenCalled(); expect(f.cancel).not.toHaveBeenCalled()
    })
  it('uses the newly bound store for ordinary abort and catches failures', async () => {
    const f = fixture(); f.binding.activate(f.b)
    f.b.store.stopStream.mockRejectedValueOnce(new Error('stop failed'))
    await f.abort.value!(); expect(f.b.store.stopStream).toHaveBeenCalledWith('b')
    expect(f.a.store.stopStream).not.toHaveBeenCalled(); expect(f.error).toHaveBeenCalledOnce()
    f.b.store.isLoading = false; await f.abort.value!(); expect(f.b.store.stopStream).toHaveBeenCalledOnce()
  })
  it('suppresses late stop errors after switch', async () => {
    const f = fixture(); f.plan(); const work = f.abort.value!(); f.binding.activate(f.b)
    f.gate.reject(new Error('late')); await work; expect(f.error).not.toHaveBeenCalled()
  })
})

it('actual remote confirmation template captures old handlers before the DOM patch', async () => {
  const f = fixture(), start = view.indexOf('<FastPlanTaskList\n', view.indexOf('<template #composer-before>'))
  const fragment = view.slice(start, view.indexOf('/>', start) + 2)
  expect(fragment).toContain('v-on="remotePlanPanel.events"')
  const render = new Function('Vue', compile(fragment, { mode: 'function' }).code)(Vue)
  const Card = defineComponent({ emits: ['confirmation-select'], template: '<button>confirm</button>' })
  const wrapper = mount(defineComponent({ components: { FastPlanTaskList: Card }, render,
    setup: () => ({ remotePlanPanel: f.remote.panel, remoteComposerPlan: { status: 'planning', phases: [] } }) }))
  cleanup.push(() => wrapper.unmount())
  const old = wrapper.findComponent(Card)
  f.binding.activate(f.b); old.vm.$emit('confirmation-select', confirm)
  expect(f.send).not.toHaveBeenCalled()
  await nextTick(); wrapper.findComponent(Card).vm.$emit('confirmation-select', confirm)
  expect(f.send).toHaveBeenCalledExactlyOnceWith('b', 'confirm'); f.gate.resolve(); await nextTick()
})
