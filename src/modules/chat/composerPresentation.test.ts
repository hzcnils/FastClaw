// @vitest-environment jsdom
import * as Vue from 'vue'
import { defineComponent, nextTick, ref } from 'vue'
import { mount, type VueWrapper } from '@vue/test-utils'
import { compile } from '@vue/compiler-dom'
import { createI18n } from 'vue-i18n'
import { ElImage, ElIcon } from 'element-plus'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { ChatComposer, type ChatComposerAttachment } from 'chat-renderer'
import ComposerAttachmentCard from '@/components/chat/workspace/ComposerAttachmentCard.vue'
import ComposerContextHints from '@/components/chat/workspace/ComposerContextHints.vue'
import view from '@/views/ChatView.vue?raw'
import { createChatTabState } from './tabState'
import { createChatContext, createChatContextBinding } from './contextBinding'
import { createComposerContextBindings } from './composerContextBindings'

const i18n = () => createI18n({ legacy: false, locale: 'zh', messages: {
  zh: { common: { retry: '重试', cancel: '取消' }, chat: { uploadFailed: '上传失败' } },
} })
const mounted: VueWrapper[] = []
const cleanup: (() => void)[] = []
afterEach(() => { mounted.splice(0).forEach(w => w.unmount()); cleanup.splice(0).forEach(fn => fn()); vi.restoreAllMocks() })
const file = (extra: Partial<ChatComposerAttachment> = {}): ChatComposerAttachment => ({
  id: 'f', kind: 'file', name: 'a.pdf', size: 2048, status: 'complete', ...extra,
})
function card(attachment = file()) {
  const wrapper = mount(ComposerAttachmentCard, { props: { attachment, imagePreviewList: ['one', 'two'], fileIcon: 'pdf.svg' },
    global: { components: { ElImage }, plugins: [i18n()] } })
  mounted.push(wrapper); return wrapper
}
describe('Composer attachment and context presentation', () => {
  it.each([[500, '500 B'], [2048, '2.0 KB'], [20480, '20 KB'], [1048576, '1.0 MB'], [NaN, ''], [undefined, '']])(
    'formats size %s as %s', (size, expected) => {
      expect(card(file({ size })).get('.jcr-composer-attachment__info small').text()).toBe(expected)
    })

  it('renders progress/errors and emits the original click only once without bubbling', async () => {
    const wrapper = card(file({ status: 'processing', progress: 150 }))
    expect(wrapper.get('.jcr-composer-attachment').attributes('aria-busy')).toBe('true')
    expect(wrapper.get('.jcr-composer-attachment__progress i').attributes('style')).toContain('100%')
    await wrapper.setProps({ attachment: file({ status: 'error', error: 'server error' }) })
    expect(wrapper.get('small').text()).toBe('server error')
    const bubbled = vi.fn(); wrapper.element.addEventListener('click', bubbled)
    const retry = new MouseEvent('click', { bubbles: true })
    wrapper.get('.jcr-composer-attachment__retry').element.dispatchEvent(retry)
    expect(wrapper.emitted('retry')).toEqual([[retry]])
    const remove = new MouseEvent('click', { bubbles: true })
    wrapper.get('.jcr-composer-attachment__remove').element.dispatchEvent(remove)
    expect(wrapper.emitted('remove')).toEqual([[remove]])
    expect(bubbled).not.toHaveBeenCalled()
  })

  it('retains image preview list/index and does not turn preview into removal', async () => {
    const wrapper = card(file({ kind: 'image', previewUrl: 'two', status: 'complete' }))
    const preview = wrapper.getComponent(ElImage)
    expect(preview.props('previewSrcList')).toEqual(['one', 'two'])
    expect(preview.props('initialIndex')).toBe(1)
    expect(preview.props('previewTeleported')).toBe(true)
    expect(wrapper.findAll('.jcr-composer-attachment__status--complete')).toHaveLength(1)
    await wrapper.setProps({ imagePreviewList: ['one'] })
    expect(preview.props('initialIndex')).toBe(0)
    expect(wrapper.emitted('remove')).toBeUndefined()
    await wrapper.setProps({ attachment: file({ kind: 'image', status: 'error' }) })
    expect(wrapper.findAll('.jcr-composer-attachment__status--error')).toHaveLength(1)
    expect(wrapper.find('.jcr-composer-attachment__retry').exists()).toBe(false)
  })

  it('keeps quote/skill externally controlled and creates no enclosing layout element', async () => {
    const wrapper = mount(ComposerContextHints, { props: { quotedText: 'quoted', selectedSkill: { type: 'mcp', labelEn: 'weather' } },
      global: { components: { ElIcon }, plugins: [i18n()] } })
    mounted.push(wrapper)
    expect(wrapper.get('.chip-badge').text()).toBe('MCP')
    await wrapper.get('.quote-ref-close').trigger('click'); await wrapper.get('.chip-close').trigger('click')
    expect(wrapper.emitted('clearQuote')).toEqual([[]]); expect(wrapper.emitted('clearSkill')).toEqual([[]])
    expect(wrapper.get('.quote-ref-text').text()).toBe('quoted')
    await wrapper.setProps({ quotedText: '', selectedSkill: null })
    expect(wrapper.find('.quote-ref-bar').exists()).toBe(false)
    expect(wrapper.find('.selected-skill-chip').exists()).toBe(false)
  })
})

