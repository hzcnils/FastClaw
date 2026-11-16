import { computed, inject, provide } from 'vue'

export const FASTPLAN_PLANNING_I18N = Symbol('fastplan-planning-i18n')

/**
 * Stable keys for every generic planning control. Hosts can pass a partial
 * language pack through `labels`; missing keys fall back to Chinese.
 */
export const defaultPlanningMessages = {
  allCompleted: '全部完成', taskList: '任务列表', taskProgress: '任务执行进度',
  bottomPanel: '底部信息类型', taskSteps: '任务步骤', confirmationForm: '确认表单',
  waitingForConfirmation: '等待人工确认', confirmationRequired: '需要您的确认',
  keyboardConfirmHint: '↑↓ 选择 · Enter 确认 · Esc 收起',
  expandDetails: '展开详情', collapseDetails: '收起详情',
  close: '关闭', cancel: '取消', confirm: '确定', pause: '暂停', resume: '继续', stop: '停止', save: '保存', saveAndContinue: '保存并继续',
  edit: '编辑', copy: '复制', delete: '删除', refresh: '刷新', retry: '重试',
  pending: '待执行', modified: '已修改', added: '新增',
  nodeName: '节点名称', taskName: '任务名称', phaseName: '阶段名称',
  inputTaskName: '输入任务名称', addTask: '添加任务', addPhase: '添加阶段',
  deleteTask: '删除任务', deletePhase: '删除阶段', addParameter: '添加参数',
  parameterName: '参数名', parameterValue: '参数值', noParameters: '暂无参数，点击“添加参数”',
  promptTemplate: 'Prompt 模板', confirmationPrompt: '确认提示语',
  taskInstruction: '请输入 AI 任务指令...', humanPrompt: '请输入需要人工确认的提示信息...',
  options: '选项（逗号分隔）', toolName: '工具名称', parameters: '参数',
  agentType: 'Agent 类型', persona: 'Persona / 角色设定', taskDescription: '任务描述 / 目标',
  executionMode: '执行方式', toolCall: '工具调用', externalWorkflow: '外部工作流',
  workflowConfig: '外部工作流配置', workflowEngine: '工作流引擎', apiUrl: 'API 地址',
  streamingTrace: '启用流式追踪', timeoutSeconds: '超时（秒）', inputMapping: '输入参数映射',
  fetchSchema: '获取参数 Schema', testExecution: '测试执行', testSucceeded: '✅ 测试成功',
  subtasks: '子任务流程（OODA 步骤）', addSubtask: '添加子任务', subtaskName: '子任务名称',
  assignedTools: '分配的工具/技能', assignedCount: '{count} 个已分配',
  refreshTools: '根据任务描述重新检索相关工具', searching: '检索中...',
  noToolsAssigned: '⚠️ 未分配任何工具，执行时可能失败', allTools: '全部技能/工具',
  recommendedTools: '按描述推荐', searchTools: '搜索技能或工具名称...',
  loadingTools: '正在检索相关工具...', loadFailed: '❌ 加载失败: {error}',
  noMatchingTools: '未找到匹配的技能/工具', noAvailableTools: '暂无可用技能/工具，请先在 Skills 管理中导入',
  noDescription: '无描述', recommended: '⭐ 推荐', relevance: '相关度: {score}%',
  upstreamDependencies: '上游依赖节点', selectNode: '选择节点',
  noDependencies: '无依赖 — 将按顺序执行', searchNodes: '搜索节点...',
  noMatchingNodes: '未找到匹配的节点', noAvailableNodes: '暂无可用节点',
  circularDependency: '⚠️ 选择此节点会导致循环依赖',
  nodeDetails: '节点详情', closeNodeDetails: '关闭节点详情',
  planView: '规划展示类型', list: '清单', timeline: '时间轴', dag: 'DAG 图',
  planning: '规划中', running: '进行中', completed: '已完成', failed: '失败', executionCancelled: '任务已取消执行',
  aiReasoning: 'AI 推理', humanConfirmation: '人工确认', builtin: '内置',
  toolInvocation: '工具调用', databaseQuery: '数据库查询', skill: '技能', phase: '阶段',
  unsaved: '未保存', addNode: '添加节点', undo: '撤销',
  keyboardSaveHint: 'Ctrl+S 快速保存 · Esc 关闭',
  noSubtasks: '暂无子任务 — 子任务由 OODA 循环自动生成，也可手动添加',
  nodeRunsAfterDependencies: '💡 该节点将在所有上游节点完成后才开始执行',
  assignedSuffix: ' 个已分配', loadFailedPrefix: '❌ 加载失败: ', relevancePrefix: '相关度: ',
}

