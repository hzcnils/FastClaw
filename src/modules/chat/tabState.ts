import { ref, reactive } from 'vue'
import type { ChatAttachment } from '@/types'
import type { SlashCommand } from '@/composables/useSlashCommands'
import type { Agent } from '@/stores/agents'
import type { ChatEmbeddedTimelineEntry } from 'chat-renderer'

// 新建 Tab 与新对话重置使用同一默认模式；历史会话仍按保存的配置恢复。
export const DEFAULT_TASK_PLAN_ENABLED = false

export interface DocVersion { label: string; content: string; timestamp: string }
export interface DocCardInfo { versionIdx: number; label: string; timestamp: string }
export interface ChatReadingAnchor { attribute: 'data-message-id' | 'data-entry-id'; id: string; offset: number }

function createState() {
  return {
    messageStr: ref(''), quotedText: ref(''), selectedSkill: ref<SlashCommand | null>(null),
    fileList: ref<ChatAttachment[]>([]), fileKbIds: ref<string[]>([]), conversationFileIds: ref<string[]>([]),
    pendingAutoSend: ref(false), sendBtnLock: ref(false), chatSubmissionError: ref(''),
    modelId: ref(''), lastUsableModelId: ref(''), knowledgeId: ref<string[]>([]),
    dialogueCharacter: ref({ image: false, ocr: false, think: false, taskPlan: DEFAULT_TASK_PLAN_ENABLED, webSearch: false }),
    selectedAgentId: ref(''), selectedAgent: ref<Agent | null>(null),
    // 'preview' 档由右侧网页预览面板使用（previewPanelStore.isOpen 驱动，
    // 见 ChatView 里同步 rightPanel 的 watch）。合并时本地那份是独立的 ref、
    // 联合类型里带 'preview'，搬进 tabState 后要把这一档一起带过来。
    rightPanel: ref<'none' | 'prompt' | 'trace' | 'websearch' | 'preview'>('none'), isHistoryCollapsed: ref(false),
    inputAreaHeightPx: ref<number | null>(null), inputAreaMinHeightPx: ref(0), chatPanelPx: ref(0),
    shouldAutoScroll: ref(true), savedScrollTop: ref(0), readingAnchor: ref<ChatReadingAnchor | null>(null),
    highlightedIndex: ref(-1), currentTraceMessageId: ref<string | null>(null), currentWebSearchMessageId: ref<string | null>(null),
    editorVisible: ref(false), editorContent: ref(''), editorTitle: ref(''),
    docVersions: ref<DocVersion[]>([]), activeVersionIdx: ref(0), pendingDocEdit: ref(false),
    docEditCardMap: reactive<Record<string, DocCardInfo>>({}), docEditMessageIds: reactive<Record<string, true>>({}),
    inlineUserEditId: ref<string | null>(null), inlineUserEditContent: ref(''),
    inlineUserEditOrigin: ref<{ conversationId: string; content: string } | null>(null),
    editingMessage: ref<{ id: string; content: string; draftContent?: string } | null>(null), editDialogVisible: ref(false),
    messageEditSaveId: ref<string | null>(null),
    asideTab: ref<'history' | 'favorites'>('history'),
    fastPlanEntries: ref<ChatEmbeddedTimelineEntry[]>([]), activeFastPlanEntryId: ref<string | null>(null),
    planningPreparationId: ref<string | null>(null),
    remotePlanControl: ref<{ waitKey: string; action: string; status: 'pending' | 'sent' } | null>(null),
    fastPlanDataObj: ref<{ question?: string; sessionId?: string } | null>(null),
    fastPlanExecutionMode: ref<'auto' | 'manual' | 'step'>('auto'),
    fastPlanBusyByConv: ref(new Map<string, boolean>()),
  }
}

