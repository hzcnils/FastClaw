import { computed } from 'vue'
import type { Message } from '@/types'
import { captureChatCommand, type createChatContextBinding } from './contextBinding'

export interface MessageEditStore {
  currentConversationId: string | null
  messages: Message[]
  isLoading: boolean
  updateMessageContent(id: string, content: string, isCurrent?: () => boolean): Promise<boolean>
}

/** Editor UI expires on presentation changes; submitted writes retain their original business target. */
export function createMessageEditorBinding<S extends MessageEditStore>(
  binding: ReturnType<typeof createChatContextBinding<S>>,
  reportError: (error: unknown) => void,
) {
  let sequence = 0
  const identities = new WeakMap<object, number>()
  function open(id: string) {
    const lease = binding.captureUi()
    if (!lease?.isCurrent()) return false
    const { owner, store } = lease.context
    if (store.isLoading || owner.state.messageEditSaveId.value) return false
    const source = store.messages.find(message => message.id === id && message.conversation_id === store.currentConversationId)
    if (!source || source.role === 'user') return false
    if (owner.state.editingMessage.value?.id !== id) {
      owner.state.editingMessage.value = { id, content: source.content }
    }
    owner.state.editDialogVisible.value = true
    return true
  }

  // Do not depend on draft/loading here: events and instance key remain stable while typing/saving.
  const session = computed(() => {
    const frame = binding.active.value
    const lease = binding.captureUi()
    if (!frame || !lease?.isCurrent()) return null
    const { owner, store } = lease.context, state = owner.state
    const target = state.editingMessage.value
    if (!state.editDialogVisible.value || !target) return null
    if (!identities.has(target)) identities.set(target, ++sequence)
    const currentUi = () => lease.isCurrent() && state.editDialogVisible.value && state.editingMessage.value === target
    const close = () => {
      if (!currentUi()) return
      state.editDialogVisible.value = false
      state.editingMessage.value = null
    }
    async function save(content: string) {
      if (!currentUi() || store.isLoading || state.messageEditSaveId.value) return
      const command = captureChatCommand(lease!.context)
      const original = target!.content
      const valid = () => command.isCurrent() && !!command.conversationId && store.messages.some(message =>
        message.id === target!.id && message.conversation_id === command.conversationId && message.content === original)
      if (!valid()) return
      const requestId = crypto.randomUUID()
      state.messageEditSaveId.value = requestId
      try {
        const saved = await store.updateMessageContent(target!.id, content, valid)
        if (!saved || !command.isCurrent() || state.editingMessage.value !== target) return
        const unchangedDraft = (target!.draftContent ?? target!.content) === content
        target!.content = content
        if (unchangedDraft) {
          state.editDialogVisible.value = false
          state.editingMessage.value = null
        }
      } catch (error) {
        if (currentUi()) reportError(error)
      } finally {
        // Cleanup belongs to the original owner, even if its UI was evicted or another owner is active.
        if (state.messageEditSaveId.value === requestId) state.messageEditSaveId.value = null
      }
    }
    return {
      key: `${frame.generation}:${identities.get(target)}`,
      target, state,
      events: {
        cancel: close,
        'update:modelValue': (visible: boolean) => { if (!visible) close() },
        'draft-change': (content: string) => { if (currentUi()) target.draftContent = content },
        save,
      },
    }
  })
  const panel = computed(() => {
    const active = session.value
    return active ? { key: active.key, content: active.target.content, draftContent: active.target.draftContent,
      loading: !!active.state.messageEditSaveId.value, events: active.events } : null
  })
  return { open, panel }
}
