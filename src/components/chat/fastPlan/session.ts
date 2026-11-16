import { defineStore } from 'pinia'
import type { TaskTree } from './treeToDAG'
import { getServiceUrl } from '@/config/index'

function getFastPlanBase(): string {
  const svc = getServiceUrl('fastplan')
  if (svc) return svc
  if (import.meta.env.DEV) return '/FastPlan'
  return `${location.protocol}//${location.host}/FastPlan`
}

const FAST_PLAN_BASE = getFastPlanBase()

export interface FastPlanSession {
  id: string
  title: string
  created_at: string
  updated_at: string
}

export interface FastPlanMessage {
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp: Date
  isPlan?: boolean
  isThinking?: boolean
  isError?: boolean
  isQuestion?: boolean
  isReflection?: boolean
  isResponse?: boolean
  duration?: number
  nodeCount?: number
}

export interface ExecutionNodeState {
  id: string
  name: string
  status: 'pending' | 'running' | 'completed' | 'failed' | 'skipped'
  streamContent: string
  output: string
  error: string | null
  startTime: number | null
  endTime: number | null
}

export interface HumanRequest {
  tree_id: string
  node_id: string
  prompt: string
  options: Array<string | { label: string; value: string; is_default?: boolean }>
  timeout_seconds: number
  timestamp: number
}

export interface FastPlanTodo {
  id: string
  content: string
  completed: boolean
  created_at: string
}

