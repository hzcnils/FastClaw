import type { LocalKnowledgeBase } from '@/types/knowledge'
import type { FilesystemSelection } from '@/components/chat/FilesystemBrowserModal.vue'

// Backend canonicalizes existing paths (including Windows casing). Preserve case
// here so distinct files in case-sensitive directories are not merged.
export const sourcePathKey = (path: string) => path.replace(/\\/g, '/').replace(/\/$/, '')

export function uniqueSelections(selections: FilesystemSelection[]): FilesystemSelection[] {
  const seen = new Set<string>()
  return selections.filter(source => {
    const key = sourcePathKey(source.path)
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
}

export function matchingLocalKnowledgeBase(kbs: LocalKnowledgeBase[], selections: FilesystemSelection[]) {
  const keys = new Set(selections.map(s => `${s.type}:${sourcePathKey(s.path)}`))
  return kbs.find(kb => kb.sources.length === keys.size && kb.sources.every(s =>
    !s.recursive && keys.has(`${s.entryType}:${sourcePathKey(s.path)}`)))
}

export function splitKnowledgeSelection(ids: string[], local: LocalKnowledgeBase[]) {
  const localIds = new Set(local.map(kb => kb.id))
  return {
    remoteIds: ids.filter(id => !localIds.has(id)),
    localSources: local.filter(kb => ids.includes(kb.id) && kb.enabled).flatMap(kb => kb.sources.map(s => ({ ...s }))),
  }
}
