<script setup>
import { ref, computed, onMounted } from 'vue'
import {
  Plus, Trash2, Search, RefreshCw, X, Check, Loader2,
  Plug, PlugZap, Unplug, TestTube, Zap, ChevronDown, ChevronRight,
  ToggleLeft, ToggleRight, Wrench, Download, AlertCircle,
  FileJson, Minus, Globe, Box, Server, Settings, Eye, Info
} from 'lucide-vue-next'
import { mcpApi } from './adminApi.js'
import AiDescriptionButton from './AiDescriptionButton.vue'
import { useI18n } from 'vue-i18n'
import { ElMessageBox, ElMessage } from 'element-plus'
import { useThemeStore } from '@/stores/theme'
import SearchClearButton from '@/components/common/SearchClearButton.vue'
import wsIcon from '@/assets/mcp/wl.svg'
import sseIcon from '@/assets/mcp/sse.svg'
import stdioIcon from '@/assets/mcp/stdio.svg'
import wsIconWhite from '@/assets/mcp/wl_white.svg'
import sseIconWhite from '@/assets/mcp/sse_white.svg'
import stdioIconWhite from '@/assets/mcp/stdio_white.svg'

const { t } = useI18n()
const themeStore = useThemeStore()

const cardBgStyle = computed(() => {
  const id = themeStore.draftPreset.id
  if (id === 'simple-dark' || id.startsWith('dark-')) return { 'background-color': 'rgba(255,255,255,0.05)' }
  if (id === 'simple-light' || id === 'scene-graphite-cyan') return { 'background-color': 'rgba(0,0,0,0.07)' }
  if (id.startsWith('scene-')) return { 'background-color': 'rgba(255,255,255,0.10)' }
  return {}
})
const searchStyle = computed(() => {
  const id = themeStore.draftPreset.id
  if (id === 'simple-dark' || id.startsWith('dark-'))
    return { 'background-color': 'transparent', color: '#fff' }
  if (id.startsWith('scene-') && id !== 'scene-graphite-cyan')
    return { 'background-color': 'transparent', color: '#fff' }
  return { 'background-color': 'transparent' }
})
const toggleStyle = computed(() => {
  const id = themeStore.draftPreset.id
  if (id === 'simple-dark' || id.startsWith('dark-'))
    return { 'background-color': 'rgba(0,0,0,0.3)' }
  if (id.startsWith('scene-') && id !== 'scene-graphite-cyan')
    return { 'background-color': 'rgba(0,0,0,0.05)' }
  return {}
})
const filterTextStyle = computed(() => {
  const id = themeStore.draftPreset.id
  if (id.startsWith('scene-') && id !== 'scene-graphite-cyan')
    return { color: '#fff' }
  return {}
})
const toggleActiveStyle = computed(() => {
  const id = themeStore.draftPreset.id
  if (id === 'simple-dark' || id.startsWith('dark-'))
    return { 'background-color': 'rgba(255,255,255,0.20)', color: '#fff' }
  if (id.startsWith('scene-') && id !== 'scene-graphite-cyan')
    return { 'background-color': 'rgba(0,0,0,0.05)', color: '#1f2937' }
  return {}
})
const toggleInactiveStyle = computed(() => {
  const id = themeStore.draftPreset.id
  if (id === 'simple-dark' || id.startsWith('dark-'))
    return { color: 'rgba(255,255,255,0.5)' }
  if (id.startsWith('scene-') && id !== 'scene-graphite-cyan')
    return { 'background-color': 'rgba(0,0,0,0.05)', color: '#1f2937' }
  return {}
})

const filterBtnStyle = computed(() => {
  const id = themeStore.draftPreset.id
  if (id === 'simple-light' || id == 'scene-graphite-cyan')
    return { color: '#282c34', border: '1px solid rgba(0,0,0,0.1)', 'border-radius': '4px' }
  if (id === 'simple-dark' || id.startsWith('dark-'))
    return { color: '#cbd5e0', border: '1px solid rgba(255,255,255,0.12)', 'border-radius': '4px' }
  if (id.startsWith('scene-') )
    return { color: '#fff', border: '1px solid rgba(255,255,255,0.20)', 'border-radius': '4px' }
  return {}
})

const filterMenuStyle = computed(() => {
  const id = themeStore.draftPreset.id
  const p = themeStore.draftPreset.palette
  const bg = p?.modalBg || p?.bgPrimary || '#fff'
  if (id === 'simple-dark' || id.startsWith('dark-') || (id.startsWith('scene-') && id !== 'scene-graphite-cyan'))
    return { background: bg, border: '1px solid rgba(255,255,255,0.1)' }
  return { background: bg }
})

const filterMenuItemStyle = computed(() => {
  const id = themeStore.draftPreset.id
  if (id === 'simple-dark' || id.startsWith('dark-'))
    return { color: '#fff' }
  if (id.startsWith('scene-') && id !== 'scene-graphite-cyan')
    return { color: '#fff' }
  return { color: 'var(--text-primary)' }
})

const items = ref([])
const loading = ref(true)
const error = ref('')
const searchQuery = ref('')
const viewMode = ref('card') // 'card' | 'list'
const protocolFilter = ref('all')
const statusFilter = ref('all')
const sourceFilter = ref('all')
const showProtocolMenu = ref(false)
const showStatusMenu = ref(false)
const showSourceMenu = ref(false)
const showCreateDialog = ref(false)
const showTestDialog = ref(false)
const expandedServer = ref(null)
const testingServer = ref(null)
const testResult = ref(null)
const showDetailPanel = ref(false)
const detailServer = ref(null)
const detailTools = ref([])
const detailLoading = ref(false)

// Sync toast
const syncToast = ref(null)
let toastTimer = null
function showSyncToast(count) {
  clearTimeout(toastTimer)
  syncToast.value = count
  toastTimer = setTimeout(() => { syncToast.value = null }, 4000)
}

function formatMcpError(err, fallbackKey = 'loadFailed') {
  const raw = err?.message || String(err || '')
  const lower = raw.toLowerCase()
  if (lower.includes('mcp manager unavailable')) return t('mcp.managerUnavailable')
  return raw || t(`mcp.${fallbackKey}`)
}

// Create form
const form = ref({
  name: '', description: '', transport_type: 'stdio',
  command: '', args: '', sse_url: '', auto_start: false, keywords: '', auto_index: true,
  auth_type: 'none', auth_token: ''
})

// Env vars (key-value rows, assembled into environment object on submit)
const envRows = ref([])
function addEnvRow() { envRows.value.push({ key: '', value: '' }) }
function removeEnvRow(i) { envRows.value.splice(i, 1) }

// ── HTTP 传输认证（SSE / Streamable HTTP） ──────────────────────
// 落点是 transport_config.headers —— FastPlan 侧只需认这一个概念，
// 不必区分「bearer」和「自定义 header」两种模式。
// Stdio 的密钥仍走 environment，与这里无关。

const headerRows = ref([])
function addHeaderRow() { headerRows.value.push({ key: '', value: '' }) }
function removeHeaderRow(i) { headerRows.value.splice(i, 1) }

/** 认证表单态 → transport_config.headers */
function buildAuthHeaders(authType, token, rows) {
  const headers = {}
  if (authType === 'bearer') {
    const t = String(token || '').trim()
    if (t) headers.Authorization = /^bearer\s/i.test(t) ? t : `Bearer ${t}`
  } else if (authType === 'headers') {
    for (const row of rows || []) {
      const key = String(row.key || '').trim()
      if (key) headers[key] = String(row.value ?? '')
    }
  }
  return headers
}

/** 组装 transport_config；HTTP 传输带上认证头（无认证时不写空 headers，保持旧数据形状） */
function buildTransportConfig(transportType, { command, args, url, authType, authToken, headers }) {
  if (transportType === 'stdio') {
    return {
      command,
      args: args ? String(args).split('\n').filter(Boolean).map(a => a.trim()) : [],
    }
  }
  const config = { url }
  const authHeaders = buildAuthHeaders(authType, authToken, headers)
  if (Object.keys(authHeaders).length) config.headers = authHeaders
  return config
}

/** transport_config.headers → 认证表单态（打开编辑时还原） */
function parseAuthFromConfig(config) {
  const raw = config && typeof config.headers === 'object' && config.headers ? config.headers : null
  const keys = raw ? Object.keys(raw) : []
  if (!keys.length) return { auth_type: 'none', auth_token: '', headerRows: [] }
  // 仅有一个 Authorization: Bearer xxx → 还原成 Token 单输入框
  const authKey = keys.find(k => k.toLowerCase() === 'authorization')
  if (keys.length === 1 && authKey && /^bearer\s/i.test(String(raw[authKey]))) {
    return { auth_type: 'bearer', auth_token: String(raw[authKey]), headerRows: [] }
  }
  return {
    auth_type: 'headers',
    auth_token: '',
    headerRows: keys.map(k => ({ key: k, value: String(raw[k]) })),
  }
}

// ── 一键导入 ─────────────────────────────────────────────────
const showImportDialog = ref(false)
const importText      = ref('')
const importError     = ref('')
const importParsed    = ref([])   // [{name, config}] when multiple servers found

function openImportDialog() {
  importText.value = ''; importError.value = ''; importParsed.value = []
  showImportDialog.value = true
}

function normalizeImportedTransportType(cfg) {
  const rawType = String(cfg?.type || '').trim().toLowerCase()
  if (rawType === 'streamable_http' || rawType === 'streamable-http') return 'sse'
  if (rawType) return rawType
  return cfg?.url ? 'sse' : 'stdio'
}

function fillFormFromConfig(name, cfg) {
  form.value.name = name
  const type = normalizeImportedTransportType(cfg)
  form.value.transport_type = type
  if (type === 'stdio') {
    form.value.command  = cfg.command || ''
    form.value.args     = Array.isArray(cfg.args) ? cfg.args.join(' ') : (cfg.args || '')
    form.value.sse_url  = ''
  } else {
    form.value.sse_url  = cfg.url || ''
    form.value.command  = ''
    form.value.args     = ''
  }
  envRows.value = cfg.env && typeof cfg.env === 'object'
    ? Object.entries(cfg.env).map(([key, value]) => ({ key, value: String(value) }))
    : []
}

function parseImport() {
  importError.value = ''; importParsed.value = []
  let json
  try { json = JSON.parse(importText.value.trim()) }
  catch (e) { importError.value = t('mcp.invalidJson') + ': ' + e.message; return }

  // Support { mcpServers: { name: cfg } } or bare { name: cfg }
  const servers = json.mcpServers || json
  if (typeof servers !== 'object' || Array.isArray(servers))
    { importError.value = t('mcp.invalidMcpFormat'); return }

  const entries = Object.entries(servers)
  if (!entries.length) { importError.value = t('mcp.noServerConfig'); return }

  if (entries.length === 1) {
    // 单个 server：直接导入
    importOne(entries[0][0], entries[0][1])
  } else {
    importParsed.value = entries.map(([name, config]) => ({ name, config }))
  }
}

function selectImportServer(item) {
  importOne(item.name, item.config)
}

// 直接导入单个 server（与 create() 使用相同 payload 格式）
async function directImport(name, cfg) {
  const type = normalizeImportedTransportType(cfg)
  let transport_config = {}
  if (type === 'stdio') {
    transport_config = {
      command: cfg.command || '',
      args: Array.isArray(cfg.args) ? cfg.args : (cfg.args ? [cfg.args] : [])
    }
  } else {
    transport_config = { url: cfg.url || '' }
  }
  const environment = cfg.env && typeof cfg.env === 'object'
    ? cfg.env
    : {}
  await mcpApi.create({
    name,
    description: cfg.description || '',
    transport_type: type,
    transport_config,
    environment: Object.keys(environment).length ? environment : null,
    auto_start: cfg.auto_start ?? false,
    keywords: cfg.keywords || [],
    auto_index: cfg.auto_index ?? true,
  })
}

// 单 server 导入（关闭弹窗 + 刷新）
async function importOne(name, cfg) {
  try {
    await directImport(name, cfg)
    showImportDialog.value = false
    importText.value = ''; importParsed.value = []
    await load()
    ElMessage.success(t('mcp.importSuccess') + ': ' + name)
  } catch (e) {
    importError.value = t('mcp.importFailed') + ': ' + (e.message || e)
  }
}

// 批量导入列表中所有 server
async function directImportAll() {
  let ok = 0, fail = 0
  for (const item of [...importParsed.value]) {
    try {
      await directImport(item.name, item.config)
      ok++
    } catch { fail++ }
  }
  showImportDialog.value = false
  importText.value = ''; importParsed.value = []
  await load()
  if (fail === 0) ElMessage.success(t('mcp.importAllSuccess') + ': ' + ok)
  else ElMessage.warning(t('mcp.importPartial') + ': ' + ok + t('mcp.success') + ', ' + fail + t('mcp.failed'))
}

// Test form
const testForm = ref({ tool_name: '', arguments: '{}' })
const testMode = ref('form') // 'form' | 'json' — ★ Issue #5a: 表单/JSON 切换

// ★ Issue #5a: 从 input_schema 解析表单字段
const testFormFields = computed(() => {
  if (!testingServer.value?._tools) return []
  const tool = testingServer.value._tools.find(t => t.name === testForm.value.tool_name)
  if (!tool?.input_schema) return []
  try {
    const schema = typeof tool.input_schema === 'string' ? JSON.parse(tool.input_schema) : tool.input_schema
    const props = schema.properties || {}
    const required = schema.required || []
    return Object.entries(props).map(([key, prop]) => ({
      key,
      type: prop.type || 'string',
      description: prop.description || '',
      required: required.includes(key),
      default: prop.default,
      enum: prop.enum || null,
    }))
  } catch { return [] }
})
const testFieldValues = ref({})

