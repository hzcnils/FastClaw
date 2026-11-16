// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { reactive } from 'vue'
import { createChatContext, createChatContextBinding } from './contextBinding'
import { createChatTabState } from './tabState'
import { createDocumentEditorBinding } from './documentEditorBinding'

const cleanup: (() => void)[] = []
afterEach(() => { cleanup.splice(0).reverse().forEach(fn => fn()); localStorage.clear() })
function fixture() {
  const make = (id: string) => {
    const owner = createChatTabState(id); cleanup.push(() => owner.release())
    const store = reactive({ currentConversationId: id as string | null })
    return createChatContext({ path: '/chat/' + id, instanceId: id }, owner, store)
  }
  const a = make('a'), b = make('b'), visible = reactive({ value: true })
  const binding = createChatContextBinding<typeof a.store>(() => visible.value)
  binding.activate(a); cleanup.push(binding.dispose)
  const editor = createDocumentEditorBinding(binding, { title: () => 'title', panelWidth: () => 400 })
  editor.actions.value!.open('original')
  return { a, b, binding, visible, editor, state: a.owner.state }
}

describe('document editor context and version callbacks', () => {
  it('keeps the same instance/events while typing and retains the draft across Tab activation', () => {
    const f = fixture(), first = f.editor.panel.value!
    first.events.save('typed')
    expect(f.editor.panel.value!.key).toBe(first.key)
    expect(f.editor.panel.value!.events).toBe(first.events)
    f.binding.activate(f.b); expect(f.editor.panel.value).toBeNull()
    f.editor.actions.value!.open('B')
    first.events.save('late'); first.events.close()
    expect(f.state.editorContent.value).toBe('typed')
    expect(f.b.owner.state.editorContent.value).toBe('B')
    f.binding.activate(f.a)
    expect(f.editor.panel.value!.content).toBe('typed')
    expect(f.editor.panel.value!.key).not.toBe(first.key)
    first.events.save('later'); expect(f.state.editorContent.value).toBe('typed')
  })
  it.each(['hidden', 'deactivate', 'conversation', 'invalidate', 'release', 'dispose'] as const)('rejects old UI actions after %s', reason => {
    const f = fixture(), panel = f.editor.panel.value!, actions = f.editor.actions.value!
    if (reason === 'hidden') f.visible.value = false
    if (reason === 'deactivate') f.binding.deactivate()
    if (reason === 'conversation') f.a.store.currentConversationId = 'other'
    if (reason === 'invalidate') f.a.owner.invalidateContext()
    if (reason === 'release') f.a.owner.release()
    if (reason === 'dispose') f.binding.dispose()
    panel.events.save('late'); panel.events.cancel(); panel.events['version-change'](0); actions.open('wrong')
    expect(f.state.editorContent.value).not.toBe('late')
    expect(f.state.editorContent.value).not.toBe('wrong')
    expect(localStorage.getItem('fastclaw:docEdit:other')).toBeNull()
  })
  it('does not revive old callbacks after synchronous close and reopen', () => {
    const f = fixture(), panel = f.editor.panel.value!
    panel.events.close(); f.editor.actions.value!.open('ignored')
    panel.events.save('late'); panel.events.close()
    expect(f.state.editorVisible.value).toBe(true)
    expect(f.state.editorContent.value).toBe('original')
    expect(f.editor.panel.value!.key).not.toBe(panel.key)
  })
  it('selects a version and rejects the previous editor save before rendering', () => {
    const f = fixture()
    f.state.docVersions.value.push({ label: 'V2', content: 'second', timestamp: '' })
    const first = f.editor.panel.value!
    first.events['version-change'](1); first.events.save('late old body')
    expect(f.state.editorContent.value).toBe('second')
    expect(f.editor.panel.value!.key).not.toBe(first.key)
    const saved = JSON.parse(localStorage.getItem('fastclaw:docEdit:a')!)
    expect(saved.activeVersionIdx).toBe(1)
    expect(saved.versions).toHaveLength(2)
  })
  it('does not revive version callbacks after switching away then back', () => {
    const f = fixture()
    f.state.docVersions.value.push({ label: 'V2', content: 'second', timestamp: '' })
    const first = f.editor.panel.value!
    f.editor.actions.value!.select(1); f.editor.actions.value!.select(0)
    first.events.save('late'); expect(f.state.editorContent.value).toBe('original')
  })
  it('rejects old editor and card actions after the runtime replaces the version list', () => {
    const f = fixture(), old = f.editor.panel.value!, actions = f.editor.actions.value!
    f.state.docVersions.value = [{ label: 'V1', content: 'new projection', timestamp: '' }]
    f.state.editorContent.value = 'new projection'
    old.events.save('stale'); actions.select(0); actions.open('stale')
    expect(f.state.editorContent.value).toBe('new projection')
    expect(f.editor.panel.value!.key).not.toBe(old.key)
  })
  it.each([-1, 0.5, 9, NaN, Infinity])('ignores invalid version index %s without mutating', index => {
    const f = fixture(), panel = f.editor.panel.value!
    expect(() => panel.events['version-change'](index)).not.toThrow()
    expect(f.state.activeVersionIdx.value).toBe(0)
    expect(f.state.editorContent.value).toBe('original')
  })
  it('opens a card after closing without discarding the version history', () => {
    const f = fixture()
    f.state.docVersions.value.push({ label: 'V2', content: 'second', timestamp: '' })
    f.editor.panel.value!.events.close(); f.editor.actions.value!.select(1)
    expect(f.state.editorVisible.value).toBe(true)
    expect(f.state.isHistoryCollapsed.value).toBe(true)
    expect(f.state.chatPanelPx.value).toBe(400)
    expect(f.state.docVersions.value).toHaveLength(2)
  })
  it('captures card identity and rejects removed or replaced cards', () => {
    const f = fixture()
    f.state.docEditCardMap.answer = { versionIdx: 0, label: 'V1', timestamp: '' }
    const first = f.editor.cards.value.answer
    f.editor.panel.value!.events.close()
    first(); expect(f.state.editorVisible.value).toBe(true)
    f.editor.panel.value!.events.close()
    delete f.state.docEditCardMap.answer
    first(); expect(f.state.editorVisible.value).toBe(false)
    f.state.docEditCardMap.answer = { versionIdx: 0, label: 'V1', timestamp: '' }
    first(); expect(f.state.editorVisible.value).toBe(false)
    f.editor.cards.value.answer(); expect(f.state.editorVisible.value).toBe(true)
  })
  it('persists the edited draft separately from immutable version content on close', () => {
    const f = fixture(), panel = f.editor.panel.value!
    panel.events.save('unsaved edits'); panel.events.close()
    const saved = JSON.parse(localStorage.getItem('fastclaw:docEdit:a')!)
    expect(saved.draftContent).toBe('unsaved edits')
    expect(saved.versions[0].content).toBe('original')
  })
})
