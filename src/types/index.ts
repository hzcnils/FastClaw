// FastClaw · 全局类型定义
// 与 Rust 后端 Command 的入参出参一一对应

// ─── 对话 ───
export interface Conversation {
  id: string
  title: string
  model_id: string | null
  assistant_id?: string | null
  mode: 'normal' | 'planning'
  is_pinned: boolean
  is_archived: boolean
  updated_at: string
  extra_config?: string | null
}

export interface Message {
  id: string
  conversation_id: string
  role: 'user' | 'assistant' | 'system' | 'tool'
  content: string
  model_id: string | null
  token_input: number | null
  token_output: number | null
  status: 'pending' | 'streaming' | 'completed' | 'error' | 'canceled'
  created_at: string
  group_id?: string | null
  think_content?: string
  think_show?: boolean
  trace_source?: TraceSource
  // ─── 扩展字段（来自 chatGLM 迁移）───
  is_liked?: number          // 0=未操作, 1=赞, -1=踩
  is_favorite?: number       // 0=未收藏, 1=已收藏
  translation?: string       // 翻译结果
  translation_lang?: string  // 翻译目标语言代码（zh/en/ja/ko/fr/es/...）
  translating?: boolean      // 翻译请求进行中
  is_edited?: boolean
  edited_at?: string | null
  context_cleared?: boolean  // 是否已被清除上下文（不参与下次 LLM 请求）
  attachments?: ChatAttachment[]
  tool_call?: ToolCallInfo     // BERT 意图路由直接工具调用
  tool_calls?: ToolCallEvent[] // 流式工具调用卡片列表
  isThinking?: boolean         // 等待响应，不表示模型返回了思考内容
  stream_phase?: 'waiting' | 'reasoning' | 'answering' | 'finalizing'
  // ─── 联网搜索 ───
  web_searching?: boolean      // 联网搜索进行中
  web_search_query?: string    // 本次搜索词
  web_hits?: WebHit[]          // 搜索命中的网页结果
  agent_name?: string
  agent_icon?: string | null
  retry_progress?: { attempt: number; total: number } // LLM 自动重试进度（运行时字段）
  skill_used?: { name: string; args?: string }  // Skill 调用信息（持久化到 extra_data）
  grounding?: GroundingManifest  // 证据台账：回答里的数字能否追溯到工具返回（持久化到 extra_data）
  grounding_note?: string        // 系统生成的提示语（不经模型，模型无法让它消失）
  planRuntime?: Record<string, any>
  _vendorConfig?: boolean    // 运行时：供应商配置补全占位消息
  _providerCode?: string     // 运行时：关联的供应商代码
}

// ─── 证据台账（接地校验）───
// 字段与 Rust 侧 `core::grounding::GroundingManifest` 一一对应，改一边必须改另一边。
export interface GroundingSource {
  index: number
  tool: string
  query: string
  excerpt: string
  success: boolean
  error?: string
}

export interface GroundingClaim {
  value: string
  /** 命中的证据序号；空数组 = 这个值在工具返回里找不到出处 */
  sources: number[]
}

/** 与 FastPlan 规划链共用同一套状态串，前端才能用同一个渲染器 */
export type GroundingState = 'grounded' | 'partial' | 'ungrounded' | 'skipped'

export interface GroundingManifest {
  sources: GroundingSource[]
  claims: GroundingClaim[]
  grounding: GroundingState
}

// ─── 联网搜索命中结果 ───
export interface WebHit {
  title: string
  url: string
  snippet: string
}

export interface ToolCallInfo {
  server_id: string
  tool_name: string
  arguments: Record<string, unknown>
}

// 流式工具调用事件（chat-tool-call 事件载荷）
export interface ToolCallEvent {
  id: string
  name: string
  kind?: 'builtin' | 'mcp' | 'skill'
  /** 人话标题（后端 tool_display_title 产出，如「搜索 "xxx"」） */
  title?: string
  args?: Record<string, unknown>
  status: 'running' | 'done' | 'error'
  result_preview?: string
  /** 结构化结果元数据（后端 ToolResult.metadata，如 jmis_search 的 { queries, results }） */
  metadata?: unknown
  artifacts?: unknown[]
  duration_ms?: number
}