function initTestFields() {
  testFieldValues.value = {}
  for (const f of testFormFields.value) {
    testFieldValues.value[f.key] = f.default ?? ''
  }
}

function buildTestArgs() {
  if (testMode.value === 'json') return JSON.parse(testForm.value.arguments)
  const args = {}
  for (const [k, v] of Object.entries(testFieldValues.value)) {
    if (v !== '' && v !== null && v !== undefined) {
      const field = testFormFields.value.find(f => f.key === k)
      if (field?.type === 'integer' || field?.type === 'number') args[k] = Number(v)
      else if (field?.type === 'boolean') args[k] = v === true || v === 'true'
      else args[k] = v
    }
  }
  return args
}

// ★ Issue #5b: MCP Server 编辑
const showEditDialog = ref(false)
const editingServer = ref(null)
const editForm = ref({ name: '', description: '', transport_type: 'stdio', sse_url: '', command: '', args: '', keywords: '', auto_start: false, auto_index: true, auth_type: 'none', auth_token: '' })
const editEnvRows = ref([])
const editHeaderRows = ref([])

const filtered = computed(() => {
  let result = items.value
  if (protocolFilter.value !== 'all') {
    result = result.filter(i => i.transport_type === protocolFilter.value)
  }
  if (statusFilter.value !== 'all') {
    result = result.filter(i => i.last_status === statusFilter.value)
  }
  if (sourceFilter.value !== 'all') {
    result = result.filter(i => sourceLabel(i) === sourceFilter.value)
  }
  if (!searchQuery.value) return result
  const q = searchQuery.value.toLowerCase()
  return result.filter(i => i.name?.toLowerCase().includes(q) || i.description?.toLowerCase().includes(q))
})

const stats = computed(() => {
  const total = items.value.length
  const connected = items.value.filter(i => i.last_status === 'running').length
  const disconnected = total - connected
  const toolCount = items.value.reduce((acc, i) => acc + (i.tool_count || 0), 0)
  return { total, connected, disconnected, toolCount }
})

function protocolLabel(type) {
  if (type === 'sse') return 'SSE'
  if (type === 'stdio') return 'STDIO'
  if (type === 'http') return 'HTTP'
  if (type === 'websocket') return 'WS'
  return type || 'Unknown'
}

function protocolBadgeClass(type) {
  if (type === 'sse') return 'bg-indigo-50 text-indigo-700'
  if (type === 'stdio') return 'bg-violet-50 text-violet-700'
  if (type === 'http') return 'bg-blue-50 text-blue-700'
  if (type === 'websocket') return 'bg-cyan-50 text-cyan-700'
  return 'bg-gray-100 text-gray-600'
}

function protocolEmoji(type) {
  if (type === 'sse') return '🌐'
  if (type === 'stdio') return '💻'
  if (type === 'http') return '🌍'
  if (type === 'websocket') return '🔗'
  return '🔌'
}

function protocolIcon(type) {
  const id = themeStore.draftPreset.id
  const useWhite = id === 'simple-dark' || id.startsWith('dark-') || (id.startsWith('scene-') && id !== 'scene-graphite-cyan')
  if (type === 'sse') return useWhite ? sseIconWhite : sseIcon
  if (type === 'stdio') return useWhite ? stdioIconWhite : stdioIcon
  if (type === 'http') return useWhite ? sseIconWhite : sseIcon  // HTTP 复用 SSE 图标
  if (type === 'websocket') return useWhite ? wsIconWhite : wsIcon
  return useWhite ? sseIconWhite : sseIcon  // 兜底统一用 SSE 图标
}

function sourceLabel(item) {
  if (item.source) return item.source
  if (item.origin) return item.origin
  return t('mcp.source_manual')
}

function sourceBadgeClass(label) {
  const lower = String(label || '').toLowerCase()
  if (lower.includes('github')) return 'bg-emerald-50 text-emerald-700 border-emerald-200'
  if (lower.includes('modelscope')) return 'bg-rose-50 text-rose-700 border-rose-200'
  if (lower.includes('npm')) return 'bg-orange-50 text-orange-700 border-orange-200'
  return 'bg-indigo-50 text-indigo-700 border-indigo-200'
}

const protocolOptions = computed(() => {
  const types = Array.from(new Set(items.value.map(i => i.transport_type).filter(Boolean)))
  return ['all', ...types]
})

const protocolLabelText = computed(() => {
  return protocolFilter.value === 'all' ? t('mcp.protocol') : protocolLabel(protocolFilter.value)
})

const statusOptions = computed(() => [
  { value: 'all', label: t('mcp.status') },
  { value: 'running', label: t('mcp.connected') },
  { value: 'stopped', label: t('mcp.disconnected') },
  { value: 'error', label: t('mcp.abnormal') },
])

const statusLabelText = computed(() => {
  return statusOptions.value.find(s => s.value === statusFilter.value)?.label || t('mcp.status')
})

const sourceOptions = computed(() => {
  const sources = Array.from(new Set(items.value.map(i => sourceLabel(i)).filter(Boolean)))
  return ['all', ...sources]
})

const sourceLabelText = computed(() => {
  return sourceFilter.value === 'all' ? t('mcp.source_filter') : sourceFilter.value
})

function normalizeStatus(status) {
  if (status === null || status === undefined) return 'stopped'
  if (typeof status === 'object' && status.error) return 'error'
  if (typeof status === 'string') {
    const s = status.toLowerCase()
    if (s === 'running') return 'running'
    if (s === 'starting' || s === 'connecting') return 'connecting'
    if (s === 'error') return 'error'
    if (s === 'stopped') return 'stopped'
  }
  return 'stopped'
}

function statusDotClass(status) {
  const s = normalizeStatus(status)
  if (s === 'running') return 'bg-emerald-500'
  if (s === 'connecting') return 'bg-yellow-500'
  if (s === 'error') return 'bg-rose-500'
  return 'bg-gray-300'
}

function statusText(status) {
  const s = normalizeStatus(status)
  if (s === 'running') return t('mcp.connected')
  if (s === 'connecting') return t('mcp.connecting')
  if (s === 'error') return t('mcp.abnormal')
  return t('mcp.disconnected')
}

async function load() {
  loading.value = true; error.value = ''
  try {
    const res = await mcpApi.list()
    const raw = res.items || res || []
    // Parse persisted discovered_tools so they show without re-connecting
    items.value = raw.map(item => {
      if (!item._tools && item.discovered_tools) {
        try { item._tools = JSON.parse(item.discovered_tools) } catch {}
      }
      if (!item.tool_count && item._tools) item.tool_count = item._tools.length
      return item
    })
  } catch (e) { error.value = formatMcpError(e, 'loadFailed') }
  finally { loading.value = false }
}

async function remove(item) {
  try {
    await ElMessageBox.confirm(
      t('mcp.confirmDelete', { name: item.name }),
      t('common.delete'),
      {
        confirmButtonText: t('common.confirm'),
        cancelButtonText: t('common.cancel'),
        type: 'warning',
        customClass: 'delete-confirm-dialog',
        cancelButtonClass: 'delete-confirm-cancel-btn',
        closeOnClickModal: false,
      }
    )
  } catch { return }
  try {
    await mcpApi.delete(item.id)
    items.value = items.value.filter(i => i.id !== item.id)
  } catch (e) { error.value = formatMcpError(e, 'deleteFailed') }
}

async function connect(item) {
  item._connecting = true
  item.error_message = null
  try {
    const res = await mcpApi.connect(item.id)
    item.last_status = 'running'
    item._connecting = false
    // Auto refresh tools
    await refreshTools(item)
    // Toast: 同步工具到工具库
    if (item.tool_count) {
      showSyncToast(item.tool_count)
    }
  } catch (e) {
    item.last_status = 'error'
    item.error_message = formatMcpError(e, 'connectFailed')
    item._connecting = false
  }
}

async function disconnect(item) {
  try {
    await mcpApi.disconnect(item.id)
    item.last_status = 'stopped'
    item.error_message = null
  } catch (e) { error.value = formatMcpError(e, 'disconnectFailed') }
}

async function refreshTools(item) {
  item._refreshing = true
  try {
    const tools = await mcpApi.refreshTools(item.id)
    item._tools = tools
    item.tool_count = tools.length
    item._toolsFresh = true
  } catch (e) { error.value = formatMcpError(e, 'refreshToolsFailed') }
  finally { item._refreshing = false }
}

async function viewDetail(item) {
  showDetailPanel.value = true
  detailServer.value = item
  detailTools.value = []
  detailLoading.value = true
  try {
    await refreshTools(item)
    detailTools.value = item._tools || []
  } finally {
    detailLoading.value = false
  }
}

function openTest(item) {
  testingServer.value = item
  testResult.value = null
  testMode.value = 'form'
  testForm.value = { tool_name: item._tools?.[0]?.name || '', arguments: '{}' }
  // ★ Issue #5a: 初始化表单字段
  setTimeout(() => initTestFields(), 0)
  showTestDialog.value = true
}

async function runTest() {
  testResult.value = null
  try {
    const args = buildTestArgs()
    testResult.value = await mcpApi.testTool(testingServer.value.id, testForm.value.tool_name, args)
  } catch (e) {
    testResult.value = { success: false, error: formatMcpError(e, 'testFailed'), duration_ms: 0 }
  }
}

// ★ Issue #5b: 打开编辑弹窗
//
// 列表接口（McpServerSummary）**不含** environment / auto_start —— 这两个字段
// 只在详情接口（McpServerDetail）里。所以先用列表数据把弹窗立刻铺开（避免白屏等待），
// 再异步取详情补齐这两项。
//
// 不补齐会踩两个坑：
//   · 环境变量永远显示为空（用户以为丢了，其实是没读回来）
//   · auto_start 读成 undefined → 保存时发 false → **每次编辑都静默关掉自动启动**
// editEnvLoaded 标记详情是否到手：没到手时保存**不碰**这两列（传 null 即"不改动"）。
const editEnvLoaded = ref(false)

async function openEditServer(item) {
  editingServer.value = item
  // Prefer item's direct fields (from backend), fall back to transport_config
  let config = item.transport_config
  if (typeof config === 'string') {
    try { config = JSON.parse(config) } catch { config = {} }
  }
  config = config && typeof config === 'object' ? config : {}
  const transportType = item.transport_type || 'stdio'
  const auth = parseAuthFromConfig(config)
  editForm.value = {
    name: item.name || '',
    description: item.description || '',
    transport_type: transportType,
    sse_url: item.url || config.url || '',
    command: item.command || config.command || '',
    args: Array.isArray(item.args) ? item.args.join('\n') : (config.args || []).join('\n'),
    keywords: Array.isArray(item.keywords) ? item.keywords.join(', ') : (item.keywords || ''),
    // null = 未知。详情到手前保持 null，保存时据此决定要不要发这个字段
    auto_start: item.auto_start ?? null,
    auto_index: item.auto_index ?? true,
    auth_type: auth.auth_type,
    auth_token: auth.auth_token,
  }
  editHeaderRows.value = auth.headerRows
  editEnvRows.value = []
  editEnvLoaded.value = false
  showEditDialog.value = true

  try {
    const detail = await mcpApi.get(item.id)
    // 等待期间用户可能已经关掉/切到别的 server，别把数据写串
    if (editingServer.value?.id !== item.id) return
    const envObj = detail?.environment
    editEnvRows.value = envObj && typeof envObj === 'object'
      ? Object.entries(envObj).map(([key, value]) => ({ key, value: String(value) }))
      : []
    editForm.value.auto_start = detail?.auto_start ?? null
    // 详情的 transport_config 更权威（列表那份可能已被同步覆盖过），顺带刷一下认证头
    if (detail?.transport_config && typeof detail.transport_config === 'object') {
      const dAuth = parseAuthFromConfig(detail.transport_config)
      editForm.value.auth_type = dAuth.auth_type
      editForm.value.auth_token = dAuth.auth_token
      editHeaderRows.value = dAuth.headerRows
    }
    editEnvLoaded.value = true
  } catch {
    // 详情拿不到：保持"未知"，保存时 environment / auto_start 都不发，DB 原值不动
  }
}

async function saveEditServer() {
  if (!editingServer.value) return
  try {
    const transport_config = buildTransportConfig(editForm.value.transport_type, {
      command: editForm.value.command,
      args: editForm.value.args,
      url: editForm.value.sse_url,
      authType: editForm.value.auth_type,
      authToken: editForm.value.auth_token,
      headers: editHeaderRows.value,
    })
    const environment = editEnvRows.value.filter(r => r.key.trim())
      .reduce((acc, r) => { acc[r.key.trim()] = r.value; return acc }, {})
    const payload = {
      name: editForm.value.name,
      description: editForm.value.description,
      transport_type: editForm.value.transport_type,
      transport_config,
      keywords: editForm.value.keywords ? editForm.value.keywords.split(',').map(k => k.trim()).filter(Boolean) : [],
      // 注意：FastPlan 的 UpdateMcpServerRequest 没有 auto_index 字段，serde 会忽略它。
      // 保留原样以免影响其它后端，但当前对 FastPlan 不生效。
      auto_index: editForm.value.auto_index,
    }
    // 详情已到手 → environment 是权威值：即便为空也要发，用户可能就是想清空。
    // 没到手 → 不发，绝不拿一份"读不到"的值去覆盖 DB 里已有的环境变量。
    if (editEnvLoaded.value) payload.environment = environment
    // 同上：null = 未知，不发。避免每次编辑都把 auto_start 静默写成 false。
    if (editForm.value.auto_start !== null) payload.auto_start = editForm.value.auto_start
    await mcpApi.update(editingServer.value.id, payload)
    showEditDialog.value = false
    editEnvRows.value = []
    editHeaderRows.value = []
    await load()
  } catch (e) { error.value = e.message }
}

