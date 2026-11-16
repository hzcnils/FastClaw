import { afterEach, describe, expect, it, vi } from 'vitest'
import { createChatTabState, getChatTabState, releaseChatTabState, syncChatTabStateConversation } from './tabState'
import { recognizeTabImage, uploadTabDocument } from './attachmentTasks'
import { captureReadingPosition, restoreReadingPosition } from './readingPosition'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}
const uploaded = { success: true, data: { file_id: 'file-a', kb_id: 'kb-a' } }
afterEach(() => { releaseChatTabState('a'); releaseChatTabState('b'); vi.restoreAllMocks() })

describe('Tab state ownership', () => {
  it('defaults new tabs to ordinary chat without overwriting a retained planning choice', () => {
    const a = getChatTabState('a')
    expect(a.state.dialogueCharacter.value.taskPlan).toBe(false)
    a.state.dialogueCharacter.value.taskPlan = true
    expect(getChatTabState('a').state.dialogueCharacter.value.taskPlan).toBe(true)
    expect(getChatTabState('b').state.dialogueCharacter.value.taskPlan).toBe(false)
    releaseChatTabState('a')
    expect(getChatTabState('a').state.dialogueCharacter.value.taskPlan).toBe(false)
  })

  it('retains drafts, choices, document edits and plan associations across view reacquisition without cloning', () => {
    const a = getChatTabState('a'), b = getChatTabState('b')
    a.state.messageStr.value = 'draft A'
    a.state.knowledgeId.value = ['kb']
    a.state.editorContent.value = 'unversioned edit'
    a.state.docVersions.value = [{ label: 'v1', content: 'old version', timestamp: 'now' }]
    a.state.fastPlanExecutionMode.value = 'step'
    a.state.activeFastPlanEntryId.value = 'entry-a'
    a.state.editingMessage.value = { id: 'message-a', content: 'original', draftContent: 'unsaved message edit' }
    const returned = getChatTabState('a')
    expect(returned).toBe(a)
    expect(returned.state.docVersions.value).toBe(a.state.docVersions.value)
    expect(returned.state.editorContent.value).toBe('unversioned edit')
    expect(returned.state.fastPlanExecutionMode.value).toBe('step')
    expect(returned.state.messageStr.value).toBe('draft A')
    expect(returned.state.editingMessage.value?.draftContent).toBe('unsaved message edit')
    expect(b.state.messageStr.value).toBe('')
    expect(b.state.knowledgeId.value).toEqual([])
    expect(b.state.activeFastPlanEntryId.value).toBeNull()
  })

  it('invalidates old operations even when a closed tab id is reused', () => {
    const old = getChatTabState('a'), current = old.capture()
    old.state.messageStr.value = 'old'
    releaseChatTabState('a')
    const reopened = getChatTabState('a')
    expect(current()).toBe(false)
    expect(old.disposed).toBe(true)
    expect(reopened).not.toBe(old)
    expect(reopened.state.messageStr.value).toBe('')
    expect(reopened.capture()()).toBe(true)
  })

  it('handles a store-driven conversation change even with no mounted view', () => {
    const owner = getChatTabState('a'), valid = owner.capture()
    owner.state.editorContent.value = 'conversation A document'
    owner.state.fileList.value.push({ id: 'a', name: 'a.txt', type: 'document', status: 'uploading' })
    syncChatTabStateConversation('a', 'conversation-b', 'conversation-a')
    expect(valid()).toBe(false)
    expect(owner.state.editorContent.value).toBe('')
    expect(owner.state.fileList.value).toEqual([])
    expect(owner.hydrationRequired).toBe(true)
    syncChatTabStateConversation('a', null, 'conversation-b')
    expect(owner.hydrationRequired).toBe(false)
  })

  it('releases raw Files and original blob URLs exactly once even if preview was converted', () => {
    const revoke = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {})
    const owner = createChatTabState('a'), file = new File(['x'], 'x.png')
    owner.state.fileList.value.push({ id: 'image', type: 'image', name: 'x.png', preview_url: 'blob:original', status: 'pending' })
    owner.retainFile('image', file, 'blob:original')
    owner.state.fileList.value[0].preview_url = 'data:image/png;base64,AA=='
    owner.removeAttachment('image')
    owner.release()
    owner.release()
    expect(revoke).toHaveBeenCalledExactlyOnceWith('blob:original')
    expect(owner.files.size).toBe(0)
  })

  it('same-tab conversation reset invalidates resources and editing identity, not global settings', () => {
    const owner = createChatTabState('a'), current = owner.capture()
    owner.state.inlineUserEditId.value = 'message-a'
    owner.state.knowledgeId.value = ['tab-local']
    owner.state.pendingAutoSend.value = true
    owner.resetConversationUi()
    expect(current()).toBe(false)
    expect(owner.state.inlineUserEditId.value).toBeNull()
    expect(owner.state.pendingAutoSend.value).toBe(false)
    expect(owner.state.knowledgeId.value).toEqual(['tab-local'])
    expect(owner.state).not.toHaveProperty('activeRole')
    expect(owner.state).not.toHaveProperty('mcp')
  })
})

