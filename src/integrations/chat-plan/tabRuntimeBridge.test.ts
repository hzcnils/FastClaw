// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { getChatTabState, releaseChatTabState, findChatTabState } from '@/modules/chat/tabState'
import { setChatTabConversation, unregisterChatTab } from '@/utils/chatTabRegistry'
import { acknowledgeFastPlanSend, retainedComposerEntry } from './tabRuntimeBridge'

const binding = { conversationId: 'a', tabPath: '/chat/a', entryId: 'entry-a' }
function setup() {
  const owner = getChatTabState('a')
  owner.state.dialogueCharacter.value.taskPlan = true
  setChatTabConversation('/chat/a', 'a')
  owner.state.activeFastPlanEntryId.value = 'entry-a'
  owner.state.fastPlanEntries.value = [{ kind: 'embedded', renderer: 'fastplan', id: 'entry-a', status: 'active',
    data: { sessionId: 'session-a', pendingPrompt: '问题', pendingAttachments: [{ id: 'file' }],
      command: { id: 'request-a', type: 'send', prompt: '问题' } } }]
  return owner
}
afterEach(() => { releaseChatTabState('a'); unregisterChatTab('/chat/a') })

describe('retained planning entry bridge', () => {
  it('clears only the transport-acknowledged command and preserves confirmation ownership', () => {
    const owner = setup()
    acknowledgeFastPlanSend(binding, 'session-a', 'request-a')
    expect(owner.state.fastPlanEntries.value[0]).toMatchObject({ status: 'active', data: {
      sessionId: 'session-a', command: undefined, pendingPrompt: undefined, pendingAttachments: undefined,
    } })
    expect(retainedComposerEntry(getChatTabState('a'))).toBe('entry-a')
    owner.state.dialogueCharacter.value.taskPlan = false
    expect(retainedComposerEntry(owner)).toBeNull()
  })

  it('does not clear a newer send, another session, or a replacement conversation', () => {
    const owner = setup()
    const data = () => owner.state.fastPlanEntries.value[0].data as any
    acknowledgeFastPlanSend(binding, 'session-a', 'old-request')
    acknowledgeFastPlanSend(binding, 'other-session', 'request-a')
    acknowledgeFastPlanSend({ ...binding, entryId: 'other-entry' }, 'session-a', 'request-a')
    setChatTabConversation('/chat/a', 'b')
    acknowledgeFastPlanSend(binding, 'session-a', 'request-a')
    expect(data().command.id).toBe('request-a')
  })

  it('does not recreate a closed tab for a delayed acknowledgement', () => {
    setup(); releaseChatTabState('a')
    acknowledgeFastPlanSend(binding, 'session-a', 'request-a')
    expect(findChatTabState('a')).toBeUndefined()
  })
})
