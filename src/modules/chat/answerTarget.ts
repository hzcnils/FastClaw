import type { Message } from '@/types'
import type { ChatTabState } from './tabState'

/** Never pair a reply with a nearby question from a different group. */
export function answerSource(messages: Message[], messageId: string) {
  const index = messages.findIndex(message => message.id === messageId && message.role === 'assistant')
  if (index < 0) return null
  const groupId = messages[index].group_id
  if (!groupId) return null
  const user = messages.slice(0, index).reverse().find(message => message.role === 'user' && message.group_id === groupId)
  return user ? { groupId, content: user.content } : null
}

export function captureAnswerTarget(owner: ChatTabState, store: { currentConversationId: string | null; messages: Message[] }, groupId: string, content: string) {
  const conversationId = store.currentConversationId, current = owner.capture()
  return { conversationId, groupId, content, isCurrent: () => !!conversationId && current()
    && store.currentConversationId === conversationId
    && store.messages.some(message => message.role === 'user' && message.group_id === groupId && message.content === content) }
}
