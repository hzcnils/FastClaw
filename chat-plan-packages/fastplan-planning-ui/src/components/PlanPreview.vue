<script setup>
import { ref, computed, onMounted, watch, inject } from 'vue';
import { fastPlanRequest, FASTPLAN_ACCESS_KEY } from 'planflow-runtime';
const access = inject(FASTPLAN_ACCESS_KEY, undefined);
import { CheckCircle2, Circle, Brain, User, Wrench, Bot, GripVertical, ChevronDown, ChevronRight, X, Trash2, Plus, GitBranch, List, Edit3 } from 'lucide-vue-next';
import { ChatModelIcon } from 'chat-renderer';
import { ElMessageBox } from 'element-plus';
import GraphView from './GraphView.vue'
import PlanTaskRow from './PlanTaskRow.vue';
import NodeEditor from './NodeEditor.vue';
import TaskBlock from './blocks/TaskBlock.vue';
import ManualReviewPanel from './ManualReviewPanel.vue';
import { treeToDAG, deleteNodeWithDependencyFix, insertNodeAfter } from '../utils/treeToDAG';
import { groupConfirmedHistoryByNode, lastPlanTaskId, revealedPhaseRows } from '../utils/taskDisplay';
import { resolveNodeModel } from '../utils/nodeModel';

async function confirmDialog(message) {
  try {
    await ElMessageBox.confirm(message, '提示', {
      type: 'warning',
      confirmButtonText: '确定',
      cancelButtonText: '取消',
    });
    return true;
  } catch {
    return false;
  }
}

const props = defineProps({
  tree: { type: Object, required: true },
  apiBase: { type: String, default: '' },
  adminBase: { type: String, default: '' },
  executionMode: { type: String, default: 'manual' },
  // 'execute' = 规划后确认执行（默认）；'template' = 编辑模板，按钮为"保存模板"
  mode: { type: String, default: 'execute' },
  // ★ 原始用户需求：传给任务行，展示前替换 {{user_input}} 占位符
  userInput: { type: String, default: '' },
  // ★ 展示层：{ instant_header, thinking_process, is_complex, direct_result }
  presentation: { type: Object, default: null },
  // ★ 单组件状态机：'plan' 规划编辑 / 'executing' 执行中 / 'finished' 执行完成
  phase: { type: String, default: 'plan' },
  // ★ 执行态：taskId → { status, logs, ... }，按 taskId 增量 merge，key 不变展开态不丢
  executionState: { type: Object, default: () => ({}) },
  executionStatus: { type: String, default: null },
  confirmedHistory: { type: Array, default: () => [] },
  nowMs: { type: Number, default: () => Date.now() },
  theme: { type: String, default: 'light', validator: value => ['light', 'dark'].includes(value) },
  // ★ 底部操作条（原 CHOICE 确认区，移入规划卡底部，不夹在执行输出中间）
  //   状态机：idle 待选 / confirmed 已确认(隐藏) / cancelled 已取消 / editing 编辑中
  choiceState:  { type: String, default: 'idle' },
  choiceOptions: { type: Array, default: () => [] },
  // ★ 全局唯一 tab 的受控 activeView（父级 ChatView 统一持有）
  activeView:   { type: String, default: 'list' },
  density: { type: String, default: 'comfortable', validator: value => ['comfortable', 'compact'].includes(value) },
});
const emit = defineEmits(['confirm', 'cancel', 'save', 'choice-pick', 'replan', 'edit', 'node-select', 'update:activeView']);

const editableTree = ref(JSON.parse(JSON.stringify(props.tree)));
// ★ v21：清单视图默认；activeView 由父级 ChatView 控制，这里同步 prop → 内部 viewMode
//   （切换 tab 不重建任务列表、不丢节点展开态与滚动位置）
const viewMode = ref(props.activeView || 'list')
watch(() => props.activeView, (v) => { if (v) viewMode.value = v })

// ★ 编辑态：原始快照 + 撤销栈 + 改动检测（Q3）
const originalTree = JSON.parse(JSON.stringify(props.tree));
const editHistory = ref([]);  // 修改前的树快照栈，用于撤销
function pushHistory() { editHistory.value.push(JSON.parse(JSON.stringify(editableTree.value))); }
function undoLastChange() {
  const prev = editHistory.value.pop();
  if (prev) editableTree.value = prev;
}
function resetToOriginal() {
  editableTree.value = JSON.parse(JSON.stringify(originalTree));
  editHistory.value = [];
}

// ★ 已注册工具清单（内置 + skills + MCP + managed）：用于校验任务引用的工具存在性，
//   未找到的在任务行标 ⚠ 警告，让用户在确认执行前就看到
//   null = 尚未加载（接口不可用时不标 ⚠，避免全部误报）；{} = 已加载但清单为空
const toolRegistry = ref(null)
onMounted(async () => {
  try {
    const res = await fastPlanRequest(`${access ? '' : props.apiBase.replace(/\/$/, '')}/api/available-tools`, {}, access)
    const data = await res.json()
    const reg = {}
    for (const it of (data.items || [])) reg[it.name] = it.kind
    toolRegistry.value = reg
  } catch { /* 接口不可用：保持 null，跳过校验，不标 ⚠ */ }
})
// 规划态用 editableTree（可编辑）；执行态/完成态用 props.tree（真实执行结构）
const isPlanPhase = computed(() => props.phase === 'plan');
const isCancelled = computed(() => props.choiceState === 'cancelled' || props.executionStatus === 'cancelled');
const displayTree = computed(() => (isPlanPhase.value ? editableTree.value : props.tree));
const dagData = computed(() => treeToDAG(displayTree.value));
const executionPhases = computed(() => revealedPhaseRows(props.tree, props.executionState, isCancelled.value));
const confirmedHistoryByNode = computed(() =>
  groupConfirmedHistoryByNode(props.tree, props.confirmedHistory).byNodeId
);
const finalTaskId = computed(() => lastPlanTaskId(props.tree));
const collapsedPhases = ref({});
const togglePhase = (id) => { collapsedPhases.value[id] = !collapsedPhases.value[id]; };

