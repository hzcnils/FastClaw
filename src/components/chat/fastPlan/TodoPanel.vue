<script setup lang="ts">
import { ref, computed } from 'vue'
import { X, Plus, Check, Trash2, ListTodo } from 'lucide-vue-next'
import type { FastPlanTodo } from './session'

const props = defineProps<{ todos: FastPlanTodo[] }>()
const emit = defineEmits<{
  'add': [content: string]
  'toggle': [id: string]
  'delete': [id: string]
  'close': []
}>()

const newTodoText = ref('')
const pendingTodos = computed(() => props.todos.filter(t => !t.completed))
const completedTodos = computed(() => props.todos.filter(t => t.completed))

function handleAdd() {
  const text = newTodoText.value.trim()
  if (!text) return
  emit('add', text)
  newTodoText.value = ''
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); handleAdd() }
}
</script>

<template>
  <div class="w-80 border-l border-gray-200 bg-gray-50/50 flex flex-col h-full flex-shrink-0">
    <div class="h-14 flex items-center justify-between px-4 border-b border-gray-200 bg-white">
      <div class="flex items-center gap-2">
        <ListTodo :size="18" class="text-teal-600" />
        <span class="font-semibold text-gray-900">TODO</span>
        <span v-if="pendingTodos.length > 0" class="text-sm text-gray-400">{{ pendingTodos.length }} 项待办</span>
      </div>
      <button @click="emit('close')" class="p-1.5 hover:bg-gray-100 rounded-lg transition-colors">
        <X :size="16" class="text-gray-400" />
      </button>
    </div>

    <div class="p-3 border-b border-gray-100 bg-white">
      <div class="flex items-center gap-2">
        <input v-model="newTodoText" @keydown="handleKeydown" placeholder="添加待办项..."
          class="flex-1 border border-gray-200 rounded-lg px-3 py-2 outline-none focus:ring-2 focus:ring-teal-500/30 focus:border-teal-300 transition-all" />
        <button @click="handleAdd" :disabled="!newTodoText.trim()"
          class="p-2 bg-teal-600 text-white rounded-lg hover:bg-teal-700 disabled:opacity-50 disabled:cursor-not-allowed transition-all">
          <Plus :size="16" />
        </button>
      </div>
    </div>

    <div class="flex-1 overflow-y-auto p-3">
      <div v-if="pendingTodos.length > 0" class="space-y-1.5 mb-4">
        <div v-for="todo in pendingTodos" :key="todo.id"
          class="flex items-start gap-2.5 p-2.5 bg-white rounded-lg border border-gray-100 hover:border-gray-200 group transition-all">
          <button @click="emit('toggle', todo.id)"
            class="w-4 h-4 mt-0.5 rounded border-2 border-gray-300 hover:border-teal-500 flex-shrink-0 transition-colors flex items-center justify-center">
          </button>
          <span class="flex-1 text-gray-700 leading-relaxed">{{ todo.content }}</span>
          <button @click="emit('delete', todo.id)"
            class="p-1 text-gray-300 hover:text-red-500 opacity-0 group-hover:opacity-100 transition-all flex-shrink-0">
            <Trash2 :size="14" />
          </button>
        </div>
      </div>

      <div v-if="completedTodos.length > 0">
        <div class="text-[10px] font-bold text-gray-400 mb-2 uppercase tracking-wider">
          已完成 ({{ completedTodos.length }})
        </div>
        <div class="space-y-1">
          <div v-for="todo in completedTodos" :key="todo.id" class="flex items-start gap-2.5 p-2 rounded-lg group">
            <button @click="emit('toggle', todo.id)"
              class="w-4 h-4 mt-0.5 rounded bg-teal-500 border-2 border-teal-500 flex-shrink-0 flex items-center justify-center">
              <Check :size="10" class="text-white" />
            </button>
            <span class="flex-1 text-gray-400 line-through">{{ todo.content }}</span>
            <button @click="emit('delete', todo.id)"
              class="p-1 text-gray-300 hover:text-red-500 opacity-0 group-hover:opacity-100 transition-all flex-shrink-0">
              <Trash2 :size="14" />
            </button>
          </div>
        </div>
      </div>

      <div v-if="todos.length === 0" class="text-center py-12 text-gray-400">
        <ListTodo :size="32" class="mx-auto mb-3 opacity-50" />
        <p>暂无待办事项</p>
        <p class="text-sm mt-1">添加待办项来追踪任务进度</p>
      </div>
    </div>
  </div>
</template>