// ─── jmis_search 结构化结果 ───
export interface SearchResult {
  title: string
  url: string
  snippet?: string
  content?: string | null
  host?: string
  engines?: string[]
  date?: string | null
  evidence?: string[]
  score?: number
  [k: string]: unknown
}

/** fetch_web_page 后端命令返回（由另一个 agent 实现，前端按此契约消费） */
export interface FetchWebPageResult {
  status: number
  headers: Record<string, string>
  html: string
  final_url: string
  embeddable: boolean
}

// 工具名称 → 展示类型
export function toolCallKind(name: string): 'skill' | 'mcp' | 'builtin' {
  if (name === 'skill' || name === 'run_skill') return 'skill'
  if (name.startsWith('mcp__')) return 'mcp'
  return 'builtin'
}

// ─── 提示词模板 ───
export interface PromptTemplate {
  id: string
  title: string
  content: string
  description: string | null
  category: string | null
  source: 'system' | 'user' | 'shared'
  author: string | null
  is_official: number
  download_count: number
  created_at: string
  updated_at: string
}

// ─── 附件 ───
export interface ChatAttachment {
  id: string
  name: string
  type: 'image' | 'document' | 'screenshot' | 'other'
  size?: number
  preview_url?: string  // 本地 blob: 或 file:// URL
  uploaded_file_id?: string  // 后端返回的文件 ID
  uploaded_kb_id?: string    // 快速上传后返回的知识库 ID
  status?: 'pending' | 'uploading' | 'uploaded' | 'error'
  error_message?: string     // 上传失败时的错误信息
  ocr_text?: string          // OCR 文字提取结果（非多模态模型使用）
}

// ====== 溯源数据 ======
export interface TraceSource {
  graph?: TraceGraph
  local?: TraceLocal
  file?: TraceFile
  result?: TraceResult
  analysis?: TraceAnalysis
}

export interface TraceGraph {
  show: boolean
  graphTable?: EntityRelationRow[]
  nodes?: any[]
  edges?: any[]
}

export interface EntityRelationRow {
  headId?: string
  head: string
  headType?: string
  headDesc?: string
  relationId?: string
  relation?: string
  relationDesc?: string
  weight?: number
  tailId?: string
  tail?: string
  tailType?: string
  tailDesc?: string
}

export interface TraceLocal {
  show: boolean
  data: { snippet: string; fileName?: string; fileId?: string }[]
}

export interface TraceFile {
  show: boolean
  current: number
  data: {
    fileName: string
    fileId?: string
    url?: string
    path?: string
    snippet: { snippet: string }[]
  }[]
}

export interface TraceResult {
  show: boolean
  rate: number
  content: string
  loading: boolean
  isSubmit: boolean
  isQuery: boolean
}

export interface TraceAnalysis {
  show: boolean
  summary: string
}

export interface MessageResponse {
  message_id: string
  content: string
  think_content?: string | null
  model: string
  token_input: number
  token_output: number
  references: KbReference[]
}

export interface KbReference {
  chunk_id: string
  document_name: string
  section: string | null
  page: number | null
  preview: string
  score: number
}

// ─── 模型 ───
export interface ModelProvider {
  id: string
  name: string
  provider_type: string
  base_url: string
  is_local: boolean
  is_enabled: boolean
}

export interface Model {
  id: string
  model_id: string
  display_name: string
  model_type: 'chat' | 'embedding' | 'image' | 'tts' | 'stt'
}

/** 连接测试日志条目（来自 Tauri conn-test-log 事件） */
export interface ConnTestLogEntry {
  level: 'info' | 'warning' | 'error' | 'success'
  message: string
  timestamp_ms: number
}

// ─── 知识库（类型从 knowledge.ts 导入，此处保留后向兼容的 KbReference） ───
// 详细知识库类型定义见 ./knowledge.ts

// ─── WSP 服务 ───
export interface WspService {
  id: string
  name: string
  display_name: string
  description: string | null
  version: string
  category: string
  category_name: string | null
  sub_category_name: string | null
  port: number
  protocol: string
  auto_start: boolean
  is_enabled: boolean
  host: string
  base_path: string
  computed_url?: string
  // 运行状态
  pid: number | null
  runtime_status: 'stopped' | 'starting' | 'running' | 'stopping' | 'error' | 'crashed' | null
  cpu_percent: number | null
  memory_mb: number | null
  gpu_memory_mb: number | null
  request_count: number | null
  error_count: number | null
  avg_latency_ms: number | null
  restart_count: number | null
  runtime_started_at: string | null
  runtime_error: string | null
}