/** In-memory Tab ownership. KeepAlive eviction must never call release(). */
export function createChatTabState(instanceId: string) {
  const state = createState()
  const files = new Map<string, File>()
  const blobUrls = new Map<string, string>()
  const operations = new Map<string, AbortController>()
  const invalidationListeners = new Set<() => void>()
  let generation = 0
  let disposed = false
  const record = {
    instanceId, state, files, fastPlanEntryOwners: new Map<string, string>(),
    planWriterId: `chat-runtime:${instanceId}:${crypto.randomUUID()}`,
    initialized: false,
    hydrationRequired: false,
    get disposed() { return disposed },
    /** Captures this record (not just the reusable tab id) and its draft context. */
    capture() {
      const epoch = generation
      return () => !disposed && epoch === generation
    },
    invalidateContext() {
      generation++
      state.remotePlanControl.value = null
      state.pendingAutoSend.value = false
      for (const listener of [...invalidationListeners]) listener()
    },
    onInvalidate(listener: () => void) {
      invalidationListeners.add(listener)
      return () => invalidationListeners.delete(listener)
    },
    retainFile(id: string, file: File, url?: string) {
      files.set(id, file)
      if (url?.startsWith('blob:')) blobUrls.set(id, url)
    },
    beginAttachment(id: string) {
      operations.get(id)?.abort()
      const controller = new AbortController()
      operations.set(id, controller)
      const current = record.capture()
      return {
        signal: controller.signal,
        current: () => current() && !controller.signal.aborted && operations.get(id) === controller
          && state.fileList.value.some(file => file.id === id),
        finish() { if (operations.get(id) === controller) operations.delete(id) },
      }
    },
    releaseAttachment(id: string) {
      operations.get(id)?.abort()
      operations.delete(id)
      files.delete(id)
      const url = blobUrls.get(id) || state.fileList.value.find(f => f.id === id)?.preview_url
      if (url?.startsWith('blob:')) URL.revokeObjectURL(url)
      blobUrls.delete(id)
    },
    clearAttachments() {
      for (const id of new Set([...files.keys(), ...blobUrls.keys(), ...operations.keys(), ...state.fileList.value.map(f => f.id)])) record.releaseAttachment(id)
      state.fileList.value = []
      state.pendingAutoSend.value = false
    },
    removeAttachment(id: string) {
      const file = state.fileList.value.find(f => f.id === id)
      record.releaseAttachment(id)
      state.fileList.value = state.fileList.value.filter(f => f.id !== id)
      if (file?.uploaded_kb_id && !state.fileList.value.some(f => f.uploaded_kb_id === file.uploaded_kb_id)) {
        state.fileKbIds.value = state.fileKbIds.value.filter(k => k !== file.uploaded_kb_id)
      }
      if (file?.uploaded_file_id) state.conversationFileIds.value = state.conversationFileIds.value.filter(k => k !== file.uploaded_file_id)
      // Waiting to send with a changed attachment set needs an explicit new send.
      state.pendingAutoSend.value = false
    },
    resetConversationUi() {
      record.invalidateContext()
      state.chatSubmissionError.value = ''
      record.clearAttachments()
      state.fileKbIds.value = []
      state.conversationFileIds.value = []
      state.inlineUserEditId.value = null
      state.inlineUserEditContent.value = ''
      state.inlineUserEditOrigin.value = null
      state.editingMessage.value = null
      state.editDialogVisible.value = false
      state.messageEditSaveId.value = null
      state.pendingDocEdit.value = false
      state.currentTraceMessageId.value = null
      state.currentWebSearchMessageId.value = null
      state.savedScrollTop.value = 0
      state.readingAnchor.value = null
      state.highlightedIndex.value = -1
    },
    resetSelectedConversation() {
      record.resetConversationUi()
      state.selectedAgent.value = null
      state.selectedAgentId.value = ''
      state.selectedSkill.value = null
      state.editorVisible.value = false
      state.editorContent.value = ''
      state.editorTitle.value = ''
      state.docVersions.value = []
      state.activeVersionIdx.value = 0
      for (const key of Object.keys(state.docEditCardMap)) delete state.docEditCardMap[key]
      for (const key of Object.keys(state.docEditMessageIds)) delete state.docEditMessageIds[key]
      state.fastPlanEntries.value = []
      state.activeFastPlanEntryId.value = null
      state.fastPlanDataObj.value = null
      state.rightPanel.value = 'none'
    },
    release() {
      if (disposed) return
      disposed = true
      record.invalidateContext()
      state.pendingDocEdit.value = false
      record.clearAttachments()
      record.fastPlanEntryOwners.clear()
      invalidationListeners.clear()
    },
  }
  return record
}

export type ChatTabState = ReturnType<typeof createChatTabState>
const records = new Map<string, ChatTabState>()
export function findChatTabState(instanceId: string): ChatTabState | undefined {
  return records.get(instanceId)
}
export function getChatTabState(instanceId = 'default'): ChatTabState {
  let record = records.get(instanceId)
  if (!record) { record = createChatTabState(instanceId); records.set(instanceId, record) }
  return record
}
export function releaseChatTabState(instanceId: string) {
  records.get(instanceId)?.release()
  records.delete(instanceId)
}

/** Store-driven changes (including remote deletion) also invalidate an evicted view's state. */
export function syncChatTabStateConversation(instanceId: string, id: string | null, previous: string | null | undefined) {
  const record = records.get(instanceId)
  if (!record || !previous || previous === id) return
  record.resetSelectedConversation()
  record.hydrationRequired = !!id
}