async function create() {
  try {
    const transport_config = buildTransportConfig(form.value.transport_type, {
      command: form.value.command,
      args: form.value.args,
      url: form.value.sse_url,
      authType: form.value.auth_type,
      authToken: form.value.auth_token,
      headers: headerRows.value,
    })
    const environment = envRows.value.filter(r => r.key.trim())
      .reduce((acc, r) => { acc[r.key.trim()] = r.value; return acc }, {})
    await mcpApi.create({
      name: form.value.name,
      description: form.value.description,
      transport_type: form.value.transport_type,
      transport_config,
      environment: Object.keys(environment).length ? environment : null,
      auto_start: form.value.auto_start,
      keywords: form.value.keywords ? form.value.keywords.split(',').map(k => k.trim()).filter(Boolean) : [],
      auto_index: form.value.auto_index,
    })
    showCreateDialog.value = false
    form.value = { name: '', description: '', transport_type: 'stdio', command: '', args: '', sse_url: '', auto_start: false, keywords: '', auto_index: true, auth_type: 'none', auth_token: '' }
    envRows.value = []
    headerRows.value = []
    await load()
  } catch (e) { error.value = e.message }
}

async function syncFastMCP() {
  try {
    await mcpApi.syncFastMCP()
    await load()
  } catch (e) { error.value = e.message }
}

function toggleExpand(id) {
  expandedServer.value = expandedServer.value === id ? null : id
}

onMounted(load)
</script>

