import { describe, expect, it } from 'vitest'
import {
  buildDependencySummary,
  buildPlanningChecklist,
  resolveChecklistMode,
} from './planningChecklistAdapter'

const tree = {
  root: { children: [
    { id: 'phase-1', name: '准备', children: [
      { id: 'a', name: '获取数据', dependencies: [], node_type: { type: 'ToolTask', config: { tool_name: 'excel' } } },
      { id: 'b', name: '汇总分析', dependencies: ['a'], node_type: { type: 'AgentTask', config: { assigned_tools: ['analysis'] } } },
      { id: 'c', name: '趋势图表', dependencies: ['a'], node_type: { type: 'ToolTask', config: { tool_name: 'chart' } } },
    ] },
    { id: 'phase-2', name: '交付', children: [
      { id: 'd', name: '生成 Word', dependencies: ['b', 'c'], node_type: { type: 'ToolTask', config: { tool_name: 'docx' } } },
    ] },
  ] },
}

describe('planning checklist adapter', () => {
  it('shows one immediate placeholder and replaces it when formal planning rows arrive', () => {
    const initial = buildPlanningChecklist({ isPlanning: true, planStartedAt: 1000 })
    expect(initial.rows).toEqual([expect.objectContaining({ id: 'planning:initial', title: 'FastPlan规划中...', status: 'running' })])

    const formal = buildPlanningChecklist({ isPlanning: true, validationFlow: [
      { stage: 'request', status: 'done', text: '规划请求已就绪', duration_ms: 0 },
      { stage: 'llm_progress', status: 'running', text: '正在编排执行步骤…', elapsed_ms: 0 },
    ] })
    expect(formal.rows.some(row => row.id === 'planning:initial')).toBe(false)
    expect(formal.rows.find(row => row.id === 'planning:request')?.durationMs).toBeNull()
    expect(formal.rows.find(row => row.id === 'planning:llm_progress')?.durationMs).toBeNull()
  })

  it('summarizes parallel dependencies without exposing DAG internals', () => {
    expect(buildDependencySummary(tree)).toBe('获取数据 → （汇总分析、趋势图表） → 生成 Word')
  })

  it('derives runtime modes without treating a restored completed tree as running', () => {
    expect(resolveChecklistMode({ planTree: tree, isPreviewing: true })).toBe('awaiting_confirmation')
    expect(resolveChecklistMode({ planTree: tree, isPlanning: true, executionState: { a: { status: 'running' } } })).toBe('executing')
    expect(resolveChecklistMode({ planTree: tree, isPlanning: true, treeCompleted: true })).toBe('completed')
    expect(resolveChecklistMode({ planTree: tree, isPaused: true, humanRequest: { node_id: 'b' } })).toBe('waiting_human')
  })

  it('builds compact live task rows with stable status, progress and timing', () => {
    const result = buildPlanningChecklist({
      planTree: tree,
      isPlanning: true,
      executionState: {
        a: { status: 'completed', durationMs: 600, logs: [{ type: 'tool_call', toolName: 'excel' }] },
        b: { status: 'running', startTime: 1000, logs: [{ type: 'tool_call', toolName: 'analysis' }] },
        c: { status: 'pending' },
        d: { status: 'pending' },
      },
    })
    expect(result.mode).toBe('executing')
    expect(result.footer).toBe('执行中：1/4 个任务完成')
    expect(result.taskRows[0]).toMatchObject({ id: 'task:a', status: 'completed', durationMs: 600, detail: 'excel 已完成' })
    expect(result.taskRows[1]).toMatchObject({ id: 'task:b', status: 'running', detail: 'analysis 正在执行…' })
    expect(result.taskRows[3].detail).toBe('等待 汇总分析、趋势图表 完成')
  })

  it('represents pause, failure, skip and human confirmation in the same flat row model', () => {
    const result = buildPlanningChecklist({
      planTree: tree,
      isPaused: true,
      humanRequest: { node_id: 'b', question: '是否继续生成报告？' },
      executionState: {
        a: { status: 'completed' }, b: { status: 'running' },
        c: { status: 'failed', error: '图表服务不可用' }, d: { status: 'skipped' },
      },
    })
    expect(result.taskRows.map(row => row.status)).toEqual(['completed', 'waiting_human', 'failed', 'skipped'])
    expect(result.taskRows[1].detail).toBe('是否继续生成报告？')
    expect(result.taskRows[2].detail).toBe('图表服务不可用')
  })

  it('normalizes stale running and pending nodes in completed history snapshots', () => {
    const result = buildPlanningChecklist({
      planTree: tree, treeCompleted: true,
      executionState: { a: { status: 'completed' }, b: { status: 'running' }, c: { status: 'pending' }, d: { status: 'failed' } },
    })
    expect(result.mode).toBe('completed')
    expect(result.taskRows.map(row => row.status)).toEqual(['completed', 'completed', 'skipped', 'failed'])
    expect(result.taskRows.some(row => row.status === 'running' || row.status === 'pending')).toBe(false)
  })
})
