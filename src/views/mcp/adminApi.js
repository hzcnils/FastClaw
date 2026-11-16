/**
 * FastClaw Admin API Service
 * Tools 走 Tauri invoke（本地），其余（Agents / Skills / MCP）走 FastPlan admin API。
 *
 * 2026-09-23：MCP 此前走本地 invoke，写的是 FastClaw 自己的空 mcp_servers 表，
 * 与规划链实际使用的 FastPlan `managed_mcp_servers` 是两份互不相通的数据——
 * 界面上配的 server 对规划不起作用。现改为直连 FastPlan admin API。
 */
import { invoke } from '@tauri-apps/api/core'
import {DEFAULT_CONFIG} from '@/config/index'
import {fastClawFastPlanAccess} from '@/integrations/chat-plan/fastplanAccess'


const API = DEFAULT_CONFIG.FastPlan || ''

/**
 * FastPlan 以 `--parent-managed` 启动时（FastClaw 拉起它的默认方式），
 * `parent_auth::require_parent` 覆盖**全部**路由，含 `/api/admin/*`
 * （见 fastplan/src/parent_auth.rs 的 "Applied to the entire router"），
 * 因此 admin 调用必须带 parent token，否则一律 401。
 *
 * 非 parent-managed（手动起的 dev 实例）时该中间件直接放行，拿不到连接也不影响。
 * 每次请求现取，不缓存——token 会随 FastPlan 重启更换。
 */
async function authHeaders() {
  try {
    const conn = await fastClawFastPlanAccess.getConnection()
    return conn?.parentToken ? { Authorization: `Bearer ${conn.parentToken}` } : {}
  } catch {
    // FastPlan 未就绪 / 非受管模式：不带 token，交由服务端判定
    return {}
  }
}

async function request(url, options = {}) {
  const auth = await authHeaders()
  const res = await fetch(`${API}${url}`, {
    ...options,
    headers: { 'Content-Type': 'application/json', ...auth, ...options.headers },
  })
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: { message: res.statusText } }))
    throw new Error(err.error?.message || `HTTP ${res.status}`)
  }
  const ct = res.headers.get('content-type') || ''
  if (!ct.includes('application/json')) {
    throw new Error(`接口暂未实现 (${res.status})`)
  }
  return res.json()
}

// ── Tools（本地 invoke） ──

function toolsInvoke(cmd, args) {
  return invoke(cmd, args).catch((e) => {
    const msg =
      (typeof e === 'string' && e) ||
      e?.message ||
      e?.error ||
      (e != null ? String(e) : '未知错误')
    // 常见于后端未重编译：命令尚未注册进当前 FastClaw.exe
    if (/not allowed|not found|unknown command|Command/i.test(msg)) {
      throw new Error(`${msg}（请重启 tauri:dev 以加载本地 tools 命令）`)
    }
    throw new Error(msg)
  })
}

export const toolsApi = {
  list: () =>
    toolsInvoke('list_local_tools'),

  get: (id) =>
    toolsInvoke('get_local_tool', { id }),

  create: (data) =>
    toolsInvoke('create_local_tool', { payload: data }),

  update: (id, data) =>
    toolsInvoke('update_local_tool', { id, payload: data }),

  delete: (id) =>
    toolsInvoke('delete_local_tool', { id }),

  toggle: (id, active) =>
    toolsInvoke('toggle_local_tool', { id, isActive: active }),
}

// ── Skills（本地 invoke） ──

export const skillsApi = {
  list: (params = {}) =>
    invoke('list_local_skills', { category: params.category ?? null }),

  get: (id) =>
    invoke('get_local_skill', { name: id }),

  create: (data) =>
    invoke('create_local_skill', { payload: data }),

  update: (id, data) =>
    invoke('update_local_skill', { name: id, payload: data }),

  delete: (id) =>
    invoke('delete_local_skill', { name: id }),

  toggle: (id, enabled) =>
    invoke('toggle_local_skill', { name: id, enabled }),

  importFolder: (dir) =>
    invoke('import_skills_from_folder', { dir }),

  importGithub: (url) =>
    invoke('import_skills_from_github', { url }),

  /** @deprecated 旧远端导入，请用 importFolder(目录路径) 替代。单文件导入暂不支持。 */
  import: async (_file) => {
    throw new Error('单文件导入已废弃。请使用导入文件夹（importFolder）或从 GitHub 导入（importGithub）。')
  },

  /** @deprecated 旧远端重建索引 */
  rebuildIndex: () =>
    invoke('rebuild_skill_index').catch(() => {
      throw new Error('重建索引暂不可用（已迁移至本地文件系统，不再需要索引重建）')
    }),
}

// ── MCP Servers（走 FastPlan admin API） ──
//
// 对外形状与旧的 invoke 版保持一致，7 处消费方（McpManager / agent-mcp /
// McpStore / NodeEditor / 两个 node-form）无需改动：
//   · list 返回 { items: [...] }，消费方本就写的 `res.items || res`
//   · create/update 收同样的 camelCase 载荷
// 换的只是底层通道：写进 FastPlan 的 managed_mcp_servers，与规划链同一份数据。

