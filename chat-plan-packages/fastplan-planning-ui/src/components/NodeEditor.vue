<script setup>
import { ref, computed, watch, nextTick, onMounted, onUnmounted, inject } from 'vue'
import { X, Brain, Wrench, User, Bot, Plus, Trash2, Save, Play, ChevronDown, Search, Link, Loader2, Check } from 'lucide-vue-next'
import { ChatModelIcon } from 'chat-renderer'
import { createFastPlanAdminClient, fastPlanRequest, FASTPLAN_ACCESS_KEY } from 'planflow-runtime'
const access = inject(FASTPLAN_ACCESS_KEY, undefined)
import { resolveNodeModel } from '../utils/nodeModel'

const props = defineProps({
  node: { type: Object, default: null },
  visible: { type: Boolean, default: false },
  // 'plan'：规划阶段编辑  'execution'：执行中编辑（显示"保存并继续"按钮）
  mode: { type: String, default: 'plan' },
  apiBase: { type: String, default: '' },
  adminBase: { type: String, default: '' },
  // 所有可用节点列表（用于依赖选择）
  allNodes: { type: Array, default: () => [] }
})

const { skillsApi, mcpApi, workflowApi } = createFastPlanAdminClient({ apiBase: props.adminBase || props.apiBase, access })

const emit = defineEmits(['close', 'save', 'save-and-continue'])

const editData = ref(null)
const promptTextareaRef = ref(null)
const isDirty = ref(false)

// ─── 依赖选择器状态 ──────────────────────────────────────────
const showDependencySelector = ref(false)
const dependencySearchQuery = ref('')

watch(() => props.node, (newNode) => {
  if (newNode) {
    editData.value = JSON.parse(JSON.stringify(newNode))
    isDirty.value = false
    nextTick(() => autoResizeTextarea())
  }
}, { immediate: true, deep: true })

watch(() => props.visible, (v) => {
  if (v) nextTick(() => autoResizeTextarea())
})

const nodeTypes = [
  { id: 'LlmTask',         label: 'AI 推理',  icon: Brain, color: 'blue',   desc: '由 AI 大模型完成的推理任务' },
  { id: 'ToolTask',        label: '工具调用', icon: Wrench, color: 'amber',  desc: '调用外部工具或函数' },
  { id: 'HumanCheckpoint', label: '人工确认', icon: User,   color: 'blue',   desc: '需要人工介入确认的检查点' },
  { id: 'AgentTask',       label: 'Agent',    icon: Bot,    color: 'violet', desc: '调用内置 Agent（含 OODA 闭环）' },
]

const currentType = computed(() => editData.value?.node_type?.type || 'LlmTask')
const currentTypeDef = computed(() => nodeTypes.find(t => t.id === currentType.value))
const currentModel = computed(() => resolveNodeModel(editData.value))

const promptValue = computed({
  get: () => {
    if (!editData.value?.node_type?.config) return ''
    const cfg = editData.value.node_type.config
    return cfg.prompt_template || cfg.prompt || ''
  },
  set: (val) => {
    if (!editData.value?.node_type?.config) return
    isDirty.value = true
    if (currentType.value === 'LlmTask') editData.value.node_type.config.prompt_template = val
    else if (currentType.value === 'HumanCheckpoint') editData.value.node_type.config.prompt = val
  }
})

const promptCharCount = computed(() => promptValue.value.length)

const switchType = (typeId) => {
  if (!editData.value) return
  const defaults = {
    LlmTask:         { prompt_template: '' },
    ToolTask:        { tool_name: '', arguments: {} },
    HumanCheckpoint: { prompt: '', options: ['continue', 'abort'] },
    AgentTask:       { agent_type: '', persona: '', task_description: '', sub_tasks: [], assigned_tools: [] },
  }
  editData.value.node_type = { type: typeId, config: { ...defaults[typeId] } }
  isDirty.value = true
  nextTick(() => autoResizeTextarea())
}

// ─── ToolTask 参数管理 ──────────────────────────────────────
const argEntries = computed(() => {
  if (currentType.value !== 'ToolTask' || !editData.value?.node_type?.config?.arguments) return []
  return Object.entries(editData.value.node_type.config.arguments)
})

const addArg = () => {
  if (!editData.value?.node_type?.config) return
  if (!editData.value.node_type.config.arguments) editData.value.node_type.config.arguments = {}
  editData.value.node_type.config.arguments[`param_${Date.now()}`] = ''
  isDirty.value = true
}
const removeArg = (key) => {
  if (editData.value?.node_type?.config?.arguments) {
    delete editData.value.node_type.config.arguments[key]
    isDirty.value = true
  }
}
const updateArgKey = (oldKey, newKey) => {
  if (!editData.value?.node_type?.config?.arguments || oldKey === newKey) return
  const val = editData.value.node_type.config.arguments[oldKey]
  delete editData.value.node_type.config.arguments[oldKey]
  editData.value.node_type.config.arguments[newKey] = val
  isDirty.value = true
}

// ─── 痛点3: 文件输入声明 ─────────────────────────────────────
// 该节点需要用户上传的文件类型(none/word/excel/pdf),
// 选择后:① 在 metadata 标记 file_input ② 建对应 arguments 参数 ③ 供执行前引导上传面板识别
const FILE_INPUT_OPTS = [
  { key: 'none',  label: '不需要',      argKey: null,        accept: [] },
  { key: 'word',  label: 'Word 文档',  argKey: 'word_path',  accept: ['docx', 'doc'] },
  { key: 'excel', label: 'Excel 文件', argKey: 'excel_path', accept: ['xlsx', 'xls'] },
  { key: 'pdf',   label: 'PDF 文件',   argKey: 'pdf_path',   accept: ['pdf'] },
]
const fileInputType = computed({
  get: () => editData.value?.metadata?.file_input || 'none',
  set: (key) => {
    if (!editData.value) return
    const opt = FILE_INPUT_OPTS.find(o => o.key === key) || FILE_INPUT_OPTS[0]
    editData.value.metadata = editData.value.metadata || {}
    // 先清掉旧的文件参数(切换类型时)
    const old = FILE_INPUT_OPTS.find(o => o.key === editData.value.metadata.file_input)
    const cfg = editData.value.node_type?.config
    if (old?.argKey && cfg?.arguments && cfg.arguments[old.argKey] !== undefined && old.argKey !== opt.argKey) {
      delete cfg.arguments[old.argKey]
    }
    editData.value.metadata.file_input = key
    if (opt.argKey) {
      // 确保有 arguments 容器并建好参数(值留空,执行时由上传文件填入)
      if (editData.value.node_type) {
        editData.value.node_type.config = editData.value.node_type.config || {}
        editData.value.node_type.config.arguments = editData.value.node_type.config.arguments || {}
        if (editData.value.node_type.config.arguments[opt.argKey] === undefined) {
          editData.value.node_type.config.arguments[opt.argKey] = ''
        }
      }
    }
    isDirty.value = true
  }
})

