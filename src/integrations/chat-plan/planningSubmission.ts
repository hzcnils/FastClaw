import { markRaw, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  createFastPlanClient, uploadFastPlanFile, type FastPlanAccess, type FastPlanExecutionController,
  type FastPlanExecutionControllerOptions,
} from 'planflow-runtime'
import type { FastPlanTimelineEntryData } from 'planflow-kit'
import type { ChatTabState } from '@/modules/chat/tabState'
import { useExecutionActivityStore } from '@/stores/executionActivity'
import { getChatTabConversation } from '@/utils/chatTabRegistry'
import { fastClawFastPlanAccess } from './fastplanAccess'
import { fastPlanSessionManager, type FastPlanSessionManager } from './sessionManager'
import { ensureManagedFastPlanWriter, persistManagedFastPlanState, type PlanningConversationStore } from './managedPersistence'

type Attachment = Record<string, unknown> & { file?: File }
export interface PlanningSubmission {
  owner: ChatTabState
  store: PlanningConversationStore
  tabPath: string
  conversationId: string
  entryId: string
  prompt: string
  attachments: Attachment[]
  model: FastPlanTimelineEntryData['model']
  executionMode: 'auto' | 'manual' | 'step'
  thinking: boolean
  apiBase?: string
  wsBase?: string
  uploadBase?: string
}
type Dependencies = {
  access?: FastPlanAccess
  createSession?: (title: string, signal: AbortSignal) => Promise<any>
  upload?: typeof uploadFastPlanFile
  persist?: typeof persistManagedFastPlanState
  controllerOptions?: FastPlanExecutionControllerOptions
}
const cancelled = () => new DOMException('规划提交已取消', 'AbortError')
function wait<T>(promise: Promise<T>, signal: AbortSignal): Promise<T> {
  return new Promise((resolve, reject) => {
    const abort = () => reject(cancelled())
    const cleanup = () => signal.removeEventListener('abort', abort)
    promise.then(value => { cleanup(); resolve(value) }, error => { cleanup(); reject(error) })
    if (signal.aborted) abort()
    else signal.addEventListener('abort', abort, { once: true })
  })
}
function waitForConnection(runtime: FastPlanExecutionController, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    let stop = () => {}
    const finish = (error?: Error) => {
      stop(); clearTimeout(timer); signal.removeEventListener('abort', abort)
      error ? reject(error) : resolve()
    }
    const abort = () => finish(cancelled())
    const timer = setTimeout(() => finish(new Error('FastPlan WebSocket 连接超时')), 10000)
    stop = watch([runtime.connected, runtime.loading, () => runtime.state.connectionStatus], () => {
      if (runtime.connected.value && !runtime.loading.value) finish()
      else if (runtime.state.connectionStatus === 'failed') finish(new Error('FastPlan 连接失败'))
    }, { flush: 'sync' })
    signal.addEventListener('abort', abort, { once: true })
    if (signal.aborted) abort()
    else if (runtime.connected.value && !runtime.loading.value) finish()
  })
}

/** Owns the complete FP submission after the host conversation/provider preflight. */
export class PlanningSubmissionService {
  private readonly jobs = new Map<string, Promise<boolean>>()
  constructor(private readonly manager: FastPlanSessionManager = fastPlanSessionManager,
    private readonly dependencies: Dependencies = {}) { markRaw(this) }

  submit(input: PlanningSubmission): Promise<boolean> {
    const key = `${input.tabPath}:${input.entryId}`
    const pending = this.jobs.get(key)
    if (pending) return pending
    // Snapshot send parameters; retain only the Tab record and application store, never a view.
    const request = { ...input, attachments: input.attachments.map(item => ({ ...item })),
      model: typeof input.model === 'object' ? { ...input.model } : input.model }
    const promise = this.run(request).finally(() => { if (this.jobs.get(key) === promise) this.jobs.delete(key) })
    this.jobs.set(key, promise)
    return promise
  }

