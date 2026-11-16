import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { fastClawFastPlanAccess } from '@/integrations/chat-plan/fastplanAccess'

export type FastPlanServiceStatus = 'idle' | 'checking' | 'ready' | 'failed'

const READY_TTL_MS = 5_000
const READY_TIMEOUT_MS = 8_000
const READY_RETRY_MS = 750

export const useFastPlanServiceStatusStore = defineStore('fastPlanServiceStatus', () => {
  const status = ref<FastPlanServiceStatus>('idle')
  const error = ref<string | null>(null)
  const lastCheckedAt = ref(0)
  let inFlight: Promise<boolean> | null = null

  const presentationStatus = computed<'connecting' | 'connected' | 'disconnected'>(() => {
    if (status.value === 'ready') return 'connected'
    if (status.value === 'checking') return 'connecting'
    return 'disconnected'
  })

  function markReady() {
    status.value = 'ready'
    error.value = null
    lastCheckedAt.value = Date.now()
  }

  function markFailed(reason: unknown) {
    status.value = 'failed'
    error.value = reason instanceof Error ? reason.message : String(reason || 'FastPlan 服务连接失败')
    lastCheckedAt.value = Date.now()
  }

  function invalidate() {
    status.value = 'idle'
    error.value = null
    lastCheckedAt.value = 0
  }

  async function ensureReady(options: {
    force?: boolean
    timeoutMs?: number
    probe?: () => Promise<unknown>
  } = {}): Promise<boolean> {
    if (!options.force && status.value === 'ready' && Date.now() - lastCheckedAt.value < READY_TTL_MS) {
      return true
    }
    if (inFlight) return inFlight

    status.value = 'checking'
    error.value = null
    const probe = options.probe || fastClawFastPlanAccess.getConnection
    const timeoutMs = options.timeoutMs ?? READY_TIMEOUT_MS
    inFlight = (async () => {
      let timer: ReturnType<typeof setTimeout> | null = null
      let cancelled = false
      try {
        const retryUntilReady = async () => {
          let lastError: unknown = new Error('FastPlan 服务尚未就绪')
          while (!cancelled) {
            try {
              return await probe()
            } catch (reason) {
              lastError = reason
              await new Promise(resolve => setTimeout(resolve, Math.min(READY_RETRY_MS, timeoutMs)))
            }
          }
          throw lastError
        }
        await Promise.race([
          retryUntilReady(),
          new Promise<never>((_, reject) => {
            timer = setTimeout(() => reject(new Error('FastPlan 服务连接检查超时')), timeoutMs)
          }),
        ])
        markReady()
        return true
      } catch (reason) {
        markFailed(reason)
        return false
      } finally {
        cancelled = true
        if (timer) clearTimeout(timer)
        inFlight = null
      }
    })()
    return inFlight
  }

  return {
    status,
    error,
    lastCheckedAt,
    presentationStatus,
    ensureReady,
    markReady,
    markFailed,
    invalidate,
  }
})
