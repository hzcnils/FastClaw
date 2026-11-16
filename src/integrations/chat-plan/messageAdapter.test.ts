import { describe, expect, it } from 'vitest'
import type { Message } from '@/types'
import { adaptMessage } from './messageAdapter'

const base = (overrides: Partial<Message> = {}): Message => ({
  id: 'm1', conversation_id: 'c1', role: 'assistant', content: 'answer', model_id: 'model-1',
  token_input: null, token_output: null, status: 'completed', created_at: '2026-08-28T00:00:00Z', ...overrides,
})

describe('adaptMessage', () => {
  it('carries the host message identity into inline web source cards', () => {
    const hits = [{ title: 'source', url: 'https://example.com', snippet: 'text' }]
    const result = adaptMessage(base({ web_hits: hits, web_search_query: 'query' }))
    expect(result.parts).toContainEqual({ type: 'custom', kind: 'fastclaw-web-search', data: {
      messageId: 'm1', searching: false, query: 'query', hits,
    } })
  })
  it('renders waiting and finalizing as notices without creating empty reasoning', () => {
    for (const think_content of [undefined, '', ' \n']) {
      const source = base({ content: '', status: 'streaming', isThinking: true, think_content })
      const waiting = adaptMessage(source)
      expect(waiting.parts.some(part => part.type === 'reasoning')).toBe(false)
      expect(waiting.parts).toContainEqual({ type: 'notice', text: '等待模型响应…' })
      expect(adaptMessage({ ...source, stream_phase: 'finalizing' }).parts)
        .toContainEqual({ type: 'notice', text: '正在生成最终回答…' })
      expect(adaptMessage({ ...source, status: 'error' }).parts).toEqual([])
    }
  })

  it('completes real reasoning when body streaming starts', () => {
    const source = base({ status: 'streaming', content: '', think_content: '分析', stream_phase: 'reasoning' })
    expect(adaptMessage(source).parts[0]).toMatchObject({ type: 'reasoning', status: 'streaming' })
    expect(adaptMessage({ ...source, content: '回答', stream_phase: 'answering' }).parts[0])
      .toMatchObject({ type: 'reasoning', status: 'complete' })
    expect(adaptMessage({ ...source, stream_phase: 'answering', tool_calls: [{ id: 't1', name: 'search', status: 'running' }] }).parts[0])
      .toMatchObject({ type: 'reasoning', status: 'complete' })
  })
  it('maps reasoning, tools, attachments, translation and feedback without mutating host data', () => {
    const source = base({ think_content: 'reason', is_liked: 1, translation: '译文', translation_lang: 'zh',
      attachments: [{ id: 'a1', name: 'a.png', type: 'image', preview_url: 'blob:a' }],
      tool_calls: [{ id: 't1', name: 'search', status: 'done', result_preview: 'ok' }],
    })
    const before = structuredClone(source)
    const result = adaptMessage(source, { modelName: () => 'Model One' })
    expect(result.parts.map(part => part.type)).toEqual(['reasoning', 'tool-call', 'image', 'text'])
    expect(result.feedback).toBe(1)
    expect(result.translations?.[0].parts[0]).toMatchObject({ type: 'text', text: '译文' })
    expect(result.model?.name).toBe('Model One')
    expect(source).toEqual(before)
  })

  it('normalizes canceled and preserves its host status', () => {
    const result = adaptMessage(base({ status: 'canceled' }))
    expect(result.status).toBe('complete')
    expect(result.metadata?.hostStatus).toBe('canceled')
  })

  it('supports dirty historical data and runtime custom cards', () => {
    const result = adaptMessage(base({ id: 'temp-1', created_at: '', content: '',
      web_searching: true, skill_used: { name: 'writer' }, _vendorConfig: true,
    }))
    expect(result.createdAt).toBe(0)
    expect(result.parts.filter(part => part.type === 'custom')).toHaveLength(3)
  })

  it('maps failed assistant messages to the package error contract without duplicate text', () => {
    const content = 'HTTP状态码:503\n响应: model unavailable'
    const result = adaptMessage(base({ status: 'error', content }))
    expect(result.status).toBe('error')
    expect(result.error).toEqual({ raw: content })
    expect(result.parts).toEqual([])
  })

  it('forwards transient retry progress only while streaming', () => {
    const progress = { attempt: 1, total: 5 }
    expect(adaptMessage(base({ status: 'streaming', retry_progress: progress })).retry).toEqual(progress)
    expect(adaptMessage(base({ status: 'completed', retry_progress: progress })).retry).toBeUndefined()
  })

  it('normalizes persisted tool result envelopes without echoing transport JSON', () => {
    const content = JSON.stringify({
      arguments: { days: 1, location: '西安' },
      result: JSON.stringify({ content: '【陕西西安 天气】\n晴，气温 30.1°C', is_error: false, metadata: null }),
      success: true,
      tool_call_id: 'call-weather-1',
      tool_name: 'get_weather',
    })
    const result = adaptMessage(base({ id: 'tool-message-1', role: 'tool', model_id: null, content }))

    expect(result.parts).toEqual([{
      type: 'tool-call',
      id: 'call-weather-1',
      name: 'get_weather',
      input: { days: 1, location: '西安' },
      output: '【陕西西安 天气】\n晴，气温 30.1°C',
      status: 'complete',
      error: undefined,
    }])
  })

  it('keeps malformed tool-role content as ordinary text', () => {
    const result = adaptMessage(base({ id: 'legacy-tool', role: 'tool', model_id: null, content: 'legacy result' }))
    expect(result.parts).toEqual([{ type: 'text', text: 'legacy result', format: 'markdown' }])
  })

  it('maps failed persisted tool results to an error tool part', () => {
    const content = JSON.stringify({
      arguments: { location: '西安' }, result: 'network unavailable', success: false,
      tool_call_id: 'call-weather-error', tool_name: 'get_weather',
    })
    const result = adaptMessage(base({ id: 'tool-error', role: 'tool', model_id: null, content }))
    expect(result.parts[0]).toMatchObject({
      type: 'tool-call', status: 'error', output: 'network unavailable', error: 'network unavailable',
    })
  })
})

