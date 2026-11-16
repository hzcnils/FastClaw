import { describe, expect, it } from 'vitest'
import { calculateDirectActionCount, groupMessagesIntoTurns, resolveComposerActions, resolveMessageActions, safeUrl, visibleContextItems } from './core'
import { MESSAGE_ACTION_DIVIDER } from './types'
import type { ChatMessage, MessageActionContext } from './types'

function message(id: string, role: ChatMessage['role'], status: ChatMessage['status'] = 'complete'): ChatMessage {
  return { id, role, status, createdAt: 1, parts: [{ type: 'text', text: id }] }
}

const context: MessageActionContext = {
  index: 0, isLastMessage: true, isLastCompletedAssistantTurn: true,
  isGenerating: false, hasStableId: true,
}

describe('groupMessagesIntoTurns', () => {
  it('starts a new turn at every user message and keeps tool messages with the answer', () => {
    const turns = groupMessagesIntoTurns([
      message('u1', 'user'), message('t1', 'tool'), message('a1', 'assistant'),
      message('u2', 'user'), message('a2', 'assistant'),
    ])
    expect(turns).toHaveLength(2)
    expect(turns[0].messages.map(item => item.id)).toEqual(['u1', 't1', 'a1'])
    expect(turns[1].messages.map(item => item.id)).toEqual(['u2', 'a2'])
  })
})

describe('resolveMessageActions', () => {
  it('uses safe role defaults and supports the global switch', () => {
    expect(resolveMessageActions(message('u1', 'user'), context).map(item => item.name)).toEqual(['copy', 'edit', 'resend'])
    expect(resolveMessageActions(message('u1', 'user'), context, false)).toEqual([])
  })

  it('never resolves standard actions for error messages', () => {
    const result = resolveMessageActions(message('a1', 'assistant', 'error'), context, true, { copy: true }, () => ['copy'])
    expect(result).toEqual([])
  })

  it('hides persistent and destructive actions without a stable id', () => {
    const result = resolveMessageActions(message('', 'assistant'), { ...context, hasStableId: false }, true, { delete: true })
    expect(result.map(item => item.name)).toEqual(['copy'])
  })

  it('blocks regenerate, edit, delete and export while generating', () => {
    const result = resolveMessageActions(message('a1', 'assistant', 'streaming'), { ...context, isGenerating: true }, true, {
      edit: true, delete: true, export: true,
    })
    expect(result.map(item => item.name)).toEqual(['copy', 'like', 'dislike'])
  })

  it('applies the resolver last', () => {
    const result = resolveMessageActions(message('a1', 'assistant'), context, true, { quote: true }, () => ['copy', 'quote'])
    expect(result.map(item => item.name)).toEqual(['copy', 'quote'])
  })

  it('uses the exact order returned by a name-array resolver', () => {
    const result = resolveMessageActions(message('a1', 'assistant'), context, true, {
      translate: { enabled: true, order: 100 },
      quote: { enabled: true, order: 1 },
    }, () => ['translate', 'copy', 'quote', 'like'])
    expect(result.map(item => item.name)).toEqual(['translate', 'copy', 'quote', 'like'])
  })

  it('continues to sort a resolved-action resolver by item order', () => {
    const resolved = resolveMessageActions(message('a1', 'assistant'), context, true, {}, () => [
      { name: 'copy', actionId: 'copy', label: 'copy', order: 20, visible: true, disabled: false },
      { name: 'like', actionId: 'like', label: 'like', order: 10, visible: true, disabled: false },
    ])
    expect(resolved.map(item => item.name)).toEqual(['like', 'copy'])
  })

  it('maps resolver divider markers onto the following visible action', () => {
    const result = resolveMessageActions(message('a1', 'assistant'), context, true, {
      quote: true, delete: true,
    }, () => ['copy', 'quote', MESSAGE_ACTION_DIVIDER, 'delete'])
    expect(result.map(item => item.name)).toEqual(['copy', 'quote', 'delete'])
    expect(result.find(item => item.name === 'quote')?.dividerBefore).not.toBe(true)
    expect(result.find(item => item.name === 'delete')?.dividerBefore).toBe(true)
  })

  it('keeps role-specific configured actions on the correct message role', () => {
    const actions = {
      edit: true, resend: true, regenerate: true, feedback: true, like: true, dislike: true,
      'edit-as-doc': true, 'switch-model': true, 'trace-source': true,
    }
    expect(resolveMessageActions(message('u1', 'user'), context, true, actions).map(item => item.name))
      .toEqual(['copy', 'edit', 'resend'])
    expect(resolveMessageActions(message('a1', 'assistant'), context, true, actions).map(item => item.name))
      .toEqual(['copy', 'regenerate', 'like', 'dislike', 'edit', 'feedback', 'edit-as-doc', 'switch-model', 'trace-source'])
  })

  it('turns a loading action into a disabled resolved action', () => {
    const result = resolveMessageActions(message('a1', 'assistant'), context, true, {
      translate: { loading: true },
    })
    expect(result.find(item => item.name === 'translate')).toMatchObject({ loading: true, disabled: true })
  })

  it('preserves action icons and second-level menu items', () => {
    const menuItems = [{ id: 'zh', label: '中文', payload: { locale: 'zh-CN' } }]
    const result = resolveMessageActions(message('a1', 'assistant'), context, true, {
      translate: { enabled: true, icon: 'translate', menuItems },
    })
    expect(result.find(item => item.name === 'translate')).toMatchObject({ icon: 'translate', menuItems })
  })
})

describe('calculateDirectActionCount', () => {
  const resolved = (['copy', 'regenerate', 'quote', 'tts', 'translate', 'favorite', 'export', 'like'] as const)
    .map((name, order) => ({ name, actionId: name, label: name, order, visible: true, disabled: false }))

  it('uses five actions before measurement and adapts in both directions', () => {
    expect(calculateDirectActionCount(resolved, 0)).toBe(5)
    expect(calculateDirectActionCount(resolved, 100)).toBe(2)
    expect(calculateDirectActionCount(resolved, 400)).toBe(8)
  })
})

describe('safeUrl', () => {
  it('rejects executable and embedded data protocols', () => {
    expect(safeUrl('javascript:alert(1)')).toBeNull()
    expect(safeUrl('data:text/html,<script>alert(1)</script>')).toBeNull()
    expect(safeUrl('data:image/svg+xml,<svg onload=alert(1)>', 'image')).toBeNull()
  })

  it('allows normal web links and safe image sources', () => {
    expect(safeUrl('https://example.com')).toBe('https://example.com')
    expect(safeUrl('/relative')).toBe('/relative')
    expect(safeUrl('blob:https://example.com/id', 'image')).toBe('blob:https://example.com/id')
    expect(safeUrl('data:image/png;base64,AAAA', 'image')).toBe('data:image/png;base64,AAAA')
  })
})

describe('conversation controls', () => {
  it('filters and stably orders composer actions', () => {
    const result = resolveComposerActions([
      { id: 'file', label: 'File', order: 20 },
      { id: 'image', label: 'Image', order: 10 },
      { id: 'hidden', label: 'Hidden', visible: false, order: 0 },
      { id: 'task', label: 'Task', order: 20 },
    ])
    expect(result.map(action => action.id)).toEqual(['image', 'file', 'task'])
  })

  it('keeps disabled context items visible but removes explicitly hidden items', () => {
    const result = visibleContextItems([
      { id: 'docs', kind: 'document', label: 'Docs', disabled: true },
      { id: 'kb', kind: 'knowledge', label: 'KB', visible: false },
    ])
    expect(result.map(item => item.id)).toEqual(['docs'])
  })
})
