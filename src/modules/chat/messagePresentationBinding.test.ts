// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { reactive } from 'vue'
import type { Message, ChatAttachment } from '@/types'
import { createChatTabState } from './tabState'
import { createChatContext, createChatContextBinding } from './contextBinding'
import { createMessagePresentationBinding } from './messagePresentationBinding'
import { browserSpeechPort, createSpeechPlayback, type SpeechPort } from './speechPlayback'

const cleanup: (() => void)[] = []
afterEach(() => { cleanup.splice(0).reverse().forEach(fn => fn()); vi.unstubAllGlobals() })
function fixture(playback = createSpeechPlayback()) {
  const owner = createChatTabState(crypto.randomUUID()); cleanup.push(() => owner.release())
  const doc = { id: 'doc', type: 'document', status: 'uploaded', name: 'report.pdf', uploaded_kb_id: 'kb', uploaded_file_id: 'file' } as ChatAttachment
  const store = reactive({ currentConversationId: 'a' as string | null,
    messages: [{ id: 'one', conversation_id: 'a', content: '# answer', attachments: [doc] },
      { id: 'two', conversation_id: 'a', content: 'second' }] as Message[] })
  const context = createChatContext({ instanceId: owner.instanceId, path: '/chat/a' }, owner, store)
  const binding = createChatContextBinding<typeof store>(); binding.activate(context); cleanup.push(binding.dispose)
  const utterances: SpeechSynthesisUtterance[] = [], ticks: (() => void)[] = []
  const port: SpeechPort = { create: vi.fn(text => ({ text, lang: '', onend: null, onerror: null }) as SpeechSynthesisUtterance),
    speak: vi.fn(u => utterances.push(u)), cancel: vi.fn() }
  const dependencies = { playback, speech: vi.fn(() => port as SpeechPort | null), scheduleFocus: (fn: () => void) => ticks.push(fn),
    focus: vi.fn(), unsupported: vi.fn(), failed: vi.fn() }
  const api = createMessagePresentationBinding(binding, dependencies); cleanup.push(api.dispose)
  return { owner, store, binding, context, doc, port, dependencies, api, ticks, utterances }
}
describe('captured quote actions', () => {
  it('normalizes text, focuses after the tick, and retains the quote on deactivation', () => {
    const f = fixture(); f.api.actions.value!.quote('  first\n second  ')
    expect(f.owner.state.quotedText.value).toBe('first second'); expect(f.dependencies.focus).not.toHaveBeenCalled()
    f.ticks[0](); expect(f.dependencies.focus).toHaveBeenCalledOnce()
    f.binding.deactivate(); expect(f.owner.state.quotedText.value).toBe('first second')
  })
  it('does not focus a newer presentation or revive old quote/clear/document callbacks after return', () => {
    const f = fixture(), old = f.api.actions.value!
    old.quote('saved'); f.binding.deactivate(); f.binding.activate(f.context)
    f.ticks[0](); old.quote('stale'); old.clearQuote(); old.quoteDocument(f.doc)
    expect(f.owner.state.quotedText.value).toBe('saved'); expect(f.dependencies.focus).not.toHaveBeenCalled()
    expect(f.owner.state.fileKbIds.value).toEqual([])
    f.api.actions.value!.clearQuote(); expect(f.owner.state.quotedText.value).toBe('')
  })
  it('uses canonical uploaded attachment metadata and deduplicates retrieval references', () => {
    const f = fixture(), action = f.api.actions.value!
    action.quoteDocument({ ...f.doc, name: 'stale', uploaded_file_id: 'wrong' }); action.quoteDocument(f.doc)
    expect(f.owner.state.quotedText.value).toBe('report.pdf')
    expect(f.owner.state.fileKbIds.value).toEqual(['kb']); expect(f.owner.state.conversationFileIds.value).toEqual(['file'])
  })
  it('captures native document clicks at render time rather than resolving a new handler after switching', () => {
    const f = fixture(), events = f.api.actions.value!.documentEvents(f.doc), event = { stopPropagation: vi.fn() } as unknown as Event
    f.binding.deactivate(); f.binding.activate(f.context); events.click(event)
    expect(f.owner.state.quotedText.value).toBe(''); expect(event.stopPropagation).toHaveBeenCalledOnce()
    f.api.actions.value!.documentEvents(f.doc).click(event)
    expect(f.owner.state.quotedText.value).toBe('report.pdf')
  })
  it.each(['missing', 'conversation', 'uploading'] as const)('rejects %s document references', change => {
    const f = fixture()
    if (change === 'missing') f.store.messages[0].attachments = []
    if (change === 'conversation') f.store.messages[0].conversation_id = 'other'
    if (change === 'uploading') f.store.messages[0].attachments![0].status = 'uploading'
    f.api.actions.value!.quoteDocument(f.doc)
    expect(f.owner.state.quotedText.value).toBe(''); expect(f.owner.state.fileKbIds.value).toEqual([])
  })
  it('binds quotes to the newly activated owner without allowing prior callbacks to mutate either', () => {
    const a = fixture(), b = fixture(), old = a.api.actions.value!
    a.binding.activate(b.context); a.api.actions.value!.quote('b'); old.quote('a')
    expect(a.owner.state.quotedText.value).toBe(''); expect(b.owner.state.quotedText.value).toBe('b')
  })
})

