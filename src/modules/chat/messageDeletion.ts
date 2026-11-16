import { shallowReactive } from 'vue'
import type { Message } from '@/types'
import type { MessageActionTarget, MessageActionOptions } from './messageActions'

export class MessageDeletionBusyError extends Error {}
export class MessageDeletionUnavailableError extends Error {}

/** Session-local deletion receipts prevent late reads/prepared snapshots from resurrecting deleted IDs. */
export class DeletedMessageLedger {
  private deleted = new Map<string, Set<string>>()
  prune(conversationId: string, list: Message[]): Message[] {
    const ids = this.deleted.get(conversationId)
    if (ids) for (let i = list.length - 1; i >= 0; i--) {
      if (list[i].conversation_id === conversationId && ids.has(list[i].id)) list.splice(i, 1)
    }
    return list
  }
  commit(conversationId: string, ids: string[], lists: Iterable<Message[]>) {
    const receipt = this.deleted.get(conversationId) || new Set<string>()
    ids.forEach(id => receipt.add(id)); this.deleted.set(conversationId, receipt)
    for (const list of new Set(lists)) this.prune(conversationId, list)
  }
}

export class MessageDeletionService {
  private pending = shallowReactive(new Map<string, { accepted: boolean; valid(): boolean }>())
  isPending(conversationId: string | null) {
    const entry = conversationId ? this.pending.get(conversationId) : undefined
    return !!entry && (entry.accepted || entry.valid())
  }

  async remove(target: MessageActionTarget, id: string, options: Pick<MessageActionOptions, 'getInvoke' | 'canStart'> & {
    busy(): boolean
    commit(conversationId: string, ids: string[]): void
  }): Promise<boolean> {
    const { owner, store } = target, conversationId = store.currentConversationId, currentOwner = owner.capture()
    const find = () => store.messages.find(message => message.id === id && message.conversation_id === conversationId)
    const message = find(), content = message?.content, role = message?.role, groupId = message?.group_id
    // Ordinary sends retain a temporary user ID until history reload. Capture only an unambiguous
    // question in this exact group; never infer it from the nearest visible message.
    const localQuestions = role === 'assistant' && groupId
      ? store.messages.filter(item => item.conversation_id === conversationId && item.role === 'user' && item.group_id === groupId)
      : []
    const localQuestionId = localQuestions.length === 1 ? localQuestions[0].id : undefined
    const valid = () => !!conversationId && currentOwner() && store.currentConversationId === conversationId
      && !!find() && find()!.content === content && find()!.role === role && find()!.group_id === groupId && options.canStart()
    if (!message || !valid()) return false
    if (this.isPending(conversationId)) return false
    if (options.busy() || message.status === 'streaming') throw new MessageDeletionBusyError('会话正在执行，请结束后再删除消息')
    const token = { accepted: false, valid }; this.pending.set(conversationId!, token)
    try {
      const invoke = await options.getInvoke()
      if (!valid()) return false
      if (options.busy() || find()!.status === 'streaming') throw new MessageDeletionBusyError('会话正在执行，请结束后再删除消息')
      if (!invoke) throw new MessageDeletionUnavailableError('消息删除需要 Tauri 后端支持')
      token.accepted = true
      const ids = await invoke('delete_message_turn', { conversationId, messageId: id, expectedContent: content })
      // Protocol order is [targetId, optionalDeletedQuestionId], not arbitrary cascade IDs.
      if (!Array.isArray(ids) || ids[0] !== id || ids.length > 2 || !ids.every(item => typeof item === 'string' && item)
        || (ids.length === 2 && (role !== 'assistant' || !groupId || ids[0] === ids[1]))) {
        throw new Error('删除结果无效，请重新加载历史核对')
      }
      // Once accepted, apply only to the original conversation, even when its presentation has gone.
      const committed = ids.length === 2 && localQuestionId ? [...new Set([...ids, localQuestionId])] : ids
      options.commit(conversationId!, committed)
      return true
    } finally { if (this.pending.get(conversationId!) === token) this.pending.delete(conversationId!) }
  }
}

export const deletedMessageLedger = new DeletedMessageLedger()
export const messageDeletionService = new MessageDeletionService()