<template>
  <div class="mcp-page mx-auto w-full transition-all duration-300"
       :data-theme-id="themeStore.draftPreset.id"
    
       style="background: var(--bg-primary); width: 100%; height: 100%; padding:20px; display: flex; flex-direction: column; overflow: hidden;"
       @click="showProtocolMenu = false; showStatusMenu = false; showSourceMenu = false">
    
    <!-- Header -->
    <div class="mcp-page-header">
      <div class="mcp-header-left">
        <div class="mcp-title text-lg font-bold">{{ $t('mcp.title') }}</div>
        <div class="mcp-subtitle text-base mt-1">{{ $t('mcp.subtitle') }}</div>
      </div>
      <div class="mcp-header-actions">
        <button @click="openImportDialog"
          class="mcp-header-btn flex items-center gap-2 px-5 py-2.5 text-white text-sm rounded transition-all"
          style="background:var(--btn-primary-bg)"
           @mouseenter="$event.currentTarget.style.background='var(--btn-primary-hover-bg)'"
          @mouseleave="$event.currentTarget.style.background='var(--color-primary)'"
            >
          <FileJson :size="16" />
          <span>{{ $t('mcp.configImport') }}</span>
        </button>
        <!-- FastMCP sync 已废弃，等待 sync_from_fastplan 就绪 -->
        <!-- <button @click="syncFastMCP" ...><Download/><span>FastMCP</span></button> -->
        <button @click="showCreateDialog = true"
          class="mcp-header-btn flex items-center gap-2 px-5 py-2.5 text-white text-sm rounded transition-all"
          style="background:var(--btn-primary-bg)"
           @mouseenter="$event.currentTarget.style.background='var(--btn-primary-hover-bg)'"
          @mouseleave="$event.currentTarget.style.background='var(--color-primary)'"
          >
          <Plus :size="16" />
          <span>{{ $t('mcp.addServer') }}</span>
        </button>
        <button @click="load"
          class="mcp-header-btn mcp-header-btn-icon p-2.5 text-white rounded transition-all"
          style="background:var(--btn-primary-bg)"
           @mouseenter="$event.currentTarget.style.background='var(--btn-primary-hover-bg)'"
          @mouseleave="$event.currentTarget.style.background='var(--color-primary)'"
          >
          <RefreshCw :size="16" :class="{ 'animate-spin': loading }" />
        </button>
      </div>
    </div>

    <!-- Stats Dashboard -->
    <div class="mcp-stats-row">
      <div class="bg-white border border-gray-100 rounded-lg p-3 flex items-center gap-4 shadow-[0_2px_8px_-2px_rgba(0,0,0,0.05)] hover:shadow-md transition-shadow group"
        :style="cardBgStyle">
        <div class="w-10 h-10  rounded-lg bg-indigo-50 text-indigo-600 flex items-center justify-center border border-indigo-100 group-hover:scale-110 transition-transform">
          <Server :size="22" />
        </div>
          <div class="text-base font text-[#86909C]">{{ $t('mcp.totalServers') }}</div>
          <div class="mcp-title text-3xl font-bold">{{ stats.total }}</div>
      </div>
      <div class="bg-white border border-gray-100 rounded-lg p-3 flex items-center gap-4 shadow-[0_2px_8px_-2px_rgba(0,0,0,0.05)] hover:shadow-md transition-shadow group"
        :style="cardBgStyle">
        <div class="w-10 h-10  rounded-lg flex items-center justify-center group-hover:scale-110 transition-transform"
          style="background: #ecfdf5; color: #059669; border: 1px solid #d1fae5;">
          <PlugZap :size="22" />
        </div>
          <div class="text-base font text-[#86909C]">{{ $t('mcp.running') }}</div>
          <div class="text-3xl font-bold text-[#1D2129]">{{ stats.connected }}</div>
      </div>
      <div class="bg-white border border-gray-100 rounded-lg p-3 flex items-center gap-4 shadow-[0_2px_8px_-2px_rgba(0,0,0,0.05)] hover:shadow-md transition-shadow group"
        :style="cardBgStyle">
        <div class="w-10 h-10  rounded-lg flex items-center justify-center group-hover:scale-110 transition-transform"
          style="background: #f9fafb; color: #6b7280; border: 1px solid #f3f4f6;">
          <Unplug :size="22" />
        </div>
          <div class="text-base font text-[#86909C]">{{ $t('mcp.disconnected') }}</div>
          <div class="text-3xl font-bold text-[#1D2129]">{{ stats.disconnected }}</div>
      </div>
      <div class="bg-white border border-gray-100 rounded-lg p-3 flex items-center gap-4 shadow-[0_2px_8px_-2px_rgba(0,0,0,0.05)] hover:shadow-md transition-shadow group"
        :style="cardBgStyle">
        <div class="w-10 h-10 rounded-lg bg-violet-50 text-violet-600 flex items-center justify-center border border-violet-100 group-hover:scale-110 transition-transform">
          <Wrench :size="22" />
        </div>
          <div class="text-base font text-[#86909C]">{{ $t('mcp.availableTools') }}</div>
          <div class="text-3xl font-bold text-[#1D2129]">{{ stats.toolCount }}</div>
      </div>
    </div>

    <!-- Filters -->
    <div class="mcp-toolbar sticky top-20 z-10">
      <div class="mcp-toolbar-main">
        <div class="mcp-toolbar-search">
          <Search :size="16" class="absolute left-3.5 top-1/2 -translate-y-1/2 text-gray-400"  style="color: var(--text-secondary);" />
          <input v-model="searchQuery" :placeholder="$t('mcp.searchPlaceholder')" 
            class="w-full pl-10 py-2.5 text-base bg-transparent border border-[var(--border-color)] rounded outline-none transition-all"
            :class="searchQuery ? 'pr-9' : 'pr-4'"
            :style="searchStyle" />
          <SearchClearButton v-if="searchQuery" overlay @clear="searchQuery = ''" />
        </div>

        <div class="mcp-toolbar-divider"></div>

        <div class="mcp-toolbar-filters">
          <div class="relative">
            <button @click.stop="showProtocolMenu = !showProtocolMenu; showStatusMenu = false"
              class="tools-filter-btn px-4 py-2 text-[var(--font-size-md)] font-medium rounded hover:bg-gray-50 inline-flex items-center gap-2 transition-colors"
              :style="filterBtnStyle"
              :class="protocolFilter !== 'all' ? 'text-[#2B53E5] bg-blue-50 hover:bg-blue-100' : ''">
              {{ protocolLabelText }} <ChevronDown :size="14" class="opacity-60" />
            </button>
            <div v-if="showProtocolMenu" class="skills-filter-menu absolute top-full left-0 mt-2 w-40 rounded shadow-xl z-20 overflow-hidden animate-in fade-in zoom-in-95 duration-150"
              :style="filterMenuStyle">
              <button @click="protocolFilter = 'all'; showProtocolMenu = false" class="w-full text-left px-4 py-2.5 text-base hover:bg-gray-50" :style="filterMenuItemStyle">{{ $t('mcp.allProtocols') }}</button>
              <button v-for="p in protocolOptions.filter(v => v !== 'all')" :key="p"
                @click="protocolFilter = p; showProtocolMenu = false"
                class="w-full text-left px-4 py-2.5 text-base hover:bg-gray-50"
                :style="filterMenuItemStyle"
                :class="protocolFilter === p ? 'bg-gray-50' : ''">
                {{ protocolLabel(p) }}
              </button>
            </div>
          </div>

          <div class="relative">
            <button @click.stop="showStatusMenu = !showStatusMenu; showProtocolMenu = false"
              class="tools-filter-btn px-4 py-2 text-[var(--font-size-md)] font-medium rounded hover:bg-gray-50 inline-flex items-center gap-2 transition-colors"
              :style="filterBtnStyle"
              :class="statusFilter !== 'all' ? 'text-[#2B53E5]' : ''">
              {{ statusLabelText }} <ChevronDown :size="14" class="opacity-60" />
            </button>
            <div v-if="showStatusMenu" class="skills-filter-menu absolute top-full left-0 mt-2 w-40 rounded shadow-xl z-20 overflow-hidden animate-in fade-in zoom-in-95 duration-150"
              :style="filterMenuStyle">
              <button v-for="s in statusOptions" :key="s.value" @click="statusFilter = s.value; showStatusMenu = false"
                class="w-full text-left px-4 py-2.5 text-base hover:bg-gray-50"
                :style="filterMenuItemStyle"
                :class="statusFilter === s.value ? 'bg-gray-50' : ''">
                {{ s.label }}
              </button>
            </div>
          </div>
        </div>
      </div>

      <div class="mcp-toolbar-view view-mode-toggle flex bg-gray-100 p-1 rounded" :style="toggleStyle">
        <button @click="viewMode = 'card'" class="px-3 py-1.5 text-[var(--font-size-md)] font-medium rounded transition-all flex items-center gap-1"
          :class="viewMode === 'card' ? 'bg-white text-[#1D2129] shadow-sm' : 'text-gray-500 hover:text-gray-700'"
          :style="viewMode === 'card' ? toggleActiveStyle : toggleInactiveStyle">
          <Box :size="14" /> {{ $t('mcp.card') }}
        </button>
        <button @click="viewMode = 'list'" class="px-3 py-1.5 text-[var(--font-size-md)] font-medium rounded transition-all flex items-center gap-1"
          :class="viewMode === 'list' ? 'bg-white text-[#1D2129] shadow-sm' : 'text-gray-500 hover:text-gray-700'"
          :style="viewMode === 'list' ? toggleActiveStyle : toggleInactiveStyle">
          <Server :size="14" /> {{ $t('mcp.list') }}
        </button>
      </div>
    </div>

    <!-- Scrollable content area: only cards/list scroll -->
    <div class="mcp-scroll-content flex-1 overflow-y-auto min-h-0">

    <!-- Error -->
    <div v-if="error" class="mb-6 p-4 bg-red-50 border border-red-100 text-red-700 text-base rounded-lg flex items-center gap-3 shadow-sm">
      <AlertCircle :size="16" class="flex-shrink-0" />
      <span class="flex-1 font-medium">{{ error }}</span>
      <button @click="error = ''" class="text-red-400 hover:text-red-600 p-1 hover:bg-red-100 rounded-lg transition-colors"><X :size="16" /></button>
    </div>

    <div v-if="loading" class="mcp-loading-state flex flex-col items-center justify-center py-24 gap-3">
      <Loader2 :size="32" class="animate-spin" style="color: var(--text-primary);" />
      <span class="text-sm font-medium" style="color: var(--text-secondary);">{{ $t('mcp.loading') }}</span>
    </div>

    <div v-else>
      <!-- Grid View (Marketplace Style) -->
      <!-- Empty State (shared, outside view containers, transparent bg) -->
      <div v-if="!filtered.length && !loading" class="text-center py-20 text-gray-400">
        <div class="w-30 h-30 rounded-full flex items-center justify-center mx-auto mb-4 border border-[var(--border-color)]" style="background: transparent;">
          <Plug :size="39" style="color: var(--text-primary);" />
        </div>
        <p class="text-xl font-medium text-gray-500" style="color: var(--text-primary);">{{ $t('mcp.noServers') }}</p>
        <button @click="showCreateDialog = true" class="mt-4 text-base text-[var(--text-primary)] hover:underline font-medium">{{ $t('mcp.addNow') }}</button>
      </div>

      <div v-if="viewMode === 'card'" class="mcp-card-grid">
        <div v-for="item in filtered" :key="item.id"
          class="bg-white rounded-2xl border border-gray-100 shadow-[0_2px_12px_-4px_rgba(0,0,0,0.05)] hover:shadow-[0_8px_24px_-6px_rgba(0,0,0,0.12)] hover:border-[#2B53E5]/30 transition-all duration-300 flex flex-col h-full group relative overflow-hidden">
          
          <!-- Icon (Top Right) -->
          <div class="absolute top-5 right-5 w-12 h-12 rounded-xl bg-gray-50 flex items-center justify-center text-3xl border border-gray-100 shadow-sm flex-shrink-0 group-hover:scale-110 transition-transform duration-300">
            <img :src="protocolIcon(item.transport_type)" class="w-6 h-6" />
          </div>

          <!-- Card Body -->
          <div class="px-6 pt-6 pb-0 flex-1 flex flex-col">
            <!-- Header -->
            <div class="mb-1 pr-14">
              <div class="flex items-center gap-2 mb-1.5">
                <span class="mcp-source-badge text-[var(--font-size-sm)] px-1.5 py-0.5 rounded font-bold uppercase tracking-wide border"
                  :class="sourceBadgeClass(sourceLabel(item))">
                  {{ sourceLabel(item) }}
                </span>
                <span class="relative flex h-2 w-2" v-if="item.last_status === 'running'">
                  <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
                  <span class="relative inline-flex rounded-full h-2 w-2 bg-emerald-500"></span>
                </span>
              </div>
              <h3 class="text-[var(--font-size-base)] font-bold text-[#1D2129] truncate leading-tight" :title="item.name">{{ item.name }}</h3>
            </div>

            <!-- Description -->
            <p class="text-[var(--font-size-sm)] text-[#86909C] line-clamp-2 h-9 leading-relaxed mb-0">
              {{ item.description || $t('mcp.noDescription') }}
            </p>

            <!-- Command / URL -->
            <div class=" mb-2 text-[var(--font-size-xs)] text-[#86909C] truncate">
              <code v-if="item.command" class="rounded bg-[var(--dropdown-hover-color)] px-2 py-1 text-[var(--font-size-xs)] ">{{ item.command }}</code>
              <span v-if="item.url" class="truncate block">{{ item.url }}</span>
            </div>

            <!-- Tags -->
            <div class="flex flex-wrap gap-2 mt-auto border-0">
              <span class="inline-flex items-center gap-1 text-[var(--font-size-sm)] px-2 py-1 rounded-full  border " style="border-color: var(--border-color);">
                <img :src="protocolIcon(item.transport_type)" class="w-2.5 h-2.5" />
                {{ protocolLabel(item.transport_type) }}
              </span>
              <span class="inline-flex items-center gap-1 text-[var(--font-size-sm)] px-2 py-1 rounded-full border" style="border-color: var(--border-color);"
                >
                <Wrench :size="10" />
                {{ item.tool_count || 0 }} {{ $t('mcp.tools') }}
              </span>
            </div>

            <!-- Error Message -->
            <div
              v-if="item.error_message"
              class="mt-2 rounded bg-red-50 px-2 py-1 text-[var(--font-size-xs)] text-red-600 dark:bg-red-950 dark:text-red-400"
            >
              {{ item.error_message }}
            </div>
          </div>

          <!-- Divider -->
          <div class="h-px bg-gray-50 mx-6 "></div>

          <!-- Footer (Actions) -->
          <div class="px-6 pt-[5px] pb-3 flex items-center justify-between">
            <div class="flex items-center gap-2 text-[var(--font-size-sm)] font-medium" :class="item.last_status === 'running' ? 'text-emerald-600' : item.last_status === 'connecting' ? 'text-yellow-600' : item.last_status === 'error' ? 'text-red-600' : 'text-gray-900'">
              <span class="w-1.5 h-1.5 rounded-full" :class="statusDotClass(item.last_status)"></span>
              {{ statusText(item.last_status) }}
            </div>

            <div class="flex items-center gap-1 rounded-lg p-0.5">
              <button @click="viewDetail(item)" class="p-1.5 text-gray-500 hover:bg-[var(--dropdown-hover-color)] hover:text-[#1D2129]  rounded-lg transition-colors" :title="$t('mcp.details')">
                <Eye :size="14" />
              </button>
              <button v-if="item.last_status !== 'running'" @click="connect(item)" :disabled="item._connecting"
                class="p-1.5 text-gray-500  hover:bg-[var(--dropdown-hover-color)] rounded-lg transition-colors" :title="$t('mcp.connect')">
                <Loader2 v-if="item._connecting" :size="14" class="animate-spin" />
                <Plug v-else :size="14" />
              </button>
              <button v-else @click="disconnect(item)"
                class="p-1.5 text-gray-500 hover:bg-[var(--dropdown-hover-color)] hover:text-red-600 rounded-lg transition-colors" :title="$t('mcp.disconnect')">
                <Unplug :size="14" />
              </button>
              <button @click="refreshTools(item)" :disabled="item._refreshing"
                class="p-1.5 text-gray-500 hover:bg-[var(--dropdown-hover-color)] hover:text-[#1D2129] rounded-lg transition-colors" :title="$t('mcp.refreshTools')">
                <Loader2 v-if="item._refreshing" :size="14" class="animate-spin" />
                <RefreshCw v-else :size="14" />
              </button>

              <button @click="openTest(item)" class="p-1.5 text-gray-500 hover:bg-[var(--dropdown-hover-color)] hover:text-[#1D2129] rounded-lg transition-colors" :title="$t('mcp.test')">
                <TestTube :size="14" />
              </button>
              <button @click="openEditServer(item)" class="p-1.5 text-gray-650 hover:bg-[var(--dropdown-hover-color)] hover:text-[#1D2129] rounded-lg transition-colors" :title="$t('mcp.edit')">
                <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
                <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
                </svg>
              </button>
              <button @click="remove(item)" class="p-1.5 text-gray-500 hover:bg-[var(--dropdown-hover-color)] hover:text-red-600 rounded-lg transition-colors" :title="$t('mcp.delete')">
                <Trash2 :size="14" />
              </button>
            </div>
          </div>
        </div>

      </div>

      <!-- List View -->
      <div v-else class="mcp-list-scroll">
        <div class="mcp-list-inner">
          <div v-for="item in filtered" :key="item.id"
            class="group border-b last:border-0 transition-colors mcp-table-row">

            <div class="mcp-list-grid px-6 py-4 cursor-pointer" @click="toggleExpand(item.id)">
              <!-- Icon & Status -->
              <div class="relative mcp-list-icon">
                <div class="w-10 h-10 rounded-lg bg-transparent flex items-center justify-center text-lg">
                  <img :src="protocolIcon(item.transport_type)" class="w-5 h-5" />
                </div>
                <div class="absolute -bottom-1 -right-1 w-4 h-4 rounded-full border-2 border-white flex items-center justify-center"
                  :class="item.last_status === 'running' ? 'bg-emerald-500' : item.last_status === 'connecting' ? 'bg-yellow-500' : item.last_status === 'error' ? 'bg-rose-500' : 'bg-gray-300'">
                  <Check v-if="item.last_status === 'running'" :size="8" class="text-white" />
                </div>
              </div>

              <!-- Name -->
              <div class="mcp-list-col mcp-list-name min-w-0">
                <div class="flex items-center gap-2 min-w-0">
                  <span class="font-bold text-[#1D2129] text-sm truncate" :title="item.name">{{ item.name }}</span>
                  <span class="text-sm px-1.5 py-0.5 rounded text-gray-500 border transport-type-tag flex-shrink-0">{{ protocolLabel(item.transport_type) }}</span>
                </div>
                <div class="text-sm truncate mt-0.5" :title="item.description || $t('mcp.noDesc')">{{ item.description || $t('mcp.noDesc') }}</div>
              </div>

              <!-- Source -->
              <div class="mcp-list-col mcp-list-meta">
                <div class="text-sm text-[#86909C]">{{ $t('mcp.source') }}</div>
                <div class="text-sm font-medium truncate">{{ sourceLabel(item) }}</div>
              </div>

              <!-- Tool count -->
              <div class="mcp-list-col mcp-list-meta">
                <div class="text-sm text-[#86909C]">{{ $t('mcp.toolCount') }}</div>
                <div class="text-sm font-bold text-[#1D2129]">{{ item.tool_count || 0 }}</div>
              </div>

              <!-- Actions -->
              <div class="mcp-list-actions flex justify-end items-center gap-2 transition-opacity group-hover:!bg-transparent" style="background: transparent !important;" @click.stop>
                <button v-if="item.last_status !== 'running'" @click="connect(item)" :disabled="item._connecting"
                  class="px-3 py-1.5 text-sm border border-gray-200 text-[#1D2129] hover:bg-[var(--dropdown-hover-color)] hover:text-[#2B53E5] hover:border-[#2B53E5] rounded-lg font-medium transition-all shadow-sm whitespace-nowrap">
                  {{ $t('mcp.connect') }}
                </button>
                <button v-else @click="disconnect(item)"
                  class="px-3 py-1.5 text-sm bg-white border border-gray-200 text-gray-500 hover:text-red-600 hover:border-red-200 rounded-lg font-medium transition-all shadow-sm whitespace-nowrap">
                  {{ $t('mcp.disconnect') }}
                </button>
                <button @click="openTest(item)" class="p-1.5 text-gray-900 hover:text-[#2B53E5] hover:bg-blue-50/20 rounded-lg transition-colors flex-shrink-0">
                  <TestTube :size="16" />
                </button>
                <button @click="remove(item)" class="p-1.5 text-gray-900 hover:text-red-600 hover:bg-red-50/20 rounded-lg transition-colors flex-shrink-0">
                  <Trash2 :size="16" />
                </button>
                <button @click="toggleExpand(item.id)" class="p-1.5 text-gray-900 hover:bg-gray-50/20 rounded-lg transition-colors flex-shrink-0">
                  <component :is="expandedServer === item.id ? ChevronDown : ChevronRight" :size="16" />
                </button>
              </div>
            </div>

            <!-- Expanded Tools List -->
            <div v-if="expandedServer === item.id" class="border-t border-gray-100 px-6 py-4 pl-[72px]">
              <div class="flex items-center justify-between mb-3">
                <span class="text-sm font-bold text-[#86909C] uppercase tracking-wider">{{ $t('mcp.toolList') }}</span>
                <button @click="refreshTools(item)" :disabled="item._refreshing"
                  class="text-sm text-[#86909C] hover:text-[#2345C5] flex items-center gap-1 font-medium">
                  <RefreshCw :size="12" :class="{ 'animate-spin': item._refreshing }" /> {{ $t('mcp.refreshList') }}
                </button>
              </div>

              <div v-if="item._tools?.length" class="grid grid-cols-2 gap-3">
                <div v-for="tool in item._tools" :key="tool.name"
                  class="flex items-center gap-3 bg-white px-3 py-2.5 rounded-xl border border-gray-100 shadow-sm">
                  <div class="w-8 h-8 rounded-lg bg-blue-50 text-blue-600 flex items-center justify-center flex-shrink-0">
                    <Wrench :size="14" />
                  </div>
                  <div class="flex-1 min-w-0">
                    <div class="text-xs font-bold text-[#1D2129] truncate">{{ tool.name }}</div>
                    <div class="text-[var(--font-size-xs)] text-[#86909C] truncate">{{ tool.description }}</div>
                  </div>
                </div>
              </div>
              <div v-else class="py-4 text-center border-2 border-dashed border-gray-200 rounded-xl">
                <p class="text-x">{{ item.last_status === 'running' ? $t('mcp.noToolsFound') : $t('mcp.noToolsHint') }}</p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    </div>

    <!-- Create Dialog -->
    <Teleport to="body">
      <div v-if="showCreateDialog" class="fixed inset-0 z-50 flex items-center justify-center p-6" style="background: rgba(15, 23, 42, 0.15); backdrop-filter: blur(0px);">
        <div class="mcp-create-dialog w-full max-w-lg mx-4 max-h-[86vh] flex flex-col" :data-theme-id="themeStore.draftPreset.id" style="background: linear-gradient(135deg, rgba(255,255,255,0.06), transparent), var(--theme-dailog-color); color: var(--text-primary); border-radius: 18px; border: 1px solid var(--border-color); box-shadow: 0 24px 70px rgba(15, 23, 42, 0.24);">
          <div class="flex items-center justify-between px-6 py-4 flex-shrink-0" style="border-bottom: 1px solid var(--border-color);">
            <h2 style="color: var(--text-primary); font-size: var(--font-size-lg); font-weight: 700; margin: 0;">{{ $t('mcp.addMcpServer') }}</h2>
            <button @click="showCreateDialog = false"
              class="p-2 rounded-lg transition-colors" style="color: var(--text-secondary);"
              @mouseenter="$event.currentTarget.style.background='var(--bg-hover)';$event.currentTarget.style.color='var(--text-primary)'"
              @mouseleave="$event.currentTarget.style.background='transparent';$event.currentTarget.style.color='var(--text-secondary)'">
              <X :size="20" />
            </button>
          </div>
          <div class="flex-1 overflow-y-auto p-6 space-y-4">
            <div>
              <label class="text-[var(--text-primary)] font-medium">{{ $t('mcp.name') }} <span class="text-red-400">*</span></label>
              <input v-model="form.name" class="w-full px-3 py-2 text border border-gray-200 rounded-lg focus:ring-2 focus:ring-[var(--color-primary)] outline-none" placeholder="e.g. everything-server" />
            </div>
            <div>
              <div class="flex items-center justify-between mb-1">
                <label class="block text font-medium ">{{ $t('mcp.description') }}</label>
                <AiDescriptionButton
                  entity-type="mcp_server"
                  :context="{ name: form.name, current_description: form.description, transport_type: form.transport_type, command: form.command }"
                  v-model="form.description"
                />
              </div>
              <input v-model="form.description" class="w-full px-3 py-2 text border border-gray-200 rounded-lg outline-none" :placeholder="$t('mcp.descPlaceholder')" />
            </div>
            <div>
              <label class="block text font-medium mb-1">{{ $t('mcp.transportType') }}</label>
              <el-select v-model="form.transport_type" style="width: 100%;">
                <el-option value="stdio" :label="$t('mcp.stdio')" />
                <el-option value="sse" :label="$t('mcp.sse')" />
                <el-option value="streamable_http" label="Streamable HTTP" />
                <el-option value="websocket" label="WebSocket" />
              </el-select>
            </div>
            <div v-if="form.transport_type === 'stdio'">
              <label class="block text font-medium mb-1">{{ $t('mcp.command') }}</label>
              <input v-model="form.command" class="w-full px-3 py-2 text border border-gray-200 rounded-lg outline-none font-mono" placeholder="npx" />
              <label class="block text font-medium mb-1 mt-2">{{ $t('mcp.args') }}</label>
              <textarea v-model="form.args" rows="3" class="w-full px-3 py-2 text border border-gray-200 rounded-lg outline-none font-mono resize-none " placeholder="-y