export const englishPlanningMessages = {
  allCompleted: 'All completed', taskList: 'Task list', taskProgress: 'Task progress',
  bottomPanel: 'Bottom panel', taskSteps: 'Task steps', confirmationForm: 'Confirmation',
  waitingForConfirmation: 'Awaiting confirmation', confirmationRequired: 'Your confirmation is required',
  keyboardConfirmHint: '↑↓ Select · Enter confirm · Esc collapse',
  expandDetails: 'Expand details', collapseDetails: 'Collapse details',
  close: 'Close', cancel: 'Cancel', confirm: 'Confirm', pause: 'Pause', resume: 'Resume', stop: 'Stop', save: 'Save', saveAndContinue: 'Save and continue',
  edit: 'Edit', copy: 'Copy', delete: 'Delete', refresh: 'Refresh', retry: 'Retry',
  pending: 'Pending', modified: 'Modified', added: 'Added',
  nodeName: 'Node name', taskName: 'Task name', phaseName: 'Phase name',
  inputTaskName: 'Enter task name', addTask: 'Add task', addPhase: 'Add phase',
  deleteTask: 'Delete task', deletePhase: 'Delete phase', addParameter: 'Add parameter',
  parameterName: 'Parameter name', parameterValue: 'Parameter value', noParameters: 'No parameters. Click “Add parameter”.',
  promptTemplate: 'Prompt template', confirmationPrompt: 'Confirmation prompt',
  taskInstruction: 'Enter an AI task instruction...', humanPrompt: 'Enter the message for human confirmation...',
  options: 'Options (comma-separated)', toolName: 'Tool name', parameters: 'Parameters',
  agentType: 'Agent type', persona: 'Persona / role', taskDescription: 'Task description / objective',
  executionMode: 'Execution mode', toolCall: 'Tool call', externalWorkflow: 'External workflow',
  workflowConfig: 'External workflow configuration', workflowEngine: 'Workflow engine', apiUrl: 'API URL',
  streamingTrace: 'Enable streaming trace', timeoutSeconds: 'Timeout (seconds)', inputMapping: 'Input mapping',
  fetchSchema: 'Fetch parameter schema', testExecution: 'Test execution', testSucceeded: '✅ Test succeeded',
  subtasks: 'Subtask flow (OODA steps)', addSubtask: 'Add subtask', subtaskName: 'Subtask name',
  assignedTools: 'Assigned tools / skills', assignedCount: '{count} assigned',
  refreshTools: 'Search tools again from the task description', searching: 'Searching...',
  noToolsAssigned: '⚠️ No tools assigned; execution may fail', allTools: 'All tools / skills',
  recommendedTools: 'Recommend by description', searchTools: 'Search tools or skills...',
  loadingTools: 'Searching relevant tools...', loadFailed: '❌ Load failed: {error}',
  noMatchingTools: 'No matching tools or skills', noAvailableTools: 'No tools available. Import them in Skills first.',
  noDescription: 'No description', recommended: '⭐ Recommended', relevance: 'Relevance: {score}%',
  upstreamDependencies: 'Upstream dependencies', selectNode: 'Select node',
  noDependencies: 'No dependencies — will run in order', searchNodes: 'Search nodes...',
  noMatchingNodes: 'No matching nodes', noAvailableNodes: 'No nodes available',
  circularDependency: '⚠️ Selecting this node creates a circular dependency',
  nodeDetails: 'Node details', closeNodeDetails: 'Close node details',
  planView: 'Planning view', list: 'List', timeline: 'Timeline', dag: 'DAG',
  planning: 'Planning', running: 'Running', completed: 'Completed', failed: 'Failed', executionCancelled: 'Task execution cancelled',
  aiReasoning: 'AI reasoning', humanConfirmation: 'Human confirmation', builtin: 'Built-in',
  toolInvocation: 'Tool invocation', databaseQuery: 'Database query', skill: 'Skill', phase: 'Phase',
  unsaved: 'Unsaved', addNode: 'Add node', undo: 'Undo',
  keyboardSaveHint: 'Ctrl+S save · Esc close',
  noSubtasks: 'No subtasks. They are generated by the OODA loop or can be added manually.',
  nodeRunsAfterDependencies: '💡 This node starts after every upstream dependency completes.',
  assignedSuffix: ' assigned', loadFailedPrefix: '❌ Load failed: ', relevancePrefix: 'Relevance: ',
}

