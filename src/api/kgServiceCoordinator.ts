import { ref, watch, type Ref } from 'vue'

export type KgServiceState = 'unknown' | 'checking' | 'starting' | 'ready' | 'unavailable'

export interface KgServiceCoordinator {
  ready: Ref<boolean>
  state: Ref<KgServiceState>
  markState: (state: KgServiceState) => void
  isReadyFresh: () => boolean
  check: (probe: () => Promise<boolean>, force?: boolean) => Promise<boolean>
  ensure: (task: () => Promise<boolean>, force?: boolean) => Promise<boolean>
}

/**
 * FastKG 前端状态协调器。
 *
 * 每个传输通道各自创建实例，但统一复用
 * 状态机、成功 TTL、健康检查单飞和确保就绪单飞语义。
 */
export function createKgServiceCoordinator(readyTtlMs = 30_000): KgServiceCoordinator {
  const ready = ref(false)
  const state = ref<KgServiceState>('unknown')
  let lastReadyAt = 0
  let checkPromise: Promise<boolean> | null = null
  let ensurePromise: Promise<boolean> | null = null

  const markState = (next: KgServiceState) => {
    state.value = next
    if (next === 'ready') {
      ready.value = true
      lastReadyAt = Date.now()
    } else if (next === 'unavailable') {
      ready.value = false
      lastReadyAt = 0
    }
  }

  // 兼容现有调用方直接写 kgServiceReady.value 的行为。
  watch(ready, (value) => {
    if (value) {
      state.value = 'ready'
      lastReadyAt = Date.now()
    } else if (state.value === 'ready') {
      state.value = 'unavailable'
      lastReadyAt = 0
    }
  }, { flush: 'sync' })

  const isReadyFresh = () => (
    ready.value
    && state.value === 'ready'
    && Date.now() - lastReadyAt < readyTtlMs
  )

  const check = (probe: () => Promise<boolean>, force = true): Promise<boolean> => {
    if (!force && isReadyFresh()) return Promise.resolve(true)
    if (checkPromise) return checkPromise

    markState('checking')
    checkPromise = probe()
      .then((ok) => {
        markState(ok ? 'ready' : 'unavailable')
        return ok
      })
      .catch(() => {
        markState('unavailable')
        return false
      })
      .finally(() => {
        checkPromise = null
      })
    return checkPromise
  }

  const ensure = (task: () => Promise<boolean>, force = false): Promise<boolean> => {
    if (!force && isReadyFresh()) return Promise.resolve(true)
    if (ensurePromise) return ensurePromise

    ensurePromise = task()
      .then((ok) => {
        markState(ok ? 'ready' : 'unavailable')
        return ok
      })
      .catch(() => {
        markState('unavailable')
        return false
      })
      .finally(() => {
        ensurePromise = null
      })
    return ensurePromise
  }

  return { ready, state, markState, isReadyFresh, check, ensure }
}