@modelcontextprotocol/server-everything" />
            </div>
            <div v-else class="space-y-4">
              <div>
                <label class="block text font-medium mb-1">{{ $t('mcp.url') }}</label>
                <input v-model="form.sse_url" class="w-full px-3 py-2 text-sm border rounded-lg outline-none font-mono" style="border-color: var(--border-color)" placeholder="http://localhost:3456/api/mcp/xxx" />
              </div>

              <!-- 认证（仅 HTTP 传输） -->
              <div>
                <label class="block text font-medium mb-1">{{ $t('mcp.authType') }}</label>
                <el-select v-model="form.auth_type" style="width: 100%;">
                  <el-option value="none" :label="$t('mcp.authNone')" />
                  <el-option value="bearer" :label="$t('mcp.authBearer')" />
                  <el-option value="headers" :label="$t('mcp.authHeaders')" />
                </el-select>
                <p class="text-xs text-gray-400 mt-1">{{ $t('mcp.authHint') }}</p>
              </div>

              <div v-if="form.auth_type === 'bearer'">
                <label class="block text font-medium mb-1">{{ $t('mcp.authToken') }}</label>
                <input v-model="form.auth_token" type="password" autocomplete="off"
                  class="w-full px-3 py-2 text-sm border rounded-lg outline-none font-mono" style="border-color: var(--border-color)"
                  :placeholder="$t('mcp.authTokenPlaceholder')" />
              </div>

              <div v-else-if="form.auth_type === 'headers'">
                <div class="flex items-center justify-between mb-1.5">
                  <label class="block text font-medium mb-1">{{ $t('mcp.authHeadersLabel') }}</label>
                  <button type="button" @click="addHeaderRow"
                    class="flex items-center gap-1 text hover:opacity-80 font-medium" style="color:var(--btn-primary-bg)">
                    <Plus :size="12" /> {{ $t('mcp.addHeader') }}
                  </button>
                </div>
                <div v-if="headerRows.length" class="space-y-1.5">
                  <div v-for="(row, i) in headerRows" :key="i" class="flex items-center gap-2">
                    <input v-model="row.key"
                      class="w-2/5 px-2.5 py-1.5 text font-mono border border-gray-200 rounded-lg outline-none focus:border-[var(--color-primary)]"
                      placeholder="Authorization" />
                    <span class="text-gray-300 text-xs">:</span>
                    <input v-model="row.value"
                      class="flex-1 px-2.5 py-1.5 text-xs font-mono border border-gray-200 rounded-lg outline-none focus:border-[var(--color-primary)]"
                      placeholder="Bearer xxxxx" />
                    <button type="button" @click="removeHeaderRow(i)"
                      class="p-1 text-gray-300 hover:text-red-500 rounded transition flex-shrink-0">
                      <Minus :size="13" />
                    </button>
                  </div>
                </div>
                <p v-else class="text text-gray-400">{{ $t('mcp.noHeaders') }}</p>
              </div>
            </div>

            <!-- Env vars -->
            <div>
              <div class="flex items-center justify-between mb-1.5">
                <label class="block text font-medium mb-1">{{ $t('mcp.envVars') }}</label>
                <button type="button" @click="addEnvRow"
                  class="flex items-center gap-1 text hover:opacity-80 font-medium" style="color:var(--btn-primary-bg)">
                  <Plus :size="12" /> {{ $t('mcp.addEnv') }}
                </button>
              </div>
              <div v-if="envRows.length" class="space-y-1.5">
                <div v-for="(row, i) in envRows" :key="i" class="flex items-center gap-2">
                  <input v-model="row.key"
                    class="w-2/5 px-2.5 py-1.5 text font-mono border border-gray-200 rounded-lg outline-none focus:border-[var(--color-primary)]"
                    placeholder="MYSQL_HOST" />
                  <span class="text-gray-300 text-xs">=</span>
                  <input v-model="row.value"
                    class="flex-1 px-2.5 py-1.5 text-xs font-mono border border-gray-200 rounded-lg outline-none focus:border-[var(--color-primary)]"
                    placeholder="localhost" />
                  <button type="button" @click="removeEnvRow(i)"
                    class="p-1 text-gray-300 hover:text-red-500 rounded transition flex-shrink-0">
                    <Minus :size="13" />
                  </button>
                </div>
              </div>
              <p v-else class="text text-gray-400">{{ $t('mcp.noEnvVars') }}</p>
            </div>
            <div>
              <label class="block text font-medium mb-1">{{ $t('mcp.keywords') }}</label>
              <input v-model="form.keywords" class="w-full px-3 py-2 text-sm border border-gray-200 rounded-lg outline-none" placeholder="search, web, file" />
            </div>
            <div class="flex items-center gap-6">
              <div class="flex items-center gap-2 cursor-pointer select-none" @click="form.auto_start = !form.auto_start">
                <component :is="form.auto_start ? ToggleRight : ToggleLeft"
                  :size="20"
                  :class="form.auto_start ? 'text-[var(--color-primary)]' : 'text-gray-300'" />
                <span class="text-sm ">{{ $t('mcp.autoConnect') }}</span>
              </div>
              <div class="flex items-center gap-2 cursor-pointer select-none" @click="form.auto_index = !form.auto_index">
                <component :is="form.auto_index ? ToggleRight : ToggleLeft"
                  :size="20"
                  :class="form.auto_index ? 'text-[var(--color-primary)]' : 'text-gray-300'" />
                <span class="text-sm">{{ $t('mcp.autoIndex') }}</span>
              </div>
            </div>
          </div>
          <div class="flex justify-end gap-3 px-6 py-4 border-t border-gray-100  rounded-b-2xl flex-shrink-0">
            <button @click="showCreateDialog = false"
              style="background: var(--cancel-btn-color); color: var(--btn-secondary-text); border: 1px solid var(--btn-secondary-border); min-width: 76px; height: 36px; padding: 0 16px; border-radius: 4px; cursor: pointer; font-size: var(--font-size-base); font-weight: 600; box-sizing: border-box;"
              @mouseenter="$event.currentTarget.style.background='var(--cancel-btn-hover-color)';$event.currentTarget.style.color='var(--btn-secondary-hover-text)';$event.currentTarget.style.borderColor='var(--btn-secondary-hover-border)'"
              @mouseleave="$event.currentTarget.style.background='var(--cancel-btn-color)';$event.currentTarget.style.color='var(--btn-secondary-text)';$event.currentTarget.style.borderColor='var(--btn-secondary-border)'">{{ $t('common.cancel') }}</button>
            <button @click="create" :disabled="!form.name"
              style="background: var(--btn-primary-bg); color: var(--btn-primary-text); border: 1px solid var(--btn-primary-border); min-width: 76px; height: 36px; padding: 0 16px; border-radius: 4px; cursor: pointer; font-size: var(--font-size-base); font-weight: 600;"
              @mouseenter="$event.currentTarget.style.background='var(--btn-primary-hover-bg)';$event.currentTarget.style.color='var(--btn-primary-hover-text)';$event.currentTarget.style.borderColor='var(--btn-primary-hover-border)'"
              @mouseleave="$event.currentTarget.style.background='var(--btn-primary-bg)';$event.currentTarget.style.color='var(--btn-primary-text)';$event.currentTarget.style.borderColor='var(--btn-primary-border)'">
              <Check :size="14" class="inline mr-1" /> {{ $t('mcp.add') }}
            </button>
          </div>
        </div>
      </div>
    </Teleport>

    <!-- Test Tool Dialog -->
    <Teleport to="body">
      <div v-if="showTestDialog" class="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
        <div class="mcp-test-dialog rounded-2xl shadow-2xl w-full max-w-lg mx-4" style="background: var(--theme-dailog-color); color: var(--text-primary);">
          <div class="flex items-center justify-between px-6 py-4 border-b" style="border-color: var(--border-color);">
            <h2 class="text-lg font-bold ">{{ $t('mcp.testToolTitle', { name: testingServer?.name }) }}</h2>
            <button @click="showTestDialog = false"
              class="p-2 rounded-lg transition-colors" style="color: var(--text-secondary);"
              @mouseenter="$event.currentTarget.style.background='var(--bg-hover)';$event.currentTarget.style.color='var(--text-primary)'"
              @mouseleave="$event.currentTarget.style.background='transparent';$event.currentTarget.style.color='var(--text-secondary)'">
              <X :size="20" />
            </button>
          </div>
          <div class="p-6 space-y-4">
            <div class="test-tool-form-group">
              <label class="block text-sm font-medium  mb-1">{{ $t('mcp.toolName') }}</label>
              <el-select v-model="testForm.tool_name" @change="initTestFields()" class="w-full" style="width: 100%;">
                <el-option v-for="t in testingServer?._tools || []" :key="t.name" :value="t.name" :label="t.name" />
              </el-select>
            </div>
            <!-- ★ Issue #5a: 表单/JSON 模式切换 -->
            <div class="flex items-center gap-2">
              <button @click="testMode = 'form'" class="px-3 py-1 text-sm rounded" :class="testMode === 'form' ? 'bg-[var(--color-primary)] text-white' : ' test-mode-btn'">{{ $t('mcp.formMode') }}</button>
              <button @click="testMode = 'json'" class="px-3 py-1 text-sm rounded" :class="testMode === 'json' ? 'bg-[var(--color-primary)] text-white' : ' test-mode-btn'">{{ $t('mcp.jsonMode') }}</button>
            </div>
            <!-- 表单模式 -->
            <div v-if="testMode === 'form' && testFormFields.length > 0" class="space-y-3">
              <div v-for="field in testFormFields" :key="field.key">
                <label class="block text-sm font-medium  mb-1">
                  {{ field.key }}
                  <span v-if="field.required" class="text-red-400">*</span>
                  <span class="text-gray-400 font-normal ml-1">({{ field.type }})</span>
                </label>
                <p v-if="field.description" class="text-[var(--font-size-xs)] text-gray-400 mb-1">{{ field.description }}</p>
                <select v-if="field.enum" v-model="testFieldValues[field.key]" class="w-full px-3 py-2 text-sm border border-gray-200 rounded-lg outline-none bg-white">
                  <option v-for="opt in field.enum" :key="opt" :value="opt">{{ opt }}</option>
                </select>
                <input v-else-if="field.type === 'boolean'" type="checkbox" v-model="testFieldValues[field.key]" class="rounded" />
                <input v-else-if="field.type === 'integer' || field.type === 'number'" type="number" v-model="testFieldValues[field.key]" class="w-full px-3 py-2 text-sm border border-gray-200 rounded-lg outline-none" :placeholder="field.default ?? ''" />
                <input v-else v-model="testFieldValues[field.key]" class="w-full px-3 py-2 text-sm border border-gray-200 rounded-lg outline-none" :placeholder="field.default ?? ''" />
              </div>
            </div>
            <div v-else-if="testMode === 'form'" class="text-sm  text-center py-4">{{ $t('mcp.noParamSchema') }}</div>
            <!-- JSON 模式 -->
            <div v-if="testMode === 'json'">
              <label class="block text-sm font-medium mb-1">{{ $t('mcp.paramsJson') }}</label>
              <textarea v-model="testForm.arguments" rows="4" class="w-full px-3 py-2 text-xs font-mono rounded-lg border  outline-none resize-none bg-gray-50" style="border-color: var(--border-color); " placeholder='{ "query": "test" }' />
            </div>
            <button @click="runTest"
            class="runTestBtn"
            >
              <Zap :size="14" /> {{ $t('mcp.executeTest') }}
            </button>
            <!-- Result -->
            <div v-if="testResult" class="border rounded-lg p-4" style="background: var(--theme-dailog-color); border-color: var(--border-color);">
              <div class="flex items-center gap-2 mb-2">
                <span :class="testResult.success ? 'text-green-700' : 'text-red-700'" class="text-sm font-medium">
                  {{ testResult.success ? '✅ ' + $t('mcp.success') : '❌ ' + $t('mcp.failed') }}
                </span>
                <span class="text-xs ">{{ testResult.duration_ms }}ms</span>
              </div>
              <pre v-if="testResult.result" class="text-xs font-mono bg-white/80 p-2 rounded overflow-x-auto max-h-48">{{ JSON.stringify(testResult.result, null, 2) }}</pre>
              <div v-if="testResult.error" class="text-xs text-red-600 font-mono">{{ testResult.error }}</div>
            </div>
          </div>
        </div>
      </div>
    </Teleport>

    <!-- ★ Issue #5b: MCP Server 编辑弹窗 -->
    <Teleport to="body">
      <div v-if="showEditDialog" class="fixed inset-0 z-50 flex items-center justify-center p-6" style="background: rgba(15, 23, 42, 0.15);">
        <div class="mcp-edit-dialog w-full max-w-lg mx-4 max-h-[86vh] flex flex-col" :data-theme-id="themeStore.draftPreset.id" style="background: linear-gradient(135deg, rgba(255,255,255,0.06), transparent), var(--theme-dailog-color); color: var(--text-primary); border-radius: 18px; border: 1px solid var(--border-color); box-shadow: 0 24px 70px rgba(15, 23, 42, 0.24);">
          <div class="flex items-center justify-between px-6 py-4 flex-shrink-0" style="border-bottom: 1px solid var(--border-color);">
            <h2 style="color: var(--text-primary); font-size: var(--font-size-lg); font-weight: 700; margin: 0;">{{ $t('mcp.editServerTitle', { name: editingServer?.name }) }}</h2>
            <button @click="showEditDialog = false"
              class="p-2 rounded-lg transition-colors" style="color: var(--text-secondary);"
              @mouseenter="$event.currentTarget.style.background='var(--bg-hover)';$event.currentTarget.style.color='var(--text-primary)'"
              @mouseleave="$event.currentTarget.style.background='transparent';$event.currentTarget.style.color='var(--text-secondary)'">
              <X :size="20" />
            </button>
          </div>
          <div class="flex-1 overflow-y-auto p-6 space-y-4">
            <!-- Name -->
            <div>
              <label class="block text font-medium mb-1">{{ $t('mcp.name') }}</label>
              <input v-model="editForm.name" class="w-full px-3 py-2 text border border-gray-200 rounded-lg outline-none focus:ring-2 focus:ring-[var(--color-primary)]" style="border-color: var(--border-color);" />
            </div>
            <!-- Description -->
            <div>
              <label class="block text font-medium mb-1">{{ $t('mcp.description') }}</label>
              <textarea v-model="editForm.description" rows="2" class="w-full px-3 py-2 text border rounded-lg outline-none resize-none" style="border-color: var(--border-color);" />
            </div>
            <!-- Transport Type -->
            <div>
              <label class="block text font-medium mb-1">{{ $t('mcp.transportType') }}</label>
              <el-select v-model="editForm.transport_type" style="width: 100%;">
                <el-option value="stdio" :label="$t('mcp.stdio')" />
                <el-option value="sse" :label="$t('mcp.sse')" />
                <el-option value="streamable_http" label="Streamable HTTP" />
                <el-option value="websocket" label="WebSocket" />
              </el-select>
            </div>
            <!-- Command + Args (stdio) -->
            <template v-if="editForm.transport_type === 'stdio'">
              <div>
                <label class="block text font-medium mb-1">{{ $t('mcp.command') }}</label>
                <input v-model="editForm.command" class="w-full px-3 py-2 text border border-gray-200 rounded-lg outline-none font-mono" style="border-color: var(--border-color);" placeholder="npx" />
              </div>
              <div>
                <label class="block text font-medium mb-1">{{ $t('mcp.args') }}</label>
                <textarea v-model="editForm.args" rows="3" class="w-full px-3 py-2 text border border-gray-200 rounded-lg outline-none font-mono resize-none" style="border-color: var(--border-color);" placeholder="-y
