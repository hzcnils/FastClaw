import { describe, expect, it } from 'vitest'
import type { Message } from '@/types'
import { adaptMessagesToTimeline, createIncrementalTimelineAdapter, groupHostMessages } from './timelineAdapter'

const message = (id: string, role: Message['role'], group_id?: string, extra: Partial<Message> = {}): Message => ({
  id, conversation_id: 'c1', role, content: id, model_id: null, token_input: null, token_output: null,
  status: 'completed', created_at: '2026-08-28T00:00:00Z', group_id, ...extra,
})

describe('timeline adapter', () => {
  it('creates a turn for contiguous multi-model answers', () => {
    const timeline = adaptMessagesToTimeline([
      message('u1', 'user', 'g1'), message('a1', 'assistant', 'g1'), message('a2', 'assistant', 'g1'),
    ])
    expect(timeline).toHaveLength(1)
    expect(timeline[0]).toMatchObject({ kind: 'turn', id: 'turn:g1', comparisonMode: 'tabs' })
    if (timeline[0].kind === 'turn') expect(timeline[0].alternatives.map(item => item.id)).toEqual(['a1', 'a2'])
  })

  it('does not merge non-contiguous reused group ids', () => {
    const groups = groupHostMessages([
      message('u1', 'user', 'g1'), message('s1', 'system'), message('a1', 'assistant', 'g1'),
    ])
    expect(groups).toHaveLength(3)
  })

  it('adds a divider only at the end of a cleared context run', () => {
    const timeline = adaptMessagesToTimeline([
      message('u1', 'user', 'g1', { context_cleared: true }),
      message('a1', 'assistant', 'g1', { context_cleared: true }),
      message('u2', 'user', 'g2'), message('a2', 'assistant', 'g2'),
    ])
    expect(timeline.map(entry => entry.kind === 'embedded' ? entry.renderer : entry.kind))
      .toEqual(['turn', 'fastclaw-context-divider', 'turn'])
  })

  it('reuses unchanged entries while replacing only the streaming turn', () => {
    const adapt = createIncrementalTimelineAdapter()
    const source = [message('u1', 'user', 'g1'), message('a1', 'assistant', 'g1'), message('u2', 'user', 'g2'), message('a2', 'assistant', 'g2')]
    const first = adapt(source)
    const next = [...source]
    next[3] = { ...next[3], content: 'stream delta', status: 'streaming' }
    const second = adapt(next)
    expect(second[0]).toBe(first[0])
    expect(second[1]).not.toBe(first[1])
  })
})
