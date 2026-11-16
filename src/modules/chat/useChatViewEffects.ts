import { nextTick } from 'vue'

type Cancel = () => void
type IdleWindow = Window & {
  requestIdleCallback?: (callback: () => void, options: { timeout: number }) => number
  cancelIdleCallback?: (handle: number) => void
}

/** 仅管理当前视图的展示资源；不拥有 Tab、消息流、连接或持久化。 */
export function useChatViewEffects(isOwner: () => boolean) {
  let active = false
  let disposed = false
  let generation = 0
  const pending = new Set<Cancel>()
  const cleanups = new Set<Cancel>()
  const isActive = () => active && !disposed && isOwner()

  function guard<T extends unknown[], R>(callback: (...args: T) => R) {
    const token = generation
    const scheduledWhileActive = isActive()
    return (...args: T) => {
      if (scheduledWhileActive && token === generation && isActive()) return callback(...args)
    }
  }

  function schedule(enqueue: (run: () => void) => Cancel, callback: () => void): Cancel {
    if (!isActive()) return () => {}
    let cancelled = false
    let release: Cancel = () => {}
    const cancel = () => {
      if (cancelled) return
      cancelled = true
      pending.delete(cancel)
      release()
    }
    const run = guard(() => {
      if (!cancelled) callback()
    })
    pending.add(cancel)
    release = enqueue(() => {
      pending.delete(cancel)
      run()
    })
    return cancel
  }

  function tick(callback: () => void) {
    return schedule(run => { void nextTick(run); return () => {} }, callback)
  }
  function frame(callback: () => void) {
    return schedule(run => {
      const id = requestAnimationFrame(run)
      return () => cancelAnimationFrame(id)
    }, callback)
  }
  function delay(callback: () => void, ms: number) {
    return schedule(run => {
      const id = setTimeout(run, ms)
      return () => clearTimeout(id)
    }, callback)
  }
  function idle(callback: () => void) {
    const idleWindow = window as IdleWindow
    if (!idleWindow.requestIdleCallback || !idleWindow.cancelIdleCallback) return delay(callback, 0)
    return schedule(run => {
      const id = idleWindow.requestIdleCallback!(run, { timeout: 1000 })
      return () => idleWindow.cancelIdleCallback!(id)
    }, callback)
  }
  /** 可取消的展示重试等待；停用时必须完成 Promise，不能留下悬挂的 async 栈。 */
  function wait(ms: number): Promise<boolean> {
    if (!isActive()) return Promise.resolve(false)
    return new Promise(resolve => {
      let settled = false
      const finish = (elapsed: boolean) => {
        if (settled) return
        settled = true
        pending.delete(cancel)
        resolve(elapsed)
      }
      const id = setTimeout(() => finish(isActive()), ms)
      const cancel = () => { clearTimeout(id); finish(false) }
      pending.add(cancel)
    })
  }
  function own(cleanup: Cancel) {
    if (!isActive()) { cleanup(); return }
    cleanups.add(cleanup)
  }
  function listen<T extends Event>(target: EventTarget, type: string,
    callback: (event: T) => void, options?: boolean | AddEventListenerOptions) {
    if (!isActive()) return
    const listener = guard(callback) as EventListener
    target.addEventListener(type, listener, options)
    own(() => target.removeEventListener(type, listener, options))
  }
  function activate() {
    if (active || disposed || !isOwner()) return false
    generation++
    active = true
    return true
  }
  function deactivate() {
    if (!active) return
    active = false
    generation++
    for (const cancel of pending) cancel()
    for (const cleanup of cleanups) cleanup()
    cleanups.clear()
  }
  function dispose() { deactivate(); disposed = true }

  return { isActive, activate, deactivate, dispose, guard, tick, frame, delay, idle, wait, own, listen }
}
