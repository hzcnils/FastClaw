/**
 * plannerAdapter.js — 规划展示适配层
 *
 * ★ 职责：把"从日志正则解析"的逻辑全部收敛到这里。
 *   - 组件只消费本模块输出的规范化结构，绝不直接碰原始日志；
 *   - 取值优先级：结构化字段 > 正则解析 > null（拿不到就不渲染，不占位、不编造）；
 *   - 所有正则 try/catch 包裹，解析失败静默降级，不影响整块渲染；
 *   - 每个正则上方留 TODO 注明对应的后端结构化字段，后端上线后可直接删除该分支。
 *
 * 后端正在并行改造（planner_step / adopted_ids / plan_summary / assets.description 等），
 * 结构化字段一到，前端只改本文件一处。
 */

// ============================================================================
// 可调常量
// ============================================================================

/** 问题2：置信度低于此阈值(0.6 = 60%)的意图行标黄 + 提示 */
export const LOW_CONFIDENCE_THRESHOLD = 0.6

/** 问题7：分数 ≥ 此值视为"高分资产" */
export const HIGH_SCORE_THRESHOLD = 0.7

/** 问题7：高分资产中未采用占比 ≥ 此值时，提示"检索结果与本次任务相关性可能不足" */
export const HIGH_SCORE_UNUSED_RATIO = 0.6

/** 问题7：挂在命中资产行上的相关性提示文案（可调常量） */
export const CORRELATION_LOW_HINT = '检索结果与本次任务相关性可能不足'

/** 问题1：实体超过 3 个折叠为「等 N 项」 */
export const ENTITY_COLLAPSE_LIMIT = 3

/** 问题8：规划过程人话行的图标 */
export const PROCESS_LINE_ICONS = { ok: '✓', warn: '!', info: '·' }

// ============================================================================
// 内部工具
// ============================================================================

function norm(s) {
  return String(s || '').toLowerCase().replace(/[-_\s]/g, '')
}

function safeNumber(v) {
  const n = Number(v)
  return Number.isFinite(n) ? n : null
}

function findStage(validationFlow, stage) {
  const flow = Array.isArray(validationFlow) ? validationFlow : []
  return flow.find(r => r && r.stage === stage) || null
}

function contentOf(log) {
  if (!log) return ''
  if (typeof log === 'string') return log.trim()
  return String(log.content || log.text || log.message || '').trim()
}

// ============================================================================
// 意图行（问题 1/2）
// ============================================================================

/**
 * 归一化意图行。
 * 返回 { kind:'intent', status, name, confidence, ruleFallback, entities, durationMs, lowConfidence }
 * 拿不到 name/confidence/entities 时对应字段为 null/[]，组件按"不渲染该项"处理。
 */
export function parseIntent(intentRow, planningLogs) {
  if (!intentRow) return null

  const out = {
    kind: 'intent',
    status: intentRow.status || 'done',
    name: null,
    confidence: null,
    ruleFallback: false,
    entities: [],
    durationMs: safeNumber(intentRow.duration_ms),
    lowConfidence: false,
  }

  // ── 结构化字段优先：后端 intent plan_stage 已带 confidence / model / rule_fallback ──
  if (typeof intentRow.confidence === 'number') out.confidence = intentRow.confidence
  out.ruleFallback = !!intentRow.rule_fallback

  // TODO: 后端 intent plan_stage 将新增 label / entities 结构化字段，
  //       届时删除下面两个"从 NLU 日志正则解析"的分支。
  if (typeof intentRow.label === 'string' && intentRow.label) {
    out.name = intentRow.label
  } else {
    const fromLog = extractIntentFromLogs(planningLogs)
    if (fromLog) {
      out.name = fromLog.name
      if (out.confidence === null) out.confidence = fromLog.confidence
    }
  }
  if (Array.isArray(intentRow.entities) && intentRow.entities.length) {
    out.entities = intentRow.entities
      .filter(e => e && e.text)
      .map(e => ({ type: e.type || '实体', text: String(e.text) }))
  } else {
    out.entities = extractEntitiesFromLogs(planningLogs)
  }

  // 问题2：置信度 < 60% 或走了规则兜底 → 标黄 + 提示；否则图标回绿色 ✓
  out.lowConfidence =
    (out.confidence !== null && out.confidence < LOW_CONFIDENCE_THRESHOLD)
    || out.ruleFallback

  return out
}

