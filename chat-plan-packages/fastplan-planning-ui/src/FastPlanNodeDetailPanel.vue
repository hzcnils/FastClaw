<script setup>
import { computed } from 'vue'
import { formatDuration } from './utils/duration'

const props = defineProps({
  nodeId: { type: [String, Number], default: null },
  tree: { type: Object, default: null },
  executionState: { type: Object, default: () => ({}) },
  isPaused: { type: Boolean, default: false },
})
defineEmits(['close', 'retry', 'skip', 'resume', 'edit'])

const phases = computed(() => props.tree?.root?.children || [])
const node = computed(() => {
  for (const phase of phases.value) {
    if (String(phase.id) === String(props.nodeId)) return phase
    const task = (phase.children || []).find(item => String(item.id) === String(props.nodeId))
    if (task) return task
  }
  return null
})
const state = computed(() => props.executionState?.[props.nodeId] || {})
const status = computed(() => state.value.status || 'pending')
const config = computed(() => node.value?.node_type?.config || {})
const prompt = computed(() => config.value.prompt_template || config.value.prompt || config.value.tool_name || '')
const output = computed(() => {
  const raw = state.value.output || state.value.streamContent || ''
  if (typeof raw !== 'string') return JSON.stringify(raw, null, 2)
  try {
    const parsed = JSON.parse(raw)
    return parsed?.message || parsed?.content || parsed?.result || parsed?.output || raw
  } catch { return raw }
})
const duration = computed(() => {
  if (!state.value.startTime) return ''
  const elapsed = Number(state.value.endTime || Date.now()) - Number(state.value.startTime)
  return formatDuration(elapsed, { live: !state.value.endTime })
})
const parent = computed(() => phases.value.find(phase => (phase.children || []).some(item => String(item.id) === String(props.nodeId))))
const nodeType = computed(() => ({ LlmTask: 'AI 推理', ToolTask: '工具调用', HumanCheckpoint: '人工确认', AgentTask: 'Agent 任务' })[node.value?.node_type?.type] || '任务')
const canResume = computed(() => props.isPaused || status.value === 'pending')
</script>

<template>
  <transition name="fpp-node-detail">
    <aside v-if="nodeId != null && node" class="fpp-node-detail" :data-status="status">
      <header><div><strong>节点详情</strong><small>{{ nodeType }}</small></div><button type="button" aria-label="关闭节点详情" @click="$emit('close')">×</button></header>
      <div class="fpp-node-detail__body">
        <section><h3>{{ node.name }}</h3><span class="fpp-node-detail__status">{{ status }}</span></section>
        <dl v-if="duration || parent"><template v-if="duration"><dt>耗时</dt><dd>{{ duration }}</dd></template><template v-if="parent"><dt>所属阶段</dt><dd>{{ parent.name }}</dd></template></dl>
        <section v-if="prompt"><h4>输入 / 描述</h4><pre>{{ prompt }}</pre></section>
        <section v-if="config.arguments"><h4>工具参数</h4><pre>{{ JSON.stringify(config.arguments, null, 2) }}</pre></section>
        <section v-if="output"><h4>输出</h4><pre>{{ output }}</pre></section>
        <section v-if="state.error"><h4>错误信息</h4><pre class="fpp-node-detail__error">{{ state.error }}</pre></section>
      </div>
      <footer>
        <button v-if="canResume" type="button" data-primary @click="$emit('resume', nodeId)">执行</button>
        <button v-if="status === 'failed'" type="button" data-primary @click="$emit('retry', nodeId)">重试</button>
        <button v-if="status === 'pending' || status === 'running'" type="button" @click="$emit('skip', nodeId)">跳过</button>
        <button type="button" @click="$emit('edit', nodeId)">编辑</button>
      </footer>
    </aside>
  </transition>
</template>
