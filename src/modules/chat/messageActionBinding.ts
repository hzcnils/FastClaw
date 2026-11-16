import { computed } from 'vue'
import type { createChatContextBinding } from './contextBinding'
import { messageActionService, type MessageActionOptions, type MessageActionStore } from './messageActions'

export function createMessageActionBinding<S extends MessageActionStore>(
  binding: ReturnType<typeof createChatContextBinding<S>>,
  dependencies: Pick<MessageActionOptions, 'getInvoke' | 'report'>,
) {
  return computed(() => {
    const lease = binding.captureUi()
    if (!lease?.isCurrent()) return null
    const options = { ...dependencies, canStart: lease.isCurrent, isVisible: lease.isCurrent }
    const target = lease.context
    return {
      favorite: (id: string, next: boolean) => messageActionService.favorite(target, id, next, options),
      like: (id: string, next: number) => messageActionService.feedback(target, id, next, options),
      dislike: (id: string, next: number) => messageActionService.feedback(target, id, next, options),
      translate: (id: string, language: string) => messageActionService.translate(target, id, language,
        target.owner.state.modelId.value || undefined, options),
    }
  })
}