/** 从 planningLogs 里的 NLU 消息提取意图名 + 置信度。 */
function extractIntentFromLogs(planningLogs) {
  const nlu = findNluLog(planningLogs)
  if (!nlu) return null
  try {
    // 形如：  意图: 信息查询 (置信度 30%)
    // TODO: 后端 intent plan_stage 新增 label 字段后删除
    const m = nlu.match(/^\s*意图:\s*(.+?)\s*\(置信度\s*([\d.]+)%\)/m)
    if (!m) return null
    return { name: m[1].trim(), confidence: Number(m[2]) / 100 }
  } catch {
    return null
  }
}

/** 从 planningLogs 里的 NLU 消息提取实体列表 [{ type, text }]。 */
function extractEntitiesFromLogs(planningLogs) {
  const nlu = findNluLog(planningLogs)
  if (!nlu) return []
  const entities = []
  try {
    // 形如：    • [时间] "今天"   /    • [地点] "昆明" → 昆明
    // TODO: 后端 intent plan_stage 新增 entities 字段后删除
    const re = /^\s*•\s*\[([^\]]+)\]\s*"([^"]*)"(?:\s*→\s*([^\n]+))?/gm
    let m
    while ((m = re.exec(nlu)) !== null) {
      if (m[2]) entities.push({ type: m[1].trim(), text: m[2].trim() })
    }
  } catch {
    // 静默降级
  }
  return entities
}

function findNluLog(planningLogs) {
  for (const log of planningLogs || []) {
    const c = contentOf(log)
    if (c.startsWith('✅ 意图分析完成')) return c
  }
  return null
}

/** 问题1：实体 → 展示文案。「时间「今天」、地点「昆明」」；>3 折叠为「等 N 项」。 */
export function formatEntities(entities) {
  const list = Array.isArray(entities) ? entities.filter(e => e && e.text) : []
  if (!list.length) return ''
  const shown = list.slice(0, ENTITY_COLLAPSE_LIMIT)
  const parts = shown.map(e => `${e.type || '实体'}「${e.text}」`)
  return list.length > shown.length
    ? `${parts.join('、')} 等 ${list.length} 项`
    : parts.join('、')
}

// ============================================================================
// 命中资产（问题 4/5/6/7）
// ============================================================================

/**
 * 归一化命中资产。tools 阶段（只有名字列表）与 assets 阶段（含明细）合并为一行。
 * 返回 { kind:'assets', status, total, adopted, assets[], durationMs, correlationLow }
 * 无任何资产数据时返回 null（不渲染）。
 */
