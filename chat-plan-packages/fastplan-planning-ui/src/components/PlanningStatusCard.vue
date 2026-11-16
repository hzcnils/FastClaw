<script setup>
import { computed, nextTick, ref, watch } from 'vue'
import { ChevronDown, ChevronRight } from 'lucide-vue-next'
import { ChatMarkdownRenderer } from 'chat-renderer'
import { nodeTokens } from '../utils/blockParser'
import { planPhaseRows } from '../utils/taskDisplay'
import { formatDuration } from '../utils/duration'

const props = defineProps({
  logs: { type: Array, default: () => [] },
  isPlanning: { type: Boolean, default: false },
  planningError: { type: Object, default: null },
  isPreviewing: { type: Boolean, default: false },
  plannedTasks: { type: Array, default: () => [] },
  presentation: { type: Object, default: null },
  promptPreview: { type: String, default: '' },
  thinkingStream: { type: String, default: '' },
  thinkingSteps: { type: Array, default: () => [] },
  planPhases: { type: Number, default: 0 },
  planTasks: { type: Number, default: 0 },
  routeFlow: { type: Array, default: () => [] },
  validationFlow: { type: Array, default: () => [] },
  executionState: { type: Object, default: () => ({}) },
  planTree: { type: Object, default: null },
  activeView: { type: String, default: 'list' },
  isPaused: { type: Boolean, default: false },
  treeCompleted: { type: Boolean, default: false },
  humanRequest: { type: Object, default: null },
  nowMs: { type: Number, default: () => Date.now() },
  theme: { type: String, default: 'light', validator: value => ['light', 'dark'].includes(value) },
  density: { type: String, default: 'comfortable', validator: value => ['comfortable', 'compact'].includes(value) },
})

const emit = defineEmits(['view-change'])

const filteredLogs = computed(() => {
  const hiddenPatterns = ['开始智能任务规划', '意图分析完成', '正在分析用户意图', 'Task completed.']
  return (props.logs || []).filter(log => {
    const text = log.content || log.text || ''
    return !hiddenPatterns.some(p => text.includes(p))
  })
})

const planningActive = computed(() => props.isPlanning && !props.planTree)
const hasContent = computed(() => props.logs.length || props.presentation || props.planTree || props.isPlanning || props.plannedTasks.length
  || props.routeFlow.length || props.validationFlow.length)
