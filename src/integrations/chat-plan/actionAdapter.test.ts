import { describe, expect, it, vi } from 'vitest'
import type { ChatMessage, ChatWorkspaceActionEvent, MessageActionContext, ResolvedMessageAction } from 'chat-renderer'
import {
  createResolveFastClawMessageActions,
  resolveFastClawMessageActions,
  routeFastClawAction,
} from './actionAdapter'

const context: MessageActionContext = {
  index: 0, isLastMessage: true, isLastCompletedAssistantTurn: true, isGenerating: false, hasStableId: true,
}
const assistant: ChatMessage = {
  id: 'a1', role: 'assistant', createdAt: 0, status: 'complete', turnId: 'g1', parts: [{ type: 'text', text: 'ok' }],
}

describe('action adapter', () => {
  it('keeps host action ordering and conditional actions', () => {
    expect(resolveFastClawMessageActions(assistant, context)).toEqual([
      'copy', 'regenerate', 'edit', 'translate', 'favorite', 'edit-as-doc', 'switch-model', 'quote',
      'tts', 'export', 'like', 'dislike', 'branch', 'delete',
    ])
  })

  it('limits actions while streaming', () => {
    expect(resolveFastClawMessageActions({ ...assistant, status: 'streaming' }, context)).toEqual(['copy'])
  })

  it('hides all standard actions for error messages', () => {
    expect(resolveFastClawMessageActions({ ...assistant, status: 'error' }, context)).toEqual([])
  })

  it('marks translate as loading while metadata.translating is true', () => {
    const resolve = createResolveFastClawMessageActions({
      translatingLabel: () => '翻译中...',
    })
    const actions = resolve(
      { ...assistant, metadata: { translating: true } },
      context,
    ) as ResolvedMessageAction[]
    const translate = actions.find((action) => action.name === 'translate')
    expect(translate).toMatchObject({
      loading: true,
      disabled: true,
      label: '翻译中...',
      tooltip: '翻译中...',
    })
    expect(actions.some((action) => action.name === 'copy' && !action.loading)).toBe(true)
  })

  it('routes supported actions and reports unsupported actions', async () => {
    const copy = vi.fn()
    const event = { action: 'copy', scope: 'message', messageId: 'a1' } as ChatWorkspaceActionEvent
    expect(await routeFastClawAction(event, { copy })).toBe(true)
    expect(copy).toHaveBeenCalledWith(event)
    expect(await routeFastClawAction({ ...event, action: 'delete' }, { copy })).toBe(false)
  })
})