@modelcontextprotocol/server-filesystem
/tmp" />
              </div>
            </template>
            <!-- URL + 认证 (sse/ws) -->
            <template v-else>
              <div>
                <label class="block text font-medium mb-1">{{ $t('mcp.url') }}</label>
                <input v-model="editForm.sse_url" class="w-full px-3 py-2 text border border-gray-200 rounded-lg outline-none font-mono" style="border-color: var(--border-color);" placeholder="http://localhost:3456/api/mcp/xxx" />
              </div>
              <div>
                <label class="block text font-medium mb-1">{{ $t('mcp.authType') }}</label>
                <el-select v-model="editForm.auth_type" style="width: 100%;">
                  <el-option value="none" :label="$t('mcp.authNone')" />
                  <el-option value="bearer" :label="$t('mcp.authBearer')" />
                  <el-option value="headers" :label="$t('mcp.authHeaders')" />
                </el-select>
                <p class="text-xs text-gray-400 mt-1">{{ $t('mcp.authHint') }}</p>
              </div>
              <div v-if="editForm.auth_type === 'bearer'">
                <label class="block text font-medium mb-1">{{ $t('mcp.authToken') }}</label>
                <input v-model="editForm.auth_token" type="password" autocomplete="off"
                  class="w-full px-3 py-2 text border border-gray-200 rounded-lg outline-none font-mono" style="border-color: var(--border-color);"
                  :placeholder="$t('mcp.authTokenPlaceholder')" />
              </div>
              <div v-else-if="editForm.auth_type === 'headers'">
                <div class="flex items-center justify-between mb-1.5">
                  <label class="block text font-medium mb-1">{{ $t('mcp.authHeadersLabel') }}</label>
                  <button type="button" @click="editHeaderRows.push({ key: '', value: '' })"
                    class="flex items-center gap-1 text hover:opacity-80 font-medium" style="color:var(--btn-primary-bg)">
                    <Plus :size="12" /> {{ $t('mcp.addHeader') }}
                  </button>
                </div>
                <div v-if="editHeaderRows.length" class="space-y-1.5">
                  <div v-for="(row, i) in editHeaderRows" :key="i" class="flex items-center gap-2">
                    <input v-model="row.key"
                      class="w-2/5 px-2.5 py-1.5 text font-mono border border-gray-200 rounded-lg outline-none focus:border-[var(--color-primary)]"
                      placeholder="Authorization" />
                    <span class="text-gray-300 text-xs">:</span>
                    <input v-model="row.value"
                      class="flex-1 px-2.5 py-1.5 text-xs font-mono border border-gray-200 rounded-lg outline-none focus:border-[var(--color-primary)]"
                      placeholder="Bearer xxxxx" />
                    <button type="button" @click="editHeaderRows.splice(i, 1)"
                      class="p-1 text-gray-300 hover:text-red-500 rounded transition flex-shrink-0">
                      <Minus :size="13" />
                    </button>
                  </div>
                </div>
                <p v-else class="text text-gray-400">{{ $t('mcp.noHeaders') }}</p>
              </div>
            </template>
            <!-- Env vars -->
            <div>
              <div class="flex items-center justify-between mb-1.5">
                <label class="block text font-medium mb-1">{{ $t('mcp.envVars') }}</label>
                <button type="button" @click="editEnvRows.push({ key: '', value: '' })"
                  class="flex items-center gap-1 text hover:opacity-80 font-medium" style="color:var(--btn-primary-bg)">
                  <Plus :size="12" /> {{ $t('mcp.addEnv') }}
                </button>
              </div>
              <div v-if="editEnvRows.length" class="space-y-1.5">
                <div v-for="(row, i) in editEnvRows" :key="i" class="flex items-center gap-2">
                  <input v-model="row.key"
                    class="w-2/5 px-2.5 py-1.5 text font-mono border rounded-lg outline-none"
                    style="border-color: var(--border-color); color: var(--text-primary);"
                    placeholder="MYSQL_HOST" />
                  <span class="text-gray-300 text-xs">=</span>
                  <input v-model="row.value"
                    class="flex-1 px-2.5 py-2.5 text-xs font-mono border rounded-lg outline-none"
                    style="border-color: var(--border-color); color: var(--text-primary);"
                    placeholder="localhost" />
                  <button type="button" @click="editEnvRows.splice(i, 1)"
                    class="p-1 text-gray-300 hover:text-red-500 rounded transition flex-shrink-0">
                    <Minus :size="13" />
                  </button>
                </div>
              </div>
              <p v-else class="block text font-medium mb-1">{{ $t('mcp.noEnvVars') }}</p>
            </div>
            <!-- Keywords -->
            <div>
              <label class="block text font-medium mb-1">{{ $t('mcp.keywords') }}</label>
              <input v-model="editForm.keywords" class="w-full px-3 py-2 text-sm border rounded-lg outline-none" style="border-color: var(--border-color); color: var(--text-primary);" placeholder="search, web, file" />
            </div>
            <!-- Toggles -->
            <div class="flex items-center gap-6">
              <div class="flex items-center gap-2 cursor-pointer select-none" @click="editForm.auto_start = !editForm.auto_start">
                <component :is="editForm.auto_start ? ToggleRight : ToggleLeft"
                  :size="20"
                  :class="editForm.auto_start ? 'text-[var(--color-primary)]' : 'text-gray-300'" />
                <span class="text-sm">{{ $t('mcp.autoConnect') }}</span>
              </div>
              <div class="flex items-center gap-2 cursor-pointer select-none" @click="editForm.auto_index = !editForm.auto_index">
                <component :is="editForm.auto_index ? ToggleRight : ToggleLeft"
                  :size="20"
                  :class="editForm.auto_index ? 'text-[var(--color-primary)]' : 'text-gray-300'" />
                <span class="text-sm">{{ $t('mcp.autoIndex') }}</span>
              </div>
            </div>
          </div>
          <div class="flex justify-end gap-3 px-6 py-4 border-t border-gray-100 bg-gray-50 rounded-b-2xl flex-shrink-0">
            <button @click="showEditDialog = false"
              style="background: var(--cancel-btn-color); color: var(--btn-secondary-text); border: 1px solid var(--btn-secondary-border); min-width: 76px; height: 36px; padding: 0 16px; border-radius: 4px; cursor: pointer; font-size: var(--font-size-base); font-weight: 600; box-sizing: border-box;"
              @mouseenter="$event.currentTarget.style.background='var(--cancel-btn-hover-color)';$event.currentTarget.style.color='var(--btn-secondary-hover-text)';$event.currentTarget.style.borderColor='var(--btn-secondary-hover-border)'"
              @mouseleave="$event.currentTarget.style.background='var(--cancel-btn-color)';$event.currentTarget.style.color='var(--btn-secondary-text)';$event.currentTarget.style.borderColor='var(--btn-secondary-border)'">{{ $t('common.cancel') }}</button>
            <button @click="saveEditServer"
              style="background: var(--btn-primary-bg); color: var(--btn-primary-text); border: 1px solid var(--btn-primary-border); min-width: 76px; height: 36px; padding: 0 16px; border-radius: 4px; cursor: pointer; font-size: var(--font-size-base); font-weight: 600;"
              @mouseenter="$event.currentTarget.style.background='var(--btn-primary-hover-bg)';$event.currentTarget.style.color='var(--btn-primary-hover-text)';$event.currentTarget.style.borderColor='var(--btn-primary-hover-border)'"
              @mouseleave="$event.currentTarget.style.background='var(--btn-primary-bg)';$event.currentTarget.style.color='var(--btn-primary-text)';$event.currentTarget.style.borderColor='var(--btn-primary-border)'">{{ $t('mcp.save') }}</button>
          </div>
        </div>
      </div>
    </Teleport>

    <!-- ══════════════════════════════════════════════════
         一键导入弹窗
    ══════════════════════════════════════════════════ -->
    <Teleport to="body">
      <div v-if="showImportDialog" class="fixed inset-0 z-50 flex items-center justify-center p-6" style="background: rgba(15, 23, 42, 0.15); backdrop-filter: blur(0px);">
        <div class="mcp-import-dialog w-full max-w-lg mx-4 flex flex-col max-h-[86vh]" :data-theme-id="themeStore.draftPreset.id" style="background: linear-gradient(135deg, rgba(255,255,255,0.06), transparent), var(--theme-dailog-color); color: var(--text-primary); border-radius: 18px; border: 1px solid var(--border-color); box-shadow: 0 24px 70px rgba(15, 23, 42, 0.24);">
          <!-- 头部 -->
          <div class="flex items-center justify-between px-6 py-4 flex-shrink-0" style="border-bottom: 1px solid var(--border-color);">
            <div class="flex items-center gap-2">
              <FileJson :size="18" class="text-blue-500" />
              <h2 style="color: var(--text-primary); font-size: var(--font-size-lg); font-weight: 700; margin: 0;">{{ $t('mcp.importTitle') }}</h2>
            </div>
            <button @click="showImportDialog = false"
              class="p-2 rounded-lg transition-colors" style="color: var(--text-secondary);"
              @mouseenter="$event.currentTarget.style.background='var(--bg-hover)';$event.currentTarget.style.color='var(--text-primary)'"
              @mouseleave="$event.currentTarget.style.background='transparent';$event.currentTarget.style.color='var(--text-secondary)'">
              <X :size="20" />
            </button>
          </div>

          <!-- 内容 -->
          <div class="flex-1 overflow-y-auto p-6 space-y-4">
            <!-- 提示 -->
            <p class="text-sm text-gray-500 leading-relaxed">
              {{ $t('mcp.importHint') }}
            </p>

            <!-- 文本域 -->
            <div>
              <textarea v-model="importText" rows="10"
                class="w-full min-h-[120px] max-h-[240px] px-3 py-2.5 text-sm font-mono border border-gray-200 rounded-xl outline-none  focus:none resize-y overflow-y-auto bg-gray-50 focus:bg-white transition"
                placeholder='{
  "mcpServers": {
    "mysql": {
      "command": "uv",
      "args": ["run", "mysql_mcp_server"],
      "env": {
        "MYSQL_HOST": "localhost",
        "MYSQL_PORT": "3306"
      }
    }
  }
}' />
            </div>

            <!-- 错误 -->
            <div v-if="importError" class="flex items-center gap-2 p-3 bg-red-50 border border-red-200 rounded-xl text-xs text-red-600">
              <AlertCircle :size="14" class="flex-shrink-0" />
              {{ importError }}
            </div>

            <!-- 多 Server 选择列表 -->
            <div v-if="importParsed.length" class="space-y-2">
              <div class="flex items-center justify-between">
                <p class="text-xs font-semibold text-gray-600">{{ $t('mcp.importParsed', { n: importParsed.length }) }}</p>
                <button @click="directImportAll" class="px-3 py-1 text-xs rounded-lg bg-green-500 text-white hover:bg-green-600 font-medium transition">
                  {{ $t('mcp.importAll') }}
                </button>
              </div>
              <div v-for="item in importParsed" :key="item.name"
                @click="selectImportServer(item)"
                class="flex items-center gap-3 px-4 py-3 rounded-xl border border-gray-200 hover:border-blue-400 hover:bg-blue-50/40 cursor-pointer transition-colors group">
                <div class="w-8 h-8 rounded-lg bg-blue-100 flex items-center justify-center flex-shrink-0 group-hover:bg-blue-200 transition-colors">
                  <Plug :size="14" class="text-blue-600" />
                </div>
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-semibold text-gray-800">{{ item.name }}</p>
                  <p class="text-xs text-gray-400 truncate">
                    {{ normalizeImportedTransportType(item.config) }}
                    {{ item.config.command ? '· ' + item.config.command : '' }}
                    {{ item.config.url ? '· ' + item.config.url : '' }}
                  </p>
                </div>
                <Check :size="14" class="text-blue-500 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0" />
              </div>
            </div>
          </div>

          <!-- 底部 -->
          <div class="flex justify-end gap-3 px-6 py-4 border-t border-gray-100 bg-gray-50 rounded-b-2xl flex-shrink-0">
            <button @click="showImportDialog = false"
              class="px-4 py-2 text-sm hover:bg-gray-200 rounded transition"
              style="background: var(--cancel-btn-color); color: var(--btn-secondary-text); border: 1px solid var(--btn-secondary-border); min-width: 76px; height: 36px; padding: 0 16px; border-radius: 4px; cursor: pointer; font-size: var(--font-size-base); font-weight: 600; box-sizing: border-box;"
              @mouseenter="$event.currentTarget.style.background='var(--cancel-btn-hover-color)';$event.currentTarget.style.color='var(--btn-secondary-hover-text)';$event.currentTarget.style.borderColor='var(--btn-secondary-hover-border)'"
              @mouseleave="$event.currentTarget.style.background='var(--cancel-btn-color)';$event.currentTarget.style.color='var(--btn-secondary-text)';$event.currentTarget.style.borderColor='var(--btn-secondary-border)'"
              >
              {{ $t('common.cancel') }}</button>
              <button @click="parseImport" :disabled="!importText.trim()"
              style="background:var(--btn-primary-bg)" class="px-5 py-2 text-sm text-white rounded disabled:opacity-50 font-medium transition flex items-center gap-1.5"
              @mouseenter="$event.currentTarget.style.background='var(--btn-primary-hover-bg)';$event.currentTarget.style.color='var(--btn-primary-hover-text)';$event.currentTarget.style.borderColor='var(--btn-primary-hover-border)'"
              @mouseleave="$event.currentTarget.style.background='var(--btn-primary-bg)';$event.currentTarget.style.color='var(--btn-primary-text)';$event.currentTarget.style.borderColor='var(--btn-primary-border)'"
              >
              <FileJson :size="14" /> {{ $t('mcp.parseAndFill') }}
            </button>
          </div>
        </div>
      </div>
    </Teleport>
  </div>

  <!-- Sync Toast -->
  <Teleport to="body">
    <Transition name="toast">
      <div v-if="syncToast" class="fixed bottom-6 right-6 z-[100] flex items-center gap-3 bg-gray-900 text-white px-4 py-3 rounded-xl shadow-xl text-sm">
        <span class="text-emerald-400">✅</span>
        <span v-html="$t('mcp.syncToast', { n: syncToast })"></span>
      </div>
    </Transition>
  </Teleport>

    <!-- Server Detail Sheet (右侧抽屉) -->
    <Teleport to="body">
      <div v-if="showDetailPanel && detailServer" class="fixed inset-0 z-50" style="background: rgba(15, 23, 42, 0.36);">
        <div class="fixed inset-y-0 right-0 w-[440px] max-w-[100vw] shadow-xl flex flex-col" :data-theme-id="themeStore.draftPreset.id" style="background: var(--theme-dailog-color); color: var(--text-primary);">
          <div class="flex items-center justify-between px-5 py-4 border-b flex-shrink-0" style="border-color: var(--border-color);">
            <div>
              <h2 class="text-lg font-bold flex items-center gap-2"><Server :size="18" />{{ detailServer.name }}</h2>
              <p class="text-sm mt-0.5" style="color: var(--text-secondary);">{{ protocolLabel(detailServer.transport_type) }} · {{ statusText(detailServer.last_status) }}</p>
            </div>
            <div class="flex items-center gap-1">
              <button @click="refreshTools(detailServer)" :disabled="detailServer._refreshing" class="p-1.5 rounded-lg hover:bg-[var(--dropdown-hover-color)]" :title="$t('mcp.refreshTools')" style="color: var(--text-secondary);">
                <RefreshCw v-if="!detailServer._refreshing" :size="15" /><Loader2 v-else :size="15" class="animate-spin" />
              </button>
              <button @click="showDetailPanel = false"
                class="p-2 rounded-lg transition-colors" style="color: var(--text-secondary);"
                @mouseenter="$event.currentTarget.style.background='var(--bg-hover)';$event.currentTarget.style.color='var(--text-primary)'"
                @mouseleave="$event.currentTarget.style.background='transparent';$event.currentTarget.style.color='var(--text-secondary)'">
                <X :size="20" />
              </button>
            </div>
          </div>
          <div v-if="detailLoading" class="flex-1 flex items-center justify-center"><Loader2 :size="24" class="animate-spin" style="color: var(--text-secondary);" /></div>
          <div v-else class="flex-1 overflow-y-auto p-5 space-y-5 text-sm">
            <div>
              <h3 class="text-md font-bold mb-2" style="color: var(--text-secondary);">{{ $t('mcp.serverConfig') }}</h3>
              <div class="space-y-2 p-3 rounded-xl text-xs" style="background: var(--bg-hover);">
                <div class="flex justify-between text-sm"><span style="color: var(--text-secondary);">{{ $t('mcp.transportType') }}</span><span class="font-medium">{{ protocolLabel(detailServer.transport_type) }}</span></div>
                <div v-if="detailServer.command" class="flex justify-between text-sm"><span style="color: var(--text-secondary);">{{ $t('mcp.command') }}</span><code class="font-mono text-sm">{{ detailServer.command }}</code></div>
                <div v-if="detailServer.args" class="flex justify-between text-sm"><span style="color: var(--text-secondary);">{{ $t('mcp.argsLabel') }}</span><code class="font-mono text-sm">{{ detailServer.args }}</code></div>
                <div v-if="detailServer.url" class="flex justify-between text-sm"><span style="color: var(--text-secondary);">URL</span><code class="font-mono text-sm truncate max-w-[200px]">{{ detailServer.url }}</code></div>
                <div class="flex justify-between text-sm"><span style="color: var(--text-secondary);">{{ $t('mcp.connectionStatus') }}</span><span :class="detailServer.last_status === 'running' ? 'text-emerald-600' : detailServer.last_status === 'connecting' ? 'text-yellow-600' : detailServer.last_status === 'error' ? 'text-red-600' : 'text-gray-400'" class="font-medium">{{ statusText(detailServer.last_status) }}</span></div>
              </div>
            </div>
            <div>
              <h3 class="text-md font-bold mb-2 flex items-center gap-1" style="color: var(--text-secondary);"><Wrench :size="12" />{{ $t('mcp.discoveredTools') }}（{{ detailTools.length || detailServer.tool_count || 0 }}）</h3>
              <div v-if="detailTools.length === 0" class="text-sm p-4 text-center rounded-xl" style="color: var(--text-secondary); background: var(--bg-hover);">{{ detailServer.last_status === 'running' ? $t('mcp.clickRefreshHint') : $t('mcp.connectFirstHint') }}</div>
              <div v-else class="space-y-1 max-h-72 overflow-y-auto">
                <div v-for="t in detailTools" :key="t.name" class="flex items-center justify-between p-2 rounded-lg text-xs" style="background: var(--bg-hover);"><span class="font-mono font-medium">{{ t.name }}</span></div>
              </div>
            </div>
          </div>
          <div class="px-5 py-3 border-t flex items-center gap-2 flex-shrink-0" style="border-color: var(--border-color);">
            <button v-if="detailServer.last_status !== 'running'" @click="connect(detailServer)" class="flex-1 flex items-center justify-center gap-1.5 px-3 py-3 bg-[var(--color-primary)] text-white text-sm rounded hover:bg-[var(--btn-primary-hover-bg)] font-medium"><Plug :size="13" />{{ $t('mcp.connect') }}</button>
            <button v-else @click="disconnect(detailServer)" class="flex-1 flex items-center justify-center gap-1.5 px-3 py-3 bg-red-50 text-red-600 text-xs rounded hover:bg-red-100 font-medium"><Unplug :size="13" />{{ $t('mcp.disconnect') }}</button>
            <button @click="showDetailPanel = false; openEditServer(detailServer)" class="flex items-center gap-1.5 px-3 py-3 text-white text-sm rounded font-medium bg-[var(--color-primary)] hover:bg-[var(--btn-primary-hover-bg)] " ><Settings :size="13" />{{ $t('mcp.edit') }}</button>
          </div>
        </div>
      </div>
    </Teleport>
