import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

export type ExecutionKind = 'chat' | 'fastplan'

export type ExecutionPhase =
  | 'idle'
  | 'submitting'
  | 'planning'
  | 'awaiting_plan_confirmation'
  | 'executing'
  | 'awaiting_step_confirmation'
  | 'awaiting_human'
  | 'paused'
  | 'reconnecting'
  | 'completed'
  | 'failed'
  | 'cancelled'

export interface ConversationExecutionActivity {
  conversationId: string
  tabPath: string
  kind: ExecutionKind
  phase: ExecutionPhase
  protected: boolean
  sessionId?: string
  entryId?: string
  executionMode?: 'auto' | 'manual' | 'step'
  connectionStatus?: 'idle' | 'connecting' | 'connected' | 'reconnecting' | 'failed'
  updatedAt: number
}

type ActivityPatch = Omit<Partial<ConversationExecutionActivity>, 'conversationId' | 'tabPath' | 'kind' | 'updatedAt'>

function activityKey(conversationId: string, kind: ExecutionKind) {
  return `${conversationId}:${kind}`
}

const PHASE_PRIORITY: Record<ExecutionPhase, number> = {
  reconnecting: 90,
  awaiting_human: 80,
  awaiting_step_confirmation: 75,
  awaiting_plan_confirmation: 70,
  paused: 65,
  executing: 60,
  planning: 55,
  submitting: 50,
  completed: 20,
  failed: 20,
  cancelled: 10,
  idle: 0,
}

export const useExecutionActivityStore = defineStore('executionActivity', () => {
  const byKey = ref<Record<string, ConversationExecutionActivity>>({})

  function update(
    conversationId: string,
    tabPath: string,
    kind: ExecutionKind,
    patch: ActivityPatch,
  ) {
    if (!conversationId || !tabPath) return
    const key = activityKey(conversationId, kind)
    const previous = byKey.value[key]
    const next: ConversationExecutionActivity = {
      ...(previous || { phase: 'idle', protected: false, updatedAt: 0 }),
      ...patch, conversationId, tabPath, kind,
    }
    // This is a presentation index, not a per-token heartbeat. Publishing unchanged
    // data invalidates every history list and TabBar, including cached views.
    if (previous && (Object.keys(next) as Array<keyof ConversationExecutionActivity>)
      .every(field => field === 'updatedAt' || Object.is(previous[field], next[field]))) return
    next.updatedAt = Date.now()
    byKey.value = { ...byKey.value, [key]: next }
  }

  function remove(conversationId: string, kind?: ExecutionKind) {
    const next = { ...byKey.value }
    if (kind) delete next[activityKey(conversationId, kind)]
    else {
      for (const [key, item] of Object.entries(next)) {
        if (item.conversationId === conversationId) delete next[key]
      }
    }
    if (Object.keys(next).length !== Object.keys(byKey.value).length) byKey.value = next
  }

  function removeByPath(path: string) {
    const next = { ...byKey.value }
    for (const [key, item] of Object.entries(next)) {
      if (item.tabPath === path) delete next[key]
    }
    if (Object.keys(next).length !== Object.keys(byKey.value).length) byKey.value = next
  }

  function selectBest(items: ConversationExecutionActivity[]) {
    return items.sort((a, b) => {
      if (a.protected !== b.protected) return a.protected ? -1 : 1
      const phase = PHASE_PRIORITY[b.phase] - PHASE_PRIORITY[a.phase]
      return phase || b.updatedAt - a.updatedAt
    })[0]
  }

  function activityByConversation(conversationId: string) {
    return selectBest(Object.values(byKey.value).filter(item => item.conversationId === conversationId))
  }

  function activityByPath(path: string) {
    return selectBest(Object.values(byKey.value).filter(item => item.tabPath === path))
  }

  function isConversationRunning(conversationId: string) {
    return Object.values(byKey.value).some(item => item.conversationId === conversationId && item.protected)
  }

  function isTabProtected(path: string) {
    return Object.values(byKey.value).some(item => item.tabPath === path && item.protected)
  }

  const protectedPaths = computed(() => [...new Set(
    Object.values(byKey.value).filter(item => item.protected).map(item => item.tabPath),
  )])

  return {
    byKey,
    protectedPaths,
    update,
    remove,
    removeByPath,
    activityByConversation,
    activityByPath,
    isConversationRunning,
    isTabProtected,
  }
})
