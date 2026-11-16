// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from 'vitest'
import { createChatTabState } from './tabState'
import { createDocumentEditRequest } from './documentEditRuntime'

const content = '已修改\n---DOCUMENT_CONTENT_START---\n新版正文\n---DOCUMENT_CONTENT_END---'
const saved = (id: string) => JSON.parse(localStorage.getItem(`fastclaw:docEdit:${id}`) || 'null')
beforeEach(() => localStorage.clear())

describe('document request ownership (no view subscription)', () => {
  it('finishes once even when the editor has been closed', () => {
    const owner = createChatTabState('a')
    owner.state.docVersions.value = [{ label: 'V1', content: '原文', timestamp: 'before' }]
    const request = createDocumentEditRequest(owner, 'conversation-a')
    owner.state.editorVisible.value = false
    expect(request.finish('conversation-a', 'final-message', content)).toBe('已修改')
    expect(request.finish('conversation-a', 'final-message', content)).toBeNull()
    expect(owner.state.docVersions.value).toHaveLength(2)
    expect(owner.state.editorContent.value).toBe('新版正文')
    expect(owner.state.pendingDocEdit.value).toBe(false)
    expect(saved('conversation-a').cardMap['final-message'].label).toBe('V2')
    expect(saved('conversation-a').visible).toBe(false)
  })

  it('persists the original document without changing the newly selected conversation', () => {
    const owner = createChatTabState('a')
    owner.state.docVersions.value = [{ label: 'V1', content: 'A', timestamp: 'before' }]
    const request = createDocumentEditRequest(owner, 'a')
    owner.resetSelectedConversation()
    owner.state.editorContent.value = 'B 草稿'
    owner.state.pendingDocEdit.value = true
    expect(request.finish('b', 'wrong', content)).toBeNull()
    expect(request.finish('a', 'right', content)).toBe('已修改')
    expect(owner.state.editorContent.value).toBe('B 草稿')
    expect(owner.state.docVersions.value).toEqual([])
    expect(owner.state.pendingDocEdit.value).toBe(true)
    expect(saved('a').versions).toHaveLength(2)
    expect(saved('b')).toBeNull()
  })

  it('binds the new conversation once and refuses another identity', () => {
    const owner = createChatTabState('a')
    const request = createDocumentEditRequest(owner, null)
    expect(request.bind('new')).toBe(true)
    expect(request.bind('other')).toBe(false)
    expect(request.finish('new', 'id', content)).toBe('已修改')
    expect(saved('new').versions).toHaveLength(1)
  })

  it.each(['cancel', 'close', 'incomplete'] as const)('does not archive a %s request', reason => {
    const owner = createChatTabState('a')
    const request = createDocumentEditRequest(owner, 'a')
    if (reason === 'cancel') request.cancel()
    if (reason === 'close') owner.release()
    expect(request.finish('a', 'id', reason === 'incomplete' ? '正文未结束' : content)).toBeNull()
    expect(saved('a')).toBeNull()
    expect(owner.state.docVersions.value).toEqual([])
  })
})
