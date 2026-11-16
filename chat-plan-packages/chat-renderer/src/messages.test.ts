import { describe, expect, it } from 'vitest'
import { defaultMessages, resolveChatRendererMessages } from './messages'

describe('chat renderer messages', () => {
  it('uses Chinese as the built-in default', () => {
    expect(defaultMessages.copy).toBe('复制')
    expect(resolveChatRendererMessages().scrollToBottom).toBe('滚动到底部')
  })

  it('selects English and lets a host language pack override it', () => {
    expect(resolveChatRendererMessages('en-US').copy).toBe('Copy')
    expect(resolveChatRendererMessages('fr-FR', { copy: 'Copier', loading: 'Chargement…' })).toMatchObject({
      copy: 'Copier',
      loading: 'Chargement…',
      cancel: '取消',
    })
  })
})
