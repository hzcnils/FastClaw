// FastClaw · 费用追踪状态
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { UsageStats } from '@/types'

export const useCostStore = defineStore('cost', () => {
  const stats = ref<UsageStats | null>(null)
  async function loadStats(period?: string) { stats.value = await invoke('get_usage_stats', { period }) }
  return { stats, loadStats }
})
