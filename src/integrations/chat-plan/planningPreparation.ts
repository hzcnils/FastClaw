import { effectScope, markRaw, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { ChatEmbeddedTimelineEntry } from 'chat-renderer'
import type { FastPlanTimelineEntryData } from 'planflow-kit'
import type { ChatAttachment } from '@/types'
import type { PreparedConversationMessages } from '@/stores/chat'
import { saveDocumentEditState } from '@/modules/chat/documentEditRuntime'
import { claimConversationTab, getChatTabConversation } from '@/utils/chatTabRegistry'
import { useExecutionActivityStore } from '@/stores/executionActivity'
import { fastPlanSessionManager, type FastPlanSessionManager } from './sessionManager'
import { planningSubmissionService, type PlanningSubmission } from './planningSubmission'
import { ensureManagedFastPlanWriter, type PlanningConversationStore } from './managedPersistence'

export interface PlanningPreparationStore extends PlanningConversationStore {
  currentConversationId: string | null
  loadConversations(force?: boolean): Promise<void>
  prepareMessages(id: string): Promise<PreparedConversationMessages | null>
  commitPreparedMessages(prepared: PreparedConversationMessages): void
  setConvFileCache(id: string, files: ChatAttachment[]): void
}
export interface PlanningPreparation extends Omit<PlanningSubmission, 'store' | 'conversationId' | 'entryId'> {
  store: PlanningPreparationStore
  conversationId: string | null
  raw: string
  quote: string
  draftAttachments: ChatAttachment[]
}

function wait<T>(promise: Promise<T>, signal: AbortSignal): Promise<T> {
  return new Promise((resolve, reject) => {
    const abort = () => reject(new DOMException('规划准备已取消', 'AbortError'))
    const cleanup = () => signal.removeEventListener('abort', abort)
    promise.then(value => { cleanup(); resolve(value) }, error => { cleanup(); reject(error) })
    if (signal.aborted) abort()
    else signal.addEventListener('abort', abort, { once: true })
  })
}

/** Owns host-side plan preparation, with no view or router callbacks. */
export class PlanningPreparationService {
  private jobs = new WeakMap<PlanningPreparation['owner'], Promise<boolean>>()
  constructor(private manager: FastPlanSessionManager = fastPlanSessionManager,
    private submission: Pick<typeof planningSubmissionService, 'submit'> = planningSubmissionService,
    private ipc: typeof invoke = invoke) { markRaw(this) }

  submit(input: PlanningPreparation): Promise<boolean> {
    const existing = this.jobs.get(input.owner)
    if (existing) return existing
    const snapshot = { ...input, attachments: input.attachments.map(item => ({ ...item })),
      draftAttachments: input.draftAttachments.map(item => ({ ...item })),
      model: typeof input.model === 'object' ? { ...input.model } : input.model }
    const task = this.run(snapshot).finally(() => { if (this.jobs.get(input.owner) === task) this.jobs.delete(input.owner) })
    this.jobs.set(input.owner, task)
    return task
  }

  private async run(input: PlanningPreparation): Promise<boolean> {
    const { owner, store, tabPath, executionMode } = input
    const state = owner.state
    const requestId = crypto.randomUUID()
    const entryId = state.activeFastPlanEntryId.value || `fastplan:${crypto.randomUUID()}`
    const sourceId = input.conversationId
    let conversationId = sourceId
    let contextCurrent = owner.capture()
    const signalOwner = new AbortController()
    const signal = signalOwner.signal
    const activity = useExecutionActivityStore()
    const activityId = sourceId || `draft:${owner.instanceId}:${requestId}`
    const owns = () => contextCurrent() && store.currentConversationId === conversationId
      && getChatTabConversation(tabPath) === conversationId
    const assertCurrent = () => {
      if (signal.aborted || !owns()) throw new DOMException('规划准备已取消', 'AbortError')
    }
    assertCurrent()
    if (sourceId) ensureManagedFastPlanWriter(owner, sourceId, this.manager)
    let removeInvalidation = () => {}
    let unregister = () => {}
    let claim: ReturnType<typeof claimConversationTab> | undefined
    const scope = effectScope(true)
    let handedOff = false
    const patchError = (message: string) => {
      if (!owns()) return
      state.fastPlanEntries.value = state.fastPlanEntries.value.map(entry => entry.id === entryId
        ? { ...entry, status: 'error', data: { ...entry.data as FastPlanTimelineEntryData,
          command: undefined, preflightError: message } } : entry)
    }
    const cancel = () => {
      signalOwner.abort()
      unregister()
      activity.remove(activityId, 'fastplan')
      patchError('规划准备已取消')
    }
    const createPendingEntry = () => {
      const previous = state.fastPlanEntries.value.find(entry => entry.id === entryId)
      const entry: ChatEmbeddedTimelineEntry = { kind: 'embedded', renderer: 'fastplan', id: entryId, status: 'active',
        data: { ...previous?.data as FastPlanTimelineEntryData,
          commandOwner: 'host', connectionMode: 'manual', command: undefined,
          prompt: input.prompt, pendingPrompt: input.prompt, pendingAttachments: input.attachments,
          preflightError: undefined, createdAt: new Date().toISOString(), model: input.model,
          executionMode, confirmationPolicy: 'strict',
        } satisfies FastPlanTimelineEntryData }
      state.fastPlanEntries.value = previous
        ? state.fastPlanEntries.value.map(item => item.id === entryId ? entry : item)
        : [...state.fastPlanEntries.value, entry]
      state.activeFastPlanEntryId.value = entryId
      if (conversationId) owner.fastPlanEntryOwners.set(entryId, conversationId)
    }
    try {
      unregister = this.manager.registerSubmission({ conversationId: sourceId || '', tabPath, entryId, executionMode }, cancel)
      removeInvalidation = owner.onInvalidate(cancel)
      state.planningPreparationId.value = requestId
      scope.run(() => watch([state.modelId, state.fastPlanExecutionMode,
        () => state.dialogueCharacter.value.taskPlan], cancel, { flush: 'sync' }))
      activity.update(activityId, tabPath, 'fastplan', { phase: 'submitting', protected: true, entryId, executionMode })
      createPendingEntry()
      state.messageStr.value = ''
      state.quotedText.value = ''
      state.fileList.value = [] // Keep raw Files alive through preparation and submission.
      // A newly attached file belongs to a changed draft, not to this accepted snapshot.
      scope.run(() => watch(() => state.fileList.value.map(item => item.id).join(','), cancel, { flush: 'sync' }))

      const result = await wait(this.ipc<{ session_id?: string; sessionId?: string }>('set_conversation_task_plan', {
        sessionId: sourceId, enabled: true,
        modelId: typeof input.model === 'object' ? input.model.id || null : input.model || null,
      }), signal)
      assertCurrent()
      const targetId = String(result?.session_id || result?.sessionId || '').trim()
      if (!targetId) throw new Error('创建规划会话成功但未返回会话 ID')
      claim = claimConversationTab(tabPath, targetId)
      if (claim.path !== tabPath) throw new Error('目标规划会话已在其他标签页打开，请在对应标签页操作')
      // The App already knows the returned successor while this Tab still displays
      // its source. Either ID must be able to cancel this one preparation job.
      unregister()
      unregister = this.manager.registerSubmission({ conversationId: sourceId || '', tabPath, entryId, executionMode }, cancel, [targetId])
      await wait(this.ipc('set_conversation_execution_mode', { sessionId: targetId, mode: executionMode }), signal)
      assertCurrent()
      await wait(store.loadConversations(true), signal)
      assertCurrent()
      const conversation = store.conversations.find(item => item.id === targetId)
      if (!conversation) throw new Error('找不到已创建的规划会话')

      if (targetId !== sourceId) {
        const prepared = await wait(store.prepareMessages(targetId), signal)
        assertCurrent()
        if (!prepared || prepared.conversationId !== targetId) throw new Error('读取规划会话失败，请重试')
        // Backend contract: a successor is newly created, never another conversation's history.
        if (prepared.messages.length) throw new Error('目标规划会话并非空会话，已停止提交以避免覆盖历史')
        saveDocumentEditState(owner, sourceId)
        if (sourceId) {
          const docs = input.draftAttachments.filter(item => item.type === 'document' && item.status === 'uploaded')
          if (docs.length) store.setConvFileCache(sourceId, docs)
        }
        // Only this synchronous, deliberate Store commit may transfer our cancellation lease.
        // Other operations still receive the Store's normal context invalidation.
        removeInvalidation()
        store.commitPreparedMessages(prepared)
        conversationId = targetId
        contextCurrent = owner.capture()
        removeInvalidation = owner.onInvalidate(cancel)
        assertCurrent()
        owner.hydrationRequired = false
        createPendingEntry()
      }
      let extra: Record<string, unknown> = {}
      try { const parsed = JSON.parse(conversation.extra_config || '{}'); if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) extra = parsed } catch { /* Repair invalid local metadata after successful IPC. */ }
      conversation.extra_config = JSON.stringify({ ...extra, task_plan: true, execution_mode: executionMode })
      assertCurrent()
      // No await between releasing preparation ownership and registering B2's submission.
      unregister(); removeInvalidation(); scope.stop()
      activity.remove(activityId, 'fastplan')
      state.planningPreparationId.value = null
      const task = this.submission.submit({ ...input, conversationId: targetId, entryId })
      handedOff = true
      const accepted = await task
      if (owns()) for (const attachment of input.draftAttachments) owner.releaseAttachment(attachment.id)
      return accepted
    } catch (error) {
      if (signal.aborted || !owns()) return false
      scope.stop() // Restoring our own draft is not a new user edit/cancellation.
      patchError(error instanceof Error ? error.message : String(error))
      if (!state.messageStr.value && !state.fileList.value.length) {
        state.messageStr.value = input.raw
        state.quotedText.value = input.quote
        state.fileList.value = input.draftAttachments
        for (const item of input.attachments) if (typeof item.id === 'string' && item.file instanceof File) owner.retainFile(item.id, item.file)
      }
      throw error
    } finally {
      unregister(); removeInvalidation(); scope.stop(); claim?.release()
      if (state.planningPreparationId.value === requestId) state.planningPreparationId.value = null
      if (!handedOff) activity.remove(activityId, 'fastplan')
    }
  }
}

export const planningPreparationService = new PlanningPreparationService()
