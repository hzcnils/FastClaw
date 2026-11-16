<script setup>
/**
 * PlanTaskRow.vue — 规划态的任务行（v21 清单视图，对照 demo nodeHTML 四行结构）
 * 与执行态的 TaskBlock 共用同一套视觉语言（时间轴竖线 + 圆点 + 序号 + 徽章）：
 *   › SEARCH 01 [检索] 联网检索 RAG 最新进展                    待执行
 *       多轮检索，直到资料覆盖度达标才退出        检索意图 → 调研结论
 *       ↻ while 覆盖度 < 85%，最多 3 轮
 *       agentQuery(scope="all", page=1)
 *     [徽章行：工具/技能]
 *     ▾ 计划检索 · 第 1 轮   3 组 · 待执行
 *
 * 关键点：关键词在确认前就摆出来、可编辑——这才是"规划是入口"的价值。
 */
import { computed, ref } from 'vue'
import { ChevronRight, Search, ChevronDown, Edit3, Copy, Trash2, GripVertical } from 'lucide-vue-next'
import { ChatModelIcon } from 'chat-renderer'
import { isSearchTool } from '../utils/blockParser'
import { resolveNodeModel } from '../utils/nodeModel'

const props = defineProps({
  task:  { type: Object, required: true },
  seq:   { type: String, default: '' },
  editable: { type: Boolean, default: true },
  // ★ 原始用户需求：用于展示前替换模板占位符 {{user_input}}，绝不原样露出
  userInput: { type: String, default: '' },
  // ★ 已注册工具清单 { name → kind }：内置/skill/mcp/managed，用于元信息徽章与存在性校验。
  //   null = 未加载（不标 ⚠）；对象 = 已加载，未命中才标 ⚠
  toolRegistry: { type: Object, default: null },
  // ★ 编辑态：显示任务操作按钮组（编辑/复制/删除/拖拽）
  editing: { type: Boolean, default: false },
})
const emit = defineEmits(['edit', 'update-query', 'copy', 'delete'])

// ★ 展开详情：默认折叠——计划检索关键词等细节收进详情；四行结构（序号/描述/循环/调用）常显
const expanded = ref(false)

// ★ 展示前替换模板占位符；替换后仍残留 {{...}} 的行直接剔除（宁可不显示也不漏占位符）
function sanitizeText(text) {
  if (!text) return ''
  let t = String(text)
    .replace(/\{\{\s*user_input\s*\}\}|\{\{\s*input\s*\}\}|\{\{\s*query\s*\}\}/gi, props.userInput || '')
  return t
    .split('\n')
    .filter(l => !/\{\{[\s\S]*\}\}/.test(l))
    .join('\n')
    .trim()
}
const safeSummary = computed(() => sanitizeText(summary.value))

/* ★ 元信息徽章：工具/技能名 + 来源类型前缀（Skill/MCP/内置/HTTP）；未注册的标 ⚠ 警告 */
const badges = computed(() => {
  const reg = props.toolRegistry // null = 未加载；对象 = 已加载
  const out = []
  const seen = new Set()
  const pushTool = (name) => {
    if (!name || seen.has(name)) return
    seen.add(name)
    const s = String(name).trim()
    if (!s) return
    const short = s.includes(':') ? s.split(':').pop() : s
    const kind = reg ? (reg[s] || reg[short] || '') : ''
    if (!reg) {
      // 清单未加载：只显示工具名，不做类型/存在性判断（避免全部误报 ⚠）
      out.push({ label: s, full: s, cls: 'bg-slate-100 text-slate-600' })
    } else if (kind === 'mcp') {
      const sep = s.indexOf(':')
      const server = sep >= 0 ? s.slice(0, sep) : ''
      out.push({ label: `[MCP] ${s}`, full: server ? `${s} · ${server}` : s, cls: 'bg-amber-50 text-amber-700' })
    } else if (kind === 'skill') {
      out.push({ label: `[Skill] ${s}`, full: s, cls: 'bg-sky-50 text-sky-700' })
    } else if (kind === 'builtin') {
      out.push({ label: `[内置] ${s}`, full: s, cls: 'bg-slate-100 text-slate-600' })
    } else if (kind === 'managed') {
      out.push({ label: `[HTTP] ${s}`, full: s, cls: 'bg-violet-50 text-violet-700' })
    } else {
      out.push({ label: `⚠ 未找到 ${s}`, full: `未找到该技能：${s}`, cls: 'bg-rose-50 text-rose-600' })
    }
  }
  if (cfg.value.tool_name) pushTool(cfg.value.tool_name)
  for (const t of tools.value) pushTool(t)
  return out
})

