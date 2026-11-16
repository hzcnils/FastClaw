<script setup>
import { ref } from 'vue'
import { Wand2, Loader2 } from 'lucide-vue-next'

const props = defineProps({
  entityType: { type: String, required: true }, // 'skill' | 'mcp_server' | 'agent'
  context: { type: Object, required: true }, // { name, current_description, ... }
  modelValue: { type: String, default: '' }
})

const emit = defineEmits(['update:modelValue'])

const generating = ref(false)
const error = ref('')

const API_BASE = import.meta.env.VITE_API_BASE || 'http://localhost:23180'

async function generateDescription() {
  if (!props.context.name && !props.context.current_description) {
    error.value = '请先填写名称或其他信息'
    setTimeout(() => { error.value = '' }, 3000)
    return
  }

  generating.value = true
  error.value = ''

  try {
    const mode = props.modelValue ? 'expand' : 'generate'
    const response = await fetch(`${API_BASE}/api/v1/ai/generate-description`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        entity_type: props.entityType,
        context: props.context,
        mode
      })
    })

    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`)
    }

    const reader = response.body.getReader()
    const decoder = new TextDecoder()
    let accumulated = ''

    while (true) {
      const { done, value } = await reader.read()
      if (done) break

      const chunk = decoder.decode(value, { stream: true })
      const lines = chunk.split('\n')

      for (const line of lines) {
        if (line.startsWith('data: ')) {
          try {
            const data = JSON.parse(line.slice(6))
            if (data.text) {
              accumulated += data.text
              emit('update:modelValue', accumulated)
            }
            if (data.done) {
              generating.value = false
              return
            }
          } catch (e) {
            console.warn('Failed to parse SSE data:', e)
          }
        }
      }
    }

    generating.value = false
  } catch (e) {
    console.error('Failed to generate description:', e)
    error.value = '生成失败，请重试'
    generating.value = false
    setTimeout(() => { error.value = '' }, 3000)
  }
}
</script>

<template>
  <div class="relative inline-block">
    <button
      @click="generateDescription"
      :disabled="generating"
      class="p-1.5 rounded-lg transition-all disabled:opacity-50 disabled:cursor-not-allowed"
      :class="generating
        ? 'text-blue-500 bg-blue-50'
        : 'text-gray-400 hover:text-blue-600 hover:bg-blue-50'"
      :title="modelValue ? '扩写描述' : '生成描述'"
    >
      <Loader2 v-if="generating" :size="14" class="animate-spin" />
      <Wand2 v-else :size="14" />
    </button>

    <!-- 错误提示 -->
    <transition
      enter-active-class="transition-opacity duration-200"
      leave-active-class="transition-opacity duration-150"
      enter-from-class="opacity-0"
      leave-to-class="opacity-0"
    >
      <div v-if="error" class="absolute top-full right-0 mt-1 px-2 py-1 bg-red-50 border border-red-200 rounded-lg text-xs text-red-600 whitespace-nowrap z-50 shadow-lg">
        {{ error }}
      </div>
    </transition>
  </div>
</template>
