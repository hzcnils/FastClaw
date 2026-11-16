<script setup>
import { Check, Zap } from 'lucide-vue-next'

defineProps({
  confirmedHistory: { type: Array, default: () => [] },
})

const formatTime = (date) =>
  new Date(date).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
</script>

<template>
  <div v-if="confirmedHistory.length > 0" class="fpp-ui space-y-1 my-2">
    <div
      v-for="item in confirmedHistory"
      :key="item.id"
      class="flex items-center gap-2 text-xs text-gray-500 px-3 py-1.5 bg-gray-50/80 rounded-lg border border-gray-100"
    >
      <Check :size="12" class="text-green-500 flex-shrink-0" />
      <span class="font-mono text-gray-400 flex-shrink-0 text-[11px]">{{ formatTime(item.timestamp) }}</span>
      <span v-if="item.kind === 'checkpoint_input'" class="truncate">
        <strong class="text-gray-700">{{ item.title || '已补充执行信息' }}</strong>
        <template v-if="item.detail">：{{ item.detail }}</template>
      </span>
      <span v-else-if="item.kind === 'checkpoint_decision'" class="truncate">
        <strong class="text-gray-700">{{ item.title || '已完成人工确认' }}</strong>
        <template v-if="item.detail">：{{ item.detail }}</template>
      </span>
      <span v-else class="truncate">
        已确认 <strong class="text-gray-700">{{ item.completedNodeName }}</strong> 继续执行
      </span>
      <span
        v-if="item.hadEdit"
        class="flex-shrink-0 text-[10px] bg-blue-50 text-blue-600 border border-blue-100 px-1.5 py-0.5 rounded-full"
      >
        <Zap :size="8" class="inline" /> 已修改
      </span>
    </div>
  </div>
</template>
