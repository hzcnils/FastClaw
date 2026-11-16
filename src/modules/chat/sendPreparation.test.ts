import { afterEach, describe, expect, it } from 'vitest'
import { reactive } from 'vue'
import { createChatContext, createChatContextBinding } from './contextBinding'
import { createChatTabState } from './tabState'
import { captureSendPreparation } from './sendPreparation'

const cleanups: (() => void)[] = []
afterEach(() => cleanups.splice(0).reverse().forEach(fn => fn()))
function fixture() {
  const owner = createChatTabState('preparation'), store = reactive({ currentConversationId: 'conversation' as string | null })
  const context = createChatContext({ path: '/chat/preparation', instanceId: 'preparation' }, owner, store)
  const binding = createChatContextBinding<typeof store>(); binding.activate(context)
  owner.state.messageStr.value = 'question'; owner.state.quotedText.value = 'quote'
  owner.state.fileList.value = [{ id: 'file', type: 'document', name: 'file', status: 'uploading' }]
  const options = reactive({ systemPrompt: 'role' })
  const intent = captureSendPreparation(binding.captureUi()!, () => options.systemPrompt)
  cleanups.push(() => { intent.dispose(); binding.dispose(); owner.release() })
  return { owner, store, context, binding, intent, options }
}

describe('ordinary send preparation lifetime', () => {
  it('does not invalidate when upload status/progress/IDs are committed', () => {
    const f = fixture(), attachment = f.owner.state.fileList.value[0]
    attachment.status = 'uploaded'; attachment.uploaded_file_id = 'backend-file'
    expect(f.intent.isCurrent()).toBe(true)
  })
  it.each(['draft', 'quote', 'files', 'model', 'role', 'owner', 'conversation', 'switch'] as const)(
    'latches invalidation for %s even if values are restored', reason => {
      const f = fixture(), s = f.owner.state
      if (reason === 'draft') { s.messageStr.value = 'edit'; s.messageStr.value = 'question' }
      if (reason === 'quote') { s.quotedText.value = 'edit'; s.quotedText.value = 'quote' }
      if (reason === 'files') { const files = s.fileList.value; s.fileList.value = []; s.fileList.value = files }
      if (reason === 'model') { s.modelId.value = 'edit'; s.modelId.value = '' }
      if (reason === 'role') { f.options.systemPrompt = 'edit'; f.options.systemPrompt = 'role' }
      if (reason === 'owner') f.owner.invalidateContext()
      if (reason === 'conversation') { f.store.currentConversationId = 'other'; f.store.currentConversationId = 'conversation' }
      if (reason === 'switch') { f.binding.deactivate(); f.binding.activate(f.context) }
      expect(f.intent.isCurrent()).toBe(false); expect(f.intent.signal.aborted).toBe(true)
    })
  it('restores moved text on UI disposal but never after owner reset', () => {
    const f = fixture(); expect(f.intent.moveDraft()).toBe(true)
    f.binding.dispose(); f.intent.dispose(); expect(f.owner.state.messageStr.value).toBe('question')
    const other = fixture(); other.intent.moveDraft(); other.owner.resetSelectedConversation(); other.intent.dispose()
    expect(other.owner.state.messageStr.value).toBe('')
  })
  it('does not restore after replacement text was entered and erased', () => {
    const f = fixture(); f.intent.moveDraft()
    f.owner.state.messageStr.value = 'replacement'; f.owner.state.messageStr.value = ''
    f.intent.dispose(); expect(f.owner.state.messageStr.value).toBe('')
  })
  it('allows controlled draft/attachment changes but rejects mutation once invalid', () => {
    const f = fixture(); f.intent.moveDraft()
    expect(f.intent.isCurrent()).toBe(true)
    f.intent.mutate(() => f.owner.removeAttachment('file')); expect(f.intent.isCurrent()).toBe(true)
    f.binding.deactivate(); expect(f.intent.mutate(() => { throw new Error('must not run') })).toBe(false)
  })
  it('handoff detaches UI invalidation and cleanup does not restore consumed drafts', () => {
    const f = fixture(); f.intent.moveDraft(); f.intent.commit()
    f.binding.deactivate(); expect(f.intent.signal.aborted).toBe(false)
    f.intent.dispose(); expect(f.owner.state.messageStr.value).toBe('')
  })
  it('aborts a stalled UI wait promptly and consumes its later rejection', async () => {
    const f = fixture(); let reject!: (error: Error) => void
    const work = f.intent.wait(new Promise((_, no) => { reject = no }))
    f.binding.deactivate(); await expect(work).rejects.toMatchObject({ name: 'AbortError' })
    reject(new Error('late')); await Promise.resolve()
  })
})
