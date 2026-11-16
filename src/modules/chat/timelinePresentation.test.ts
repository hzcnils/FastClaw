import { ref, shallowRef } from 'vue'
import { describe, expect, it } from 'vitest'
import type { ChatEmbeddedTimelineEntry } from 'chat-renderer'
import type { Message } from '@/types'
import { useChatTimelinePresentation, type ProviderTimelinePanel } from './useChatTimelinePresentation'

const message = (id: string, role: Message['role'], group_id?: string, extra: Partial<Message> = {}): Message => ({
  id, role, group_id, content: id, conversation_id: 'c1', model_id: 'model', token_input: null, token_output: null,
  status: 'completed', created_at: '2026-09-23T00:00:00Z', ...extra,
})
function setup() {
  const messages = ref<Message[]>([]), plans = shallowRef<ChatEmbeddedTimelineEntry[]>([]), enabled = ref(false)
  const panel = ref<ProviderTimelinePanel | null>(null), revision = ref(0)
  const presentation = useChatTimelinePresentation({
    messages: () => messages.value, fastPlanEntries: () => { void revision.value; return plans.value },
    taskPlanEnabled: () => enabled.value, providerPanel: () => panel.value,
    modelName: () => 'Model name', contextDividerLabel: () => '上下文已清除',
  })
  return { messages, plans, enabled, panel, revision, ...presentation }
}
describe('ChatView timeline presentation', () => {
  it('keeps ordinary/planning/provider order and hides planning without deleting it', () => {
    const s = setup()
    s.messages.value = [message('u', 'user', 'g'), message('a', 'assistant', 'g')]
    s.plans.value = [{ kind: 'embedded', id: 'plan', renderer: 'fastplan', data: { phase: 'waiting' } }]
    s.panel.value = { id: 'config', createdAt: 1, modelId: 'model',
      userMessage: { id: 'question', role: 'user', createdAt: 1, parts: [{ type: 'text', text: 'question' }] } }
    const ids = () => s.chatPlanTimeline.value.map(item => item.id)
    expect(ids()).toEqual(['turn:g', 'question', 'config'])
    s.enabled.value = true
    expect(ids()).toEqual(['turn:g', 'plan', 'question', 'config'])
    expect(s.chatPlanTimeline.value[1]).toBe(s.plans.value[0])
    s.enabled.value = false
    expect(ids()).toEqual(['turn:g', 'question', 'config'])
    expect(s.plans.value).toHaveLength(1)
    s.messages.value = []; s.enabled.value = true; s.panel.value = null
    expect(ids()).toEqual(['plan'])
  })

  it('keeps previous turns/messages by reference while streaming replaces only the changed answer', () => {
    const s = setup()
    s.messages.value = [message('u1', 'user', 'g1'), message('a1', 'assistant', 'g1'),
      message('u2', 'user', 'g2'), message('a2', 'assistant', 'g2', { status: 'streaming' })]
    const first = s.chatPlanTimeline.value
    s.messages.value[3].content = 'new token'
    const next = s.chatPlanTimeline.value
    expect(next[0]).toBe(first[0])
    expect(next[1]).not.toBe(first[1])
    if (next[1].kind !== 'turn' || first[1].kind !== 'turn') throw new Error('missing turn')
    expect(next[1].user).toBe(first[1].user)
    expect(next[1].alternatives[0].parts).toContainEqual(expect.objectContaining({ text: 'new token' }))
    const stable = s.chatPlanTimeline.value
    s.revision.value++
    expect(s.chatPlanTimeline.value).toBe(stable)
  })

  it('preserves multi-model, attachment, context-divider and action identities', () => {
    const s = setup()
    s.messages.value = [message('u', 'user', 'g', { context_cleared: true,
      attachments: [{ id: 'file', name: 'a.pdf', type: 'document', size: 2048 }] }),
    message('a', 'assistant', 'g', { context_cleared: true, is_favorite: 1 }),
    message('b', 'assistant', 'g', { context_cleared: true, is_liked: -1 })]
    const [turn, divider] = s.chatPlanTimeline.value
    expect(turn).toMatchObject({ kind: 'turn', id: 'turn:g', activeId: 'a', comparisonMode: 'tabs', metadata: { groupId: 'g' } })
    if (turn.kind !== 'turn') throw new Error('missing turn')
    expect(turn.user.id).toBe('u')
    expect(turn.user.parts).toContainEqual(expect.objectContaining({ type: 'attachment', name: 'a.pdf' }))
    expect(turn.alternatives.map(item => item.id)).toEqual(['a', 'b'])
    expect(turn.alternatives[0].metadata?.favorite).toBe(true)
    expect(turn.alternatives[1].feedback).toBe(-1)
    expect(divider).toMatchObject({ id: 'context-divider:g', renderer: 'fastclaw-context-divider' })
  })

  it('excludes provider form credentials and reuses the original pending question', () => {
    const s = setup()
    const pending = { id: 'config', createdAt: 1, modelId: 'model',
      userMessage: { id: 'question', role: 'user' as const, createdAt: 1, parts: [{ type: 'text' as const, text: 'hello' }] },
      provider: { apiKey: 'secret-do-not-render' }, values: { apiKey: 'secret-do-not-render' } }
    s.panel.value = pending
    const entries = s.providerCompletionTimeline.value
    expect(entries[0].kind === 'message' && entries[0].message).toBe(s.panel.value.userMessage)
    expect(entries[1]).toMatchObject({ kind: 'message', id: 'config',
      message: { model: { id: 'model', name: 'Model name' }, parts: [{ type: 'custom', kind: 'fastclaw-vendor-config', data: {} }] } })
    expect(JSON.stringify(entries)).not.toContain('secret-do-not-render')
    s.panel.value = null
    expect(s.providerCompletionTimeline.value).toEqual([])
  })

  it('does not share caches between workspaces or retain a removed conversation', () => {
    const a = setup(), b = setup()
    a.messages.value = [message('same', 'user', undefined, { content: 'first tab' })]
    b.messages.value = [message('same', 'user', undefined, { content: 'second tab' })]
    expect(a.chatPlanTimeline.value[0]).not.toBe(b.chatPlanTimeline.value[0])
    expect(JSON.stringify(a.chatPlanTimeline.value)).toContain('first tab')
    expect(JSON.stringify(b.chatPlanTimeline.value)).toContain('second tab')
    a.messages.value = []
    expect(a.chatPlanTimeline.value).toEqual([])
    expect(b.chatPlanTimeline.value).toHaveLength(1)
    a.messages.value = [message('same', 'user', undefined, { content: 'restored' })]
    expect(JSON.stringify(a.chatPlanTimeline.value)).toContain('restored')
  })
})