export function parseAssets(assetsRow, toolsRow, retrievalRow, plannedTree) {
  const list = []
  let total = 0
  let adoptedCount = 0

  // 耗时取"检索步骤自身"：tools/retrieval 阶段（retrieval_ms），不是 assets 阶段（plan_ms）
  // TODO: 后端合并 tools/assets 为单一 assets 事件后，直接读该事件的 duration_ms
  const durationMs =
    safeNumber(toolsRow?.duration_ms) ??
    safeNumber(retrievalRow?.duration_ms) ??
    safeNumber(assetsRow?.duration_ms)

  // TODO: 后端 assets 将新增 adopted_ids 字段，届时用它反查 adopted，
  //       删除 plannedTree 反推分支。
  const treeAdopted = adoptedNamesFromTree(plannedTree)

  if (assetsRow && Array.isArray(assetsRow.assets) && assetsRow.assets.length) {
    total = safeNumber(assetsRow.total) ?? assetsRow.assets.length
    for (const a of assetsRow.assets) {
      const adopted = a.adopted !== undefined ? !!a.adopted : treeAdopted.has(norm(a.name))
      if (adopted) adoptedCount++
      const score = safeNumber(a.score)
      list.push({
        name: a.name || '',
        type: a.type || '',
        score,
        adopted,
        reason: a.reason || '',
        // 问题6：资产描述必须用资产自身 description；后端暂不给就 null，不渲染、不拿别处文案顶上
        // TODO: 后端 assets 将新增 description 字段
        description: (typeof a.description === 'string' && a.description) ? a.description : null,
        highScore: score !== null && score >= HIGH_SCORE_THRESHOLD,
      })
    }
  } else if (toolsRow && Array.isArray(toolsRow.tools) && toolsRow.tools.length) {
    // 只有 tools 阶段（旧后端）：按名字构造，adopted 用 plannedTree 反推
    total = toolsRow.tools.length
    for (const name of toolsRow.tools) {
      const adopted = treeAdopted.has(norm(name))
      if (adopted) adoptedCount++
      list.push({
        name, type: '', score: null, adopted,
        reason: '', description: null, highScore: false,
      })
    }
  }

  if (!list.length) return null

  return {
    kind: 'assets',
    status: assetsRow?.status || toolsRow?.status || 'done',
    total,
    adopted: adoptedCount,
    assets: list,
    durationMs,
    // 问题7：反向判断——高分资产全部/大部分未采用 → 检索结果相关性可能不足
    correlationLow: computeCorrelationLow(list),
  }
}

/** 问题7：高分未采用占比超过阈值 → 提示检索相关性不足（阈值/文案为可调常量）。 */
function computeCorrelationLow(assets) {
  const high = assets.filter(a => a.highScore)
  if (!high.length) return false
  const unusedHigh = high.filter(a => !a.adopted).length
  return (unusedHigh / high.length) >= HIGH_SCORE_UNUSED_RATIO
}

/** 从 plannedTree 各节点反推已采用工具名集合（标准化去重）。 */
function adoptedNamesFromTree(tree) {
  const set = new Set()
  const walk = (n) => {
    if (!n) return
    const nt = n.node_type
    if (nt) {
      if (nt.type === 'ToolTask' && nt.config?.tool_name) {
        set.add(norm(nt.config.tool_name))
      } else if (nt.type === 'AgentTask') {
        const tools = nt.config?.assigned_tools || nt.assigned_tools || []
        for (const t of tools) set.add(norm(t))
      } else if (nt.type === 'LlmTask') {
        const sk = nt.config?.skill || nt.config?.skill_name
        if (sk) set.add(norm(sk))
      }
    }
    for (const c of (n.children || [])) walk(c)
  }
  if (tree?.root) walk(tree.root)
  return set
}

// ============================================================================
// 校验流水行（兼容当前模板：输出 { stage, status, text, duration_ms, ... }）
// ============================================================================

/**
 * 兼容层：把 validationFlow 压平成当前模板可渲染的行（行为与改造前一致）。
 * 问题 1~7 会逐步替换成上面 parseIntent / parseAssets 的 kind 结构，
 * 本函数用于过渡期（后端字段未就绪时保持既有展示）。
 */