const routeFlow = computed(() => props.routeFlow || props.presentation?.routeFlow || props.presentation?.route_stages || [])
const routeOpen = ref({})
const planningOutputBox = ref(null)
const followPlanningOutput = ref(true)
const planningOutput = computed(() => String(props.thinkingStream || '').trim())
// ★ 规划域只展示三个阶段（意图理解 / 匹配分析 / 任务规划）：
//   后端 7 个 route_stage 归并到 3 个展示行，界面不再暴露内部阶段名。
//   每组的展示状态 = 组内最积极状态（running > failed > done > pending），
//   文本取组内最后一条非空 summary，详情展开 = 组内全部行详情合并。
// ★ 2026-08-26：三阶段外增两个过程行——「联网检索」（jmis_search 事件，规划期外网
//   证据召回；工具名统一 jmis_search，旧名仅执行端兼容）与「大模型思考」
//   （llm_progress 心跳，正在编排…已思考 Ns 实时跳动），
//   让用户看到大模型思考与联网过程，不只看到三段结论。
const DOMAIN_GROUPS = [
  { key: 'intent', label: '意图理解', stages: ['intent'] },
  { key: 'capability', label: '匹配分析', stages: ['retrieval', 'routing', 'capability', 'assets'] },
  { key: 'plan', label: '任务规划', stages: ['request', 'llm_progress', 'plan'] },
]
const CANONICAL_MATCHING_STAGES = ['retrieval', 'capability', 'assets']
const statusRank = { running: 3, failed: 2, done: 1, pending: 0 }
const planningRows = computed(() => {
  const byStage = new Map()
  const routeByStage = new Map((routeFlow.value || []).filter(row => row?.stage).map(row => [row.stage, row]))
  const validationRows = (props.validationFlow || []).filter(row => row?.stage)
  const validationByStage = new Map(validationRows.map(row => [row.stage, row]))

  // route intent 只负责首屏即时反馈；最终结论必须由 planner 的 plan_stage(intent) 覆盖。
  // route retrieval/routing 是内部预路由结果，不能在最终意图完成前提前变成“匹配分析”。
  const routeIntent = routeByStage.get('intent')
  if (routeIntent) byStage.set('intent', { ...routeIntent, source: 'route' })
  for (const row of validationRows) byStage.set(row.stage, row)

  const canonicalIntent = validationByStage.get('intent')
  // planTree fallback 兼容 single_step、模板直出和旧的持久化记录；这些路径可能没有
  // canonical plan_stage，但计划树到达已经证明规划阶段结束。
  const legacyPlanReady = Boolean(props.planTree && !canonicalIntent)
  const intentDone = Boolean(canonicalIntent && statusDone(canonicalIntent)) || legacyPlanReady
  const matchingStarted = validationRows.some(row => CANONICAL_MATCHING_STAGES.includes(row.stage))
    || (legacyPlanReady && routeByStage.has('retrieval'))

  // planner 正式进入匹配阶段后，才把预路由的命中明细作为上下文并入；状态与文案仍以
  // canonical plan_stage 为准，避免 route retrieval 提前泄露匹配结果。
  if (matchingStarted) {
    for (const stage of ['retrieval', 'routing']) {
      const routeRow = routeByStage.get(stage)
      if (routeRow) byStage.set(stage, { ...routeRow, source: 'route-context' })
    }
    for (const row of validationRows) {
      const routeRow = routeByStage.get(row.stage)
      byStage.set(row.stage, routeRow
        ? { ...routeRow, ...row, payload: { ...(routeRow.payload || {}), ...(row.payload || {}) } }
        : row)
    }
  }

  const matchingDone = legacyPlanReady || validationRows.some(row =>
    CANONICAL_MATCHING_STAGES.includes(row.stage) && statusDone(row),
  )

  return DOMAIN_GROUPS
    .map(group => {
      // 用户可见阶段采用后端 canonical plan_stage 作为发布门：前一阶段未完成时，
      // 后一阶段的标题和结果都不出现。内部检索仍可提前或并行执行。
      if (group.key === 'capability' && !intentDone) return null
      if (group.key === 'plan' && !matchingDone && !props.planTree && !props.planningError) return null

      const groupRows = group.stages.map(s => byStage.get(s)).filter(Boolean)
      if (!groupRows.length) return null

      const status = groupRows.reduce(
        (best, row) => (statusRank[statusKind(row)] > statusRank[best] ? statusKind(row) : best),
        'pending',
      )

      // 优先取意图理解的最终结果作为摘要
      let last = null
      if (group.key === 'capability') {
        last = groupRows.find(r => r.stage === 'capability' && stageText(r))
        if (!last) last = groupRows.find(r => r.stage === 'retrieval' && stageText(r))
      } else if (group.key === 'intent') {
        // 对于意图理解，优先取 source !== 'route' 的最终大模型深度理解结果
        last = groupRows.find(r => r.stage === 'intent' && r.source !== 'route' && stageText(r))
        if (!last) last = groupRows.find(r => r.stage === 'intent' && stageText(r))
      } else {
        last = [...groupRows].reverse().find(row => stageText(row))
      }

      return {
        stage: group.key,
        label: group.label,
        summary: last ? stageText(last) : '',
        status,
        source: 'group',
        _groupRows: groupRows,
      }
    })
    .filter(Boolean)
})
const stageLabels = { intent: '意图理解', retrieval: '资源匹配', routing: '路由决策', capability: '匹配分析', assets: '候选资产', llm_progress: '深度思考', request: '构建规划请求', plan: '任务规划' }
function stageLabel(row) { return stageLabels[row.stage] || row.label || row.stage }
function stageText(row) {
  // 意图理解阶段，如果有深度的 restated_goal 则直接使用，并且去除生硬的“意图已确认：”前缀
  if (row.stage === 'intent') {
    const rawText = row.summary || row.text || row.content || ''
    // 后端可能会拼接前缀，前端负责剥离，还原为纯大白话
    if (rawText.startsWith('意图已确认：')) {
      return rawText.replace('意图已确认：', '').trim()
    }
    // 如果存在 payload 并且有 restated_goal（深度分析结果）
    if (row.payload && row.payload.restated_goal) {
      return row.payload.restated_goal
    }
    // 对于路由早期的浅层意图，如果包含机器状态枚举汇，统一拦截并显示友好的进行中提示
    const machineKeywords = ['TaskExecution', '判定为', '执行任务', '信息查询', '语义分析', '闲聊问候', '文档生成', '代码相关', '数据查询', '未知诉求', 'Chitchat', 'InformationQuery', 'DocumentGeneration', 'CodeRelated', 'StatisticalQuery', 'Unknown']
    if (machineKeywords.some(kw => rawText.includes(kw))) {
      // 检查是否因为直接回答而跳过了深层规划
      const routingRow = routeFlow.value.find(r => r.stage === 'routing' && statusDone(r))
      const isDirect = routingRow?.payload?.decision === 'reflex' || routingRow?.payload?.decision === 'direct'
      if (isDirect) {
        return '已初步理解您的诉求，正在直接解答'
      }
      return '正在深入分析您的诉求...'
    }
  }
  // 匹配分析阶段，直接使用最详细的人话结论，而不是内部的简短行摘要
  if (row.stage === 'capability') {
    const detailText = row.report?.detail || row.payload?.report?.detail
    if (detailText) {
      return detailText
    }
  }
  return row.summary || row.text || row.content || ''
}
function statusDone(row) { return ['done', 'completed', 'success'].includes(String(row.status || '').toLowerCase()) }
function routeDetails(row) {
  const p = row.payload || {}
  if (row.stage === 'intent') return [p.intent_type && `类型：${p.intent_type}`, Array.isArray(p.verbs) && p.verbs.length && `动作：${p.verbs.join('、')}`, p.multi_step != null && `多步骤：${p.multi_step ? '是' : '否'}`, p.source_hint && `来源偏好：${p.source_hint}`].filter(Boolean)
  if (row.stage === 'retrieval') return [p.top_score != null && `最高匹配分：${Number(p.top_score).toFixed(3)}`, ...(p.degraded || []).map(item => `降级：${item}`)].filter(Boolean)
  if (row.stage === 'routing') return [p.decision && `决策：${p.decision}`, p.reason_code && `原因：${p.reason_code}`, p.matched_rule && `规则：${p.matched_rule}`, p.selected_tool && `工具：${p.selected_tool}`, p.requires_confirm != null && `需要确认：${p.requires_confirm ? '是' : '否'}`].filter(Boolean)
  return []
}
function formatItemType(type) {
  if (!type) return ''
  // 翻译内置系统词汇为更易读的中文，且去掉括号
  if (type.toLowerCase() === 'builtin') return '内置工具'
  if (type.toLowerCase() === 'skill') return '业务技能'
  if (type.toLowerCase() === 'mcptool') return 'MCP 工具'
  return type
}

