<script setup lang="ts">
import { defineComponent, h, ref } from 'vue'
import {
  ChatWorkspace,
  type ChatStreamEvent,
  type ChatTimelineEntry,
  type ChatWorkspaceAdapter,
  type ChatWorkspacePlugins,
} from '../src'

const input = ref('')
const timeline = ref<ChatTimelineEntry[]>([
  {
    kind: 'turn', id: 'welcome-turn', activeId: 'welcome-a', comparisonMode: 'tabs',
    user: { id: 'welcome-u', role: 'user', createdAt: Date.now(), parts: [{ type: 'text', text: '这个工作台支持哪些内容？' }] },
    alternatives: [
      { id: 'welcome-a', role: 'assistant', createdAt: Date.now(), model: { id: 'demo', name: 'Demo Model' }, status: 'complete', parts: [{ type: 'text', format: 'markdown', text: '普通消息、多模型答案和嵌入式内容可以出现在**同一时间线**。' }] },
      { id: 'welcome-b', role: 'assistant', createdAt: Date.now(), model: { id: 'compact', name: 'Compact Model' }, status: 'complete', parts: [{ type: 'text', text: '它也支持适配器驱动的流式回答。' }] },
    ],
  },
  { kind: 'embedded', id: 'mock-plan', renderer: 'mock-plan', data: { steps: ['分析目标', '生成方案', '等待宿主确认'] }, status: 'active', metadata: { navigationLabel: '示例规划' } },
])

const MockPlan = defineComponent({
  props: { data: { type: Object, required: true } },
  setup(props) {
    return () => h('section', { class: 'demo-plan' }, [
      h('strong', '嵌入式规划插件（仅本地 mock）'),
      h('ol', (props.data as { steps: string[] }).steps.map(step => h('li', step))),
    ])
  },
})

const plugins: ChatWorkspacePlugins = { timelineRenderers: { 'mock-plan': MockPlan } }
const adapter: ChatWorkspaceAdapter = {
  async *send(input): AsyncIterable<ChatStreamEvent> {
    const text = `收到：${input.text}\n\n此回答由本地 mock adapter 分段生成，没有调用任何后端接口。`
    for (const chunk of text.match(/.{1,8}/gs) || []) {
      await new Promise(resolve => setTimeout(resolve, 90))
      yield { type: 'text-delta', delta: chunk }
    }
    yield { type: 'message-complete' }
  },
}
</script>

<template>
  <main class="demo-shell">
    <ChatWorkspace v-model="input" v-model:timeline="timeline" :adapter="adapter" :plugins="plugins"
      selected-model-id="mock-model" :show-question-navigator="true" :context-items="[
        { id: 'model', kind: 'model', label: '模型', value: 'mock-model', active: true },
      ]" :composer-actions="[{ id: 'attachment', label: '附件', position: 'start' }]" />
  </main>
</template>