export function parseFlowRows(validationFlow, isPlanning = false, plannedTree = null) {
  const flow = Array.isArray(validationFlow) ? validationFlow : []
  const rows = []
  let assetsRow = null
  let toolsRow = null
  let retrievalRow = null
  let assetsInsertAt = -1   // tools/assets 第一次出现的位置（合并后的资产行插回原处）

  for (let i = 0; i < flow.length; i++) {
    const r = flow[i]
    if (!r || !r.stage) continue

    if (r.stage === 'assets' || r.stage === 'tools') {
      // 问题4：tools/assets 两行合并为一行，这里先收集，循环后统一插入
      if (r.stage === 'assets') assetsRow = r
      else toolsRow = r
      if (assetsInsertAt === -1) assetsInsertAt = rows.length
      continue
    }

    if (r.stage === 'retrieval') {
      retrievalRow = r
      // 问题3：固定文案「探到 N 条线索，确认需要多轮检索直至覆盖达标」是编造的，丢弃。
      // 仅当有真实知识块（KB 命中）或仍处于 running 骨架时才渲染该行。
      // TODO: 后端 retrieval 阶段改为直接下发真实文案后，删除此分支
      const knowledge = Array.isArray(r.knowledge) ? r.knowledge : null
      if (r.status === 'done' && (!knowledge || !knowledge.length)) continue
      const text = knowledge && knowledge.length
        ? `命中知识块 ${knowledge.length} 个：${knowledge.join('、')}`
        : String(r.text || '')
      rows.push({ ...r, key: 'retrieval', text: text.trim() })
      continue
    }

    if (r.stage === 'plan') {
      // 去掉「其中 N 个循环节点」尾巴；循环信息单独成行（下方），不重复
      // TODO: 后端 plan 文本去掉循环尾巴后，删除此正则
      const text = String(r.text || '')
        .replace(/，其中\s*\d+\s*个循环节点\s*$/, '')
        .trim()
      rows.push({ ...r, key: 'plan', text })
      // 循环行：仅当计划里确实存在循环节点才渲染（NodeType::Loop / plan.loops），
      // 文案由实际数据生成；没有循环节点时整行不渲染，不写「0 个循环节点」。
      const loops = r.loops ?? countLoopsInTree(plannedTree) ?? 0
      if (loops > 0) {
        rows.push({ key: 'loop', stage: 'loop', status: r.status || 'done', text: `↻ 含 ${loops} 个循环节点`, duration_ms: null })
      }
      continue
    }

    rows.push({ ...r, key: r.stage, text: String(r.text || '').trim() })
  }

  // 问题4/5：tools + assets 合并为一行（含明细、adopted、相关性判断），插回原位置
  const assets = parseAssets(assetsRow, toolsRow, retrievalRow, plannedTree)
  if (assets) {
    const at = assetsInsertAt >= 0 ? assetsInsertAt : rows.length
    rows.splice(at, 0, assets)
  }

  if (isPlanning && !rows.some(r => r.stage === 'intent')) {
    rows.unshift({ key: 'intent', stage: 'intent', status: 'running', text: '正在理解用户意图…', duration_ms: null })
  }
  return rows
}

// ============================================================================
// 4 步思考（问题 8/9 共用）
// ============================================================================

/** 规划期从流式 planningLogs 提取 [需求理解] 等前缀正文 → { [idx]: content } */
export function extractLiveSteps(logs) {
  const found = {}
  for (const log of logs || []) {
    const c = contentOf(log)
    for (let i = 0; i < STEP_LABELS.length; i++) {
      const label = STEP_LABELS[i]
      if (c.startsWith(`[${label}]`)) found[i] = c.slice(label.length + 2).trim()
    }
  }
  return found
}

/** 去掉正文自带的 [label] 前缀（模板已渲染 label）。 */
export function stripStepLabel(content, label) {
  try {
    return String(content || '').replace(new RegExp(`^\\[${label}\\]`), '').trim()
  } catch {
    return content
  }
}

export const STEP_LABELS = ['需求理解', '数据检索', '模型分析', '系统步骤']

// ============================================================================
// 循环节点 / 底部状态条摘要
// ============================================================================

/** 递归统计计划树里的 Loop 节点（NodeType::Loop）。无树时返回 null。 */
export function countLoopsInTree(tree) {
  if (!tree?.root?.children) return null
  let n = 0
  const walk = (node) => {
    if (!node) return
    if (node.node_type?.type === 'Loop') n++
    for (const c of (node.children || [])) walk(c)
  }
  for (const phase of tree.root.children) walk(phase)
  return n
}

