// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick, reactive } from 'vue'
import { createI18n } from 'vue-i18n'
import { ElRate, ElInput, ElButton } from 'element-plus'
import TraceSource from '@/components/chat/TraceSource.vue'
import type { Message } from '@/types'

const mounted: Array<{ unmount(): void }> = []
afterEach(() => mounted.splice(0).forEach(wrapper => wrapper.unmount()))
function fixture() {
  const message = reactive({ id: 'message', trace_source: { result: {
    show: true, rate: 8.5, content: 'feedback', loading: false, isSubmit: false, isQuery: true,
  } } }) as Message
  const wrapper = mount(TraceSource, { props: { message }, global: { plugins: [createI18n({
    legacy: false, locale: 'test', missingWarn: false, fallbackWarn: false, messages: { test: {} },
  })] } })
  mounted.push(wrapper); return { wrapper, trace: message.trace_source!.result! }
}
describe('real inline quality feedback component', () => {
  it('emits without owning loading, so a missing parent cannot leave an endless spinner', async () => {
    const { wrapper, trace } = fixture()
    await wrapper.get('button').trigger('click')
    expect(wrapper.emitted('quality-submit')).toEqual([['message', 8.5, 'feedback']])
    expect(trace.loading).toBe(false)
  })
  it('locks form and duplicate events while the owning request is pending, and permits retry after failure', async () => {
    const { wrapper, trace } = fixture()
    trace.loading = true; await nextTick()
    expect(wrapper.getComponent(ElRate).props('disabled')).toBe(true)
    expect(wrapper.getComponent(ElInput).props('readonly')).toBe(true)
    wrapper.getComponent(ElButton).vm.$emit('click'); await nextTick()
    expect(wrapper.emitted('quality-submit')).toBeUndefined()
    trace.loading = false; await nextTick()
    expect(wrapper.getComponent(ElRate).props('disabled')).toBe(false)
    await wrapper.get('button').trigger('click')
    expect(wrapper.emitted('quality-submit')).toEqual([['message', 8.5, 'feedback']])
  })
  it('shows completion without a submit button and preserves the rating and content', async () => {
    const { wrapper, trace } = fixture()
    trace.isSubmit = true; await nextTick()
    expect(wrapper.find('button').exists()).toBe(false)
    expect(wrapper.text()).toContain('chat.feedbackSubmittedTag')
    expect(wrapper.getComponent(ElRate).props('disabled')).toBe(true)
    expect(trace).toMatchObject({ rate: 8.5, content: 'feedback', loading: false })
  })
})
