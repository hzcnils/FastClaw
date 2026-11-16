import { effectScope, watch } from 'vue'
import { abortable } from './abortable'
import { captureChatCommand, type ChatContextStore, type createChatContextBinding } from './contextBinding'

/** UI intent before handoff; accepted services deliberately do not depend on this lease. */
export function captureSendPreparation<S extends ChatContextStore>(
  lease: NonNullable<ReturnType<ReturnType<typeof createChatContextBinding<S>>['captureUi']>>,
  extra: () => unknown,
) {
  const command = captureChatCommand(lease.context), state = lease.context.owner.state
  const scope = effectScope(true), controller = new AbortController()
  const draft = state.messageStr.value, quote = state.quotedText.value
  let ownMutation = false, moved = false, edited = false, committed = false, disposed = false
  const snapshot = () => JSON.stringify([
    state.messageStr.value, state.quotedText.value, state.fileList.value.map(file => file.id),
    state.modelId.value, state.dialogueCharacter.value, state.editorVisible.value,
    state.knowledgeId.value, state.selectedSkill.value?.name, state.selectedAgentId.value, extra(),
  ])
  let expected = snapshot()
  const isCurrent = () => !disposed && !controller.signal.aborted && lease.isCurrent() && snapshot() === expected
  scope.run(() => {
    watch(() => [lease.isCurrent(), snapshot()], () => {
      if (!ownMutation && !isCurrent()) controller.abort()
    }, { flush: 'sync' })
    watch([state.messageStr, state.quotedText], () => { if (!ownMutation) edited = true }, { flush: 'sync' })
  })
  function mutate(action: () => void) {
    if (!isCurrent()) return false
    ownMutation = true
    try { action(); expected = snapshot(); return true }
    finally { ownMutation = false }
  }
  return {
    isCurrent, mutate, signal: controller.signal,
    moveDraft() { return mutate(() => { moved = true; state.messageStr.value = ''; state.quotedText.value = '' }) },
    wait<T>(operation: Promise<T>) { return abortable(operation, controller.signal) },
    commit() { committed = true; scope.stop() },
    dispose() {
      if (disposed) return
      disposed = true; scope.stop(); controller.abort()
      // Recover the original owner's temporary empty draft even after UI deactivation.
      if (!committed && moved && !edited && command.isCurrent()
        && !state.messageStr.value && !state.quotedText.value) {
        state.messageStr.value = draft; state.quotedText.value = quote
      }
    },
  }
}
export type SendPreparation = ReturnType<typeof captureSendPreparation>