function mcpServerPath(id) {
  return `/api/admin/mcp-servers/${encodeURIComponent(id)}`
}

export const mcpApi = {
  /** @returns {Promise<{items: Array, total: number, page: number, page_size: number}>} */
  list: () =>
    request('/api/admin/mcp-servers?page_size=500'),

  /**
   * 详情。**列表接口的 McpServerSummary 不含 `environment` / `auto_start`
   * （只存在于 McpServerDetail）**，编辑弹窗必须拿这个，否则环境变量显示为空、
   * 且保存时会把 auto_start 当成 false 写回去。
   * @returns {Promise<{transport_config, environment, auto_start, tools, ...}>}
   */
  get: (id) =>
    request(mcpServerPath(id)),

  create: (data) =>
    request('/api/admin/mcp-servers', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  /**
   * 载荷字段与 UpdateMcpServerRequest 对齐（FastPlan 侧已同步扩展）：
   * {name, description, transport_type, transport_config, environment,
   *  keywords, auto_start}。此前 FastPlan 只收
   * {name, description, transport_config, keywords, reindex}，编辑弹窗里改的
   * 传输类型 / 环境变量 / 自启会静默丢失。
   */
  update: (id, data) =>
    request(mcpServerPath(id), {
      method: 'PUT',
      body: JSON.stringify(data),
    }),

  delete: (id) =>
    request(mcpServerPath(id), { method: 'DELETE' }),

  status: (id) =>
    request(`${mcpServerPath(id)}/status`),

  connect: (id) =>
    request(`${mcpServerPath(id)}/connect`, { method: 'POST' }),

  disconnect: (id) =>
    request(`${mcpServerPath(id)}/disconnect`, { method: 'POST' }),

  /** @returns {Promise<Array<{id, name, description, input_schema}>>} */
  refreshTools: (id) =>
    request(`${mcpServerPath(id)}/refresh-tools`, { method: 'POST' }),

  /** @returns {Promise<{success, result, error, duration_ms}>} */
  testTool: (id, toolName, args) =>
    request(`${mcpServerPath(id)}/test-tool`, {
      method: 'POST',
      body: JSON.stringify({ tool_name: toolName, arguments: args ?? {} }),
    }),

  /** 从 FastMCP 注册中心同步 server 列表 */
  syncFastMCP: () =>
    request('/api/admin/advanced/sync-fastmcp', { method: 'POST' }),
}

// ── Agents ──

export const agentsApi = {
  list: (params = {}) => {
    const qs = new URLSearchParams(params).toString()
    return request(`/api/admin/agents${qs ? '?' + qs : ''}`)
  },
  get: (id) => request(`/api/admin/agents/${id}`),
  create: (data) => request('/api/admin/agents', { method: 'POST', body: JSON.stringify(data) }),
  update: (id, data) => request(`/api/admin/agents/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
  delete: (id) => request(`/api/admin/agents/${id}`, { method: 'DELETE' }),
  toggle: (id, enabled) => request(`/api/admin/agents/${id}/toggle`, {
    method: 'POST', body: JSON.stringify({ enabled })
  }),
  rebuildIndex: () => request('/api/admin/advanced/rebuild-agent-index', { method: 'POST' }),
}

// ── Agent Tool Bindings (agent_tool_bindings) ──

export const bindingsApi = {
  /** 获取某个 Agent 的所有工具绑定 */
  list: (agentId) => request(`/api/admin/agents/${agentId}/tools`),
  /** 添加工具绑定: { tool_type, tool_id, server_id? } */
  add: (agentId, data) => request(`/api/admin/agents/${agentId}/tools`, {
    method: 'POST', body: JSON.stringify(data)
  }),
  /** 删除工具绑定 */
  remove: (agentId, bindingId) => request(`/api/admin/agents/${agentId}/tools/${bindingId}`, {
    method: 'DELETE'
  }),
  /** 启用/禁用某条绑定 */
  toggle: (agentId, bindingId, enabled) => request(`/api/admin/agents/${agentId}/tools/${bindingId}/toggle`, {
    method: 'POST', body: JSON.stringify({ enabled })
  }),
}

// ── Vector / Advanced ──

export const vectorApi = {
  stats: () => request('/api/admin/advanced/vector-stats'),
  search: (query, scope = 'all', topK = 10) => request('/api/admin/advanced/vector-search', {
    method: 'POST', body: JSON.stringify({ query, scope, top_k: topK })
  }),
  rebuildAll: () => request('/api/admin/advanced/rebuild-index', { method: 'POST' }),
}

// ── Workflow (Issue #32) ──

export const workflowApi = {
  /** 获取工作流参数 schema */
  schema: (binding) => request('/api/workflow/schema', {
    method: 'POST', body: JSON.stringify({ binding })
  }),
  /** 测试工作流执行 */
  test: (binding, input) => request('/api/workflow/test', {
    method: 'POST', body: JSON.stringify({ binding, input })
  }),
}
