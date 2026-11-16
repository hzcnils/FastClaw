import { computed, effectScope, watch, type InjectionKey } from 'vue'
import type { ExportControl } from '@/utils/exportControl'
import type { createChatContextBinding } from './contextBinding'
import type { MessageActionStore } from './messageActions'
import { abortable } from './abortable'

export type MessageExportFormat = 'md' | 'docx' | 'pdf'
export interface MessageExportActions {
  captureExport(id: string, isDisplayed: () => boolean): (format: string) => Promise<void>
}
export const CHAT_MESSAGE_EXPORT: InjectionKey<MessageExportActions> = Symbol('chat-message-export')
interface Dependencies {
  run(content: string, format: MessageExportFormat, control: ExportControl): Promise<boolean>
  loading(): () => void
  report(kind: 'empty' | 'streaming' | 'success' | 'error', format?: MessageExportFormat, error?: unknown): void
}

export function createMessageExportBinding<S extends MessageActionStore>(
  binding: ReturnType<typeof createChatContextBinding<S>>, dependencies: Dependencies,
) {
  const pending = new Map<string, { current(): boolean }>()
  function captureExport(id: string, isDisplayed: () => boolean = () => true) {
    const lease = binding.captureUi()
    const find = () => lease?.context.store.messages.find(message => message.id === id && message.conversation_id === lease.conversationId)
    const visible = () => !!lease?.isCurrent() && isDisplayed()
    return async (format: string) => {
      if (!visible() || !['md', 'docx', 'pdf'].includes(format)) return
      const source = find()
      if (!source) return
      const kind = format as MessageExportFormat, content = source.content
      if (!content.trim()) { dependencies.report('empty'); return }
      if (source.status === 'streaming') { dependencies.report('streaming'); return }
      const controller = new AbortController(), scope = effectScope(true)
      const current = () => visible() && find()?.content === content && find()?.status !== 'streaming'
      const valid = () => !controller.signal.aborted && current()
      const key = JSON.stringify([lease!.context.owner.instanceId, lease!.conversationId, id])
      if (pending.get(key)?.current()) return
      const request = { current: valid }; pending.set(key, request)
      scope.run(() => watch(current, value => { if (!value) controller.abort() }, { flush: 'sync' }))
      let close: (() => void) | undefined
      try {
        if (kind !== 'md') close = dependencies.loading()
        if (!valid()) return
        const saved = await abortable(dependencies.run(content, kind, { isCurrent: valid }), controller.signal)
        if (saved && valid()) dependencies.report('success', kind)
      } catch (error) {
        if (valid()) dependencies.report('error', kind, error)
      } finally {
        try { close?.() }
        finally {
          scope.stop(); controller.abort()
          if (pending.get(key) === request) pending.delete(key)
        }
      }
    }
  }
  const actions = computed(() => {
    const lease = binding.captureUi()
    if (!lease?.isCurrent()) return null
    return { export: (id: string, format: string) => lease.isCurrent() ? captureExport(id)(format) : Promise.resolve() }
  })
  return { actions, captureExport }
}