</template>

<style scoped>
button { cursor: pointer; }
.toast-enter-active, .toast-leave-active { transition: all 0.3s ease; }
.toast-enter-from, .toast-leave-to { opacity: 0; transform: translateY(12px); }

.mcp-loading-state {
  background: transparent !important;
}

.mcp-page-header {
  container-type: inline-size;
  display: flex;
  align-items: flex-start;
  flex-wrap: wrap;
  gap: 12px;
  margin-bottom: 12px;

  .mcp-header-left {
    flex: 1 1 auto;
    min-width: 0;
  }

  .mcp-header-actions {
    flex: 1 1 360px;
    margin-left: auto;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
    min-width: 0;
    max-width: 100%;
  }

  .mcp-header-btn {
    flex-shrink: 0;
    white-space: nowrap;
  }

  @container (max-width: 780px) {
    .mcp-header-left {
      flex: 1 1 100%;
      width: 100%;
    }

    .mcp-header-actions {
      flex: 1 1 100%;
      width: 100%;
      margin-left: 0;
      justify-content: flex-start;
    }
  }
}

.mcp-toolbar {
  container-type: inline-size;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 24px;
  padding: 0;

  @container (max-width: 720px) {
    .mcp-toolbar-main {
      flex: 1 1 100%;
      width: 100%;
    }

    .mcp-toolbar-search {
      flex: 1 1 100%;
      max-width: 100%;
    }

    .mcp-toolbar-view {
      flex: 1 1 100%;
      width: 100%;
      margin-left: 0;
      justify-content: flex-start;
    }
  }
}

.mcp-toolbar-main {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
  flex: 1 1 520px;
  min-width: 0;
}

.mcp-toolbar-search {
  position: relative;
  flex: 1 1 220px;
  min-width: 180px;
  max-width: 28rem;
}

.mcp-toolbar-divider {
  width: 1px;
  height: 24px;
  background: var(--border-color, #e5e7eb);
  flex-shrink: 0;
}

.mcp-toolbar-filters {
  display: flex;
  flex-wrap: nowrap;
  align-items: center;
  gap: 12px;
  flex-shrink: 0;

  .tools-filter-btn {
    white-space: nowrap;
  }
}

.mcp-toolbar-view {
  display: flex;
  flex: 0 1 auto;
  margin-left: auto;

  button {
    white-space: nowrap;
  }
}

.mcp-stats-row {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 20px;
  margin-bottom: 16px;
}

.mcp-stats-row > div {
  min-width: 0;
  white-space: nowrap;
}

.mcp-card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 12px;
}

.mcp-card-grid > div {
  min-width: 0;
}

</style>