describe('presentation-owned speech', () => {
  it('cleans markdown, sets Chinese locale, toggles stop, and does not cancel an unowned idle engine', () => {
    const f = fixture(); f.api.stop(); expect(f.port.cancel).not.toHaveBeenCalled()
    f.api.actions.value!.tts('one', '# answer')
    expect(f.utterances[0]).toMatchObject({ text: ' answer', lang: 'zh-CN' }); expect(f.api.readingMessageId.value).toBe('one')
    f.api.actions.value!.tts('one', '# answer')
    expect(f.port.cancel).toHaveBeenCalledOnce(); expect(f.api.readingMessageId.value).toBeNull()
    expect(f.utterances[0].onend).toBeNull(); f.api.stop(); expect(f.port.cancel).toHaveBeenCalledOnce()
  })
  it.each(['end', 'error'] as const)('releases natural %s without cancelling unrelated engine work', event => {
    const f = fixture(); f.api.actions.value!.tts('one', '# answer')
    const callback = event === 'end' ? f.utterances[0].onend : f.utterances[0].onerror
    callback!.call(f.utterances[0], {} as any)
    expect(f.api.readingMessageId.value).toBeNull(); expect(f.port.cancel).not.toHaveBeenCalled()
    f.binding.deactivate(); expect(f.port.cancel).not.toHaveBeenCalled()
  })
  it('ignores late callbacks from an earlier playback of the same message', () => {
    const f = fixture(); f.api.actions.value!.tts('one', '# answer')
    const late = f.utterances[0].onend!
    f.api.stop(); f.api.actions.value!.tts('one', '# answer'); late.call(f.utterances[0], {} as any)
    expect(f.api.readingMessageId.value).toBe('one'); expect(f.utterances[1].onend).not.toBeNull()
  })
  it('replaces the global chat slot without letting an old Tab cleanup cancel the new playback', () => {
    const broker = createSpeechPlayback(), a = fixture(broker), b = fixture(broker)
    a.api.actions.value!.tts('one', '# answer'); const late = a.utterances[0].onerror!
    b.api.actions.value!.tts('one', '# answer')
    expect(a.port.cancel).toHaveBeenCalledOnce(); expect(a.api.readingMessageId.value).toBeNull()
    a.binding.deactivate(); a.api.stop(); a.api.dispose(); late.call(a.utterances[0], {} as any)
    expect(b.port.cancel).not.toHaveBeenCalled(); expect(b.api.readingMessageId.value).toBe('one')
  })
  it.each(['deactivate', 'reset', 'release', 'conversation', 'remove', 'content', 'revert', 'dispose'] as const)(
    'stops promptly on %s without resuming automatically', change => {
      const f = fixture(); f.api.actions.value!.tts('one', '# answer')
      if (change === 'deactivate') f.binding.deactivate()
      if (change === 'reset') f.owner.invalidateContext()
      if (change === 'release') f.owner.release()
      if (change === 'conversation') f.store.currentConversationId = 'b'
      if (change === 'remove') f.store.messages = []
      if (change === 'content' || change === 'revert') f.store.messages[0].content = 'new'
      if (change === 'revert') f.store.messages[0].content = '# answer'
      if (change === 'dispose') f.api.dispose()
      expect(f.api.readingMessageId.value).toBeNull(); expect(f.port.cancel).toHaveBeenCalledOnce()
      f.binding.activate(f.context); expect(f.port.speak).toHaveBeenCalledOnce()
    })
  it('rejects obsolete callbacks and stale/missing/empty message text', () => {
    const f = fixture(), old = f.api.actions.value!
    f.binding.deactivate(); f.binding.activate(f.context); old.tts('one', '# answer')
    f.api.actions.value!.tts('one', 'outdated'); f.api.actions.value!.tts('missing', 'text')
    f.store.messages[0].content = '# * '; f.api.actions.value!.tts('one', '# * ')
    expect(f.port.speak).not.toHaveBeenCalled()
  })
  it('does not stop speech on unrelated message or draft edits', () => {
    const f = fixture(); f.api.actions.value!.tts('one', '# answer')
    f.owner.state.messageStr.value = 'draft'; f.store.messages[1].content = 'changed'
    expect(f.port.cancel).not.toHaveBeenCalled(); expect(f.api.readingMessageId.value).toBe('one')
  })
  it('reports missing synthesis without retaining reading state', () => {
    const f = fixture(); f.dependencies.speech.mockReturnValue(null); f.api.actions.value!.tts('one', '# answer')
    expect(f.dependencies.unsupported).toHaveBeenCalledOnce(); expect(f.api.readingMessageId.value).toBeNull()
  })
  it.each(['create', 'speak'] as const)('handles native %s exceptions and permits retry', method => {
    const f = fixture(); vi.mocked(f.port[method]).mockImplementationOnce(() => { throw new Error('native') })
    f.api.actions.value!.tts('one', '# answer')
    expect(f.api.readingMessageId.value).toBeNull(); expect(f.dependencies.failed).toHaveBeenCalledOnce()
    f.api.actions.value!.tts('one', '# answer'); expect(f.api.readingMessageId.value).toBe('one')
  })
  it('releases ownership even when native cancel throws', () => {
    const f = fixture(); vi.mocked(f.port.cancel).mockImplementation(() => { throw new Error('cancel') })
    f.api.actions.value!.tts('one', '# answer'); expect(() => f.binding.deactivate()).not.toThrow()
    expect(f.api.readingMessageId.value).toBeNull()
  })
  it('handles synchronous completion and deactivation inside speak without retaining a stale watch', () => {
    const f = fixture()
    vi.mocked(f.port.speak).mockImplementationOnce(u => u.onend!.call(u, {} as any))
    f.api.actions.value!.tts('one', '# answer'); expect(f.api.readingMessageId.value).toBeNull()
    vi.mocked(f.port.speak).mockImplementationOnce(() => f.binding.deactivate())
    f.api.actions.value!.tts('one', '# answer'); expect(f.api.readingMessageId.value).toBeNull()
    expect(f.port.cancel).toHaveBeenCalledOnce()
  })
  it('adapts the real browser API shape and detects a missing constructor', () => {
    const synthesis = { speak: vi.fn(), cancel: vi.fn() }
    vi.stubGlobal('speechSynthesis', synthesis)
    vi.stubGlobal('SpeechSynthesisUtterance', class { constructor(public text: string) {} })
    const port = browserSpeechPort()!, utterance = port.create('test'); port.speak(utterance); port.cancel()
    expect(synthesis.speak).toHaveBeenCalledWith(utterance); expect(synthesis.cancel).toHaveBeenCalledOnce()
    vi.stubGlobal('SpeechSynthesisUtterance', undefined); expect(browserSpeechPort()).toBeNull()
  })
})
