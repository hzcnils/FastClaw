<script setup lang="ts">
import { ref, computed, onMounted, nextTick } from 'vue'
import { useFastPlanStore } from './session'
import { useWebSocket } from './websocket'
import type { TaskTree, TaskTreeNode } from './treeToDAG'
import TaskItem from './TaskItem.vue'
import ChatMessage from './ChatMessage.vue'
import PlanPreview from './PlanPreview.vue'
import ExecutionTimeline from './ExecutionTimeline.vue'
import ExecutionToolbar from './ExecutionToolbar.vue'
import TodoPanel from './TodoPanel.vue'
import NodeEditor from './NodeEditor.vue'

const FAST_PLAN_BASE = '/FastPlan'

const props = defineProps<{
  dataObj?: { question?: string; sessionId?: string } | null
  /** 当前选中的模型 ID，来自 fastclaw ChatView 的模型选择器 */
  selectedModel?: string
  /** 仅复用原初始化欢迎态，不连接旧 FastPlan 服务。 */
  presentationOnly?: boolean
  /** presentationOnly 模式由公共运行时包提供的真实服务连接状态。 */
  connectionStatus?: 'connecting' | 'connected' | 'disconnected'
}>()

const emit = defineEmits<{
  'scrollToBottom': []
  'saveFastPlan': [sessionId: string, prompt: string]
}>()

const store = useFastPlanStore()
const ws = useWebSocket()

const isConnected = ws.isConnected
const welcomeConnectionStatus = computed(() => props.presentationOnly
  ? (props.connectionStatus || 'disconnected')
  : (isConnected.value ? 'connected' : 'disconnected'))
const welcomeConnectionLabel = computed(() => ({
  connecting: '○ 服务启动/连接中...',
  connected: '● 已连接',
  disconnected: '○ 未连接',
})[welcomeConnectionStatus.value])
const currentHumanRequest = computed(() => store.currentHumanRequest)
const taskList = computed(() => store.taskList)

const showTodoPanel = ref(false)
const showNodeEditor = ref(false)
const editingNode = ref<TaskTreeNode | null>(null)

onMounted(async () => {
  console.log('[FastPlanView] mounted, dataObj:', props.dataObj)
  if (props.presentationOnly) return
  ws.connect()
  if (props.dataObj?.question) {
    await handleSubmit(props.dataObj.question)
  } else if (props.dataObj?.sessionId) {
    await processData(props.dataObj.sessionId)
  }
})

async function processData(id: string) {
  await store.loadSession(id)
  ws.subscribeSession(id)
  emit('scrollToBottom')
}

