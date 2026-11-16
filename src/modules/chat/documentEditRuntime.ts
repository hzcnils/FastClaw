import type { ChatTabState } from './tabState'

const START = '---DOCUMENT_CONTENT_START---'
const END = '---DOCUMENT_CONTENT_END---'

function snapshot(owner: ChatTabState) {
  const state = owner.state
  return {
    versions: [...state.docVersions.value],
    activeVersionIdx: state.activeVersionIdx.value,
    cardMap: { ...state.docEditCardMap },
    messageIds: { ...state.docEditMessageIds },
    title: state.editorTitle.value,
    visible: state.editorVisible.value,
    draftContent: state.editorContent.value,
  }
}

function persist(conversationId: string, value: ReturnType<typeof snapshot>) {
  try { localStorage.setItem(`fastclaw:docEdit:${conversationId}`, JSON.stringify(value)) }
  catch (error) { console.warn('[chat] persist document versions failed', error) }
}

export function saveDocumentEditState(owner: ChatTabState, conversationId: string | null) {
  if (!conversationId || owner.disposed) return
  if (!owner.state.editorVisible.value && !owner.state.docVersions.value.length) return
  persist(conversationId, snapshot(owner))
}

/** A request belongs to its original conversation, never to the last visible message. */
export function createDocumentEditRequest(owner: ChatTabState, conversationId: string | null) {
  const isCurrent = owner.capture()
  const original = snapshot(owner)
  let target = conversationId
  let settled = false
  owner.state.pendingDocEdit.value = true
  return {
    bind(id: string) {
      if (!target) target = id
      return !settled && target === id && !owner.disposed
    },
    finish(id: string, messageId?: string, content?: string): string | null {
      if (settled || target !== id) return null
      settled = true
      if (isCurrent()) owner.state.pendingDocEdit.value = false
      if (owner.disposed || !messageId || content == null) return null
      const start = content.indexOf(START)
      const end = content.indexOf(END, start + START.length)
      if (start < 0 || end <= start) return null
      const body = content.slice(start + START.length, end).trim()
      // A context reset must not copy the new conversation's document into the old one.
      const saved = isCurrent() ? snapshot(owner) : original
      const versionIdx = saved.versions.length
      const label = `V${versionIdx + 1}`
      const timestamp = new Date().toISOString()
      saved.versions.push({ label, content: body, timestamp })
      saved.activeVersionIdx = versionIdx
      saved.draftContent = body
      saved.cardMap[messageId] = { versionIdx, label, timestamp }
      saved.messageIds[messageId] = true
      if (isCurrent()) {
        owner.state.docVersions.value = saved.versions
        owner.state.activeVersionIdx.value = versionIdx
        owner.state.editorContent.value = body
        owner.state.docEditCardMap[messageId] = saved.cardMap[messageId]
        owner.state.docEditMessageIds[messageId] = true
      }
      persist(id, saved)
      return content.slice(0, start).trim() || '已根据指令修改文档内容。'
    },
    cancel() {
      if (settled) return
      settled = true
      if (isCurrent()) owner.state.pendingDocEdit.value = false
    },
  }
}

export type DocumentEditRequest = ReturnType<typeof createDocumentEditRequest>
