// @vitest-environment jsdom
import { defineComponent, h, nextTick, reactive } from 'vue'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createFastPlanExecutionController } from 'planflow-runtime'
import { createFastPlanTimelinePlugin } from 'planflow-kit'
import { FastPlanSessionManager } from './sessionManager'
import { retainedComposerEntry } from './tabRuntimeBridge'
import { getChatTabState, releaseChatTabState } from '@/modules/chat/tabState'
import { setChatTabConversation, unregisterChatTab } from '@/utils/chatTabRegistry'

const invoke = vi.hoisted(() => vi.fn().mockResolvedValue(undefined))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

class Socket {
  static OPEN = 1; static CONNECTING = 0; static instances: Socket[] = []
  readyState = 0; sent: any[] = []; onopen: any; onmessage: any; onerror: any; onclose: any
  constructor(public url: string) { Socket.instances.push(this) }
  open() { this.readyState = 1; this.onopen?.({}) }
  send(data: string) { this.sent.push(JSON.parse(data)) }
  frame(event: any) { this.onmessage?.({ data: JSON.stringify(event) }) }
  close() { this.readyState = 3 }
}
class Events {
  static instances: Events[] = []
  constructor() { Events.instances.push(this) }
  addEventListener() {}
  close() {}
}

beforeEach(() => {
  setActivePinia(createPinia())
  Socket.instances = []; Events.instances = []
  invoke.mockClear()
  Object.defineProperty(HTMLElement.prototype, 'scrollTo', { configurable: true, value: vi.fn() })
})

