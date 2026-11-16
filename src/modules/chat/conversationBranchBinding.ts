import { computed, effectScope, watch } from 'vue'
import type { createChatContextBinding } from './contextBinding'
import type { MessageActionStore } from './messageActions'
import { abortable } from './abortable'
import { BranchUnavailableError } from './conversationBranch'

export interface ConversationBranchStore extends MessageActionStore {
  branchConversation(id: string, canStart?: () => boolean): Promise<string | null>
}

export function createConversationBranchBinding<S extends ConversationBranchStore>(
  binding: ReturnType<typeof createChatContextBinding<S>>,
  dependencies: { open(id: string): Promise<unknown>; report(kind: 'created' | 'unavailable' | 'error', error?: unknown): void },
) {
  return computed(() => {
    const lease = binding.captureUi()
    if (!lease?.isCurrent()) return null
    return { branch: async (messageId: string) => {
      const store = lease.context.store
      const find = () => store.messages.find(message => message.id === messageId && message.conversation_id === lease.conversationId)
      const source = find()
      if (!lease.isCurrent() || !source || source.status === 'streaming') return
      const content = source.content, controller = new AbortController(), scope = effectScope(true)
      const current = () => lease.isCurrent() && !!find() && find()!.content === content && find()!.status !== 'streaming'
      const valid = () => !controller.signal.aborted && current()
      scope.run(() => watch(current, value => { if (!value) controller.abort() }, { flush: 'sync' }))
      try {
        const id = await abortable(store.branchConversation(messageId, valid), controller.signal)
        if (!id || !valid()) return
        dependencies.report('created')
        // New Tab uses the standard registry/hydration flow, preserving the source draft and execution.
        if (valid()) await dependencies.open(id)
      } catch (error) {
        if (valid()) dependencies.report(error instanceof BranchUnavailableError ? 'unavailable' : 'error', error)
      } finally { scope.stop(); controller.abort() }
    } }
  })
}
