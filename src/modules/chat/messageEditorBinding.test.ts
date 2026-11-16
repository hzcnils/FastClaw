// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { reactive } from 'vue'
import type { Message } from '@/types'
import { createChatTabState } from './tabState'
import { createChatContext, createChatContextBinding } from './contextBinding'
import { createMessageEditorBinding } from './messageEditorBinding'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).reverse().forEach(fn => fn()))
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (error: Error) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no }); return { promise, resolve, reject }
}
function fixture() {
  const make = (id: string) => {
    const owner = createChatTabState(id); cleanup.push(() => owner.release())
    const pending = deferred<boolean>()
    const store = reactive({ currentConversationId: id as string | null, isLoading: false,
      messages: [{ id: 'answer', conversation_id: id, role: 'assistant', content: id + ':original' }] as Message[],
      updateMessageContent: vi.fn((_id: string, _content: string, _valid?: () => boolean) => pending.promise) })
    return { ...createChatContext({ path: '/chat/' + id, instanceId: id }, owner, store), pending }
  }
  const a = make('a'), b = make('b'), visible = reactive({ path: '/chat/a' })
  const binding = createChatContextBinding<typeof a.store>(ctx => ctx.identity.path === visible.path)
  binding.activate(a); cleanup.push(binding.dispose)
  const report = vi.fn(), editor = createMessageEditorBinding(binding, report)
  const switchTo = (ctx: typeof a) => { visible.path = ctx.identity.path; binding.activate(ctx) }
  const open = () => { expect(editor.open('answer')).toBe(true); return editor.panel.value! }
  return { a, b, binding, editor, report, switchTo, open, visible }
}

