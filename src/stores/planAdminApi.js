// Stub for FastPlan admin API - skills, mcp, workflows
const API_BASE = import.meta.env.VITE_PLAN_API_BASE || 'http://localhost:23180'

export const skillsApi = {
  list: async () => {
    try {
      const res = await fetch(`${API_BASE}/api/skills`)
      if (res.ok) return await res.json()
    } catch {}
    return []
  }
}

export const mcpApi = {
  list: async () => {
    try {
      const res = await fetch(`${API_BASE}/api/mcp/servers`)
      if (res.ok) return await res.json()
    } catch {}
    return []
  }
}

export const workflowApi = {
  list: async () => {
    try {
      const res = await fetch(`${API_BASE}/api/workflows`)
      if (res.ok) return await res.json()
    } catch {}
    return []
  }
}