function stageDetails(row) {
  if (row.stage === 'routing') return (row.routes || []).map(r => `${r.name} - ${r.description}`)
  if (row.stage === 'capability') return []
  if (row.stage === 'assets') return (row.assets || []).map(item => `${item.adopted ? '已采用' : '候选'} · ${item.name} ${formatItemType(item.type)}`)
  return []
}

function validationDetails(row) {
  const p = row.payload || {}
  if (row.stage === 'capability') {
    return (row.needs || p.needs || []).map(item => `${item.label || item.need}：${item.status}${item.why ? ` · ${item.why}` : ''}`)
  }
  if (row.stage === 'assets') return (row.assets || p.assets || []).map(item => `${item.adopted ? '已采用' : '候选'} · ${item.name} ${formatItemType(item.type)}`)
  // ★ 2026-08-26：联网检索/大模型思考的过程明细（payload 字段可能在行顶层或 payload 内）
  if (row.stage === 'jmis_search') {
    const p = row.payload || {}
    return [p.query && `检索词：${p.query}`, row.evidence_count != null || p.evidence_count != null ? `外部证据：${row.evidence_count ?? p.evidence_count} 条` : '', row.evidence_chars != null || p.evidence_chars != null ? `正文 ${row.evidence_chars ?? p.evidence_chars} 字` : ''].filter(Boolean)
  }
  if (row.stage === 'llm_progress') {
    const p = row.payload || {}
    const elapsed = row.elapsed_ms ?? p.elapsed_ms
    const chars = row.received_chars ?? p.received_chars
    return [elapsed != null && `已思考 ${formatDuration(elapsed, { live: true })}`, chars != null && `已接收 ${chars} 字`].filter(Boolean)
  }
  return []
}
function planningText(row) {
  if (props.planningError && statusKind(row) === 'failed') {
    return { intent: '意图理解未完成', capability: '匹配分析未完成', plan: '任务规划失败' }[row.stage] || '处理失败'
  }
  if (row.stage === 'request' && row.prompt_chars) return `规划上下文 ${row.prompt_chars} 字符`
  return stageText(row)
}
function groupRowsOf(row) { return row._groupRows || [row] }
function planningDetails(row) {
  // ★ 分组渲染（2026-08-25 重构，修「匹配分析内容乱」）：
  //   narrative = 三段输出完整话术（trace.detail / report.detail，一段叙述）
  //   detail    = 结构化明细（路由/能力 needs/意图槽位），每条独立一行
  //   intent 槽位（术语/知识图谱/问答库/实体/缺失）独立列出——此前只混在
  //   长话术里，用户看不到「知识图谱检索/术语匹配/问答库匹配」的过程痕迹。
  const details = []
  for (const sub of groupRowsOf(row)) {
    if (sub.stage === 'intent') {
      const t = sub.trace || {}
      if (t.term_direct_hits?.length) details.push({ kind: 'detail', text: `命中业务术语：${t.term_direct_hits.map(s => `「${s}」`).join('、')}` })
      if (t.kg_relations?.length) details.push({ kind: 'detail', text: `知识图谱关联：${t.kg_relations.join('、')}` })
      if (t.kb_references?.length) details.push({ kind: 'detail', text: `问答库参考：${t.kb_references.map(r => `「${r.title}」`).join('、')}` })
      if (t.entities?.length) details.push({ kind: 'detail', text: `实体识别：${t.entities.map(e => `${e.kind}「${e.text}」`).join('、')}` })
      if (t.missing_inputs?.length) details.push({ kind: 'detail is-warn', text: `需补充：${t.missing_inputs.join('、')}` })
    }
    if (sub.trace?.detail) details.push({ kind: 'narrative', text: sub.trace.detail })
    // 注意：匹配分析 (capability) 的 report.detail 已经提到了外层 stageText 显示，不再在内部详情中重复渲染
    const rDetail = sub.report?.detail || sub.payload?.report?.detail
    if (rDetail && sub.stage !== 'capability') details.push({ kind: 'narrative', text: rDetail })
    if (sub.stage === 'retrieval') details.push(...routeDetails(sub).map(text => ({ kind: 'detail', text })))
    else details.push(...validationDetails(sub).map(text => ({ kind: 'detail', text })))
  }
  return details
}
function matchedAssets(row) {
  return groupRowsOf(row).flatMap(sub => Array.isArray(sub?.payload?.matched) ? sub.payload.matched : [])
}
function rowHasDetails(row) {
  return planningDetails(row).length || matchedAssets(row).length || (row.stage === 'plan' && (planningOutput.value || (filteredLogs.value && filteredLogs.value.length)))
}
function statusKind(row) {
  // route intent 是预路由分类，不是用户可见“意图理解”的最终结论。规划类请求在
  // canonical intent 到达前始终保持 running，不再依赖具体中文枚举词猜测状态。
  if (row.stage === 'intent' && row.source === 'route' && statusDone(row)) {
    const routingRow = routeFlow.value.find(r => r.stage === 'routing' && statusDone(r))
    const isDirect = routingRow?.payload?.decision === 'reflex' || routingRow?.payload?.decision === 'direct'
    if (!isDirect && !props.planTree) return props.planningError ? 'failed' : 'running'
  }
  if (statusDone(row)) return 'done'
  if (String(row.status || '').toLowerCase() === 'failed') return 'failed'
  if (String(row.status || '').toLowerCase() === 'running') return props.planningError ? 'failed' : 'running'
  return 'pending'
}
watch(planningOutput, async value => {
  if (!value) return
  if (planningActive.value) routeOpen.value['planning-plan'] = true
  await nextTick()
  if (planningOutputBox.value && followPlanningOutput.value) planningOutputBox.value.scrollTop = planningOutputBox.value.scrollHeight
}, { immediate: true })
watch(planningActive, value => {
  if (!value) routeOpen.value['planning-plan'] = false
})
function handlePlanningOutputScroll() {
  const element = planningOutputBox.value
  if (!element) return
  followPlanningOutput.value = element.scrollHeight - element.scrollTop - element.clientHeight < 20
}
const taskRows = computed(() => planPhaseRows(props.planTree).flatMap(row => row.tasks))
const aggregateTokens = computed(() => {
  let input = 0, output = 0
  for (const { node } of taskRows.value) {
    const state = props.executionState?.[node.id]
    if (!state || state.status === 'pending') continue
    const usage = nodeTokens(state, JSON.stringify(node?.node_type?.config || {}))
    input += Number(usage.input || 0)
    output += Number(usage.output || 0)
  }
  return { input, output }
})
const tokenSummary = computed(() => {
  const value = aggregateTokens.value
  return value.input || value.output ? `↑${compactNumber(value.input)}  ↓${compactNumber(value.output)} tokens` : ''
})
const executionDurationMs = computed(() => {
  const nodes = Object.values(props.executionState || {})
  const starts = nodes.map(node => Number(node?.startTime || node?.start_time)).filter(Number.isFinite)
  if (!starts.length) return null
  const ends = nodes.map(node => Number(node?.endTime || node?.end_time)).filter(Number.isFinite)
  const end = props.treeCompleted && ends.length ? Math.max(...ends) : props.nowMs
  const duration = end - Math.min(...starts)
  return duration > 0 ? duration : null
})
const viewTabs = [
  { key: 'list', label: '清单' },
  { key: 'timeline', label: '时间轴' },
  { key: 'dag', label: 'DAG 图' },
]
function compactNumber(value) {
  const number = Number(value || 0)
  if (number >= 1000000) return `${(number / 1000000).toFixed(number >= 10000000 ? 0 : 1)}m`
  if (number >= 1000) return `${(number / 1000).toFixed(number >= 10000 ? 0 : 1)}k`
  return String(Math.round(number))
}
</script>