// ─── 状态机 Header 文案 ─────────────────────────────────────
const completedCount = computed(() =>
  Object.values(props.executionState || {}).filter(n => n?.status === 'completed').length
);
// ★ 编辑态：choiceState === 'editing'
const isEditing = computed(() => props.choiceState === 'editing');

const nodeTypeOptions = [
  { id: 'LlmTask',         label: 'AI 推理',  icon: Brain,  defaultConfig: { prompt_template: '' } },
  { id: 'ToolTask',        label: '工具调用', icon: Wrench, defaultConfig: { tool_name: '', arguments: {} } },
  { id: 'HumanCheckpoint', label: '人工确认', icon: User,   defaultConfig: { prompt: '', options: ['continue', 'abort'] } },
  { id: 'AgentTask',       label: 'Agent',    icon: Bot,    defaultConfig: { persona: '你是一个专业的任务执行助手', task_description: '', assigned_tools: [], expected_output_format: null } },
];

const switchNodeType = (task, newTypeId) => {
  const cfg = nodeTypeOptions.find(t => t.id === newTypeId);
  if (cfg) task.node_type = { type: newTypeId, config: { ...cfg.defaultConfig } };
};

const editingTypeId = ref(null);
const toggleTypeEdit = (taskId) => { editingTypeId.value = editingTypeId.value === taskId ? null : taskId; };

// ─── 阶段 / 任务 增删 ─────────────────────────────────────
const deletePhase = async (idx) => {
  if (await confirmDialog('确定要删除这个阶段吗？')) editableTree.value.root.children.splice(idx, 1);
};
const deleteTask = (phase, idx) => { phase.children.splice(idx, 1); };

// ★ 编辑态操作：都先压撤销历史，就地改 editableTree，不重建列表
function copyTask(task) {
  pushHistory();
  for (const phase of editableTree.value.root.children) {
    const idx = (phase.children || []).findIndex(t => t.id === task.id);
    if (idx >= 0) {
      const copy = JSON.parse(JSON.stringify(task));
      copy.id = `task_copy_${Date.now()}`;
      copy.name = `${task.name}（副本）`;
      copy.isNew = true;
      delete copy.isEdited;
      phase.children.splice(idx + 1, 0, copy);
      return;
    }
  }
}
function deleteTaskById(task) {
  pushHistory();
  for (const phase of editableTree.value.root.children) {
    const idx = (phase.children || []).findIndex(t => t.id === task.id);
    if (idx >= 0) { phase.children.splice(idx, 1); return; }
  }
}
function addTaskToPhase(phase) {
  pushHistory();
  addTask(phase);
  const last = phase.children[phase.children.length - 1];
  if (last) last.isNew = true;
}
async function deletePhaseById(phaseId) {
  const idx = editableTree.value.root.children.findIndex(p => p.id === phaseId);
  if (idx < 0) return;
  const phase = editableTree.value.root.children[idx];
  const n = (phase.children || []).length;
  if (!await confirmDialog(`删除阶段「${phase.name}」？其下有 ${n} 个任务。`)) return;
  pushHistory();
  editableTree.value.root.children.splice(idx, 1);
}
const addPhase = () => {
  pushHistory();
  const id = `phase_${Date.now()}`;
  const order = editableTree.value.root.children.length + 1;
  editableTree.value.root.children.push({
    id,
    name: '新阶段',
    node_type: {
      type: 'Phase',
      config: { order, description: null }
    },
    children: [],
    dependencies: [],
    metadata: {},
    priority: 'medium',
    requires_confirmation: false,
    retry_config: null,
    tags: [],
    timeout_seconds: null
  });
  collapsedPhases.value[id] = false;
};
const addTask = (phase) => {
  phase.children.push({
    id: `task_${Date.now()}`,
    name: '新任务',
    node_type: {
      type: 'AgentTask',
      config: {
        persona: '你是一个专业的任务执行助手',
        task_description: '请输入任务描述...',
        assigned_tools: [],
        expected_output_format: null
      }
    },
    dependencies: [],
    metadata: {},
    priority: 'medium',
    requires_confirmation: false,
    retry_config: null,
    tags: ['agent_task'],
    timeout_seconds: 300
  });
};

// ─── DAG 视图事件处理 (PRD §1.1 双视图联动) ──────────────────
const handleDagNodeClick = (node) => {
  const selectedNode = findNodeById(node.id) || node;
  emit('node-select', selectedNode);
  for (const p of (editableTree.value.root?.children || [])) {
    if (p.id === node.id || (p.children || []).some(t => t.id === node.id)) {
      collapsedPhases.value[p.id] = false;
      viewMode.value = 'timeline';
      emit('update:activeView', 'timeline');  // ★ 受控 tab：同步回父级
      return;
    }
  }
};

