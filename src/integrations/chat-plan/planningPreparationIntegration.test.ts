// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { effectScope } from 'vue'
import { flushPromises } from '@vue/test-utils'
import { createFastPlanExecutionController } from 'planflow-runtime'
import { useChatStore, disposeChatStore } from '@/stores/chat'
import type { Message } from '@/types'
import { getChatTabState } from '@/modules/chat/tabState'
import { getChatTabConversation } from '@/utils/chatTabRegistry'
import { useExecutionActivityStore } from '@/stores/executionActivity'
import { PlanningPreparationService, type PlanningPreparationStore } from './planningPreparation'
import { PlanningSubmissionService } from './planningSubmission'
import { FastPlanSessionManager } from './sessionManager'

const ipc = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke: ipc }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }))
class Socket {
  static OPEN = 1; static CONNECTING = 0; static instances: Socket[] = []
  readyState = 0; sent: any[] = []; onopen: any; onmessage: any; onclose: any
  constructor() { Socket.instances.push(this) }
  send(value: string) { this.sent.push(JSON.parse(value)) }
  frame(event: any) { this.onmessage?.({ data: JSON.stringify(event) }) }
  open() { this.readyState = 1; this.onopen?.({}); this.frame({ type: 'authenticated' }) }
  close() { this.readyState = 3 }
}

describe('actual ChatStore → preparation → submission → controller', () => {
  it.each([false, true])('survives view scope disposal and sends to the correct successor (existing source=%s)', async existing => {
    setActivePinia(createPinia()); sessionStorage.clear(); Socket.instances = []
    const id = `preparation-integration-${existing}`, path = `/chat/${id}`, target = `target-${existing}`
    const rows: Array<{ id: string; extra_config: string; title: string }> = []
    let resolveCreate!: (result: any) => void
    const created = new Promise<any>(resolve => { resolveCreate = resolve })
    ipc.mockReset().mockImplementation(async (command, args) => {
      if (command === 'set_conversation_task_plan') return created
      if (command === 'get_conversations') return rows
      if (command === 'get_messages') return []
      if (command === 'update_conversation_extra_config') rows.find(item => item.id === args.id)!.extra_config = args.extraConfig
      return undefined
    })
    // The Store factory currently erases its inferred type; keep that baseline issue local.
    const store = useChatStore(id) as unknown as PlanningPreparationStore & { messages: Message[] }
    const owner = getChatTabState(id)
    if (existing) {
      rows.push({ id: `source-${id}`, extra_config: '{}', title: '原普通对话' })
      store.commitPreparedMessages({ conversationId: `source-${id}`, messages: [{ id: 'old-message', content: 'old history' } as Message] })
    }
    owner.state.modelId.value = 'model'
    owner.state.dialogueCharacter.value.taskPlan = true
    owner.state.fastPlanExecutionMode.value = 'manual'
    const file = new File(['content'], 'input.txt')
    owner.retainFile('file', file)
    const manager = new FastPlanSessionManager(options => createFastPlanExecutionController({
      ...options, WebSocket: Socket as any,
      fetch: vi.fn(async (url: any, init?: RequestInit) => {
        if (String(url).includes('/events?')) return new Response(new ReadableStream({ start(controller) {
          init?.signal?.addEventListener('abort', () => controller.close(), { once: true })
        } }), { headers: { 'Content-Type': 'text/event-stream' } })
        return new Response(JSON.stringify(String(url).includes('execution-state') ? { has_state: false } : { messages: [] }))
      }),
    }))
    const upload = vi.fn(() => ({ xhr: { abort: vi.fn() } as unknown as XMLHttpRequest, promise: Promise.resolve({ path: '/input.txt', raw: {} }) }))
    const submission = new PlanningSubmissionService(manager, {
      access: {
        getConnection: async () => ({ apiBase: 'http://test', wsBase: 'ws://test', parentToken: 'test' }),
        resolveModel: async () => ({ model: 'model', base_url: 'http://test', provider: 'test', auth_type: 'none' }),
      },
      createSession: async () => ({ id: `fp-${id}` }), upload,
    })
    const service = new PlanningPreparationService(manager, submission, ipc)
    const view = effectScope()
    try {
      const task = view.run(() => service.submit({ owner, store, tabPath: path, conversationId: store.currentConversationId,
        raw: 'question', prompt: 'question', quote: '', model: { id: 'model', model: 'model' }, executionMode: 'manual', thinking: false,
        attachments: [{ id: 'file', file }], draftAttachments: [{ id: 'file', name: 'input.txt', type: 'document' }],
      }))!
      view.stop()
      rows.push({ id: target, extra_config: '{"task_plan":true}', title: '规划会话' })
      resolveCreate({ session_id: target })
      await vi.waitFor(() => expect(Socket.instances).toHaveLength(1))
      Socket.instances[0].open()
      await expect(task).resolves.toBe(true)
      await flushPromises()
      expect(store.currentConversationId).toBe(target)
      expect(getChatTabConversation(path)).toBe(target)
      expect(store.messages).toEqual([])
      expect(owner.state.fastPlanEntries.value).toHaveLength(1)
      expect(owner.state.fastPlanExecutionMode.value).toBe('manual')
      expect(owner.hydrationRequired).toBe(false)
      expect(upload).toHaveBeenCalledTimes(1)
      const frames = Socket.instances[0].sent.filter(item => item.type === 'chat')
      expect(frames).toHaveLength(1)
      expect(frames[0]).toMatchObject({ content: 'question' })
      expect(ipc).toHaveBeenCalledWith('sync_desktop_plan_user_message', expect.objectContaining({ conversationId: target, content: 'question' }))
      Socket.instances[0].frame({ type: 'plan_generated', tree: { id: 'tree', root: { children: [] }, metadata: {} } })
      await vi.waitFor(() => expect(manager.get(`fp-${id}`)!.state.isPreviewing).toBe(true))
      expect(useExecutionActivityStore().activityByPath(path)?.phase).toBe('awaiting_plan_confirmation')
    } finally { view.stop(); resolveCreate({ session_id: target }); manager.disposeAll(); await disposeChatStore(id) }
  })
})
