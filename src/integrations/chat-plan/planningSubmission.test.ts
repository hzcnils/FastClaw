// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { defineComponent, h, nextTick, reactive } from 'vue'
import { mount, flushPromises } from '@vue/test-utils'
import { createFastPlanExecutionController, type FastPlanAccess } from 'planflow-runtime'
import { createFastPlanTimelinePlugin } from 'planflow-kit'
import { FastPlanSessionManager } from './sessionManager'
import { PlanningSubmissionService, type PlanningSubmission } from './planningSubmission'
import { persistManagedFastPlanState } from './managedPersistence'
import { getChatTabState, releaseChatTabState } from '@/modules/chat/tabState'
import { setChatTabConversation, unregisterChatTab } from '@/utils/chatTabRegistry'
import { useExecutionActivityStore } from '@/stores/executionActivity'

const invoke = vi.hoisted(() => vi.fn().mockResolvedValue(undefined))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}
class Socket {
  static OPEN = 1; static CONNECTING = 0; static instances: Socket[] = []
  readyState = 0; sent: any[] = []; onopen: any; onmessage: any; onclose: any
  constructor(public url: string) { Socket.instances.push(this) }
  open() { this.readyState = 1; this.onopen?.({}); this.frame({ type: 'authenticated' }) }
  send(data: string) { this.sent.push(JSON.parse(data)) }
  frame(value: any) { this.onmessage?.({ data: JSON.stringify(value) }) }
  close() { this.readyState = 3 }
}
const cleanup: Array<() => void> = []
let sequence = 0
beforeEach(() => {
  setActivePinia(createPinia()); Socket.instances = []; invoke.mockReset().mockResolvedValue(undefined)
  Object.defineProperty(HTMLElement.prototype, 'scrollTo', { configurable: true, value: vi.fn() })
})
afterEach(async () => { for (const dispose of cleanup.splice(0).reverse()) dispose(); await flushPromises(); vi.unstubAllGlobals() })

function setup() {
  const id = `submission-${++sequence}`; const path = `/chat/${id}`; const conversationId = `conversation-${id}`
  const owner = getChatTabState(id)
  setChatTabConversation(path, conversationId)
  owner.state.activeFastPlanEntryId.value = 'entry'
  owner.state.fastPlanEntries.value = [{ kind: 'embedded', renderer: 'fastplan', id: 'entry', status: 'active', data: {} }]
  const access: FastPlanAccess = {
    getConnection: async () => ({ apiBase: 'http://test', wsBase: 'ws://test', parentToken: 'test-token' }),
    resolveModel: vi.fn(async () => ({ model: 'model', base_url: 'http://test', provider: 'test', auth_type: 'none' as const })),
  }
  const fetch = vi.fn(async (url: any, init?: RequestInit) => {
    if (String(url).includes('/events?')) return new Response(new ReadableStream({ start(controller) {
      init?.signal?.addEventListener('abort', () => controller.close(), { once: true })
    } }), { headers: { 'Content-Type': 'text/event-stream' } })
    return new Response(JSON.stringify(String(url).includes('execution-state') ? { has_state: false } : { messages: [] }))
  })
  const manager = new FastPlanSessionManager(options => createFastPlanExecutionController({ ...options, WebSocket: Socket as any, fetch }))
  const uploadResult = deferred<{ path: string; raw: unknown }>()
  const abortUpload = vi.fn()
  const upload = vi.fn(() => ({ xhr: { abort: abortUpload } as unknown as XMLHttpRequest, promise: uploadResult.promise }))
  const createSession = vi.fn(async (_title: string, _signal: AbortSignal) => ({ id: `fp-${id}` }))
  const dependencies = { access, createSession, upload }
  const service = new PlanningSubmissionService(manager, dependencies)
  const store = { conversations: [{ id: conversationId, extra_config: '{"execution_mode":"manual"}' }], applyFirstMessageTitle: vi.fn(async () => {}) }
  const input: PlanningSubmission = { owner, store, tabPath: path, conversationId, entryId: 'entry', prompt: 'compare weather',
    attachments: [{ id: 'file', file: new File(['brief'], 'brief.txt') }],
    model: { id: 'model', model: 'model' }, executionMode: 'manual', thinking: true }
  cleanup.push(() => { manager.disposeAll(); releaseChatTabState(id); unregisterChatTab(path) })
  const open = async () => {
    await vi.waitFor(() => expect(Socket.instances.find(item => item.url.endsWith(`fp-${id}`))).toBeDefined())
    const socket = Socket.instances.find(item => item.url.endsWith(`fp-${id}`))!
    socket.open(); await flushPromises(); return socket
  }
  return { id, path, owner, input, manager, service, dependencies, access, store, createSession, upload, uploadResult, abortUpload, fetch, open }
}

