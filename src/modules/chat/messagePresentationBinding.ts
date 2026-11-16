import { computed, effectScope, readonly, ref, watch } from 'vue'
import type { ChatAttachment } from '@/types'
import type { createChatContextBinding } from './contextBinding'
import type { MessageActionStore } from './messageActions'
import { browserSpeechPort, chatSpeechPlayback, type SpeechPort } from './speechPlayback'

interface Dependencies {
  scheduleFocus(callback: () => void): void
  focus(): void
  unsupported(): void
  failed(): void
  speech?: () => SpeechPort | null
  playback?: typeof chatSpeechPlayback
}

/** Quotes mutate the captured draft; speech belongs only to the visible presentation. */
export function createMessagePresentationBinding<S extends MessageActionStore>(
  binding: ReturnType<typeof createChatContextBinding<S>>, dependencies: Dependencies,
) {
  const readingMessageId = ref<string | null>(null)
  let stopPlayback: (() => void) | undefined
  let disposed = false
  function stop() { stopPlayback?.(); stopPlayback = undefined }
  const actions = computed(() => {
    const lease = binding.captureUi()
    if (!lease?.isCurrent() || disposed) return null
    const { owner, store } = lease.context
    const current = () => !disposed && lease.isCurrent()
    function quote(content: string) {
      if (!current()) return
      owner.state.quotedText.value = (content || '').replace(/\s+/g, ' ').trim()
      dependencies.scheduleFocus(() => { if (current()) dependencies.focus() })
    }
    function quoteDocument(doc: ChatAttachment) {
      if (!current()) return
      // Read canonical attachment metadata, not a delayed renderer payload.
      const attachment = store.messages.filter(message => message.conversation_id === lease!.conversationId)
        .flatMap(message => message.attachments ?? []).find(item => item.id === doc.id && item.type === 'document' && item.status === 'uploaded')
      if (!attachment) return
      quote(attachment.name)
      const state = owner.state
      if (attachment.uploaded_kb_id && !state.fileKbIds.value.includes(attachment.uploaded_kb_id)) state.fileKbIds.value.push(attachment.uploaded_kb_id)
      if (attachment.uploaded_file_id && !state.conversationFileIds.value.includes(attachment.uploaded_file_id)) state.conversationFileIds.value.push(attachment.uploaded_file_id)
    }
    function tts(id: string, content: string) {
      if (!current()) return
      if (readingMessageId.value === id) { stop(); return }
      const find = () => store.messages.find(message => message.id === id && message.conversation_id === lease!.conversationId)
      if (!find() || find()!.content !== content) return
      const text = content.replace(/[#*`~>]/g, '')
      if (!text.trim()) return
      const scope = effectScope(true)
      let ended = false
      try {
        const port = (dependencies.speech ?? browserSpeechPort)()
        if (!port) { scope.stop(); dependencies.unsupported(); return }
        stop()
        readingMessageId.value = id
        const request = (dependencies.playback ?? chatSpeechPlayback).play(port, text, () => {
          ended = true; scope.stop(); readingMessageId.value = null; stopPlayback = undefined
        })
        if (!ended) {
          if (!current() || find()?.content !== content) { request.stop(); return }
          stopPlayback = request.stop
          scope.run(() => watch(() => [current(), find()?.content], () => {
            if (!current() || find()?.content !== content) request.stop()
          }, { flush: 'sync' }))
        }
      } catch {
        scope.stop(); readingMessageId.value = null; stopPlayback = undefined
        if (current()) dependencies.failed()
      }
    }
    return { quote, quoteDocument, tts,
      documentEvents: (doc: ChatAttachment) => ({ click: (event: Event) => { event.stopPropagation(); quoteDocument(doc) } }),
      clearQuote: () => { if (current()) owner.state.quotedText.value = '' },
    }
  })
  return { actions, readingMessageId: readonly(readingMessageId), stop,
    dispose() { disposed = true; stop() },
  }
}
