import { listen } from '@tauri-apps/api/event'
import type { ScreenshotReply } from '@/types/screenshot'

interface ScreenshotRequest {
  confirm: (dataUrl: string) => void | Promise<void>
  /** Always runs, including after the originating view is hidden/closed. */
  cleanup: () => void | Promise<void>
}
let active: { id: string; finish: (dataUrl?: string) => Promise<void> } | null = null
let registration: Promise<boolean> | null = null
let unlisteners: Array<() => void | Promise<void>> = []
let lifecycleGeneration = 0
let disposing = false

function receive(payload: unknown, confirm: boolean) {
  if (!payload || typeof payload !== 'object') return
  const reply = payload as Partial<ScreenshotReply>
  const request = active
  if (!request || typeof reply.requestId !== 'string' || reply.requestId !== request.id) return
  if (confirm && typeof reply.dataUrl !== 'string') return
  void request.finish(confirm ? reply.dataUrl : undefined).catch(() => {})
}

async function ensureListeners(): Promise<boolean> {
  if (unlisteners.length === 2) return true
  if (registration) return registration
  const generation = lifecycleGeneration
  registration = (async () => {
    const pending: Array<() => void | Promise<void>> = []
    try {
      pending.push(await listen('screenshot-confirm', event => receive(event.payload, true)))
      pending.push(await listen('screenshot-cancel', event => receive(event.payload, false)))
      if (generation !== lifecycleGeneration) {
        await Promise.allSettled(pending.map(fn => Promise.resolve().then(fn)))
        return false
      }
      unlisteners = pending
      return true
    } catch {
      await Promise.allSettled(pending.map(fn => Promise.resolve().then(fn)))
      return false
    }
  })().finally(() => { registration = null })
  return registration
}

/** One native capture owns shared backend pixels/window until terminal cleanup finishes. */
export function beginChatScreenshotRequest(callbacks: ScreenshotRequest) {
  if (active || disposing) return null
  const id = crypto.randomUUID()
  let completion: Promise<void> | null = null
  const finish = (dataUrl?: string): Promise<void> => {
    if (completion) return completion
    completion = Promise.resolve().then(async () => {
      try { if (dataUrl !== undefined) await callbacks.confirm(dataUrl) }
      finally {
        try { await callbacks.cleanup() }
        finally { if (active?.id === id) active = null }
      }
    })
    return completion
  }
  active = { id, finish }
  return { id, isPending: () => active?.id === id && !completion,
    ready: ensureListeners, confirm: (dataUrl: string) => finish(dataUrl), cancel: () => finish() }
}

/** Only application-root teardown removes shared listeners; tab changes do not retarget requests. */
export async function disposeChatScreenshotEventBridge() {
  disposing = true
  lifecycleGeneration++
  try {
    await registration
    await active?.finish().catch(() => {})
    const pending = unlisteners.splice(0)
    await Promise.allSettled(pending.map(fn => Promise.resolve().then(fn)))
  } finally { disposing = false }
}
