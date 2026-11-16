import { describe, expect, it } from 'vitest'
import { parseChatStreamText } from '../chatStreamText'

describe('streamed text and thinking separation', () => {
  it('preserves ordinary text and whitespace', () => {
    expect(parseChatStreamText('## 雨夜\n\n琴声 ')).toEqual({ content: '## 雨夜\n\n琴声 ', thinkContent: '', inThink: false })
  })

  it('withholds partial tags and handles every possible split point', () => {
    const text = '<think>分析一</think>正文<thinking>分析二</thinking>尾部'
    for (let i = 0; i <= text.length; i++) {
      const parsed = parseChatStreamText(text.slice(0, i))
      expect(parsed.content).not.toMatch(/<\/?(?:t|th|thi|thin|think)/i)
      expect(parsed.thinkContent).not.toMatch(/<\/?(?:t|th|thi|thin|think)/i)
    }
    expect(parseChatStreamText(text)).toEqual({ content: '正文尾部', thinkContent: '分析一\n\n分析二', inThink: false })
  })

  it('supports mixed case, unclosed reasoning and literal less-than signs', () => {
    expect(parseChatStreamText('前文<THINKING>分析')).toEqual({ content: '前文', thinkContent: '分析', inThink: true })
    expect(parseChatStreamText('a < b')).toEqual({ content: 'a < b', thinkContent: '', inThink: false })
    expect(parseChatStreamText('<think></think>回答').thinkContent).toBe('')
  })
})
