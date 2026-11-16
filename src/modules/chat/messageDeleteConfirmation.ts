import { effectScope, onBeforeUnmount, onDeactivated, watch } from 'vue'
import type { Message } from '@/types'

/** The popconfirm confirms the message that opened it, never the latest recycled prop. */
export function useMessageDeleteConfirmation(message: () => Message, remove: (id: string) => void) {
  let confirmCurrent = () => {}, stop: (() => void) | undefined
  const cancel = () => { confirmCurrent = () => {}; stop?.(); stop = undefined }
  onBeforeUnmount(cancel); onDeactivated(cancel)
  return {
    cancel,
    open() {
      cancel()
      const source = message(), id = source.id, content = source.content, conversationId = source.conversation_id
      const current = () => message() === source && source.id === id && source.content === content && source.conversation_id === conversationId
      const scope = effectScope(true); stop = () => scope.stop()
      scope.run(() => watch(current, valid => { if (!valid) cancel() }, { flush: 'sync' }))
      confirmCurrent = () => { if (current()) remove(id) }
    },
    confirm() { const run = confirmCurrent; cancel(); run() },
  }
}