// ─── HumanCheckpoint 选项管理 ───────────────────────────────
const optionsText = computed({
  get: () => (editData.value?.node_type?.config?.options || []).join(', '),
  set: (val) => {
    if (editData.value?.node_type?.config) {
      editData.value.node_type.config.options = val.split(',').map(s => s.trim()).filter(Boolean)
      isDirty.value = true
    }
  }
})

// ─── AgentTask 子任务管理 ────────────────────────────────────
const agentSubTasks = computed(() => editData.value?.node_type?.config?.sub_tasks || [])

const addSubTask = () => {
  if (!editData.value?.node_type?.config) return
  if (!editData.value.node_type.config.sub_tasks) editData.value.node_type.config.sub_tasks = []
  editData.value.node_type.config.sub_tasks.push({
    id: `sub_${Date.now()}`,
    name: '新子任务',
    type: 'LlmTask'
  })
  isDirty.value = true
}

const removeSubTask = (idx) => {
  editData.value.node_type.config.sub_tasks.splice(idx, 1)
  isDirty.value = true
}

// ─── 工作流绑定管理（Issue #32）─────────────────────────────
const executionMode = computed({
  get: () => editData.value?.metadata?.workflow_binding ? 'workflow' : 'tools',
  set: (val) => {
    if (!editData.value) return
    if (!editData.value.metadata) editData.value.metadata = {}
    if (val === 'workflow') {
      if (!editData.value.metadata.workflow_binding) {
        editData.value.metadata.workflow_binding = {
          engine: 'coze',
          api_base: '',
          api_key: '',
          streaming: false,
          timeout_seconds: 300,
          input_mapping: {},
        }
      }
    } else {
      delete editData.value.metadata.workflow_binding
    }
    isDirty.value = true
  }
})

const workflowBinding = computed(() => editData.value?.metadata?.workflow_binding || null)

// 工作流输入映射管理
const wfMappingEntries = computed(() => {
  if (!workflowBinding.value?.input_mapping) return []
  return Object.entries(workflowBinding.value.input_mapping)
})

const addWfMapping = () => {
  if (!workflowBinding.value) return
  if (!workflowBinding.value.input_mapping) workflowBinding.value.input_mapping = {}
  workflowBinding.value.input_mapping[`param_${Date.now()}`] = '{{task_description}}'
  isDirty.value = true
}

const removeWfMapping = (key) => {
  if (workflowBinding.value?.input_mapping) {
    delete workflowBinding.value.input_mapping[key]
    isDirty.value = true
  }
}

const updateWfMappingKey = (oldKey, newKey) => {
  if (!workflowBinding.value?.input_mapping || oldKey === newKey) return
  const val = workflowBinding.value.input_mapping[oldKey]
  delete workflowBinding.value.input_mapping[oldKey]
  workflowBinding.value.input_mapping[newKey] = val
  isDirty.value = true
}

// 获取工作流 Schema
const wfSchemaLoading = ref(false)
const wfSchemaResult = ref(null)

const fetchWorkflowSchema = async () => {
  if (!workflowBinding.value?.api_base || !workflowBinding.value?.api_key) return
  wfSchemaLoading.value = true
  try {
    const data = await workflowApi.schema(workflowBinding.value)
    wfSchemaResult.value = data
    // 自动填充 input_mapping
    if (data.input_schema?.properties) {
      const mapping = {}
      for (const [key, schema] of Object.entries(data.input_schema.properties)) {
        mapping[key] = '{{task_description}}'
      }
      workflowBinding.value.input_mapping = mapping
      isDirty.value = true
    }
  } catch (e) {
    console.error('[NodeEditor] Failed to fetch workflow schema:', e)
    wfSchemaResult.value = { error: e.message }
  } finally {
    wfSchemaLoading.value = false
  }
}

// 测试工作流执行
const wfTestLoading = ref(false)
const wfTestResult = ref(null)

const testWorkflow = async () => {
  if (!workflowBinding.value?.api_base || !workflowBinding.value?.api_key) return
  wfTestLoading.value = true
  wfTestResult.value = null
  try {
    const testInput = {}
    for (const [key, tmpl] of Object.entries(workflowBinding.value.input_mapping || {})) {
      testInput[key] = tmpl.includes('{{') ? '测试数据' : tmpl
    }
    const data = await workflowApi.test(workflowBinding.value, testInput)
    wfTestResult.value = data
  } catch (e) {
    console.error('[NodeEditor] Workflow test failed:', e)
    wfTestResult.value = { error: e.message }
  } finally {
    wfTestLoading.value = false
  }
}

// ─── 技能/工具管理（AgentTask 的 assigned_tools）─────────────
const availableTools = ref([])
const loadingTools = ref(false)
const toolsLoadError = ref(null)
// 痛点1: 浏览模式 — 'all' 直接列全部 skill/工具,'relevant' 按描述检索
const toolBrowseMode = ref('all')