describe('message editor presentation and command ownership', () => {
  it('retains key/events while typing, and captures the original owner', () => {
    const f = fixture(), first = f.open(); first.events['draft-change']('unsaved')
    expect(f.editor.panel.value!.key).toBe(first.key); expect(f.editor.panel.value!.events).toBe(first.events)
    f.switchTo(f.b); expect(f.editor.panel.value).toBeNull(); const other = f.open()
    first.events['draft-change']('late'); first.events.cancel()
    expect(f.a.owner.state.editingMessage.value!.draftContent).toBe('unsaved')
    expect(f.editor.panel.value!.content).toBe('b:original'); expect(f.editor.panel.value!.key).not.toBe(first.key)
    other.events['draft-change']('B'); expect(f.b.owner.state.editingMessage.value!.draftContent).toBe('B')
  })
  it('recreates same-message presentation after A → B → A without reviving saved handlers', async () => {
    const f = fixture(), first = f.open(); first.events['draft-change']('unsaved')
    f.switchTo(f.b); f.switchTo(f.a)
    expect(f.editor.panel.value!.draftContent).toBe('unsaved'); expect(f.editor.panel.value!.key).not.toBe(first.key)
    first.events.cancel(); await first.events.save('late')
    expect(f.a.store.updateMessageContent).not.toHaveBeenCalled(); expect(f.editor.panel.value).not.toBeNull()
  })
  it('reopening the same message gets a new instance key and rejects the old close', () => {
    const f = fixture(), first = f.open(); first.events.cancel(); const second = f.open()
    expect(second.key).not.toBe(first.key); first.events['update:modelValue'](false)
    expect(f.editor.panel.value!.key).toBe(second.key)
  })
  it.each(['conversation', 'invalidate', 'release', 'hidden', 'deactivate', 'dispose'] as const)('rejects saved UI handlers after %s', async reason => {
    const f = fixture(), first = f.open()
    if (reason === 'conversation') f.a.store.currentConversationId = 'changed'
    if (reason === 'invalidate') f.a.owner.invalidateContext()
    if (reason === 'release') f.a.owner.release()
    if (reason === 'hidden') f.visible.path = '/other'
    if (reason === 'deactivate') f.binding.deactivate()
    if (reason === 'dispose') f.binding.dispose()
    first.events['draft-change']('late'); first.events.cancel(); await first.events.save('late')
    expect(f.a.owner.state.editingMessage.value!.draftContent).toBeUndefined()
    expect(f.a.store.updateMessageContent).not.toHaveBeenCalled()
  })
  it('deduplicates save, completes original owner in the background, and leaves another owner loading', async () => {
    const f = fixture(), first = f.open(); first.events['draft-change']('saved A')
    const saveA = first.events.save('saved A'); await first.events.save('duplicate')
    expect(f.editor.panel.value!.loading).toBe(true); expect(f.a.store.updateMessageContent).toHaveBeenCalledOnce()
    f.switchTo(f.b); const second = f.open(); second.events['draft-change']('saved B'); const saveB = second.events.save('saved B')
    expect(f.a.store.updateMessageContent.mock.calls[0][2]!()).toBe(true)
    f.a.pending.resolve(true); await saveA
    expect(f.a.owner.state.editingMessage.value).toBeNull(); expect(f.editor.panel.value!.loading).toBe(true)
    f.b.pending.resolve(true); await saveB; expect(f.editor.panel.value).toBeNull()
  })
  it('keeps business save valid through binding disposal/remount and preserves the owner loading lock', async () => {
    const f = fixture(), first = f.open(); first.events['draft-change']('saved'); const save = first.events.save('saved')
    f.binding.dispose()
    const binding = createChatContextBinding<typeof f.a.store>(); binding.activate(f.a); cleanup.push(binding.dispose)
    const remounted = createMessageEditorBinding(binding, vi.fn())
    expect(remounted.panel.value!.loading).toBe(true)
    await remounted.panel.value!.events.save('duplicate'); expect(f.a.store.updateMessageContent).toHaveBeenCalledOnce()
    f.a.pending.resolve(true); await save; expect(remounted.panel.value).toBeNull()
  })
  it.each(['reject', 'false'] as const)('retains draft and releases loading when persistence returns %s', async result => {
    const f = fixture(), first = f.open(); first.events['draft-change']('unsaved')
    const save = first.events.save('unsaved')
    if (result === 'reject') f.a.pending.reject(new Error('disk failed')); else f.a.pending.resolve(false)
    await save
    expect(f.editor.panel.value).toMatchObject({ draftContent: 'unsaved', loading: false, content: 'a:original' })
    expect(f.report).toHaveBeenCalledTimes(result === 'reject' ? 1 : 0)
  })
  it('does not display a late error on another tab or discard the hidden draft', async () => {
    const f = fixture(), first = f.open(); first.events['draft-change']('unsaved')
    const save = first.events.save('unsaved'); f.switchTo(f.b); f.a.pending.reject(new Error('late')); await save
    expect(f.report).not.toHaveBeenCalled(); f.switchTo(f.a)
    expect(f.editor.panel.value).toMatchObject({ draftContent: 'unsaved', loading: false })
  })
  it('preserves further typing while advancing the successfully saved baseline', async () => {
    const f = fixture(), first = f.open(); first.events['draft-change']('sent'); const save = first.events.save('sent')
    first.events['draft-change']('typed later'); f.a.pending.resolve(true); await save
    expect(f.editor.panel.value).toMatchObject({ content: 'sent', draftContent: 'typed later', loading: false })
    expect(f.editor.panel.value!.key).toBe(first.key)
  })
  it('old completion cannot close a replacement editor or clear a new save token', async () => {
    const f = fixture(), first = f.open(); first.events['draft-change']('sent'); const save = first.events.save('sent')
    f.a.owner.state.editingMessage.value = { id: 'other', content: 'other' }
    f.a.owner.state.messageEditSaveId.value = 'new-save'
    f.a.pending.resolve(true); await save
    expect(f.editor.panel.value).toMatchObject({ content: 'other', loading: true })
    expect(f.a.owner.state.messageEditSaveId.value).toBe('new-save')
  })
  it.each(['conversation', 'invalidate', 'release', 'delete', 'content'] as const)('invalidates a captured save command after %s', async reason => {
    const f = fixture(), first = f.open(); first.events['draft-change']('sent'); const save = first.events.save('sent')
    if (reason === 'conversation') f.a.store.currentConversationId = 'other'
    if (reason === 'invalidate') f.a.owner.invalidateContext()
    if (reason === 'release') f.a.owner.release()
    if (reason === 'delete') f.a.store.messages = []
    if (reason === 'content') f.a.store.messages[0].content = 'newer'
    expect(f.a.store.updateMessageContent.mock.calls[0][2]!()).toBe(false)
    f.a.pending.resolve(false); await save
    expect(f.a.owner.state.editingMessage.value!.draftContent).toBe('sent')
    expect(f.a.owner.state.messageEditSaveId.value).toBeNull()
  })
  it('does not open missing/user/busy messages or save a changed source', async () => {
    const f = fixture(); expect(f.editor.open('missing')).toBe(false)
    f.a.store.messages[0].role = 'user'; expect(f.editor.open('answer')).toBe(false)
    f.a.store.messages[0].role = 'assistant'; f.a.store.isLoading = true; expect(f.editor.open('answer')).toBe(false)
    f.a.store.isLoading = false; const first = f.open(); f.a.store.messages[0].content = 'changed'
    await first.events.save('old'); expect(f.a.store.updateMessageContent).not.toHaveBeenCalled()
  })
})
