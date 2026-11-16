/** Cancellation of a local wait does not imply rollback of a server-side request. */
export function throwIfFastPlanAborted(signal?: AbortSignal | null) {
  if (signal?.aborted) throw signal.reason || new DOMException('FastPlan operation cancelled', 'AbortError')
}

export function waitForFastPlanOperation<T>(work: Promise<T>, signal?: AbortSignal | null): Promise<T> {
  if (!signal) return work
  return new Promise<T>((resolve, reject) => {
    const abort = () => reject(signal.reason || new DOMException('FastPlan operation cancelled', 'AbortError'))
    const cleanup = () => signal.removeEventListener('abort', abort)
    // Always handle the original promise, including failures arriving after cancellation.
    work.then(value => { cleanup(); resolve(value) }, error => { cleanup(); reject(error) })
    if (signal.aborted) abort()
    else signal.addEventListener('abort', abort, { once: true })
  })
}
