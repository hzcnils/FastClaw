import { ref } from 'vue'
import { useFastPlanStore } from './session'
import { getServiceUrl } from '@/config/index'

/** 动态获取 FastPlan 基础 URL */
export function getFastPlanBase(): string {
  const svc = getServiceUrl('fastplan')
  if (svc) return svc
  if (import.meta.env.DEV) return '/FastPlan'
  return `${location.protocol}//${location.host}/FastPlan`
}

const FAST_PLAN_BASE = getFastPlanBase()

/** 从动态配置解析 WebSocket URL（延迟到首次连接时，确保 Store 已初始化） */
let _wsUrl: string | null = null
function resolveWsUrl(): string {
  if (_wsUrl) return _wsUrl
  const fastplanUrl = getServiceUrl('fastplan')
  if (fastplanUrl) {
    _wsUrl = fastplanUrl.replace(/^http/, 'ws') + '/ws'
  } else {
    _wsUrl = 'ws://127.0.0.1:23180/ws'
  }
  return _wsUrl
}

export const isConnected = ref(false)
let socket: WebSocket | null = null
let reconnectAttempts = 0
let pendingSubscriptions: string[] = []
const maxReconnectAttempts = 10
const reconnectDelay = 3000

export function connect() {
  if (socket && socket.readyState === WebSocket.OPEN) return

  const ws = new WebSocket(resolveWsUrl())

  ws.onopen = () => {
    console.log('✅ FastPlan WebSocket Connected')
    isConnected.value = true
    reconnectAttempts = 0
    // 重放待订阅队列
    if (pendingSubscriptions.length > 0) {
      pendingSubscriptions.forEach(sid => send({ type: 'subscribe', session_id: sid }))
      pendingSubscriptions = []
    }
    const store = useFastPlanStore()
    if (store.currentSessionId && !pendingSubscriptions.includes(store.currentSessionId)) {
      send({ type: 'subscribe', session_id: store.currentSessionId })
    }
  }

  ws.onmessage = (event: MessageEvent) => {
    try {
      const msg = JSON.parse(event.data)
      handleMessage(msg)
    } catch (e) {
      console.error('WS Parse Error:', e)
      const store = useFastPlanStore()
      store.addMessage({ role: 'system', content: '⚠️ 规划服务响应异常，请重试', timestamp: new Date(), isError: true })
    }
  }

  ws.onclose = () => {
    console.log('❌ FastPlan WebSocket Closed')
    isConnected.value = false
    if (reconnectAttempts < maxReconnectAttempts) {
      reconnectAttempts++
      setTimeout(connect, reconnectDelay)
    }
  }

  ws.onerror = (error: Event) => {
    console.error('FastPlan WebSocket Error:', error)
  }

  socket = ws
}

export function send(data: unknown) {
  if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify(data))
  }
}

export function subscribeSession(sessionId: string) {
  if (socket && socket.readyState === WebSocket.OPEN) {
    send({ type: 'subscribe', session_id: sessionId })
  } else {
    if (!pendingSubscriptions.includes(sessionId)) {
      pendingSubscriptions.push(sessionId)
    }
  }
}

export function sendTreeEdit(treeId: string, nodeId: string, changes: unknown) {
  send({ type: 'edit_node', tree_id: treeId, node_id: nodeId, changes })
}

export function sendSkipNode(treeId: string, nodeId: string) {
  send({ type: 'skip_node', tree_id: treeId, node_id: nodeId })
}

export function sendPauseTree(treeId: string) {
  send({ type: 'pause_tree', tree_id: treeId })
}

export function sendResumeTree(treeId: string) {
  send({ type: 'resume_tree', tree_id: treeId })
}

export function sendInsertNode(treeId: string, afterNodeId: string, node: unknown) {
  send({ type: 'insert_node', tree_id: treeId, after_node_id: afterNodeId, node })
}

export function disconnect() {
  if (socket) {
    socket.close()
    socket = null
  }
}

function countTreeTasks(tree: any): number {
  if (!tree?.root?.children) return 0
  return tree.root.children.reduce((sum: number, phase: any) => sum + (phase.children?.length || 0), 0)
}