describe('actual planning renderer with application-owned runtime', () => {
  it.each(['manual', 'step'] as const)('%s: updates WS stages, survives eviction without resending, and restores host confirmation', async (mode) => {
    const owner = getChatTabState('remount')
    setChatTabConversation('/chat/remount', 'conversation-a')
    let finishCheckpoint: (() => void) | undefined
    const fetchMock = vi.fn(async (url: any, init?: RequestInit) => {
      if (String(url).endsWith('/api/checkpoint')) {
        await new Promise<void>(resolve => { finishCheckpoint = resolve })
        return new Response('{}')
      }
      if (String(url).includes('/events?')) {
        const stream = new ReadableStream({ start(controller) {
          init?.signal?.addEventListener('abort', () => controller.close(), { once: true })
        } })
        return new Response(stream, { headers: { 'Content-Type': 'text/event-stream' } })
      }
      return new Response(JSON.stringify(
        String(url).includes('execution-state') ? { has_state: false } : { id: 'session-a', messages: [] },
      ))
    })
    const manager = new FastPlanSessionManager(options => createFastPlanExecutionController({
      ...options, WebSocket: Socket as any, EventSource: Events as any, fetch: fetchMock,
    }))
    manager.bindSession('session-a', { conversationId: 'conversation-a', tabPath: '/chat/remount', entryId: 'entry-a' })
    owner.state.fastPlanExecutionMode.value = mode
    owner.state.dialogueCharacter.value.taskPlan = true
    owner.state.activeFastPlanEntryId.value = 'entry-a'
    owner.state.fastPlanEntries.value = [{ kind: 'embedded', id: 'entry-a', renderer: 'fastplan', status: 'active', data: {
      sessionId: 'session-a', executionMode: mode, confirmationPolicy: 'strict', connectionMode: 'live',
      prompt: '对比两地天气', model: { id: 'model-a', model: 'model-a' },
      command: { id: 'send-a', type: 'send', prompt: '对比两地天气' },
    } }]
    const renderErrors = vi.fn()
    const renderWarnings = vi.fn()
    const targetText = () => document.querySelector('#test-plan-composer')?.textContent
    const createView = () => {
      const options = reactive({ sessionManager: manager, executionMode: mode,
        confirmationPolicy: 'strict' as const, composerMode: 'host' as const,
        composerTarget: '#test-plan-composer', composerEntryId: retainedComposerEntry(owner),
        access: {
          getConnection: async () => ({ apiBase: 'http://test', wsBase: 'ws://test', parentToken: 'test-token' }),
          resolveModel: async () => ({ model: 'model-a', api_key: 'test', base_url: 'http://test' }) as any,
        },
      })
      const plugin = createFastPlanTimelinePlugin(options)
      return mount(defineComponent({ setup: () => () => {
        const entry = owner.state.fastPlanEntries.value[0]
        // Match ChatWorkspace: the timeline renders BEFORE the host composer sibling.
        return h('section', [h(plugin.timelineRenderers.fastplan, { entry, data: entry.data }),
          h('div', { id: 'test-plan-composer' })])
      } }), { attachTo: document.body, global: { config: { errorHandler: renderErrors, warnHandler: renderWarnings } } })
    }
    let wrapper: ReturnType<typeof createView> | null = null
    try {
      wrapper = createView()
      await vi.waitFor(() => expect(Socket.instances).toHaveLength(1))
      const socket = Socket.instances[0]
      socket.open()
      socket.frame({ type: 'authenticated' })
      await vi.waitFor(() => expect(socket.sent.filter(event => event.type === 'chat')).toHaveLength(1))
      await nextTick()
      expect((owner.state.fastPlanEntries.value[0].data as any).command).toBeUndefined()
      socket.frame({ type: 'route_stage_update', stage: 'intent', status: 'done', summary: '意图完成测试', payload: {} })
      socket.frame({ type: 'thinking_update', phase: 'planning', data: { kind: 'plan_stage', stage: 'intent', status: 'done', content: '意图完成测试' } })
      socket.frame({ type: 'thinking_update', phase: 'planning', data: { kind: 'plan_stage', stage: 'capability', status: 'done', content: '匹配完成测试' } })
      await vi.waitFor(() => expect(wrapper!.text()).toContain('匹配完成测试'))
      expect(invoke).not.toHaveBeenCalledWith('sync_desktop_plan_assistant_message', expect.anything())
      const runtime = manager.get('session-a')!
      wrapper.unmount(); wrapper = null
      const tree = { id: 'tree-a', root: { id: 'root', children: [{ id: 'phase-1', name: '天气获取', children: [
        { id: 'node-1', name: '查询天气', node_type: { type: 'LlmTask', config: { prompt_template: '查询天气' } } },
      ] }] }, metadata: {} }
      socket.frame({ type: 'plan_generated', tree })
      await vi.waitFor(() => expect(runtime.state.isPreviewing).toBe(true))
      wrapper = createView()
      await vi.waitFor(() => expect(targetText()).toContain('确认'))
      expect(renderErrors).not.toHaveBeenCalled()
      expect(renderWarnings.mock.calls.filter(([message]) => String(message).includes('Teleport'))).toHaveLength(0)
      expect(wrapper.findAllComponents({ name: 'NodeEditor' })).toHaveLength(0)
      expect(runtime.state.isPreviewing).toBe(true)
      const dispatch = vi.spyOn(manager, 'executeEntryControl')
      const confirmButton = Array.from(document.querySelectorAll('#test-plan-composer button'))
        .find(button => button.textContent?.includes('确认并执行')) as HTMLButtonElement
      expect(confirmButton).toBeTruthy()
      confirmButton.click()
      await vi.waitFor(() => expect(socket.sent.filter(event => event.type === 'confirm_plan')).toHaveLength(1))
      expect(dispatch).toHaveBeenCalledWith('session-a', 'entry-a', expect.objectContaining({ type: 'confirm', tree }))
      socket.frame({ type: 'tree_started', tree_id: 'tree-a' })
      if (mode === 'step') {
        wrapper.unmount(); wrapper = null
        socket.frame({ type: 'node_started', tree_id: 'tree-a', node_id: 'node-1', node_name: '查询天气' })
        socket.frame({ type: 'node_completed', tree_id: 'tree-a', node_id: 'node-1', output: '天气查询已完成' })
        socket.frame({ type: 'tree_paused', tree_id: 'tree-a', node_id: 'node-1' })
        await vi.waitFor(() => expect(runtime.state.isPaused).toBe(true))
        wrapper = createView()
        await vi.waitFor(() => expect(targetText()).toContain('继续下一步'))
        expect(runtime.state.executionState['node-1'].status).toBe('completed')
        expect(runtime.state.lastCompletedNodeId).toBe('node-1')
        const continueButton = Array.from(document.querySelectorAll('#test-plan-composer button'))
          .find(button => button.textContent?.includes('继续下一步')) as HTMLButtonElement
        continueButton.click()
        await vi.waitFor(() => expect(socket.sent.filter(event => event.type === 'resume_tree')).toHaveLength(1))
        expect(dispatch).toHaveBeenCalledWith('session-a', 'entry-a', { type: 'resume' })
      }
      socket.frame({ type: 'waiting_for_human', tree_id: 'tree-a', node_id: 'node-1',
        interaction: { kind: 'input', fields: [{ key: 'city', label: '城市', type: 'text', required: true }] } })
      await vi.waitFor(() => expect(document.querySelector('#test-plan-composer form input')).toBeTruthy())
      const input = document.querySelector('#test-plan-composer form input') as HTMLInputElement
      input.value = '西安'
      input.dispatchEvent(new Event('input', { bubbles: true }))
      await nextTick()
      const form = document.querySelector('#test-plan-composer form')!
      form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }))
      form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }))
      await vi.waitFor(() => expect(finishCheckpoint).toBeTypeOf('function'))
      const checkpointCalls = fetchMock.mock.calls.filter(([url]) => String(url).endsWith('/api/checkpoint'))
      expect(checkpointCalls).toHaveLength(1)
      expect(JSON.parse(String(checkpointCalls[0][1]?.body))).toEqual({
        tree_id: 'tree-a', node_id: 'node-1', data: { action: 'submit', values: { city: '西安' } },
      })
      wrapper.unmount(); wrapper = null
      finishCheckpoint!()
      await vi.waitFor(() => expect(runtime.state.humanRequestQueue).toHaveLength(0))
      wrapper = createView()
      await nextTick()
      expect(document.querySelector('#test-plan-composer form')).toBeNull()
      expect(runtime.state.confirmedHistory.some(item => item.values?.city === '西安')).toBe(true)
      expect(manager.get('session-a')).toBe(runtime)
      expect(runtime.state.messages.filter(message => message.role === 'user')).toHaveLength(1)
      expect(socket.sent.filter(event => event.type === 'chat')).toHaveLength(1)
      expect(Socket.instances).toHaveLength(1)
      // Managed access uses an authenticated fetch stream, not native EventSource.
      expect(fetchMock.mock.calls.filter(([url]) => String(url).includes('/events?'))).toHaveLength(1)
      expect(Events.instances).toHaveLength(0)
      expect(renderErrors).not.toHaveBeenCalled()
      expect(renderWarnings.mock.calls.filter(([message]) => String(message).includes('Teleport'))).toHaveLength(0)
    } finally {
      finishCheckpoint?.()
      wrapper?.unmount(); manager.disposeAll()
      releaseChatTabState('remount'); unregisterChatTab('/chat/remount')
    }
  })
})
