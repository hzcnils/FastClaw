import type { ChatContextStore, createChatContextBinding } from './contextBinding'

/** Each native file dialog has its own element and captured UI lifetime, never the latest listener. */
export function createContextFilePicker<S extends ChatContextStore>(binding: ReturnType<typeof createChatContextBinding<S>>) {
  let pending: (() => void) | undefined
  let disposed = false
  function cancel() { pending?.() }
  function open(options: { accept: string; multiple: boolean }, select: (files: File[]) => void) {
    if (disposed) return
    const lease = binding.captureUi()
    if (!lease?.isCurrent()) return
    cancel()
    const input = document.createElement('input')
    input.type = 'file'
    input.accept = options.accept
    input.multiple = options.multiple
    input.hidden = true
    let finished = false
    const cleanup = () => {
      if (finished) return
      finished = true
      input.removeEventListener('change', onChange)
      input.removeEventListener('cancel', cleanup)
      input.remove()
      if (pending === cleanup) pending = undefined
    }
    const onChange = () => {
      const files = Array.from(input.files || [])
      cleanup()
      if (files.length && lease.isCurrent()) select(files)
    }
    pending = cleanup
    input.addEventListener('change', onChange)
    input.addEventListener('cancel', cleanup)
    document.body.append(input)
    try { input.click() } catch (error) { cleanup(); throw error }
  }
  function dispose() { disposed = true; cancel() }
  return { open, cancel, dispose }
}
