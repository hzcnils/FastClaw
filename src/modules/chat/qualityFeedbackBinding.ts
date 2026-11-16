import { computed, effectScope, watch } from 'vue'
import type { Message } from '@/types'
import { abortable } from './abortable'
import { captureChatCommand, type createChatContextBinding } from './contextBinding'
import type { MessageActionStore, MessageActionOptions } from './messageActions'
import type { ChatTabState } from './tabState'

interface Feedback { score: number; content: string }
interface Dependencies {
  getInvoke: MessageActionOptions['getInvoke']
  prompt(): Promise<string>
  report(kind: 'success' | 'error' | 'unavailable'): void
}
const pending = new WeakMap<ChatTabState, Map<string, { current(): boolean; task: Promise<boolean> }>>()
const loading = new WeakMap<object, symbol>()

/** Prompt and preparation are UI-owned; accepted persistence retains its original target. */
export function createQualityFeedbackBinding<S extends MessageActionStore>(
  binding: ReturnType<typeof createChatContextBinding<S>>, dependencies: Dependencies,
) {
  return computed(() => {
    const lease = binding.captureUi()
    if (!lease?.isCurrent()) return null
    function start(id: string, feedback?: Feedback): Promise<boolean> {
      if (!lease!.isCurrent()) return Promise.resolve(false)
      const command = captureChatCommand(lease!.context), { owner, store } = command.context
      const source = store.messages.find(message => message.id === id && message.conversation_id === command.conversationId)
      if (!source || !command.conversationId) return Promise.resolve(false)
      const original = source.content
      const find = (): Message | undefined => command.isCurrent() ? store.messages.find(message =>
        message.id === id && message.conversation_id === command.conversationId && message.content === original) : undefined
      let jobs = pending.get(owner)
      if (!jobs) { jobs = new Map(); pending.set(owner, jobs) }
      const key = JSON.stringify([command.conversationId, id])
      const previous = jobs.get(key)
      if (previous?.current()) return Promise.resolve(false)
      const scope = effectScope(true), controller = new AbortController()
      let accepted = false
      const valid = () => lease!.isCurrent() && !!find() && !controller.signal.aborted
      scope.run(() => watch(() => [lease!.isCurrent(), find()?.content], () => {
        if (!valid()) controller.abort()
      }, { flush: 'sync' }))
      const job = { current: () => accepted ? !!find() : valid(), task: Promise.resolve(false) }
      // Publish the lock before invoking dependencies, including synchronous prompt implementations.
      jobs.set(key, job)
      job.task = Promise.resolve().then(async () => {
        let trace = source.trace_source?.result
        const token = Symbol()
        const clearLoading = () => {
          if (trace && loading.get(trace) === token) { loading.delete(trace); trace.loading = false }
        }
        let removeInvalidation: (() => void) | undefined
        let collecting = !feedback
        try {
          if (!valid()) return false
          const input = feedback ?? { score: 0, content: await abortable(dependencies.prompt(), controller.signal) }
          collecting = false
          const content = input.content.trim(), score = input.score
          if (!valid() || !Number.isFinite(score) || score < 0 || score > 10 || (!content && score === 0)) return false
          trace = find()?.trace_source?.result
          if (trace) { loading.set(trace, token); trace.loading = true }
          removeInvalidation = owner.onInvalidate(clearLoading)
          const invoke = await abortable(dependencies.getInvoke(), controller.signal)
          if (!valid()) return false
          if (!invoke) { dependencies.report('unavailable'); return false }
          accepted = true; scope.stop()
          await invoke('submit_quality_evaluation', { messageId: id, conversationId: command.conversationId,
            score, content, expectedContent: original })
          const currentTrace = find()?.trace_source?.result
          // Do not mark a later edited/replaced form as submitted by this older request.
          if (trace && currentTrace === trace && trace.rate === score && trace.content.trim() === content) trace.isSubmit = true
          if (lease!.isCurrent() && find()) dependencies.report('success')
          return true
        } catch {
          // Prompt rejection is cancellation; persistence errors belong only to this visible target.
          if (!collecting && valid()) dependencies.report('error')
          return false
        } finally {
          removeInvalidation?.(); clearLoading(); scope.stop(); controller.abort()
          if (jobs!.get(key) === job) jobs!.delete(key)
          if (!jobs!.size && pending.get(owner) === jobs) pending.delete(owner)
        }
      })
      return job.task
    }
    return { request: (id: string) => start(id), submit: (id: string, score: number, content: string) => start(id, { score, content }) }
  })
}