<template>
  <Transition enter-active-class="transition-all duration-300 ease-out"
    enter-from-class="opacity-0 -translate-y-2" enter-to-class="opacity-100 translate-y-0"
    leave-active-class="transition-all duration-200 ease-in"
    leave-from-class="opacity-100 translate-y-0" leave-to-class="opacity-0 -translate-y-2">
    <section v-if="hasContent || $slots.default" class="fpp-ui fp-plan-card" :data-density="density">
      <div class="fp-plan-card__body">
        <div v-if="planningRows.length" class="fp-trace-flow">
          <div v-for="row in planningRows" :key="`planning-${row.stage}`" class="fp-trace-row">
            <div class="fp-trace-row__main">
              <button type="button" class="fp-trace-row__content" :class="{ 'is-expandable': rowHasDetails(row) }" :data-stage="row.stage"
                :aria-expanded="rowHasDetails(row) ? Boolean(routeOpen[`planning-${row.stage}`]) : undefined"
                :title="rowHasDetails(row) ? (routeOpen[`planning-${row.stage}`] ? '折叠详情' : '展开详情') : undefined"
                @click="rowHasDetails(row) && (routeOpen[`planning-${row.stage}`] = !routeOpen[`planning-${row.stage}`])">
                <span class="fp-step-status" :data-status="statusKind(row)">
                  <span v-if="statusKind(row) === 'running'" class="fp-step-spinner" aria-label="进行中"></span>
                  <span v-else-if="statusKind(row) === 'done'">✓</span>
                  <span v-else-if="statusKind(row) === 'failed'">{{ density === 'compact' ? '✕' : '!' }}</span>
                  <span v-else>{{ density === 'compact' ? '○' : '•' }}</span>
                </span>
                <span class="fp-trace-row__label" :data-stage="row.stage" :data-status="statusKind(row)">{{ stageLabel(row) }}</span>
                <span class="fp-trace-row__text">{{ planningText(row) }}</span>
                <template v-if="density === 'comfortable' && row.stage === 'plan'">
                  <span v-if="executionDurationMs" class="fp-plan-card__metrics is-inline">{{ formatDuration(executionDurationMs, { live: !treeCompleted }) }}</span>
                  <span v-if="tokenSummary" class="fp-plan-card__metrics is-inline is-token">{{ tokenSummary }}</span>
                </template>
              </button>
              <div v-if="density === 'comfortable' && row.stage === 'plan' && planTree" class="fp-plan-card__tabs" role="tablist" aria-label="规划展示类型">
                <button v-for="tab in viewTabs" :key="tab.key" type="button" role="tab"
                  :aria-selected="activeView === tab.key" :class="{ 'is-active': activeView === tab.key }"
                  @click="emit('view-change', tab.key)">{{ tab.label }}</button>
              </div>
              <button v-if="rowHasDetails(row)" type="button" class="fp-trace-row__chevron-button"
                :aria-expanded="Boolean(routeOpen[`planning-${row.stage}`])"
                :title="routeOpen[`planning-${row.stage}`] ? '折叠详情' : '展开详情'"
                @click="routeOpen[`planning-${row.stage}`] = !routeOpen[`planning-${row.stage}`]">
                <component :is="routeOpen[`planning-${row.stage}`] ? ChevronDown : ChevronRight"
                  :size="15" class="fp-trace-row__chevron" />
              </button>
            </div>
            <div v-if="rowHasDetails(row) && routeOpen[`planning-${row.stage}`]" class="fp-trace-details">
              <div v-for="(detail, index) in planningDetails(row)" :key="`${index}-${detail.text}`" :class="`fp-trace-detail is-${detail.kind}`">{{ detail.text }}</div>
              <div v-if="matchedAssets(row).length" class="fp-asset-list">
                <span v-for="asset in matchedAssets(row)" :key="`${asset.type}-${asset.name}`" class="fp-asset-chip">{{ asset.type }} · {{ asset.name }}</span>
              </div>
              <div v-if="row.stage === 'plan' && planningOutput" ref="planningOutputBox" class="fp-planning-output"
                @scroll.passive="handlePlanningOutputScroll">
                <ChatMarkdownRenderer :content="planningOutput" :max-chars="100000" :theme="theme" />
              </div>
              <div v-if="row.stage === 'plan' && filteredLogs.length" class="fp-raw-logs">
                <div v-for="(log, index) in filteredLogs" :key="index" :class="{ 'is-error': log.type === 'error' }">{{ log.content || log.text }}</div>
              </div>
            </div>
          </div>
        </div>

        <div v-if="planTree && $slots.default" class="fp-plan-card__details is-direct">
          <slot />
        </div>

      </div>
    </section>
  </Transition>