async function handleSubmit(message: string) {
  if (!isConnected.value) {
    alert('未连接到 FastPlan 服务，请检查服务是否运行。')
    return
  }
  const prompt = message.trim()
  if (!prompt) return

  let sessionId = store.currentSessionId
  if (!sessionId) {
    const session = await store.createSession(prompt.substring(0, 50))
    if (!session) return
    sessionId = session.id
    store.currentSessionId = sessionId
    await processData(sessionId)
    ws.subscribeSession(sessionId)
    emit('saveFastPlan', sessionId, prompt)
  }

  store.isPlanning = true
  store.clearExecutionState()
  store.messages.push({ role: 'system', content: 'FastPlan 正在规划', timestamp: new Date(), isThinking: true })
  emit('scrollToBottom')

  try {
    const body: Record<string, unknown> = { input: prompt, preview: true, session_id: sessionId }
    if (props.selectedModel) {
      body.model = props.selectedModel
    }
    const response = await fetch(`${FAST_PLAN_BASE}/api/plan`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    })
    if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`)
  } catch (error: any) {
    store.isPlanning = false
    store.removeThinkingMessage()
    store.addMessage({ role: 'system', content: `❌ 提交失败: ${error.message}`, timestamp: new Date(), isError: true })
  }
  emit('scrollToBottom')

  if (store.messages.filter(m => m.role === 'user').length <= 1 && sessionId) {
    store.updateSessionTitle(sessionId, prompt.substring(0, 50))
  }
}

async function handleConfirmPlan(modifiedTree: TaskTree) {
  if (!modifiedTree) return
  store.isPreviewing = false
  store.isPlanning = true
  store.addMessage({ role: 'user', content: '确认计划，开始执行', timestamp: new Date() })
  emit('scrollToBottom')

  try {
    await fetch(`${FAST_PLAN_BASE}/api/execute_tree`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        ...modifiedTree,
        session_id: store.currentSessionId,
        step_mode: store.executionMode === 'step',
      }),
    })
  } catch (e: any) {
    store.isPlanning = false
    store.addMessage({ role: 'system', content: `❌ 执行请求失败: ${e.message}`, timestamp: new Date(), isError: true })
  }
}

function handleCancelPlan() {
  store.isPreviewing = false
  store.plannedTree = null
  store.isPlanning = false
  store.addMessage({ role: 'system', content: '已取消任务规划', timestamp: new Date() })
  emit('scrollToBottom')
}

async function handleRetry(taskId: string) {
  try {
    if (store.executionState[taskId]) {
      store.executionState[taskId].status = 'running'
      store.executionState[taskId].error = null
    }
    await fetch(`${FAST_PLAN_BASE}/api/retry_task`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ tree_id: store.currentTreeId, node_id: taskId }),
    })
  } catch (e: any) {
    if (store.executionState[taskId]) {
      store.executionState[taskId].status = 'failed'
      store.executionState[taskId].error = e.message
    }
  }
}

async function handleHumanResponse(data: Record<string, unknown>) {
  const targetRequest = store.currentHumanRequest
  if (!targetRequest) return
  const displayContent = String(data.value || data.action)
  store.addMessage({ role: 'user', content: displayContent, timestamp: new Date() })
  try {
    await fetch(`${FAST_PLAN_BASE}/api/checkpoint`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ tree_id: targetRequest.tree_id, node_id: targetRequest.node_id, data }),
    })
  } catch (e: any) {
    store.addMessage({ role: 'system', content: `❌ 确认失败: ${e.message}`, timestamp: new Date(), isError: true })
  }
  store.humanRequestQueue.shift()
  emit('scrollToBottom')
}

function handlePause() {
  if (store.currentTreeId) ws.sendPauseTree(store.currentTreeId)
}
function handleResume() {
  if (store.currentTreeId) ws.sendResumeTree(store.currentTreeId)
}
function handleSkipNode(nodeId: string) {
  if (store.currentTreeId) ws.sendSkipNode(store.currentTreeId, nodeId)
}
function handleReplan() {
  store.clearExecutionState()
  store.isPlanning = false
}

function handleNodeSelect(nodeId: string) {
  store.selectNode(nodeId)
}

function handleNodeEdit(nodeId: string) {
  if (!store.plannedTree?.root?.children) return
  for (const phase of store.plannedTree.root.children) {
    if (phase.id === nodeId) { editingNode.value = phase; showNodeEditor.value = true; return }
    for (const task of (phase.children || [])) {
      if (task.id === nodeId) { editingNode.value = task; showNodeEditor.value = true; return }
    }
  }
}

function handleNodeEditorSave(updatedNode: TaskTreeNode) {
  if (!store.plannedTree?.root?.children || !updatedNode) return
  for (const phase of store.plannedTree.root.children) {
    if (phase.id === updatedNode.id) { Object.assign(phase, updatedNode); break }
    const idx = (phase.children || []).findIndex(t => t.id === updatedNode.id)
    if (idx !== -1) { Object.assign(phase.children![idx], updatedNode); break }
  }
  if (store.currentTreeId) ws.sendTreeEdit(store.currentTreeId, updatedNode.id, updatedNode)
  showNodeEditor.value = false
  editingNode.value = null
}

defineExpose({ handleRetry, handleSkipNode, handleResume, handleNodeEdit, processData, handleSubmit })
</script>

<template>
  <div class="fastplan-view">
    <!-- 初始欢迎态：位置与 ChatView welcome-screen 对齐 -->
    <div
      v-if="props.presentationOnly || (!store.messages.length && !store.isPlanning && !Object.keys(store.executionState).length)"
      class="fastplan-welcome"
    >
      <div class="fastplan-welcome-inner">
        <span class="welcome-logo welcome-logo-svg fastplan-welcome-logo">
          <svg xmlns="http://www.w3.org/2000/svg" width="58" height="58" viewBox="0 0 58 58" aria-hidden="true">
            <g transform="translate(-16436.793 -97.793)">
              <rect width="58" height="58" transform="translate(16436.793 97.793)" fill="currentColor" opacity="0.02" />
              <g transform="translate(16437.793 103.793)">
                <g transform="translate(0.001 2.223)">
                  <path
                    d="M540.03,518.039a8.94,8.94,0,0,0,.177-1.755,9.312,9.312,0,0,0-.063-1.035l.979-1.111c1-1.275,1.458-1.844.941-2.614l-1.256-1.862c-.524-.77-.972-.612-3.125-.133l-.467.2a8.743,8.743,0,0,0-2.248-1.421l-.335-1.326c-.474-1.547-.688-2.254-1.616-2.26l-2.241-.019c-.928-.006-1.054.455-1.881,2.494l-.114.549-.063.316a7.271,7.271,0,0,0-1.654.8,8.758,8.758,0,0,0-1.212.884l-1.345-.366c-1.578-.341-2.3-.505-2.759.3l-1.117,1.951c-.461.8-.12,1.143,1.256,2.86l1.023.89a8.906,8.906,0,0,0-.051.922,8.688,8.688,0,0,0,.158,1.629c-.057.051-.114.1-.177.164l-.17.29L522,519.535c-.12.234-.221.442-.309.619a1.938,1.938,0,0,0-.1,2.128l1.6,1.578c.657.65,1.073.41,3.087-.48l.126-.088a8.79,8.79,0,0,0,2.355,1.225l.253.89c.524,1.528.758,2.222,1.686,2.2l2.241-.05c.934-.019,1.042-.486,1.806-2.551l.12-.694a8.992,8.992,0,0,0,1.673-.928l.063.044,1.6.474c1.572.372,2.285.549,2.765-.246l1.155-1.919c.48-.8.145-1.143-1.193-2.891Zm-8.662,2.513a4.339,4.339,0,0,1-2.4-.713,4.547,4.547,0,0,1-1.376-1.421,4.408,4.408,0,0,1,3.775-6.679c.145,0,.29.006.429.019a4.408,4.408,0,0,1-.429,8.795Z"
                    transform="translate(-491.63 -484.566)"
                    fill="currentColor"
                  />
                  <path
                    d="M94.362,218.8a9.948,9.948,0,0,1-.088-1.307c0-.152.006-.3.013-.442l-.6-.518a1,1,0,0,1-.139-.145,10.931,10.931,0,0,1-1.522-2.14,2.028,2.028,0,0,1-.17-.821,2.289,2.289,0,0,1,.341-1.155l1.117-1.945a2.2,2.2,0,0,1,.871-.878,2.373,2.373,0,0,1,1.143-.265,10.3,10.3,0,0,1,1.951.3l.057.013.783.221a7.853,7.853,0,0,1,.707-.492,15.435,15.435,0,0,0,.29-2.98A14.52,14.52,0,0,0,99,204.429l1.724-1.963c1.774-2.241,2.582-3.251,1.667-4.615l-2.216-3.283c-.922-1.357-1.717-1.08-5.512-.24l-.827.347a15.5,15.5,0,0,0-3.965-2.506l-.587-2.342c-.84-2.734-1.212-3.971-2.854-3.99l-3.965-.038c-1.641-.013-1.862.8-3.321,4.407l-.316,1.528a15.19,15.19,0,0,0-5.095,2.936l-2.374-.644c-2.8-.6-4.059-.884-4.874.537L64.519,198c-.814,1.427-.215,2.02,2.216,5.057l1.806,1.566a15.231,15.231,0,0,0,.183,4.5c-.095.082-.2.177-.309.29l-1.471,2.544c-1.3,2.544-1.566,2.974-.732,4.849l2.816,2.784c1.168,1.155,1.894.726,5.455-.846l.221-.152a15.264,15.264,0,0,0,4.16,2.165l.448,1.572c.928,2.7,1.338,3.933,2.98,3.9l3.958-.088c1.641-.038,1.837-.859,3.182-4.508l.215-1.225a15.365,15.365,0,0,0,2.961-1.635l.12.076,1.376.4ZM83.5,213.816a7.784,7.784,0,1,1,7.784-7.784A7.784,7.784,0,0,1,83.5,213.816Z"
                    transform="translate(-64.177 -185.798)"
                    fill="currentColor"
                  />
                </g>
                <g transform="translate(41.47 0)">
                  <path
                    d="M106.057,127.916H96.935a3.126,3.126,0,0,1-2.983-3.246v-7.658a3.126,3.126,0,0,1,2.983-3.246h9.122a3.126,3.126,0,0,1,2.983,3.246v4.044a.553.553,0,0,1-1.105,0v-4.044a2.025,2.025,0,0,0-1.878-2.141H96.935a2.025,2.025,0,0,0-1.878,2.141v7.658a2.025,2.025,0,0,0,1.878,2.141h9.122a2.025,2.025,0,0,0,1.878-2.141.553.553,0,0,1,1.105,0A3.126,3.126,0,0,1,106.057,127.916Z"
                    transform="translate(-93.952 -113.766)"
                    fill="currentColor"
                    stroke="currentColor"
                    stroke-width="0.4"
                  />
                  <path
                    d="M241.726,283.2l-1.684,1.77-.728-.751a.552.552,0,1,0-.793.769l1.128,1.164a.55.55,0,0,0,.4.168h0a.556.556,0,0,0,.4-.171l2.081-2.187a.553.553,0,0,0-.8-.762Zm2.229,2.027h3.962a.553.553,0,1,0,0-1.105h-3.962a.553.553,0,1,0,0,1.105Zm3.962,3.545h-3.962a.553.553,0,0,0,0,1.105h3.962a.553.553,0,1,0,0-1.105Zm-7.4-1.687a1.963,1.963,0,1,0,1.963,1.963A1.966,1.966,0,0,0,240.521,287.08Zm0,2.822a.858.858,0,1,1,.858-.858A.859.859,0,0,1,240.521,289.9Z"
                    transform="translate(-235.766 -279.979)"
                    fill="currentColor"
                    stroke="currentColor"
                    stroke-width="0.4"
                  />
                </g>
              </g>
            </g>
          </svg>
        </span>
        <div class="fastplan-welcome-text">
          <div class="fastplan-welcome-title">任务规划引擎</div>
          <div class="fastplan-welcome-sub">在下方输入框中描述您的任务</div>
          <div class="fastplan-welcome-sub">AI 将自动拆解为可执行的步骤并逐步完成</div>
        </div>
        <div
          class="mt-4 px-3 py-1.5 rounded-full text-sm"
          :class="welcomeConnectionStatus === 'connected'
            ? 'bg-green-50 text-green-600'
            : welcomeConnectionStatus === 'connecting'
              ? 'bg-amber-50 text-amber-600'
              : 'bg-red-50 text-red-500'"
        >
          {{ welcomeConnectionLabel }}
        </div>
      </div>
    </div>

    <div v-else class="w-full p-4">
      <div class="w-full">
        <div v-if="store.messages.length > 0" class="space-y-4 mb-6">
          <ChatMessage v-for="(msg, idx) in store.messages" :key="idx" :message="msg" />
        </div>

        <div v-if="store.isPreviewing && store.plannedTree" class="mb-6">
          <PlanPreview :tree="store.plannedTree" :executionMode="store.executionMode"
            @confirm="handleConfirmPlan" @cancel="handleCancelPlan" />
        </div>

        <div v-if="(Object.keys(store.executionState).length > 0 || store.isPlanning) && !store.isPreviewing" class="space-y-4">
          <ExecutionToolbar :executionState="store.executionState" :isPaused="store.isPaused"
            :isPlanning="store.isPlanning" :executionMode="store.executionMode"
            @pause="handlePause" @resume="handleResume" @skip-current="handleSkipNode" @replan="handleReplan" />

          <div v-if="store.executionMode === 'step' && store.isPaused && store.stepConfirmNodeId"
            class="bg-violet-50 border border-violet-200 rounded-xl p-4 flex items-center justify-between">
            <div>
              <p class="font-medium text-violet-800">等待确认下一步</p>
              <p class="text-sm text-violet-600">节点已完成，点击继续执行下一个节点</p>
            </div>
            <div class="flex gap-2">
              <button @click="handleResume" class="px-4 py-2 bg-violet-600 text-white text-sm font-medium rounded-lg hover:bg-violet-700 transition-colors">▶ 继续执行</button>
              <button @click="handleSkipNode(store.stepConfirmNodeId!)" class="px-3 py-2 border border-violet-300 text-violet-700 text-sm font-medium rounded-lg hover:bg-violet-100 transition-colors">⏭ 跳过</button>
            </div>
          </div>

          <div v-if="store.plannedTree" class="rounded-2xl border border-gray-200 shadow-sm overflow-hidden p-6">
            <ExecutionTimeline :tree="store.plannedTree" :executionState="store.executionState"
              :currentHumanRequest="currentHumanRequest" :isPaused="store.isPaused" :executionMode="store.executionMode"
              @retry="handleRetry" @human-response="handleHumanResponse"
              @skip-node="handleSkipNode" @pause="handlePause" @resume="handleResume"
              @select-node="handleNodeSelect" />
          </div>
          <div v-else class="rounded-2xl border border-gray-200 shadow-sm overflow-hidden">
            <TaskItem v-for="task in taskList" :key="task.id" :task="task" mode="standalone" @retry="handleRetry(task.id)" />
          </div>
        </div>
      </div>
    </div>

    <transition enter-active-class="transition-all duration-200" leave-active-class="transition-all duration-150"
      enter-from-class="translate-x-full opacity-0" leave-to-class="translate-x-full opacity-0">
      <TodoPanel v-if="showTodoPanel" :todos="store.todos"
        @add="store.addTodo" @toggle="store.toggleTodo" @delete="store.deleteTodo" @close="showTodoPanel = false" />
    </transition>

    <NodeEditor :node="editingNode" :visible="showNodeEditor"
      @close="showNodeEditor = false; editingNode = null" @save="handleNodeEditorSave" />
  </div>
</template>

<style scoped>
.fastplan-view {
  width: 100%;
  height: 100%;
  min-height: 0;
}

.fastplan-welcome {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-start;
  padding-top: clamp(48px, 12vh, 120px);
  gap: 18px;
  text-align: center;
}

.fastplan-welcome-inner {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
}

.fastplan-welcome-logo {
  width: 65px;
  height: 65px;
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.fastplan-welcome-logo :deep(svg) {
  width: 100%;
  height: 100%;
  display: block;
}

.fastplan-welcome-text {
  text-align: center;
}

.fastplan-welcome-title {
  font-size: var(--font-size-3xl);
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 6px;
}

.fastplan-welcome-sub {
  font-size: var(--font-size-base);
  color: var(--text-secondary);
}

</style>