/** 依赖服务状态（WSP管理页面 → 依赖服务Tab） */
export interface DepServiceStatus {
  model_sync_status?: 'pending' | 'syncing' | 'synced' | 'retrying' | null;
  id: string
  name: string
  port: number
  preferred_port: number
  using_fallback: boolean
  host: string
  running: boolean
  pid: number | null
  lifecycle_state: 'stopped' | 'port_occupied' | 'probing' | 'starting' | 'running' | 'unhealthy' | 'stopping' | 'failed'
  effective_port: number | null
  port_occupied: boolean
  occupied_by_pid: number | null
  occupied_by_name: string | null
  can_start: boolean
  can_stop: boolean
  status_message: string
  description: string
  installed: boolean
  configurable: boolean
  ownership?: 'self_managed' | 'shared_managed' | 'external_unmanaged' | 'none'
  starter_app?: string | null
  consumers?: string[]
  lease_count?: number
  lease_registered?: boolean
  shared_instance?: boolean
  pending_restart?: boolean
}

export interface WspEventLog {
  id: string
  service_id: string
  event_type: string
  pid: number | null
  crash_reason: string | null
  created_at: string
}

// ─── AI 助手 ───
export interface Assistant {
  id: string
  name: string
  description: string | null
  avatar: string | null
  system_prompt: string
  model_id: string | null
  category: string
  is_builtin: boolean
  use_count: number
}

// ─── 费用 ───
export interface UsageStats {
  token_input: number
  token_output: number
  cost_usd: number
  call_count: number
}

export interface CostBudget {
  id: string
  budget_type: 'daily' | 'monthly'
  limit_usd: number
  action: 'warn' | 'downgrade' | 'pause'
  is_enabled: boolean
}

// ─── 规划 ───
export interface PlanTask {
  id: string
  user_intent: string
  status: 'planning' | 'executing' | 'paused' | 'completed' | 'failed' | 'cancelled'
  total_nodes: number
  completed_nodes: number
  created_at: string
}

// ─── 系统 ───
export interface SystemInfo {
  os: string
  os_version: string
  total_memory_mb: number
  used_memory_mb: number
  cpu_count: number
  app_version: string
}

// ─── 通用 ───
export interface AppError {
  code: string
  message: string
}

// ─── 左侧菜单 ───
export interface MenuItem {
  id: string
  label: string
  icon?: string
  route?: string
  children?: MenuItem[]
  badge?: string | number
  rawLabel?: boolean
  disabled?: boolean
  disabledReason?: string
}

// ─── 动态菜单（WSP 绑定） ───
export interface DynamicMenu {
  id: string
  parent_id: string | null
  title: string
  title_en: string | null
  icon: string | null
  menu_type: 'iframe' | 'url' | 'route' | 'separator'
  // iframe: 嵌入外部页面
  // url: 打开外部链接
  // route: 跳转内部路由
  // separator: 分隔线
  target_url: string | null        // iframe src 或外部 URL
  route_path: string | null        // 内部路由路径
  wsp_service_id: string | null    // 绑定的 WSP 服务 ID
  category: string                 // WSP 四大分类: ocr | document | media | feature
  sort_order: number
  is_visible: boolean
  children?: DynamicMenu[]
}

// ─── WSP 菜单分类定义 ───
export interface WspCategory {
  id: string
  label: string
  label_en?: string
  i18nKey?: string
  icon?: string
  children: DynamicMenu[]
}

// ─── MCP 服务 ───
export interface McpServer {
  id: string
  name: string
  description: string | null
  server_type: 'stdio' | 'sse'
  command: string | null
  url: string | null
  args: string | null
  env_vars: string | null
  is_enabled: boolean
  status: 'connected' | 'disconnected' | 'error'
  tools_cache: McpTool[] | null
}

export interface McpTool {
  name: string
  description: string | null
  input_schema: Record<string, any> | null
}

// ─── Skill ───
export interface Skill {
  id: string
  name: string
  display_name: string
  description: string | null
  version: string
  author: string | null
  source: 'builtin' | 'community' | 'enterprise'
  executor_type: 'rust' | 'deno' | 'mcp'
  entry_point: string | null
  icon: string | null
  category: string
  is_enabled: boolean
}