/* ── 角色映射：沿用工程既有的 node_type，只换配色到 v21 口径 ── */
const TYPE = {
  AgentTask:       { code: 'TASK',   n: 'Agent',    c: 'text-purple-700 bg-purple-50', d: 'bg-purple-400' },
  LlmTask:         { code: 'TASK',   n: 'LLM',      c: 'text-blue-700 bg-blue-50',     d: 'bg-blue-400'   },
  ToolTask:        { code: 'TOOL',   n: 'MCP 工具', c: 'text-amber-700 bg-amber-50',   d: 'bg-amber-400'  },
  HumanCheckpoint: { code: 'HUMAN',  n: '人工确认', c: 'text-violet-700 bg-violet-50', d: 'bg-violet-400' },
  Unknown:         { code: 'TASK',   n: '任务',     c: 'text-slate-600 bg-slate-100',  d: 'bg-slate-400'  },
}
const typeStr = computed(() => props.task?.node_type?.type || 'Unknown')
const cfg     = computed(() => props.task?.node_type?.config || {})
const model   = computed(() => resolveNodeModel(props.task))
const tools   = computed(() => cfg.value.assigned_tools || props.task?.node_type?.assigned_tools || [])

/* 含检索工具 → 整行按 SEARCH 块呈现 */
const searchTools = computed(() => {
  const all = [...tools.value]
  if (cfg.value.tool_name) all.unshift(cfg.value.tool_name)
  return all.filter(t => isSearchTool(t))
})
const isSearch = computed(() => searchTools.value.length > 0)

/* 循环声明：后端若给了 loop 配置就渲染退出条件（LOOP 序号标签） */
const loop = computed(() => {
  const l = cfg.value.loop || props.task?.metadata?.loop
  if (!l) return null
  if (typeof l === 'string') return l
  if (l.while) return `while ${l.while}${l.max ? `，最多 ${l.max} 轮` : ''}`
  if (l.foreach) return `foreach ${l.foreach}${l.count ? `（${l.count} 项）` : ''}`
  return null
})
const role = computed(() => {
  const base = TYPE[typeStr.value] || TYPE.Unknown
  if (isSearch.value)
    return { code: 'SEARCH', n: '检索', c: 'text-indigo-700 bg-indigo-50', d: 'bg-indigo-500' }
  if (loop.value)
    return { code: 'LOOP', n: '循环', c: 'text-teal-700 bg-teal-50', d: 'bg-teal-500' }
  return base
})

/* 摘要与依赖 */
/* 后端实际字段：config.description / task_description / prompt_template */
const summary = computed(() => {
  const c = cfg.value
  const t = c.description || c.task_description || props.task?.description || ''
  if (t) return t
  const p = c.prompt_template || c.prompt || ''
  return p ? String(p).replace(/\s+/g, ' ').slice(0, 60) : ''
})
const inputs  = computed(() => props.task?.metadata?.input_refs || [])
const outputs = computed(() => props.task?.metadata?.output_key || '')
const ioText  = computed(() => {
  const i = inputs.value.length ? inputs.value.join('、') : '上游输出'
  return outputs.value ? `${i} → ${outputs.value}` : ''
})

/* 计划中的检索关键词：后端若给了 queries / keywords 就用，否则从描述兜底 */
/* 计划检索关键词
   优先级：config.queries（若后端将来补） → config.arguments 里的查询参数（现有字段）
   → assigned_tools 兜底。arguments 是后端真实存在的 key，所以关键词多数能直接读到。 */
