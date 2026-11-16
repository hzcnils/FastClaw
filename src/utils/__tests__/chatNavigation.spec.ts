import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createMemoryHistory, createRouter, type Router } from 'vue-router'
import { navigateToConversation, openConversationInTab, redirectDuplicateConversation } from '../chatNavigation'
import {
  claimConversationTab, findConversationTab, getChatTabConversation,
  setChatTabConversation, syncRestoredChatTabs, unregisterChatTab,
} from '../chatTabRegistry'

const paths = new Set<string>()
function bind(path: string, id: string | null) {
  paths.add(path)
  setChatTabConversation(path, id)
}

describe('unique conversation tabs', () => {
  let router: Router
  beforeEach(async () => {
    sessionStorage.clear()
    router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: '/chat', component: { template: '<div />' } },
        { path: '/chat/:iid', component: { template: '<div />' } },
        { path: '/favorites', component: { template: '<div />' } },
      ],
    })
    router.beforeEach(redirectDuplicateConversation)
    await router.push('/chat/new')
  })
  afterEach(() => {
    paths.add(router.currentRoute.value.path)
    for (const path of paths) unregisterChatTab(path)
    paths.clear()
    syncRestoredChatTabs([])
  })

  it('activates the existing tab without modifying the source draft or attachments', async () => {
    bind('/chat/original', 'conversation-x')
    bind('/chat/new', null)
    const draft = { text: '未发送问题', files: ['附件.pdf'] }
    const load = vi.fn(async () => { draft.text = ''; draft.files = [] })
    const openedHere = await openConversationInTab(router, '/chat/new', 'conversation-x', load)
    expect(openedHere).toBe(false)
    expect(router.currentRoute.value.path).toBe('/chat/original')
    expect(load).not.toHaveBeenCalled()
    expect(draft).toEqual({ text: '未发送问题', files: ['附件.pdf'] })
    expect(getChatTabConversation('/chat/new')).toBeNull()
  })

  it('loads in the current tab when redirect to an existing tab does not change the route', async () => {
    bind('/chat/original', 'conversation-x')
    bind('/chat/new', 'conversation-a')
    await router.push('/chat/new')
    router.beforeEach(() => false)
    const load = vi.fn(async () => { bind('/chat/new', 'conversation-x') })
    expect(await openConversationInTab(router, '/chat/new', 'conversation-x', load)).toBe(true)
    expect(router.currentRoute.value.path).toBe('/chat/new')
    expect(load).toHaveBeenCalledOnce()
  })

  it('does not reload the current conversation, but allows its initial hydration', async () => {
    bind('/chat/new', 'conversation-x')
    const load = vi.fn(async () => {})
    expect(await openConversationInTab(router, '/chat/new', 'conversation-x', load)).toBe(true)
    expect(load).not.toHaveBeenCalled()
    await openConversationInTab(router, '/chat/new', 'conversation-x', load, { hydrate: true })
    expect(load).toHaveBeenCalledOnce()
  })

  it('reserves the first tab before asynchronous loading so a second click redirects', async () => {
    bind('/chat/first', null)
    bind('/chat/second', null)
    let finish!: () => void
    const firstLoad = openConversationInTab(router, '/chat/first', 'conversation-x', async () => {
      await new Promise<void>(resolve => { finish = resolve })
      bind('/chat/first', 'conversation-x')
    })
    const secondLoad = vi.fn(async () => {})
    expect(await openConversationInTab(router, '/chat/second', 'conversation-x', secondLoad)).toBe(false)
    expect(secondLoad).not.toHaveBeenCalled()
    expect(router.currentRoute.value.path).toBe('/chat/first')
    finish()
    await firstLoad
  })

  it('reuses an existing tab for favorites and retains the requested message id', async () => {
    bind('/chat/original', 'conversation-x')
    await router.push('/favorites')
    const target = await navigateToConversation(router, 'conversation-x', { messageId: 'message-42' })
    expect(target).toBe('/chat/original')
    expect(router.currentRoute.value.query).toEqual({ convId: 'conversation-x', msgId: 'message-42' })
  })

  it('checks a direct legacy link before mounting a duplicate route', async () => {
    bind('/chat/original', 'conversation-x')
    await router.push({ path: '/chat/duplicate', query: { convId: 'conversation-x', msgId: 'message-42' } })
    expect(router.currentRoute.value.path).toBe('/chat/original')
    expect(router.currentRoute.value.query.msgId).toBe('message-42')
  })

  it('finds restored tabs before their chat views have mounted', async () => {
    paths.add('/chat/restored')
    syncRestoredChatTabs([{ path: '/chat/restored', conversationId: 'conversation-x' }])
    expect(await navigateToConversation(router, 'conversation-x')).toBe('/chat/restored')
  })

  it('creates only one new route for repeated external opens during lazy loading', async () => {
    bind('/chat', 'another-conversation')
    const first = await navigateToConversation(router, 'conversation-x')
    paths.add(first)
    const second = await navigateToConversation(router, 'conversation-x')
    const third = await navigateToConversation(router, 'conversation-x')
    expect(first).not.toBe('/chat')
    expect(second).toBe(first)
    expect(third).toBe(first)
    expect(getChatTabConversation('/chat')).toBe('another-conversation')
  })

  it('releases ownership when a tab switches conversation or closes', async () => {
    bind('/chat/original', 'conversation-x')
    bind('/chat/original', 'conversation-y')
    expect(findConversationTab('conversation-x')).toBeUndefined()
    unregisterChatTab('/chat/original')
    expect(findConversationTab('conversation-y')).toBeUndefined()
    bind('/chat/new', null)
    const load = vi.fn(async () => bind('/chat/new', 'conversation-y'))
    expect(await openConversationInTab(router, '/chat/new', 'conversation-y', load)).toBe(true)
    expect(load).toHaveBeenCalledOnce()
  })

  it('releases a failed load reservation and does not release a newer request', async () => {
    bind('/chat/new', null)
    await expect(openConversationInTab(router, '/chat/new', 'failed', async () => {
      throw new Error('load failed')
    })).rejects.toThrow('load failed')
    expect(findConversationTab('failed')).toBeUndefined()
    const oldClaim = claimConversationTab('/chat/new', 'old')
    const newClaim = claimConversationTab('/chat/new', 'new')
    oldClaim.release()
    expect(findConversationTab('new')).toBe('/chat/new')
    newClaim.release()
  })

  it('releases the reservation when navigation is rejected', async () => {
    router.beforeEach(to => to.query.convId === 'blocked' ? false : undefined)
    await navigateToConversation(router, 'blocked', { preferredPath: '/chat/new' })
    expect(findConversationTab('blocked')).toBeUndefined()
  })
})
