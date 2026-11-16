import { beforeEach, describe, expect, it } from 'vitest'
import {
  buildSnapshot,
  parseSnapshot,
  resolveTabsFromSnapshot,
  type TabSessionSnapshot,
} from '../tabSession'

describe('tab session normalization', () => {
  beforeEach(() => sessionStorage.clear())
  it('deduplicates restored tabs by path and keeps the latest metadata', () => {
    const snapshot: TabSessionSnapshot = {
      tabs: [
        { path: '/chat', title: 'sidebar.smartChat', closable: true },
        { path: '/chat/topic-1', title: '旧标题', closable: true },
        { path: '/chat/topic-1', title: '新标题', closable: true },
      ],
      activePath: '/chat/topic-1',
      unfinishedPaths: ['/chat/topic-1', '/chat/topic-1'],
      savedAt: 1,
    }

    const restored = resolveTabsFromSnapshot(snapshot, 'all')
    expect(restored.tabs.map(tab => tab.path)).toEqual(['/chat', '/chat/topic-1'])
    expect(restored.tabs[1].title).toBe('新标题')
    expect(restored.activePath).toBe('/chat/topic-1')
  })

  it('cleans duplicate tabs when parsing and writing snapshots', () => {
    const raw = JSON.stringify({
      tabs: [
        { path: '/chat', title: '智能对话', closable: true },
        { path: '/chat', title: '智能对话', closable: true },
      ],
      activePath: '/chat',
      unfinishedPaths: [],
      savedAt: 1,
    })

    expect(parseSnapshot(raw)?.tabs).toHaveLength(1)
    expect(buildSnapshot(parseSnapshot(raw)!.tabs, '/chat').tabs).toHaveLength(1)
  })

  it('restores only one tab per conversation and redirects the duplicate active tab', () => {
    const snapshot: TabSessionSnapshot = {
      tabs: [
        { path: '/chat', title: '问题 X', closable: true, conversationId: 'x' },
        { path: '/chat/duplicate', title: '问题 X', closable: true, conversationId: 'x' },
        { path: '/chat/other', title: '问题 Y', closable: true, conversationId: 'y' },
      ],
      activePath: '/chat/duplicate', unfinishedPaths: [], savedAt: 1,
    }
    const result = resolveTabsFromSnapshot(snapshot, 'all')
    expect(result.tabs.map(tab => tab.path)).toEqual(['/chat', '/chat/other'])
    expect(result.activePath).toBe('/chat')
    expect(parseSnapshot(JSON.stringify(buildSnapshot(result.tabs, result.activePath)))?.tabs[1].conversationId).toBe('y')
  })

  it('deduplicates legacy sessionStorage selections while preserving blank tabs', () => {
    sessionStorage.setItem('fastclaw-active-conversation:a', 'x')
    sessionStorage.setItem('fastclaw-active-conversation:b', 'x')
    const result = resolveTabsFromSnapshot({
      tabs: [
        { path: '/chat/a', title: 'X', closable: true },
        { path: '/chat/b', title: 'X', closable: true },
        { path: '/chat/draft', title: '草稿', closable: true },
      ], activePath: '/chat/b', unfinishedPaths: [], savedAt: 1,
    }, 'all')
    expect(result.tabs.map(tab => tab.path)).toEqual(['/chat', '/chat/b', '/chat/draft'])
    expect(result.activePath).toBe('/chat/b')
  })
})