<style>
/* MCP 页面暗色主题样式（限定 .mcp-page 避免影响其它页面） */
.dark .mcp-page [class*="rounded-2xl"][class*="shadow-2xl"] { background: #1e1e24 !important; }
.dark .mcp-page .bg-white {
  background: rgba(255, 255, 255, 0.15) !important;
  border-color: rgba(255, 255, 255, 0.1) !important;
}
.dark .mcp-page .el-dialog,
.mcp-title { color: #1f2937; }
.mcp-subtitle { color: #6b7280; }
.dark .mcp-page .mcp-title { color: #ffffff !important; }
.dark .mcp-page .mcp-subtitle { color: #cbd5e0 !important; }
.dark .mcp-page .mcp-stats-row .text-3xl {
  color: #ffffff !important;
}
.dark .mcp-page .mcp-stats-row .text-xs,
.dark .mcp-page .mcp-stats-row .text-base {
  color: #cbd5e0 !important;
}
.dark .mcp-page .el-empty__description,
.dark .mcp-page input,
.dark .mcp-page .el-input__wrapper,
.dark .mcp-page .bg-red-50 {
  background: rgba(255, 255, 255, 0.15) !important;
}
.dark .mcp-page .mcp-toolbar-search input {
  background: transparent !important;
  background-color: transparent !important;
}
.dark .mcp-page .bg-red-50 {
  border-color: rgba(255, 100, 100, 0.3) !important;
  color: #fca5a5 !important;
}

.dark .mcp-page [class*="rounded-2xl shadow-2xl"] {
  color: #ffffff;
}
.dark .mcp-page .rounded-2xl h2,
.dark .mcp-page .rounded-2xl h3 {
  color: #ffffff;
}
.dark .mcp-page .rounded-2xl label {
  color: #cbd5e0;
}
.dark .mcp-page .rounded-2xl .text-sm {
  color: #cbd5e0;
}
.dark .mcp-page .rounded-2xl .text-gray-600 {
  color: #cbd5e0 !important;
}
.dark .mcp-page .rounded-2xl .hover\:bg-gray-100:hover {
  background: rgba(255, 255, 255, 0.1) !important;
}
.dark .mcp-page .rounded-2xl input,
.dark .mcp-page .rounded-2xl textarea,
.dark .mcp-page .rounded-2xl select {
  background: rgba(255,255,255,0.08);
  color: #fff;
  border-color: rgba(255,255,255,0.15);
}

.dark .mcp-page [class*="rounded-2xl"] .border-t {
  background: #1e1e24;
  border-color: rgba(255,255,255,0.1);
}
.dark .mcp-page [class*="rounded-2xl"] select option {
  background: #1a1a1a;
  color: #fff;
}
.dark .mcp-page [class*="rounded-2xl"] .flex.justify-end {
  background: #1e1e24;
}

.dark .mcp-page [class*="rounded-2xl"] .hover\:bg-gray-100:hover {
  background: rgba(255, 255, 255, 0.15) !important;
  color: #fff !important;
}
.dark .mcp-page [class*="rounded-2xl"] .hover\:bg-gray-200:hover {
  background: rgba(255, 255, 255, 0.15) !important;
  color: #fff !important;
}
.light .mcp-page .rounded-2xl select option {
  color: #1f2937;
}

.light .mcp-page .rounded-2xl input,
.light .mcp-page .rounded-2xl textarea,
.light .mcp-page .rounded-2xl select {
  color: #1f2937;
}

/* MCP 测试工具弹窗：工具名称下拉框样式对齐添加模型供应商弹窗 */
.mcp-test-dialog .test-tool-form-group .el-select .el-select__wrapper {
  padding: 10px;
  height: auto !important;
  min-height: unset !important;
  box-sizing: border-box;
  background: transparent !important;
  border: 1px solid var(--border-color) !important;
  border-radius: 6px !important;
  box-shadow: none !important;
}
.mcp-test-dialog .test-tool-form-group .el-select .el-select__placeholder,
.mcp-test-dialog .test-tool-form-group .el-select .el-select__selected-item {
  color: var(--text-primary) !important;
}

/* MCP 编辑弹窗：下拉框背景透明、边框使用主题色、文字使用主题文字色 */
.mcp-edit-dialog .el-select .el-select__wrapper {
  padding: 10px;
  height: auto !important;
  min-height: unset !important;
  box-sizing: border-box;
  background: transparent !important;
  border: 1px solid var(--border-color) !important;
  border-radius: 6px !important;
  box-shadow: none !important;
}
.mcp-edit-dialog .el-select .el-select__placeholder,
.mcp-edit-dialog .el-select .el-select__selected-item {
  color: var(--text-primary) !important;
}

.mcp-test-dialog .runTestBtn {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 6px 16px;
  background: var(--color-primary) !important;
  color: #fff !important;
  border: 1px solid var(--color-primary) !important;
  border-radius: 4px;
  cursor: pointer;
  font-size: var(--font-size-md);
  font-weight: 500;
  transition: all 0.2s;
}

.mcp-test-dialog .runTestBtn:hover {
  background: var(--color-primary) !important;
  color: #fff !important;
  border-color: var(--color-primary) !important;
}


</style>

<style>
.dark .mcp-page [class*="shadow"]:where(.bg-white) {
  background: #1a1a1a !important;
  border-color: rgba(255,255,255,0.15) !important;
}
.dark .mcp-page [class*="shadow"]:where(.bg-white) button {
  color: #ffffff !important;
}
.dark .mcp-page [class*="shadow"]:where(.bg-white) button:hover,
.dark .mcp-page [class*="shadow"]:where(.bg-white) .bg-gray-50 {
  background: rgba(255,255,255,0.08) !important;
}
.light .mcp-page input[type="text"],
.light .mcp-page input:not([type]) {
  color: #1f2937;
}

/* MCP 表格分割线颜色 */
.mcp-table-row {
  border-color: var(--border-color) !important;
}

.mcp-list-scroll {
  background: transparent;
  border-radius: 1rem;
  overflow-x: auto;
  overflow-y: visible;
  scrollbar-width: thin;
  scrollbar-color: var(--scrollbar-thumb, rgba(0, 0, 0, 0.15)) transparent;
}

.mcp-list-scroll::-webkit-scrollbar { height: 6px; }
.mcp-list-scroll::-webkit-scrollbar-track { background: transparent; }
.mcp-list-scroll::-webkit-scrollbar-thumb {
  background: var(--scrollbar-thumb, rgba(0, 0, 0, 0.15));
  border-radius: 3px;
}

.mcp-list-inner {
  width: 100%;
  min-width: 560px;
}

.mcp-list-grid {
  display: grid;
  grid-template-columns:
    2.5rem
    minmax(0, 1.35fr)
    minmax(4.5rem, 0.85fr)
    minmax(4rem, 0.7fr)
    auto;
  align-items: center;
  column-gap: 0.75rem;
  width: 100%;
  box-sizing: border-box;
}

.mcp-list-icon {
  flex-shrink: 0;
}

.mcp-list-name {
  min-width: 0;
}

.mcp-list-meta {
  text-align: center;
  min-width: 0;
}

.mcp-list-actions {
  justify-self: end;
  flex-shrink: 0;
  white-space: nowrap;
}

/* MCP 表格行鼠标悬浮颜色 - 白色透明度20% */
.mcp-table-row:hover {
  background: rgba(255, 255, 255, 0.03) !important;
}

/* 除浅色主题和雾灰调外，transport-type-tag 背景为黑色透明度5% */
.dark .transport-type-tag,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .transport-type-tag {
  background: rgba(0, 0, 0, 0.05) !important;
  border-color: var(--border-color) !important;
}

/* Dark theme: view mode toggle */
.dark .mcp-page .view-mode-toggle {
  background: rgba(255,255,255,0.06);
}
.dark .mcp-page .view-mode-toggle button.bg-white {
  background: rgba(255,255,255,0.15) !important;
  color: #ffffff !important;
}
.dark .mcp-page .view-mode-toggle .text-gray-500 { color: #cbd5e0; }
/* 搜索框透明 */
.dark .mcp-page input[type="text"],
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .mcp-page input[type="text"] { background: transparent; }

/* 主题背景色 */
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .mcp-page .shadow-sm { background: rgba(255,255,255,0.10) !important; }
.light [data-theme-id="simple-light"] .mcp-page .shadow-sm,
.light [data-theme-id="scene-graphite-cyan"] .mcp-page .shadow-sm { background: rgba(0,0,0,0.07) !important; }
.dark .mcp-page .shadow-sm { background: rgba(255,255,255,0.05) !important; }
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .mcp-page .bg-white.shadow-sm,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .mcp-page .rounded-2xl.bg-white { background: rgba(255,255,255,0.10) !important; }
.light [data-theme-id="simple-light"] .mcp-page .bg-white.shadow-sm,
.light [data-theme-id="simple-light"] .mcp-page .rounded-2xl.bg-white,
.light [data-theme-id="scene-graphite-cyan"] .mcp-page .bg-white.shadow-sm,
.light [data-theme-id="scene-graphite-cyan"] .mcp-page .rounded-2xl.bg-white { background: rgba(0,0,0,0.07) !important; }
/* 标题白色 */
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .mcp-title,
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .mcp-subtitle { color: #fff !important; }
/* 筛选按钮悬浮背景 */
.dark .tools-filter-btn:hover { background: rgba(255,255,255,0.08) !important; }
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .tools-filter-btn:hover { background: rgba(255,255,255,0.10) !important; }
.light [data-theme-id="simple-light"] .tools-filter-btn:hover,
.light [data-theme-id="scene-graphite-cyan"] .tools-filter-btn:hover { background: rgba(0,0,0,0.07) !important; }

/* 暗色主题 */
.dark .mcp-page input::placeholder { color: rgba(255,255,255,0.7) !important; }
/* 情景主题 */
.light [data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .mcp-page input::placeholder { color: rgb(255,255,255) !important; }
.light [data-theme-id="simple-light"] .mcp-page input::placeholder ,
.light [data-theme-id="scene-graphite-cyan"].mcp-page input::placeholder { color: rgba(0,0,0,0.7) !important; }

/* 情景主题（除雾灰调以外）：MCP卡片标签白色半透明背景 */
/* .light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .mcp-page .rounded-2xl.bg-white .mt-auto > span {
  background: rgba(255, 255, 255, 0.10) !important;
  color: #fff !important;
  border-color: rgba(255, 255, 255, 0.25) !important;
} */
.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .mcp-page .rounded-2xl.bg-white .mt-auto > span svg {
  color: #fff !important;
}
/* 恢复蓝色标签（有工具数量时）的原样式 */
.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .mcp-page .rounded-2xl.bg-white .mt-auto > span.border-blue-100 {
  background: #EFF6FF !important;
  color: #2563EB !important;
  border-color: #DBEAFE !important;
}

/* 情景主题（除雾灰调）+ 暗色：来源标签（手动等）透明底 + --border-color 边框 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .mcp-page .mcp-source-badge,
html.dark .mcp-page .mcp-source-badge {
  background: transparent !important;
  border-color: var(--border-color) !important;
}


/* 暗色主题下 MCP 卡片文字和标签样式 */
.dark .mcp-page .text-gray-900 { color: #fff !important; }
.dark .mcp-page [class*="rounded-full"].bg-\[\#F7F8FA\] {
  background: rgba(255, 255, 255, 0.05) !important;
  color: #fff !important;
  border-color: rgba(255, 255, 255, 0.15) !important;
}
.dark .mcp-page [class*="rounded-full"].bg-\[\#F7F8FA\] svg { color: #fff !important; }
.dark .mcp-page .bg-blue-50 { background: transparent !important; }

/* 添加/编辑 MCP Server 弹窗：输入框/文本域/下拉框聚焦时不变色、无外阴影 */
.mcp-create-dialog input:focus,
.mcp-create-dialog textarea:focus,
.mcp-create-dialog select:focus,
.mcp-edit-dialog input:focus,
.mcp-edit-dialog textarea:focus,
.mcp-edit-dialog select:focus {
  border-color: var(--border-color) !important;
  box-shadow: none !important;
  outline: none !important;
  --tw-ring-offset-shadow: none !important;
  --tw-ring-shadow: none !important;
}
.dark .mcp-page .bg-gray-50 { background: rgba(255, 255, 255, 0.06) !important; }
.dark .mcp-page .hover\:bg-gray-50:hover { background: rgba(255, 255, 255, 0.10) !important; }
.dark .mcp-page .hover\:bg-gray-100:hover { background: rgba(255, 255, 255, 0.08) !important; }
.dark .mcp-page .text-gray-500 { color: rgba(255, 255, 255, 0.7) !important; }
.dark .mcp-page .hover\:text-\[\#1D2129\]:hover { color: #fff !important; }

/* 暗色/情景主题下 bg-gray-50 / bg-gray-100 改为半透明白色 */
.mcp-page[data-theme-id="simple-dark"] .bg-gray-50,
.mcp-page[data-theme-id^="dark-"] .bg-gray-50,
.mcp-page[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .bg-gray-50 { background: rgba(255, 255, 255, 0.06) !important; }

.mcp-page[data-theme-id="simple-dark"] .hover\:bg-gray-50:hover,
.mcp-page[data-theme-id^="dark-"] .hover\:bg-gray-50:hover,
.mcp-page[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .hover\:bg-gray-50:hover { background: rgba(255, 255, 255, 0.10) !important; }

.mcp-page[data-theme-id="simple-dark"] .hover\:bg-gray-100:hover,
.mcp-page[data-theme-id^="dark-"] .hover\:bg-gray-100:hover,
.mcp-page[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .hover\:bg-gray-100:hover { background: rgba(255, 255, 255, 0.08) !important; }

.mcp-page[data-theme-id="simple-dark"] .text-gray-500,
.mcp-page[data-theme-id^="dark-"] .text-gray-500,
.mcp-page[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .text-gray-500 { color: rgba(255, 255, 255, 0.7) !important; }

.mcp-page[data-theme-id="simple-dark"] .hover\:text-\[\#1D2129\]:hover,
.mcp-page[data-theme-id^="dark-"] .hover\:text-\[\#1D2129\]:hover,
.mcp-page[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .hover\:text-\[\#1D2129\]:hover { color: #fff !important; }

.mcp-page[data-theme-id="simple-dark"] .bg-white,
.mcp-page[data-theme-id^="dark-"] .bg-white,
.mcp-page[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .bg-white { background: rgba(255, 255, 255, 0.06) !important; }

.mcp-page[data-theme-id="simple-dark"] .border-gray-100,
.mcp-page[data-theme-id^="dark-"] .border-gray-100,
.mcp-page[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .border-gray-100 { border-color: rgba(255, 255, 255, 0.10) !important; }

.mcp-page[data-theme-id="simple-dark"] .border-gray-200,
.mcp-page[data-theme-id^="dark-"] .border-gray-200,
.mcp-page[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .border-gray-200 { border-color: rgba(255, 255, 255, 0.12) !important; }

.mcp-page[data-theme-id="simple-dark"] .text-gray-900,
.mcp-page[data-theme-id^="dark-"] .text-gray-900,
.mcp-page[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .text-gray-900 { color: #e5e7eb !important; }

.mcp-page[data-theme-id="simple-dark"] .text-gray-700,
.mcp-page[data-theme-id^="dark-"] .text-gray-700,
.mcp-page[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .text-gray-700 { color: rgba(255, 255, 255, 0.8) !important; }
.test-mode-btn:hover { background: var(--dropdown-hover-color) !important; }

/* ElMessageBox 取消按钮样式（与 .theme-action--secondary 一致） */
.el-message-box__btns .el-button:not(.el-button--primary) {
  background: var(--cancel-btn-color) !important;
  color: var(--btn-secondary-text) !important;
  border: 1px solid var(--btn-secondary-border) !important;
  font-weight: 600 !important;
  border-radius: 4px !important;
  min-width: 76px !important;
  height: 36px !important;
  padding: 0 16px !important;
  box-sizing: border-box !important;
}
.el-message-box__btns .el-button:not(.el-button--primary):hover {
  background: var(--cancel-btn-hover-color) !important;
  color: var(--btn-secondary-hover-text) !important;
  border-color: var(--btn-secondary-hover-border) !important;
}
.el-message-box__btns .el-button.el-button--primary {
  min-width: 76px !important;
  height: 36px !important;
  padding: 0 16px !important;
  box-sizing: border-box !important;
}
</style>