const QKEYS = ['query', 'q', 'keyword', 'keywords', 'search_query', 'input', 'text']
const queries = computed(() => {
  const c = cfg.value
  const q = c.queries || c.keywords || props.task?.metadata?.queries
  if (Array.isArray(q) && q.length) {
    return q.map((x, i) => typeof x === 'string'
      ? { tool: searchTools.value[0] || toolName.value || 'web_search', source: '互联网', kw: x, argKey: null, idx: i }
      : { tool: x.tool || toolName.value || 'web_search', source: x.source || '互联网',
          kw: x.query || x.kw || '', argKey: null, idx: i })
  }
  if (!isSearch.value) return []
  const args = c.arguments || {}
  const hit = QKEYS.find(k => typeof args[k] === 'string' && args[k].trim())
  if (hit) return [{ tool: toolName.value || searchTools.value[0], source: '互联网', kw: args[hit], argKey: hit, idx: 0 }]
  return searchTools.value.map((t, i) => ({
    tool: t, source: '互联网', kw: summary.value || props.task.name, argKey: null, idx: i }))
})

/* 工具签名行：非检索类工具显示调用形态 */
/* 工具签名：后端真实字段是 config.tool_name + config.arguments */
const toolName = computed(() =>
  cfg.value.tool_name || tools.value.filter(x => !isSearchTool(x))[0] || tools.value[0] || '')
const toolSig = computed(() => {
  const name = cfg.value.tool_name || tools.value.filter(x => !isSearchTool(x))[0]
  if (!name || isSearchTool(name)) return ''
  const args = cfg.value.arguments
  const argStr = args && typeof args === 'object'
    ? Object.entries(args).slice(0, 2)
        .map(([k, v]) => `${k}=${typeof v === 'string' ? JSON.stringify(v.slice(0, 24)) : JSON.stringify(v)}`)
        .join(', ')
    : ''
  return { name, args: argStr }
})

/* 编辑关键词 → 直接写回 config.arguments，确保提交的树里带着改动 */
function onKw(i, e) {
  const value = (e.target.innerText || '').trim()
  const q = queries.value[i]
  if (q?.argKey) {
    const c = props.task.node_type?.config
    if (c) { c.arguments = c.arguments || {}; c.arguments[q.argKey] = value }
    props.task.isEdited = true
  }
  emit('update-query', { index: i, value, argKey: q?.argKey || null })
}
</script>

