// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import ChatMarkdownRenderer from './ChatMarkdownRenderer.vue'

describe('ChatMarkdownRenderer lightweight path', () => {
  it('renders ordinary text without mounting MdPreview', () => {
    const wrapper = mount(ChatMarkdownRenderer, {
      props: { content: '普通文本\n第二行' },
      global: { stubs: { MdPreview: true } },
    })

    expect(wrapper.find('.jcr-markdown__plain').text()).toContain('普通文本')
    expect(wrapper.find('md-preview-stub').exists()).toBe(false)
  })

  it('uses lightweight text while streaming and full markdown after completion', async () => {
    const wrapper = mount(ChatMarkdownRenderer, {
      props: { content: '**正在生成**', streaming: true },
      global: { stubs: { MdPreview: true } },
    })

    expect(wrapper.find('.jcr-markdown__plain').exists()).toBe(true)
    await wrapper.setProps({ streaming: false })
    expect(wrapper.find('.jcr-markdown__plain').exists()).toBe(false)
    expect(wrapper.find('md-preview-stub').exists()).toBe(true)
  })

  it('keeps the existing truncation control on the lightweight path', () => {
    const wrapper = mount(ChatMarkdownRenderer, {
      props: { content: 'abcdefgh', maxChars: 4 },
      global: { stubs: { MdPreview: true } },
    })

    expect(wrapper.find('.jcr-markdown__plain').text()).toBe('abcd')
    expect(wrapper.find('.jcr-show-full').exists()).toBe(true)
  })
})
