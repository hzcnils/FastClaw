// @vitest-environment jsdom
import { h } from 'vue'
import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import ChatMessageItem from './ChatMessageItem.vue'
import { defaultMessages } from './messages'
import type { ChatMessage, MessageActionMenuItem, ResolvedMessageAction } from './types'

describe('custom action menu interactions', () => {
  it('keeps menu item events on the public protocol and closes the menu', async () => {
    const message: ChatMessage = {
      protocolVersion: 1,
      id: 'a1',
      role: 'assistant',
      createdAt: 1,
      parts: [{ type: 'text', text: 'answer' }],
    }
    const action: ResolvedMessageAction = {
      name: 'export', actionId: 'export', label: 'Export', order: 0,
      visible: true, disabled: false, customMenu: true,
    }
    const wrapper = mount(ChatMessageItem, {
      props: { message, actions: [action], messages: defaultMessages },
      slots: {
        'action-menu': ({ triggerItem }: { triggerItem: (item: MessageActionMenuItem) => void }) =>
          h('button', {
            class: 'custom-export-pdf',
            onClick: () => triggerItem({ id: 'pdf', label: 'PDF', payload: { format: 'pdf' } }),
          }, 'PDF'),
      },
    })
    const details = wrapper.get('details.jcr-action-menu').element as HTMLDetailsElement
    details.open = true
    await wrapper.get('.custom-export-pdf').trigger('click')
    await flushPromises()
    expect(wrapper.emitted('message-action')?.[0]?.[0]).toMatchObject({
      action: 'export', messageId: 'a1', menuItemId: 'pdf', payload: { format: 'pdf' },
    })
    expect(details.open).toBe(false)
  })

  it('keeps only one direct parameter menu open at a time', async () => {
    const message: ChatMessage = {
      id: 'a1', role: 'assistant', createdAt: 1,
      parts: [{ type: 'text', text: 'answer' }],
    }
    const actions: ResolvedMessageAction[] = [
      { name: 'translate', actionId: 'translate', label: 'Translate', order: 0, visible: true, disabled: false, menuItems: [{ id: 'en', label: 'English' }] },
      { name: 'switch-model', actionId: 'switch-model', label: 'Model', order: 1, visible: true, disabled: false, menuItems: [{ id: 'm1', label: 'Model 1' }] },
    ]
    const wrapper = mount(ChatMessageItem, { props: { message, actions, messages: defaultMessages } })
    const menus = wrapper.findAll('details.jcr-action-menu')
    ;(menus[0].element as HTMLDetailsElement).open = true
    await menus[0].trigger('toggle')
    ;(menus[1].element as HTMLDetailsElement).open = true
    await menus[1].trigger('toggle')
    expect((menus[0].element as HTMLDetailsElement).open).toBe(false)
    expect((menus[1].element as HTMLDetailsElement).open).toBe(true)
  })
})