const handleTimelineNodeClick = (node) => {
  emit('node-select', node);
  if (isPlanPhase.value) openNodeEditor(node);
};

// DAG 右键菜单动作
const handleDagContextMenu = ({ action, node }) => {
  switch (action) {
    case 'edit':
      openNodeEditor(findNodeById(node.id));
      break;
    case 'delete':
      handleDagDeleteNode(node.id);
      break;
    case 'add-after':
      openAddNodeDialog(node.id);
      break;
    case 'skip':
      // 规划阶段跳过无意义，提示用户
      break;
  }
};

// ─── DAG 删除节点（带依赖自动修复，PRD §1.2）────────────────
const handleDagDeleteNode = async (nodeId) => {
  const node = findNodeById(nodeId);
  if (!node) return;
  const isPhase = editableTree.value.root.children.some(p => p.id === nodeId);
  const confirmMsg = isPhase ? '确定要删除这个阶段及其所有任务吗？' : '确定要删除这个任务吗？';
  if (!await confirmDialog(confirmMsg)) return;
  deleteNodeWithDependencyFix(editableTree.value, nodeId);
};

// ─── 插入新节点对话框 ─────────────────────────────────────
const addNodeDialog = ref({ show: false, afterNodeId: null });
const openAddNodeDialog = (afterNodeId) => {
  addNodeDialog.value = { show: true, afterNodeId };
  newNodeName.value = '新任务';
  newNodeType.value = 'LlmTask';
};
const newNodeName = ref('新任务');
const newNodeType = ref('LlmTask');

const confirmAddNode = () => {
  const cfg = nodeTypeOptions.find(t => t.id === newNodeType.value);
  const newNode = {
    id: `task_${Date.now()}`,
    name: newNodeName.value || '新任务',
    node_type: { type: newNodeType.value, config: { ...(cfg?.defaultConfig || {}) } }
  };
  insertNodeAfter(editableTree.value, addNodeDialog.value.afterNodeId, newNode);
  addNodeDialog.value.show = false;
};

// ─── Drag & Drop (timeline) ──────────────────────────────
const draggedItem = ref(null), draggedType = ref(null), dragOverItem = ref(null), draggedSourceList = ref(null);
const onDragStart = (e, item, type, src = null) => {
  draggedItem.value = item; draggedType.value = type;
  if (type === 'task' && src) draggedSourceList.value = src;
  e.dataTransfer.effectAllowed = 'move';
  setTimeout(() => { if (e.target) e.target.classList.add('opacity-50'); }, 0);
};
const onDragEnd = (e) => { if (e.target) e.target.classList.remove('opacity-50'); draggedItem.value = null; draggedType.value = null; dragOverItem.value = null; draggedSourceList.value = null; };
const onDragOver = (e, item, type) => {
  if (draggedType.value === 'task' && type === 'phase') { e.preventDefault(); dragOverItem.value = item; return; }
  if (draggedType.value !== type || draggedItem.value === item) return;
  e.preventDefault(); dragOverItem.value = item;
};
const onDrop = (e, target, type, list) => {
  e.preventDefault();
  if (draggedType.value === 'task' && type === 'task') {
    const s = draggedSourceList.value; if (!s) return;
    const fi = s.indexOf(draggedItem.value), ti = list.indexOf(target);
    if (fi > -1 && ti > -1) { const [m] = s.splice(fi, 1); list.splice(ti, 0, m); }
  } else if (draggedType.value === 'phase' && type === 'phase') {
    const fi = list.indexOf(draggedItem.value), ti = list.indexOf(target);
    if (fi > -1 && ti > -1) { const [m] = list.splice(fi, 1); list.splice(ti, 0, m); }
  } else if (draggedType.value === 'task' && type === 'phase') {
    const s = draggedSourceList.value;
    if (s && target.children) { const fi = s.indexOf(draggedItem.value); if (fi > -1) { const [m] = s.splice(fi, 1); target.children.push(m); collapsedPhases.value[target.id] = false; } }
  }
};

// ─── 工具函数 ─────────────────────────────────────────────
const findNodeById = (nodeId) => {
  for (const phase of (editableTree.value.root?.children || [])) {
    if (phase.id === nodeId) return phase;
    const task = (phase.children || []).find(t => t.id === nodeId);
    if (task) return task;
  }
  return null;
};

const getNodeTypeStr = (t) => t.node_type?.type || 'Unknown';
const getNodeModel = (t) => resolveNodeModel(t, props.executionState?.[t?.id]);
const getNodeIcon = (t) => {
  const tp = getNodeTypeStr(t);
  if (tp === 'HumanCheckpoint') return User;
  if (tp === 'LlmTask') return Brain;
  if (tp === 'ToolTask') return Wrench;
  if (tp === 'AgentTask') return Bot;
  return Circle;
};
const getNodeLabel = (t) => {
  const tp = getNodeTypeStr(t);
  return { HumanCheckpoint: '人工确认', LlmTask: 'AI 推理', ToolTask: '工具调用', AgentTask: 'Agent', Phase: '阶段' }[tp] || '任务';
};
const getNodeLabelColor = (t) => {
  const tp = getNodeTypeStr(t);
  if (tp === 'AgentTask') return 'bg-purple-100 text-purple-700';
  if (tp === 'LlmTask') return 'bg-green-100 text-green-700';
  if (tp === 'ToolTask') return 'bg-orange-100 text-orange-700';
  if (tp === 'HumanCheckpoint') return 'bg-blue-100 text-blue-700';
  return 'bg-gray-100 text-gray-500';
};
const getTaskDetailKey = (t) => {
  const tp = getNodeTypeStr(t);
  if (tp === 'LlmTask') return 'prompt_template';
  if (tp === 'HumanCheckpoint') return 'prompt';
  if (tp === 'AgentTask') return 'task_description';
  return 'tool_name';
};