describe('application-owned FP submission', () => {
  it.each([false, true])('survives actual renderer eviction and sends once (existing history=%s)', async existingHistory => {
    const ctx = setup()
    const renderError = vi.fn(), renderWarn = vi.fn()
    const view = () => {
      const plugin = createFastPlanTimelinePlugin(reactive({ sessionManager: ctx.manager, access: ctx.access,
        composerMode: 'host' as const, composerEntryId: 'entry', composerTarget: '#submission-composer' }))
      return mount(defineComponent({ setup: () => () => {
        const entry = ctx.owner.state.fastPlanEntries.value[0]
        return h('main', [h(plugin.timelineRenderers.fastplan, { entry, data: entry.data }), h('div', { id: 'submission-composer' })])
      } }), { attachTo: document.body, global: { config: { errorHandler: renderError, warnHandler: renderWarn } } })
    }
    let wrapper: ReturnType<typeof view>
    if (existingHistory) {
      vi.stubGlobal('fetch', ctx.fetch)
      ctx.owner.state.fastPlanEntries.value[0].data = { sessionId: `fp-${ctx.id}`, connectionMode: 'history' }
      wrapper = view()
      await flushPromises()
    }
    const task = ctx.service.submit(ctx.input)
    expect(ctx.service.submit(ctx.input)).toBe(task)
    if (!existingHistory) wrapper = view()
    const socket = await ctx.open()
    expect(ctx.upload).toHaveBeenCalledOnce()
    const runtime = ctx.manager.get(`fp-${ctx.id}`)!
    wrapper!.unmount()
    ctx.uploadResult.resolve({ path: '/uploads/brief.txt', raw: {} })
    await expect(task).resolves.toBe(true)
    await runtime.processEvent({ type: 'plan_generated', tree: { id: 'tree', root: { children: [] }, metadata: {} } })
    wrapper = view()
    try {
      await nextTick(); await flushPromises()
      expect(ctx.manager.get(`fp-${ctx.id}`)).toBe(runtime)
      expect(socket.sent.filter(item => item.type === 'chat')).toHaveLength(1)
      expect(socket.sent.find(item => item.type === 'chat').attachments).toEqual([expect.objectContaining({ path: '/uploads/brief.txt' })])
      expect(ctx.owner.state.fastPlanEntries.value[0].data).toMatchObject({ command: undefined, pendingPrompt: undefined })
      expect(runtime.state.isPreviewing).toBe(true)
      expect(document.querySelector('#submission-composer')?.textContent).toContain('确认')
      expect(ctx.store.applyFirstMessageTitle).toHaveBeenCalledExactlyOnceWith(ctx.input.conversationId, ctx.input.prompt)
      expect(invoke.mock.calls.filter(([name]) => name === 'sync_desktop_plan_user_message')).toHaveLength(1)
      expect(renderError).not.toHaveBeenCalled(); expect(renderWarn).not.toHaveBeenCalled()
      expect(Socket.instances).toHaveLength(1)
    } finally { wrapper.unmount() }
  })

  it.each(['stop', 'close', 'reset'] as const)('%s during create rejects late creation without creating a replacement entry', async action => {
    const ctx = setup(); const created = deferred<any>()
    ctx.createSession.mockReturnValue(created.promise)
    const task = ctx.service.submit(ctx.input)
    if (action === 'stop') await ctx.manager.stopConversation(ctx.input.conversationId)
    else if (action === 'close') await ctx.manager.closeTabSessions(ctx.path, true)
    else { ctx.owner.resetSelectedConversation(); setChatTabConversation(ctx.path, 'another-conversation') }
    await expect(task).resolves.toBe(false)
    expect(ctx.createSession.mock.calls[0][1].aborted).toBe(true)
    created.resolve({ id: 'late-session' }); await flushPromises()
    expect(ctx.manager.has('late-session')).toBe(false)
    expect(Socket.instances).toHaveLength(0)
    expect(invoke).not.toHaveBeenCalledWith('update_conversation_extra_config', expect.anything())
    expect(useExecutionActivityStore().isTabProtected(ctx.path)).toBe(false)
    if (action === 'reset') expect(ctx.owner.state.fastPlanEntries.value).toEqual([])
  })

  it('aborts an upload immediately; a late response cannot send or clear another tab', async () => {
    const a = setup(), b = setup()
    const taskA = a.service.submit(a.input); const taskB = b.service.submit(b.input)
    const socketA = await a.open(), socketB = await b.open()
    await a.manager.stopConversation(a.input.conversationId)
    await expect(taskA).resolves.toBe(false)
    expect(a.abortUpload).toHaveBeenCalledOnce()
    expect(useExecutionActivityStore().isTabProtected(a.path)).toBe(false)
    expect(useExecutionActivityStore().isTabProtected(b.path)).toBe(true)
    a.uploadResult.resolve({ path: '/old', raw: {} }); b.uploadResult.resolve({ path: '/new', raw: {} })
    await taskB
    expect(socketA.sent.some(item => item.type === 'chat')).toBe(false)
    expect(socketB.sent.filter(item => item.type === 'chat')).toHaveLength(1)
  })

  it('does not send when stopped during asynchronous model resolution', async () => {
    const ctx = setup(); ctx.input.attachments = []
    const model = deferred<any>(); vi.mocked(ctx.access.resolveModel).mockReturnValue(model.promise)
    const task = ctx.service.submit(ctx.input); const socket = await ctx.open()
    await vi.waitFor(() => expect(ctx.access.resolveModel).toHaveBeenCalledOnce())
    await ctx.manager.stopConversation(ctx.input.conversationId)
    await expect(task).resolves.toBe(false)
    model.resolve({ model: 'old', auth_type: 'none' }); await flushPromises()
    expect(socket.sent.some(item => item.type === 'chat')).toBe(false)
    expect(useExecutionActivityStore().isTabProtected(ctx.path)).toBe(false)
  })

  it('cancels a connection wait without a renderer or a polling timer', async () => {
    const ctx = setup()
    const task = ctx.service.submit(ctx.input)
    await vi.waitFor(() => expect(Socket.instances).toHaveLength(1))
    // No authenticated socket: stop fails to reach the server but still invalidates the local job.
    await expect(ctx.manager.stopConversation(ctx.input.conversationId)).rejects.toThrow('not connected')
    await expect(task).resolves.toBe(false)
    expect(useExecutionActivityStore().isTabProtected(ctx.path)).toBe(false)
    Socket.instances[0].open(); await flushPromises()
    expect(ctx.upload).not.toHaveBeenCalled()
    expect(Socket.instances[0].sent.some(item => item.type === 'chat')).toBe(false)
  })

  it('does not connect/send when the binding cannot be persisted, and preserves retry files', async () => {
    const ctx = setup()
    invoke.mockRejectedValueOnce(new Error('disk write failed'))
    await expect(ctx.service.submit(ctx.input)).rejects.toThrow('disk write failed')
    expect(Socket.instances).toHaveLength(0)
    expect(ctx.store.conversations[0].extra_config).toBe('{"execution_mode":"manual"}')
    expect(ctx.owner.state.fastPlanEntries.value[0].data).toMatchObject({ command: undefined,
      preflightError: 'disk write failed', pendingAttachments: [expect.objectContaining({ id: 'file' })] })
    expect(useExecutionActivityStore().isTabProtected(ctx.path)).toBe(false)
  })

  it('refuses a competing writer before creating a session or stealing the running indicator', async () => {
    const ctx = setup()
    ctx.manager.acquireWriter(ctx.input.conversationId, 'another-tab')
    useExecutionActivityStore().update(ctx.input.conversationId, '/chat/other', 'fastplan', { phase: 'executing', protected: true })
    await expect(ctx.service.submit(ctx.input)).rejects.toThrow('其他标签页')
    expect(ctx.createSession).not.toHaveBeenCalled()
    expect(useExecutionActivityStore().activityByConversation(ctx.input.conversationId)?.tabPath).toBe('/chat/other')
  })

  it('releases a writer when closed before FP creation finishes, without affecting a reused tab id', async () => {
    const ctx = setup(); const created = deferred<any>()
    ctx.createSession.mockReturnValue(created.promise)
    const task = ctx.service.submit(ctx.input)
    releaseChatTabState(ctx.id)
    const replacement = getChatTabState(ctx.id)
    replacement.state.messageStr.value = 'new draft'
    await expect(task).resolves.toBe(false)
    expect(ctx.manager.acquireWriter(ctx.input.conversationId, 'new-owner')).toBe(true)
    created.resolve({ id: 'late' }); await flushPromises()
    expect(replacement.state.fastPlanEntries.value).toEqual([])
    expect(replacement.state.messageStr.value).toBe('new draft')
  })

  it('stops a queued persistence job from later connecting', async () => {
    const ctx = setup(); const persisted = deferred<void>()
    invoke.mockImplementation(name => name === 'update_conversation_extra_config' ? persisted.promise : Promise.resolve())
    const task = ctx.service.submit(ctx.input)
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith('update_conversation_extra_config', expect.anything()))
    await ctx.manager.closeTabSessions(ctx.path, true)
    await expect(task).resolves.toBe(false)
    persisted.resolve(); await flushPromises()
    expect(Socket.instances).toHaveLength(0)
    expect(ctx.upload).not.toHaveBeenCalled()
  })
})

