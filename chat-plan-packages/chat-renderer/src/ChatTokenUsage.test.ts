// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import ChatTokenUsage from './ChatTokenUsage.vue'

const usage = {
  round: 2,
  inputTokens: 32_097,
  outputTokens: 211,
  cacheReadTokens: 30_720,
  cacheMissTokens: 1_280,
  firstTokenMs: 800,
  contextParts: [
    { id: 'system', label: '系统', value: 7_400, color: '#38bdf8' },
    { id: 'user', label: '用户', value: 8, color: '#fb923c' },
  ],
}

describe('ChatTokenUsage', () => {
  it('starts collapsed and toggles all details from the summary row', async () => {
    const wrapper = mount(ChatTokenUsage, { props: { usage } })
    const summary = wrapper.get('button')
    const details = wrapper.get('.jcr-token-usage__details')

    expect(summary.attributes('aria-expanded')).toBe('false')
    expect(details.attributes('style')).toContain('display: none')
    expect(wrapper.get('.jcr-token-usage__chevron').classes()).not.toContain('is-expanded')

    await summary.trigger('click')

    expect(summary.attributes('aria-expanded')).toBe('true')
    expect(details.attributes('style') ?? '').not.toContain('display: none')
    expect(wrapper.get('.jcr-token-usage__chevron').classes()).toContain('is-expanded')
    expect(wrapper.emitted('update:expanded')).toEqual([[true]])
  })

  it('renders a normalized cache ratio and flowing context entries', async () => {
    const wrapper = mount(ChatTokenUsage, {
      props: {
        usage,
        defaultExpanded: true,
        labels: { cache: '缓存', contextComposition: '上下文构成：' },
      },
    })

    expect(wrapper.text()).toContain('缓存 96%')
    expect(wrapper.text()).toContain('上下文构成：')
    expect(wrapper.findAll('.jcr-token-usage__legend-item')).toHaveLength(2)
  })

  it('falls back to cache token count when miss data is unavailable', () => {
    const wrapper = mount(ChatTokenUsage, {
      props: { usage: { inputTokens: 10, outputTokens: 5, cacheReadTokens: 1_200 } },
    })

    expect(wrapper.text()).toContain('缓存 1.2k')
    expect(wrapper.text()).not.toContain('%')
  })
})
