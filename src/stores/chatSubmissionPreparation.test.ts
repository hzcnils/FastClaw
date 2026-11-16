// @vitest-environment jsdom
import { createPinia, setActivePinia } from 'pinia'
import { effectScope } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { disposeChatStore, useChatStore } from './chat'
import { getChatTabState } from '@/modules/chat/tabState'
import { DeferredChatSubmissionService, type DeferredChatStore } from '@/modules/chat/deferredSubmission'
import { uploadTabDocument } from '@/modules/chat/attachmentTasks'
import { DocumentSubmissionService } from '@/modules/chat/documentSubmission'
import { useExecutionActivityStore } from './executionActivity'
import type { Conversation, Message } from '@/types'

const transport = vi.hoisted(() => ({ invoke: vi.fn(), listeners: new Set<() => void>() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: transport.invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => {
  const off = () => { transport.listeners.delete(off) }
  transport.listeners.add(off)
  return off
}) }))

interface Store extends DeferredChatStore {
  messages: Message[]
  conversations: Conversation[]
  isLoading: boolean
  loadMessages(id: string): Promise<void>
  loadConversations(force?: boolean): Promise<void>
  stopStream(id?: string): Promise<void>
  activateDocEditStream(marker: string, conversationId?: string | null): () => void
  getDocStreamRaw(): string
}
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(yes => { resolve = yes }); return { promise, resolve } }
let sequence = 0
const tabs = new Set<string>()
function tab(id = `send-preparation-${++sequence}`) {
  tabs.add(id)
  const chat = useChatStore(id) as unknown as Store
  return { id, chat, owner: getChatTabState(id) }
}
const row = (id: string) => ({ id, title: '智能对话', mode: 'normal', updated_at: new Date().toISOString() })
const sends = () => transport.invoke.mock.calls.filter(([command]) => command === 'send_message_stream')

