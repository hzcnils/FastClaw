import * as plannerAdapter from './plannerAdapter'
import {
  cleanLine,
  collectExecutionNames,
  normalizeNodeOutput,
  resolveNodeMetrics,
} from './blockParser'

const DONE = new Set(['done', 'completed', 'success', 'succeeded'])

function normalizeStatus(status) {
  const value = String(status || 'pending').toLowerCase()
  if (DONE.has(value)) return 'completed'
  if (['running', 'planning', 'executing', 'in_progress'].includes(value)) return 'running'
  if (['failed', 'error'].includes(value)) return 'failed'
  if (value === 'skipped') return 'skipped'
  if (value === 'paused') return 'paused'
  if (value === 'waiting_human') return 'waiting_human'
  return 'pending'
}

function finiteNumber(...values) {
  for (const value of values) {
    if (value === null || value === undefined || value === '') continue
    const number = Number(value)
    if (Number.isFinite(number)) return number
  }
  return null
}

function positiveDuration(...values) {
  const value = finiteNumber(...values)
  return value !== null && value > 0 ? value : null
}

function text(value) {
  return typeof value === 'string' ? value.trim() : ''
}

export function flattenPlanTasks(planTree) {
  const rows = []
  for (const [phaseIndex, phase] of (planTree?.root?.children || []).entries()) {
    for (const [taskIndex, task] of (phase.children || []).entries()) {
      rows.push({ task, phase, phaseIndex, taskIndex })
    }
  }
  return rows
}

function dependencyIds(task) {
  return (task?.dependencies || task?.depends_on || [])
    .map(item => typeof item === 'string' ? item : item?.id || item?.node_id)
    .filter(Boolean)
}

export function buildDependencySummary(planTree) {
  const entries = flattenPlanTasks(planTree)
  if (!entries.length) return ''
  const byId = new Map(entries.map(({ task }) => [task.id, task]))
  const remaining = new Map(entries.map(({ task }) => [task.id, new Set(dependencyIds(task).filter(id => byId.has(id)))]))
  const layers = []
  const emitted = new Set()

  while (emitted.size < entries.length) {
    const layer = entries
      .map(({ task }) => task)
      .filter(task => !emitted.has(task.id) && [...(remaining.get(task.id) || [])].every(id => emitted.has(id)))
    if (!layer.length) break
    layers.push(layer)
    layer.forEach(task => emitted.add(task.id))
  }

  // Cyclic or incomplete historical data falls back to the stable plan order.
  if (emitted.size < entries.length) {
    layers.push(entries.map(({ task }) => task).filter(task => !emitted.has(task.id)))
  }

  const label = layer => {
    const names = layer.map(task => text(task.name) || task.id).filter(Boolean)
    const visible = names.slice(0, 3)
    const suffix = names.length > visible.length ? `等 ${names.length} 项` : ''
    const joined = [...visible, suffix].filter(Boolean).join('、')
    return names.length > 1 ? `（${joined}）` : joined
  }
  return layers.slice(0, 4).map(label).filter(Boolean).join(' → ')
}

function taskTools(task, nodeState) {
  return collectExecutionNames(nodeState?.logs || [], task)
}

function latestBusinessProgress(nodeState, tools) {
  const logs = Array.isArray(nodeState?.logs) ? nodeState.logs : []
  for (let index = logs.length - 1; index >= 0; index--) {
    const log = logs[index] || {}
    if (log.type === 'tool_call' && (log.toolName || tools[0])) return `${log.toolName || tools[0]} 正在执行…`
    if (['reasoning', 'info', 'result', 'tool_result'].includes(log.type)) {
      const value = cleanLine(log.content || '', 100)
      if (value) return value
    }
  }
  if (tools.length) return `${tools.join('、')} 正在执行…`
  return '正在处理任务…'
}

function completedDetail(nodeState, tools) {
  if (tools.length) return `${tools.join('、')} 已完成`
  const output = normalizeNodeOutput(nodeState?.output).content
  return cleanLine(output, 100) || '任务输出已就绪'
}

function pendingDetail(task, taskNameById, tools) {
  const dependencies = dependencyIds(task).map(id => taskNameById.get(id)).filter(Boolean)
  if (dependencies.length) return `等待 ${dependencies.join('、')} 完成`
  if (tools.length) return `将使用 ${tools.join('、')}`
  return ''
}

function buildTaskRows(planTree, executionState, humanRequest, isPaused, treeCompleted) {
  const entries = flattenPlanTasks(planTree)
  const taskNameById = new Map(entries.map(({ task }) => [task.id, text(task.name) || task.id]))
  return entries.map(({ task, phase }) => {
    const nodeState = executionState?.[task.id] || { status: 'pending' }
    let status = normalizeStatus(nodeState.status)
    if (treeCompleted && status === 'running') status = 'completed'
    else if (treeCompleted && status === 'pending') status = 'skipped'
    if (humanRequest?.node_id === task.id) status = 'waiting_human'
    else if (isPaused && status === 'running') status = 'paused'
    const tools = taskTools(task, nodeState)
    const metrics = resolveNodeMetrics(nodeState)
    let detail = ''
    if (status === 'running') detail = latestBusinessProgress(nodeState, tools)
    else if (status === 'completed') detail = completedDetail(nodeState, tools)
    else if (status === 'failed') detail = cleanLine(nodeState.error || '任务执行失败', 100)
    else if (status === 'skipped') detail = '任务已跳过'
    else if (status === 'paused') detail = '执行已暂停'
    else if (status === 'waiting_human') detail = cleanLine(humanRequest?.question || humanRequest?.prompt || humanRequest?.message || '等待人工确认', 100)
    else detail = pendingDetail(task, taskNameById, tools)
    return {
      id: `task:${task.id}`,
      kind: 'task',
      status,
      title: text(task.name) || task.id,
      detail,
      durationMs: metrics.durationMs,
      startedAt: finiteNumber(nodeState.startTime, nodeState.started_at),
      phase: text(phase?.name),
    }
  })
}

