import { describe, expect, it } from 'vitest'
import { CHAT_WORKSPACE_PROTOCOL_VERSION } from './types'
import type {
  ChatTimelineEntry,
  ChatWorkspaceActionEvent,
  ChatWorkspaceAdapter,
} from './types'

describe('ChatWorkspace protocol', () => {
  it('publishes an independent version for the mixed workspace contract', () => {
    expect(CHAT_WORKSPACE_PROTOCOL_VERSION).toBe(1)
  })

  it('represents messages, answer alternatives, and embedded renderers without host fields', () => {
    const user = { id: 'u-1', role: 'user' as const, createdAt: 1, parts: [{ type: 'text' as const, text: 'question' }] }
    const assistant = { id: 'a-1', role: 'assistant' as const, createdAt: 2, parts: [{ type: 'text' as const, text: 'answer' }] }
    const entries: ChatTimelineEntry[] = [
      { kind: 'message', id: 'm-1', message: user },
      { kind: 'turn', id: 'turn-1', user, alternatives: [assistant], activeId: 'a-1', comparisonMode: 'tabs' },
      { kind: 'embedded', id: 'fastplan-1', renderer: 'fastplan', data: { sessionId: 'fp-1' }, status: 'active' },
    ]

    expect(entries.map(entry => entry.kind)).toEqual(['message', 'turn', 'embedded'])
    expect(entries[2]).toMatchObject({ renderer: 'fastplan', data: { sessionId: 'fp-1' } })
  })

  it('streams semantic events and keeps host actions behind the adapter boundary', async () => {
    const actions: ChatWorkspaceActionEvent[] = []
    const adapter: ChatWorkspaceAdapter = {
      async *send(input) {
        yield { type: 'text-delta', delta: input.text }
        yield { type: 'message-complete' }
      },
      executeAction(event) {
        actions.push(event)
        return { status: 'success' }
      },
    }

    const events = []
    for await (const event of adapter.send({ text: 'hello' }, {})) events.push(event)
    await adapter.executeAction?.({ action: 'trace-source', scope: 'message', messageId: 'a-1' })

    expect(events).toEqual([
      { type: 'text-delta', delta: 'hello' },
      { type: 'message-complete' },
    ])
    expect(actions[0]).toMatchObject({ action: 'trace-source', scope: 'message', messageId: 'a-1' })
  })
})
