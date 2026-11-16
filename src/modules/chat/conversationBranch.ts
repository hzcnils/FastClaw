import type { MessageActionOptions, MessageActionTarget } from './messageActions'
import type { ChatTabState } from './tabState'

export class BranchUnavailableError extends Error {}

/** Accepted database work outlives its presentation; in-flight duplicate clicks do not create another branch. */
export class ConversationBranchService {
  private pending = new WeakMap<ChatTabState, Map<string, { accepted: boolean; valid(): boolean }>>()

  async create(target: MessageActionTarget, messageId: string,
    options: Pick<MessageActionOptions, 'getInvoke' | 'canStart'> & { refresh(): Promise<unknown> },
  ): Promise<string | null> {
    const { owner, store } = target, conversationId = store.currentConversationId
    const ownerCurrent = owner.capture()
    const find = () => store.messages.find(message => message.id === messageId && message.conversation_id === conversationId)
    const source = find(), content = source?.content
    const valid = () => !!conversationId && ownerCurrent() && store.currentConversationId === conversationId
      && !!find() && find()!.content === content && find()!.status !== 'streaming' && options.canStart()
    if (!source || !valid()) return null
    let pending = this.pending.get(owner)
    if (!pending) { pending = new Map(); this.pending.set(owner, pending) }
    const key = JSON.stringify([conversationId, messageId])
    const previous = pending.get(key)
    if (previous && (previous.accepted || previous.valid())) return null
    const request = { accepted: false, valid }; pending.set(key, request)
    try {
      const invoke = await options.getInvoke()
      if (!valid()) return null
      if (!invoke) throw new BranchUnavailableError('对话分支功能需要 Tauri 后端支持')
      request.accepted = true
      const id = await invoke('branch_conversation', { conversationId, branchPointMessageId: messageId, expectedContent: content })
      if (typeof id !== 'string' || !id) throw new Error('分支创建未返回有效会话 ID')
      // Refresh is shared application data, not a request to replace this Tab's current conversation.
      // A refresh failure cannot turn a committed branch into a failed creation/retry.
      void Promise.resolve().then(options.refresh).catch(error => console.warn('Branch created; history refresh failed', error))
      return id
    } finally { if (pending.get(key) === request) pending.delete(key) }
  }
}

export const conversationBranchService = new ConversationBranchService()
