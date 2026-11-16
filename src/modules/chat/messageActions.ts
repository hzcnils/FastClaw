import type { Message } from '@/types'
import type { ChatTabState } from './tabState'

// Enumerable symbol follows immutable message copies but is never persisted by JSON.
const translationToken = Symbol('chat-translation-request')
type TranslatingMessage = Message & { [translationToken]?: symbol }

export interface MessageActionStore { currentConversationId: string | null; messages: Message[] }
export interface MessageActionTarget { owner: ChatTabState; store: MessageActionStore }
export interface MessageActionOptions {
  getInvoke(): Promise<((command: string, args?: Record<string, unknown>) => Promise<unknown>) | null>
  canStart(): boolean
  isVisible(): boolean
  report(kind: 'favorite' | 'feedback' | 'translate' | 'unavailable', error?: unknown): void
}

/** Application-owned operations. Serialize each message field, including across view remounts. */
export class MessageActionService {
  private pending = new WeakMap<ChatTabState, Map<string, { intent: readonly unknown[]; current(): boolean; task: Promise<unknown> }>>()

  private enqueue<T>(target: MessageActionTarget, id: string, field: 'favorite' | 'feedback' | 'translate', intent: readonly unknown[],
    options: MessageActionOptions, run: (find: () => number) => Promise<T>, empty: T): Promise<T> {
    const { owner, store } = target, conversationId = store.currentConversationId, ownerCurrent = owner.capture()
    const find = () => ownerCurrent() && store.currentConversationId === conversationId
      ? store.messages.findIndex(m => m.id === id && m.conversation_id === conversationId) : -1
    if (!conversationId || find() < 0 || !options.canStart()) return Promise.resolve(empty)
    let queue = this.pending.get(owner)
    if (!queue) { queue = new Map(); this.pending.set(owner, queue) }
    const key = JSON.stringify([conversationId, id, field]), previous = queue.get(key)
    if (previous?.intent.length === intent.length && previous.intent.every((value, i) => Object.is(value, intent[i]))
      && previous.current()) return previous.task as Promise<T>
    const entry: { intent: readonly unknown[]; current(): boolean; task: Promise<T> } = {
      intent, current: () => find() >= 0 && options.canStart(), task: Promise.resolve(empty),
    }
    entry.task = (previous?.task ?? Promise.resolve()).then(async () => {
      if (find() < 0 || !options.canStart()) return empty
      return run(find)
    }).catch(error => {
      if (find() >= 0 && options.isVisible()) options.report(field, error)
      return empty
    }).finally(() => {
      if (queue!.get(key) === entry) queue!.delete(key)
      if (!queue!.size && this.pending.get(owner) === queue) this.pending.delete(owner)
    })
    queue.set(key, entry)
    return entry.task
  }

  favorite(target: MessageActionTarget, id: string, value: boolean, options: MessageActionOptions) {
    return this.mutate(target, id, 'favorite', value ? 1 : 0, options)
  }
  feedback(target: MessageActionTarget, id: string, value: number, options: MessageActionOptions) {
    if (![0, 1, -1].includes(value)) return Promise.resolve(false)
    return this.mutate(target, id, 'feedback', value, options)
  }
  private mutate(target: MessageActionTarget, id: string, field: 'favorite' | 'feedback', value: number, options: MessageActionOptions) {
    return this.enqueue(target, id, field, [value], options, async find => {
      const invoke = await options.getInvoke()
      let index = find()
      if (!invoke || index < 0 || !options.canStart()) return false
      const messages = target.store.messages, property = field === 'favorite' ? 'is_favorite' : 'is_liked'
      const before = messages[index][property] ?? 0
      // Keep existing favorite optimism; feedback is published only after persistence succeeds.
      if (field === 'favorite') messages[index] = { ...messages[index], [property]: value }
      try {
        await invoke(field === 'favorite' ? 'favorite_message' : 'like_message', field === 'favorite'
          ? { messageId: id, isFavorite: value === 1 } : { messageId: id, isLiked: value })
        index = find()
        if (index >= 0 && field === 'feedback') target.store.messages[index] = { ...target.store.messages[index], [property]: value }
        return true
      } catch (error) {
        index = find()
        if (index >= 0 && field === 'favorite' && target.store.messages[index][property] === value) {
          target.store.messages[index] = { ...target.store.messages[index], [property]: before }
        }
        throw error
      }
    }, false)
  }

  translate(target: MessageActionTarget, id: string, language: string, modelId: string | undefined, options: MessageActionOptions) {
    const conversationId = target.store.currentConversationId
    const source = target.store.messages.find(m => m.id === id && m.conversation_id === target.store.currentConversationId)
    const content = source?.content
    return this.enqueue<string | null>(target, id, 'translate', [language, modelId, content], options, async find => {
      const invoke = await options.getInvoke()
      let index = find()
      if (index < 0 || !options.canStart() || target.store.messages[index].content !== content) return null
      if (!invoke) { if (options.isVisible()) options.report('unavailable'); return null }
      const token = Symbol(), originalMessages = target.store.messages
      originalMessages[index] = { ...originalMessages[index], translating: true, [translationToken]: token } as TranslatingMessage
      const clearLoading = () => {
        for (const list of new Set([originalMessages, target.store.messages])) {
          const i = list.findIndex(message => message.id === id && (message as TranslatingMessage)[translationToken] === token)
          if (i < 0) continue
          const next: TranslatingMessage = { ...list[i], translating: false }
          delete next[translationToken]
          list[i] = next
        }
      }
      const removeInvalidation = target.owner.onInvalidate(clearLoading)
      try {
        const text = await invoke('translate_message', { messageId: id, targetLang: language, modelId: modelId ?? null,
          expectedContent: content, expectedConversationId: conversationId })
        index = find()
        if (index < 0 || target.store.messages[index].content !== content) return null
        if (typeof text !== 'string') throw new Error('Invalid translation response')
        target.store.messages[index] = { ...target.store.messages[index], translation: text, translation_lang: language }
        return text
      } finally {
        removeInvalidation()
        clearLoading()
      }
    }, null)
  }
}

export const messageActionService = new MessageActionService()