</template>

<style scoped>
.fp-plan-card { min-width: 0; margin-bottom: 12px; font-size: inherit; }
.fp-plan-card__code { flex: 0 0 auto; color: var(--jcr-muted,#6b7280); font: 500 12px/1.4 ui-sans-serif,system-ui,sans-serif; }
.fp-plan-card__metrics { flex: 0 0 auto; color: var(--jcr-muted,#6b7280); font: 500 11px/1.4 ui-monospace,SFMono-Regular,Menlo,monospace; white-space: nowrap; }
.fp-plan-card__metrics.is-inline { margin-left:2px; color:color-mix(in srgb,var(--jcr-muted,#6b7280) 88%,var(--jcr-fg,#1f2937) 12%); }
.fp-plan-card__metrics.is-token { color:#6366f1; }
.fp-plan-card__duration { flex: 0 0 auto; color: #059669; font: 600 11px/1.4 ui-monospace,SFMono-Regular,Menlo,monospace; }
.fp-plan-card__duration.is-running { color: #4f46e5; }
.fp-plan-card__body { min-width: 0; }
.fp-trace-flow { margin: 0 0 10px; }
.fp-trace-flow .fp-trace-row { padding: 0 2px; }
.fp-step-status { display:grid; width:14px; height:14px; flex:0 0 14px; place-items:center; color:var(--jcr-muted,#94a3b8); font-weight:700; }
.fp-step-status[data-status="done"] { color:#10b981; }
.fp-step-status[data-status="failed"] { color:#e11d48; }
.fp-step-spinner { width:11px; height:11px; border:2px solid color-mix(in srgb,#6366f1 24%,transparent); border-top-color:#6366f1; border-radius:50%; animation:fp-step-spin .75s linear infinite; }
.fp-trace-row + .fp-trace-row { border-top: 1px solid color-mix(in srgb,var(--jcr-border,#e5e7eb) 55%,transparent); }
.fp-trace-row__main { display:flex; width:100%; min-width:0; align-items:center; gap:7px; }
.fp-trace-row__content { display:flex; min-width:0; flex:1; align-items:center; gap:7px; padding:6px 0; border:0; color:inherit; background:transparent; text-align:left; cursor:default; }
.fp-trace-row__content.is-expandable { cursor:pointer; }
/* ★ 三阶段 label 徽章：意图理解 / 匹配分析 / 任务规划 各一色 */
.fp-trace-row__label { flex:0 0 auto; min-width:0; padding:0; border-radius:0; font-weight:600; font-size:inherit; text-align:left; background:transparent; color:inherit; }
.fp-trace-row__label[data-stage="capability"],
.fp-trace-row__label[data-stage="jmis"],
.fp-trace-row__label[data-stage="think"],
.fp-trace-row__label[data-stage="plan"],
.fp-trace-row__label[data-status="done"],
.fp-trace-row__label[data-status="failed"] { background:transparent; color:inherit; animation:none; }
.fp-trace-row__label[data-status="running"] { color:#4338ca; animation:none; }
.fp-plan-card[data-density="comfortable"] .fp-trace-row__label { min-width:64px; padding:2px 9px; border-radius:999px; font-size:11px; text-align:center; background:color-mix(in srgb,#6366f1 9%,transparent); color:#4f46e5; }
.fp-plan-card[data-density="comfortable"] .fp-trace-row__label[data-stage="capability"] { background:color-mix(in srgb,#0d9488 10%,transparent); color:#0f766e; }
.fp-plan-card[data-density="comfortable"] .fp-trace-row__label[data-stage="jmis"] { background:color-mix(in srgb,#0284c7 10%,transparent); color:#0369a1; }
.fp-plan-card[data-density="comfortable"] .fp-trace-row__label[data-stage="think"] { background:color-mix(in srgb,#7c3aed 10%,transparent); color:#6d28d9; }
.fp-plan-card[data-density="comfortable"] .fp-trace-row__label[data-stage="plan"] { background:color-mix(in srgb,#d97706 10%,transparent); color:#b45309; }
.fp-plan-card[data-density="comfortable"] .fp-trace-row__label[data-status="done"] { background:color-mix(in srgb,#16a34a 10%,transparent); color:#15803d; }
.fp-plan-card[data-density="comfortable"] .fp-trace-row__label[data-status="failed"] { background:color-mix(in srgb,#ef4444 10%,transparent); color:#dc2626; }
.fp-plan-card[data-density="comfortable"] .fp-trace-row__label[data-status="running"] { color:#4f46e5; animation:fp-plan-pulse 1.2s infinite; }
.fp-trace-row__text { min-width:0; flex:1; overflow:hidden; color:var(--jcr-muted,#6b7280); text-overflow:ellipsis; white-space:nowrap; }
.fp-trace-row__content[data-stage="plan"] .fp-trace-row__text { flex:0 1 auto; }
.fp-trace-row__chevron-button { display:inline-flex; width:24px; height:24px; flex:0 0 24px; align-items:center; justify-content:center; padding:0; border:0; border-radius:5px; color:var(--jcr-muted,#6b7280); background:transparent; cursor:pointer; }
.fp-trace-row__chevron-button:hover { color:var(--jcr-fg,#1f2937); background:color-mix(in srgb,var(--jcr-border,#e5e7eb) 55%,transparent); }
.fp-trace-row__chevron { flex:0 0 auto; }.fp-trace-details { padding:0 0 7px 22px; color:var(--jcr-muted,#6b7280); line-height:1.65; }
.fp-plan-card[data-density="comfortable"] .fp-trace-details { padding-left:87px; }
/* ★ 分组渲染（2026-08-25）：话术段（narrative）常规正文、明细段（detail）紧凑条目 */
.fp-trace-detail.is-narrative { color:color-mix(in srgb,var(--jcr-fg,#1f2937) 82%,var(--jcr-muted,#6b7280) 18%); }
.fp-trace-detail.is-detail { font-size:11px; padding:1px 0 1px 10px; border-left:2px solid color-mix(in srgb,var(--jcr-border,#e5e7eb) 75%,transparent); margin:2px 0; }
.fp-trace-detail.is-detail.is-warn { color:#b45309; border-left-color:#f59e0b; }
.fp-asset-list { display:flex; flex-wrap:wrap; gap:5px; margin-top:5px; }.fp-asset-chip { padding:2px 6px; border-radius:999px; background:color-mix(in srgb,#6366f1 9%,transparent); color:#4f46e5; }
.fp-planning-output { max-height:220px; overflow:auto; margin-top:7px; padding:8px 10px; border:1px solid color-mix(in srgb,var(--jcr-border,#e5e7eb) 70%,transparent); border-radius:7px; color:var(--jcr-fg,#1f2937); background:color-mix(in srgb,var(--jcr-muted,#6b7280) 4%,var(--jcr-bg,#fff)); overscroll-behavior:contain; }
.fp-planning-output :deep(.jcr-markdown > div > :first-child) { margin-top:0; }.fp-planning-output :deep(.jcr-markdown > div > :last-child) { margin-bottom:0; }
.fp-raw-logs { max-height:220px; overflow:auto; margin-top:7px; padding:7px; border-radius:6px; color:var(--jcr-muted,#6b7280); background:color-mix(in srgb,#0f172a 4%,transparent); font-family:ui-monospace,SFMono-Regular,Menlo,monospace; white-space:pre-wrap; }.fp-raw-logs .is-error { color:#e11d48; }
.fp-plan-card__details { min-width: 0; margin-top: 10px; padding-top: 10px; border-top: 1px solid var(--jcr-border,#e5e7eb); }
.fp-plan-card__details.is-direct { margin-top: 0; padding-top: 0; border-top: 0; }
.fp-plan-card__tabs { display: flex; width: max-content; max-width: 100%; flex: 0 0 auto; gap: 2px; margin: 0; padding: 2px; border-radius: 7px; background: color-mix(in srgb,var(--jcr-border,#e5e7eb) 48%,transparent); }
.fp-plan-card__tabs button { padding: 3px 8px; border: 0; border-radius: 5px; color: var(--jcr-muted,#6b7280); background: transparent; font-size: 11px; cursor: pointer; }
.fp-plan-card__tabs button.is-active { color: var(--jcr-fg,#1f2937); background: var(--jcr-bg,#fff); box-shadow: 0 1px 2px rgb(15 23 42 / .1); }
@keyframes fp-step-spin { to { transform:rotate(360deg); } }
@keyframes fp-plan-pulse { 50% { opacity: .35; } }
@media (prefers-reduced-motion:reduce) { .fp-step-spinner { animation: none !important; } }
.fp-plan-card[data-density="compact"] .fp-trace-row__main { flex-wrap:wrap; }
.fp-plan-card[data-density="compact"] .fp-trace-row__chevron-button { margin-left:auto; }
</style>
