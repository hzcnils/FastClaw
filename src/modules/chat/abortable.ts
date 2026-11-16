/** Observe cancellation promptly without leaving a late rejection unhandled. */
export function abortable<T>(operation: Promise<T>, signal: AbortSignal): Promise<T> {
  return new Promise((resolve, reject) => {
    const abort = () => reject(new DOMException('发送已取消', 'AbortError'))
    const cleanup = () => signal.removeEventListener('abort', abort)
    operation.then(value => { cleanup(); resolve(value) }, error => { cleanup(); reject(error) })
    if (signal.aborted) abort()
    else signal.addEventListener('abort', abort, { once: true })
  })
}