export const useFastPlanStore = defineStore('fastPlan', {
  state: () => ({
    sessions: [] as FastPlanSession[],
    sessionsLoaded: false,
    currentSessionId: null as string | null,
    messages: [] as FastPlanMessage[],
    executionState: {} as Record<string, ExecutionNodeState>,
    currentTreeId: null as string | null,
    plannedTree: null as TaskTree | null,
    isPreviewing: false,
    isPlanning: false,
    todos: [] as FastPlanTodo[],
    humanRequestQueue: [] as HumanRequest[],
    templates: [] as unknown[],
    executionMode: (localStorage.getItem('fastplan_exec_mode') || 'manual') as 'auto' | 'manual' | 'step',
    isPaused: false,
    stepConfirmNodeId: null as string | null,
    selectedNodeId: null as string | null,
  }),

  getters: {
    currentSession: (state) => state.sessions.find(s => s.id === state.currentSessionId) || null,
    currentHumanRequest: (state) => state.humanRequestQueue[0] || null,
    taskList: (state) =>
      Object.values(state.executionState).sort((a, b) => (a.startTime || 0) - (b.startTime || 0)),
    stats: (state) => {
      const tasks = Object.values(state.executionState)
      return {
        total: tasks.length,
        completed: tasks.filter(t => t.status === 'completed').length,
        running: tasks.filter(t => t.status === 'running').length,
        failed: tasks.filter(t => t.status === 'failed').length,
      }
    },
    groupedSessions: (state) => {
      const now = new Date()
      const today = new Date(now.getFullYear(), now.getMonth(), now.getDate())
      const yesterday = new Date(today.getTime() - 86400000)
      const weekAgo = new Date(today.getTime() - 7 * 86400000)
      const groups: Record<string, FastPlanSession[]> = { today: [], yesterday: [], thisWeek: [], older: [] }
      state.sessions.forEach(session => {
        const date = new Date(session.updated_at || session.created_at)
        if (date >= today) groups.today.push(session)
        else if (date >= yesterday) groups.yesterday.push(session)
        else if (date >= weekAgo) groups.thisWeek.push(session)
        else groups.older.push(session)
      })
      return groups
    },
  },

  actions: {
    async loadSessions() {
      try {
        const res = await fetch(`${FAST_PLAN_BASE}/api/sessions`)
        if (res.ok) {
          const data = await res.json()
          this.sessions = data.sessions || data
          this.sessionsLoaded = true
        }
      } catch (e) {
        console.error('Failed to load sessions:', e)
      }
    },

    async createSession(title = ''): Promise<FastPlanSession | null> {
      try {
        const res = await fetch(`${FAST_PLAN_BASE}/api/sessions`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ title }),
        })
        if (res.ok) {
          const session = await res.json()
          this.sessions.unshift(session)
          return session
        }
      } catch (e) {
        console.error('Failed to create session:', e)
      }
      return null
    },

    async loadSession(sessionId: string) {
      if (!sessionId) return null
      if (this.currentSessionId === sessionId && this.messages.length > 0) return null

      try {
        const res = await fetch(`${FAST_PLAN_BASE}/api/sessions/${sessionId}`)
        if (!res.ok) {
          if (res.status === 404) return null
          throw new Error(`HTTP ${res.status}`)
        }
        const data = await res.json()
        const msgs = (data.messages || []).map((m: any) => ({
          role: m.role,
          content: m.content,
          timestamp: new Date(m.created_at),
          ...(m.metadata ? JSON.parse(m.metadata) : {}),
        }))
        if (data.executions?.length > 0) {
          const latest = data.executions[data.executions.length - 1]
          if (latest.tree_json) {
            this.plannedTree = typeof latest.tree_json === 'string'
              ? JSON.parse(latest.tree_json)
              : latest.tree_json
          }
          if (latest.state_json) {
            const saved = typeof latest.state_json === 'string'
              ? JSON.parse(latest.state_json)
              : latest.state_json
            Object.assign(this.executionState, saved)
          }
          this.currentTreeId = latest.tree_id
        }
        // ★ 全部数据加载成功后才设置 sessionId，失败时不留脏状态
        this.currentSessionId = sessionId
        this.messages = msgs
        this.todos = data.todos || []
        return data
      } catch (e) {
        console.error('Failed to load session:', e)
        return null
      }
    },

    async updateSessionTitle(sessionId: string, title: string) {
      try {
        await fetch(`${FAST_PLAN_BASE}/api/sessions/${sessionId}`, {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ title }),
        })
        const session = this.sessions.find(s => s.id === sessionId)
        if (session) session.title = title
      } catch (e) {
        console.error('Failed to update session title:', e)
      }
    },

    async deleteSession(sessionId: string) {
      try {
        await fetch(`${FAST_PLAN_BASE}/api/sessions/${sessionId}`, { method: 'DELETE' })
        this.sessions = this.sessions.filter(s => s.id !== sessionId)
        if (this.currentSessionId === sessionId) this.clearCurrentSession()
      } catch (e) {
        console.error('Failed to delete session:', e)
      }
    },

    async addMessage(msg: FastPlanMessage) {
      this.messages.push(msg)
      if (this.currentSessionId) {
        try {
          await fetch(`${FAST_PLAN_BASE}/api/sessions/${this.currentSessionId}/messages`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              role: msg.role,
              content: msg.content,
              msg_type: msg.isPlan ? 'plan' : msg.isThinking ? 'thinking' : msg.isError ? 'error' : 'text',
              metadata: JSON.stringify({
                isPlan: msg.isPlan, isThinking: msg.isThinking, isError: msg.isError,
                isQuestion: msg.isQuestion, isReflection: msg.isReflection,
                duration: msg.duration, nodeCount: msg.nodeCount,
              }),
            }),
          })
        } catch (e) {
          console.error('Failed to save message:', e)
        }
      }
    },

    removeThinkingMessage() {
      const idx = this.messages.findIndex(m => m.isThinking)
      if (idx !== -1) this.messages.splice(idx, 1)
    },

    async loadTodos(sessionId: string) {
      try {
        const res = await fetch(`${FAST_PLAN_BASE}/api/sessions/${sessionId}/todos`)
        if (res.ok) this.todos = await res.json()
      } catch (e) {
        console.error('Failed to load todos:', e)
      }
    },

    async addTodo(content: string) {
      if (!this.currentSessionId) return
      const tempId = `temp_${Date.now()}`
      const todo: FastPlanTodo = { id: tempId, content, completed: false, created_at: new Date().toISOString() }
      this.todos.push(todo)
      try {
        const res = await fetch(`${FAST_PLAN_BASE}/api/sessions/${this.currentSessionId}/todos`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ content }),
        })
        if (res.ok) {
          const saved = await res.json()
          const idx = this.todos.findIndex(t => t.id === tempId)
          if (idx !== -1) this.todos[idx] = saved
        }
      } catch (e) {
        console.error('Failed to add todo:', e)
      }
    },

    async toggleTodo(todoId: string) {
      const todo = this.todos.find(t => t.id === todoId)
      if (!todo) return
      todo.completed = !todo.completed
      try {
        await fetch(`${FAST_PLAN_BASE}/api/sessions/${this.currentSessionId}/todos/${todoId}`, {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ completed: todo.completed }),
        })
      } catch (e) {
        console.error('Failed to toggle todo:', e)
        todo.completed = !todo.completed
      }
    },

    async deleteTodo(todoId: string) {
      const idx = this.todos.findIndex(t => t.id === todoId)
      if (idx === -1) return
      const removed = this.todos.splice(idx, 1)[0]
      try {
        await fetch(`${FAST_PLAN_BASE}/api/sessions/${this.currentSessionId}/todos/${todoId}`, {
          method: 'DELETE',
        })
      } catch (e) {
        console.error('Failed to delete todo:', e)
        this.todos.splice(idx, 0, removed)
      }
    },

    clearCurrentSession() {
      this.currentSessionId = null
      this.messages.length = 0
      this.executionState = {}
      this.humanRequestQueue = []
      this.plannedTree = null
      this.isPreviewing = false
      this.isPlanning = false
      this.currentTreeId = null
      this.todos = []
      this.isPaused = false
      this.stepConfirmNodeId = null
      this.selectedNodeId = null
    },

    clearExecutionState() {
      this.executionState = {}
      this.humanRequestQueue = []
      this.plannedTree = null
      this.isPreviewing = false
    },

    async saveExecutionSnapshot() {
      if (!this.currentSessionId || !this.currentTreeId) return
      try {
        await fetch(`${FAST_PLAN_BASE}/api/sessions/${this.currentSessionId}/executions`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            tree_id: this.currentTreeId,
            tree_json: this.plannedTree,
            state_json: this.executionState,
            status: this.isPlanning ? 'running' : 'completed',
          }),
        })
      } catch (e) {
        console.error('Failed to save execution snapshot:', e)
      }
    },

    async loadTemplates() {
      try {
        const res = await fetch(`${FAST_PLAN_BASE}/api/templates`)
        if (res.ok) this.templates = await res.json()
      } catch (e) {
        console.error('Failed to load templates:', e)
      }
    },

    async saveTemplate(name: string, description: string, tree: TaskTree) {
      try {
        const res = await fetch(`${FAST_PLAN_BASE}/api/templates`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ name, description, tree_json: tree }),
        })
        if (res.ok) {
          const template = await res.json()
          this.templates.unshift(template)
          return template
        }
      } catch (e) {
        console.error('Failed to save template:', e)
      }
      return null
    },

    async deleteTemplate(templateId: string) {
      try {
        await fetch(`${FAST_PLAN_BASE}/api/templates/${templateId}`, { method: 'DELETE' })
        this.templates = this.templates.filter((t: any) => t.id !== templateId)
      } catch (e) {
        console.error('Failed to delete template:', e)
      }
    },

    setExecutionMode(mode: 'auto' | 'manual' | 'step') {
      this.executionMode = mode
      localStorage.setItem('fastplan_exec_mode', mode)
    },

    selectNode(nodeId: string) {
      this.selectedNodeId = nodeId
    },

    deselectNode() {
      this.selectedNodeId = null
    },
  },
})
