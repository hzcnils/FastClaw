/// <reference types="vite/client" />
// FastClaw · 规划状态管理（集成 FastPlan 功能）
import { defineStore } from 'pinia'
import { ref, reactive, computed, readonly } from 'vue'
import { fastPlanRequest } from 'planflow-runtime'
import { fastClawFastPlanAccess } from '@/integrations/chat-plan/fastplanAccess'
import { waitForDependencyReady } from '@/api/dependencyRecovery'

// ===== 类型定义 =====
export interface PlanSession {
  id: string
  title: string
  created_at: string
  updated_at: string
}

export interface PlanNodeState {
  id: string
  name: string
  status: 'pending' | 'running' | 'completed' | 'failed' | 'skipped'
  logs: Array<{ type: string; content: string; toolName?: string }>
  streamContent: string
  output: string
  error: string | null
  startTime: number | null
  endTime: number | null
  workflowNodes?: any[]
}

export interface PlanTree {
  root: {
    id: string
    name: string
    children: PlanPhase[]
  }
  metadata?: any
}

export interface PlanPhase {
  id: string
  name: string
  node_type: any
  children: PlanNode[]
  dependencies: string[]
  metadata?: any
}

export interface PlanNode {
  id: string
  name: string
  node_type: any
  dependencies: string[]
  metadata?: any
  priority?: string
  requires_confirmation?: boolean
  retry_config?: any
  tags?: string[]
  timeout_seconds?: number | null
}

