import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
vi.mock('@/api/knowledge', () => ({
  knowledgeBaseApi: {}, kgServiceReady: { value: false }, checkKgServiceHealth: vi.fn(), ensureKgReady: vi.fn(), startKgService: vi.fn(),
}))
import { useKnowledgeBaseStore } from './index'

describe('drop batch persistence and generation boundary', () => {
  beforeEach(() => { localStorage.clear(); setActivePinia(createPinia()) })

  it('persists four mixed sources as one existing-format record and reuses a repeated batch', () => {
    const store = useKnowledgeBaseStore()
    const selections = [
      { path: 'D:/a.txt', name: 'a.txt', type: 'file' as const },
      { path: 'D:/b.docx', name: 'b.docx', type: 'file' as const },
      { path: 'D:/c', name: 'c', type: 'directory' as const },
      { path: 'D:/d', name: 'd', type: 'directory' as const },
    ]
    const kb = store.importLocalKnowledgeBatch(selections)!
    expect(store.localKnowledgeBases).toHaveLength(1)
    expect(kb.sources).toHaveLength(4)
    expect(kb.name).toBe('a.txt 等4项')
    const saved = JSON.parse(localStorage.getItem('fastclaw-local-kb')!)
    expect(saved).toEqual([kb])
    expect(saved[0]._type).toBeUndefined()
    expect(store.importLocalKnowledgeBatch([...selections].reverse())?.id).toBe(kb.id)
    expect(store.localKnowledgeBases).toHaveLength(1)
    expect(store.getEnabledLocalPaths([kb.id])).toHaveLength(4)
    expect(store.importLocalKnowledgeBatch([])).toBeNull()
  })
})
