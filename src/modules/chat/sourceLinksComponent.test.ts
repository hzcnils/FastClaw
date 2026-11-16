// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { reactive } from 'vue'
import { createI18n } from 'vue-i18n'
import type { Message } from '@/types'
import WebSearchSources from '@/components/chat/WebSearchSources.vue'
import part from '@/integrations/chat-plan/FastClawCustomPart.vue?raw'
import { createChatTabState } from './tabState'
import { createChatContext, createChatContextBinding } from './contextBinding'
import { CHAT_SOURCE_LINKS, createSourceInteractionBinding } from './sourceInteractionBinding'

const cleanup: (() => void)[] = []
afterEach(() => cleanup.splice(0).reverse().forEach(fn => fn()))
function fixture() {
  const owner = createChatTabState('component'); cleanup.push(() => owner.release())
  const store = reactive({ currentConversationId: 'a', messages: [{ id: 'm', conversation_id: 'a', web_hits: [
    { url: 'https://example.com', title: 'Source', snippet: 'snippet' },
  ] }] as Message[] })
  const binding = createChatContextBinding<typeof store>()
  binding.activate(createChatContext({ instanceId: owner.instanceId, path: '/chat/a' }, owner, store)); cleanup.push(binding.dispose)
  let reject!: (reason: unknown) => void
  const invoke = vi.fn(() => new Promise<void>((_, no) => { reject = no }))
  const fallback = vi.fn()
  const links = createSourceInteractionBinding(binding, { traceActions: () => [], schedule: () => {}, scroll: () => {},
    getInvoke: async () => invoke, fallback, linkFailed: vi.fn() })
  const wrapper = mount(WebSearchSources, { props: { hits: store.messages[0].web_hits!, messageId: 'm', query: 'query' },
    global: { plugins: [createI18n({ legacy: false, locale: 'test', missingWarn: false, fallbackWarn: false, messages: { test: {} } })],
      provide: { [CHAT_SOURCE_LINKS as symbol]: links } } })
  cleanup.push(() => wrapper.unmount())
  return { store, binding, wrapper, invoke, fallback, fail: () => reject(new Error('native')) }
}
describe('real WebSearchSources component', () => {
  it('renders the source and delegates opening through the supplied context boundary', async () => {
    const f = fixture(); expect(f.wrapper.text()).toContain('Source'); expect(f.wrapper.text()).toContain('query')
    await f.wrapper.get('.ws-item').trigger('click'); await flushPromises()
    expect(f.invoke).toHaveBeenCalledExactlyOnceWith('open_external', { url: 'https://example.com' })
    f.fail(); await flushPromises(); expect(f.fallback).toHaveBeenCalledExactlyOnceWith('https://example.com')
  })
  it.each(['unmount', 'query', 'target', 'hits', 'tab'] as const)('does not fallback after %s invalidation', async reason => {
    const f = fixture(); await f.wrapper.get('.ws-item').trigger('click'); await flushPromises()
    if (reason === 'unmount') { f.wrapper.unmount(); cleanup.pop() }
    if (reason === 'query') { await f.wrapper.setProps({ query: 'next' }); await f.wrapper.setProps({ query: 'query' }) }
    if (reason === 'target') await f.wrapper.setProps({ messageId: 'other' })
    if (reason === 'hits') await f.wrapper.setProps({ hits: [] })
    if (reason === 'tab') f.binding.deactivate()
    f.fail(); await flushPromises(); expect(f.fallback).not.toHaveBeenCalled()
  })
  it('preserves source identity through the inline renderer adapter', () => {
    expect(part).toContain(':message-id="data.messageId"')
  })
})