export const usePlanningStore = defineStore('planning', () => {
  // ===== 核心状态 =====
  const enabled = ref(false)  // 规划模式开关
  const isPlanning = ref(false)
  const isPreviewing = ref(false)
  const treeCompleted = ref(false)
  const currentSessionId = ref<string | null>(null)
  const currentTreeId = ref<string | null>(null)
  const plannedTree = ref<PlanTree | null>(null)
  const executionState = reactive<Record<string, PlanNodeState>>({})
  const taskBubbles = ref<string[]>([])
  const planningLogs = ref<Array<{ type: string; content: string; toolName?: string }>>([])
  const resultFiles = ref<Array<{ name: string; url: string }>>([])
  const executionMode = ref(localStorage.getItem('fastclaw_plan_mode') || 'auto')

  // WebSocket 连接
  let ws: WebSocket | null = null
  const wsConnected = ref(false)
  let heartbeatTimer: ReturnType<typeof setInterval> | null = null

  // SSE 连接
  let sseController: AbortController | null = null
  const sseConnected = ref(false)
  let ssePhase = 'idle'

  // ===== Getters =====
  const stats = computed(() => {
    const tasks = Object.values(executionState)
    return {
      total: tasks.length,
      completed: tasks.filter(t => t.status === 'completed').length,
      running: tasks.filter(t => t.status === 'running').length,
      failed: tasks.filter(t => t.status === 'failed').length,
    }
  })

  const phasedBubbles = computed(() => {
    const phases = plannedTree.value?.root?.children
    if (!phases || !taskBubbles.value.length) return null
    return phases
      .map(phase => ({
        id: phase.id,
        name: phase.name,
        taskIds: (phase.children || [])
          .map(t => t.id)
          .filter(id => taskBubbles.value.includes(id)),
      }))
      .filter(p => p.taskIds.length > 0)
  })

  // ===== WebSocket 管理 =====
  async function connectWs(sessionId: string) {
    if (ws && ws.readyState === WebSocket.OPEN) return
    if (ws && ws.readyState === WebSocket.CONNECTING) return

    try {
      await waitForDependencyReady('fastplan')
      const connection = await fastClawFastPlanAccess.getConnection()
      const url = `${connection.wsBase}/ws/session/${encodeURIComponent(sessionId)}`
      if (ws && [WebSocket.OPEN, WebSocket.CONNECTING].includes(ws.readyState)) return
      ws = new WebSocket(url)

      ws.onopen = () => {
        ws?.send(JSON.stringify({ type: 'authenticate', token: connection.parentToken }))
      }

      ws.onmessage = (event) => {
        try {
          const msg = JSON.parse(event.data)
          if (msg.type === 'authenticated') {
            wsConnected.value = true
        heartbeatTimer = setInterval(() => {
          if (ws?.readyState === WebSocket.OPEN) {
            ws.send(JSON.stringify({ type: 'ping' }))
          }
        }, 30000)
            return
          }
          if (!wsConnected.value) return
          handleWsMessage(msg)
        } catch (e) {
          console.error('[Plan WS] Parse error:', e)
        }
      }

      ws.onclose = () => {
        console.log(`🔌 [Plan WS] Closed`)
        wsConnected.value = false
        if (heartbeatTimer) { clearInterval(heartbeatTimer); heartbeatTimer = null }
        // 执行中自动重连
        if (isPlanning.value) {
          setTimeout(() => connectWs(sessionId), 2000)
        }
      }

      ws.onerror = (error) => {
        console.error('[Plan WS] Error:', error)
      }
    } catch (e) {
      console.error('[Plan WS] Failed to create:', e)
    }
  }

  function disconnectWs() {
    if (heartbeatTimer) { clearInterval(heartbeatTimer); heartbeatTimer = null }
    if (ws) { ws.close(); ws = null }
    wsConnected.value = false
  }

  function wsSend(data: any): boolean {
    if (ws?.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(data))
      return true
    }
    console.warn('[Plan WS] Cannot send, not connected')
    return false
  }

  function wsSendChat(content: string): boolean {
    return wsSend({ type: 'chat', content })
  }

  function wsConfirmPlan(tree?: PlanTree | null) {
    return wsSend({ type: 'confirm_plan', tree: tree || null })
  }

  function wsCancelPlan() {
    return wsSend({ type: 'cancel_plan' })
  }

  // ===== SSE 管理 =====
  async function connectSse(sessionId: string): Promise<void> {
    closeSse()
    const controller = new AbortController()
    sseController = controller
    await waitForDependencyReady('fastplan')
    // EventSource 不能传父进程鉴权头，改用带鉴权的流式 fetch；令牌不进入 URL。
    const response = await fastPlanRequest(`/api/sessions/${encodeURIComponent(sessionId)}/events?include_fine_log=true`,
      { signal: controller.signal }, fastClawFastPlanAccess)
    if (!response.ok || !response.body) throw new Error('规划事件流连接失败')
    if (controller.signal.aborted) return
    sseConnected.value = true
    const reader = response.body.getReader()
    void (async () => {
      const decoder = new TextDecoder()
      let buffer = ''
      try {
        while (!controller.signal.aborted) {
          const { value, done } = await reader.read()
          if (done) break
          buffer += decoder.decode(value, { stream: true })
          let boundary: number
          while ((boundary = buffer.search(/\r?\n\r?\n/)) >= 0) {
            const frame = buffer.slice(0, boundary)
            const separator = buffer.slice(boundary).match(/^\r?\n\r?\n/)![0]
            buffer = buffer.slice(boundary + separator.length)
            const lines = frame.split(/\r?\n/)
            const event = lines.find(line => line.startsWith('event:'))?.slice(6).trim()
            const data = lines.filter(line => line.startsWith('data:')).map(line => line.slice(5).trimStart()).join('\n')
            try {
              if (event === 'execution_log') handleSseExecutionLog(JSON.parse(data))
              if (event === 'thinking') handleSseThinking(JSON.parse(data))
            } catch { /* 忽略不完整业务帧 */ }
          }
        }
      } catch { /* 关闭或恢复期间断开，由 WS 提供状态 */ }
      finally { reader.releaseLock(); if (sseController === controller) sseConnected.value = false }
    })()
  }

  function closeSse() {
    sseController?.abort()
    sseController = null
    sseConnected.value = false
    ssePhase = 'idle'
  }

  // ===== SSE 事件处理 =====
  function handleSseExecutionLog(data: any) {
    if (data.tree_id === 'planner' || data.tree_id === 'planning') {
      const content = data.content || data.message || data.text || data.data?.content || data.token || ''
      if (content) addPlanningLog(content, data.phase)
      return
    }

    // 执行状态事件
    if (data.type) {
      switch (data.type) {
        case 'plan_generated':
          isPlanning.value = false
          plannedTree.value = data.tree
          if (executionMode.value === 'auto') {
            isPreviewing.value = false
          } else {
            isPreviewing.value = true
          }
          break
        case 'tree_started':
          isPlanning.value = true
          currentTreeId.value = data.tree_id
          // tree 可能在 plan_generated 中已经设置，也可能在 data 中携带
          if (data.tree && !plannedTree.value) {
            plannedTree.value = data.tree
          }
          initTaskBubblesFromTree()
          isPreviewing.value = false
          break
        case 'node_started':
          // 如果骨架还没初始化（可能 tree_started 被跳过），尝试动态添加
          if (!executionState[data.node_id]) {
            taskBubbles.value.push(data.node_id)
            executionState[data.node_id] = {
              id: data.node_id,
              name: data.node_name || data.name || data.node_id,
              status: 'running',
              logs: [], streamContent: '', output: '', error: null,
              startTime: Date.now(), endTime: null,
            }
          } else {
            executionState[data.node_id].status = 'running'
            executionState[data.node_id].startTime = Date.now()
          }
          if (!taskBubbles.value.includes(data.node_id)) {
            taskBubbles.value.push(data.node_id)
          }
          break
        case 'node_completed':
          if (executionState[data.node_id]) {
            executionState[data.node_id].status = 'completed'
            executionState[data.node_id].endTime = Date.now()
            if (data.output) executionState[data.node_id].output = data.output
          }
          break
        case 'node_failed':
          if (executionState[data.node_id]) {
            executionState[data.node_id].status = 'failed'
            executionState[data.node_id].error = data.error
            executionState[data.node_id].endTime = Date.now()
          }
          break
        case 'tree_completed':
          isPlanning.value = false
          treeCompleted.value = true
          // 防御：将所有还在 running 状态的节点标记为 completed
          for (const id of taskBubbles.value) {
            const s = executionState[id]
            if (s && s.status === 'running') {
              s.status = 'completed'
              s.endTime = Date.now()
            }
          }
          if (_onTreeCompleted) _onTreeCompleted(data.success !== false)
          break
        case 'final_answer':
          if (data.content && _onFinalAnswer) {
            _onFinalAnswer(data.content)
          }
          break
        case 'streaming_token':
          if (data.node_id && executionState[data.node_id]) {
            executionState[data.node_id].streamContent += (data.token || '')
          }
          break
      }
    }
  }

  function handleSseThinking(data: any) {
    const content = data.data?.content || data.content || ''
    if (!content) return
    const treeId = data.tree_id
    const nodeId = data.node_id

    if (treeId === 'planner' || treeId === 'planning') {
      addPlanningLog(content)
      return
    }

    if (nodeId && executionState[nodeId]) {
      executionState[nodeId].logs.push({ type: 'info', content })
    }
  }

  // ===== WebSocket 事件处理 =====
  function handleWsMessage(msg: any) {
    switch (msg.type) {
      case 'pong':
        break

      case 'plan_generated':
        isPlanning.value = false
        isPreviewing.value = executionMode.value !== 'auto'
        plannedTree.value = msg.tree
        if (executionMode.value === 'auto') {
          // 自动模式：不预览，直接进入执行
        }
        break

      case 'tree_started':
        isPlanning.value = true
        treeCompleted.value = false
        currentTreeId.value = msg.tree_id
        initTaskBubblesFromTree()
        isPreviewing.value = false
        break

      case 'node_started':
        if (!taskBubbles.value.includes(msg.node_id)) {
          taskBubbles.value.push(msg.node_id)
        }
        if (executionState[msg.node_id]) {
          executionState[msg.node_id].status = 'running'
          executionState[msg.node_id].startTime = Date.now()
        }
        break

      case 'streaming_token':
        if (executionState[msg.node_id]) {
          executionState[msg.node_id].streamContent += msg.token
        }
        break

      case 'node_completed':
        if (executionState[msg.node_id]) {
          executionState[msg.node_id].status = 'completed'
          executionState[msg.node_id].endTime = Date.now()
          if (msg.output) executionState[msg.node_id].output = msg.output
          // 检测文件输出
          const file = detectFileOutput(msg.output)
          if (file) resultFiles.value.push(file)
        }
        break

      case 'node_failed':
        if (executionState[msg.node_id]) {
          executionState[msg.node_id].status = 'failed'
          executionState[msg.node_id].error = msg.error
          executionState[msg.node_id].endTime = Date.now()
        }
        break

      case 'tree_completed':
        isPlanning.value = false
        treeCompleted.value = true
        if (_onTreeCompleted) _onTreeCompleted(msg.success !== false)
        break

      case 'final_answer':
        if (msg.content && _onFinalAnswer) {
          _onFinalAnswer(msg.content)
        }
        break

      case 'thinking_update': {
        const content = msg.data?.content || msg.content || ''
        if (!content) break
        if (ssePhase === 'planning' || !currentTreeId.value) {
          addPlanningLog(content)
        }
        break
      }
    }
  }

  // ===== 辅助函数 =====
  function initTaskBubblesFromTree() {
    taskBubbles.value = []
    Object.keys(executionState).forEach(k => delete executionState[k])
    if (!plannedTree.value?.root?.children) return
    for (const phase of plannedTree.value.root.children) {
      for (const task of (phase.children || [])) {
        taskBubbles.value.push(task.id)
        executionState[task.id] = {
          id: task.id,
          name: task.name,
          status: 'pending',
          logs: [],
          streamContent: '',
          output: '',
          error: null,
          startTime: null,
          endTime: null,
        }
      }
    }
  }

  function addPlanningLog(content: string, phase?: string) {
    let type = 'reasoning'
    if (content.includes('🔧') || content.includes('tool_call')) type = 'tool_call'
    else if (content.includes('✅') || content.includes('tool_result')) type = 'tool_result'
    else if (content.includes('❌')) type = 'error'
    // 去重
    const last = planningLogs.value[planningLogs.value.length - 1]
    if (last?.content === content) return
    planningLogs.value.push({ type, content: content.trim() })
  }

  function detectFileOutput(content: any): { name: string; url: string } | null {
    if (!content) return null
    try {
      const obj = typeof content === 'string' ? JSON.parse(content.trim()) : content
      if (obj.file_path || obj.download_url) {
        const url = obj.download_url || obj.file_path
        return { name: obj.file_name || url.split('/').pop() || '文件', url }
      }
      if (obj.download?.download_url) {
        return { name: obj.download.files?.[0]?.name || '文件', url: obj.download.download_url }
      }
    } catch {}
    return null
  }

  // ===== 核心动作 =====

  /**
   * 发送规划请求
   * 由 ChatView 在规划模式下调用
   */
  async function startPlanning(prompt: string): Promise<{ sessionId: string; error?: string }> {
    // 防止重复点击：已在规划中则直接返回当前 session
    if (isPlanning.value && currentSessionId.value) {
      return { sessionId: currentSessionId.value }
    }
    // 1. 创建 session
    let sessionId = currentSessionId.value
    if (!sessionId) {
      try {
        await waitForDependencyReady('fastplan')
        const res = await fastPlanRequest('/api/sessions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ title: prompt.substring(0, 50) }),
        }, fastClawFastPlanAccess)
        if (res.ok) {
          const session = await res.json()
          sessionId = session.id
          currentSessionId.value = sessionId
        } else {
          return { sessionId: '', error: '创建规划会话失败' }
        }
      } catch (e) {
        return { sessionId: '', error: '连接规划服务失败' }
      }
    }

    // 2. 重置状态
    isPlanning.value = true
    isPreviewing.value = false
    treeCompleted.value = false
    plannedTree.value = null
    taskBubbles.value = []
    planningLogs.value = []
    resultFiles.value = []
    currentTreeId.value = null
    Object.keys(executionState).forEach(k => delete executionState[k])

    // 3. 连接 WS
    await connectWs(sessionId!)

    // 4. 等待 WS 就绪
    const wsReady = await waitForWs(5000)
    if (!wsReady) {
      isPlanning.value = false
      return { sessionId: sessionId!, error: 'WebSocket 连接超时' }
    }

    // 5. 连接 SSE
    ssePhase = 'planning'
    try {
      await connectSse(sessionId!)
    } catch {
      // SSE 连接失败不阻断，WS 也能接收事件
      console.warn('[Plan] SSE 连接失败')
    }

    // 6. 发送消息
    const sent = wsSendChat(prompt)
    if (!sent) {
      isPlanning.value = false
      return { sessionId: sessionId!, error: '发送失败' }
    }

    return { sessionId: sessionId! }
  }

  async function waitForWs(timeout: number): Promise<boolean> {
    if (wsConnected.value) return true
    return new Promise(resolve => {
      let elapsed = 0
      const check = () => {
        if (wsConnected.value) return resolve(true)
        elapsed += 100
        if (elapsed > timeout) return resolve(false)
        setTimeout(check, 100)
      }
      check()
    })
  }

  function confirmPlan(modifiedTree?: PlanTree | null) {
    if (modifiedTree) {
      plannedTree.value = JSON.parse(JSON.stringify(modifiedTree))
    }
    isPreviewing.value = false
    isPlanning.value = true
    wsConfirmPlan(modifiedTree || plannedTree.value)
  }

  function cancelPlan() {
    wsCancelPlan()
    isPreviewing.value = false
    plannedTree.value = null
    isPlanning.value = false
  }

  function reset() {
    isPlanning.value = false
    isPreviewing.value = false
    treeCompleted.value = false
    plannedTree.value = null
    taskBubbles.value = []
    planningLogs.value = []
    resultFiles.value = []
    currentTreeId.value = null
    Object.keys(executionState).forEach(k => delete executionState[k])
    disconnectWs()
    closeSse()
    currentSessionId.value = null
  }

  function toggleEnabled() {
    enabled.value = !enabled.value
    if (!enabled.value) {
      reset()
    }
  }

  function setExecutionMode(mode: string) {
    executionMode.value = mode
    localStorage.setItem('fastclaw_plan_mode', mode)
  }

  // ===== 回调钩子（供 ChatView 注册） =====
  let _onFinalAnswer: ((content: string) => void) | null = null
  let _onPlanGenerated: ((tree: PlanTree) => void) | null = null
  let _onTreeCompleted: ((success: boolean) => void) | null = null

  function registerCallbacks(callbacks: {
    onFinalAnswer?: (content: string) => void
    onPlanGenerated?: (tree: PlanTree) => void
    onTreeCompleted?: (success: boolean) => void
  }) {
    if (callbacks.onFinalAnswer) _onFinalAnswer = callbacks.onFinalAnswer
    if (callbacks.onPlanGenerated) _onPlanGenerated = callbacks.onPlanGenerated
    if (callbacks.onTreeCompleted) _onTreeCompleted = callbacks.onTreeCompleted
  }

  return {
    // 状态
    enabled,
    isPlanning: readonly(isPlanning),
    isPreviewing: readonly(isPreviewing),
    treeCompleted: readonly(treeCompleted),
    currentSessionId: readonly(currentSessionId),
    currentTreeId: readonly(currentTreeId),
    plannedTree: readonly(plannedTree),
    executionState: readonly(executionState),
    taskBubbles: readonly(taskBubbles),
    planningLogs: readonly(planningLogs),
    resultFiles: readonly(resultFiles),
    executionMode,
    wsConnected: readonly(wsConnected),
    sseConnected: readonly(sseConnected),

    // Getters
    stats,
    phasedBubbles,

    // 动作
    toggleEnabled,
    setExecutionMode,
    startPlanning,
    confirmPlan,
    cancelPlan,
    reset,
    connectWs,
    disconnectWs,
    registerCallbacks,

    // 内部状态（供组件直接修改）
    _isPlanning: isPlanning,
    _isPreviewing: isPreviewing,
    _plannedTree: plannedTree,
    _executionState: executionState,
    _taskBubbles: taskBubbles,
  }
})
