import { invoke } from '@tauri-apps/api/core'
import type { ChatEmbeddedTimelineEntry } from 'chat-renderer'
import type { ChatTabState } from '@/modules/chat/tabState'
import { fastPlanSessionManager, type FastPlanSessionManager } from './sessionManager'
import { writeFastPlanPersistence } from './persistence'

export interface PlanningConversationStore {
  conversations: Array<{ id: string; extra_config?: string | null }>
  applyFirstMessageTitle: (conversationId: string, prompt: string) => Promise<unknown>
}

const writes = new Map<string, Promise<unknown>>()
const leases = new WeakMap<ChatTabState, { conversationId: string; release: () => void }>()

/** The writer is owned by a Tab record, not by any particular renderer instance. */
export function ensureManagedFastPlanWriter(
  owner: ChatTabState,
  conversationId: string,
  manager: FastPlanSessionManager = fastPlanSessionManager,
) {
  if (owner.disposed) throw new Error('规划标签页已关闭')
  if (!manager.acquireWriter(conversationId, owner.planWriterId)) {
    throw new Error('当前规划会话正在其他标签页中使用')
  }
  const previous = leases.get(owner)
  if (previous?.conversationId !== conversationId) {
    previous?.release()
    const release = () => {
      manager.releaseWriter(conversationId, owner.planWriterId)
      remove()
      if (leases.get(owner)?.release === release) leases.delete(owner)
    }
    const remove = owner.onInvalidate(release)
    leases.set(owner, { conversationId, release })
  }
}

export async function persistManagedFastPlanState(
  owner: ChatTabState,
  store: Pick<PlanningConversationStore, 'conversations'>,
  conversationId: string,
  entries: ChatEmbeddedTimelineEntry[],
  activeEntryId: string | null,
  manager: FastPlanSessionManager = fastPlanSessionManager,
): Promise<void> {
  ensureManagedFastPlanWriter(owner, conversationId, manager)
  const current = owner.capture()
  const task = (writes.get(conversationId) || Promise.resolve()).catch(() => {}).then(async () => {
    if (!current()) throw new Error('规划会话上下文已改变')
    const conversation = store.conversations.find(item => item.id === conversationId)
    if (!conversation) throw new Error('找不到需要保存的规划会话')
    const extraConfig = writeFastPlanPersistence(conversation.extra_config, entries, activeEntryId, owner.planWriterId)
    await invoke('update_conversation_extra_config', { id: conversationId, extraConfig })
    // A failed IPC must never look like a successfully persisted binding in memory.
    const retainedConversation = store.conversations.find(item => item.id === conversationId)
    if (current() && retainedConversation) retainedConversation.extra_config = extraConfig
  }).finally(() => { if (writes.get(conversationId) === task) writes.delete(conversationId) })
  writes.set(conversationId, task)
  return task
}
