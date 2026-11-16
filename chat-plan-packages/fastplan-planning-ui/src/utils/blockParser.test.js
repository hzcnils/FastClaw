import { describe, expect, it } from 'vitest'
import {
  collectExecutionNames,
  dedupeOutputSections,
  extractArgs,
  extractCompletedNodeOutput,
  normalizeNodeOutput,
  nodeTokens,
  resolveNodeElapsed,
  resolveNodeMetrics,
} from './blockParser'

describe('execution presentation normalization', () => {
  it('extracts Agent business output and hides its metadata envelope', () => {
    const output = JSON.stringify({
      content: '最终业务结论',
      agent_metrics: { confidence: 86, duration_ms: 1840 },
      persona: 'internal persona',
    })
    expect(normalizeNodeOutput(output)).toMatchObject({
      content: '最终业务结论',
      confidence: 86,
      durationMs: 1840,
    })
  })

  it('uses explicit and historical timing fields without inventing confidence', () => {
    expect(resolveNodeMetrics({ output: '{"rows":[1,2]}', startTime: 1000, endTime: 2420 }))
      .toEqual({ confidence: null, durationMs: 1420 })
    expect(resolveNodeMetrics({ confidence: 0.91, duration_ms: 800 }))
      .toEqual({ confidence: 91, durationMs: 800 })
  })

  it('uses a live frontend clock while running and backend duration after completion', () => {
    expect(resolveNodeElapsed({ status: 'running', startTime: 1000 }, 4250))
      .toEqual({ durationMs: 3250, live: true })
    expect(resolveNodeElapsed({ status: 'running', start_time: '2026-08-23T00:00:00Z' }, '2026-08-23T00:00:03Z'))
      .toEqual({ durationMs: 3000, live: true })
    expect(resolveNodeElapsed({ status: 'completed', startTime: 1000, endTime: 5000, durationMs: 3600 }, 9000))
      .toEqual({ durationMs: 3600, live: false })
  })

  it('prefers actual Tool, Skill and MCP calls and removes duplicate names', () => {
    const logs = [
      { type: 'tool_call', toolName: 'weather' },
      { type: 'tool_call', toolName: 'server:forecast' },
      { type: 'tool_call', toolName: 'weather' },
    ]
    const tree = { node_type: { config: { agent_type: 'researcher', assigned_tools: ['docx'] } } }
    expect(collectExecutionNames(logs, tree)).toEqual(['weather', 'server:forecast'])
    expect(collectExecutionNames([], tree)).toEqual(['researcher', 'docx'])
  })

  it('extracts nested runtime tool argument values for the execution row', () => {
    expect(extractArgs(JSON.stringify({
      tool_name: 'jmis_search',
      tool_args: { query: '2023–2026 年医学影像大语言模型文献', num_results: 10 },
    }))).toBe('2023–2026 年医学影像大语言模型文献')
    expect(extractArgs(JSON.stringify({ arguments: { city: '西安', unit: 'celsius' } })))
      .toBe('西安 · celsius')
  })

  it('keeps generic JSON output and deduplicates repeated result sections', () => {
    expect(normalizeNodeOutput('{"rows":[1,2]}').content).toBe('{"rows":[1,2]}')
    expect(dedupeOutputSections([
      { kind: 'tool', content: 'same result' },
      { kind: 'final', content: ' same   result ' },
      { kind: 'final', content: 'different' },
    ])).toHaveLength(2)
  })

  it('keeps estimated input fixed while output grows until real usage arrives', () => {
    const first = nodeTokens({ streamContent: '开始' }, '固定提示词')
    const next = nodeTokens({ streamContent: '开始并继续生成更多内容' }, '固定提示词')
    expect(next.input).toBe(first.input)
    expect(next.output).toBeGreaterThan(first.output)
    expect(nodeTokens({ streamContent: 'ignored', usage: { input: 20, output: 30 } }, '固定提示词'))
      .toMatchObject({ input: 20, output: 30, inputExact: true, outputExact: true })
  })

  it('shows only the latest completed output and ignores intermediate progress', () => {
    const state = {
      status: 'running',
      streamContent: '不应显示的流式内容',
      logs: [
        { type: 'info', content: '{"content":"🔄 正在进行推理（迭代 1）..."}' },
        { type: 'tool_call', content: '{"status":"running","output":"中间工具结果"}' },
        { type: 'info', content: '{"status":"completed","output":"第一次完成结果"}' },
        { type: 'info', data: { status: 'completed', output: { content: '最终完成结果' } } },
      ],
    }
    expect(extractCompletedNodeOutput(state)).toBe('最终完成结果')
    expect(extractCompletedNodeOutput({ status: 'running', streamContent: '流式过程', logs: state.logs.slice(0, 2) })).toBe('')
    expect(extractCompletedNodeOutput({ status: 'completed', output: '{"content":"节点最终输出"}' })).toBe('节点最终输出')
  })
})
