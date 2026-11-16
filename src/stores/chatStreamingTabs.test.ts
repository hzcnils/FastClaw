// @vitest-environment jsdom
import { createPinia, setActivePinia } from 'pinia'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createMemoryHistory, createRouter } from 'vue-router'
import { openConversationInTab } from '@/utils/chatNavigation'
import { setForegroundConversation, forgetConversationActivity } from '@/utils/conversationActivity'
import { defineComponent, effectScope, h } from 'vue'
import { DocumentSubmissionService, type DocumentSubmissionStore } from '@/modules/chat/documentSubmission'
import { mount } from '@vue/test-utils'
import { createI18n } from 'vue-i18n'
import ConversationStatusBadge from '@/components/chat/ConversationStatusBadge.vue'
import { disposeChatStore, useChatStore } from './chat'
import { useExecutionActivityStore } from './executionActivity'
import { getChatTabState } from '@/modules/chat/tabState'

const transport = vi.hoisted(() => ({
  invoke: vi.fn(),
  handlers: new Map<string, Set<(event: { payload: any }) => unknown>>(),
  registrationGate: null as Promise<void> | null,
}))

vi.mock('@tauri-apps/api/core', () => ({ invoke: transport.invoke }))
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, handler: (event: { payload: any }) => unknown) => {
    const handlers = transport.handlers.get(name) || new Set()
    transport.handlers.set(name, handlers)
    handlers.add(handler)
    if (transport.registrationGate) await transport.registrationGate
    return () => handlers.delete(handler)
  }),
}))

async function emit(name: string, payload: Record<string, unknown>) {
  for (const handler of [...(transport.handlers.get(name) || [])]) await handler({ payload })
}

let sequence = 0
let conversationId: string
const tabs = new Set<string>()

function newTab() {
  const id = `stream-tab-${++sequence}`
  tabs.add(id)
  return { id, chat: useChatStore(id) }
}

async function startStream() {
  const tab = newTab()
  await tab.chat.loadMessages(conversationId)
  await tab.chat.sendMessage('问题', 'model-1')
  await emit('chat-stream-token', { conversation_id: conversationId, token: '前半段' })
  return tab
}

const stopCalls = () => transport.invoke.mock.calls.filter(([command]) => command === 'stop_stream')
// remote-session-event 是应用常驻订阅，不属于单次回答的监听器。
const listenerCount = () => [...transport.handlers]
  .filter(([name]) => name !== 'remote-session-event')
  .reduce((sum, [, handlers]) => sum + handlers.size, 0)