// 根据任务描述动态检索相关工具
const loadRelevantTools = async (taskDescription) => {
  if (!taskDescription || taskDescription.trim().length < 2) {
    console.warn('[NodeEditor] Task description too short, skipping tool search')
    return []
  }

  try {
    const response = await fastPlanRequest(`${access ? '' : (props.adminBase || props.apiBase).replace(/\/$/, '')}/api/admin/advanced/vector-search`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        query: taskDescription,
        scope: 'tool',  // 只搜索工具 (MCP/SKILL/HTTP)
        top_k: 20      // 返回前 20 个最相关的结果
      })
    }, access)

    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`)
    }

    const data = await response.json()
    const tools = []

    // 解析向量搜索结果 — 统一为 entity_type="tool" + tool_type 子类型
    for (const result of data.results || []) {
      if (result.entity_type === 'tool') {
        const toolType = (result.tool_type || '').toUpperCase()

        if (toolType === 'MCP') {
          // MCP 工具: entity_id 格式为 "server_id:tool_name"
          const [serverId, ...rest] = result.entity_id.split(':')
          const toolName = rest.join(':')
          tools.push({
            id: result.entity_id,
            name: toolName || result.entity_id,
            description: extractDescription(result.text),
            type: 'mcp',
            serverId: serverId,
            serverName: 'MCP',
            displayName: result.entity_id,
            value: result.entity_id,
            score: result.score
          })
        } else if (toolType === 'HTTP') {
          // HTTP 工具
          tools.push({
            id: result.entity_id,
            name: extractName(result.text),
            description: extractDescription(result.text),
            type: 'http',
            displayName: extractName(result.text),
            value: result.entity_id,
            score: result.score
          })
        } else {
          // SKILL 或其他: value 用技能名(executor 按名查找), 不是 UUID
          const skillName = extractName(result.text)
          tools.push({
            id: result.entity_id,
            name: skillName,
            description: extractDescription(result.text),
            type: 'skill',
            displayName: skillName,
            value: skillName,
            score: result.score
          })
        }
      } else if (result.entity_type === 'agent') {
        // Agent 不作为可分配工具，跳过
        continue
      }
    }

    console.log(`✅ [NodeEditor] Found ${tools.length} relevant tools for: "${taskDescription.substring(0, 50)}..."`)
    return tools
  } catch (e) {
    console.error('❌ [NodeEditor] Failed to search tools:', e)
    throw e
  }
}

// 从向量搜索结果文本中提取名称
const extractName = (text) => {
  // 格式: "name: description. Category: ..."
  const match = text.match(/^([^:]+):/)
  return match ? match[1].trim() : text.split('.')[0].trim()
}

// 从向量搜索结果文本中提取描述
const extractDescription = (text) => {
  // 提取第一个句号前的内容作为描述
  const match = text.match(/^[^:]+:\s*([^.]+)/)
  return match ? match[1].trim() : text.substring(0, 100)
}

// 从 MCP 工具文本中提取服务器名称
const extractServerName = (text) => {
  // 尝试从 entity_id 中提取
  return 'MCP'
}

const loadAvailableTools = async () => {
  loadingTools.value = true
  toolsLoadError.value = null

  try {
    // 获取当前任务描述
    const taskDescription = editData.value?.node_type?.config?.task_description ||
                           editData.value?.node_type?.config?.description ||
                           editData.value?.name ||
                           ''

    if (!taskDescription) {
      console.warn('[NodeEditor] No task description available, loading all tools')
    }

    // 痛点1: 'all' 模式直接列全部 skill/工具(不依赖任务描述)
    if (toolBrowseMode.value === 'all') {
      // 跳到下方"加载所有工具"逻辑
    } else {
      // 'relevant' 模式: 有任务描述则向量检索
      if (taskDescription && taskDescription.length > 5) {
        const relevantTools = await loadRelevantTools(taskDescription)
        availableTools.value = relevantTools
        return
      }
    }

    // 方案2: 没有任务描述时，从 API 加载所有工具
    const [skillsRes, mcpRes] = await Promise.all([
      skillsApi.list({ enabled: true }),
      mcpApi.list()
    ])

    const tools = []

    // Skills
    if (skillsRes.skills) {
      skillsRes.skills.forEach(skill => {
        tools.push({
          id: skill.id,
          name: skill.name,
          description: skill.description,
          type: 'skill',
          displayName: skill.name,
          value: skill.name
        })
      })
    }

    // MCP Tools
    if (mcpRes.servers) {
      mcpRes.servers.forEach(server => {
        if (server._tools && server.status === 'connected') {
          server._tools.forEach(tool => {
            tools.push({
              id: `${server.id}:${tool.name}`,
              name: tool.name,
              description: tool.description || '',
              type: 'mcp',
              serverId: server.id,
              serverName: server.name,
              displayName: `${server.name}:${tool.name}`,
              value: `${server.id}:${tool.name}`
            })
          })
        }
      })
    }

    availableTools.value = tools
    console.log(`✅ [NodeEditor] Loaded ${tools.length} tools from API`)
  } catch (e) {
    console.error('❌ [NodeEditor] Failed to load tools:', e)
    toolsLoadError.value = e.message
    availableTools.value = []
  } finally {
    loadingTools.value = false
  }
}

// 痛点1: 切换浏览模式并重新加载
const switchToolBrowseMode = (mode) => {
  if (toolBrowseMode.value === mode) return
  toolBrowseMode.value = mode
  loadAvailableTools()
}

// 监听任务描述变化，重新检索工具(仅"按描述推荐"模式)
watch(() => editData.value?.node_type?.config?.task_description, (newDesc, oldDesc) => {
  if (toolBrowseMode.value === 'relevant' && newDesc && newDesc !== oldDesc && newDesc.length > 5) {
    console.log('[NodeEditor] Task description changed, reloading tools...')
    loadAvailableTools()
  }
}, { deep: true })

// 当前已分配的工具
const assignedTools = computed({
  get: () => {
    if (currentType.value !== 'AgentTask') return []
    return editData.value?.node_type?.config?.assigned_tools || []
  },
  set: (val) => {
    if (currentType.value === 'AgentTask' && editData.value?.node_type?.config) {
      editData.value.node_type.config.assigned_tools = val
      isDirty.value = true
    }
  }
})

const addTool = (toolValue) => {
  if (!assignedTools.value.includes(toolValue)) {
    assignedTools.value = [...assignedTools.value, toolValue]
  }
}

const removeTool = (toolValue) => {
  assignedTools.value = assignedTools.value.filter(t => t !== toolValue)
}

// 工具搜索
const toolSearchQuery = ref('')
const filteredTools = computed(() => {
  let tools = availableTools.value
  // 过滤低相关度工具（< 30%）
  tools = tools.filter(t => !t.score || t.score >= 0.3)
  if (toolSearchQuery.value) {
    const q = toolSearchQuery.value.toLowerCase()
    tools = tools.filter(t =>
      t.name.toLowerCase().includes(q) ||
      t.description.toLowerCase().includes(q) ||
      (t.serverName && t.serverName.toLowerCase().includes(q))
    )
  }
  // 按类型分组排序：skill 优先，mcp 其次
  tools.sort((a, b) => {
    if (a.type === b.type) return (b.score || 0) - (a.score || 0)
    if (a.type === 'skill') return -1
    if (b.type === 'skill') return 1
    return 0
  })
  return tools
})

// 获取工具显示信息
const getToolDisplay = (toolValue) => {
  const tool = availableTools.value.find(t => t.value === toolValue)
  if (tool) return tool
  // 简化 MCP 工具名称：去掉 UUID 前缀
  let displayName = toolValue
  if (toolValue.includes(':')) {
    const parts = toolValue.split(':')
    displayName = parts[parts.length - 1]
  }
  return {
    displayName,
    fullId: toolValue,
    type: toolValue.includes(':') ? 'mcp' : 'skill',
    description: ''
  }
}

// 工具类型 badge 样式
const toolBadgeClass = (type) => {
  if (type === 'mcp') return 'bg-purple-100 text-purple-700'
  if (type === 'http') return 'bg-orange-100 text-orange-700'
  return 'bg-blue-100 text-blue-700'
}

onMounted(() => {
  loadAvailableTools()
})

// ─── 显式依赖管理（PRD §4 可视化选择器）────────────────────────
const availableNodes = computed(() => {
  // 从 allNodes 中过滤出可选节点（排除当前节点和会导致循环的节点）
  return props.allNodes.filter(n => n.id !== editData.value?.id)
})

const filteredAvailableNodes = computed(() => {
  if (!dependencySearchQuery.value) return availableNodes.value
  const q = dependencySearchQuery.value.toLowerCase()
  return availableNodes.value.filter(n =>
    n.name.toLowerCase().includes(q) || n.id.toLowerCase().includes(q)
  )
})

const addDependency = (nodeId) => {
  if (!editData.value) return
  if (!editData.value.dependencies) editData.value.dependencies = []
  if (!editData.value.dependencies.includes(nodeId)) {
    editData.value.dependencies.push(nodeId)
    isDirty.value = true
  }
}

const removeDependency = (nodeId) => {
  if (!editData.value?.dependencies) return
  editData.value.dependencies = editData.value.dependencies.filter(id => id !== nodeId)
  isDirty.value = true
}

const getNodeName = (nodeId) => {
  const node = props.allNodes.find(n => n.id === nodeId)
  return node?.name || nodeId
}

const getNodeStepNumber = (nodeId) => {
  const idx = props.allNodes.findIndex(n => n.id === nodeId)
  return idx >= 0 ? `${idx + 1}` : '?'
}

// 循环依赖检测（PRD §4.2.3）
const wouldCreateCycle = (newDependencyId) => {
  if (!editData.value) return false
  const taskId = editData.value.id

  // 从 newDependencyId 出发，DFS 搜索其所有下游节点
  const visited = new Set()
  const queue = [taskId]

  while (queue.length > 0) {
    const current = queue.shift()
    if (current === newDependencyId) return true // 循环！

    // 找到所有依赖 current 的节点
    const downstream = props.allNodes
      .filter(t => (t.dependencies || []).includes(current))
      .map(t => t.id)

    for (const d of downstream) {
      if (!visited.has(d)) {
        visited.add(d)
        queue.push(d)
      }
    }
  }

  return false
}

// ─── 文本域自动伸缩 ─────────────────────────────────────────
const autoResizeTextarea = () => {
  const el = promptTextareaRef.value
  if (!el) return
  el.style.height = 'auto'
  el.style.height = Math.max(el.scrollHeight, 200) + 'px'
}

const handleSave = () => {
  if (!editData.value) return
  const savedData = JSON.parse(JSON.stringify(editData.value))
  savedData.isEdited = true  // 标记为已修改
  emit('save', savedData)
  isDirty.value = false
}

const handleSaveAndContinue = () => {
  if (!editData.value) return
  const savedData = JSON.parse(JSON.stringify(editData.value))
  savedData.isEdited = true  // 标记为已修改
  emit('save-and-continue', savedData)
  isDirty.value = false
}

const handleClose = () => { emit('close') }

// Ctrl+S / Esc 快捷键 — 仅在编辑器可见时生效
const handleKeydown = (e) => {
  if (!props.visible) return
  if ((e.ctrlKey || e.metaKey) && e.key === 's') { e.preventDefault(); handleSave() }
  if (e.key === 'Escape') handleClose()
}
onMounted(() => window.addEventListener('keydown', handleKeydown))
onUnmounted(() => window.removeEventListener('keydown', handleKeydown))

const typeColorMap = {
  teal:   { bg: 'bg-blue-50',   text: 'text-blue-700',   border: 'border-blue-200',   active: 'bg-blue-600 text-white border-blue-600'   },
  amber:  { bg: 'bg-amber-50',  text: 'text-amber-700',  border: 'border-amber-200',  active: 'bg-amber-500 text-white border-amber-500'  },
  blue:   { bg: 'bg-blue-50',   text: 'text-blue-700',   border: 'border-blue-200',   active: 'bg-blue-600 text-white border-blue-600'    },
  violet: { bg: 'bg-violet-50', text: 'text-violet-700', border: 'border-violet-200', active: 'bg-violet-600 text-white border-violet-600' },
}
</script>

<template>
  <teleport to="body">
    <div class="fpp-ui">
    <!-- 背景遮罩 -->
    <transition enter-active-class="transition-opacity duration-200" leave-active-class="transition-opacity duration-150" enter-from-class="opacity-0" leave-to-class="opacity-0">
      <div v-if="visible && editData" class="fixed inset-0 z-[99] bg-black/40 backdrop-blur-[2px]" @click="handleClose" />
    </transition>

    <!-- 右侧滑出面板 -->
    <transition enter-active-class="transition-transform duration-250 ease-out" leave-active-class="transition-transform duration-200 ease-in" enter-from-class="translate-x-full" leave-to-class="translate-x-full">
      <div v-if="visible && editData" class="fixed top-0 right-0 h-full z-[100] w-full max-w-[600px] bg-white shadow-2xl flex flex-col" @click.stop>

        <!-- 顶部 Header -->
        <div class="flex-shrink-0 border-b border-gray-100">
          <!-- 节点类型色条 -->
          <div class="h-1 w-full" :class="{
            'bg-gradient-to-r from-blue-400 to-blue-600':     currentType === 'LlmTask',
            'bg-gradient-to-r from-amber-400 to-amber-600':   currentType === 'ToolTask',
            'bg-gradient-to-r from-blue-400 to-blue-600':     currentType === 'HumanCheckpoint',
            'bg-gradient-to-r from-violet-400 to-violet-600': currentType === 'AgentTask',
          }" />
          <div class="px-6 py-4 flex items-start justify-between gap-4">
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2 mb-1">
                <div class="p-1.5 rounded-lg" :class="typeColorMap[currentTypeDef?.color]?.bg">
                  <ChatModelIcon v-if="currentType === 'LlmTask'" class="fpp-model-icon" :name="currentModel.name" :model-id="currentModel.id" :size="16" />
                  <component v-else :is="currentTypeDef?.icon" :size="16" :class="typeColorMap[currentTypeDef?.color]?.text" />
                </div>
                <span class="text-xs font-semibold tracking-wide uppercase" :class="typeColorMap[currentTypeDef?.color]?.text">
                  {{ currentTypeDef?.label }}
                </span>
                <span v-if="isDirty" class="text-[10px] px-1.5 py-0.5 bg-amber-100 text-amber-600 rounded-full font-medium">未保存</span>
              </div>
              <input v-model="editData.name" @input="isDirty = true"
                class="w-full text-lg font-bold text-gray-900 bg-transparent border-b-2 border-transparent hover:border-gray-200 focus:border-blue-400 outline-none transition-colors py-0.5"
                placeholder="节点名称" />
            </div>
            <button @click="handleClose" class="flex-shrink-0 p-2 hover:bg-gray-100 rounded-xl transition-colors text-gray-400 hover:text-gray-600 mt-1" title="关闭 (Esc)">
              <X :size="18" />
            </button>
          </div>

          <!-- 节点类型切换 -->
          <div class="px-6 pb-4 flex gap-2 flex-wrap">
            <button v-for="nt in nodeTypes" :key="nt.id" @click="switchType(nt.id)"
              class="flex items-center gap-1.5 px-3 py-2 rounded-lg border text-xs font-medium transition-all"
              :class="currentType === nt.id ? typeColorMap[nt.color]?.active + ' shadow-sm' : 'border-gray-200 text-gray-500 hover:border-gray-300 hover:bg-gray-50'"
              :title="nt.desc">
              <component :is="nt.icon" :size="13" />
              {{ nt.label }}
            </button>
          </div>
        </div>

        <!-- 内容区域 -->
        <div class="flex-1 overflow-y-auto px-6 py-5 space-y-6">

          <!-- 痛点3: 文件输入声明(对所有节点类型显示) -->
          <div class="bg-blue-50/40 border border-blue-100 rounded-xl p-4">
            <label class="text-xs font-semibold text-gray-600 uppercase tracking-wider block mb-1">
              此节点需要用户上传的文件吗?
            </label>
            <p class="text-[11px] text-gray-400 mb-2.5">
              选择后,执行前会自动提示用户上传对应文件,并把文件路径注入本节点参数。
            </p>
            <div class="flex gap-2 flex-wrap">
              <button v-for="opt in FILE_INPUT_OPTS" :key="opt.key"
                @click="fileInputType = opt.key"
                class="px-3 py-1.5 rounded-lg border text-xs font-medium transition-all"
                :class="fileInputType === opt.key ? 'bg-blue-600 text-white border-blue-600 shadow-sm' : 'bg-white border-gray-200 text-gray-600 hover:border-blue-300'">
                {{ opt.label }}
              </button>
            </div>
            <p v-if="fileInputType !== 'none'" class="text-[11px] text-blue-600 mt-2 flex items-center gap-1">
              已声明:执行前用户需上传{{ FILE_INPUT_OPTS.find(o => o.key === fileInputType)?.label }},
              将注入参数 <code class="font-mono bg-blue-100 px-1 rounded">{{ FILE_INPUT_OPTS.find(o => o.key === fileInputType)?.argKey }}</code>
            </p>
          </div>

          <!-- LlmTask -->
          <div v-if="currentType === 'LlmTask'">
            <div class="flex items-center justify-between mb-2">
              <label class="text-xs font-semibold text-gray-500 uppercase tracking-wider">Prompt 模板</label>
              <span class="text-[11px] text-gray-400 font-mono">{{ promptCharCount }} 字符</span>
            </div>
            <textarea ref="promptTextareaRef" v-model="promptValue" @input="autoResizeTextarea(); isDirty = true"
              class="w-full px-4 py-3 border border-gray-200 rounded-xl text-sm text-gray-800 leading-relaxed focus:ring-2 focus:ring-blue-500/20 focus:border-blue-400 outline-none resize-none transition-all bg-gray-50 focus:bg-white placeholder-gray-400 font-mono"
              style="min-height: 200px"
              placeholder="请输入 AI 任务指令...&#10;&#10;支持 {变量} 引用上下文输出，例如：&#10;根据以下信息：{previous_output}&#10;请完成..." />
            <p class="mt-2 text-xs text-gray-400">提示：使用 <code class="bg-gray-100 px-1 rounded">{变量名}</code> 引用前一节点的输出</p>
          </div>

          <!-- HumanCheckpoint -->
          <div v-if="currentType === 'HumanCheckpoint'" class="space-y-5">
            <div>
              <div class="flex items-center justify-between mb-2">
                <label class="text-xs font-semibold text-gray-500 uppercase tracking-wider">确认提示语</label>
                <span class="text-[11px] text-gray-400 font-mono">{{ promptCharCount }} 字符</span>
              </div>
              <textarea ref="promptTextareaRef" v-model="promptValue" @input="autoResizeTextarea(); isDirty = true"
                class="w-full px-4 py-3 border border-gray-200 rounded-xl text-sm text-gray-800 leading-relaxed focus:ring-2 focus:ring-blue-500/20 focus:border-blue-400 outline-none resize-none transition-all bg-gray-50 focus:bg-white placeholder-gray-400"
                style="min-height: 100px" placeholder="请输入需要人工确认的提示信息..." />
            </div>
            <div>
              <label class="text-xs font-semibold text-gray-500 uppercase tracking-wider block mb-2">选项（逗号分隔）</label>
              <input v-model="optionsText"
                class="w-full px-4 py-2.5 border border-gray-200 rounded-xl text-sm focus:ring-2 focus:ring-blue-500/20 focus:border-blue-400 outline-none transition-all bg-gray-50 focus:bg-white"
                placeholder="continue, abort, modify" />
              <div class="flex flex-wrap gap-1.5 mt-2">
                <span v-for="opt in (editData.node_type?.config?.options || [])" :key="opt"
                  class="px-2 py-0.5 bg-blue-50 text-blue-600 text-xs rounded-full border border-blue-100 font-medium">
                  {{ opt }}
                </span>
              </div>
            </div>
          </div>

          <!-- ToolTask -->
          <div v-if="currentType === 'ToolTask'" class="space-y-5">
            <div>
              <label class="text-xs font-semibold text-gray-500 uppercase tracking-wider block mb-2">工具名称</label>
              <input v-model="editData.node_type.config.tool_name" @input="isDirty = true"
                class="w-full px-4 py-2.5 border border-gray-200 rounded-xl text-sm font-mono focus:ring-2 focus:ring-amber-500/20 focus:border-amber-400 outline-none transition-all bg-gray-50 focus:bg-white"
                placeholder="e.g. web_search, code_executor, file_reader" />
            </div>
            <div>
              <div class="flex items-center justify-between mb-2">
                <label class="text-xs font-semibold text-gray-500 uppercase tracking-wider">参数</label>
                <button @click="addArg" class="text-xs text-amber-600 hover:text-amber-700 flex items-center gap-1 px-2 py-1 hover:bg-amber-50 rounded-lg transition-colors">
                  <Plus :size="12" /> 添加参数
                </button>
              </div>
              <div class="space-y-2">
                <div v-for="[key, val] in argEntries" :key="key" class="flex items-center gap-2 bg-gray-50 rounded-lg p-2 border border-gray-100">
                  <input :value="key" @change="updateArgKey(key, $event.target.value)"
                    class="w-32 flex-shrink-0 px-2.5 py-1.5 border border-gray-200 rounded-lg text-xs font-mono focus:border-amber-400 outline-none bg-white" placeholder="参数名" />
                  <span class="text-gray-300 text-sm flex-shrink-0">=</span>
                  <input :value="val" @input="editData.node_type.config.arguments[key] = $event.target.value; isDirty = true"
                    class="flex-1 px-2.5 py-1.5 border border-gray-200 rounded-lg text-xs focus:border-amber-400 outline-none bg-white" placeholder="参数值" />
                  <button @click="removeArg(key)" class="flex-shrink-0 p-1.5 text-gray-300 hover:text-red-500 hover:bg-red-50 rounded-lg transition-colors">
                    <Trash2 :size="13" />
                  </button>
                </div>
                <div v-if="argEntries.length === 0" class="text-center py-6 text-xs text-gray-400 border-2 border-dashed border-gray-200 rounded-xl">
                  暂无参数，点击"添加参数"
                </div>
              </div>
            </div>
          </div>

          <!-- AgentTask（PRD §2）-->
          <div v-if="currentType === 'AgentTask'" class="space-y-5">
            <div>
              <label class="text-xs font-semibold text-gray-500 uppercase tracking-wider block mb-2">Agent 类型</label>
              <input v-model="editData.node_type.config.agent_type" @input="isDirty = true"
                class="w-full px-4 py-2.5 border border-gray-200 rounded-xl text-sm font-mono focus:ring-2 focus:ring-violet-500/20 focus:border-violet-400 outline-none transition-all bg-gray-50 focus:bg-white"
                placeholder="e.g. research_agent, code_agent, market_agent" />
            </div>
            <div>
              <label class="text-xs font-semibold text-gray-500 uppercase tracking-wider block mb-2">Persona / 角色设定</label>
              <textarea v-model="editData.node_type.config.persona" @input="isDirty = true" rows="3"
                class="w-full px-4 py-3 border border-gray-200 rounded-xl text-sm text-gray-800 focus:ring-2 focus:ring-violet-500/20 focus:border-violet-400 outline-none resize-none transition-all bg-gray-50 focus:bg-white"
                placeholder="描述该 Agent 的角色设定..." />
            </div>
            <div>
              <label class="text-xs font-semibold text-gray-500 uppercase tracking-wider block mb-2">任务描述 / 目标</label>
              <textarea v-model="editData.node_type.config.task_description" @input="isDirty = true" rows="3"
                class="w-full px-4 py-3 border border-gray-200 rounded-xl text-sm text-gray-800 focus:ring-2 focus:ring-violet-500/20 focus:border-violet-400 outline-none resize-none transition-all bg-gray-50 focus:bg-white"
                placeholder="描述该 Agent 需要完成的目标..." />
            </div>

            <!-- ★ Issue #32: 执行方式选择 -->
            <div class="border-t border-gray-100 pt-5">
              <label class="text-xs font-semibold text-gray-500 uppercase tracking-wider block mb-2">执行方式</label>
              <div class="flex gap-2">
                <button @click="executionMode = 'tools'" type="button"
                  class="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl border text-sm font-medium transition-all"
                  :class="executionMode === 'tools' ? 'bg-violet-600 text-white border-violet-600 shadow-sm' : 'border-gray-200 text-gray-600 hover:border-gray-300 hover:bg-gray-50'">
                  <Wrench :size="14" />
                  工具调用 (OODA)
                </button>
                <button @click="executionMode = 'workflow'" type="button"
                  class="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl border text-sm font-medium transition-all"
                  :class="executionMode === 'workflow' ? 'bg-violet-600 text-white border-violet-600 shadow-sm' : 'border-gray-200 text-gray-600 hover:border-gray-300 hover:bg-gray-50'">
                  <Link :size="14" />
                  外部工作流
                </button>
              </div>
            </div>

            <!-- ★ Issue 流配置表单 -->
            <div v-if="executionMode === 'workflow' && workflowBinding" class="space-y-4 border border-violet-100 rounded-xl p-4 bg-violet-50/30">
              <div class="flex items-center gap-2 text-xs text-violet-700 font-medium">
                <Link :size="14" />
                外部工作流配置
              </div>

              <div>
                <label class="text-xs font-semibold text-gray-500 uppercase tracking-wider block mb-2">工作流引擎</label>
                <select v-model="workflowBinding.engine" @change="isDirty = true"
                  class="w-full px-4 py-2.5 border border-gray-200 rounded-xl text-sm focus:ring-2 focus:ring-violet-500/20 focus:border-violet-400 outline-none transition-all bg-white">
                  <option value="coze">Coze</option>
                  <option value="dify">Dify</option>
                  <option value="custom">自定义</option>
                </select>
              </div>

              <div>
                <label class="text-xs font-semibold text-gray-500 uppercase tracking-wider block mb-2">API 地址</label>
                <input v-model="workflowBinding.api_base" @input="isDirty = true"
                  class="w-full px-4 py-2.5 border border-gray-200 rounded-xl text-sm font-mono focus:ring-2 focus:ring-violet-500/20 focus:border-violet-400 outline-none transition-all bg-white"
                  placeholder="https://api.coze.com/v1/workflow/run" />
              </div>

              <div>
                <label class="text-xs font-semibold text-gray-500 uppercase tracking-wider block mb-2">API Token</label>
                <input v-model="workflowBinding.api_key" @input="isDirty = true" type="password"
                  class="w-full px-4 py-2.5 border border-gray-200 rounded-xl text-sm font-mono focus:ring-2 focus:ring-violet-500/20 focus:border-violet-400 outline-none transition-all bg-white"
                  placeholder="pat_xxx..." />
              </div>

              <div class="flex items-center gap-4">
                <label class="flex items-center gap-2 cursor-pointer">
                  <input v-model="workflowBinding.streaming" @change="isDirty = true" type="checkbox"
                    class="w-4 h-4 text-violet-600 border-gray-300 rounded focus:ring-violet-500" />
                  <span class="text-xs text-gray-700">启用流式追踪</span>
                </label>
                <div class="flex items-center gap-2">
                  <label class="text-xs text-gray-500">超时（秒）</label>
                  <input v-model.number="workflowBinding.timeout_seconds" @input="isDirty = true" type="number"
                    class="w-20 px-2 py-1 border border-gray-200 rounded-lg text-xs focus:border-violet-400 outline-none bg-white" />
                </div>
              </div>

              <div>
                <div class="flex items-center justify-between mb-2">
                  <label class="text-xs font-semibold text-gray-500 uppercase tracking-wider">输入参数映射</label>
                  <div class="flex items-center gap-2">
                    <button @click="fetchWorkflowSchema" :disabled="wfSchemaLoading || !workflowBinding.api_base || !workflowBinding.api_key" type="button"
                      class="text-xs text-violet-600 hover:text-violet-700 flex items-center gap-1 px-2 py-1 hover:bg-violet-50 rounded-lg transition-colors disabled:opacity-50">
                      <Loader2 v-if="wfSchemaLoading" :size="12" class="animate-spin" />
                      <Search v-else :size="12" />
                      获取参数 Schema
                    </button>
                    <button @click="addWfMapping" type="button"
                      class="text-xs text-violet-600 hover:text-violet-700 flex items-center gap-1 px-2 py-1 hover:bg-violet-50 rounded-lg transition-colors">
                      <Plus :size="12" /> 添加参数
                    </button>
                  </div>
                </div>
                <div class="space-y-2">
                  <div v-for="[key, val] in wfMappingEntries" :key="key" class="flex items-center gap-2 bg-white rounded-lg p-2 border border-gray-200">
                    <input :value="key" @change="updateWfMappingKey(key, $event.target.value)"
                      class="w-32 flex-shrink-0 px-2.5 py-1.5 border border-gray-200 rounded-lg text-xs font-mono focus:border-violet-400 outline-none" placeholder="参数名" />
                    <span class="text-gray-300 text-sm flex-shrink-0">=</span>
                    <input :value="val" @input="workflowBinding.input_mapping[key] = $event.target.value; isDirty = true"
                      class="flex-1 px-2.5 py-1.5 border border-gray-200 rounded-lg text-xs font-mono focus:border-violet-400 outline-none" :placeholder="'{{task_description}}'" />
                    <button @click="removeWfMapping(key)" type="button" class="flex-shrink-0 p-1.5 text-gray-300 hover:text-red-500 hover:bg-red-50 rounded-lg transition-colors">
                      <Trash2 :size="13" />
                    </button>
                  </div>
                  <div v-if="wfMappingEntries.length === 0" class="text-center py-4 text-xs text-gray-400 border-2 border-dashed border-gray-200 rounded-xl">
                    暂无参数映射，点击"添加参数"或"获取参数 Schema"
                  </div>
                </div>
                <p class="mt-2 text-xs text-gray-400">
                  💡 支持模板: <code class="bg-gray-100 px-1 rounded" v-text="'{{task_description}}'"></code>, <code class="bg-gray-100 px-1 rounded" v-text="'{{extract:字段}}'"></code>, <code class="bg-gray-100 px-1 rounded" v-text="'{{default:值}}'"></code>
                </p>
              </div>

              <div class="flex items-center gap-2">
                <button @click="testWorkflow" :disabled="wfTestLoading || !workflowBinding.api_base || !workflowBinding.api_key" type="button"
                  class="flex items-center gap-2 px-4 py-2 bg-violet-600 text-white text-xs font-medium rounded-lg hover:bg-violet-700 transition-colors disabled:opacity-50">
                  <Loader2 v-if="wfTestLoading" :size="14" class="animate-spin" />
                  <Play v-else :size="14" />
                  测试执行
                </button>
                <span v-if="wfTestResult" class="text-xs"
                  :class="wfTestResult.error ? 'text-red-600' : 'text-green-600'">
                  {{ wfTestResult.error ? `❌ ${wfTestResult.error}` : '✅ 测试成功' }}
                </span>
              </div>
            </div>
            <!-- 子任务列表（PRD §3 嵌套展示）-->
            <div>
              <div class="flex items-center justify-between mb-2">
                <label class="text-xs font-semibold text-gray-500 uppercase tracking-wider">子任务流程（OODA 步骤）</label>
                <button @click="addSubTask" class="text-xs text-violet-600 hover:text-violet-700 flex items-center gap-1 px-2 py-1 hover:bg-violet-50 rounded-lg transition-colors">
                  <Plus :size="12" /> 添加子任务
                </button>
              </div>
              <div class="space-y-2">
                <div v-for="(sub, idx) in agentSubTasks" :key="sub.id"
                  class="flex items-center gap-2 bg-violet-50/60 rounded-lg p-2 border border-violet-100">
                  <div class="w-5 h-5 rounded-full bg-violet-200 text-violet-700 flex items-center justify-center text-[10px] font-bold flex-shrink-0">{{ idx + 1 }}</div>
                  <input v-model="sub.name" @input="isDirty = true"
                    class="flex-1 px-2 py-1 border border-violet-200 rounded-lg text-xs focus:border-violet-400 outline-none bg-white"
                    placeholder="子任务名称" />
                  <select v-model="sub.type" @change="isDirty = true"
                    class="px-2 py-1 border border-violet-200 rounded-lg text-xs focus:border-violet-400 outline-none bg-white text-violet-700">
                    <option value="LlmTask">AI 推理</option>
                    <option value="ToolTask">工具调用</option>
                    <option value="HumanCheckpoint">人工确认</option>
                  </select>
                  <button @click="removeSubTask(idx)" class="flex-shrink-0 p-1 text-gray-300 hover:text-red-500 hover:bg-red-50 rounded transition-colors">
                    <Trash2 :size="12" />
                  </button>
                </div>
                <div v-if="agentSubTasks.length === 0" class="text-center py-5 text-xs text-gray-400 border-2 border-dashed border-violet-100 rounded-xl">
                  暂无子任务 — 子任务由 OODA 循环自动生成，也可手动添加
                </div>
              </div>
            </div>

            <!-- 分配的工具/技能 -->
            <div>
              <div class="flex items-center justify-between mb-2">
                <label class="text-xs font-semibold text-gray-500 uppercase tracking-wider">分配的工具/技能</label>
                <div class="flex items-center gap-2">
                  <span class="text-[10px] text-gray-400">{{ assignedTools.length }} 个已分配</span>
                  <button @click="loadAvailableTools" :disabled="loadingTools"
                    class="text-xs text-violet-600 hover:text-violet-700 flex items-center gap-1 px-2 py-1 hover:bg-violet-50 rounded-lg transition-colors disabled:opacity-50"
                    title="根据任务描述重新检索相关工具">
                    <svg v-if="loadingTools" class="animate-spin h-3 w-3" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                      <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                      <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                    </svg>
                    <Search v-else :size="12" />
                    {{ loadingTools ? '检索中...' : '刷新' }}
                  </button>
                </div>
              </div>

              <!-- 已分配的工具列表 -->
              <div v-if="assignedTools.length > 0" class="space-y-1.5 mb-3">
                <div v-for="tool in assignedTools" :key="tool"
                  class="flex items-center justify-between gap-2 bg-violet-50 rounded-lg p-2 border border-violet-200">
                  <div class="flex items-center gap-2 flex-1 min-w-0">
                    <span class="text-[10px] font-bold px-1.5 py-0.5 rounded uppercase"
                      :class="toolBadgeClass(getToolDisplay(tool).type)">
                      {{ getToolDisplay(tool).type }}
                    </span>
                    <span class="text-xs font-medium text-gray-800 truncate">{{ getToolDisplay(tool).displayName }}</span>
                  </div>
                  <button @click="removeTool(tool)" class="flex-shrink-0 p-1 text-gray-400 hover:text-red-500 hover:bg-red-50 rounded transition-colors">
                    <X :size="14" />
                  </button>
                </div>
              </div>
              <div v-else class="text-center py-4 text-xs text-amber-600 bg-amber-50 border border-amber-200 rounded-xl mb-3">
                ⚠️ 未分配任何工具，执行时可能失败
              </div>

              <!-- 工具选择器 -->
              <div class="border border-gray-200 rounded-xl overflow-hidden">
                <div class="bg-gray-50 px-3 py-2 border-b border-gray-200 space-y-2">
                  <!-- 痛点1: 全部 / 相关推荐 切换 -->
                  <div class="flex items-center gap-1 bg-gray-100 rounded-lg p-0.5 w-fit">
                    <button @click="switchToolBrowseMode('all')"
                      class="px-2.5 py-1 text-[11px] font-medium rounded-md transition-colors"
                      :class="toolBrowseMode === 'all' ? 'bg-white text-violet-700 shadow-sm' : 'text-gray-500 hover:text-gray-700'">
                      全部技能/工具
                    </button>
                    <button @click="switchToolBrowseMode('relevant')"
                      class="px-2.5 py-1 text-[11px] font-medium rounded-md transition-colors"
                      :class="toolBrowseMode === 'relevant' ? 'bg-white text-violet-700 shadow-sm' : 'text-gray-500 hover:text-gray-700'">
                      按描述推荐
                    </button>
                  </div>
                  <div class="relative">
                    <Search :size="14" class="absolute left-2.5 top-1/2 -translate-y-1/2 text-gray-400" />
                    <input v-model="toolSearchQuery"
                      class="w-full pl-8 pr-3 py-1.5 text-xs border border-gray-200 rounded-lg focus:border-violet-400 focus:ring-1 focus:ring-violet-400/20 outline-none bg-white"
                      placeholder="搜索技能或工具名称..." />
                  </div>
                </div>
                <div class="max-h-48 overflow-y-auto">
                  <div v-if="loadingTools" class="text-center py-6 text-xs text-gray-400">
                    <svg class="animate-spin h-5 w-5 mx-auto mb-2 text-violet-500" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                      <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                      <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                    </svg>
                    正在检索相关工具...
                  </div>
                  <div v-else-if="toolsLoadError" class="text-center py-6 px-4">
                    <p class="text-xs text-red-600 mb-2">❌ 加载失败: {{ toolsLoadError }}</p>
                    <button @click="loadAvailableTools" class="text-xs text-violet-600 hover:text-violet-700 underline">
                      重试
                    </button>
                  </div>
                  <div v-else-if="filteredTools.length === 0" class="text-center py-6 text-xs text-gray-400">
                    {{ toolSearchQuery ? '未找到匹配的技能/工具' : (toolBrowseMode === 'relevant' ? '在节点描述里写明任务后点"按描述推荐"' : '暂无可用技能/工具,请先在 Skills 管理中导入') }}
                  </div>
                  <button v-else v-for="tool in filteredTools" :key="tool.value"
                    @click="addTool(tool.value)"
                    :disabled="assignedTools.includes(tool.value)"
                    class="w-full text-left px-3 py-2 hover:bg-violet-50 transition-colors border-b border-gray-100 last:border-b-0 disabled:opacity-40 disabled:cursor-not-allowed">
                    <div class="flex items-start gap-2">
                      <span class="text-[9px] font-bold px-1.5 py-0.5 rounded uppercase flex-shrink-0 mt-0.5"
                        :class="toolBadgeClass(tool.type)">
                        {{ tool.type }}
                      </span>
                      <div class="flex-1 min-w-0">
                        <div class="text-xs font-medium text-gray-800 truncate" :title="tool.value">{{ tool.displayName }}</div>
                        <div class="text-[11px] text-gray-500 line-clamp-1 mt-0.5">{{ tool.description || '无描述' }}</div>
                        <div v-if="tool.score" class="text-[10px] mt-0.5"
                          :class="tool.score >= 0.6 ? 'text-green-500' : 'text-gray-400'">
                          {{ tool.score >= 0.6 ? '⭐ 推荐' : '' }} 相关度: {{ (tool.score * 100).toFixed(0) }}%
                        </div>
                      </div>
                    </div>
                  </button>
                </div>
              </div>
              <p class="mt-2 text-xs text-gray-400">
                💡 提示：修改任务描述后，点击"刷新"按钮重新检索相关工具
              </p>
            </div>
          </div>

          <!-- 显式依赖关系（PRD §4 可视化节点选择器）-->
          <div class="border-t border-gray-100 pt-5">
            <div class="flex items-center justify-between mb-2">
              <label class="text-xs font-semibold text-gray-500 uppercase tracking-wider">上游依赖节点</label>
              <button @click="showDependencySelector = !showDependencySelector"
                class="text-xs text-blue-600 hover:text-blue-700 flex items-center gap-1 px-2 py-1 hover:bg-blue-50 rounded-lg transition-colors">
                <Plus :size="12" /> 选择节点
              </button>
            </div>

            <!-- 已选依赖列表 -->
            <div v-if="editData.dependencies?.length" class="space-y-1.5 mb-3">
              <div v-for="dep in editData.dependencies" :key="dep"
                class="flex items-center justify-between gap-2 bg-gray-50 rounded-lg p-2 border border-gray-200">
                <div class="flex items-center gap-2 flex-1 min-w-0">
                  <span class="text-[10px] font-bold px-1.5 py-0.5 rounded bg-gray-200 text-gray-600">
                    {{ getNodeStepNumber(dep) }}
                  </span>
                  <span class="text-xs font-medium text-gray-800 truncate">{{ getNodeName(dep) }}</span>
                  <span class="text-[10px] text-gray-400 font-mono truncate">{{ dep }}</span>
                </div>
                <button @click="removeDependency(dep)"
                  class="flex-shrink-0 p-1 text-gray-400 hover:text-red-500 hover:bg-red-50 rounded transition-colors">
                  <X :size="14" />
                </button>
              </div>
            </div>
            <div v-else class="text-center py-4 text-xs text-gray-400 bg-gray-50 border border-gray-200 rounded-xl mb-3">
              无依赖 — 将按顺序执行
            </div>

            <!-- 节点选择器下拉面板 -->
            <div v-if="showDependencySelector" class="border border-gray-200 rounded-xl overflow-hidden mb-3">
              <div class="bg-gray-50 px-3 py-2 border-b border-gray-200">
                <div class="relative">
                  <Search :size="14" class="absolute left-2.5 top-1/2 -translate-y-1/2 text-gray-400" />
                  <input v-model="dependencySearchQuery"
                    class="w-full pl-8 pr-3 py-1.5 text-xs border border-gray-200 rounded-lg focus:border-blue-400 focus:ring-1 focus:ring-blue-400/20 outline-none bg-white"
                    placeholder="搜索节点..." />
                </div>
              </div>
              <div class="max-h-48 overflow-y-auto">
                <div v-if="availableNodes.length === 0" class="text-center py-6 text-xs text-gray-400">
                  {{ dependencySearchQuery ? '未找到匹配的节点' : '暂无可用节点' }}
                </div>
                <button v-else v-for="node in filteredAvailableNodes" :key="node.id"
                  @click="addDependency(node.id)"
                  :disabled="editData.dependencies?.includes(node.id) || node.id === editData.id || wouldCreateCycle(node.id)"
                  class="w-full text-left px-3 py-2 hover:bg-blue-50 transition-colors border-b border-gray-100 last:border-b-0 disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent">
                  <div class="flex items-start gap-2">
                    <span class="text-[9px] font-bold px-1.5 py-0.5 rounded bg-gray-200 text-gray-600 flex-shrink-0 mt-0.5">
                      {{ node.stepNumber }}
                    </span>
                    <div class="flex-1 min-w-0">
                      <div class="text-xs font-medium text-gray-800 truncate">{{ node.name }}</div>
                      <div class="text-[11px] text-gray-500 truncate mt-0.5">{{ node.id }}</div>
                      <div v-if="wouldCreateCycle(node.id)" class="text-[10px] text-red-500 mt-0.5">
                        ⚠️ 选择此节点会导致循环依赖
                      </div>
                    </div>
                    <Check v-if="editData.dependencies?.includes(node.id)" :size="14" class="text-blue-500 flex-shrink-0 mt-0.5" />
                  </div>
                </button>
              </div>
            </div>

            <p class="text-xs text-gray-400">
              💡 该节点将在所有上游节点完成后才开始执行
            </p>
          </div>

        </div>

        <!-- 底部操作栏 -->
        <div class="flex-shrink-0 border-t border-gray-100 px-6 py-4">
          <div class="flex items-center justify-between">
            <span class="text-xs text-gray-400">Ctrl+S 快速保存 · Esc 关闭</span>
            <div class="flex items-center gap-2">
              <button @click="handleClose" class="px-4 py-2 text-sm text-gray-600 hover:bg-gray-100 rounded-xl transition-colors">取消</button>
              <button @click="handleSave" class="px-4 py-2 text-sm border border-gray-200 text-gray-700 font-medium rounded-xl hover:bg-gray-50 transition-colors flex items-center gap-1.5">
                <Save :size="14" /> 保存
              </button>
              <button v-if="mode === 'execution'" @click="handleSaveAndContinue"
                class="px-4 py-2 text-sm bg-blue-600 text-white font-medium rounded-xl hover:bg-blue-700 transition-all shadow-sm hover:shadow-md active:scale-95 flex items-center gap-1.5">
                <Play :size="14" fill="currentColor" /> 保存并继续
              </button>
            </div>
          </div>
        </div>
      </div>
    </transition>
    </div>
  </teleport>
</template>