// ─── 业务语义化（★ 后端 display_summary / display_input_name / display_output_name）───
// 卡片上优先展示业务字段，隐藏内部变量（如 user_input()、fault_data、agents list）。
const getNodeSummary = (t) => t.metadata?.display_summary || null;
const getNodeInputLabel = (t) => t.metadata?.display_input_name || null;
const getNodeOutputLabel = (t) => t.metadata?.display_output_name || null;

// 工具 badge 辅助函数
const formatToolName = (tool) => {
  if (tool.includes(':')) return tool.split(':').pop()
  return tool
};
const getToolBadgeClass = (tool) => {
  if (tool.includes(':')) return 'bg-purple-50 text-purple-700'
  if (['xlsx', 'docx', 'pptx', 'pdf', 'weather'].includes(tool)) return 'bg-yellow-50 text-yellow-700'
  return 'bg-green-50 text-green-700'
};

// ★ Issue #26: 获取任务的工具列表，兼容两种数据路径
const getTaskTools = (task) => {
  if (getNodeTypeStr(task) !== 'AgentTask') return []
  return task.node_type?.config?.assigned_tools || task.node_type?.assigned_tools || []
}

// ─── 痛点2: 节点"输入 → 处理 → 输出"可视化 ───────────────────
// 输入:该节点引用的上游输出(input_refs),或依赖节点的 output_key
const getNodeInputs = (task) => {
  const refs = task.metadata?.input_refs
  if (Array.isArray(refs) && refs.length) return refs
  // 退化:用依赖节点的 output_key
  const deps = task.dependencies || []
  return deps.map(id => findNodeById(id)?.metadata?.output_key).filter(Boolean)
}
// 输出:该节点产出的 key
const getNodeOutput = (task) => task.metadata?.output_key || null
// 处理方式:把类型+工具翻译成人话
const getNodeProcessing = (task) => {
  const tp = getNodeTypeStr(task)
  if (tp === 'LlmTask') return { label: 'AI 推理', kind: 'llm' }
  if (tp === 'HumanCheckpoint') return { label: '人工确认', kind: 'human' }
  if (tp === 'ToolTask') {
    const tool = task.node_type?.config?.tool_name
    return { label: tool ? `工具 ${formatToolName(tool)}` : '工具调用', kind: 'tool' }
  }
  if (tp === 'AgentTask') {
    const tools = getTaskTools(task)
    if (tools.length) {
      const isSql = tools.some(t => t.includes('sql') || t.includes('mysql'))
      const isSkill = tools.some(t => !t.includes(':') && !['xlsx','docx','pdf','pptx','file_write','code_execute','web_search','weather'].includes(t))
      if (isSql) return { label: '数据库查询', kind: 'sql' }
      if (isSkill) return { label: `技能 ${formatToolName(tools[0])}`, kind: 'skill' }
      return { label: `Agent · ${tools.map(formatToolName).join('/')}`, kind: 'agent' }
    }
    return { label: 'Agent', kind: 'agent' }
  }
  return { label: tp, kind: 'other' }
}
const procColor = {
  llm:   'bg-blue-50 text-blue-700 border-blue-200',
  human: 'bg-orange-50 text-orange-700 border-orange-200',
  tool:  'bg-amber-50 text-amber-700 border-amber-200',
  sql:   'bg-cyan-50 text-cyan-700 border-cyan-200',
  skill: 'bg-violet-50 text-violet-700 border-violet-200',
  agent: 'bg-violet-50 text-violet-700 border-violet-200',
  other: 'bg-gray-50 text-gray-600 border-gray-200',
}
// 友好显示 output_key(去下划线)
const prettyKey = (k) => (k || '').replace(/_/g, ' ')

const totalPhases = computed(() => editableTree.value.root?.children?.length || 0);
const totalTasks = computed(() => (editableTree.value.root?.children || []).reduce((s, p) => s + (p.children?.length || 0), 0));

// 校验规划树：返回错误信息数组（空数组=通过）
const validateTree = (tree) => {
  const errors = [];
  const root = tree?.root;
  if (!root) { errors.push('规划树缺少根节点'); return errors; }
  const phases = root.children || [];
  if (phases.length === 0) errors.push('至少需要一个阶段');

  const ids = new Set();
  const collect = (n) => { ids.add(n.id); (n.children || []).forEach(collect); };
  collect(root);

  const walk = (n, path) => {
    const tp = getNodeTypeStr(n);
    // 任务名不能为空
    if (n !== root && (!n.name || !n.name.trim())) {
      errors.push(`${path}：存在未命名的节点`);
    }
    // 叶子任务（非 Phase/Root）必须有内容
    if (tp === 'AgentTask') {
      const d = n.node_type?.config?.task_description;
      if (!d || !d.trim() || d.trim() === '请输入任务描述...') {
        errors.push(`「${n.name || n.id}」Agent 任务描述为空`);
      }
    } else if (tp === 'LlmTask') {
      const d = n.node_type?.config?.prompt_template;
      if (!d || !d.trim()) errors.push(`「${n.name || n.id}」AI 推理的提示词为空`);
    } else if (tp === 'ToolTask') {
      const d = n.node_type?.config?.tool_name;
      if (!d || !d.trim()) errors.push(`「${n.name || n.id}」工具调用未指定工具名`);
    }
    // 依赖必须指向存在的节点
    for (const dep of (n.dependencies || [])) {
      if (!ids.has(dep)) errors.push(`「${n.name || n.id}」依赖了不存在的节点：${dep}`);
    }
    (n.children || []).forEach(c => walk(c, n.name || path));
  };
  walk(root, '根');
  return errors;
};

