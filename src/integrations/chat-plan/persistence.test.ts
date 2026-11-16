import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it } from 'vitest'
import type { ChatEmbeddedTimelineEntry } from 'chat-renderer'
import { usePlanActivityStore } from '@/stores/planActivity'
import {
  persistedEntriesToTimeline, readFastPlanPersistence, releaseFastPlanWriter,
  tryAcquireFastPlanWriter, writeFastPlanPersistence,
} from './persistence'

describe('FastPlan persistence', () => {
  it('removes credentials and endpoints on both save and restore', () => {
    const model = { id: 'db-model', model: 'display-name', api_key: 'secret', apiKey: 'secret', parentToken: 'token', baseUrl: 'http://user:pass@host' }
    const entry: ChatEmbeddedTimelineEntry = { kind: 'embedded', id: 'fp-credentials', renderer: 'fastplan', data: { sessionId: 's', model } }
    const stored = writeFastPlanPersistence(null, [entry], entry.id, 'writer')
    expect(JSON.parse(stored).fastplan.entries[0].model).toEqual({ id: 'db-model', model: 'display-name' })
    const old = JSON.stringify({ fastplan: { version: 2, entries: [{ id: entry.id, sessionId: 's', model }] } })
    expect(readFastPlanPersistence(old).entries[0].model).toEqual({ id: 'db-model', model: 'display-name' })
  })
  it('restores the legacy session field as one historical entry', () => {
    const state = readFastPlanPersistence(JSON.stringify({ fastplan_session_id: 'legacy-1' }))
    expect(state.entries[0]).toMatchObject({ id: 'fastplan:legacy:legacy-1', sessionId: 'legacy-1' })
    expect(persistedEntriesToTimeline(state)[0]).toMatchObject({ kind: 'embedded', renderer: 'fastplan' })
  })

  it('merges unrelated config, increments revision and dual-writes legacy id', () => {
    const entry: ChatEmbeddedTimelineEntry = { kind: 'embedded', id: 'fp-1', renderer: 'fastplan', status: 'active', data: { sessionId: 's1', prompt: 'plan', executionMode: 'step' } }
    const first = writeFastPlanPersistence('{"mcp":true}', [entry], 'fp-1', 'tab-a')
    const second = writeFastPlanPersistence(first, [entry], 'fp-1', 'tab-a')
    expect(JSON.parse(second)).toMatchObject({ mcp: true, fastplan_session_id: 's1', fastplan: { version: 2, revision: 2, writerId: 'tab-a' } })
    expect(readFastPlanPersistence(second).entries[0].executionMode).toBe('step')
    expect(persistedEntriesToTimeline(readFastPlanPersistence(second))[0].data).toMatchObject({ executionMode: 'step' })
  })

  it('restores an empty V2 timeline through the retained session link and preserves revision', () => {
    const config = JSON.stringify({
      fastplan: { version: 2, revision: 16, writerId: 'old-tab', updatedAt: '2026-09-04T01:40:21.755Z', activeEntryId: null, entries: [] },
      fastplan_session_id: 'd84ad8d7-c519-4032-8dcf-e4ea53a12ebd',
    })
    const state = readFastPlanPersistence(config)
    const id = 'fastplan:legacy:d84ad8d7-c519-4032-8dcf-e4ea53a12ebd'
    expect(state).toMatchObject({ revision: 16, writerId: 'old-tab', updatedAt: '2026-09-04T01:40:21.755Z', activeEntryId: id })
    const timeline = persistedEntriesToTimeline(state)
    expect(timeline).toHaveLength(1)
    expect(timeline[0]).toMatchObject({ id, renderer: 'fastplan', data: { sessionId: 'd84ad8d7-c519-4032-8dcf-e4ea53a12ebd', connectionMode: 'history' } })
    const saved = JSON.parse(writeFastPlanPersistence(config, timeline, state.activeEntryId!, 'new-tab'))
    expect(saved.fastplan).toMatchObject({ revision: 17, activeEntryId: id, entries: [{ id, sessionId: 'd84ad8d7-c519-4032-8dcf-e4ea53a12ebd' }] })
  })

  it('falls back when all V2 entries are invalid', () => {
    const state = readFastPlanPersistence(JSON.stringify({ fastplan: { version: 2, entries: [null, {}, { id: 'bad', sessionId: '  ' }, { id: '', sessionId: 's' }] }, fastplan_session_id: 'legacy' }))
    expect(state.entries).toEqual([{ id: 'fastplan:legacy:legacy', sessionId: 'legacy', status: 'complete' }])
  })

  it('prefers valid V2 entries over a different legacy link', () => {
    const state = readFastPlanPersistence(JSON.stringify({ fastplan: { version: 2, activeEntryId: 'v2', entries: [{ id: 'v2', sessionId: 'new', status: 'active' }] }, fastplan_session_id: 'old' }))
    expect(state.activeEntryId).toBe('v2')
    expect(state.entries).toHaveLength(1)
    expect(state.entries[0]).toMatchObject({ id: 'v2', sessionId: 'new', status: 'active' })
  })

  it.each([undefined, '', '   '])('keeps an empty timeline without a valid legacy link (%s)', legacy => {
    const state = readFastPlanPersistence(JSON.stringify({ fastplan: { version: 2, entries: [] }, fastplan_session_id: legacy }))
    expect(state.entries).toEqual([])
    expect(state.activeEntryId).toBeNull()
  })

  it('uses a single writer per conversation to avoid multi-tab lost updates', () => {
    expect(tryAcquireFastPlanWriter('c1', 'tab-a')).toBe(true)
    expect(tryAcquireFastPlanWriter('c1', 'tab-b')).toBe(false)
    releaseFastPlanWriter('c1', 'tab-a')
    expect(tryAcquireFastPlanWriter('c1', 'tab-b')).toBe(true)
    releaseFastPlanWriter('c1', 'tab-b')
  })
})

describe('plan activity registry', () => {
  beforeEach(() => setActivePinia(createPinia()))
  it('isolates busy state by chat tab path', () => {
    const store = usePlanActivityStore()
    store.update('/chat/a', { busy: true, entryId: 'a' })
    store.update('/chat/b', { busy: false, entryId: 'b' })
    expect(store.busyPaths).toEqual(['/chat/a'])
    store.update('/chat/b', { busy: true })
    expect(new Set(store.busyPaths)).toEqual(new Set(['/chat/a', '/chat/b']))
    store.remove('/chat/a')
    expect(store.busyPaths).toEqual(['/chat/b'])
  })
})

