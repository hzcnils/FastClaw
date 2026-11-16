import { describe, expect, it } from 'vitest'
import { matchingLocalKnowledgeBase, splitKnowledgeSelection, uniqueSelections } from '../localKnowledgeBatch'
import type { LocalKnowledgeBase } from '@/types/knowledge'

const selections = [
  { name: 'a.txt', path: 'D:/a.txt', type: 'file' as const },
  { name: 'b.txt', path: 'D:/b.txt', type: 'file' as const },
  { name: 'c', path: 'D:/c', type: 'directory' as const },
  { name: 'd', path: 'D:/d', type: 'directory' as const },
]
const kb: LocalKnowledgeBase = { id: 'local-id', name: 'a.txt 等4项', type: 'local', enabled: true,
  sources: selections.map(s => ({ path: s.path, entryType: s.type, recursive: false })),
}

describe('local knowledge batches', () => {
  it('preserves a mixed batch and removes repeated paths without merging distinct names', () => {
    expect(uniqueSelections([...selections, selections[0]])).toEqual(selections)
    expect(uniqueSelections([selections[0], { ...selections[0], path: 'D:/other/a.txt' }])).toHaveLength(2)
  })
  it('reuses only the complete matching source set regardless of order', () => {
    expect(matchingLocalKnowledgeBase([kb], [...selections].reverse())).toBe(kb)
    expect(matchingLocalKnowledgeBase([kb], selections.slice(0, 2))).toBeUndefined()
    expect(matchingLocalKnowledgeBase([{ ...kb, sources: kb.sources.map(s => ({ ...s, recursive: true })) }], selections)).toBeUndefined()
  })
  it('never includes local IDs in remote selection, and excludes disabled local sources', () => {
    expect(splitKnowledgeSelection(['remote-id', kb.id], [kb])).toEqual({ remoteIds: ['remote-id'], localSources: kb.sources })
    expect(splitKnowledgeSelection([kb.id], [{ ...kb, enabled: false }])).toEqual({ remoteIds: [], localSources: [] })
  })
})