describe('Tab-owned planning writer', () => {
  it('survives renderer changes and releases on actual context invalidation', async () => {
    const ctx = setup()
    ctx.owner.state.fastPlanEntries.value[0].data = { sessionId: 'fp' }
    const save = () => persistManagedFastPlanState(ctx.owner, ctx.store, ctx.input.conversationId,
      ctx.owner.state.fastPlanEntries.value, 'entry', ctx.manager)
    await save()
    const first = JSON.parse(ctx.store.conversations[0].extra_config).fastplan
    const retained = getChatTabState(ctx.id)
    expect(retained.planWriterId).toBe(ctx.owner.planWriterId)
    await save()
    expect(JSON.parse(ctx.store.conversations[0].extra_config).fastplan.revision).toBe(first.revision + 1)
    expect(ctx.manager.acquireWriter(ctx.input.conversationId, 'other-tab')).toBe(false)
    ctx.owner.resetSelectedConversation()
    expect(ctx.manager.acquireWriter(ctx.input.conversationId, 'other-tab')).toBe(true)
  })

  it('serializes writes and computes each revision from the last successful write', async () => {
    const ctx = setup(); const first = deferred<void>()
    ctx.owner.state.fastPlanEntries.value[0].data = { sessionId: 'fp' }
    invoke.mockReturnValueOnce(first.promise)
    const save = () => persistManagedFastPlanState(ctx.owner, ctx.store, ctx.input.conversationId,
      ctx.owner.state.fastPlanEntries.value, 'entry', ctx.manager)
    const one = save(), two = save()
    await flushPromises()
    expect(invoke).toHaveBeenCalledOnce()
    first.resolve(); await Promise.all([one, two])
    expect(JSON.parse(ctx.store.conversations[0].extra_config).fastplan.revision).toBe(2)
  })
})