  private async run(input: PlanningSubmission): Promise<boolean> {
    const { owner, conversationId, entryId, tabPath, store } = input
    const contextCurrent = owner.capture()
    const controller = new AbortController()
    const signal = controller.signal
    const requestId = crypto.randomUUID()
    const groupId = crypto.randomUUID()
    const access = this.dependencies.access || fastClawFastPlanAccess
    const binding = { conversationId, entryId, tabPath, executionMode: input.executionMode,
      modelId: typeof input.model === 'object' ? input.model.id || null : input.model || null, groupId }
    const entry = () => owner.state.fastPlanEntries.value.find(item => item.id === entryId)
    const ownsEntry = () => contextCurrent() && getChatTabConversation(tabPath) === conversationId && !!entry()
    const current = () => !signal.aborted && ownsEntry()
    const assertCurrent = () => { if (!current()) throw cancelled() }
    const patch = (data: Partial<FastPlanTimelineEntryData>, status?: 'active' | 'error') => {
      if (!ownsEntry()) return
      owner.state.fastPlanEntries.value = owner.state.fastPlanEntries.value.map(item => item.id === entryId
        ? { ...item, ...(status ? { status } : {}), data: { ...item.data as FastPlanTimelineEntryData, ...data } } : item)
    }
    let runtime: FastPlanExecutionController | undefined
    let transportAccepted = false
    let removeSendListener = () => {}
    let sessionId = String((entry()?.data as FastPlanTimelineEntryData | undefined)?.sessionId || '')
    let abortUpload: (() => void) | undefined
    let unregister = () => {}
    const cancel = () => {
      controller.abort()
      unregister()
      abortUpload?.()
      abortUpload = undefined
      patch({ command: undefined, pendingAttachments: undefined, uploadProgress: undefined, preflightError: '规划提交已取消' }, 'error')
      useExecutionActivityStore().remove(conversationId, 'fastplan')
    }
    assertCurrent()
    ensureManagedFastPlanWriter(owner, conversationId, this.manager)
    unregister = this.manager.registerSubmission(binding, cancel)
    const removeInvalidation = owner.onInvalidate(() => {
      cancel()
      // Closing/resetting the owner is different from releasing a renderer.
      runtime?.disconnect()
    })
    useExecutionActivityStore().update(conversationId, tabPath, 'fastplan', {
      phase: 'submitting', protected: true, entryId, executionMode: input.executionMode,
    })
    patch({ commandOwner: 'host', connectionMode: 'manual', prompt: input.prompt, pendingPrompt: input.prompt, pendingAttachments: input.attachments,
      preflightError: undefined, model: input.model, executionMode: input.executionMode,
      command: { id: requestId, type: 'send', prompt: input.prompt, thinking: input.thinking },
      metadata: { conversationId, groupId } }, 'active')
    try {
      if (!sessionId) {
        const create = this.dependencies.createSession || ((title: string, abort: AbortSignal) =>
          createFastPlanClient({ access, apiBase: input.apiBase }).createSession(title, abort))
        const created = await wait(create(input.prompt.slice(0, 50), signal), signal)
        assertCurrent()
        sessionId = String(created?.id || created?.session_id || '')
        if (!sessionId) throw new Error('FastPlan 创建会话成功但未返回 session id')
        owner.fastPlanEntryOwners.set(entryId, conversationId)
        // Until persistence succeeds the renderer must not load/connect this session.
        patch({ sessionId, connectionMode: 'manual' })
      }
      await wait((this.dependencies.persist || persistManagedFastPlanState)(owner, store, conversationId,
        owner.state.fastPlanEntries.value, entryId, this.manager), signal)
      assertCurrent()
      this.manager.bindSession(sessionId, binding)
      runtime = this.manager.getOrCreate(sessionId, { ...this.dependencies.controllerOptions, access,
        apiBase: input.apiBase, wsBase: input.wsBase, model: input.model,
        executionMode: input.executionMode, confirmationPolicy: 'strict' })
      removeSendListener = runtime.on('message-sent', payload => {
        if (String(payload?.request_id || '') === requestId) transportAccepted = true
      })
      this.manager.retain(sessionId, requestId)
      // Start loading before publishing live entry data so mounting cannot race a second load.
      const hasSnapshot = runtime.connected.value || runtime.state.messages.length || runtime.state.plannedTree
        || runtime.state.isPlanning || runtime.state.executionHistory.length
      if (!runtime.loading.value && !hasSnapshot) {
        const loading = runtime.loadSession(sessionId, { connect: true })
        patch({ connectionMode: 'live' })
        await wait(loading, signal)
      } else {
        if (!runtime.loading.value && !runtime.connected.value) runtime.connect(sessionId)
        patch({ connectionMode: 'live' })
      }
      assertCurrent()
      await waitForConnection(runtime, signal)
      assertCurrent()
      const attachments: Record<string, unknown>[] = []
      for (const item of input.attachments) {
        assertCurrent()
        if (item.path || item.url) { attachments.push(item); continue }
        const metadata = item.metadata as Record<string, unknown> | undefined
        const file = item.file || item.rawFile || metadata?.rawFile
        if (!(file instanceof File)) throw new Error(`附件「${item.name || '未命名'}」内容不可用，请重新添加`)
        const upload = (this.dependencies.upload || uploadFastPlanFile)(file, {
          access, apiBase: input.uploadBase || input.apiBase,
          onProgress: progress => { if (current()) patch({ uploadProgress: { id: String(item.id || file.name), progress } }) },
        })
        abortUpload = () => upload.xhr.abort()
        const result = await wait(upload.promise, signal)
        abortUpload = undefined
        assertCurrent()
        attachments.push({ id: item.id || file.name, name: item.name || file.name, filename: item.name || file.name,
          size: item.size || file.size, mimeType: item.mimeType || file.type, path: result.path })
      }
      assertCurrent()
      const sent = await wait(Promise.resolve(runtime.sendChat(input.prompt, { requestId, model: input.model,
        attachments, thinking: input.thinking, userOverride: 'plan', metadata: { conversationId, groupId } })), signal)
      assertCurrent()
      if (!sent) throw new Error('规划提交已失效，请重新发送')
      patch({ command: undefined, pendingPrompt: undefined, pendingAttachments: undefined, uploadProgress: undefined })
      // Use fixed IDs, including when the view was evicted before the transport accepted the send.
      void invoke('sync_desktop_plan_user_message', { conversationId, content: input.prompt, groupId })
        .then(() => store.applyFirstMessageTitle(conversationId, input.prompt))
        .catch(error => console.warn('[FastPlan] sync submitted user message failed', error))
      return true
    } catch (error) {
      if (signal.aborted || !ownsEntry()) return false
      patch({ command: undefined, preflightError: error instanceof Error ? error.message : String(error),
        uploadProgress: undefined }, 'error')
      useExecutionActivityStore().remove(conversationId, 'fastplan')
      throw error
    } finally {
      abortUpload?.()
      unregister(); removeInvalidation()
      removeSendListener()
      // No new prompt was accepted: stop connection retries and invalidate a
      // pending model lookup. This runs after the synchronous stop packet attempt.
      if (runtime && !transportAccepted) runtime.disconnect()
      if (runtime) this.manager.refreshExecutionActivity(sessionId)
      if (sessionId) this.manager.release(sessionId, requestId)
    }
  }
}

export const planningSubmissionService = new PlanningSubmissionService()