const staticTextKeys = {
  ...Object.fromEntries(Object.entries(defaultPlanningMessages).map(([key, value]) => [value, key])),
  '全部完成': 'allCompleted', '任务列表': 'taskList', '任务执行进度': 'taskProgress', '底部信息类型': 'bottomPanel',
  '任务步骤': 'taskSteps', '确认表单': 'confirmationForm', '等待人工确认': 'waitingForConfirmation', '需要您的确认': 'confirmationRequired',
  '展开详情': 'expandDetails', '折叠详情': 'collapseDetails', '收起详情': 'collapseDetails',
  '关闭': 'close', '取消': 'cancel', '确定': 'confirm', '保存': 'save', '保存并继续': 'saveAndContinue',
  '编辑': 'edit', '复制': 'copy', '删除': 'delete', '刷新': 'refresh', '重试': 'retry', '待执行': 'pending',
  '已修改': 'modified', '新增': 'added', '节点名称': 'nodeName', '任务名称': 'taskName', '阶段名称': 'phaseName',
  '输入任务名称': 'inputTaskName', '参数名': 'parameterName', '参数值': 'parameterValue',
  'Prompt 模板': 'promptTemplate', '确认提示语': 'confirmationPrompt', '工具名称': 'toolName', '参数': 'parameters',
  'Agent 类型': 'agentType', 'Persona / 角色设定': 'persona', '任务描述 / 目标': 'taskDescription',
  '执行方式': 'executionMode', '工具调用': 'toolCall', '外部工作流': 'externalWorkflow',
  '外部工作流配置': 'workflowConfig', '工作流引擎': 'workflowEngine', 'API 地址': 'apiUrl',
  '启用流式追踪': 'streamingTrace', '超时（秒）': 'timeoutSeconds', '输入参数映射': 'inputMapping',
  '获取参数 Schema': 'fetchSchema', '测试执行': 'testExecution', '子任务流程（OODA 步骤）': 'subtasks',
  '添加子任务': 'addSubtask', '子任务名称': 'subtaskName', '分配的工具/技能': 'assignedTools',
  '全部技能/工具': 'allTools', '按描述推荐': 'recommendedTools', '上游依赖节点': 'upstreamDependencies',
  '选择节点': 'selectNode', '规划展示类型': 'planView', '清单': 'list', '时间轴': 'timeline', 'DAG 图': 'dag',
  '任务已取消执行': 'executionCancelled',
  'AI 推理': 'aiReasoning', '人工确认': 'humanConfirmation', '内置': 'builtin', '技能': 'skill', '阶段': 'phase',
  '未保存': 'unsaved', '添加节点': 'addNode', '撤销': 'undo',
  'Ctrl+S 快速保存 · Esc 关闭': 'keyboardSaveHint',
  '暂无子任务 — 子任务由 OODA 循环自动生成，也可手动添加': 'noSubtasks',
  '💡 该节点将在所有上游节点完成后才开始执行': 'nodeRunsAfterDependencies',
  '暂无参数，点击"添加参数"': 'noParameters',
  '暂无可用技能/工具,请先在 Skills 管理中导入': 'noAvailableTools',
  ' 个已分配': 'assignedSuffix', '❌ 加载失败: ': 'loadFailedPrefix', '相关度: ': 'relevancePrefix',
}

export function resolvePlanningMessages(locale = 'zh-CN', labels = {}) {
  return { ...(String(locale).toLowerCase().startsWith('en') ? englishPlanningMessages : defaultPlanningMessages), ...labels }
}

export function formatPlanningMessage(messages, key, variables = {}) {
  return String(messages?.[key] || defaultPlanningMessages[key] || key).replace(/\{(\w+)\}/g, (_, name) => String(variables[name] ?? ''))
}

export function providePlanningI18n(locale, labels) {
  const messages = computed(() => resolvePlanningMessages(locale?.value ?? locale, labels?.value ?? labels))
  provide(FASTPLAN_PLANNING_I18N, messages)
  return messages
}

export function usePlanningI18n() {
  const messages = inject(FASTPLAN_PLANNING_I18N, computed(() => defaultPlanningMessages))
  return { messages, t: (key, variables) => formatPlanningMessage(messages.value, key, variables) }
}

function translateText(raw, messages) {
  return Object.entries(staticTextKeys)
    .sort(([left], [right]) => right.length - left.length)
    .reduce((value, [source, key]) => {
      const translated = formatPlanningMessage(messages, key)
      return translated === source ? value : value.split(source).join(translated)
    }, raw)
}

function translateNode(node, messages) {
  if (node.nodeType === Node.TEXT_NODE) {
    const raw = node.nodeValue || ''
    const translated = translateText(raw, messages)
    if (translated !== raw) node.nodeValue = translated
    return
  }
  if (!(node instanceof Element)) return
  for (const attribute of ['title', 'aria-label', 'placeholder']) {
    const raw = node.getAttribute(attribute)
    if (raw) {
      const translated = translateText(raw, messages)
      if (translated !== raw) node.setAttribute(attribute, translated)
    }
  }
  node.childNodes.forEach(child => translateNode(child, messages))
}

/** Applies translations to static labels in the planning editor's nested UI. */
export const planI18n = {
  mounted(element, binding) {
    const apply = () => translateNode(element, binding.value || defaultPlanningMessages)
    apply()
    element.__fastPlanI18nObserver = new MutationObserver(apply)
    element.__fastPlanI18nObserver.observe(element, { childList: true, subtree: true, characterData: true, attributes: true, attributeFilter: ['title', 'aria-label', 'placeholder'] })
  },
  updated(element, binding) {
    translateNode(element, binding.value || defaultPlanningMessages)
  },
  unmounted(element) {
    element.__fastPlanI18nObserver?.disconnect()
    delete element.__fastPlanI18nObserver
  },
}
