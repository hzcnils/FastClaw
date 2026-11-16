import { computed, effectScope, shallowReadonly, shallowRef, watch, type Ref } from 'vue'
import type { ChatTabState } from './tabState'
import type { ChatViewIdentity } from './viewIdentity'

export interface ChatContextStore { currentConversationId: string | null }
export interface ChatContext<S extends ChatContextStore> {
  readonly identity: ChatViewIdentity
  readonly owner: ChatTabState
  readonly store: S
}

export function createChatContext<S extends ChatContextStore>(identity: ChatViewIdentity, owner: ChatTabState, store: S): ChatContext<S> {
  if (owner.disposed || owner.instanceId !== identity.instanceId) throw new Error('Invalid Chat context owner')
  // Freeze only the identity/container; retain the original Store, refs, File and runtime objects.
  return Object.freeze({ identity: Object.freeze({ ...identity }), owner, store })
}

/** Captured command target survives presentation changes, but not owner/conversation invalidation. */
export function captureChatCommand<S extends ChatContextStore>(context: ChatContext<S>) {
  const conversationId = context.store.currentConversationId
  const ownerCurrent = context.owner.capture()
  return Object.freeze({ context, conversationId,
    isCurrent: () => ownerCurrent() && context.store.currentConversationId === conversationId,
  })
}

/** Presentation binding only. Never creates/disposes Stores, releases Tabs or stops execution. */
export function createChatContextBinding<S extends ChatContextStore>(isVisible: (context: ChatContext<S>) => boolean = () => true) {
  let generation = 0
  let disposed = false
  let releaseOwner: (() => void) | undefined
  let stopConversation: (() => void) | undefined
  const active = shallowRef<Readonly<{ context: ChatContext<S>; generation: number }> | null>(null)

  function detachSubscriptions() {
    releaseOwner?.(); releaseOwner = undefined
    stopConversation?.(); stopConversation = undefined
  }
  function deactivate() {
    if (!active.value && !releaseOwner && !stopConversation) return
    detachSubscriptions()
    generation++
    active.value = null
  }
  function activate(context: ChatContext<S>) {
    if (disposed || context.owner.disposed) return false
    if (active.value?.context === context) return true
    detachSubscriptions()
    const publish = () => {
      if (disposed || context.owner.disposed) { deactivate(); return }
      active.value = Object.freeze({ context, generation: ++generation })
    }
    releaseOwner = context.owner.onInvalidate(publish)
    const scope = effectScope(true)
    scope.run(() => watch(() => context.store.currentConversationId, publish, { flush: 'sync' }))
    stopConversation = () => scope.stop()
    publish()
    return true
  }
  function captureUi() {
    const frame = active.value
    if (!frame) return null
    const command = captureChatCommand(frame.context)
    const isCurrent = () => !disposed && active.value === frame && isVisible(frame.context) && command.isCurrent()
    return {
      ...command, isCurrent,
      guard<A extends unknown[], R>(callback: (...args: A) => R) {
        return (...args: A): R | undefined => isCurrent() ? callback(...args) : undefined
      },
      model<T>(source: Ref<T>) {
        return computed({ get: () => source.value, set: (value: T) => { if (isCurrent()) source.value = value } })
      },
    }
  }
  function dispose() { if (disposed) return; disposed = true; deactivate() }
  return { active: shallowReadonly(active), activate, deactivate, captureUi, dispose }
}
