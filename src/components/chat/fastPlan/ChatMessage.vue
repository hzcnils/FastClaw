<script setup lang="ts">
import { computed } from 'vue'
import { Bot, User, Settings, AlertCircle, Lightbulb, Loader2, FileText } from 'lucide-vue-next'
import type { FastPlanMessage } from './session'

const props = defineProps<{ message: FastPlanMessage }>()

const isUser = computed(() => props.message.role === 'user')
const isSystem = computed(() => props.message.role === 'system')
const isAssistant = computed(() => props.message.role === 'assistant')

const icon = computed(() => {
  if (isSystem.value) {
    if (props.message.isReflection) return Lightbulb
    if (props.message.isError) return AlertCircle
    if (props.message.isThinking) return Loader2
    return Settings
  }
  if (isAssistant.value) {
    if (props.message.isPlan) return FileText
    return Bot
  }
  return User
})

const iconContainerClass = computed(() => {
  if (isSystem.value) {
    if (props.message.isReflection) return 'bg-amber-100'
    if (props.message.isError) return 'bg-red-100'
    if (props.message.isThinking) return 'bg-transparent'
    return 'bg-gray-100'
  }
  if (isAssistant.value) {
    if (props.message.isPlan) return 'bg-emerald-100'
    return 'bg-teal-100'
  }
  return 'bg-gray-200'
})

const iconClass = computed(() => {
  if (isSystem.value) {
    if (props.message.isReflection) return 'text-amber-600'
    if (props.message.isError) return 'text-red-500'
    if (props.message.isThinking) return 'text-gray-800'
    return 'text-gray-500'
  }
  if (isAssistant.value) {
    if (props.message.isPlan) return 'text-emerald-600'
    return 'text-teal-600'
  }
  return 'text-gray-600'
})

const bubbleClass = computed(() => {
  if (isUser.value) return 'bg-white text-gray-900 border border-gray-100 shadow-sm rounded-br-md'
  if (isSystem.value) {
    if (props.message.isReflection) return 'bg-amber-50 text-amber-700 border border-amber-200'
    if (props.message.isError) return 'bg-red-50 text-red-600 border border-red-200'
    if (props.message.isThinking) return 'bg-transparent text-gray-800 p-0 shadow-none border-none'
    return 'bg-gray-200 text-gray-600 text-lg'
  }
  if (props.message.isPlan) return 'bg-emerald-50 text-emerald-800 border border-emerald-200 rounded-bl-md text-lg'
  return 'bg-gray-100 text-gray-800 rounded-bl-md'
})

const formattedTime = computed(() => {
  if (!props.message.timestamp) return ''
  return new Date(props.message.timestamp).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
})
</script>

<template>
  <template v-if="isUser"><span></span></template>
  <template v-else>
    <div :class="['flex gap-3 animate-fade-in', isUser ? 'justify-end' : 'justify-start']">
      <div v-if="!isUser && !message.isThinking" :class="['w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0 transition-all', iconContainerClass]">
        <component :is="icon" :size="16" :class="iconClass" />
      </div>
      <div class="flex flex-col max-w-[80%]" :class="isUser ? 'items-end' : 'items-start'">
        <div v-if="message.isThinking" class="flex flex-col gap-2">
          <div class="flex items-center gap-2">
            <Bot :size="18" class="text-gray-900" />
            <span class="font-bold text-gray-900">FastPlan</span>
            <span class="text-[10px] text-gray-500 border border-gray-200 px-1 rounded bg-gray-50">Pro</span>
          </div>
          <div class="flex items-center gap-1 pl-0">
            <span class="text-gray-500">{{ message.content }}</span>
            <span class="flex gap-0.5 items-end h-4 pb-1">
              <span class="w-1 h-1 bg-gray-400 rounded-full animate-bounce" style="animation-delay:0s"></span>
              <span class="w-1 h-1 bg-gray-400 rounded-full animate-bounce" style="animation-delay:0.15s"></span>
              <span class="w-1 h-1 bg-gray-400 rounded-full animate-bounce" style="animation-delay:0.3s"></span>
            </span>
          </div>
        </div>
        <div v-else :class="['px-4 py-2.5 rounded-2xl leading-relaxed whitespace-pre-line', bubbleClass]">
          {{ message.content }}
        </div>
        <div v-if="!message.isThinking" class="flex items-center gap-2 mt-1 px-1">
          <span v-if="formattedTime" class="text-[10px] text-gray-400">{{ formattedTime }}</span>
          <span v-if="message.duration" class="text-[10px] text-gray-400">· {{ (message.duration / 1000).toFixed(1) }}s</span>
          <span v-if="message.nodeCount" class="text-[10px] text-gray-400">· {{ message.nodeCount }} 步骤</span>
        </div>
      </div>
    </div>
  </template>
</template>

<style scoped>
@keyframes fade-in {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}
.animate-fade-in { animation: fade-in 0.3s ease-out forwards; }
</style>
