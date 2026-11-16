import type { FastPlanTimelineEntryData } from 'planflow-kit'
import type { FastPlanSessionBinding } from './sessionManager'
import { findChatTabState, type ChatTabState } from '@/modules/chat/tabState'
import { getChatTabConversation } from '@/utils/chatTabRegistry'

/** A transport-accepted send is an event, not a command to replay on the next mount. */
export function acknowledgeFastPlanSend(binding: FastPlanSessionBinding, sessionId: string, requestId: string) {
  if (!requestId || getChatTabConversation(binding.tabPath) !== binding.conversationId) return
  const instanceId = binding.tabPath === '/chat' ? 'default' : binding.tabPath.slice('/chat/'.length)
  const owner = findChatTabState(instanceId)
  if (!owner || owner.disposed) return
  owner.state.fastPlanEntries.value = owner.state.fastPlanEntries.value.map(entry => {
    const data = entry.data as FastPlanTimelineEntryData
    if (entry.id !== binding.entryId || data.sessionId !== sessionId
      || data.command?.type !== 'send' || data.command.id !== requestId) return entry
    return { ...entry, data: { ...data, command: undefined, pendingPrompt: undefined, pendingAttachments: undefined } }
  })
}

/** Read the retained owner before creating the renderer, including on KeepAlive eviction. */
export function retainedComposerEntry(owner: ChatTabState): string | null {
  const { dialogueCharacter, activeFastPlanEntryId, fastPlanEntries } = owner.state
  const id = activeFastPlanEntryId.value
  return dialogueCharacter.value.taskPlan && id && fastPlanEntries.value.some(entry => entry.id === id) ? id : null
}
