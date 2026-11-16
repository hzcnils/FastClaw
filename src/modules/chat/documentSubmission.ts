import { effectScope, markRaw, watch } from 'vue'
import type { ChatTabState } from './tabState'
import type { DeferredChatStore } from './deferredSubmission'

export interface DocumentSubmissionStore extends Pick<DeferredChatStore, 'currentConversationId' | 'sendMessage'> {
  isLoading: boolean
  activateDocEditStream(marker: string, conversationId?: string | null): () => void
}
export interface DocumentSubmission {
  owner: ChatTabState
  store: DocumentSubmissionStore
  conversationId: string | null
  raw: string
  quote: string
  document: string
  modelId: string
  providerCode?: string
}

function documentPrompt(document: string) {
  return `你是专业文档编辑助手。请根据用户的修改指令对下方文档进行修改，并严格按照以下格式输出，不得省略任何部分：

1.先输出1-2句话的简要说明（说明你修改了什么）。
2.在新的一行输出下面这个标记（原样输出，不能修改）：
---DOCUMENT_CONTENT_START---
3.输出修改后的完整文档内容（保持原有 Markdown 格式，不要省略）。
4.在新的一行输出下面这个标记（原样输出，不能修改）：
---DOCUMENT_CONTENT_END---

重要规则：
- 两个标记行必须单独占一行，行内不含其他任何字符
- ---DOCUMENT_CONTENT_START--- 和 ---DOCUMENT_CONTENT_END--- 之间只放修改后的文档内容
- 必须输出完整的文档，不得省略未修改的部分

当前文档内容如下：
${document}`
}

/** Prepares a document command; Store owns stream and version finalization after acceptance. */
export class DocumentSubmissionService {
  private jobs = new WeakMap<ChatTabState, Promise<boolean>>()
  constructor() { markRaw(this) }

  submit(input: DocumentSubmission): Promise<boolean> {
    const existing = this.jobs.get(input.owner)
    if (existing) return existing
    const task = this.run({ ...input }).finally(() => {
      if (this.jobs.get(input.owner) === task) this.jobs.delete(input.owner)
    })
    this.jobs.set(input.owner, task)
    return task
  }

  private async run(input: DocumentSubmission): Promise<boolean> {
    const { owner, store } = input
    const current = owner.capture(), state = owner.state
    if (!current() || store.isLoading || store.currentConversationId !== input.conversationId
      || !input.raw.trim() || !input.modelId) return false
    let cancelRequest: (() => void) | undefined
    let accepted = false
    let edited = false
    const scope = effectScope(true)
    const removeInvalidation = owner.onInvalidate(() => scope.stop())
    const sameConversation = () => input.conversationId === null || store.currentConversationId === input.conversationId
    const restore = () => {
      if (current() && sameConversation() && !edited && !store.isLoading && !state.pendingDocEdit.value
        && !state.messageStr.value && !state.quotedText.value) {
        state.messageStr.value = input.raw
        state.quotedText.value = input.quote
      }
    }
    try {
      state.chatSubmissionError.value = ''
      cancelRequest = store.activateDocEditStream('---DOCUMENT_CONTENT_START---', input.conversationId)
      state.messageStr.value = ''
      state.quotedText.value = ''
      scope.run(() => watch([state.messageStr, state.quotedText], () => { edited = true }, { flush: 'sync' }))
      // Preserve the document-edit contract: only the instruction is a visible user message.
      // 位置参数：skillName / enabledSkills 也得各占一位，文档编辑两者都不带
      const result = await store.sendMessage(input.raw, input.modelId, [], [], undefined,
        documentPrompt(input.document), undefined, undefined, undefined, undefined, undefined,
        undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, input.providerCode)
      accepted = result !== false
      if (!accepted) { cancelRequest?.(); cancelRequest = undefined; restore() }
      return accepted
    } catch (error) {
      cancelRequest?.(); cancelRequest = undefined
      if (current() && sameConversation() && !store.isLoading && !state.pendingDocEdit.value) {
        state.chatSubmissionError.value = error instanceof Error ? error.message : String(error)
        restore()
      }
      return false
    } finally {
      removeInvalidation()
      scope.stop()
      // Identity-scoped handle: an old failure must not clear a newer document request.
      if (!accepted) cancelRequest?.()
    }
  }
}

export const documentSubmissionService = new DocumentSubmissionService()