<template>
  <div class="fp-row group relative">
    <!-- 时间轴圆点：盖在左侧竖线上（规划态一律 待执行 slate-300） -->
    <span class="absolute -left-[15px] top-1.5 w-2 h-2 rounded-full bg-slate-300 ring-4 ring-white"></span>

    <!-- 首行：展开箭头 + 序号标签 + 类型徽章 + 标题 + 状态 -->
    <div class="flex items-center gap-1.5 text-xs cursor-pointer" @click="emit('edit', task)">
      <ChevronRight :size="10" class="text-slate-200 shrink-0" />
      <span class="font-mono text-[9px] text-slate-400 shrink-0">{{ role.code }}</span>
      <span v-if="seq" class="font-mono text-[10px] text-slate-300 shrink-0">{{ seq }}</span>
      <ChatModelIcon v-if="typeStr === 'LlmTask'" class="fpp-model-icon" :name="model.name" :model-id="model.id" :size="14" />
      <span class="text-[10px] font-bold px-1.5 py-0.5 rounded shrink-0" :class="role.c">{{ role.n }}</span>
      <span class="fp-t1 truncate min-w-0 text-slate-800 group-hover:text-indigo-600 transition-colors">{{ task.name }}</span>
      <span v-if="task.isEdited" class="text-[9px] text-amber-600 bg-amber-50 px-1 rounded shrink-0">已修改</span>
      <span v-else-if="task.isNew" class="text-[9px] text-emerald-600 bg-emerald-50 px-1 rounded shrink-0">新增</span>
      <span class="flex-1"></span>
      <!-- ★ 编辑态：任务操作按钮组（淡显 hover 加深） -->
      <template v-if="editing">
        <button @click.stop="emit('edit', task)" :title="'编辑：' + task.name"
                class="p-0.5 rounded text-slate-300 hover:text-indigo-600 hover:bg-indigo-50 transition-colors"><Edit3 :size="11" /></button>
        <button @click.stop="emit('copy', task)" :title="'复制：' + task.name"
                class="p-0.5 rounded text-slate-300 hover:text-indigo-600 hover:bg-indigo-50 transition-colors"><Copy :size="11" /></button>
        <button @click.stop="emit('delete', task)" :title="'删除：' + task.name"
                class="p-0.5 rounded text-slate-300 hover:text-rose-600 hover:bg-rose-50 transition-colors"><Trash2 :size="11" /></button>
        <GripVertical :size="11" class="text-slate-200 cursor-grab shrink-0" />
      </template>
      <!-- ★ 详情展开切换（计划检索等可编辑细节，默认折叠） -->
      <button v-if="queries.length"
              @click.stop="expanded = !expanded"
              class="p-0.5 hover:bg-slate-100 rounded"
              :title="expanded ? '收起详情' : '展开详情'">
        <ChevronDown :size="11" class="text-slate-300 transition-transform" :class="{ 'rotate-180': expanded }" />
      </button>
      <span class="font-mono text-[11px] text-slate-400 shrink-0">待执行</span>
    </div>

    <!-- 第二行：任务描述一句话（13px 次级灰）+ 行尾输入→输出 -->
    <div v-if="safeSummary || ioText" class="flex items-baseline gap-2">
      <span v-if="safeSummary" class="fp-t2 truncate">{{ safeSummary }}</span>
      <span v-if="ioText" class="font-mono text-[10px] text-slate-400 shrink-0 truncate">{{ ioText }}</span>
    </div>

    <!-- 循环声明：退出条件在确认前可见 -->
    <div v-if="loop" class="flex items-center gap-1 text-[10px] text-teal-600 font-mono">
      <span class="shrink-0">↻</span><span class="truncate">{{ loop }}</span>
    </div>

    <!-- 第三行：计划调用（执行前只显示 工具名(参数)，不加 → 与返回，不写占位） -->
    <div v-if="toolSig && toolSig.name" class="fp-t3 truncate">
      <span class="text-violet-600">{{ toolSig.name }}</span>({{ toolSig.args }})
    </div>

    <!-- ★ 元信息徽章行：工具/技能（默认可见，单行省略、hover 全名） -->
    <div v-if="badges.length" class="flex items-center gap-1.5 overflow-hidden">
      <span v-for="(b, i) in badges" :key="i" :title="b.full"
            class="text-[10px] font-medium px-1.5 py-px rounded-full whitespace-nowrap shrink-0"
            :class="b.cls">{{ b.label }}</span>
    </div>

    <!-- 展开详情：计划检索关键词（确认前可编辑） -->
    <div v-if="expanded" class="pl-[14px]">
      <div v-if="queries.length" class="mt-1">
        <div class="flex items-center gap-1.5 text-[11px] text-sky-600">
          <Search :size="11" class="shrink-0" />
          <span class="font-medium">计划检索</span>
          <span class="font-mono text-[10px] text-slate-300">{{ queries.length }} 组 · 待执行</span>
        </div>
        <div class="fp-indent mt-0.5 space-y-px">
          <div v-for="(q, i) in queries" :key="i"
               class="flex items-center gap-1.5 text-[11px] py-px">
            <ChevronRight :size="10" class="text-slate-200 shrink-0" />
            <span class="w-3 shrink-0 text-slate-300 text-[10px] text-center">○</span>
            <span class="font-mono text-violet-600 shrink-0">{{ q.tool }}</span>
            <span class="text-[10px] px-1 rounded bg-sky-50 text-sky-700 shrink-0">{{ q.source }}</span>
            <span class="text-slate-600 truncate flex-1 rounded px-0.5"
                  :contenteditable="editable" spellcheck="false"
                  :class="editable ? 'hover:bg-indigo-50 focus:bg-indigo-50 focus:ring-1 focus:ring-indigo-200 outline-none' : ''"
                  @click.stop @blur="onKw(i, $event)">{{ q.kw }}</span>
            <span class="font-mono text-[10px] text-slate-300 shrink-0">待执行</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.fp-row { padding: 2px 0; }
.fp-indent { margin-left: 6px; padding-left: 14px; border-left: 1px solid #eef2f6; }
</style>
