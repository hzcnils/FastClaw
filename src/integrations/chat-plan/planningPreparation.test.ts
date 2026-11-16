// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { effectScope, ref, watch } from 'vue'
import { flushPromises } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { PlanningPreparationService, type PlanningPreparation } from './planningPreparation'
import { FastPlanSessionManager } from './sessionManager'
import { getChatTabState, releaseChatTabState, syncChatTabStateConversation } from '@/modules/chat/tabState'
import { setChatTabConversation, unregisterChatTab } from '@/utils/chatTabRegistry'
import { useExecutionActivityStore } from '@/stores/executionActivity'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue([]) }))
const cleanups: Array<() => void> = []
let sequence = 0
function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(done => { resolve = done })
  return { promise, resolve }
}
beforeEach(() => { setActivePinia(createPinia()); localStorage.clear() })
afterEach(async () => { for (const cleanup of cleanups.splice(0).reverse()) cleanup(); await flushPromises() })

function setup(source: string | null = 'source', target = 'target') {
  const id = `prepare-${++sequence}`, path = `/chat/${id}`
  const owner = getChatTabState(id)
  const current = ref(source)
  const scope = effectScope(true)
  scope.run(() => watch(current, (next, previous) => {
    syncChatTabStateConversation(id, next, previous)
    setChatTabConversation(path, next)
  }, { flush: 'sync', immediate: true }))
  const file = new File(['payload'], 'input.txt')
  const draft = { id: 'file', name: 'input.txt', type: 'document' as const, status: 'uploaded' as const, uploaded_file_id: 'file-server' }
  owner.state.messageStr.value = 'question'
  owner.state.quotedText.value = 'quote'
  owner.state.fileList.value = [draft]
  owner.state.modelId.value = 'model'
  owner.state.fastPlanExecutionMode.value = 'manual'
  owner.retainFile('file', file)
  const store = {
    get currentConversationId() { return current.value },
    conversations: source ? [{ id: source, extra_config: '{"execution_mode":"auto","keep":"yes"}' }] : [] as Array<{ id: string; extra_config: string }>,
    loadConversations: vi.fn(async () => {
      if (!store.conversations.some(item => item.id === target)) store.conversations.push({ id: target, extra_config: '{}' })
    }),
    prepareMessages: vi.fn(async (conversationId: string) => ({ conversationId, messages: [], hasMore: false })),
    commitPreparedMessages: vi.fn((prepared: { conversationId: string }) => { current.value = prepared.conversationId }),
    setConvFileCache: vi.fn(), applyFirstMessageTitle: vi.fn(async () => {}),
  }
  const ipc = vi.fn(async (command: string, _args?: any): Promise<any> => command === 'set_conversation_task_plan' ? { session_id: target } : undefined)
  const submit = vi.fn(async (_input: any) => true)
  const manager = new FastPlanSessionManager()
  const service = new PlanningPreparationService(manager, { submit }, ipc)
  const input: PlanningPreparation = { owner, store, tabPath: path, conversationId: source,
    raw: 'question', prompt: 'quote\nquestion', quote: 'quote', draftAttachments: [draft],
    attachments: [{ id: 'file', name: 'input.txt', file }], model: { id: 'model', model: 'model' }, executionMode: 'manual', thinking: true }
  cleanups.push(() => { manager.disposeAll(); releaseChatTabState(id); unregisterChatTab(path); scope.stop() })
  return { owner, current, store, ipc, submit, manager, service, input, path, id, file }
}

