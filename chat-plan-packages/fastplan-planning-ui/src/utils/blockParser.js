/**
 * 块协议解析器 (Block Protocol)
 * ---------------------------------------------------------------------------
 * 把后端已有的 executionState[nodeId].logs 转换成 v21 的块模型。
 * 不需要后端改协议即可工作；若后端补充了 usage / results 字段会自动使用。
 *
 * logs 现有结构（stores/websocket.js: agent_task_progress）：
 *   { type: 'reasoning' | 'tool_call' | 'result' | 'error', toolName?, content }
 *
 * 输出块结构：
 *   { kind: 'search', calls: [{ tool, source, kw, results, done, ms }] }
 *   { kind: 'tool',   name, args, result, status }
 *   { kind: 'loop',   round, verdict, tone }
 *   { kind: 'line',   text, tone }
 */

/* 判定为"检索类"工具 → 渲染成行式搜索块 */
const SEARCH_TOOLS =
  /^(web_search|web_fetch|websearch|webfetch|tavily|serper|bing|google|baidu|search|kb_search|knowledge_search|vector_search|rag_search|doc_search)/i

/* 来源猜测：只认工具名与显式字段，绝不拿自由文本关键词去猜
   （否则用户搜「RAG 评测」会被里面的 rag 误判成内部知识库） */
const SOURCE_HINTS = [
  [/baidu|百度/i, '百度'],
  [/google/i, 'Google'],
  [/bing/i, 'Bing'],
  [/tavily/i, 'Tavily'],
  [/serper/i, 'Serper'],
  [/arxiv/i, 'arXiv'],
  [/cnki|知网/i, '知网'],
  [/^(kb_|knowledge|vector|doc_)|内部/i, '内部知识库'],
]

export function isSearchTool(name = '') {
  return SEARCH_TOOLS.test(String(name).trim())
}

function guessSource(toolName = '', content = '') {
  // ① 显式字段优先：{"source":"百度"} / {"engine":"bing"} / {"provider":"tavily"}
  let explicit = ''
  try {
    const j = JSON.parse(String(content).trim())
    explicit = j.source || j.engine || j.provider || j.site || j.channel || ''
  } catch { /* 非 JSON */ }
  if (explicit) {
    for (const [re, label] of SOURCE_HINTS) if (re.test(explicit)) return label
    return String(explicit)
  }
  // ② 再看工具名本身（不看关键词，避免 "RAG 评测" 被误判成知识库）
  for (const [re, label] of SOURCE_HINTS) if (re.test(toolName)) return label
  // ③ 兜底
  if (/^(kb_|knowledge|vector|doc_|rag_)/i.test(toolName)) return '内部知识库'
  return isSearchTool(toolName) ? '互联网' : '工具'
}

/* 从 tool_call 的 content 中提取"关键词/入参" */
export function extractArgs(content = '') {
  const raw = String(content).trim()
  if (!raw) return ''
  // JSON 入参 → 取 query / keyword / q / input
  try {
    const j = JSON.parse(raw)
    const nested = j.tool_args ?? j.arguments ?? j.args ?? j.parameters ?? j.params
    const values = nested && typeof nested === 'object' && !Array.isArray(nested) ? nested : j
    const v = values.query ?? values.keyword ?? values.keywords ?? values.q ?? values.input ?? values.text ?? values.prompt
    if (v !== undefined && v !== null && v !== '') return typeof v === 'string' ? v : JSON.stringify(v)
    return Object.values(values)
      .slice(0, 3)
      .filter(val => val !== undefined && val !== null && val !== '')
      .map(val => typeof val === 'string' ? val : JSON.stringify(val))
      .join(' · ')
  } catch { /* 非 JSON */ }
  // 形如 tool(args) 的写法
  const m = raw.match(/\(([^)]*)\)/)
  if (m && m[1]) return m[1]
  return raw.replace(/\s+/g, ' ').slice(0, 160)
}

