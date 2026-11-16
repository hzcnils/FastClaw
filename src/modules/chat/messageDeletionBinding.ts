import { computed, effectScope, watch } from 'vue'
import type { createChatContextBinding } from './contextBinding'
import type { MessageActionStore } from './messageActions'
import { abortable } from './abortable'
import { MessageDeletionBusyError, MessageDeletionUnavailableError } from './messageDeletion'

export interface MessageDeletionStore extends MessageActionStore {
  deleteMessage(id: string, canStart?: () => boolean): Promise<boolean>
}
export function createMessageDeletionBinding<S extends MessageDeletionStore>(
  binding: ReturnType<typeof createChatContextBinding<S>>,
  report: (kind: 'busy' | 'unavailable' | 'error', error: unknown) => void,
) {
  return computed(() => {
    const lease = binding.captureUi()
    if (!lease?.isCurrent()) return null
    return { remove: async (id: string) => {
      const find = () => lease.context.store.messages.find(m => m.id === id && m.conversation_id === lease.conversationId)
      const message = find()
      if (!lease.isCurrent() || !message) return
      const content = message.content, controller = new AbortController(), scope = effectScope(true)
      const current = () => lease.isCurrent() && !!find() && find()!.content === content
      const valid = () => !controller.signal.aborted && current()
      scope.run(() => watch(current, value => { if (!value) controller.abort() }, { flush: 'sync' }))
      try { await abortable(lease.context.store.deleteMessage(id, valid), controller.signal) }
      catch (error) {
        if (valid()) report(error instanceof MessageDeletionBusyError ? 'busy'
          : error instanceof MessageDeletionUnavailableError ? 'unavailable' : 'error', error)
      } finally { scope.stop(); controller.abort() }
    } }
  })
}