function handleMessage(msg: any) {
  const store = useFastPlanStore()

  switch (msg.type) {
    case 'plan_generated':
      store.removeThinkingMessage()
      store.plannedTree = msg.tree
      store.isPlanning = false

      if (store.executionMode === 'auto') {
        store.isPreviewing = false
        store.addMessage({
          role: 'assistant',
          content: `已自动开始执行规划（共 ${countTreeTasks(msg.tree)} 个任务）`,
          timestamp: new Date(),
          isPlan: true,
        })
        fetch(`${FAST_PLAN_BASE}/api/execute_tree`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ ...msg.tree, session_id: store.currentSessionId, step_mode: false }),
        }).then(r => { if (!r.ok) store.addMessage({ role:'system', content:'⚠️ 自动执行失败，请手动确认方案', timestamp:new Date(), isError:true }) }).catch(e => { console.error('Auto-execute failed:', e); store.addMessage({ role:'system', content:'⚠️ 自动执行失败', timestamp:new Date(), isError:true }) })
      } else {
        store.isPreviewing = true
        store.addMessage({
          role: 'assistant',
          content: '已为您生成任务规划，请确认方案：',
          timestamp: new Date(),
          isPlan: true,
        })
      }
      break

    case 'tree_started':
      store.removeThinkingMessage()
      store.isPlanning = true
      store.currentTreeId = msg.tree_id
      store.humanRequestQueue = []
      if (!store.isPreviewing) {
        store.addMessage({
          role: 'system',
          content: '开始执行任务规划...',
          timestamp: new Date(),
          nodeCount: msg.node_count,
        })
      }
      store.isPreviewing = false
      break

    case 'node_started':
      if (msg.node_id === '0') return
      store.executionState[msg.node_id] = {
        id: msg.node_id,
        name: msg.node_name,
        status: 'running',
        streamContent: '',
        output: '',
        error: null,
        startTime: Date.now(),
        endTime: null,
      }
      break

    case 'streaming_token':
      if (store.executionState[msg.node_id]) {
        store.executionState[msg.node_id].streamContent += msg.token
      }
      break

    case 'node_completed': {
      let isInternalNode = false
      if (msg.output) {
        try {
          const jsonOut = JSON.parse(msg.output)
          if (jsonOut.type === 'root') isInternalNode = true
        } catch (_) { /* ignore */ }
      }
      if (isInternalNode) {
        if (store.executionState[msg.node_id]) delete store.executionState[msg.node_id]
        return
      }
      if (store.executionState[msg.node_id]) {
        store.executionState[msg.node_id].status = 'completed'
        store.executionState[msg.node_id].endTime = Date.now()
        if (msg.output) store.executionState[msg.node_id].output = msg.output
      }
      if (store.executionMode === 'step' && store.currentTreeId) {
        sendPauseTree(store.currentTreeId)
        store.isPaused = true
      }
      break
    }

    case 'node_failed':
      if (store.executionState[msg.node_id]) {
        store.executionState[msg.node_id].status = 'failed'
        store.executionState[msg.node_id].error = msg.error
        store.executionState[msg.node_id].endTime = Date.now()
      }
      break

    case 'waiting_for_human': {
      const newRequest = {
        tree_id: msg.tree_id,
        node_id: msg.node_id,
        prompt: msg.prompt,
        options: msg.options,
        timeout_seconds: msg.timeout_seconds || 60,
        timestamp: Date.now(),
      }
      const exists = store.humanRequestQueue.some(req => req.node_id === msg.node_id)
      if (!exists) store.humanRequestQueue.push(newRequest)
      store.addMessage({
        role: 'assistant',
        content: msg.prompt,
        timestamp: new Date(),
        isQuestion: true,
      })
      break
    }

    case 'reflection':
      store.addMessage({
        role: 'system',
        content: `[${msg.phase}] ${msg.content}`,
        timestamp: new Date(),
        isReflection: true,
      })
      break

    case 'tree_completed':
      store.isPlanning = false
      store.humanRequestQueue = []
      store.isPaused = false
      store.stepConfirmNodeId = null
      store.addMessage({
        role: 'system',
        content: msg.success ? '✅ 任务执行完成' : '❌ 任务执行失败',
        timestamp: new Date(),
        duration: msg.duration_ms,
      })
      store.saveExecutionSnapshot()
      break

    case 'tree_paused':
      store.isPaused = true
      break

    case 'tree_resumed':
      store.isPaused = false
      store.stepConfirmNodeId = null
      break

    case 'node_skipped':
      if (store.executionState[msg.node_id]) {
        store.executionState[msg.node_id].status = 'skipped'
        store.executionState[msg.node_id].endTime = Date.now()
      }
      break

    case 'tree_modified':
      store.plannedTree = msg.tree
      break

    case 'session_updated':
      if (msg.field === 'title') {
        const session = store.sessions.find(ss => ss.id === msg.session_id)
        if (session) session.title = msg.value
      }
      break
  }
}

export function useWebSocket() {
  return {
    isConnected,
    connect,
    disconnect,
    send,
    subscribeSession,
    sendTreeEdit,
    sendSkipNode,
    sendPauseTree,
    sendResumeTree,
    sendInsertNode,
  }
}
