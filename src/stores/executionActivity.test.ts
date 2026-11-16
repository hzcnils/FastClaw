import { beforeEach, describe, expect, it } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useExecutionActivityStore } from './executionActivity'
import { watch } from 'vue'

describe('executionActivity store', () => {
  beforeEach(() => setActivePinia(createPinia()))

  it('publishes only semantic changes, not every token or removal of an absent activity', () => {
    const store = useExecutionActivityStore()
    let notifications = 0
    const stop = watch(() => store.byKey, () => { notifications++ }, { flush: 'sync' })
    const patch = { phase: 'planning' as const, protected: true, connectionStatus: 'connected' as const }
    store.update('a', '/chat/a', 'fastplan', patch)
    const first = store.activityByConversation('a')
    for (let i = 0; i < 100; i++) store.update('a', '/chat/a', 'fastplan', patch)
    store.remove('missing'); store.remove('missing', 'chat'); store.removeByPath('/chat/missing')
    expect(store.activityByConversation('a')).toBe(first)
    expect(notifications).toBe(1)
    store.update('a', '/chat/a', 'fastplan', { phase: 'awaiting_plan_confirmation' })
    expect(notifications).toBe(2)
    store.removeByPath('/chat/a')
    expect(notifications).toBe(3)
    stop()
  })

  it('protects a tab while chat or FastPlan work is unfinished', () => {
    const store = useExecutionActivityStore()
    store.update('conversation-1', '/chat/a', 'chat', { phase: 'executing', protected: true })
    expect(store.isTabProtected('/chat/a')).toBe(true)
    expect(store.isConversationRunning('conversation-1')).toBe(true)

    store.remove('conversation-1', 'chat')
    expect(store.isTabProtected('/chat/a')).toBe(false)
  })

  it('prefers an actionable waiting state for badges and close protection', () => {
    const store = useExecutionActivityStore()
    store.update('conversation-1', '/chat/a', 'chat', { phase: 'completed', protected: false })
    store.update('conversation-1', '/chat/a', 'fastplan', {
      phase: 'awaiting_step_confirmation',
      protected: true,
      executionMode: 'step',
    })

    expect(store.activityByPath('/chat/a')?.phase).toBe('awaiting_step_confirmation')
    expect(store.activityByConversation('conversation-1')?.kind).toBe('fastplan')
  })

  it('removes all activity owned by a closed tab', () => {
    const store = useExecutionActivityStore()
    store.update('conversation-1', '/chat/a', 'chat', { phase: 'executing', protected: true })
    store.update('conversation-2', '/chat/a', 'fastplan', { phase: 'paused', protected: true })
    store.removeByPath('/chat/a')
    expect(store.protectedPaths).toEqual([])
  })
})