/* 从 result 文本里尽量解析出条目列表，供搜索行展开 */
export function extractResults(content = '') {
  const raw = String(content || '').trim()
  if (!raw) return []
  try {
    const j = JSON.parse(raw)
    const arr = Array.isArray(j) ? j : (j.results || j.items || j.docs || j.data)
    if (Array.isArray(arr)) {
      return arr.slice(0, 8).map(r => ({
        title: r.title || r.name || r.text || String(r).slice(0, 80),
        source: r.source || r.site || r.domain || hostOf(r.url || r.link || ''),
        url: r.url || r.link || '',
      }))
    }
  } catch { /* 非 JSON，按行解析 */ }
  return raw
    .split('\n')
    .map(l => l.replace(/^[\s\-*·•\d.]+/, '').trim())
    .filter(l => l.length > 4)
    .slice(0, 6)
    .map(l => ({ title: l.slice(0, 90), source: hostOf(l), url: '' }))
}

function hostOf(s = '') {
  const m = String(s).match(/https?:\/\/([^/\s)]+)/)
  return m ? m[1].replace(/^www\./, '') : ''
}

/* 循环 / 迭代信号识别：命中则渲染成 ↻ 提示行 */
const LOOP_RE = /(第\s*(\d+)\s*轮|迭代\s*(\d+)|round\s*(\d+)|iteration\s*(\d+)|重试|retry|再次检索|补充检索|覆盖度|未达标|继续检索)/i

/* 轮次单独提取：LOOP_RE 用于"是否是循环信号"，ROUND_RE 用于"第几轮"。
   合在一个正则里会因交替顺序丢掉捕获组（"覆盖度"比"第 2 轮"更靠前）。 */
const ROUND_RE = /(?:第\s*(\d+)\s*[轮次]|迭代\s*(\d+)|round\s*(\d+)|iteration\s*(\d+)|retry\s*(\d+))/i

export function detectLoop(text = '') {
  const t = String(text)
  if (!LOOP_RE.test(t)) return null
  const r = t.match(ROUND_RE)
  const round = r ? Number(r[1] || r[2] || r[3] || r[4] || r[5]) : null
  const isExit = /达标|满足|完成|退出|符合|通过|成功/.test(t)
  return { round: Number.isFinite(round) ? round : null, tone: isExit ? 'ok' : 'warn' }
}

/**
 * 主入口：logs -> 有序块数组
 * 连续的检索类 tool_call 会自动合并进同一个 search 块。
 */
export function parseBlocks(logs = []) {
  const blocks = []
  let cursor = null   // 当前正在累积的 search 块

  const pushLine = (text, tone = 'muted') => {
    if (!text) return
    blocks.push({ kind: 'line', text: String(text).trim(), tone })
  }

  for (let i = 0; i < logs.length; i++) {
    const log = logs[i] || {}
    const content = String(log.content || '').trim()

    if (log.type === 'tool_call') {
      const name = log.toolName || ''
      const args = extractArgs(content)

      if (isSearchTool(name)) {
        const call = {
          tool: name,
          source: guessSource(name, content),
          kw: args,
          results: [],
          done: false,
          ms: null,
        }
        if (!cursor) { cursor = { kind: 'search', calls: [] }; blocks.push(cursor) }
        cursor.calls.push(call)
      } else {
        cursor = null
        blocks.push({ kind: 'tool', name, args, result: '', status: 'running' })
      }
      continue
    }

    if (log.type === 'result') {
      // 结果归属：优先补给最近一次未完成的检索调用 / 工具调用
      const lastSearch = cursor?.calls?.slice(-1)[0]
      if (lastSearch && !lastSearch.done) {
        lastSearch.results = extractResults(content)
        lastSearch.done = true
        continue
      }
      const lastTool = [...blocks].reverse().find(b => b.kind === 'tool' && b.status === 'running')
      if (lastTool) {
        // 完整保留原始返回（可能是大 JSON）——展示层负责折叠/滚动/复制，
        // 这里截断会直接丢数据，用户就永远看不到 API 的完整结果。
        lastTool.result = content
        lastTool.status = 'ok'
        continue
      }
      pushLine(content, 'ok')
      continue
    }

    if (log.type === 'error') {
      cursor = null
      blocks.push({ kind: 'line', text: content, tone: 'error' })
      continue
    }

    // reasoning：后端把整段推理累积进同一条 log（websocket.js: last.content += ...），
    // 里面带换行。必须按行拆开，否则滚动窗会塞进一坨文本。
    cursor = null
    for (const seg of String(content).split(/\r?\n/)) {
      const line = seg.trim()
      if (!line) continue
      const loop = detectLoop(line)
      if (loop) blocks.push({ kind: 'loop', round: loop.round, verdict: line, tone: loop.tone })
      else pushLine(line, 'muted')
    }
  }
  return blocks
}