const validationErrors = ref([]);

// ─── 节点编辑器 ───────────────────────────────────────────
const showNodeEditor = ref(false);
const editingNode = ref(null);

const openNodeEditor = (task) => {
  if (!task) return;
  editingNode.value = task;
  showNodeEditor.value = true;
};

// 获取所有可用节点列表（用于依赖选择）
const allAvailableNodes = computed(() => {
  const nodes = [];
  let stepNumber = 1;
  for (const phase of (editableTree.value.root?.children || [])) {
    for (const task of (phase.children || [])) {
      nodes.push({
        id: task.id,
        name: task.name,
        stepNumber: stepNumber++,
        dependencies: task.dependencies || []
      });
    }
  }
  return nodes;
});

const handleNodeEditorSave = (updatedNode) => {
  if (!updatedNode) return;
  for (const phase of (editableTree.value.root?.children || [])) {
    if (phase.id === updatedNode.id) { Object.assign(phase, updatedNode); break; }
    const idx = (phase.children || []).findIndex(t => t.id === updatedNode.id);
    if (idx !== -1) { Object.assign(phase.children[idx], updatedNode); break; }
  }
  showNodeEditor.value = false;
  editingNode.value = null;
};

// ★ 供父组件（ChatView）在确认执行时取回编辑后的树 + 触发校验
defineExpose({
  getEditedTree: () => editableTree.value,
  // ★ 校验流水底部状态条的视图分段控件可联动切换 PlanPreview 视图（受控，同步回父级）
  setViewMode: (m) => { if (['list', 'timeline', 'dag'].includes(m)) { viewMode.value = m; emit('update:activeView', m) } },
  validateTree: () => {
    const errs = validateTree(editableTree.value);
    validationErrors.value = errs;
    return errs;
  },
});
</script>

