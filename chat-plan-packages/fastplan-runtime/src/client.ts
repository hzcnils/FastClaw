import { fastPlanRequest, type FastPlanAccess } from './access'
export interface FastPlanClientOptions {
  access?: FastPlanAccess
  apiBase?: string
  fetch?: typeof fetch
}

export function createFastPlanClient(options: FastPlanClientOptions = {}) {
  const base = (options.apiBase || '').replace(/\/$/, '')
  const request = options.fetch || fetch
  async function json(path: string, init?: RequestInit) {
    const response = await fastPlanRequest(options.access ? path : `${base}${path}`, init, options.access, request)
    if (!response.ok) {
      const error = new Error(`FastPlan API ${response.status}: ${path}`) as Error & { status?: number }
      error.status = response.status
      throw error
    }
    if (response.status === 204) return undefined
    const text = await response.text()
    if (!text.trim()) return undefined
    try { return JSON.parse(text) } catch { return text }
  }
  function messagePayload(message: unknown) {
    if (!message || typeof message !== 'object' || Array.isArray(message)) return message
    const payload = message as Record<string, unknown>
    if (payload.metadata === undefined || payload.metadata === null || typeof payload.metadata === 'string') return payload
    return { ...payload, metadata: JSON.stringify(payload.metadata) }
  }
  return {
    listSessions: () => json('/api/sessions'),
    getSession: (sessionId: string, signal?: AbortSignal) => json(`/api/sessions/${encodeURIComponent(sessionId)}`, { signal }),
    createSession: (title = '', signal?: AbortSignal) => json('/api/sessions', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ title }), signal }),
    deleteSession: (sessionId: string) => json(`/api/sessions/${encodeURIComponent(sessionId)}`, { method: 'DELETE' }),
    updateSessionTitle: (sessionId: string, title: string) => json(`/api/sessions/${encodeURIComponent(sessionId)}`, { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ title }) }),
    saveMessage: (sessionId: string, message: unknown) => json(`/api/sessions/${encodeURIComponent(sessionId)}/messages`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(messagePayload(message)) }),
    saveExecutionSnapshot: (sessionId: string, snapshot: unknown) => json(`/api/sessions/${encodeURIComponent(sessionId)}/executions`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(snapshot) }),
    listTemplates: () => json('/api/templates'),
    createTemplate: (template: unknown, signal?: AbortSignal) => json('/api/templates', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(template), signal }),
    deleteTemplate: (templateId: string) => json(`/api/templates/${encodeURIComponent(templateId)}`, { method: 'DELETE' }),
    executeTree: (sessionId: string, tree: unknown, signal?: AbortSignal) => json(`/api/execute_tree?session_id=${encodeURIComponent(sessionId)}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(tree), signal }),
    retryTask: (treeId: string, nodeId: string, signal?: AbortSignal) => json('/api/retry_task', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ tree_id: treeId, node_id: nodeId }), signal }),
    respondCheckpoint: (treeId: string, nodeId: string, data: unknown, signal?: AbortSignal) => json('/api/checkpoint', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ tree_id: treeId, node_id: nodeId, data }), signal }),
    getExecutionState: (sessionId: string, signal?: AbortSignal) => json(`/api/v2/sessions/${encodeURIComponent(sessionId)}/execution-state`, { signal }),
    listTodos: (sessionId: string) => json(`/api/sessions/${encodeURIComponent(sessionId)}/todos`),
    createTodo: (sessionId: string, content: string) => json(`/api/sessions/${encodeURIComponent(sessionId)}/todos`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ content }) }),
    updateTodo: (sessionId: string, todoId: string, completed: boolean) => json(`/api/sessions/${encodeURIComponent(sessionId)}/todos/${encodeURIComponent(todoId)}`, { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ completed }) }),
    deleteTodo: (sessionId: string, todoId: string) => json(`/api/sessions/${encodeURIComponent(sessionId)}/todos/${encodeURIComponent(todoId)}`, { method: 'DELETE' }),
    eventUrl: (sessionId: string) => `${base}/api/sessions/${encodeURIComponent(sessionId)}/events?include_fine_log=true`,
  }
}