/** 底部状态条右侧灰字阶段摘要：N 个阶段 · M 个任务 · 含 K 个循环（K=0 隐藏）。 */
export function buildSummaryLine(validationFlow, planTree, planPhases, planTasks) {
  const planRow = findStage(validationFlow, 'plan')
  const p = planRow?.phases ?? planPhases ?? 0
  const t = planRow?.tasks ?? planTasks ?? 0
  const loops = countLoopsInTree(planTree) ?? planRow?.loops ?? 0
  const parts = []
  if (p) parts.push(`${p} 个阶段`)
  if (t) parts.push(`${t} 个任务`)
  if (loops) parts.push(`含 ${loops} 个循环`)
  return parts.join(' · ')
}

// ============================================================================
// 规划过程人话行（问题 8）
// ============================================================================

/**
 * 规划过程折叠块 → 若干行人话，每行可再展开看整理过的细节。
 * 返回 [ { icon, title, detail } ]；映射不出的行直接丢弃，不兜底显示原文。
 * 绝不出现：Prompt 长度 / 响应字符数 / 推理耗时 / rule_fallback / DAG 逐行明细 / 请求原文。
 */
export function parseProcessLines(validationFlow, planningLogs, plannedTree) {
  const lines = []
  const intentRow = findStage(validationFlow, 'intent')
  const assetsRow = findStage(validationFlow, 'assets')
  const toolsRow = findStage(validationFlow, 'tools')
  const retrievalRow = findStage(validationFlow, 'retrieval')
  const planRow = findStage(validationFlow, 'plan')

  // 1) 已理解你的需求
  if (intentRow?.status === 'done') {
    const intent = parseIntent(intentRow, planningLogs)
    const detail = []
    if (intent?.name) detail.push(`意图：${intent.name}`)
    if (intent?.confidence !== null && intent.confidence !== undefined) {
      detail.push(`置信度 ${Math.round(intent.confidence * 100)}%`)
    }
    const ents = formatEntities(intent?.entities)
    if (ents) detail.push(`识别到 ${ents}`)
    lines.push({ icon: PROCESS_LINE_ICONS.ok, title: '已理解你的需求', detail: detail.join(' · ') || null })
  }

  // 2) 已检索 N 个可用资产
  const assets = parseAssets(assetsRow, toolsRow, retrievalRow, plannedTree)
  if (assets?.status === 'done') {
    const detail = assets.assets.map(a => `${a.name}${a.adopted ? ' ✓' : ''}`).join('、') || null
    lines.push({ icon: PROCESS_LINE_ICONS.ok, title: `已检索 ${assets.total} 个可用资产`, detail })
  }

  // 3) 已生成 N 阶段 · N 任务
  if (planRow?.status === 'done') {
    const phases = planRow.phases ?? 0
    const tasks = planRow.tasks ?? 0
    lines.push({ icon: PROCESS_LINE_ICONS.ok, title: `已生成 ${phases} 个阶段 · ${tasks} 个任务`, detail: null })
  }

  // TODO: 后端新增 planner_step 结构化事件后，直接消费该事件；
  //       以上为从现有结构化事件映射的降级实现。
  return lines
}

// ============================================================================
// 分析条目人话（问题 9）
// ============================================================================

/**
 * 规划分析自然语言段落。
 * 优先级：presentation.plan_summary（后端新字段）> 合并 4 步思考正文（去 [xx] 前缀）。
 * 都拿不到返回 null（不渲染）。
 */
export function parseAnalysisSummary(presentation) {
  // TODO: 后端 plan_generated 将新增 plan_summary 字段，届时优先使用
  const ps = presentation?.plan_summary
  if (typeof ps === 'string' && ps.trim()) return ps.trim()

  const tp = presentation?.thinking_process
  if (tp) {
    const parts = []
    for (const key of ['requirement', 'data_search', 'model_analysis', 'system_steps']) {
      const text = String(tp[key] || '').replace(/^\[[^\]]*\]/, '').trim()
      if (text) parts.push(text)
    }
    return parts.length ? parts.join(' ') : null
  }
  return null
}