describe('actual host slot bindings with the real public Composer', () => {
  function host() {
    const attachment = view.match(/<ComposerAttachmentCard\b[\s\S]*?\/>/)![0]
    const hints = view.match(/<ComposerContextHints\b[\s\S]*?\/>/)![0]
    const draft = ref('draft'), attachments = ref([file({ status: 'error' })])
    const owner = createChatTabState('hints'), binding = createChatContextBinding()
    const chatContext = createChatContext({ path: '/chat/hints', instanceId: 'hints' }, owner, { currentConversationId: null })
    binding.activate(chatContext); cleanup.push(binding.dispose, () => owner.release())
    owner.state.quotedText.value = 'quote'
    owner.state.selectedSkill.value = { type: 'skill', name: 'skill', labelEn: 'skill', labelZh: 'skill', description: '' }
    const { quotedText, selectedSkill } = owner.state
    const composer = createComposerContextBindings(binding, chatContext)
    const submit = vi.fn(), attachmentAction = vi.fn()
    const context = { draft, attachments, quotedText, selectedSkill, submit, attachmentAction,
      composerHintProps: composer.hints, composerHintEvents: composer.hintEvents,
      imagePreviewList: [], composerAttachmentIcon: () => 'pdf.svg', clearQuote: () => { quotedText.value = '' } }
    const code = compile('<ChatComposer v-model="draft" :attachments="attachments" @submit="submit" @attachment-action="attachmentAction">' +
      '<template #above>' + hints + '</template><template #attachment="{ attachment, remove, retry }">' + attachment + '</template></ChatComposer>',
      { mode: 'function' }).code
    const wrapper = mount(defineComponent({ components: { ChatComposer, ComposerAttachmentCard, ComposerContextHints },
      setup: () => context, render: new Function('Vue', code)(Vue) }),
    { global: { components: { ElImage, ElIcon }, plugins: [i18n()] } })
    mounted.push(wrapper)
    return { wrapper, ...context }
  }
  it('forwards remove/retry slot actions with the exact attachment ID and native event', async () => {
    const s = host()
    await s.wrapper.get('.jcr-composer-attachment__retry').trigger('click')
    expect(s.attachmentAction).toHaveBeenLastCalledWith(expect.objectContaining({
      action: 'retry', attachment: expect.objectContaining({ id: 'f' }), index: 0, nativeEvent: expect.any(MouseEvent),
    }))
    await s.wrapper.get('.jcr-composer-attachment__remove').trigger('click')
    expect(s.attachmentAction).toHaveBeenLastCalledWith(expect.objectContaining({ action: 'remove', attachment: expect.objectContaining({ id: 'f' }) }))
    expect(s.submit).not.toHaveBeenCalled()
    await s.wrapper.get('.quote-ref-close').trigger('click'); await s.wrapper.get('.chip-close').trigger('click')
    expect(s.quotedText.value).toBe(''); expect(s.selectedSkill.value).toBeNull()
    expect(s.draft.value).toBe('draft')
  })

  it('retains IME/Enter and external draft ownership across display remount', async () => {
    const s = host(), textarea = s.wrapper.get('textarea')
    await textarea.trigger('compositionstart')
    await textarea.trigger('keydown', { key: 'Enter' })
    expect(s.submit).not.toHaveBeenCalled()
    await textarea.trigger('compositionend'); await textarea.setValue('edited draft')
    await textarea.trigger('keydown', { key: 'Enter' })
    expect(s.submit).toHaveBeenCalledOnce()
    expect(s.submit).toHaveBeenCalledWith(expect.objectContaining({ text: 'edited draft' }))
    s.wrapper.unmount(); mounted.splice(mounted.indexOf(s.wrapper), 1)
    expect(s.draft.value).toBe('edited draft')
    const restored = mount(ChatComposer, { props: { modelValue: s.draft.value } })
    mounted.push(restored); await nextTick()
    expect((restored.get('textarea').element as HTMLTextAreaElement).value).toBe('edited draft')
  })
})
