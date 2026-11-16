import { computed } from 'vue'
import type { Message } from '@/types'
import { planConfirmationFromRuntime } from '@/integrations/chat-plan/remotePlanRuntime'
import type { ChatContext, createChatContextBinding } from './contextBinding'
import type { ChatTabState } from './tabState'

interface PlanningUiStore {
  currentConversationId: string | null
  messages: Message[]
  isLoading: boolean
  stopStream(conversationId?: string): unknown
}

function lastUserId(store: Pick<PlanningUiStore, 'messages' | 'currentConversationId'>) {
  for (let index = store.messages.length - 1; index >= 0; index--) {
    const message = store.messages[index]
    if (message.role === 'user' && message.conversation_id === store.currentConversationId) return message.id
  }
  return ''
}

function stopEntry(owner: ChatTabState) {
  const { fastPlanEntries, activeFastPlanEntryId } = owner.state
  return fastPlanEntries.value.find(entry => entry.id === activeFastPlanEntryId.value)?.id
    || fastPlanEntries.value.find(entry => entry.status === 'active')?.id
    || fastPlanEntries.value.at(-1)?.id || null
}

/** Only the workspace stop button; closing a Tab and background controls have their own lifetime. */
export function createWorkspaceAbortBinding<S extends PlanningUiStore>(
  binding: ReturnType<typeof createChatContextBinding<S>>,
  effects: {
    cancelPending(owner: ChatTabState): unknown
    stopPlan(conversationId: string, entryId: string): Promise<unknown>
    reportError(error: unknown): void
  },
) {
  return computed(() => {
    const lease = binding.captureUi()
    if (!lease) return undefined
    const { owner, store } = lease.context, state = owner.state
    const pending = state.pendingAutoSend.value, planning = state.dialogueCharacter.value.taskPlan
    const entryId = planning ? stopEntry(owner) : null, userId = lastUserId(store)
    const current = () => lease.isCurrent() && lastUserId(store) === userId
      && state.pendingAutoSend.value === pending && state.dialogueCharacter.value.taskPlan === planning
      && (!planning || stopEntry(owner) === entryId)
    return async () => {
      if (!current()) return
      try {
        if (pending) { effects.cancelPending(owner); return }
        // stopConversation also cancels pending session creation: a live runtime is not required.
        if (entryId && lease.conversationId) { await effects.stopPlan(lease.conversationId, entryId); return }
        if (store.isLoading) await store.stopStream(lease.conversationId || undefined)
      } catch (error) { if (current()) effects.reportError(error) }
    }
  })
}

/** UI commands capture a wait/turn, while an accepted IPC remains owned by the original Tab. */
export function createRemotePlanConfirmationBinding<S extends PlanningUiStore>(
  binding: ReturnType<typeof createChatContextBinding<S>>,
  resolveRuntime: (context: ChatContext<S>) => Record<string, any> | null,
  send: (conversationId: string, action: string, expectedWaitId?: string) => Promise<unknown>,
  reportError: (error: unknown) => void,
) {
  function read(context: ChatContext<S>) {
    if (!context.store.currentConversationId || context.owner.state.fastPlanEntries.value.length) return null
    const runtime = resolveRuntime(context), confirmation = planConfirmationFromRuntime(runtime)
    if (!runtime || !confirmation) return null
    const waitKey = JSON.stringify([context.store.currentConversationId, lastUserId(context.store),
      confirmation.id, runtime.currentTreeId || runtime.plannedTree?.id || runtime.plannedTree?.tree_id || '',
      runtime.controlWaitId || ''])
    const expectedWaitId = runtime.controlProtocol === 1 ? String(runtime.controlWaitId) : undefined
    return { runtime, confirmation, waitKey, expectedWaitId }
  }
  const session = computed(() => {
    const frame = binding.active.value, lease = binding.captureUi()
    if (!frame || !lease?.isCurrent()) return null
    const wait = read(lease.context)
    if (!wait) return null
    const receipt = lease.context.owner.state.remotePlanControl
    const current = () => lease.isCurrent() && read(lease.context)?.waitKey === wait.waitKey
    async function select(option: { action?: string; value?: string }) {
      if (!current()) return
      const selected = wait!.confirmation.options.find(item => item.value === option?.value
        && (!option.action || item.action === option.action))
      if (!selected) return
      const previous = receipt.value
      if (previous?.waitKey === wait!.waitKey && (previous.status === 'pending'
        || !!wait!.expectedWaitId || previous.action === selected.action || previous.action === 'cancel')) return
      receipt.value = { waitKey: wait!.waitKey, action: selected.action, status: 'pending' }
      const accepted = receipt.value
      try {
        // No await between UI validation and dispatch; never look up the active conversation later.
        if (wait!.expectedWaitId) await send(lease!.conversationId!, selected.action, wait!.expectedWaitId)
        else await send(lease!.conversationId!, selected.action)
        if (receipt.value === accepted) accepted.status = 'sent'
      } catch (error) {
        if (receipt.value === accepted) receipt.value = null
        if (current()) reportError(error)
      }
    }
    return { ...wait, key: `${frame.generation}:${wait.waitKey}`, receipt,
      events: { 'confirmation-select': select } }
  })
  const panel = computed(() => {
    const active = session.value
    if (!active) return null
    return { key: active.key, runtime: active.runtime, confirmation: active.confirmation, events: active.events,
      pending: active.receipt.value?.waitKey === active.waitKey && active.receipt.value.status === 'pending' }
  })
  return { panel }
}