function parseObject(value) {
  if (value && typeof value === 'object' && !Array.isArray(value)) return value
  if (typeof value !== 'string') return null
  try {
    const parsed = JSON.parse(value)
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? parsed : null
  } catch {
    return null
  }
}

function finiteNumber(...values) {
  for (const value of values) {
    if (value === null || value === undefined || value === '') continue
    const number = Number(value)
    if (Number.isFinite(number)) return number
  }
  return null
}

/** Normalizes an execution output envelope without exposing Agent internals in the UI. */
export function normalizeNodeOutput(value) {
  const object = parseObject(value)
  const raw = typeof value === 'string' ? value : value == null ? '' : JSON.stringify(value)
  const contentValue = object?.content ?? object?.output ?? object?.result
  const content = typeof contentValue === 'string'
    ? contentValue
    : contentValue !== undefined
      ? JSON.stringify(contentValue, null, 2)
      : raw
  const confidence = finiteNumber(
    object?.confidence,
    object?.agent_metrics?.confidence,
    object?.metrics?.confidence,
    object?.metadata?.confidence,
  )
  const durationMs = finiteNumber(
    object?.duration_ms,
    object?.elapsed_ms,
    object?.agent_metrics?.duration_ms,
    object?.agent_metrics?.elapsed_ms,
    object?.metrics?.duration_ms,
    object?.metrics?.elapsed_ms,
  )
  return { content: String(content || '').trim(), confidence, durationMs, object }
}

function completedOutputFrom(value) {
  const object = parseObject(value)
  if (!object || String(object.status || '').toLowerCase() !== 'completed') return ''
  if (!Object.prototype.hasOwnProperty.call(object, 'output') || object.output == null || object.output === '') return ''
  return normalizeNodeOutput(object.output).content
}

/**
 * Returns only the latest completed payload's output. Intermediate reasoning,
 * tool-call and progress messages are intentionally excluded from task details.
 */
export function extractCompletedNodeOutput(state = {}) {
  const logs = Array.isArray(state.logs) ? state.logs : []
  for (let index = logs.length - 1; index >= 0; index -= 1) {
    const log = logs[index] || {}
    for (const candidate of [log.data, log.content, log]) {
      const content = completedOutputFrom(candidate)
      if (content) return content
    }
  }
  if (String(state.status || '').toLowerCase() !== 'completed') return ''
  if (state.output == null || state.output === '') return ''
  return normalizeNodeOutput(state.output).content
}

/** Returns real node metrics only; missing confidence is intentionally left null. */
export function resolveNodeMetrics(state = {}) {
  const output = normalizeNodeOutput(state.output)
  let confidence = finiteNumber(state.confidence, state.confidenceScore, state.confidence_score, output.confidence)
  if (confidence !== null && confidence > 0 && confidence <= 1) confidence *= 100
  if (confidence !== null) confidence = Math.max(0, Math.min(100, confidence))

  let durationMs = finiteNumber(
    state.durationMs,
    state.duration_ms,
    state.elapsedMs,
    state.elapsed_ms,
    output.durationMs,
  )
  if (durationMs === null) {
    const start = finiteNumber(state.startTime, state.start_time)
    const end = finiteNumber(state.endTime, state.end_time)
    if (start !== null && end !== null && end >= start) durationMs = end - start
  }
  return { confidence, durationMs }
}

function timestampMs(value) {
  if (value === null || value === undefined || value === '') return null
  const numeric = Number(value)
  if (Number.isFinite(numeric)) return numeric
  const parsed = Date.parse(String(value))
  return Number.isFinite(parsed) ? parsed : null
}

