import { computed, effectScope, ref, watch, type InjectionKey } from 'vue'
import type { Message, WebHit } from '@/types'
import type { createChatContextBinding } from './contextBinding'
import type { MessageActionOptions, MessageActionStore } from './messageActions'
import { abortable } from './abortable'

export interface SourceLinkActions {
  captureLink(messageId: string, url: string, isDisplayed: () => boolean): () => Promise<void>
}
export const CHAT_SOURCE_LINKS: InjectionKey<SourceLinkActions> = Symbol('chat-source-links')

interface Dependencies {
  traceActions(): { name: string; toggle(message: Message): void }[]
  schedule(callback: () => void): void
  scroll(): void
  getInvoke: MessageActionOptions['getInvoke']
  fallback(url: string): void
  linkFailed(): void
}
interface WebSourcePanel { messageId: string; hits: WebHit[]; query?: string; close(): void }

/** Source selections are Tab state; delayed effects and link fallbacks belong to one presentation. */
export function createSourceInteractionBinding<S extends MessageActionStore>(
  binding: ReturnType<typeof createChatContextBinding<S>>, dependencies: Dependencies,
) {
  const selectionVersion = ref(0)
  let traceVersion = 0
  const actions = computed(() => {
    const lease = binding.captureUi()
    if (!lease?.isCurrent()) return null
    const { store, owner } = lease.context
    const find = (id: string) => store.messages.find(message => message.id === id && message.conversation_id === lease.conversationId)
    return {
      trace(type: string, id: string) {
        if (!lease.isCurrent()) return
        const message = find(id), action = dependencies.traceActions().find(item => item.name === type)
        if (!message || !action || !message.trace_source) return
        const version = ++traceVersion
        owner.state.currentTraceMessageId.value = id
        action.toggle(message)
        dependencies.schedule(() => {
          if (lease.isCurrent() && traceVersion === version && find(id) === message && owner.state.currentTraceMessageId.value === id) dependencies.scroll()
        })
      },
      webSearch(id: string) {
        if (!lease.isCurrent() || !find(id)) return
        selectionVersion.value++
        owner.state.currentWebSearchMessageId.value = id
        owner.state.rightPanel.value = 'websearch'
      },
    }
  })
  const webPanel = computed<WebSourcePanel | null>(() => {
    const lease = binding.captureUi()
    if (!lease?.isCurrent()) return null
    const { owner, store } = lease.context, state = owner.state
    if (state.rightPanel.value !== 'websearch') return null
    const message = store.messages.find(item => item.id === state.currentWebSearchMessageId.value && item.conversation_id === lease.conversationId)
    if (!message) return null
    const version = selectionVersion.value
    const panel = { messageId: message.id, hits: message.web_hits ?? [], query: message.web_search_query,
      close: () => {
        if (lease.isCurrent() && selectionVersion.value === version && webPanel.value === panel) {
          selectionVersion.value++; state.rightPanel.value = 'none'
        }
      },
    }
    return panel
  })
  const pending = new Map<string, { current(): boolean }>()
  function captureLink(id: string, url: string, isDisplayed: () => boolean) {
    const lease = binding.captureUi()
    const source = lease?.context.store.messages.find(message => message.id === id && message.conversation_id === lease.conversationId)
    const current = () => !!lease?.isCurrent() && isDisplayed() && !!source
      && lease.context.store.messages.includes(source) && source.conversation_id === lease.conversationId
      && !!source.web_hits?.some(hit => hit.url === url)
    return async () => {
      if (!current()) return
      try { if (!['http:', 'https:'].includes(new URL(url).protocol)) return } catch { return }
      const key = JSON.stringify([lease!.context.owner.instanceId, lease!.conversationId, id, url])
      if (pending.get(key)?.current()) return
      const controller = new AbortController(), scope = effectScope(true)
      const valid = () => !controller.signal.aborted && current()
      const request = { current: valid }
      pending.set(key, request)
      scope.run(() => watch(current, value => { if (!value) controller.abort() }, { flush: 'sync' }))
      try {
        const invoke = await abortable(dependencies.getInvoke(), controller.signal)
        if (!valid()) return
        if (invoke) {
          try { await abortable(invoke('open_external', { url }), controller.signal); return }
          catch { if (!valid()) return }
        }
        if (valid()) dependencies.fallback(url)
      } catch {
        if (valid()) dependencies.linkFailed()
      } finally {
        scope.stop(); controller.abort()
        if (pending.get(key) === request) pending.delete(key)
      }
    }
  }
  return { actions, webPanel, captureLink }
}