describe('application-owned host planning preparation', () => {
  it.each([null, 'source'])('prepares a new/successor conversation and transfers its context: %s', async source => {
    const ctx = setup(source)
    ctx.owner.state.docVersions.value = [{ label: 'V1', content: 'saved', timestamp: 'now' }]
    await expect(ctx.service.submit(ctx.input)).resolves.toBe(true)
    expect(ctx.current.value).toBe('target')
    expect(ctx.store.commitPreparedMessages).toHaveBeenCalledTimes(1)
    expect(ctx.ipc).toHaveBeenCalledWith('set_conversation_execution_mode', { sessionId: 'target', mode: 'manual' })
    expect(JSON.parse(ctx.store.conversations.find(item => item.id === 'target')!.extra_config)).toMatchObject({ task_plan: true, execution_mode: 'manual' })
    expect(ctx.submit).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ conversationId: 'target', prompt: 'quote\nquestion', executionMode: 'manual',
      attachments: [expect.objectContaining({ file: ctx.file })] }))
    expect(ctx.owner.state.fastPlanEntries.value).toHaveLength(1)
    expect(ctx.owner.hydrationRequired).toBe(false)
    expect(ctx.owner.state.planningPreparationId.value).toBeNull()
    expect(useExecutionActivityStore().isTabProtected(ctx.path)).toBe(false)
    if (source) expect(JSON.parse(localStorage.getItem(`fastclaw:docEdit:${source}`)!)).toMatchObject({ versions: [{ content: 'saved' }] })
  })

  it('reuses an existing planning entry without loading or replacing its history', async () => {
    const ctx = setup('source', 'source')
    ctx.owner.state.activeFastPlanEntryId.value = 'existing'
    ctx.owner.state.fastPlanEntries.value = [{ id: 'existing', kind: 'embedded', renderer: 'fastplan', status: 'complete', data: { sessionId: 'fp-existing' } }]
    await ctx.service.submit(ctx.input)
    expect(ctx.store.prepareMessages).not.toHaveBeenCalled()
    expect(ctx.store.commitPreparedMessages).not.toHaveBeenCalled()
    expect(ctx.submit).toHaveBeenCalledWith(expect.objectContaining({ entryId: 'existing', conversationId: 'source' }))
    expect(ctx.owner.state.fastPlanEntries.value[0].data).toMatchObject({ sessionId: 'fp-existing' })
    expect(JSON.parse(ctx.store.conversations[0].extra_config).keep).toBe('yes')
  })

  it('continues after the calling view scope stops, deduplicates submission and freezes its parameters', async () => {
    const ctx = setup(null)
    const created = deferred<any>()
    ctx.ipc.mockImplementationOnce(() => created.promise)
    const view = effectScope()
    const task = view.run(() => ctx.service.submit(ctx.input))!
    expect(ctx.service.submit(ctx.input)).toBe(task)
    expect(useExecutionActivityStore().isTabProtected(ctx.path)).toBe(true)
    expect(ctx.owner.state.fastPlanEntries.value[0].data).toMatchObject({ pendingPrompt: 'quote\nquestion' })
    view.stop()
    ctx.input.attachments[0].name = 'later'
    ;(ctx.input.model as { id: string }).id = 'later-model'
    created.resolve({ session_id: 'target' })
    expect(await task).toBe(true)
    expect(ctx.submit).toHaveBeenCalledWith(expect.objectContaining({ model: { id: 'model', model: 'model' },
      attachments: [expect.objectContaining({ name: 'input.txt' })] }))
  })

  it.each(['create', 'mode', 'list', 'messages'] as const)('stops during %s and rejects its late result', async stage => {
    const ctx = setup(null)
    const blocked = deferred<any>()
    if (stage === 'create') ctx.ipc.mockImplementationOnce(() => blocked.promise)
    if (stage === 'mode') ctx.ipc.mockImplementation(async name => name === 'set_conversation_task_plan' ? { session_id: 'target' } : blocked.promise)
    if (stage === 'list') ctx.store.loadConversations.mockImplementation(() => blocked.promise)
    if (stage === 'messages') ctx.store.prepareMessages.mockImplementation(() => blocked.promise)
    const task = ctx.service.submit(ctx.input)
    await flushPromises()
    expect(ctx.manager.hasSubmission('')).toBe(true)
    await ctx.manager.stopConversation('')
    await expect(task).resolves.toBe(false)
    blocked.resolve(stage === 'messages' ? { conversationId: 'target', messages: [] } : { session_id: 'target' })
    await flushPromises()
    expect(ctx.current.value).toBeNull()
    expect(ctx.submit).not.toHaveBeenCalled()
    expect(ctx.store.commitPreparedMessages).not.toHaveBeenCalled()
    expect(useExecutionActivityStore().isTabProtected(ctx.path)).toBe(false)
  })

  it.each(['close', 'change-conversation', 'change-model', 'change-mode'] as const)('invalidates preparation on %s even without a view', async action => {
    const ctx = setup()
    const blocked = deferred<any>()
    ctx.ipc.mockImplementationOnce(() => blocked.promise)
    const view = effectScope()
    const task = view.run(() => ctx.service.submit(ctx.input))!
    view.stop()
    if (action === 'close') await ctx.manager.closeTabSessions(ctx.path, true)
    if (action === 'change-conversation') ctx.current.value = 'other'
    if (action === 'change-model') ctx.owner.state.modelId.value = 'other'
    if (action === 'change-mode') ctx.owner.state.fastPlanExecutionMode.value = 'step'
    await expect(task).resolves.toBe(false)
    blocked.resolve({ session_id: 'target' }); await flushPromises()
    expect(ctx.submit).not.toHaveBeenCalled()
    expect(ctx.current.value).toBe(action === 'change-conversation' ? 'other' : 'source')
    expect(ctx.owner.state.planningPreparationId.value).toBeNull()
  })

  it('preserves draft and files and refuses to send if mode persistence fails', async () => {
    const ctx = setup()
    ctx.ipc.mockImplementation(async command => {
      if (command === 'set_conversation_execution_mode') throw new Error('mode failed')
      return { session_id: 'target' }
    })
    await expect(ctx.service.submit(ctx.input)).rejects.toThrow('mode failed')
    expect(ctx.submit).not.toHaveBeenCalled()
    expect(ctx.current.value).toBe('source')
    expect(ctx.owner.state.messageStr.value).toBe('question')
    expect(ctx.owner.state.quotedText.value).toBe('quote')
    expect(ctx.owner.files.get('file')).toBe(ctx.file)
    expect(ctx.owner.state.fastPlanEntries.value[0]).toMatchObject({ status: 'error', data: { preflightError: 'mode failed' } })
    expect(JSON.parse(ctx.store.conversations[0].extra_config).execution_mode).toBe('auto')
    expect(useExecutionActivityStore().isTabProtected(ctx.path)).toBe(false)
  })

  it('does not claim another Tab or overwrite unexpected successor history', async () => {
    const ctx = setup()
    setChatTabConversation('/chat/other-prepare', 'target')
    try { await expect(ctx.service.submit(ctx.input)).rejects.toThrow('其他标签页') }
    finally { unregisterChatTab('/chat/other-prepare') }
    expect(ctx.store.commitPreparedMessages).not.toHaveBeenCalled()
    ctx.store.prepareMessages.mockResolvedValueOnce({ conversationId: 'target', messages: [{ id: 'existing' }] as any, hasMore: false })
    await expect(ctx.service.submit(ctx.input)).rejects.toThrow('并非空会话')
    expect(ctx.submit).not.toHaveBeenCalled()
    expect(ctx.current.value).toBe('source')
  })

  it('does not overwrite a draft typed while a preparation failure is pending', async () => {
    const ctx = setup()
    const blocked = deferred<any>()
    ctx.ipc.mockImplementationOnce(() => blocked.promise)
    const task = ctx.service.submit(ctx.input)
    ctx.owner.state.messageStr.value = 'new draft'
    blocked.resolve({})
    await expect(task).rejects.toThrow('未返回会话 ID')
    expect(ctx.owner.state.messageStr.value).toBe('new draft')
    expect(ctx.submit).not.toHaveBeenCalled()
  })

  it('cancels rather than clearing a newly attached draft file on successor transfer', async () => {
    const ctx = setup()
    const blocked = deferred<any>()
    ctx.ipc.mockImplementationOnce(() => blocked.promise)
    const task = ctx.service.submit(ctx.input)
    ctx.owner.state.fileList.value = [{ id: 'new', name: 'new.txt', type: 'document' }]
    await expect(task).resolves.toBe(false)
    blocked.resolve({ session_id: 'target' }); await flushPromises()
    expect(ctx.owner.state.fileList.value.map(item => item.id)).toEqual(['new'])
    expect(ctx.current.value).toBe('source')
    expect(ctx.submit).not.toHaveBeenCalled()
  })

  it('does not recreate entries in a new owner that reuses a closed Tab ID', async () => {
    const ctx = setup()
    const blocked = deferred<any>()
    ctx.ipc.mockImplementationOnce(() => blocked.promise)
    const task = ctx.service.submit(ctx.input)
    releaseChatTabState(ctx.id)
    const next = getChatTabState(ctx.id)
    next.state.messageStr.value = 'next owner'
    await expect(task).resolves.toBe(false)
    blocked.resolve({ session_id: 'target' }); await flushPromises()
    expect(next.state.messageStr.value).toBe('next owner')
    expect(next.state.fastPlanEntries.value).toEqual([])
    expect(ctx.submit).not.toHaveBeenCalled()
  })

})