/** Prefers backend timing, falling back to a frontend wall clock only while running. */
export function resolveNodeElapsed(state = {}, nowMs = Date.now()) {
  const durationMs = resolveNodeMetrics(state).durationMs
  if (durationMs !== null) return { durationMs, live: false }
  if (String(state.status || '').toLowerCase() !== 'running') return { durationMs: null, live: false }
  const startedAt = timestampMs(state.startTime ?? state.start_time)
  const current = timestampMs(nowMs)
  if (startedAt === null || current === null) return { durationMs: null, live: false }
  return { durationMs: Math.max(0, current - startedAt), live: true }
}

/** Actual runtime calls take precedence over planned Tool/Agent/Skill/MCP configuration. */
export function collectExecutionNames(logs = [], treeNode = null) {
  const actual = logs
    .filter(log => log?.type === 'tool_call' && log.toolName)
    .map(log => String(log.toolName).trim())
    .filter(Boolean)
  const unique = values => [...new Set(values)]
  if (actual.length) return unique(actual)

  const cfg = treeNode?.node_type?.config || {}
  const configured = [
    cfg.tool_name,
    cfg.agent_type,
    cfg.skill_name,
    cfg.skill,
    ...(cfg.assigned_tools || treeNode?.node_type?.assigned_tools || treeNode?.assigned_tools || []),
  ].map(value => String(value || '').trim()).filter(Boolean)
  return unique(configured)
}

/** Removes repeated tool/final output sections while preserving their original order. */
export function dedupeOutputSections(sections = []) {
  const seen = new Set()
  return sections.filter(section => {
    const content = String(section?.content || '').trim()
    if (!content) return false
    const key = content.replace(/\s+/g, ' ')
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
}

/** 展平出所有检索结果，用于答复里的 [n] 引用角标 */
export function collectSources(blocks = []) {
  const out = []
  for (const b of blocks) {
    if (b.kind !== 'search') continue
    for (const c of b.calls) for (const r of (c.results || [])) out.push(r)
  }
  return out
}

/** 后端未返回 usage 时的 token 估算（中文≈1.5字/token，英文≈4字符/token） */
export function estimateTokens(str = '') {
  const s = String(str)
  if (!s) return 0
  const cjk = (s.match(/[\u4e00-\u9fa5]/g) || []).length
  const rest = s.length - cjk
  return Math.round(cjk / 1.5 + rest / 4)
}

/** 统一读取节点 token：优先后端 usage，其次估算 */
export function nodeTokens(state = {}, inputHint = '') {
  const u = state.usage || state.token_usage || null
  const realInput = u?.input ?? u?.prompt_tokens ?? u?.input_tokens
  const realOutput = u?.output ?? u?.completion_tokens ?? u?.output_tokens
  const inputExact = realInput !== undefined && realInput !== null
  const outputExact = realOutput !== undefined && realOutput !== null
  return {
    input: inputExact ? realInput : estimateTokens(inputHint || state.inputText || state.prompt || state.input || ''),
    output: outputExact ? realOutput : estimateTokens(state.streamContent || state.output || ''),
    inputExact,
    outputExact,
    exact: inputExact && outputExact,
  }
}

/** 单行清洗：折叠空白、遮蔽 UUID、砍掉内部 id 噪音，避免把技能 ID 甩给用户 */
const UUID_RE = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/gi
export function cleanLine(text = '', max = 160) {
  let t = String(text)
    .replace(/\s+/g, ' ')
    .replace(/\(skill=[^)]*\)/gi, '')          // (skill=63f3b3df-…) 全删
    .replace(UUID_RE, (m) => '#' + m.slice(0, 6)) // 其余 UUID 缩成 #63f3b3
    .replace(/[├└│]\s*/g, ' ')                   // 树状字符在单行里没意义
    .trim()
  if (t.length > max) t = t.slice(0, max - 1) + '…'
  return t
}

export function fmtNum(n) {
  return Number(n || 0).toLocaleString('en-US')
}