describe('Store message preparation ownership', () => {
  beforeEach(() => {
    sessionStorage.clear(); setActivePinia(createPinia())
    transport.invoke.mockReset()
    transport.invoke.mockImplementation(async command => {
      if (command === 'get_messages' || command === 'get_conversations') return []
      if (command === 'create_conversation') return `created-${sequence}`
    })
  })
  afterEach(async () => { for (const id of tabs) await disposeChatStore(id); tabs.clear() })

  it.each(['create', 'title'] as const)('cleans document preparation on stop/close/switch during %s', async stage => {
    for (const action of ['stop', 'close', 'switch']) {
      const c = tab(), service = new DocumentSubmissionService(), gate = deferred<any>(), source = `doc-${c.id}`
      const command = stage === 'create' ? 'create_conversation' : 'update_conversation'
      transport.invoke.mockClear()
      transport.invoke.mockImplementation(async cmd => {
        if (cmd === command) return gate.promise
        if (cmd === 'get_conversations') return [row(source)]
        if (cmd === 'get_messages') return []
      })
      if (stage === 'title') { await c.chat.loadConversations(true); await c.chat.loadMessages(source) }
      c.owner.state.messageStr.value = '修改文档'
      const task = service.submit({ owner: c.owner, store: c.chat, conversationId: c.chat.currentConversationId,
        raw: '修改文档', quote: '', document: '原文', modelId: 'model' })
      await vi.waitFor(() => expect(transport.invoke.mock.calls.some(([cmd]) => cmd === command)).toBe(true))
      expect(c.owner.state.pendingDocEdit.value).toBe(true)
      if (action === 'stop') await c.chat.stopStream()
      if (action === 'close') await disposeChatStore(c.id)
      if (action === 'switch') await c.chat.loadMessages(`other-${c.id}`)
      expect(await task).toBe(false)
      expect(c.owner.state.pendingDocEdit.value).toBe(false)
      expect(c.chat.getDocStreamRaw()).toBe('')
      if (action === 'stop') expect(c.owner.state.messageStr.value).toBe('修改文档')
      gate.resolve(source)
      await new Promise(resolve => setTimeout(resolve, 0))
      expect(sends()).toHaveLength(0)
      expect(c.chat.messages).toHaveLength(0)
    }
  })

  it('cleans a failed document create even for a direct Store caller', async () => {
    const c = tab()
    transport.invoke.mockImplementation(async command => {
      if (command === 'create_conversation') throw new Error('文档会话创建失败')
      return []
    })
    c.chat.activateDocEditStream('---DOCUMENT_CONTENT_START---')
    await expect(c.chat.sendMessage('修改', 'model')).rejects.toThrow('文档会话创建失败')
    expect(c.owner.state.pendingDocEdit.value).toBe(false)
    expect(c.chat.getDocStreamRaw()).toBe('')
    expect(c.chat.messages).toEqual([])
  })

  it('late document preparation and its cancellation handle cannot clear a newer request', async () => {
    const c = tab(), service = new DocumentSubmissionService(), gate = deferred<any>(), source = `doc-${c.id}`
    transport.invoke.mockImplementation(async command => {
      if (command === 'update_conversation') return gate.promise
      if (command === 'get_conversations') return [row(source)]
      if (command === 'get_messages') return []
    })
    await c.chat.loadConversations(true); await c.chat.loadMessages(source)
    const task = service.submit({ owner: c.owner, store: c.chat, conversationId: source,
      raw: '旧指令', quote: '', document: '旧文', modelId: 'model' })
    await vi.waitFor(() => expect(transport.invoke).toHaveBeenCalledWith('update_conversation', expect.anything()))
    const cancelNew = c.chat.activateDocEditStream('---DOCUMENT_CONTENT_START---', source)
    gate.resolve(undefined)
    expect(await task).toBe(false)
    expect(c.owner.state.pendingDocEdit.value).toBe(true)
    expect(c.owner.state.messageStr.value).toBe('')
    expect(c.owner.state.chatSubmissionError.value).toBe('')
    expect(sends()).toHaveLength(0)
    cancelNew()
    expect(c.owner.state.pendingDocEdit.value).toBe(false)
  })

  it.each(['create', 'list', 'title'] as const)('rejects close/stop/switch while waiting for %s', async stage => {
    for (const action of ['stop', 'close', 'switch']) {
      const c = tab(), gate = deferred<any>(), target = `target-${c.id}`, source = `source-${c.id}`
      const command = stage === 'create' ? 'create_conversation' : stage === 'list' ? 'get_conversations' : 'update_conversation'
      transport.invoke.mockImplementation(async cmd => {
        if (cmd === command) return gate.promise
        if (cmd === 'create_conversation') return target
        if (cmd === 'get_conversations') return [row(source), row(target)]
        if (cmd === 'get_messages') return []
      })
      if (stage === 'title') { await c.chat.loadConversations(true); await c.chat.loadMessages(source) }
      const before = sends().length
      const task = c.chat.sendMessage('待发送问题', 'model')
      await vi.waitFor(() => expect(transport.invoke.mock.calls.some(([cmd]) => cmd === command)).toBe(true))
      // Reset calls between iterations so this gate has really been reached.
      if (action === 'stop') await c.chat.stopStream()
      if (action === 'close') await disposeChatStore(c.id)
      if (action === 'switch') await c.chat.loadMessages(`other-${c.id}`)
      expect(await task).toBe(false)
      const selected = c.chat.currentConversationId
      gate.resolve(stage === 'create' ? target : stage === 'list' ? [row(target)] : undefined)
      await new Promise(resolve => setTimeout(resolve, 0))
      expect(sends()).toHaveLength(before)
      expect(c.chat.currentConversationId).toBe(selected)
      expect(c.chat.messages.some(message => message.content === '待发送问题')).toBe(false)
      expect(c.chat.isLoading).toBe(false)
      expect(useExecutionActivityStore().isTabProtected(`/chat/${c.id}`)).toBe(false)
      transport.invoke.mockClear()
    }
  })

  it('blocks concurrent first sends and transfers the original message array to the new conversation', async () => {
    const c = tab(), gate = deferred<string>(), target = `target-${c.id}`
    transport.invoke.mockImplementation(async command => {
      if (command === 'create_conversation') return gate.promise
      if (command === 'get_conversations') return [row(target)]
    })
    const first = c.chat.sendMessage('第一问', 'model')
    expect(c.chat.messages).toHaveLength(2)
    expect(c.chat.isLoading).toBe(true)
    expect(await c.chat.sendMessage('重复', 'model')).toBe(false)
    gate.resolve(target)
    expect(await first).toBe(true)
    expect(c.chat.currentConversationId).toBe(target)
    expect(c.chat.messages.every(message => message.conversation_id === target)).toBe(true)
    expect(sends()).toHaveLength(1)
    expect(sends()[0][1]).toMatchObject({ conversationId: target, content: '第一问' })
  })

  it('does not select a late created ID into a reopened Tab with the same path', async () => {
    const old = tab(), gate = deferred<string>()
    transport.invoke.mockImplementation(async command => command === 'create_conversation' ? gate.promise : [])
    const task = old.chat.sendMessage('旧请求', 'model')
    await vi.waitFor(() => expect(transport.invoke).toHaveBeenCalledWith('create_conversation', expect.anything()))
    await disposeChatStore(old.id)
    const reopened = tab(old.id)
    await reopened.chat.loadMessages(`new-${old.id}`)
    expect(await task).toBe(false)
    gate.resolve('late-created')
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(reopened.chat.currentConversationId).toBe(`new-${old.id}`)
    expect(reopened.chat.messages).toEqual([])
    expect(sends()).toHaveLength(0)
  })

  it('surfaces desktop create failure without a mock fallback or phantom messages and allows retry', async () => {
    const c = tab()
    transport.invoke.mockImplementation(async command => {
      if (command === 'create_conversation') throw new Error('数据库暂不可用')
      return []
    })
    await expect(c.chat.sendMessage('失败', 'model')).rejects.toThrow('数据库暂不可用')
    expect(c.chat.currentConversationId).toBeNull()
    expect(c.chat.messages).toEqual([])
    expect(c.chat.isLoading).toBe(false)
    transport.invoke.mockImplementation(async command => command === 'create_conversation' ? 'retry' : [])
    expect(await c.chat.sendMessage('重试', 'model')).toBe(true)
    expect(c.chat.messages).toHaveLength(2)
    expect(sends()).toHaveLength(1)
  })

  it.each([false, true])('real upload -> detached command -> Store sends once (existing=%s)', async existing => {
    const c = tab(), service = new DeferredChatSubmissionService(), gate = deferred<any>(), scope = effectScope()
    const target = `target-${c.id}`
    transport.invoke.mockImplementation(async command => {
      if (command === 'create_conversation') return target
      if (command === 'get_conversations') return [row(target)]
      if (command === 'get_messages') return []
    })
    if (existing) await c.chat.loadMessages(target)
    const state = c.owner.state
    state.messageStr.value = '问题'; state.modelId.value = 'model'; state.dialogueCharacter.value.taskPlan = false
    const upload = uploadTabDocument(c.owner, new File(['text'], 'a.txt'), { upload: () => gate.promise, persist: vi.fn() })
    const task = scope.run(() => service.submit({
      owner: c.owner, store: c.chat, tabPath: `/chat/${c.id}`, conversationId: c.chat.currentConversationId,
      raw: '问题', quote: '', content: '问题', modelId: 'model', vision: false,
      knowledgeBaseIds: ['selected'], localKbPaths: [], webSearch: false, thinking: true, mcpTools: false, mcpServerIds: [],
    }))!
    scope.stop()
    gate.resolve({ success: true, data: { knowledge_base_id: 'kb', file_id: 'file' } })
    await upload
    expect(await task).toBe(true)
    expect(sends()).toHaveLength(1)
    expect(sends()[0][1]).toMatchObject({ conversationId: target, knowledgeBaseIds: ['selected', 'kb'], fileIds: ['file'], content: '问题', enableThinking: true })
    expect(c.chat.messages).toHaveLength(2)
    expect(state.fileList.value).toEqual([])
    expect(state.chatSubmissionError.value).toBe('')
  })

  it.each([false, true])('immediate image -> detached preparation -> Store sends once (existing=%s)', async existing => {
    const c = tab(), image = deferred<string>(), reader = vi.fn(() => image.promise)
    const service = new DeferredChatSubmissionService(reader), scope = effectScope()
    const target = `image-target-${c.id}`
    transport.invoke.mockImplementation(async command => {
      if (command === 'create_conversation') return target
      if (command === 'get_conversations') return [row(target)]
      if (command === 'get_messages') return []
    })
    if (existing) await c.chat.loadMessages(target)
    const state = c.owner.state
    state.messageStr.value = '分析图片'; state.modelId.value = 'vision-model'; state.dialogueCharacter.value.taskPlan = false
    state.fileList.value = [{ id: 'image', name: 'image.png', type: 'image', status: 'pending', preview_url: 'blob:image' }]
    const task = scope.run(() => service.submit({
      owner: c.owner, store: c.chat, tabPath: `/chat/${c.id}`, conversationId: c.chat.currentConversationId,
      raw: '分析图片', quote: '', content: '分析图片', modelId: 'vision-model', vision: true, waitForUploads: false,
      knowledgeBaseIds: [], localKbPaths: [], webSearch: false, thinking: false, mcpTools: true, mcpServerIds: ['original-tool'],
    }))!
    scope.stop()
    await vi.waitFor(() => expect(reader).toHaveBeenCalledTimes(1))
    expect(sends()).toHaveLength(0)
    image.resolve('data:image/png;base64,A')
    expect(await task).toBe(true)
    expect(sends()).toHaveLength(1)
    expect(sends()[0][1]).toMatchObject({ conversationId: target, content: '分析图片', modelId: 'vision-model', images: ['data:image/png;base64,A'], mcpServerIds: ['original-tool'] })
    expect(c.chat.messages).toHaveLength(2)
    expect(c.chat.messages[0].attachments?.[0].preview_url).toBe('data:image/png;base64,A')
    expect(state.fileList.value).toEqual([])
    expect(state.pendingAutoSend.value).toBe(false)
  })
})