describe('streaming a conversation across chat tabs', () => {
  beforeEach(() => {
    sessionStorage.clear()
    setActivePinia(createPinia())
    conversationId = `stream-conversation-${++sequence}`
    transport.invoke.mockReset()
    transport.invoke.mockImplementation(async (command, args) => {
      if (command === 'get_messages' || command === 'get_conversations') return []
      if (command === 'save_assistant_message') return 'saved-error'
      if (command === 'stop_stream') {
        await emit('chat-stream-error', { conversation_id: args.conversationId, canceled: true, error: '已停止' })
        await emit('chat-stream-done', { conversation_id: args.conversationId, stopped: true, content: args.content })
      }
      return undefined
    })
  })

  afterEach(async () => {
    transport.registrationGate = null
    for (const id of tabs) await disposeChatStore(id)
    tabs.clear()
    await vi.waitFor(() => expect(listenerCount()).toBe(0))
    transport.handlers.clear()
    forgetConversationActivity(conversationId)
  })

  it('archives a background document on the actual done event without a ChatView', async () => {
    const tab = newTab()
    await tab.chat.loadMessages(conversationId)
    const state = getChatTabState(tab.id).state
    state.docVersions.value = [{ label: 'V1', content: '原文', timestamp: 'before' }]
    const scope = effectScope()
    const task = scope.run(() => new DocumentSubmissionService().submit({
      owner: getChatTabState(tab.id), store: tab.chat as unknown as DocumentSubmissionStore,
      conversationId, raw: '修改文档', quote: '', document: '原文', modelId: 'model-1',
    }))!
    scope.stop()
    expect(await task).toBe(true)
    const content = '已完成\n---DOCUMENT_CONTENT_START---\n新正文\n---DOCUMENT_CONTENT_END---'
    await emit('chat-stream-token', { conversation_id: conversationId, token: content })
    state.editorVisible.value = false
    await emit('chat-stream-done', { conversation_id: conversationId, message_id: 'doc-final', content })
    expect(state.pendingDocEdit.value).toBe(false)
    expect(state.docVersions.value).toHaveLength(2)
    expect(state.editorContent.value).toBe('新正文')
    expect(state.docEditCardMap['doc-final'].label).toBe('V2')
    expect(tab.chat.messages.find((m: any) => m.id === 'doc-final')?.content).toBe('已完成')
    await vi.waitFor(() => expect(transport.invoke).toHaveBeenCalledWith('update_message_content', { id: 'doc-final', content: '已完成' }))
  })

  it('finishes the original document and message after the store selects another conversation', async () => {
    const tab = newTab()
    await tab.chat.loadMessages(conversationId)
    tab.chat.activateDocEditStream('---DOCUMENT_CONTENT_START---')
    await tab.chat.sendMessage('修改', 'model-1')
    const content = '简介\n---DOCUMENT_CONTENT_START---\nA 正文\n---DOCUMENT_CONTENT_END---'
    await tab.chat.loadMessages(`${conversationId}-other`)
    const state = getChatTabState(tab.id).state
    state.editorContent.value = 'B 草稿'
    await emit('chat-stream-done', { conversation_id: conversationId, message_id: 'a-final', content })
    expect(state.editorContent.value).toBe('B 草稿')
    expect(tab.chat.messages).toEqual([])
    const doc = JSON.parse(localStorage.getItem(`fastclaw:docEdit:${conversationId}`)!)
    expect(doc.versions[0].content).toBe('A 正文')
    expect(doc.cardMap['a-final']).toBeDefined()
    await tab.chat.loadMessages(conversationId)
    expect(tab.chat.messages.find((m: any) => m.id === 'a-final')?.content).toBe('简介')
  })

  it.each(['error', 'stopped'] as const)('clears the document request on %s without producing a version', async terminal => {
    const tab = newTab()
    await tab.chat.loadMessages(conversationId)
    tab.chat.activateDocEditStream('---DOCUMENT_CONTENT_START---')
    await tab.chat.sendMessage('修改', 'model-1')
    if (terminal === 'error') await emit('chat-stream-error', { conversation_id: conversationId, error: '失败' })
    else await tab.chat.stopStream(conversationId)
    expect(getChatTabState(tab.id).state.pendingDocEdit.value).toBe(false)
    expect(getChatTabState(tab.id).state.docVersions.value).toEqual([])
    expect(tab.chat.getDocStreamRaw()).toBe('')
  })

  it('prepares another conversation without clearing the visible one before commit', async () => {
    const tab = newTab()
    await tab.chat.loadMessages(conversationId)
    tab.chat.messages.push({
      id: 'visible-message', conversation_id: conversationId, role: 'assistant',
      content: '旧会话仍可见', status: 'completed', created_at: new Date().toISOString(),
    } as any)
    const targetId = `${conversationId}-prepared`
    let resolveMessages!: (messages: any[]) => void
    transport.invoke.mockImplementation(async (command, args) => {
      if (command === 'get_messages' && args.conversationId === targetId) {
        return new Promise<any[]>(resolve => { resolveMessages = resolve })
      }
      if (command === 'get_messages' || command === 'get_conversations') return []
      return undefined
    })

    const pending = tab.chat.prepareMessages(targetId)
    expect(tab.chat.currentConversationId).toBe(conversationId)
    expect(tab.chat.messages[0]?.content).toBe('旧会话仍可见')

    await Promise.resolve()
    await Promise.resolve()
    resolveMessages([{
      id: 'prepared-message', conversation_id: targetId, role: 'assistant',
      content: '新会话已就绪', status: 'completed', created_at: new Date().toISOString(),
    }])
    const prepared = await pending
    expect(tab.chat.currentConversationId).toBe(conversationId)
    expect(tab.chat.messages[0]?.content).toBe('旧会话仍可见')

    expect(prepared).not.toBeNull()
    tab.chat.commitPreparedMessages(prepared!)
    expect(tab.chat.currentConversationId).toBe(targetId)
    expect(tab.chat.messages[0]?.content).toBe('新会话已就绪')
  })

  it('updates the history badge from spinner to unread dot for a background completion', async () => {
    const original = await startStream()
    const historyTab = newTab()
    const wrapper = mount(defineComponent({
      setup: () => () => h(ConversationStatusBadge, {
        streaming: historyTab.chat.isConversationStreaming(conversationId),
        unread: historyTab.chat.hasUnreadCompletion(conversationId),
      }),
    }), { global: {
      stubs: {
        'el-tooltip': { template: '<span><slot /></span>' },
      },
      plugins: [createI18n({ legacy: false, locale: 'en', messages: { en: { chat: {
      historyStreaming: 'Generating a response', historyUnreadCompletion: 'Response completed. Click to view.',
    } } } })],
    } })
    try {
      expect(wrapper.find('.conversation-status-spinner').exists()).toBe(true)
      await emit('chat-stream-done', { conversation_id: conversationId, message_id: 'completed', content: '完整回答' })
      expect(original.chat.isConversationStreaming(conversationId)).toBe(false)
      expect(historyTab.chat.hasUnreadCompletion(conversationId)).toBe(true)
      expect(wrapper.find('.conversation-status-spinner').exists()).toBe(false)
      expect(wrapper.find('.conversation-status-dot').exists()).toBe(true)
      setForegroundConversation(`/chat/${original.id}`, conversationId)
      await wrapper.vm.$nextTick()
      expect(wrapper.find('.conversation-status').exists()).toBe(false)
      expect(historyTab.chat.hasUnreadCompletion(conversationId)).toBe(false)
    } finally {
      wrapper.unmount()
    }
  })

  it('keeps close protection in the store after the view can be evicted', async () => {
    const original = await startStream()
    const activity = useExecutionActivityStore()
    expect(activity.activityByConversation(conversationId)).toMatchObject({
      tabPath: `/chat/${original.id}`,
      kind: 'chat',
      phase: 'executing',
      protected: true,
    })

    await emit('chat-stream-done', {
      conversation_id: conversationId,
      message_id: 'completed',
      content: '完整回答',
    })

    expect(activity.activityByConversation(conversationId)).toBeUndefined()
  })

  it('does not show a dot when the conversation is in the foreground at completion', async () => {
    const original = await startStream()
    setForegroundConversation(`/chat/${original.id}`, conversationId)
    await emit('chat-stream-done', { conversation_id: conversationId, message_id: 'completed', content: '完整回答' })
    expect(original.chat.hasUnreadCompletion(conversationId)).toBe(false)
    expect(original.chat.isConversationStreaming(conversationId)).toBe(false)
  })

  it('marks a conversation unread if the user leaves it before completion', async () => {
    const original = await startStream()
    setForegroundConversation(`/chat/${original.id}`, conversationId)
    setForegroundConversation(`/chat/${original.id}`, null)
    await emit('chat-stream-done', { conversation_id: conversationId, message_id: 'completed', content: '完整回答' })
    expect(original.chat.hasUnreadCompletion(conversationId)).toBe(true)
    // 恢复/预加载缓存不等于用户已经查看。
    await original.chat.loadMessages(conversationId)
    expect(original.chat.hasUnreadCompletion(conversationId)).toBe(true)
    setForegroundConversation(`/chat/${original.id}`, conversationId)
    expect(original.chat.hasUnreadCompletion(conversationId)).toBe(false)
  })

  it('clears the old completion reminder when another answer starts', async () => {
    const original = await startStream()
    await emit('chat-stream-done', { conversation_id: conversationId, message_id: 'completed', content: '完整回答' })
    expect(original.chat.hasUnreadCompletion(conversationId)).toBe(true)
    await original.chat.sendMessage('继续', 'model-1')
    expect(original.chat.hasUnreadCompletion(conversationId)).toBe(false)
    expect(original.chat.isConversationStreaming(conversationId)).toBe(true)
  })

  it.each(['stopped', 'error', 'stream-error'])('does not mark %s responses as unread completions', async outcome => {
    const original = await startStream()
    if (outcome === 'stopped') await original.chat.stopStream()
    else if (outcome === 'error') await emit('chat-stream-error', { conversation_id: conversationId, error: '连接失败' })
    else await emit('chat-stream-done', { conversation_id: conversationId, message_id: 'partial', content: '部分回答', stream_error: '连接中断' })
    expect(original.chat.isConversationStreaming(conversationId)).toBe(false)
    expect(original.chat.hasUnreadCompletion(conversationId)).toBe(false)
  })

  it('keeps unread reminders when closing a tab and clears them when deleting the conversation', async () => {
    const original = await startStream()
    await emit('chat-stream-done', { conversation_id: conversationId, message_id: 'completed', content: '完整回答' })
    await disposeChatStore(original.id)
    const other = newTab()
    expect(other.chat.hasUnreadCompletion(conversationId)).toBe(true)
    await other.chat.deleteConversation(conversationId)
    expect(other.chat.hasUnreadCompletion(conversationId)).toBe(false)
  })

  it('shows the first tool-mode text before done and does not append the final answer twice', async () => {
    const tab = newTab()
    await tab.chat.loadMessages(conversationId)
    await tab.chat.sendMessage('生成小说', 'model-1')
    await emit('chat-stream-status', { conversation_id: conversationId, status: 'thinking' })
    await emit('chat-stream-reset', { conversation_id: conversationId })
    expect(tab.chat.messages.at(-1)).toMatchObject({ isThinking: true, content: '', stream_phase: 'waiting' })
    await emit('chat-stream-token', { conversation_id: conversationId, token: '## 雨夜' })
    expect(tab.chat.messages.at(-1)).toMatchObject({ isThinking: false, content: '## 雨夜', status: 'streaming' })
    expect(tab.chat.messages.at(-1)?.think_content).toBeUndefined()
    await emit('chat-stream-token', { conversation_id: conversationId, token: '琴声' })
    await emit('chat-stream-done', { conversation_id: conversationId, message_id: 'final', content: '## 雨夜琴声', think_content: null })
    expect(tab.chat.messages.at(-1)).toMatchObject({ content: '## 雨夜琴声', status: 'completed', isThinking: false })
  })

  it('resets only the current round text and reasoning while keeping tool results', async () => {
    const tab = await startStream()
    await emit('chat-stream-token', { conversation_id: conversationId, token: '<think>需要查询</think>' })
    await emit('chat-tool-call', { conversation_id: conversationId, id: 't1', name: 'search', status: 'done', result_preview: '结果' })
    await emit('chat-stream-reset', { conversation_id: `${conversationId}-other` })
    expect(tab.chat.messages.at(-1)?.content).toBe('前半段')
    await emit('chat-stream-status', { conversation_id: conversationId, status: 'finalizing' })
    await emit('chat-stream-reset', { conversation_id: conversationId })
    expect(tab.chat.messages.at(-1)).toMatchObject({ content: '', stream_phase: 'finalizing', isThinking: true })
    expect(tab.chat.messages.at(-1)?.think_content).toBeUndefined()
    expect(tab.chat.messages.at(-1)?.tool_calls).toHaveLength(1)
    await emit('chat-stream-token', { conversation_id: conversationId, token: '最终回答' })
    await emit('chat-stream-done', { conversation_id: conversationId, message_id: 'final', content: '最终回答', think_content: null })
    expect(tab.chat.messages.at(-1)?.content).toBe('最终回答')
    expect(tab.chat.messages.at(-1)?.tool_calls?.[0].result_preview).toBe('结果')
  })

  it('handles split thinking tags and synchronizes authoritative empty final fields', async () => {
    const tab = await startStream()
    await emit('chat-stream-reset', { conversation_id: conversationId })
    for (const token of ['<thi', 'nk>分析', '</th', 'ink>正', '文']) {
      await emit('chat-stream-token', { conversation_id: conversationId, token })
    }
    expect(tab.chat.messages.at(-1)).toMatchObject({ content: '正文', think_content: '分析', stream_phase: 'answering', isThinking: false })
    await emit('chat-stream-done', { conversation_id: conversationId, message_id: 'final', content: '', think_content: null })
    expect(tab.chat.messages.at(-1)?.content).toBe('')
    expect(tab.chat.messages.at(-1)?.think_content).toBeUndefined()
  })

  it('clears retry text and unfinished reasoning before the next attempt', async () => {
    const tab = await startStream()
    await emit('chat-stream-token', { conversation_id: conversationId, token: '<think>旧推理' })
    await emit('chat-stream-retry', { conversation_id: conversationId, attempt: 1, max_retries: 3 })
    await emit('chat-stream-reset', { conversation_id: conversationId })
    expect(tab.chat.messages.at(-1)).toMatchObject({ content: '', retry_progress: { attempt: 1, total: 3 } })
    await emit('chat-stream-token', { conversation_id: conversationId, token: '新回答' })
    expect(tab.chat.messages.at(-1)).toMatchObject({ content: '新回答', isThinking: false })
    expect(tab.chat.messages.at(-1)?.think_content).toBeUndefined()
    expect(tab.chat.messages.at(-1)?.retry_progress).toBeUndefined()
  })

  it('clears waiting on idle, errors and manual stop without accepting late tokens', async () => {
    const tab = await startStream()
    await emit('chat-stream-reset', { conversation_id: conversationId })
    await emit('chat-stream-status', { conversation_id: conversationId, status: 'idle' })
    expect(tab.chat.messages.at(-1)?.isThinking).toBe(false)
    await emit('chat-stream-status', { conversation_id: conversationId, status: 'thinking' })
    await emit('chat-stream-error', { conversation_id: conversationId, error: '服务错误' })
    expect(tab.chat.messages.at(-1)).toMatchObject({ status: 'error', isThinking: false })
    await tab.chat.sendMessage('继续', 'model-1')
    await emit('chat-stream-reset', { conversation_id: conversationId })
    await emit('chat-stream-token', { conversation_id: conversationId, token: '部分回答' })
    await tab.chat.stopStream()
    expect(tab.chat.messages.at(-1)).toMatchObject({ status: 'canceled', isThinking: false })
    const stopped = tab.chat.messages.at(-1)?.content
    await emit('chat-stream-token', { conversation_id: conversationId, token: '迟到内容' })
    expect(tab.chat.messages.at(-1)?.content).toBe(stopped)
  })

  it('keeps receiving tokens and completes after closing the originating tab', async () => {
    const original = await startStream()
    const observer = newTab()
    await observer.chat.loadMessages(conversationId)
    expect(observer.chat.isLoading).toBe(true)
    expect(observer.chat.messages).toBe(original.chat.messages)
    const count = listenerCount()

    await disposeChatStore(original.id)
    expect(stopCalls()).toHaveLength(0)
    expect(listenerCount()).toBe(count)
    await emit('chat-stream-token', { conversation_id: conversationId, token: '后半段' })
    expect(observer.chat.messages.at(-1)?.content).toBe('前半段后半段')

    await emit('chat-stream-done', {
      conversation_id: conversationId, message_id: 'final-answer', content: '完整回答',
    })
    expect(observer.chat.messages.at(-1)).toMatchObject({ id: 'final-answer', content: '完整回答', status: 'completed' })
    expect(observer.chat.isLoading).toBe(false)
    expect(listenerCount()).toBe(0)
    await observer.chat.sendMessage('继续', 'model-1')
    expect(transport.invoke.mock.calls.filter(([cmd]) => cmd === 'send_message_stream')).toHaveLength(2)
  })

  it('toggles isLoading by current conversation when switching away and back in the same tab', async () => {
    const tab = await startStream()
    expect(tab.chat.isLoading).toBe(true)
    const otherId = `${conversationId}-other`
    await tab.chat.loadMessages(otherId)
    expect(tab.chat.currentConversationId).toBe(otherId)
    expect(tab.chat.isLoading).toBe(false)
    expect(tab.chat.isConversationStreaming(conversationId)).toBe(true)
    await emit('chat-stream-token', { conversation_id: conversationId, token: '后台续写' })
    await tab.chat.loadMessages(conversationId)
    expect(tab.chat.isLoading).toBe(true)
    expect(tab.chat.messages.at(-1)?.content).toBe('前半段后台续写')
  })

  it('redirects a new tab to the existing streaming conversation without loading a duplicate', async () => {
    const original = await startStream()
    const blank = newTab()
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: '/chat/:iid', component: { template: '<div />' } }],
    })
    await router.push(`/chat/${blank.id}`)
    const load = vi.fn(() => blank.chat.loadMessages(conversationId))
    expect(await openConversationInTab(router, `/chat/${blank.id}`, conversationId, load)).toBe(false)
    expect(router.currentRoute.value.path).toBe(`/chat/${original.id}`)
    expect(load).not.toHaveBeenCalled()
    expect(blank.chat.currentConversationId).toBeNull()
    await disposeChatStore(blank.id)
    expect(stopCalls()).toHaveLength(0)
    await emit('chat-stream-token', { conversation_id: conversationId, token: '跳转后继续' })
    expect(original.chat.messages.at(-1)?.content).toBe('前半段跳转后继续')
  })

  it('closing a viewing tab leaves the originating tab streaming', async () => {
    const original = await startStream()
    const observer = newTab()
    await observer.chat.loadMessages(conversationId)
    await disposeChatStore(observer.id)
    expect(stopCalls()).toHaveLength(0)
    await emit('chat-stream-token', { conversation_id: conversationId, token: '继续' })
    expect(original.chat.messages.at(-1)?.content).toBe('前半段继续')
    expect(original.chat.isLoading).toBe(true)
  })

  it('moves ownership repeatedly and stops once when the last tab closes', async () => {
    const original = await startStream()
    const second = newTab()
    const third = newTab()
    await second.chat.loadMessages(conversationId)
    await third.chat.loadMessages(conversationId)
    const messages = third.chat.messages
    await disposeChatStore(original.id)
    await disposeChatStore(second.id)
    expect(stopCalls()).toHaveLength(0)
    await emit('chat-stream-token', { conversation_id: conversationId, token: '尾部' })
    await disposeChatStore(third.id)
    await vi.waitFor(() => expect(stopCalls()).toHaveLength(1))
    expect(stopCalls()[0][1]).toMatchObject({ conversationId, content: '前半段尾部' })
    expect(messages.at(-1)?.status).toBe('canceled')
    expect(listenerCount()).toBe(0)
    const reopened = newTab()
    await reopened.chat.loadMessages(conversationId)
    expect(reopened.chat.isLoading).toBe(false)
    expect(reopened.chat.messages.at(-1)?.status).toBe('canceled')
  })

  it('allows the viewing tab to stop the shared stream without leaving either tab loading', async () => {
    const original = await startStream()
    const observer = newTab()
    await observer.chat.loadMessages(conversationId)
    await observer.chat.sendMessage('重复提交', 'model-1')
    expect(transport.invoke.mock.calls.filter(([cmd]) => cmd === 'send_message_stream')).toHaveLength(1)
    await observer.chat.stopStream()
    expect(stopCalls()).toHaveLength(1)
    expect(original.chat.isLoading).toBe(false)
    expect(observer.chat.isLoading).toBe(false)
    expect(observer.chat.messages.at(-1)).toMatchObject({ status: 'canceled', isThinking: false })
    expect(listenerCount()).toBe(0)
  })

  it('finishes an error in the surviving tab', async () => {
    const original = await startStream()
    const observer = newTab()
    await observer.chat.loadMessages(conversationId)
    await disposeChatStore(original.id)
    await emit('chat-stream-error', { conversation_id: conversationId, error: '连接中断' })
    expect(observer.chat.isLoading).toBe(false)
    expect(observer.chat.messages.at(-1)).toMatchObject({ status: 'error', content: '错误: 连接中断' })
    expect(listenerCount()).toBe(0)
  })

  it('stops only unshared conversations when their owner closes', async () => {
    const original = await startStream()
    const observer = newTab()
    await observer.chat.loadMessages(conversationId)
    const otherId = `${conversationId}-other`
    await original.chat.loadMessages(otherId)
    await original.chat.sendMessage('其他问题', 'model-1')
    await disposeChatStore(original.id)
    await vi.waitFor(() => expect(stopCalls()).toHaveLength(1))
    expect(stopCalls()[0][1].conversationId).toBe(otherId)
    await emit('chat-stream-token', { conversation_id: conversationId, token: '继续' })
    expect(observer.chat.messages.at(-1)?.content).toBe('前半段继续')
    expect(observer.chat.isLoading).toBe(true)
  })

  it('preserves a shared stream when the original tab resets to home', async () => {
    const original = await startStream()
    const observer = newTab()
    await observer.chat.loadMessages(conversationId)
    original.chat.resetToHome()
    expect(original.chat.currentConversationId).toBeNull()
    expect(stopCalls()).toHaveLength(0)
    await emit('chat-stream-token', { conversation_id: conversationId, token: '继续' })
    expect(observer.chat.messages.at(-1)?.content).toBe('前半段继续')
    expect(observer.chat.isLoading).toBe(true)
  })

  it('does not launch a request after the last tab closes during listener registration', async () => {
    let resume!: () => void
    transport.registrationGate = new Promise<void>(resolve => { resume = resolve })
    const original = newTab()
    await original.chat.loadMessages(conversationId)
    const pending = original.chat.sendMessage('问题', 'model-1')
    await vi.waitFor(() => expect(listenerCount()).toBeGreaterThan(0))
    await disposeChatStore(original.id)
    resume()
    await pending
    expect(transport.invoke.mock.calls.filter(([cmd]) => cmd === 'send_message_stream')).toHaveLength(0)
    expect(listenerCount()).toBe(0)
  })

  it('keeps the original buffer when its tab switches and closes during registration', async () => {
    let resume!: () => void
    transport.registrationGate = new Promise<void>(resolve => { resume = resolve })
    const original = newTab()
    await original.chat.loadMessages(conversationId)
    const pending = original.chat.sendMessage('问题', 'model-1')
    await vi.waitFor(() => expect(listenerCount()).toBeGreaterThan(0))
    const observer = newTab()
    await observer.chat.loadMessages(conversationId)
    await original.chat.loadMessages(`${conversationId}-other`)
    await disposeChatStore(original.id)
    resume()
    await pending
    expect(stopCalls()).toHaveLength(0)
    await emit('chat-stream-token', { conversation_id: conversationId, token: '注册后输出' })
    expect(observer.chat.messages.at(-1)?.content).toBe('注册后输出')
    expect(observer.chat.isLoading).toBe(true)
  })

  it('clears loading after stopping even if the transport delays the final event', async () => {
    const original = await startStream()
    const observer = newTab()
    await observer.chat.loadMessages(conversationId)
    transport.invoke.mockImplementation(async () => undefined)
    await observer.chat.stopStream()
    expect(original.chat.isLoading).toBe(false)
    expect(observer.chat.messages.at(-1)).toMatchObject({ status: 'canceled', content: '前半段' })
    expect(listenerCount()).toBe(0)
  })

  it('does not clear a new answer when an earlier stop request returns late', async () => {
    const original = await startStream()
    let finishStop!: () => void
    transport.invoke.mockImplementation(async command => {
      if (command === 'stop_stream') await new Promise<void>(resolve => { finishStop = resolve })
    })
    const stopping = original.chat.stopStream()
    await vi.waitFor(() => expect(finishStop).toBeTypeOf('function'))
    await emit('chat-stream-error', { conversation_id: conversationId, canceled: true, error: '已停止' })
    await original.chat.sendMessage('下一轮', 'model-1')
    finishStop()
    await stopping
    await emit('chat-stream-token', { conversation_id: conversationId, token: '下一轮回答' })
    expect(original.chat.isLoading).toBe(true)
    expect(original.chat.messages.at(-1)?.content).toBe('下一轮回答')
  })
})
