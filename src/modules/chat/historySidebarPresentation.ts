import { computed } from 'vue'
import type { ExecutionPhase } from '@/stores/executionActivity'

export interface FavoriteHistoryItem {
  id: string
  message_id: string | null
  role: string
  content: string
  created_at: string
  conversation_id: string | null
  conversation_title: string
  conversation_deleted: boolean
  model_id: string | null
}

export interface HistoryActivity {
  streaming: boolean
  phase?: ExecutionPhase
  unread: boolean
}

export type HistoryActivitySummary = Readonly<Record<string, HistoryActivity>>

/** Only badge semantics invalidate the sidebar; never subscribe to message contents. */
export function useHistoryActivitySummary(
  conversations: () => readonly { id: string }[],
  read: (id: string) => HistoryActivity,
) {
  let previous: HistoryActivitySummary = Object.create(null)
  return computed(() => {
    const next: Record<string, HistoryActivity> = Object.create(null)
    let changed = false
    for (const { id } of conversations()) {
      const value = read(id)
      const old = previous[id]
      const same = old && old.streaming === value.streaming && old.phase === value.phase && old.unread === value.unread
      next[id] = same ? old : value
      if (!same) changed = true
    }
    if (!changed && Object.keys(previous).length === Object.keys(next).length) return previous
    previous = next
    return previous
  })
}
