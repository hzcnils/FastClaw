<script setup lang="ts">
import { ref, watch, computed, onUnmounted } from 'vue'
import { UserCheck, Clock, Calendar, DollarSign, MessageSquare, X, CheckCircle2 } from 'lucide-vue-next'
import type { HumanRequest } from './session'

const props = defineProps<{ request: HumanRequest }>()
const emit = defineEmits<{ 'response': [data: Record<string, unknown>] }>()

const inputValue = ref('')
const timeLeft = ref(0)
let timer: ReturnType<typeof setInterval> | null = null

interface NormalizedOption {
  label: string
  value: string
  isDefault?: boolean
}

const normalizedOptions = computed<NormalizedOption[]>(() => {
  if (!props.request?.options) return []
  return props.request.options.map(opt => {
    if (typeof opt === 'string') return { label: opt, value: opt }
    return { label: opt.label || opt.value, value: opt.value, isDefault: opt.is_default }
  })
})

const inputType = computed(() => {
  if (!props.request) return 'none'
  const p = (props.request.prompt || '').toLowerCase()
  if (p.includes('日期') || p.includes('date') || p.includes('时间') || p.includes('when')) return 'date'
  if (p.includes('预算') || p.includes('budget') || p.includes('多少钱') || p.includes('金额') || p.includes('price') || p.includes('人数')) return 'number'
  const isControlOption = normalizedOptions.value.some(o => o.value === 'continue' || o.value === 'abort')
  if (normalizedOptions.value.length > 0 && !isControlOption && !p.includes('确认') && !p.includes('输入')) return 'options'
  return 'text'
})

const placeholder = computed(() => {
  switch (inputType.value) {
    case 'date': return '请选择日期...'
    case 'number': return '请输入数字/金额...'
    default: return '请输入您的回复...'
  }
})

const inputIcon = computed(() => {
  switch (inputType.value) {
    case 'date': return Calendar
    case 'number': return DollarSign
    default: return MessageSquare
  }
})

const progressPercent = computed(() => {
  if (!props.request) return 0
  const total = props.request.timeout_seconds || 60
  return Math.min(100, Math.max(0, (timeLeft.value / total) * 100))
})

function stopTimer() {
  if (timer) { clearInterval(timer); timer = null }
}

function startTimer() {
  stopTimer()
  timer = setInterval(() => {
    if (timeLeft.value > 0) {
      timeLeft.value--
    } else {
      handleSubmit({ action: 'continue', timeout: true })
    }
  }, 1000)
}

watch(() => props.request, (newVal) => {
  stopTimer()
  if (newVal) {
    inputValue.value = ''
    timeLeft.value = newVal.timeout_seconds || 60
    startTimer()
  }
}, { immediate: true, deep: true })

onUnmounted(() => stopTimer())

function handleSubmit(val?: unknown) {
  stopTimer()
  let responseData: Record<string, unknown> = {}
  if (val && typeof val === 'object' && (val as any).action) {
    responseData = val as Record<string, unknown>
  } else if (val && typeof val === 'object' && (val as any).value) {
    responseData = { action: (val as any).value, value: (val as any).label }
  } else if (typeof val === 'string') {
    responseData = { action: val, value: val }
  } else {
    const defaultAction = normalizedOptions.value.find(o => o.isDefault)?.value || 'continue'
    responseData = { action: defaultAction, value: inputValue.value }
  }
  emit('response', responseData)
}

function handleCancel() {
  const cancelAction = normalizedOptions.value.find(o => o.value === 'abort' || o.value === 'cancel')?.value || 'abort'
  handleSubmit({ action: cancelAction, value: '已取消' })
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey && inputType.value !== 'options' && inputValue.value.trim()) {
    e.preventDefault()
    handleSubmit()
  }
}
</script>

<template>
  <transition enter-active-class="transition-all duration-300 ease-out" leave-active-class="transition-all duration-200 ease-in"
    enter-from-class="opacity-0 translate-y-4" enter-to-class="opacity-100 translate-y-0"
    leave-from-class="opacity-100 translate-y-0" leave-to-class="opacity-0 translate-y-4">
    <div v-if="request" class="bg-blue-50 border-t border-blue-100 p-5 w-full relative overflow-hidden">
      <button @click="handleCancel" class="absolute top-3 right-3 p-1.5 rounded-full hover:bg-blue-100 text-blue-400 hover:text-red-500 transition-colors" title="取消任务">
        <X :size="16" />
      </button>
      <div class="flex gap-4 items-start">
        <div class="w-8 h-8 rounded-lg bg-blue-500 flex items-center justify-center text-white flex-shrink-0 mt-1 shadow-sm">
          <UserCheck :size="18" />
        </div>
        <div class="flex-1 min-w-0">
          <div class="mb-3">
            <h4 class="font-bold text-gray-900 mb-1">需要您的确认</h4>
            <p class="text-gray-700 leading-relaxed">{{ request.prompt }}</p>
          </div>
          <div>
            <div v-if="inputType === 'options'" class="flex flex-wrap gap-2">
              <button v-for="(option, idx) in normalizedOptions" :key="idx" @click="handleSubmit(option)"
                class="px-4 py-2 bg-white border border-blue-200 hover:border-blue-500 hover:bg-blue-50 text-gray-700 hover:text-blue-700 font-medium rounded-lg transition-all shadow-sm active:scale-95 flex items-center gap-2">
                {{ option.label }}
              </button>
            </div>
            <div v-else class="flex flex-col gap-2 max-w-xl">
              <div class="flex flex-wrap gap-2 mb-1">
                <button v-for="quickReply in ['确认无误', '继续执行', '需修改']" :key="quickReply"
                  @click="inputValue = quickReply; handleSubmit()"
                  class="px-3 py-1.5 bg-white border border-blue-200 hover:border-blue-400 hover:bg-blue-50 text-blue-600 hover:text-blue-700 text-sm rounded-full transition-all shadow-sm active:scale-95">
                  {{ quickReply }}
                </button>
              </div>
              <div class="flex gap-2">
                <input :type="inputType === 'date' ? 'date' : (inputType === 'number' ? 'number' : 'text')"
                  v-model="inputValue" @keydown="handleKeydown" :placeholder="placeholder"
                  class="flex-1 px-3 py-2 bg-white border border-blue-200 rounded-lg focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500 outline-none transition-all"
                  autofocus />
                <button @click="handleSubmit()" :disabled="!inputValue"
                  class="bg-blue-600 text-white px-4 py-2 rounded-lg hover:bg-blue-700 disabled:opacity-50 font-medium transition-colors shadow-sm">
                  确认
                </button>
              </div>
            </div>
            <div class="flex items-center gap-2 mt-2 text-[10px] text-gray-400 font-mono">
              <Clock :size="10" />
              <span>{{ timeLeft }}s 自动确认</span>
              <span class="text-gray-300">|</span>
              <span>ID: {{ request.node_id }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </transition>
</template>

<style scoped>
input[type="number"]::-webkit-inner-spin-button,
input[type="number"]::-webkit-outer-spin-button {
  -webkit-appearance: none;
  margin: 0;
}
</style>
