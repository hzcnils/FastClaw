import type { ChatEmbeddedTimelineEntry } from 'chat-renderer'
import type { FastPlanTimelineEntryData } from 'planflow-kit'
import { publicFastPlanModel } from 'planflow-runtime'
import { fastPlanSessionManager } from './sessionManager'

export const FASTPLAN_PERSISTENCE_VERSION = 2 as const

export interface PersistedFastPlanEntry {
  id: string
  sessionId: string
  prompt?: string
  createdAt?: string | number
  status?: ChatEmbeddedTimelineEntry['status']
  model?: unknown
  executionMode?: 'auto' | 'manual' | 'step'
}

export interface FastPlanPersistenceState {
  version: typeof FASTPLAN_PERSISTENCE_VERSION
  revision: number
  writerId?: string
  updatedAt?: string
  activeEntryId?: string | null
  entries: PersistedFastPlanEntry[]
}

export function tryAcquireFastPlanWriter(conversationId: string, writerId: string): boolean {
  return fastPlanSessionManager.acquireWriter(conversationId, writerId)
}

export function releaseFastPlanWriter(conversationId: string, writerId: string): void {
  fastPlanSessionManager.releaseWriter(conversationId, writerId)
}

function parseObject(extraConfig?: string | null): Record<string, any> {
  if (!extraConfig) return {}
  try {
    const value = JSON.parse(extraConfig)
    return value && typeof value === 'object' && !Array.isArray(value) ? value : {}
  } catch {
    return {}
  }
}

export function readFastPlanPersistence(extraConfig?: string | null): FastPlanPersistenceState {
  const root = parseObject(extraConfig)
  const fastplan = root.fastplan
  const legacySessionId = typeof root.fastplan_session_id === 'string' ? root.fastplan_session_id.trim() : ''
  const legacyEntry: PersistedFastPlanEntry | undefined = legacySessionId
    ? { id: `fastplan:legacy:${legacySessionId}`, sessionId: legacySessionId, status: 'complete' }
    : undefined
  if (fastplan?.version === FASTPLAN_PERSISTENCE_VERSION && Array.isArray(fastplan.entries)) {
    const entries: PersistedFastPlanEntry[] = fastplan.entries
      .filter((entry: any) => entry && typeof entry.id === 'string' && entry.id.trim()
        && typeof entry.sessionId === 'string' && entry.sessionId.trim())
      .map((entry: PersistedFastPlanEntry) => ({
        ...entry,
        model: publicFastPlanModel(entry.model),
        executionMode: ['auto', 'manual', 'step'].includes(String(entry.executionMode)) ? entry.executionMode : undefined,
      }))
    // Older writes can leave an empty V2 timeline while retaining the session link.
    const fallback = entries.length === 0 ? legacyEntry : undefined
    return {
      version: FASTPLAN_PERSISTENCE_VERSION,
      revision: Number(fastplan.revision) || 0,
      writerId: typeof fastplan.writerId === 'string' ? fastplan.writerId : undefined,
      updatedAt: typeof fastplan.updatedAt === 'string' ? fastplan.updatedAt : undefined,
      activeEntryId: fallback ? fallback.id : typeof fastplan.activeEntryId === 'string' ? fastplan.activeEntryId : null,
      entries: fallback ? [fallback] : entries,
    }
  }
  return {
    version: FASTPLAN_PERSISTENCE_VERSION,
    revision: 0,
    activeEntryId: legacyEntry?.id || null,
    entries: legacyEntry ? [legacyEntry] : [],
  }
}

export function persistedEntriesToTimeline(state: FastPlanPersistenceState): ChatEmbeddedTimelineEntry[] {
  return state.entries.map((entry) => ({
    kind: 'embedded',
    id: entry.id,
    renderer: 'fastplan',
    status: entry.status || 'complete',
    data: {
      sessionId: entry.sessionId,
      prompt: entry.prompt,
      createdAt: entry.createdAt,
      connectionMode: entry.status === 'active' ? 'live' : 'history',
      executionMode: entry.executionMode,
      model: publicFastPlanModel(entry.model),
    } satisfies FastPlanTimelineEntryData,
  }))
}

export function writeFastPlanPersistence(
  extraConfig: string | null | undefined,
  timelineEntries: readonly ChatEmbeddedTimelineEntry[],
  activeEntryId: string | null,
  writerId: string,
): string {
  const root = parseObject(extraConfig)
  const previous = readFastPlanPersistence(extraConfig)
  const entries: PersistedFastPlanEntry[] = timelineEntries.flatMap((entry) => {
    const data = entry.data as FastPlanTimelineEntryData
    if (!data.sessionId) return []
    return [{
      id: entry.id,
      sessionId: data.sessionId,
      prompt: data.prompt,
      createdAt: typeof data.createdAt === 'object' ? data.createdAt.toISOString() : data.createdAt,
      status: entry.status,
      executionMode: data.executionMode,
      model: publicFastPlanModel(data.model),
    }]
  })
  const state: FastPlanPersistenceState = {
    version: FASTPLAN_PERSISTENCE_VERSION,
    revision: previous.revision + 1,
    writerId,
    updatedAt: new Date().toISOString(),
    activeEntryId,
    entries,
  }
  const legacy = entries.find((entry) => entry.id === activeEntryId)?.sessionId || entries.at(-1)?.sessionId
  return JSON.stringify({
    ...root,
    task_plan: true,
    fastplan: state,
    fastplan_session_id: legacy || root.fastplan_session_id || undefined,
  })
}