export function resolveChecklistMode({ planTree, executionState = {}, isPlanning, isPreviewing, isPaused, treeCompleted, humanRequest }) {
  const statuses = Object.values(executionState || {}).map(node => normalizeStatus(node?.status))
  if (treeCompleted) return 'completed'
  if (humanRequest) return 'waiting_human'
  if (isPaused) return 'paused'
  if (statuses.includes('failed') && !statuses.includes('running')) return 'failed'
  if (planTree && !isPreviewing && (isPlanning || statuses.some(status => status !== 'pending'))) return 'executing'
  if (isPreviewing) return 'awaiting_confirmation'
  if (isPlanning) return 'planning'
  return planTree ? 'ready' : 'idle'
}

function planningRows(options) {
  const { validationFlow = [], logs = [], planTree, isPlanning, planDurationMs, planStartedAt, presentation } = options
  const hasFormalContent = !!planTree || !!presentation || validationFlow.length > 0 || logs.some(log => text(log?.content || log?.text || log?.message))
  if (isPlanning && !hasFormalContent) {
    return [{ id: 'planning:initial', kind: 'initial', status: 'running', title: 'FastPlan规划中...', detail: '', durationMs: null, startedAt: planStartedAt || null }]
  }
  const rows = []
  for (const row of plannerAdapter.parseFlowRows(validationFlow, isPlanning && !planTree, planTree)) {
    if (row.stage === 'intent') {
      const intent = plannerAdapter.parseIntent(row, logs)
      rows.push({
        id: 'planning:intent', kind: 'intent', status: normalizeStatus(row.status || (planTree ? 'completed' : 'running')),
        title: intent?.name ? `已理解任务目标：${intent.name}` : text(row.text) || '正在理解任务目标',
        detail: intent ? plannerAdapter.formatEntities(intent.entities) : '',
        durationMs: positiveDuration(intent?.durationMs, row.duration_ms, row.elapsed_ms),
      })
      continue
    }
    if (row.kind === 'assets') {
      const adopted = (row.assets || []).filter(asset => asset.adopted)
      rows.push({
        id: 'planning:capabilities', kind: 'capability', status: normalizeStatus(row.status || (planTree ? 'completed' : 'running')),
        title: row.total === row.adopted
          ? `${row.total} 项能力需求已有可用工具覆盖`
          : `已匹配 ${row.adopted || 0}/${row.total || 0} 项可用能力`,
        detail: adopted.map(asset => asset.name).filter(Boolean).join('、'),
        durationMs: positiveDuration(row.durationMs, row.duration_ms, row.elapsed_ms),
      })
      continue
    }
    if (!['plan', 'dag', 'validation', 'complete'].includes(row.stage)) {
      rows.push({
        id: `planning:${row.stage || rows.length}`, kind: 'validation', status: normalizeStatus(row.status || (planTree ? 'completed' : 'running')),
        title: text(row.text) || text(row.label) || '规划校验完成', detail: '', durationMs: positiveDuration(row.duration_ms, row.elapsed_ms),
      })
    }
  }

  const tasks = flattenPlanTasks(planTree)
  if (planTree) {
    const phases = planTree?.root?.children?.length || 0
    const planEvent = [...validationFlow].reverse().find(row => ['plan', 'dag', 'validation', 'complete'].includes(row?.stage))
    rows.push({
      id: 'planning:plan', kind: 'plan', status: 'completed',
      title: `规划完成：${phases} 个阶段 · ${tasks.length} 个任务，依赖关系已校验`,
      detail: buildDependencySummary(planTree),
      durationMs: positiveDuration(planEvent?.duration_ms, planDurationMs),
    })
  } else if (isPlanning) {
    rows.push({ id: 'planning:plan', kind: 'plan', status: 'running', title: '正在生成并校验执行计划', detail: '', durationMs: null, startedAt: planStartedAt || null })
  }
  return rows
}

export function buildPlanningChecklist(options = {}) {
  const mode = resolveChecklistMode(options)
  const planRows = planningRows(options)
  const showTasks = ['executing', 'paused', 'waiting_human', 'failed', 'completed'].includes(mode)
  const taskRows = showTasks
    ? buildTaskRows(options.planTree, options.executionState || {}, options.humanRequest, options.isPaused, options.treeCompleted)
    : []
  const completed = taskRows.filter(row => row.status === 'completed').length
  const total = flattenPlanTasks(options.planTree).length
  const footer = mode === 'completed' ? `执行完成：${completed}/${total} 个任务`
    : mode === 'executing' ? `执行中：${completed}/${total} 个任务完成`
      : mode === 'paused' ? `执行已暂停：${completed}/${total} 个任务完成`
        : mode === 'waiting_human' ? `等待人工确认：${completed}/${total} 个任务完成`
          : mode === 'failed' ? `执行遇到错误：${completed}/${total} 个任务完成`
            : mode === 'awaiting_confirmation' ? '规划已就绪，等待确认'
              : mode === 'planning' ? '正在规划任务' : '规划已就绪'
  return { mode, rows: [...planRows, ...taskRows], planRows, taskRows, completed, total, footer }
}