describe('attachment task lifetime', () => {
  it('uploads to its original record while another tab is active or the original view has gone', async () => {
    const owner = getChatTabState('a'), pending = deferred<typeof uploaded>(), persist = vi.fn()
    const operation = uploadTabDocument(owner, new File(['a'], 'a.txt'), { upload: () => pending.promise, persist })
    const other = getChatTabState('b')
    other.state.messageStr.value = 'B'
    pending.resolve(uploaded)
    await operation
    expect(getChatTabState('a').state.fileList.value[0]).toMatchObject({ status: 'uploaded', uploaded_file_id: 'file-a' })
    expect(owner.state.conversationFileIds.value).toEqual(['file-a'])
    expect(other.state.fileList.value).toEqual([])
    expect(persist).toHaveBeenCalledTimes(1)
  })

  it.each(['remove', 'reset', 'close'] as const)('rejects late upload success after %s, including cache/KB writes', async action => {
    const owner = getChatTabState('a'), pending = deferred<typeof uploaded>(), persist = vi.fn()
    let signal!: AbortSignal
    const task = uploadTabDocument(owner, new File(['a'], 'a.txt'), { upload: (_file, captured) => { signal = captured; return pending.promise }, persist })
    if (action === 'remove') owner.removeAttachment(owner.state.fileList.value[0].id)
    if (action === 'reset') owner.resetConversationUi()
    if (action === 'close') releaseChatTabState('a')
    pending.resolve(uploaded)
    await task
    expect(signal.aborted).toBe(true)
    expect(owner.state.fileList.value).toEqual([])
    expect(owner.state.fileKbIds.value).toEqual([])
    expect(owner.state.conversationFileIds.value).toEqual([])
    expect(persist).not.toHaveBeenCalled()
  })

  it('marks current upload failure and cancels queued auto-send', async () => {
    const owner = createChatTabState('a'), pending = deferred<typeof uploaded>()
    const task = uploadTabDocument(owner, new File(['a'], 'a.txt'), { upload: () => pending.promise, persist: vi.fn() })
    owner.state.pendingAutoSend.value = true
    pending.reject(new Error('offline'))
    await task
    expect(owner.state.fileList.value[0]).toMatchObject({ status: 'error', error_message: 'offline' })
    expect(owner.state.pendingAutoSend.value).toBe(false)
  })

  it('OCR finds the original attachment after the list is reordered, not the old array index', async () => {
    const owner = createChatTabState('a'), pending = deferred<string>()
    owner.state.fileList.value = ['a', 'b'].map(id => ({ id, type: 'image', name: id, status: 'pending', preview_url: 'data:x' }))
    const task = recognizeTabImage(owner, 'a', () => pending.promise)
    owner.state.fileList.value.reverse()
    pending.resolve(' extracted text ')
    await task
    expect(owner.state.fileList.value[1].ocr_text).toBe('extracted text')
    expect(owner.state.fileList.value[0].ocr_text).toBeUndefined()
  })

  it('ignores older OCR attempts and deletion followed by an unrelated attachment', async () => {
    const owner = createChatTabState('a'), old = deferred<string>(), fresh = deferred<string>()
    owner.state.fileList.value = [{ id: 'a', type: 'image', name: 'a', status: 'pending', preview_url: 'data:x' }]
    const oldTask = recognizeTabImage(owner, 'a', () => old.promise)
    const newTask = recognizeTabImage(owner, 'a', () => fresh.promise)
    fresh.resolve('new')
    await newTask
    old.resolve('old')
    await oldTask
    expect(owner.state.fileList.value[0].ocr_text).toBe('new')
    const removed = deferred<string>()
    const removedTask = recognizeTabImage(owner, 'a', () => removed.promise)
    owner.removeAttachment('a')
    owner.state.fileList.value.push({ id: 'b', name: 'b', type: 'image', status: 'pending' })
    removed.resolve('should not appear')
    await removedTask
    expect(owner.state.fileList.value[0]).not.toHaveProperty('ocr_text')
  })
})

describe('reading position', () => {
  it('restores the same message offset after content before it changes height', () => {
    const wrap = document.createElement('div'), message = document.createElement('article')
    message.setAttribute('data-message-id', 'id"[]')
    wrap.appendChild(message)
    wrap.scrollTop = 120
    vi.spyOn(wrap, 'getBoundingClientRect').mockReturnValue({ top: 50, bottom: 350 } as DOMRect)
    const bounds = vi.spyOn(message, 'getBoundingClientRect').mockReturnValue({ top: 70, bottom: 180 } as DOMRect)
    const position = captureReadingPosition(wrap)
    bounds.mockReturnValue({ top: 170, bottom: 280 } as DOMRect)
    restoreReadingPosition(wrap, position.top, position.anchor)
    expect(wrap.scrollTop).toBe(220)
    message.remove()
    restoreReadingPosition(wrap, position.top, position.anchor)
    expect(wrap.scrollTop).toBe(120)
  })
})
