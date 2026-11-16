import { effectScope, markRaw, watch } from 'vue'
import type { ChatAttachment } from '@/types'
import { useExecutionActivityStore } from '@/stores/executionActivity'
import type { ChatTabState } from './tabState'
import { abortable } from './abortable'

export interface DeferredChatStore {
  currentConversationId: string | null
  sendMessage(content: string, modelId?: string, ...options: any[]): Promise<unknown>
  setConvFileCache(id: string, files: ChatAttachment[]): void
}
export interface DeferredChatSubmission {
  owner: ChatTabState
  store: DeferredChatStore
  tabPath: string
  conversationId: string | null
  raw: string
  quote: string
  content: string
  modelId: string
  vision: boolean
  knowledgeBaseIds: string[]
  localKbPaths: { path: string; recursive: boolean }[]
  systemPrompt?: string
  assistantId?: string
  assistantDisplay?: { agent_name: string; agent_icon?: string | null }
  webSearch: boolean
  searchProvider?: string
  thinking: boolean
  mcpTools: boolean
  mcpServerIds: string[]
  skillName?: string
  /** 「本次对话」的 Skill 白名单（输入框「工具」弹层里的开关）。 */
  enabledSkills?: string[]
  providerCode?: string
  /** False for immediate sends; never silently queue an unconfirmed upload. Defaults to B5's wait behavior. */
  waitForUploads?: boolean
}

async function readImage(url: string, signal: AbortSignal): Promise<string> {
  if (url.startsWith('data:')) return url
  const blob = await fetch(url, { signal }).then(response => response.blob())
  return abortable(new Promise<string>((resolve, reject) => {
    const reader = new FileReader()
    const abort = () => reader.abort()
    reader.onload = () => resolve(String(reader.result))
    reader.onerror = () => reject(reader.error || new Error('读取图片失败'))
    reader.onabort = () => reject(new DOMException('发送已取消', 'AbortError'))
    reader.onloadend = () => signal.removeEventListener('abort', abort)
    signal.addEventListener('abort', abort, { once: true })
    reader.readAsDataURL(blob)
    if (signal.aborted) reader.abort()
  }), signal)
}

/** Shared ordinary-chat preparation for immediate and accepted upload-wait sends.
 * Retains the B5 module/API name; neither path retains a ChatView callback. */
export class DeferredChatSubmissionService {
  private jobs = new WeakMap<ChatTabState, { task: Promise<boolean>; cancel(): void }>()
  constructor(private imageReader = readImage) { markRaw(this) }

  cancel(owner: ChatTabState): boolean {
    const job = this.jobs.get(owner)
    if (!job) return false
    job.cancel()
    return true
  }

  submit(input: DeferredChatSubmission): Promise<boolean> {
    const existing = this.jobs.get(input.owner)
    if (existing) return existing.task
    const snapshot = { ...input, knowledgeBaseIds: [...input.knowledgeBaseIds],
      localKbPaths: input.localKbPaths.map(item => ({ ...item })), mcpServerIds: [...input.mcpServerIds],
      assistantDisplay: input.assistantDisplay ? { ...input.assistantDisplay } : undefined }
    const controller = new AbortController()
    const job = { task: Promise.resolve(false), cancel: () => controller.abort() }
    this.jobs.set(input.owner, job)
    job.task = this.run(snapshot, controller).finally(() => {
      if (this.jobs.get(input.owner) === job) this.jobs.delete(input.owner)
    })
    return job.task
  }

