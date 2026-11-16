import { computed } from 'vue'
import type { Message } from '@/types'
import type { ChatContext, createChatContextBinding } from './contextBinding'

interface QuestionStore { currentConversationId: string | null; messages: Message[] }

/** Inline editing creates a new turn, never updates the historical user message. */
export function createInlineQuestionBinding<S extends QuestionStore>(
  binding: ReturnType<typeof createChatContextBinding<S>>,
  busy: (context: ChatContext<S>) => boolean,
  resend: (content: string, isCurrent: () => boolean, onDispatch: () => void) => Promise<void>,
) {
  let sequence = 0
  const identities = new WeakMap<object, number>()
  function open(id: string) {
    const lease = binding.captureUi()
    if (!lease?.isCurrent() || busy(lease.context)) return false
    const { owner, store } = lease.context
    const message = store.messages.find(item => item.id === id && item.role === 'user'
      && item.conversation_id === store.currentConversationId)
    if (!message || !store.currentConversationId) return false
    const state = owner.state
    if (state.inlineUserEditId.value === id && state.inlineUserEditOrigin.value?.conversationId === store.currentConversationId) return true
    state.inlineUserEditOrigin.value = { conversationId: store.currentConversationId, content: message.content }
    state.inlineUserEditId.value = id
    state.inlineUserEditContent.value = message.content
    return true
  }
  const session = computed(() => {
    const frame = binding.active.value, lease = binding.captureUi()
    if (!frame || !lease?.isCurrent()) return null
    const { owner, store } = lease.context, state = owner.state
    const id = state.inlineUserEditId.value, origin = state.inlineUserEditOrigin.value
    if (!id || !origin || origin.conversationId !== store.currentConversationId) return null
    if (!identities.has(origin)) identities.set(origin, ++sequence)
    const current = () => lease.isCurrent() && state.inlineUserEditId.value === id && state.inlineUserEditOrigin.value === origin
    const sourceCurrent = () => store.messages.some(message => message.id === id && message.role === 'user'
      && message.conversation_id === origin.conversationId && message.content === origin.content)
    const cancel = () => {
      if (!current()) return
      state.inlineUserEditId.value = null
      state.inlineUserEditContent.value = ''
      state.inlineUserEditOrigin.value = null
    }
    let pending = false
    async function submit(event?: KeyboardEvent | MouseEvent) {
      if (event instanceof KeyboardEvent && event.isComposing) return
      if (pending || !current() || !sourceCurrent() || busy(lease!.context)) return
      const draft = state.inlineUserEditContent.value, content = draft.trim()
      if (!content) return
      pending = true
      try {
        await resend(content, () => current() && sourceCurrent() && state.inlineUserEditContent.value === draft, cancel)
      } finally { pending = false }
    }
    return { id, key: `${frame.generation}:${identities.get(origin)}`, state,
      events: {
        input: (event: Event) => {
          if (current()) state.inlineUserEditContent.value = (event.target as HTMLTextAreaElement).value
        },
        cancel,
        submit,
        keydown(event: KeyboardEvent) {
          if (event.isComposing) return
          if (event.key === 'Escape') { event.preventDefault(); cancel() }
          else if (event.key === 'Enter' && !event.shiftKey && !event.ctrlKey && !event.altKey && !event.metaKey) {
            event.preventDefault(); void submit(event)
          }
        },
      },
    }
  })
  const panel = computed(() => {
    const active = session.value
    return active ? { id: active.id, key: active.key, content: active.state.inlineUserEditContent.value, events: active.events } : null
  })
  return { open, panel }
}
