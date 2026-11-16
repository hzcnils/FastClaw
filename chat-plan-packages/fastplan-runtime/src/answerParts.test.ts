import { describe, expect, it } from 'vitest'
import { splitFastPlanFinalAnswer } from './answerParts'
import { adaptFastPlanMessages } from './chatAdapter'

const option = { series: [{ type: 'bar', data: [1, 2] }] }
const json = JSON.stringify(option)
const fence = '```echarts\n' + json + '\n```\n'
const split = (content: string) => splitFastPlanFinalAnswer({ kind: 'final_answer', content })
const original = (parts: ReturnType<typeof splitFastPlanFinalAnswer>) => parts.map(p => p.type === 'text' ? p.text : p.source).join('')

describe('final answer inline charts', () => {
  it('keeps repeated charts at their original positions and preserves the exact source', () => {
    const text = 'Before\n\n' + fence + '\nBetween\n\n' + fence + '\nAfter'
    const parts = split(text)
    expect(parts.map(p => p.type)).toEqual(['text', 'chart', 'text', 'chart', 'text'])
    expect(original(parts)).toBe(text)
  })
  it('accepts standalone bare JSON and json fences, including chart-only answers', () => {
    for (const text of [json, fence.replace('echarts', 'json'), 'Before\n\n' + JSON.stringify(option, null, 2) + '\n\nAfter']) {
      const parts = split(text)
      expect(parts.filter(p => p.type === 'chart')).toHaveLength(1)
      expect(original(parts)).toBe(text)
    }
  })
  it('preserves malformed, unfinished, ordinary code, nested and inline JSON', () => {
    for (const text of ['```echarts\n' + json, '```echarts\n{bad}\n```', fence.replace('echarts', 'javascript'), 'Example ' + json,
      '  ```echarts\n  ' + json + '\n  ```', '> ```echarts\n> ' + json + '\n> ```', JSON.stringify({ output: option })]) {
      expect(split(text).every(p => p.type === 'text')).toBe(true)
      expect(original(split(text))).toBe(text)
    }
  })
  it('preserves raw final content for copy/history and never promotes node charts', () => {
    const messages = adaptFastPlanMessages([{ id: 'u', role: 'user', content: 'chart' }], {
      executionHistory: [{ treeId: 't', anchorMessageId: 'u', finalResult: { content: fence }, executionState: { a: { output: fence + fence } } }],
    })
    expect(messages[1].metadata.completeFinalResult).toBe(true)
    expect(messages[1].metadata.contentKind).toBe('final_answer')
    expect(messages[1].parts[0].text).toBe(fence.trim())
    expect(split(messages[1].parts[0].text).filter(p => p.type === 'chart')).toHaveLength(1)
    const ordinary = adaptFastPlanMessages([{ id: 'a', role: 'assistant', content: fence }])[0]
    expect(ordinary.parts[0].text).toBe(fence)
    expect(ordinary.metadata.contentKind).toBe('ordinary_message')
    expect(ordinary.metadata.completeFinalResult).toBeUndefined()
  })
  it('rejects untyped node output at the extraction boundary', () => {
    expect(splitFastPlanFinalAnswer({ kind: 'node_output', content: fence } as any)).toEqual([])
    expect(splitFastPlanFinalAnswer(fence as any)).toEqual([])
  })
})
