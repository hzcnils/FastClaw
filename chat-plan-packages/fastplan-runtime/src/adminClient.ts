import type { FastPlanClientOptions } from './client'
import { fastPlanRequest } from './access'

/** Backend catalog APIs required by the reusable plan node editor. */
export function createFastPlanAdminClient(options: FastPlanClientOptions = {}) {
  const base = (options.apiBase || '').replace(/\/$/, '')
  const requestImpl = options.fetch || fetch
  async function request(path: string, init: RequestInit = {}) {
    const response = await fastPlanRequest(options.access ? path : `${base}${path}`, { ...init, headers: { 'Content-Type': 'application/json', ...init.headers } }, options.access, requestImpl)
    if (!response.ok) throw new Error(`FastPlan API ${response.status}: ${path}`)
    return response.json()
  }
  return {
    skillsApi: { list: (params: Record<string, string | boolean> = {}) => { const query = new URLSearchParams(Object.entries(params).map(([key, value]) => [key, String(value)])).toString(); return request(`/api/admin/skills${query ? `?${query}` : ''}`) } },
    mcpApi: { list: (params: Record<string, string | boolean> = {}) => { const query = new URLSearchParams(Object.entries(params).map(([key, value]) => [key, String(value)])).toString(); return request(`/api/admin/mcp-servers${query ? `?${query}` : ''}`) } },
    workflowApi: {
      schema: (binding: unknown) => request('/api/workflow/schema', { method: 'POST', body: JSON.stringify({ binding }) }),
      test: (binding: unknown, input: unknown) => request('/api/workflow/test', { method: 'POST', body: JSON.stringify({ binding, input }) }),
    },
  }
}