<template>
  <div data-plan-preview-card class="fpp-ui w-full transition-colors" :data-density="density"
       :class="isEditing ? 'ring-1 ring-amber-300 rounded-lg' : ''">
    <!-- 卡头已删除：标题信息由状态条胶囊承担，视图切换完全由外部 activeView 控制 -->
    <div class="px-1 pt-1 pb-2">
      <!-- ★ 校验错误提示：有问题时显示，不执行/不保存 -->
      <div v-if="isPlanPhase && validationErrors.length > 0" class="mt-2 p-3 rounded-lg bg-red-50 border border-red-200">
        <div class="flex items-center gap-1.5 text-sm font-semibold text-red-600 mb-1">
          <X :size="15" /> 无法{{ mode === 'template' ? '保存' : '执行' }}，请先修正以下问题（{{ validationErrors.length }}）：
        </div>
        <ul class="list-disc list-inside space-y-0.5 text-xs text-red-500">
          <li v-for="(err, i) in validationErrors" :key="i">{{ err }}</li>
        </ul>
      </div>
    </div>

    <!-- Content（自然流动，无内部滚动盒/无卡片壳） -->
    <div class="relative">
      <!-- ★ v21 清单视图：时间轴竖线 + 圆点，规划态/执行态同一套视觉语言（对照 demo #tree） -->
      <div v-if="viewMode === 'list'" class="pt-1">
        <!-- 规划态：PlanTaskRow 可编辑 -->
        <template v-if="isPlanPhase">
          <div v-if="!editableTree.root.children?.length" class="text-xs text-slate-400 py-6 text-center">
            暂无任务节点
          </div>
          <!-- ★ 时间轴容器：左侧 1px 竖线（demo #tree 写法），节点圆点盖在线上 -->
          <div v-else class="relative pl-4 space-y-3
                      before:absolute before:left-[5px] before:top-2 before:bottom-2 before:w-px before:bg-slate-200">
            <div v-for="(phase, pIdx) in editableTree.root.children" :key="phase.id" class="space-y-1">
              <!-- 阶段标题：小号灰字 + 右侧「N 个任务」，无分隔线，靠留白+竖线区分 -->
              <div class="flex items-center justify-between text-[10px] font-semibold text-slate-400">
                <span>阶段{{ ['一','二','三','四','五','六','七','八'][pIdx] || (pIdx+1) }} · {{ phase.name }}</span>
                <span class="flex items-center gap-2">
                  <template v-if="isEditing">
                    <button @click.stop="addTaskToPhase(phase)" :title="'向 ' + phase.name + ' 添加任务'"
                            class="text-[10px] text-slate-400 hover:text-indigo-600 hover:underline px-1">+ 添加任务</button>
                    <button @click.stop="deletePhaseById(phase.id)" :title="'删除阶段（含 ' + (phase.children||[]).length + ' 个任务）'"
                            class="text-[10px] text-slate-400 hover:text-rose-600 hover:underline px-1">✕ 删除阶段</button>
                  </template>
                  <span v-else class="font-mono text-slate-300">{{ (phase.children || []).length }} 个任务</span>
                </span>
              </div>
              <div class="space-y-1.5">
                <PlanTaskRow
                  v-for="(task, tIdx) in (phase.children || [])"
                  :key="task.id"
                  :task="task"
                  :seq="String(pIdx + 1).padStart(2,'0') + (phase.children.length > 1 ? '.' + (tIdx + 1) : '')"
                  :user-input="props.userInput"
                  :tool-registry="toolRegistry"
                  :editing="isEditing"
                  @edit="openNodeEditor"
                  @copy="copyTask"
                  @delete="deleteTaskById"
                  @update-query="() => {}"
                />
              </div>
            </div>
            <!-- ★ 编辑态：添加阶段 -->
            <button v-if="isEditing" @click="addPhase"
                    class="text-[11px] text-slate-400 hover:text-indigo-600 hover:underline px-1">+ 添加阶段</button>
          </div>
        </template>

        <!-- 执行态/完成态：TaskBlock 实时状态，:key="task.id" 保证展开态不丢、不跳动。
             头部已含「执行中 x/N · 共 N 个步骤」，此处不再重复校验流水/状态条 -->
        <template v-else>
          <div v-if="!props.tree?.root?.children?.length" class="text-xs text-slate-400 py-6 text-center">
            暂无任务节点
          </div>
          <div v-else-if="!executionPhases.length && !isCancelled" class="text-xs text-slate-400 py-5">
            等待任务开始执行…
          </div>
          <div v-else-if="executionPhases.length" class="space-y-3">
            <section v-for="row in executionPhases" :key="row.phase.id" class="space-y-1">
              <div v-if="density === 'comfortable'" class="flex items-center justify-between text-[11px] text-slate-400">
                <span>阶段{{ row.phaseIndex + 1 }} · {{ row.phase.name }}</span>
                <span>{{ row.total }} 个任务</span>
              </div>
              <div>
                <template v-for="item in row.tasks" :key="item.node.id">
                  <ManualReviewPanel
                    v-if="confirmedHistoryByNode[item.node.id]?.length"
                    :confirmed-history="confirmedHistoryByNode[item.node.id]"
                  />
                  <TaskBlock
                    :state="executionState[item.node.id] || { id: item.node.id, name: item.node.name, status: 'pending' }"
                    :tree-node="item.node"
                    :seq="item.sequence"
                    :now-ms="nowMs"
                    :is-last-task="item.node.id === finalTaskId"
                    :theme="theme"
                    :density="density"
                  />
                </template>
              </div>
            </section>
          </div>
        </template>
      </div>

      <!-- DAG View -->
      <div v-else-if="viewMode === 'dag'" class="p-4 h-[500px]">
        <GraphView
          :nodes="dagData.nodes"
          :edges="dagData.edges"
          :groups="dagData.groups"
          :executionState="executionState"
          @node-click="handleDagNodeClick"
          @node-context-menu="handleDagContextMenu"
        />
      </div>

      <!-- Timeline View -->
      <div v-else class="py-4">
        <div class="relative pl-4">
          <div class="absolute left-[27px] top-4 bottom-8 w-0.5 bg-gray-200 z-0"></div>

          <div v-for="(phase, pIdx) in displayTree.root.children" :key="phase.id"
            class="mb-6 relative z-10 transition-all duration-200" :draggable="isPlanPhase ? 'true' : 'false'"
            @dragstart="onDragStart($event, phase, 'phase')" @dragend="onDragEnd"
            @dragover="onDragOver($event, phase, 'phase')" @drop="onDrop($event, phase, 'phase', displayTree.root.children)"
            :class="{ 'scale-[0.98] opacity-80': draggedItem === phase, 'translate-y-2': dragOverItem === phase && draggedType === 'phase' }">

            <!-- Phase Header（轻量：小序号 + 阶段名 + 目标 + 折叠） -->
            <div class="flex items-start gap-3 mb-3 group select-none">
              <div class="w-6 h-6 rounded-full bg-indigo-600 text-white flex items-center justify-center text-xs font-bold ring-4 ring-white shrink-0 mt-0.5 cursor-pointer" @click="togglePhase(phase.id)">{{ pIdx + 1 }}</div>
              <div class="flex-1 min-w-0">
                <div class="flex items-center justify-between gap-2">
                  <input v-if="isPlanPhase" v-model="phase.name" class="flex-1 font-semibold text-base text-gray-800 bg-transparent border-b border-transparent hover:border-gray-200 focus:border-indigo-500 focus:outline-none transition-colors px-0.5 py-0.5" placeholder="阶段名称" />
                  <span v-else class="flex-1 font-semibold text-base text-gray-800 px-0.5 py-0.5">{{ phase.name }}</span>
                  <div class="flex items-center gap-1 flex-shrink-0">
                    <button v-if="isPlanPhase" @click="deletePhase(pIdx)" class="p-1 text-gray-300 hover:text-red-500 opacity-0 group-hover:opacity-100 transition-opacity" title="删除阶段"><Trash2 :size="14" /></button>
                    <button @click="togglePhase(phase.id)" class="p-1 text-gray-400 hover:text-gray-600"><component :is="collapsedPhases[phase.id] ? ChevronRight : ChevronDown" :size="18" /></button>
                  </div>
                </div>
                <!-- ★ 阶段目标（业务语义化） -->
                <p v-if="phase.node_type?.config?.description" class="mt-0.5 text-xs text-slate-500 leading-5">{{ phase.node_type.config.description }}</p>
              </div>
            </div>

            <!-- Tasks -->
            <transition enter-active-class="transition-all duration-300 ease-out" leave-active-class="transition-all duration-200 ease-in" enter-from-class="opacity-0 -translate-y-2" leave-to-class="opacity-0 -translate-y-2">
              <div v-if="!collapsedPhases[phase.id]" class="pl-[3.5rem] space-y-3">
                <template v-for="(task, tIdx) in phase.children" :key="task.id">
                  <ManualReviewPanel
                    v-if="!isPlanPhase && confirmedHistoryByNode[task.id]?.length"
                    :confirmed-history="confirmedHistoryByNode[task.id]"
                  />
                  <div
                    class="group relative bg-white hover:bg-indigo-50/50 p-3 rounded-lg border border-slate-100 hover:border-indigo-200 transition-all"
                    :class="{ 'cursor-pointer': isPlanPhase, 'scale-[0.98] opacity-80': draggedItem === task, 'translate-y-2': dragOverItem === task && draggedType === 'task' }"
                    :draggable="isPlanPhase ? 'true' : 'false'" @dragstart.stop="onDragStart($event, task, 'task', phase.children)" @dragend.stop="onDragEnd"
                    @dragover.stop="onDragOver($event, task, 'task')" @drop.stop="onDrop($event, task, 'task', phase.children)"
                    @click="handleTimelineNodeClick(task)">
                  <!-- 时间线圆点 -->
                  <span class="absolute -left-[23px] top-4 w-2.5 h-2.5 rounded-full bg-indigo-600 ring-4 ring-white"></span>

                  <div class="flex items-center justify-between gap-2">
                    <div class="flex items-center gap-2 min-w-0">
                      <ChatModelIcon v-if="getNodeTypeStr(task) === 'LlmTask'" class="fpp-model-icon" :name="getNodeModel(task).name" :model-id="getNodeModel(task).id" :size="15" />
                      <span class="text-[10px] font-semibold px-1.5 py-0.5 rounded whitespace-nowrap" :class="getNodeLabelColor(task)">{{ getNodeLabel(task) }}</span>
                      <h4 class="text-sm font-semibold text-slate-800 group-hover:text-indigo-600 truncate">{{ task.name }}</h4>
                      <template v-if="getNodeTypeStr(task) === 'AgentTask' && getTaskTools(task).length">
                        <span v-for="tool in getTaskTools(task).slice(0, 2)" :key="tool"
                          class="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium whitespace-nowrap flex-shrink-0"
                          :class="getToolBadgeClass(tool)">{{ formatToolName(tool) }}</span>
                        <span v-if="getTaskTools(task).length > 2" class="text-[10px] text-gray-400 flex-shrink-0">+{{ getTaskTools(task).length - 2 }}</span>
                      </template>
                      <span v-else-if="getNodeTypeStr(task) === 'LlmTask'" class="text-[10px] px-1.5 py-0.5 rounded bg-blue-50 text-blue-700 whitespace-nowrap flex-shrink-0">{{ getNodeModel(task).name || 'LLM' }}</span>
                    </div>
                    <div class="flex items-center gap-1.5 flex-shrink-0">
                      <span v-if="isPlanPhase" class="text-[10px] opacity-0 group-hover:opacity-100 text-indigo-600 transition-opacity whitespace-nowrap">点击配置 ⚙️</span>
                      <button v-if="isPlanPhase" @click.stop="deleteTask(phase, tIdx)" class="text-gray-300 hover:text-red-500 p-1 opacity-0 group-hover:opacity-100 transition-opacity" title="删除任务"><Trash2 :size="13" /></button>
                    </div>
                  </div>

                  <!-- ★ 业务摘要（display_summary，纯文本；隐藏内部变量/Markdown） -->
                  <p v-if="getNodeSummary(task)" class="text-xs text-slate-500 mt-1 line-clamp-1">{{ getNodeSummary(task) }}</p>

                  <!-- 输入 → 处理 → 输出（轻量小字，业务名优先，隐藏内部变量） -->
                  <div class="mt-1 flex items-center gap-1.5 flex-wrap text-[10px] text-slate-400">
                    <span class="text-slate-500">输入</span>
                    <span class="font-medium text-slate-600">{{ getNodeInputLabel(task) || '用户请求' }}</span>
                    <span class="text-slate-300">→</span>
                    <span class="font-medium text-slate-600">{{ getNodeProcessing(task).label }}</span>
                    <span class="text-slate-300">→</span>
                    <span class="text-slate-500">输出</span>
                    <span class="font-medium text-slate-600">{{ getNodeOutputLabel(task) || '结果' }}</span>
                  </div>

                  <!-- AgentTask 子任务预览（轻量） -->
                  <div v-if="getNodeTypeStr(task) === 'AgentTask' && task.node_type.config.sub_tasks?.length" class="mt-1.5 pt-1.5 border-t border-dashed border-violet-100">
                    <div class="space-y-0.5">
                      <div v-for="sub in task.node_type.config.sub_tasks" :key="sub.id" class="text-[10px] text-gray-500 flex items-center gap-1.5">
                        <div class="w-1 h-1 rounded-full bg-violet-300 flex-shrink-0"></div>
                        {{ sub.name }}
                      </div>
                    </div>
                  </div>
                  </div>
                </template>

                <button v-if="isPlanPhase" @click="addTask(phase)" class="w-full py-1.5 border border-dashed border-slate-200 hover:border-indigo-300 hover:text-indigo-600 rounded-lg text-xs text-slate-400 flex items-center justify-center gap-1 transition-all">
                  <Plus :size="12" /> 添加执行任务节点
                </button>
              </div>
            </transition>
          </div>

          <div v-if="isPlanPhase" class="pl-1 mb-8">
            <button @click="addPhase" class="w-full py-1.5 border border-dashed border-slate-200 hover:border-indigo-300 hover:text-indigo-600 rounded-lg text-xs text-slate-400 flex items-center justify-center gap-1 transition-all">
              <Plus :size="12" /> 添加新阶段
            </button>
          </div>

          <div class="flex items-center gap-4 pl-0 pb-4">
            <div class="w-6 h-6 rounded-full bg-gray-100 flex items-center justify-center z-10 border-4 border-white shadow-sm"><CheckCircle2 class="text-gray-400" :size="20" /></div>
            <span class="text-sm text-gray-400 font-medium">{{ isPlanPhase ? '规划结束' : '执行结束' }}</span>
          </div>
        </div>
      </div>
    </div>

    <div v-if="isCancelled" data-plan-cancelled-status role="status"
         class="mt-3 px-3 py-2 rounded-lg border border-slate-200 bg-slate-50 text-xs font-medium text-slate-500">
      任务已取消执行
    </div>

    <!-- 规划确认已统一迁移至输入框上方的信息区域；正文编辑态仅保留编辑辅助操作。 -->
    <div v-if="isEditing" class="pt-3 border-t border-slate-100 flex items-center gap-2 min-h-[30px]">
      <button @click="undoLastChange" :disabled="!editHistory.length"
              class="px-3 py-1.5 rounded-lg text-slate-500 hover:bg-slate-50 disabled:opacity-40 transition-colors">
        撤销修改
      </button>
      <button @click="resetToOriginal"
              class="px-3 py-1.5 rounded-lg text-rose-500 hover:bg-rose-50 transition-colors">
        放弃修改，恢复原计划
      </button>
    </div>
  </div>

  <!-- 节点编辑器 -->
  <NodeEditor
    v-if="showNodeEditor && editingNode"
    :node="editingNode"
    :visible="showNodeEditor"
    :api-base="apiBase"
    :admin-base="adminBase"
    :allNodes="allAvailableNodes"
    mode="plan"
    @close="showNodeEditor = false; editingNode = null"
    @save="handleNodeEditorSave"
  />

  <!-- DAG "在后方插入" 对话框 -->
  <teleport to="body">
    <div class="fpp-ui">
    <transition enter-active-class="transition-opacity duration-200" leave-active-class="transition-opacity duration-150" enter-from-class="opacity-0" leave-to-class="opacity-0">
      <div v-if="addNodeDialog.show" class="fixed inset-0 z-[200] bg-black/40 backdrop-blur-sm flex items-center justify-center" @click.self="addNodeDialog.show = false">
        <div class="bg-white rounded-2xl shadow-2xl p-6 w-full max-w-sm" @click.stop>
          <h3 class="font-bold text-gray-800 mb-4 flex items-center gap-2"><Plus :size="18" class="text-blue-600" /> 插入新任务节点</h3>
          <div class="space-y-4">
            <div>
              <label class="text-xs font-semibold text-gray-500 uppercase tracking-wider block mb-1.5">任务名称</label>
              <input v-model="newNodeName" class="w-full px-3 py-2.5 border border-gray-200 rounded-xl text-sm focus:border-blue-400 focus:ring-2 focus:ring-blue-500/20 outline-none transition-all" placeholder="输入任务名称" />
            </div>
            <div>
              <label class="text-xs font-semibold text-gray-500 uppercase tracking-wider block mb-1.5">节点类型</label>
              <div class="grid grid-cols-2 gap-2">
                <button v-for="nt in nodeTypeOptions" :key="nt.id" @click="newNodeType = nt.id"
                  class="flex items-center gap-2 px-3 py-2 rounded-xl border text-xs font-medium transition-all"
                  :class="newNodeType === nt.id ? 'border-blue-400 bg-blue-50 text-blue-700 shadow-sm' : 'border-gray-200 text-gray-500 hover:border-gray-300'">
                  <component :is="nt.icon" :size="14" /> {{ nt.label }}
                </button>
              </div>
            </div>
          </div>
          <div class="flex gap-2 mt-5">
            <button @click="addNodeDialog.show = false" class="flex-1 px-4 py-2.5 border border-gray-200 text-gray-600 rounded-xl text-sm hover:bg-gray-50 transition-colors">取消</button>
            <button @click="confirmAddNode" class="flex-1 px-4 py-2.5 bg-blue-600 text-white rounded-xl text-sm font-medium hover:bg-blue-700 transition-colors shadow-sm">插入</button>
          </div>
        </div>
      </div>
    </transition>
    </div>
  </teleport>
</template>
