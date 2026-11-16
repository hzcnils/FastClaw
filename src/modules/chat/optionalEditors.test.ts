// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { nextTick } from 'vue'
import { createI18n } from 'vue-i18n'
import { ElDialog, ElButton, ElMessageBox, ElSelect, ElOption } from 'element-plus'
import { EditorView } from '@codemirror/view'
import EditMessageDialog from '@/components/chat/EditMessageDialog.vue'
import DocumentEditor from '@/components/editor/DocumentEditor.vue'

vi.mock('@/utils/exportMessage', () => ({ exportMarkdown: vi.fn(), exportWord: vi.fn(), exportPdf: vi.fn() }))
vi.mock('@/composables/useMarkdown', () => ({ useMarkdown: () => ({ render: (text: string) => text }) }))
const i18n = () => createI18n({ legacy: false, locale: 'test', missingWarn: false, fallbackWarn: false, messages: { test: {} } })
const mounted: Array<{ unmount(): void }> = []
function dialog(props = {}) {
  const wrapper = mount(EditMessageDialog, { attachTo: document.body,
    props: { modelValue: true, content: 'original', draftContent: 'unsaved', ...props },
    global: { plugins: [i18n()], components: { ElDialog, ElButton } } })
  mounted.push(wrapper); return wrapper
}
function editor() {
  const element = document.querySelector('.cm-editor') as HTMLElement
  expect(element).toBeTruthy()
  return EditorView.findFromDOM(element)!
}
afterEach(async () => {
  for (const wrapper of mounted.splice(0)) wrapper.unmount()
  await flushPromises(); document.body.innerHTML = ''; vi.restoreAllMocks()
})

describe('real optional editor mounts', () => {
  it('blocks keyboard saves while pending, then saves the latest draft after completion', async () => {
    const wrapper = dialog({ loading: true })
    await flushPromises()
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: true }))
    expect(wrapper.emitted('save')).toBeUndefined()
    editor().dispatch({ changes: { from: 0, to: editor().state.doc.length, insert: 'typed during save' } })
    await wrapper.setProps({ loading: false, content: 'previous save' })
    expect(editor().state.doc.toString()).toBe('typed during save')
    expect(document.querySelector('.qe-dirty')).not.toBeNull()
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: true }))
    expect(wrapper.emitted('save')?.at(-1)).toEqual(['typed during save'])
  })

  it('updates the saved baseline without rebuilding CodeMirror or losing selection', async () => {
    const wrapper = dialog(); await flushPromises()
    const instance = editor(); instance.dispatch({ selection: { anchor: 2 } })
    await wrapper.setProps({ content: 'unsaved' })
    expect(editor()).toBe(instance); expect(editor().state.selection.main.anchor).toBe(2)
    expect(document.querySelector('.qe-dirty')).toBeNull()
  })

  it('initializes CodeMirror on first visible mount with draft and original dirty baseline', async () => {
    const wrapper = dialog()
    await flushPromises(); await nextTick()
    expect(editor().state.doc.toString()).toBe('unsaved')
    expect(document.querySelector('.qe-dirty')?.textContent).toContain('未保存')
    editor().dispatch({ changes: { from: 0, to: editor().state.doc.length, insert: 'latest edit' } })
    expect(wrapper.emitted('draft-change')?.at(-1)).toEqual(['latest edit'])
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: true }))
    expect(wrapper.emitted('save')?.at(-1)).toEqual(['latest edit'])
  })

  it('does not construct an editor after immediate unmount and removes its key listener', async () => {
    const add = vi.spyOn(window, 'addEventListener'), remove = vi.spyOn(window, 'removeEventListener')
    const wrapper = dialog()
    const keyListeners = add.mock.calls.filter(([name]) => name === 'keydown').map(([, callback]) => callback)
    wrapper.unmount(); mounted.splice(mounted.indexOf(wrapper), 1)
    await flushPromises()
    expect(document.querySelector('.cm-editor')).toBeNull()
    expect(keyListeners.length).toBeGreaterThan(0)
    for (const callback of keyListeners) expect(remove.mock.calls.some(([name, fn]) => name === 'keydown' && fn === callback)).toBe(true)
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: true }))
    expect(wrapper.emitted('save')).toBeUndefined()
  })

  it('ignores an unsaved-close confirmation resolved after the old dialog unmounts', async () => {
    let resolve!: () => void
    const pending = new Promise<void>(yes => { resolve = yes })
    vi.spyOn(ElMessageBox, 'confirm').mockReturnValue(pending as any)
    const wrapper = dialog()
    await flushPromises()
    ;(document.querySelector('.qe-cancel-btn') as HTMLElement).click()
    expect(ElMessageBox.confirm).toHaveBeenCalledTimes(1)
    wrapper.unmount(); mounted.splice(mounted.indexOf(wrapper), 1)
    resolve(); await flushPromises()
    expect(wrapper.emitted('update:modelValue')).toBeUndefined()
  })

  it('preserves real document save and externally selected version behavior across remount', async () => {
    const props = { content: 'version one', versions: [{ label: 'V1', content: 'version one', timestamp: '' },
      { label: 'V2', content: 'version two', timestamp: '' }], activeVersionIdx: 0 }
    const global = { plugins: [i18n()], components: { ElSelect, ElOption } }
    const wrapper = mount(DocumentEditor, { props, global }); mounted.push(wrapper)
    await wrapper.get('textarea').setValue('unsaved document')
    expect(wrapper.emitted('save')?.at(-1)).toEqual(['unsaved document'])
    wrapper.getComponent(ElSelect).vm.$emit('update:modelValue', 1)
    expect(wrapper.emitted('version-change')?.at(-1)).toEqual([1])
    await wrapper.setProps({ content: 'version two', activeVersionIdx: 1 })
    expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('version two')
    wrapper.unmount(); mounted.splice(mounted.indexOf(wrapper), 1)
    const restored = mount(DocumentEditor, { props: { ...props, content: 'unsaved document' }, global })
    mounted.push(restored)
    expect((restored.get('textarea').element as HTMLTextAreaElement).value).toBe('unsaved document')
  })

  it('saves real document input synchronously before unmount and never echoes external content', async () => {
    const save = vi.fn()
    const wrapper = mount(DocumentEditor, { props: { content: 'original', onSave: save }, global: { plugins: [i18n()] } })
    mounted.push(wrapper)
    await flushPromises()
    expect(save).not.toHaveBeenCalled()
    await wrapper.setProps({ content: 'external version' })
    expect(save).not.toHaveBeenCalled()
    const input = wrapper.get('textarea').element as HTMLTextAreaElement
    input.value = 'last keystroke'; input.dispatchEvent(new Event('input', { bubbles: true }))
    expect(save).toHaveBeenLastCalledWith('last keystroke')
    wrapper.unmount(); mounted.splice(mounted.indexOf(wrapper), 1)
    await flushPromises(); expect(save).toHaveBeenCalledTimes(1)
  })
})
