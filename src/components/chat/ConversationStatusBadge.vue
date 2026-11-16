<script setup lang="ts">
import { computed } from 'vue'
import { Loading } from '@element-plus/icons-vue'
import { useI18n } from 'vue-i18n'
import type { ExecutionPhase } from '@/stores/executionActivity'

const props = withDefaults(defineProps<{
  streaming?: boolean
  unread: boolean
  status?: ExecutionPhase
}>(), {
  streaming: false,
  status: undefined,
})
const { t } = useI18n()
const activeStatus = computed(() => {
  const status = props.status
  if (status && !['idle', 'completed', 'failed', 'cancelled'].includes(status)) return status
  return props.streaming ? 'executing' : undefined
})
const spinning = computed(() => ['submitting', 'planning', 'executing', 'reconnecting'].includes(activeStatus.value || ''))
const statusLabel = computed(() => {
  const labels: Partial<Record<ExecutionPhase, string>> = {
    submitting: '正在提交',
    planning: '正在规划任务',
    executing: '正在执行',
    awaiting_plan_confirmation: '等待确认规划',
    awaiting_step_confirmation: '等待继续下一步',
    awaiting_human: '等待人工输入',
    paused: '任务已暂停',
    reconnecting: '连接中断，正在恢复',
  }
  return activeStatus.value ? labels[activeStatus.value] : undefined
})
</script>

<template>
  <span v-if="activeStatus || unread" class="conversation-status"
    :class="{ 'is-waiting': activeStatus && !spinning }" role="status"
    :title="statusLabel || t('chat.historyUnreadCompletion')"
    :aria-label="statusLabel || t('chat.historyUnreadCompletion')">
    <Loading v-if="spinning" class="conversation-status-spinner" aria-hidden="true" />
    <span v-else class="conversation-status-dot" aria-hidden="true" />
  </span>
</template>

<style scoped>
.conversation-status {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  flex-shrink: 0;
  color: #409eff;
}
.conversation-status-spinner {
  width: 14px;
  height: 14px;
  animation: conversation-status-spin 1s linear infinite;
}
.conversation-status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #409eff;
}
.conversation-status.is-waiting { color: #e6a23c; }
.conversation-status.is-waiting .conversation-status-dot { background: #e6a23c; }
@keyframes conversation-status-spin {
  to { transform: rotate(360deg); }
}
@media (prefers-reduced-motion: reduce) {
  .conversation-status-spinner { animation: none; }
}
</style>