  private async run(input: DeferredChatSubmission, controller: AbortController): Promise<boolean> {
    const { owner, store } = input
    const state = owner.state, signal = controller.signal, current = owner.capture()
    const scope = effectScope(true)
    const activity = useExecutionActivityStore()
    const activityId = `draft:${owner.instanceId}:${crypto.randomUUID()}`
    const attachmentIds = state.fileList.value.map(file => file.id)
    const inheritedKbIds = [...state.fileKbIds.value], inheritedFileIds = [...state.conversationFileIds.value]
    const removeInvalidation = owner.onInvalidate(() => controller.abort())
    let consumed = false
    let attachments: ChatAttachment[] = []
    const assertCurrent = () => {
      if (signal.aborted || !current() || store.currentConversationId !== input.conversationId)
        throw new DOMException('发送已取消', 'AbortError')
    }
    try {
      assertCurrent()
      state.chatSubmissionError.value = ''
      state.pendingAutoSend.value = true
      activity.update(activityId, input.tabPath, 'chat', { phase: 'submitting', protected: true })
      if (input.waitForUploads === false && state.fileList.value.some(file => file.status === 'uploading'))
        throw new Error('附件仍在处理中，请确认等待完成后发送')
      scope.run(() => watch([state.messageStr, state.quotedText, state.modelId,
        () => state.dialogueCharacter.value.taskPlan, () => state.dialogueCharacter.value.image,
        state.editorVisible, () => state.fileList.value.map(file => file.id).join(','), state.pendingAutoSend,
        () => store.currentConversationId], () => {
          if (current() && state.fileList.value.some(file => file.status === 'error'))
            state.chatSubmissionError.value = '附件处理失败，请重试附件后重新发送'
          controller.abort()
        }, { flush: 'sync' }))
      // Post flush: upload commits status and returned KB/file IDs in one synchronous turn.
      await abortable(new Promise<void>((resolve, reject) => {
        scope.run(() => watch(() => state.fileList.value.map(file => file.status), () => {
          if (state.fileList.value.some(file => file.status === 'error')) reject(new Error('附件处理失败，请重试附件后重新发送'))
          else if (!state.fileList.value.some(file => file.status === 'uploading')) resolve()
        }, { immediate: true, flush: 'post' }))
      }), signal)
      assertCurrent()
      attachments = attachmentIds.map(id => ({ ...state.fileList.value.find(file => file.id === id)! }))
        .filter(file => file.status === 'uploaded' || file.type === 'image' || file.type === 'screenshot')
      const images: string[] = [], ocr: string[] = []
      const imageAttachments = attachments.filter(file => file.type === 'image' || file.type === 'screenshot')
      for (const [index, file] of imageAttachments.entries()) {
        if (file.preview_url) {
          file.preview_url = await abortable(this.imageReader(file.preview_url, signal), signal)
          assertCurrent()
          if (input.vision) images.push(file.preview_url)
        }
        if (file.ocr_text?.trim()) ocr.push(`[图片${index + 1} OCR结果]:\n${file.ocr_text.trim()}`)
      }
      assertCurrent()
      const docs = attachments.filter(file => file.type === 'document')
      const kbIds = [...new Set([...input.knowledgeBaseIds, ...inheritedKbIds, ...docs.flatMap(file => file.uploaded_kb_id ? [file.uploaded_kb_id] : [])])]
      const fileIds = [...new Set([...inheritedFileIds, ...attachments.flatMap(file => file.uploaded_file_id ? [file.uploaded_file_id] : [])])]
      scope.stop() // Our own draft consumption and first-conversation binding are not edits.
      state.pendingAutoSend.value = false
      state.messageStr.value = ''
      state.quotedText.value = ''
      state.fileList.value = []
      consumed = true
      const result = await store.sendMessage(input.content, input.modelId, kbIds, attachments, undefined,
        input.systemPrompt, input.assistantId, undefined, input.localKbPaths, fileIds,
        input.webSearch, input.searchProvider, images, input.thinking, input.assistantDisplay,
        input.mcpTools, input.mcpServerIds,
        !input.vision && ocr.length ? '以下是从用户上传的图片中通过OCR提取的文字内容，请基于这些文字回答用户问题：\n\n' + ocr.join('\n\n') : undefined,
        input.skillName, input.enabledSkills, input.providerCode, { signal })
      if (result === false) {
        if (signal.aborted || !current()) return false
        // Store preparation was stopped before transport acceptance. Leave an explicit retry draft.
        if (!state.messageStr.value && !state.fileList.value.length) {
          state.messageStr.value = input.raw
          state.quotedText.value = input.quote
          state.fileList.value = attachments
        }
        return false
      }
      if (docs.length && current() && store.currentConversationId) store.setConvFileCache(store.currentConversationId, docs)
      if (current()) for (const id of attachmentIds) owner.releaseAttachment(id)
      return true
    } catch (error) {
      if (!signal.aborted && current()) {
        state.chatSubmissionError.value = error instanceof Error ? error.message : String(error)
        if (consumed && !state.messageStr.value && !state.fileList.value.length) {
          state.messageStr.value = input.raw
          state.quotedText.value = input.quote
          state.fileList.value = attachments
        }
      }
      return false
    } finally {
      scope.stop(); removeInvalidation()
      state.pendingAutoSend.value = false
      if (consumed) for (const id of attachmentIds) {
        if (!state.fileList.value.some(file => file.id === id)) owner.releaseAttachment(id)
      }
      activity.remove(activityId, 'chat')
    }
  }
}

export const deferredChatSubmissionService = new DeferredChatSubmissionService()
