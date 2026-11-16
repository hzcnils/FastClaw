// @vitest-environment jsdom
import { createPinia, setActivePinia } from 'pinia'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import type { Message } from '@/types'
import { disposeChatStore, useChatStore } from './chat'

const instanceId = 'planning-reset-test'

function ordinaryMessage(): Message {
  return {
    id: 'ordinary-message',
    conversation_id: 'ordinary-conversation',
    role: 'assistant',
    content: 'ordinary answer',
    model_id: 'model-1',
    token_input: null,
    token_output: null,
    status: 'completed',
    created_at: '2026-09-01T00:00:00Z',
  }
}

describe('chat planning conversation reset', () => {
  beforeEach(() => {
    sessionStorage.clear()
    setActivePinia(createPinia())
  })

  afterEach(async () => {
    await disposeChatStore(instanceId)
  })

  it('detaches the selected ordinary conversation without mutating its message object', () => {
    const chat = useChatStore(instanceId)
    const message = ordinaryMessage()
    chat.currentConversationId = message.conversation_id
    chat.messages = [message]

    expect(chat.detachCurrentConversation('another-conversation')).toBe(false)
    expect(chat.currentConversationId).toBe(message.conversation_id)

    expect(chat.detachCurrentConversation(message.conversation_id)).toBe(true)
    expect(chat.currentConversationId).toBeNull()
    expect(chat.messages).toEqual([])
    expect(message).toMatchObject({ conversation_id: 'ordinary-conversation', content: 'ordinary answer' })
  })
})
