// FastClaw · 对话状态管理（含流式输出支持）
import { defineStore } from 'pinia'
import { getChatTabState, releaseChatTabState, syncChatTabStateConversation } from '@/modules/chat/tabState'
import { conversationBranchService } from '@/modules/chat/conversationBranch'
import { deletedMessageLedger, messageDeletionService } from '@/modules/chat/messageDeletion'
import { messageActionService } from '@/modules/chat/messageActions'
import { abortable } from '@/modules/chat/abortable'
import { answerSource } from '@/modules/chat/answerTarget'
import { createDocumentEditRequest, type DocumentEditRequest } from '@/modules/chat/documentEditRuntime'
import { parseChatStreamText } from '@/utils/chatStreamText'
import { ref, watch, computed, reactive, effectScope } from 'vue'
import { hasUnreadCompletion, markConversationViewed, markConversationCompleted, forgetConversationActivity, setForegroundConversation } from '@/utils/conversationActivity'
import { chatTabPath, getChatTabConversation, setChatTabConversation, unregisterChatTab } from '@/utils/chatTabRegistry'
import { useExecutionActivityStore } from '@/stores/executionActivity'
import { useChatToolsStore } from '@/stores/chatTools'
import { useMcpChatStore } from '@/stores/mcpChat'
import { ElMessage } from 'element-plus'
import type { Conversation, Message, MessageResponse, KbReference, ChatAttachment, ToolCallInfo, WebHit, GroundingManifest } from '@/types'
import { DEFAULT_CONFIG } from '@/config/index'
import { hydrateMessagePlanRuntime } from '@/integrations/chat-plan/remotePlanRuntime'

/** 过程卡标题用：优先会话 model_id，其次同回合已有助手气泡上的 model_id。 */
function resolveSessionModelId(sessionId: string, messages?: Message[] | null): string | null {
  const conv = sharedConversations.value.find(c => c.id === sessionId)
  const fromConv = String(conv?.model_id || '').trim()
  if (fromConv) return fromConv
  const list = messages || sharedMessageCache.get(sessionId) || []
  for (let i = list.length - 1; i >= 0; i -= 1) {
    const m = list[i]
    if (!m || m.role !== 'assistant') continue
    const id = String(m.model_id || '').trim()
    if (id) return id
  }
  return null
}

/** 仅在新增过程气泡等结构变化时刷新 messages 列表。 */
function publishSessionMessages(sessionId: string, messages: Message[]) {
  sharedMessageCache.set(sessionId, messages)
  forEachActiveChatStore((s) => {
    if (s.currentConversationId.value !== sessionId) return
    if (s.messages.value !== messages) {
      s.messages.value = messages
      return
    }
    s.messages.value = messages.slice()
    sharedMessageCache.set(sessionId, s.messages.value)
  })
}

// Credentials must come from the selected model provider; never ship a shared key.

// 检测是否在 Tauri 环境（模块级，供 store 内外的辅助函数共用）
type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>
let tauriInvoke: InvokeFn | null = null
async function getInvoke(): Promise<InvokeFn | null> {
  if (tauriInvoke !== null) return tauriInvoke
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    tauriInvoke = invoke as InvokeFn
    return tauriInvoke
  } catch {
    return null
  }
}

// 历史会话属于应用级目录数据：所有对话 Tab 共享同一个引用和请求。
// 当前会话和草稿按 Tab 隔离；消息与流运行时按会话共享。
const sharedConversations = ref<Conversation[]>([])

/** Apply backend-owned metadata without forcing every Chat tab to reload the directory. */
export function patchSharedConversationExtraConfig(conversationId: string, extraConfig: string) {
  const conversation = sharedConversations.value.find(item => item.id === conversationId)
  if (conversation) conversation.extra_config = extraConfig
}
let conversationsLoaded = false
let conversationsRevision = 0
let appliedConversationsRevision = -1
let conversationsLoadPromise: Promise<boolean> | null = null
let conversationsRefreshScheduled = false

/** 标记会话列表已变更（本地增删 / 远程同步）：下一次加载会取最新快照。 */
function bumpConversationsRevision() {
  conversationsRevision += 1
}

/** 合并调度一次会话列表刷新：同 tick 内多次变更只触发一次网络请求。 */
function scheduleConversationsRefresh() {
  if (conversationsRefreshScheduled) return
  conversationsRefreshScheduled = true
  queueMicrotask(() => {
    conversationsRefreshScheduled = false
    void loadSharedConversations()
  })
}

async function loadSharedConversations(): Promise<void> {
  if (conversationsLoaded && appliedConversationsRevision >= conversationsRevision) return

  if (conversationsLoadPromise) {
    const succeeded = await conversationsLoadPromise
    // 若请求期间发生了新增、删除等变更，再补一次刷新，避免应用旧快照。
    if (succeeded && appliedConversationsRevision < conversationsRevision) {
      await loadSharedConversations()
    }
    return
  }

  const requestRevision = conversationsRevision
  conversationsLoadPromise = (async () => {
    const invoke = await getInvoke()
    if (!invoke) return false
    try {
      const result = await invoke<Conversation[]>('get_conversations', {})
      sharedConversations.value = Array.isArray(result) ? result : []
      conversationsLoaded = true
      appliedConversationsRevision = requestRevision
      return true
    } catch (err) {
      console.warn('Failed to load conversations', err)
      return false
    }
  })().finally(() => {
    conversationsLoadPromise = null
  })

  const succeeded = await conversationsLoadPromise
  if (succeeded && appliedConversationsRevision < conversationsRevision) {
    await loadSharedConversations()
  }
}

// ─── 对话级文档附件缓存（模块级单例）──────────────────────────
// 与 sharedConversations 同层共享：多个对话 Tab 读写的都是同一份缓存，
// 避免各实例各自从 localStorage 载入快照后 last-write-wins 互相覆盖。
const FILE_CACHE_STORAGE_KEY = 'fastclaw_conv_file_cache'

function loadFileCacheFromStorage(): Map<string, ChatAttachment[]> {
  try {
    const raw = localStorage.getItem(FILE_CACHE_STORAGE_KEY)
    if (raw) {
      const entries: [string, ChatAttachment[]][] = JSON.parse(raw)
      return new Map(entries)
    }
  } catch (e) {
    console.warn('[ChatStore] 文件缓存恢复失败，将使用空缓存', e)
  }
  return new Map()
}

function saveFileCacheToStorage() {
  try {
    const entries = [...convFileCache.value.entries()]
    localStorage.setItem(FILE_CACHE_STORAGE_KEY, JSON.stringify(entries))
  } catch (e) {
    console.warn('[ChatStore] 文件缓存写入 localStorage 失败', e)
  }
}

const convFileCache = ref<Map<string, ChatAttachment[]>>(loadFileCacheFromStorage())

// ─── 会话消息共享缓存（模块级）────────────────────────────
// conversationId → 已解析的 Message[]（数组引用：本地就地修改自动同步；替换引用处需显式 set）。
// 加载一次、解析一次，多 Tab 复用，避免切会话反复重拉 + 重跑字段解析。
const sharedMessageCache = new Map<string, Message[]>()
// 监听器属于会话，不随发起请求的 Tab 销毁。owner 只决定最后关闭时由谁释放流。
const _streamCleanups = new Map<string, () => void>()
const streamLeases = new Map<string, AbortController>()
const streamStopWaits = new Map<AbortController, Promise<void>>()
const _streamMsgBuffers = reactive(new Map<string, Message[]>())
/** 与 buffer 键同步；用 ref&新 Set 替换，保证跨会话切换时 isLoading 一定触发重算 */
const streamingConversationIds = ref(new Set<string>())
const streamOwners = new Map<string, string>()
// conversationId → 是否还有更早的历史消息（游标分页；reactive 使 ChatView 可直接响应）。
const sharedHasMore = reactive(new Map<string, boolean>())

function setStreamBuffer(convId: string, msgs: Message[]) {
  _streamMsgBuffers.set(convId, msgs)
  if (streamingConversationIds.value.has(convId)) return
  const next = new Set(streamingConversationIds.value)
  next.add(convId)
  streamingConversationIds.value = next
}

function clearStreamBuffer(convId: string) {
  streamLeases.get(convId)?.abort()
  streamLeases.delete(convId)
  _streamMsgBuffers.delete(convId)
  if (!streamingConversationIds.value.has(convId)) return
  const next = new Set(streamingConversationIds.value)
  next.delete(convId)
  streamingConversationIds.value = next
}

// 游标分页：每页条数（与后端 get_messages_page 默认一致）。
const PAGE_SIZE = 50

export interface PreparedConversationMessages {
  conversationId: string
  messages: Message[]
  hasMore?: boolean
}

// ─── Web（非 Tauri）环境内存 mock（方案 9）──────────────────
// 无 Tauri 后端时，消息读写落到这张内存表，让「创建会话 → 发消息 → 切换 Tab」基本流程可跑通。
// 与 sharedMessageCache 同语义：loadMessages 命中后共享数组引用，push 就地生效、跨 Tab 不丢。
const webMockMessages = new Map<string, Message[]>()

function patchConversationTaskPlan(conversationId: string, enabled: boolean) {
  const conv = sharedConversations.value.find(c => c.id === conversationId)
  if (!conv) return
  let root: Record<string, unknown> = {}
  try {
    const parsed = conv.extra_config ? JSON.parse(conv.extra_config) : {}
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) root = parsed
  } catch {
    root = {}
  }
  conv.extra_config = JSON.stringify({ ...root, task_plan: enabled })
}

function parseExecutionMode(raw: unknown): 'auto' | 'manual' | 'step' {
  return raw === 'manual' || raw === 'step' ? raw : 'auto'
}

function conversationExecutionModeFromCache(sessionId: string): 'auto' | 'manual' | 'step' {
  const conv = sharedConversations.value.find(c => c.id === sessionId)
  try {
    const cfg = JSON.parse(conv?.extra_config || '{}') as Record<string, unknown>
    return parseExecutionMode(cfg.execution_mode ?? cfg.executionMode)
  } catch {
    return 'auto'
  }
}

function patchConversationExecutionMode(conversationId: string, mode: unknown) {
  const conv = sharedConversations.value.find(c => c.id === conversationId)
  if (!conv) return
  const next = parseExecutionMode(mode)
  let root: Record<string, unknown> = {}
  try {
    const parsed = conv.extra_config ? JSON.parse(conv.extra_config) : {}
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) root = parsed
  } catch {
    root = {}
  }
  conv.extra_config = JSON.stringify({ ...root, execution_mode: next })
}

function patchConversationRemoteFastPlan(conversationId: string, fastplanSessionId: string) {
  const conv = sharedConversations.value.find(c => c.id === conversationId)
  if (!conv) return
  let root: Record<string, unknown> = {}
  try {
    const parsed = conv.extra_config ? JSON.parse(conv.extra_config) : {}
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) root = parsed
  } catch {
    root = {}
  }
  const entryId = `fastplan:remote:${fastplanSessionId}`
  const previous = root.fastplan && typeof root.fastplan === 'object' && !Array.isArray(root.fastplan)
    ? root.fastplan as Record<string, unknown>
    : { version: 2, revision: 0, entries: [] }
  const entries = Array.isArray(previous.entries) ? [...previous.entries] : []
  const idx = entries.findIndex((item: any) => item && (item.sessionId === fastplanSessionId || item.id === entryId))
  const entry = { id: entryId, sessionId: fastplanSessionId, status: 'active' }
  if (idx >= 0) entries[idx] = { ...(entries[idx] as object), ...entry }
  else entries.push(entry)
  conv.extra_config = JSON.stringify({
    ...root,
    task_plan: true,
    fastplan_remote: true,
    fastplan_session_id: fastplanSessionId,
    fastplan: { ...previous, version: 2, activeEntryId: entryId, entries },
  })
}

function parseRemoteAttachments(raw: unknown): ChatAttachment[] | undefined {
  if (!raw) return undefined
  let value: unknown = raw
  if (typeof value === 'string') {
    try { value = JSON.parse(value) } catch { return undefined }
  }
  if (!Array.isArray(value) || value.length === 0) return undefined
  return value as ChatAttachment[]
}

async function hydrateRemoteAttachments(sessionId: string, messageId: string): Promise<ChatAttachment[] | undefined> {
  const invoke = await getInvoke()
  if (!invoke) return undefined
  try {
    const raw = await invoke<(Message & { attachments?: string | any[] })[]>('get_messages', {
      conversationId: sessionId,
      limit: 20,
    })
    const row = raw.find(m => m.id === messageId)
    return parseRemoteAttachments(row?.attachments)
  } catch {
    return undefined
  }
}

function findLastStreamingAssistant(list: Message[]): Message | undefined {
  for (let i = list.length - 1; i >= 0; i--) {
    const msg = list[i]
    if (msg.role === 'assistant' && msg.status === 'streaming') return msg
  }
  return undefined
}

// ─── 远程控制事件：模块级单订阅 + 写共享缓存 ─────────────────
// 原实现：每个 chat 实例各自 startRemoteEventListener，handler 数为 N，同一广播被
// N 次空跑；且写入目标是 per-instance messages。
// 现收敛为：模块级只注册 1 个 handler，直接写 sharedMessageCache（方案 1 已让
// messages.value 与缓存数组共享引用，当前显示该会话的 tab 会随之自动响应）。
// 需要 UI 状态重置的场景（会话删除/截断/新建）通过遍历活跃实例完成。

interface ChatStoreRemoteHandle {
  currentConversationId: { value: string | null }
  messages: { value: Message[] }
  loadMessages: (conversationId: string) => Promise<void>
  removeLoadingConv: (convId: string) => void
}

// 活跃 chat store 实例注册表：createChatStore 注册、disposeChatStore 注销。
const activeChatStores = new Map<string, ChatStoreRemoteHandle>()

function forEachActiveChatStore(fn: (store: ChatStoreRemoteHandle, instanceId: string) => void) {
  for (const [instanceId, store] of activeChatStores) {
    try { fn(store, instanceId) } catch { /* 实例异常跳过 */ }
  }
}

function isSessionVisible(sessionId: string): boolean {
  for (const store of activeChatStores.values()) {
    if (store.currentConversationId.value === sessionId) return true
  }
  return false
}

/**
 * 解析后端返回的原始消息行：补全缺失的 group_id、反序列化 attachments/extra_data/kb_references、
 * 追加停止标记、修正 error 状态。
 * 从 loadMessages 内联逻辑提取，供首屏加载与「加载更早」复用（避免重复解析）。
 */
function parseMessages(raw: (Message & { attachments?: string | any[] })[]): Message[] {
  let pendingGid: string | null = null
  for (const msg of raw) {
    // 为缺失 group_id 的历史消息补全配对：相邻 user+AI 共享同一个 group_id
    if (!msg.group_id) {
      if (msg.role === 'user') {
        pendingGid = crypto.randomUUID()
        msg.group_id = pendingGid
      } else if (msg.role === 'assistant' && pendingGid) {
        msg.group_id = pendingGid
        pendingGid = null
      }
    } else {
      pendingGid = null
    }
    // 手动停止的消息：回显时追加停止标记
    if (msg.role === 'assistant' && msg.status === 'canceled') {
      msg.content = msg.content + '\n\n---\n\n🛑 **已停止**'
    }
    // 错误消息：确保 status 为 error（兼容旧数据未写 status 的情况）
    if (msg.role === 'assistant' && msg.status !== 'error' && typeof msg.content === 'string') {
      if (
        msg.content.startsWith('错误:') ||
        msg.content.startsWith('错误：') ||
        msg.content.includes('\n⚠️ 错误:') ||
        msg.content.includes('\n---\n⚠️ ')
      ) {
        msg.status = 'error'
      }
    }
    // 解析 attachments 字段（后端返回 JSON 字符串，需要反序列化）
    if (msg.role === 'user' && typeof msg.attachments === 'string' && msg.attachments) {
      try {
        msg.attachments = JSON.parse(msg.attachments)
      } catch {
        msg.attachments = undefined
      }
    }
    // 解析 extra_data：恢复联网搜索 / 工具调用 / Skill 标签等
    if (typeof (msg as any).extra_data === 'string' && (msg as any).extra_data) {
      try {
        const ed = JSON.parse((msg as any).extra_data)
        if (msg.role === 'assistant') {
          if (ed && Array.isArray(ed.web_hits)) {
            msg.web_hits = ed.web_hits
            msg.web_search_query = ed.web_search_query
            msg.web_searching = false
          }
          // 恢复工具调用卡片（refresh 后从 DB 重建 tool_calls）
          if (ed && Array.isArray(ed.tool_calls) && ed.tool_calls.length > 0) {
            msg.tool_calls = ed.tool_calls
          }
          // 恢复证据台账与系统提示语（refresh 后仍要能看到"这个数字没有出处"）
          if (ed && ed.grounding && typeof ed.grounding === 'object') {
            msg.grounding = ed.grounding
          }
          if (ed && typeof ed.grounding_note === 'string' && ed.grounding_note) {
            msg.grounding_note = ed.grounding_note
          }
        }
        // 用户/助手消息都恢复 Skill 标签（用户选择的 skill 挂在用户消息上）
        if (ed && ed.skill_used && typeof ed.skill_used.name === 'string') {
          msg.skill_used = { name: ed.skill_used.name, args: ed.skill_used.args }
        }
        // 远程任务规划过程卡（App 同款 planRuntime）
        hydrateMessagePlanRuntime(msg, ed)
      } catch {
        /* ignore malformed extra_data */
      }
    }
    // 解析 kb_references：恢复知识库溯源数据
    if (msg.role === 'assistant' && typeof (msg as any).kb_references === 'string' && (msg as any).kb_references) {
      try {
        const refs = JSON.parse((msg as any).kb_references)
        if (Array.isArray(refs) && refs.length > 0) {
          msg.trace_source = buildTraceSource(refs)
        }
      } catch {
        /* ignore malformed kb_references */
      }
    }
  }
  return raw as Message[]
}

export type AnswerChatStore = {
  currentConversationId: string | null
  messages: Message[]
  isLoading: boolean
  regenerateMessage: ReturnType<typeof createChatStore>['regenerateMessage']
  addModelAnswer: ReturnType<typeof createChatStore>['addModelAnswer']
  stopStream: ReturnType<typeof createChatStore>['stopStream']
}

function createChatStore(instanceId: string) {
  const ACTIVE_CONV_KEY = `fastclaw-active-conversation:${instanceId}`
  const restoredId = getChatTabConversation(chatTabPath(instanceId))
  const conversations = sharedConversations
  const currentConversationId = ref<string | null>(restoredId)
  const messages = ref<Message[]>([])
  const executionActivity = useExecutionActivityStore()
  const tabPath = chatTabPath(instanceId)
  const preparingMessage = ref(false)
  let pendingMessage: { conversationId: string | null; cancel(): void } | null = null

  function updateMsg(id: string, patch: Partial<Message>) {
    const idx = messages.value.findIndex(m => m.id === id)
    if (idx >= 0) messages.value[idx] = { ...messages.value[idx], ...patch }
  }

  // ─── 对话级加载状态（替代全局 isLoading）───
  const loadingConversationIds = ref<Set<string>>(new Set())
  /** 只读：仅当「当前激活会话」有活跃流时为 true，外部代码直接读取 */
  const isLoading = computed(() => {
    if (preparingMessage.value) return true
    const cid = currentConversationId.value
    return cid
      ? streamingConversationIds.value.has(cid) || loadingConversationIds.value.has(cid)
      : false
  })
  function isConversationStreaming(conversationId: string): boolean {
    return streamingConversationIds.value.has(conversationId)
  }
  /** 只读：当前激活会话是否还有更早的历史消息可加载（游标分页） */
  const hasMore = computed(() => {
    const cid = currentConversationId.value
    return cid ? !!sharedHasMore.get(cid) : false
  })

  function _addLoadingConv(convId: string) {
    const next = new Set(loadingConversationIds.value)
    next.add(convId)
    loadingConversationIds.value = next
    executionActivity.update(convId, tabPath, 'chat', {
      phase: 'executing',
      protected: true,
    })
  }
  function _removeLoadingConv(convId: string) {
    if (!_streamMsgBuffers.has(convId)) streamOwners.delete(convId)
    const next = new Set(loadingConversationIds.value)
    next.delete(convId)
    loadingConversationIds.value = next
    executionActivity.remove(convId, 'chat')
  }
  function _clearAllLoading() {
    for (const convId of loadingConversationIds.value) executionActivity.remove(convId, 'chat')
    loadingConversationIds.value = new Set()
  }

  /** 移除指定会话的流监听器 + 缓冲区 + loading（不传参 = 当前会话） */
  function cleanupStreamListeners(convId?: string) {
    const cid = convId || currentConversationId.value
    if (!cid) return
    if (docStream.value.conversationId === cid) clearDocEditStream()
    _streamCleanups.get(cid)?.()
    _streamCleanups.delete(cid)
    clearStreamBuffer(cid)
    _removeLoadingConv(cid)
  }

  // ─── 文档编辑模式：流式内容实时截断（绑定会话，避免跨会话污染） ───
  const docStream = ref<{ marker: string | null; raw: string; done: boolean; conversationId: string | null }>({
    marker: null, raw: '', done: false, conversationId: null,
  })
  let docEditRequest: DocumentEditRequest | null = null
  function activateDocEditStream(marker: string, conversationId?: string | null) {
    docEditRequest?.cancel()
    docEditRequest = createDocumentEditRequest(getChatTabState(instanceId), conversationId ?? currentConversationId.value)
    const request = docEditRequest
    docStream.value = {
      marker,
      raw: '',
      done: false,
      conversationId: conversationId ?? currentConversationId.value,
    }
    return () => cancelDocumentRequest(request)
  }
  function cancelDocumentRequest(request: DocumentEditRequest | null) {
    if (!request) return
    if (docEditRequest === request) clearDocEditStream()
    else request.cancel()
  }
  function getDocStreamRaw() { return docStream.value.raw }
  function clearDocEditStream() {
    docEditRequest?.cancel()
    docEditRequest = null
    docStream.value = { marker: null, raw: '', done: false, conversationId: null }
  }

  // 持久化当前对话 ID（sessionStorage：刷新不丢失，关程序清除）
  watch(currentConversationId, (id, previous) => {
    syncChatTabStateConversation(instanceId, id, previous)
    setChatTabConversation(chatTabPath(instanceId), id)
    if (id) sessionStorage.setItem(ACTIVE_CONV_KEY, id)
    else sessionStorage.removeItem(ACTIVE_CONV_KEY)
  }, { immediate: true, flush: 'sync' })

  async function loadConversations(force = false) {
    if (force) bumpConversationsRevision()
    await loadSharedConversations()
  }

  async function createConversation(
    title?: string,
    modelId?: string,
    assistantId?: string,
    mode?: string,
    preserveMessages = false,
    isCurrent: () => boolean = () => true,
    beforeCommit?: (id: string) => void,
  ) {
    const invoke = await getInvoke()
    if (!isCurrent()) throw new DOMException('发送已取消', 'AbortError')
    if (invoke) {
      try {
        const id = await invoke<string>('create_conversation', { title, modelId, assistantId, mode })
        if (!isCurrent()) throw new DOMException('发送已取消', 'AbortError')
        await loadConversations(true)
        if (!isCurrent()) throw new DOMException('发送已取消', 'AbortError')
        beforeCommit?.(id)
        currentConversationId.value = id
        if (!preserveMessages) messages.value = []
        return id
      } catch (err) {
        if (!isCurrent()) throw new DOMException('发送已取消', 'AbortError')
        console.warn('Tauri create_conversation failed', err)
        // A guarded send must not silently change a failed desktop create into a mock conversation.
        if (beforeCommit) throw err
      }
    }
    // Web fallback
    const mockId = crypto.randomUUID()
    const conv: Conversation = {
      id: mockId,
      title: title || '智能对话',
      model_id: modelId || null,
      assistant_id: assistantId || null,
      mode: (mode as Conversation['mode']) || 'normal',
      is_pinned: false,
      is_archived: false,
      updated_at: new Date().toISOString(),
    }
    conversations.value.unshift(conv)
    beforeCommit?.(mockId)
    currentConversationId.value = mockId
    if (!preserveMessages) messages.value = []
    // 建立内存 mock 引用：后续 push 就地反映，切换 Tab 后 loadMessages 仍能读到（方案 9）。
    webMockMessages.set(mockId, messages.value)
    return mockId
  }

  /** Ensure persistence exists without inserting a visible user message. */
  async function ensureConversation(options: {
    title?: string
    modelId?: string
    assistantId?: string
    mode?: string
    preserveMessages?: boolean
  } = {}) {
    if (currentConversationId.value) return currentConversationId.value
    return createConversation(
      options.title,
      options.modelId,
      options.assistantId,
      options.mode,
      options.preserveMessages ?? true,
    )
  }

  /**
   * Detach this tab from its selected persisted conversation and return to the
   * lazy new-conversation state. This does not delete or mutate the previous
   * conversation. Callers must stop an active stream before detaching.
   */
  function detachCurrentConversation(expectedConversationId?: string): boolean {
    if (expectedConversationId && currentConversationId.value !== expectedConversationId) return false
    currentConversationId.value = null
    messages.value = []
    return true
  }

  // 加载序列号：取代「精确比对 currentConversationId」。任何新的 loadMessages 调用
  // 都会使旧的 in-flight 请求失效，快速切换会话时避免过期结果串话。
  let loadSeq = 0

  /**
   * 在不改变当前会话/可见消息的前提下准备目标会话。
   * ChatView 用它实现“旧内容保留到新内容就绪”的原子切换。
   */
  async function prepareMessages(conversationId: string): Promise<PreparedConversationMessages | null> {
    // 1) 流缓冲优先（正在流式的会话，切回保留实时内容）
    const streamBuf = _streamMsgBuffers.get(conversationId)
    if (streamBuf) {
      return { conversationId, messages: streamBuf, hasMore: sharedHasMore.get(conversationId) }
    }
    // 2) 已加载过的会话直接复用共享缓存，不重拉、不重解析
    const cached = sharedMessageCache.get(conversationId)
    if (cached) {
      return { conversationId, messages: cached, hasMore: sharedHasMore.get(conversationId) }
    }
    const invoke = await getInvoke()
    if (!invoke) {
      // Web fallback：无 Tauri 后端，从内存 mock 读取（首次访问初始化空数组，共享引用）。
      const mock = webMockMessages.get(conversationId) ?? []
      webMockMessages.set(conversationId, mock)
      return { conversationId, messages: mock, hasMore: false }
    }
    try {
      const raw = await invoke<(Message & { attachments?: string | any[] })[]>('get_messages', { conversationId, limit: PAGE_SIZE })
      const parsed = deletedMessageLedger.prune(conversationId, parseMessages(raw))
      sharedMessageCache.set(conversationId, parsed)
      return { conversationId, messages: parsed, hasMore: raw.length >= PAGE_SIZE }
    } catch (err) {
      console.warn('Failed to load messages', err)
      return null
    }
  }

  /** 唯一的会话消息提交点；提交前由调用方完成过期请求校验。 */
  function commitPreparedMessages(prepared: PreparedConversationMessages) {
    const conversationId = prepared.conversationId
    currentConversationId.value = conversationId
    const activeBuffer = _streamMsgBuffers.get(conversationId)
    messages.value = deletedMessageLedger.prune(conversationId, activeBuffer || prepared.messages)
    sharedMessageCache.set(conversationId, messages.value)
    if (prepared.hasMore !== undefined) sharedHasMore.set(conversationId, prepared.hasMore)
  }

  async function loadMessages(conversationId: string) {
    const seq = ++loadSeq
    // 保留旧 API 的即时选中语义；需要无白屏切换的页面使用
    // prepareMessages + commitPreparedMessages。
    currentConversationId.value = conversationId
    const prepared = await prepareMessages(conversationId)
    if (!prepared || seq !== loadSeq || currentConversationId.value !== conversationId) return
    commitPreparedMessages(prepared)
  }

  // 滚动到顶：按游标加载更早一页并 prepend（游标分页，见方案 3）
  async function loadMoreMessages(conversationId: string) {
    // 流式中的会话不加载历史（避免 prepend 后流缓冲引用失配导致 token 不落 UI）
    if (_streamMsgBuffers.has(conversationId)) return
    const arr = sharedMessageCache.get(conversationId)
    if (!arr || arr.length === 0) return
    const invoke = await getInvoke()
    if (!invoke) return
    const first = arr[0]
    const seq = ++loadSeq
    try {
      const older = await invoke<(Message & { attachments?: string | any[] })[]>('get_messages_page', {
        conversationId,
        beforeCreatedAt: first.created_at,
        beforeId: first.id,
        limit: PAGE_SIZE,
      })
      if (seq !== loadSeq || currentConversationId.value !== conversationId) return
      const parsed = deletedMessageLedger.prune(conversationId, parseMessages(older))
      sharedHasMore.set(conversationId, older.length >= PAGE_SIZE)
      const merged = deletedMessageLedger.prune(conversationId, [...parsed, ...arr])
      sharedMessageCache.set(conversationId, merged)
      if (currentConversationId.value === conversationId) {
        messages.value = merged
      }
    } catch (err) {
      console.warn('Failed to load more messages', err)
    }
  }

  // 流式输出核心逻辑（Tauri + Web 回退），供 sendMessage / addModelAnswer 复用
  async function streamResponse(aiMsg: Message, content: string, modelId?: string, knowledgeBaseIds?: string[], attachments?: any[], groupId?: string, skipUserMessage?: boolean, systemPrompt?: string, localKbPaths?: { path: string; recursive: boolean }[], fileIds?: string[], webSearch?: boolean, searchProvider?: string, images?: string[], allAttachmentsJson?: string, enableThinking?: boolean, mcpTools?: boolean, mcpServerIds?: string[], ocrContext?: string, skillName?: string, enabledSkills?: string[], isRetry = false, replaceMessageId?: string, target?: { conversationId: string; messages: Message[] }) {
    const convId = target?.conversationId || currentConversationId.value!
    const streamLease = new AbortController()
    streamLeases.get(convId)?.abort()
    streamLeases.set(convId, streamLease)
    const isActiveStream = () => streamLeases.get(convId) === streamLease && !streamLease.signal.aborted
    const requestOwner = getChatTabState(instanceId), ownerCurrent = requestOwner.capture()
    const documentRequest = docEditRequest?.bind(convId) ? docEditRequest : null
    if (documentRequest) docStream.value.conversationId = convId
    const finishDocument = (message?: Message, finalContent?: string) => {
      if (!documentRequest) return
      const intro = documentRequest.finish(convId, message?.id, finalContent)
      if (docEditRequest === documentRequest) clearDocEditStream()
      if (message && intro !== null) {
        message.content = intro
        // Capture the final database message id, not an index in the visible conversation.
        const id = message.id
        void getInvoke().then(invoke => invoke?.('update_message_content', { id, content: intro }))
          .catch(error => console.warn('[chat] persist document response failed', error))
      }
    }
    // 在第一个 await 前固定消息数组；注册监听期间切换会话也不能换掉流的目标。
    const streamMessages = target?.messages || messages.value
    setStreamBuffer(convId, streamMessages)
    executionActivity.update(convId, tabPath, 'chat', {
      phase: 'executing',
      protected: true,
    })
    markConversationViewed(convId)
    sharedMessageCache.set(convId, streamMessages)
    streamOwners.set(convId, instanceId)
    const invoke = await getInvoke()
    if (!isActiveStream()) return
    if (invoke) {
      // 提前声明，确保 catch 块也能访问以清理监听器（避免 listener 泄漏）
      let cleanup = () => {}
      let terminalStarted = false
      const finishStream = () => {
        cleanup()
        if (!isActiveStream()) return
        _streamCleanups.delete(convId)
        clearStreamBuffer(convId)
        _removeLoadingConv(convId)
      }
      // 密钥恢复流程的重入保护（per-call 闭包变量，防同流内 error/catch 重复触发）

      // 共享的最终错误持久化助手（恢复失败 / 既有 error 兜底复用）
      const persistErrorMsg = async (convIdArg: string, errText: string, msg: Message) => {
        try {
          const invokeFn = await getInvoke()
          if (invokeFn && isActiveStream()) {
            const savedId = await abortable(invokeFn<string>('save_assistant_message', {
              conversationId: convIdArg,
              content: `错误: ${errText}`,
              groupId: msg.group_id || groupId || null,
              modelId: msg.model_id || modelId || null,
              status: 'error',
            }), streamLease.signal)
            const i = streamMessages.findIndex(m => m.id === msg.id)
            if (isActiveStream() && savedId && i >= 0) streamMessages[i] = { ...streamMessages[i], id: savedId }
          }
        } catch (e) {
          console.warn('[chat] persist error message failed', e)
        }
      }

      try {
        const { listen: rawListen } = await import('@tauri-apps/api/event')
        const listen = async <T>(name: string, handler: (event: { payload: T }) => unknown) => {
          if (!isActiveStream()) return () => {}
          const off = await rawListen<T>(name, event => {
            if (isActiveStream() && !terminalStarted) return handler(event)
          })
          if (!isActiveStream()) off()
          return off
        }
        let rawBuffer = ''
        let inThink = false
        // 边注册边可清理：任一 listen 中途失败时，catch 里 cleanup() 仍能卸掉已注册的监听器
        const unlistens: Array<() => void> = []
        cleanup = () => {
          while (unlistens.length) {
            try { unlistens.pop()!() } catch { /* ignore */ }
          }
        }

        unlistens.push(await listen<{ conversation_id: string }>('chat-stream-reset', (event) => {
          if (event.payload.conversation_id !== convId) return
          const buf = _streamMsgBuffers.get(convId)
          const idx = buf?.findIndex(m => m.id === aiMsg.id) ?? -1
          if (!buf || idx < 0) return
          rawBuffer = ''; inThink = false
          if (docStream.value.conversationId === convId) {
            docStream.value.raw = ''; docStream.value.done = false
          }
          buf[idx] = {
            ...buf[idx], content: '', think_content: undefined, think_show: undefined,
            isThinking: true,
            stream_phase: buf[idx].stream_phase === 'finalizing' ? 'finalizing' : 'waiting',
          }
        }))

        unlistens.push(await listen<{ token: string; conversation_id?: string; tool_call?: ToolCallInfo }>('chat-stream-token', (event) => {
          // 会话隔离：仅处理发起本次流的会话事件
          if (event.payload.conversation_id && event.payload.conversation_id !== convId) return
          const buf = _streamMsgBuffers.get(convId)
          if (!buf) return
          const idx = buf.findIndex(m => m.id === aiMsg.id)
          if (idx < 0) return

          // 检测 BERT 意图路由的工具调用标记
          if (event.payload.tool_call && !buf[idx].tool_call) {
            buf[idx] = {
              ...buf[idx],
              tool_call: event.payload.tool_call,
            }
          }

          rawBuffer += event.payload.token
          const parsed = parseChatStreamText(rawBuffer)
          let displayContent = parsed.content
          const thinkContent = parsed.thinkContent
          inThink = parsed.inThink
          // 文档编辑只截断显示正文；当前轮完整正文仍供文档编辑器读取。
          const isDocOwner = !!docStream.value.marker && docStream.value.conversationId === convId
          if (isDocOwner) {
            docStream.value.raw = displayContent
            const si = displayContent.indexOf(docStream.value.marker!)
            if (si !== -1) {
              docStream.value.done = true
              displayContent = displayContent.substring(0, si).trim() + '\n\n> ⏳ 正在生成文档修改内容...'
            }
          }
          buf[idx] = {
            ...buf[idx],
            content: displayContent,
            think_content: thinkContent || undefined,
            think_show: buf[idx].think_show ?? !!thinkContent,
            isThinking: !displayContent && !thinkContent.trim(),
            stream_phase: inThink ? 'reasoning' : (displayContent || thinkContent) ? 'answering' : buf[idx].stream_phase,
            retry_progress: undefined,
          }
        }))

        // 等待状态不等同于模型思考内容；所有终态均收尾。
        unlistens.push(await listen<{ conversation_id?: string; status: string }>('chat-stream-status', (event) => {
          if (event.payload.conversation_id && event.payload.conversation_id !== convId) return
          const buf = _streamMsgBuffers.get(convId)
          if (!buf) return
          const idx = buf.findIndex(m => m.id === aiMsg.id)
          if (idx < 0) return
          if (event.payload.status === 'thinking' || event.payload.status === 'finalizing') {
            buf[idx] = {
              ...buf[idx], isThinking: !buf[idx].content && !buf[idx].think_content?.trim(),
              stream_phase: event.payload.status === 'finalizing' ? 'finalizing' : 'waiting',
            }
          } else if (event.payload.status === 'idle') {
            buf[idx] = { ...buf[idx], isThinking: false, stream_phase: undefined }
          }
        }))

        // LLM 内部自动重试进度：只更新本会话的临时 AI 消息，不写入正文或数据库。
        unlistens.push(await listen<{
          conversation_id?: string
          attempt: number
          max_retries: number
          reason?: string
        }>('chat-stream-retry', (event) => {
          if (event.payload.conversation_id && event.payload.conversation_id !== convId) return
          const buf = _streamMsgBuffers.get(convId)
          if (!buf) return
          const idx = buf.findIndex(m => m.id === aiMsg.id)
          if (idx < 0) return
          buf[idx] = {
            ...buf[idx],
            isThinking: false,
            stream_phase: undefined,
            retry_progress: {
              attempt: event.payload.attempt,
              total: event.payload.max_retries,
            },
          }
        }))

        // 工具调用可见化：监听结构化 chat-tool-call 事件（running→done/error）
        unlistens.push(await listen<{
          conversation_id?: string
          id: string; name: string; kind?: 'builtin' | 'mcp' | 'skill'; args?: Record<string, unknown>
          status: 'running' | 'done' | 'error'; result_preview?: string
          title?: string; metadata?: unknown; artifacts?: unknown[]; duration_ms?: number
        }>('chat-tool-call', (event) => {
          if (event.payload.conversation_id && event.payload.conversation_id !== convId) return
          const buf = _streamMsgBuffers.get(convId)
          if (!buf) return
          const idx = buf.findIndex(m => m.id === aiMsg.id)
          if (idx < 0) return
          // 收到首个工具调用事件时关闭 thinking 状态
          buf[idx] = { ...buf[idx], isThinking: false }
          const existing = buf[idx].tool_calls || []
          const found = existing.findIndex(tc => tc.id === event.payload.id)
          if (found >= 0) {
            existing[found] = { ...existing[found], ...event.payload }
          } else {
            existing.push({
              id: event.payload.id, name: event.payload.name, kind: event.payload.kind,
              args: event.payload.args, status: event.payload.status,
              result_preview: event.payload.result_preview,
              title: event.payload.title, metadata: event.payload.metadata,
              artifacts: event.payload.artifacts, duration_ms: event.payload.duration_ms,
            })
          }
          buf[idx] = { ...buf[idx], tool_calls: [...existing] }
        }))

        unlistens.push(await listen<MessageResponse & { stream_error?: string; stopped?: boolean; conversation_id?: string; skill_used?: { name: string; args?: string } | null; grounding?: GroundingManifest | null; grounding_note?: string | null }>('chat-stream-done', async (event) => {
          if (event.payload.conversation_id && event.payload.conversation_id !== convId) return
          terminalStarted = true
          let replacementReady = false
          const buf = _streamMsgBuffers.get(convId)
          if (buf) {
            const idx = buf.findIndex(m => m.id === aiMsg.id)
            if (idx >= 0) {
              // 如果流中断，在内容末尾追加错误标记（保留已有内容）
              if (!event.payload.stream_error && !event.payload.stopped && buf[idx].status === 'streaming') {
                markConversationCompleted(convId)
              }
              const finalContent = event.payload.stream_error
                ? (event.payload.content || '') + `\n\n---\n⚠️ ${event.payload.stream_error}`
                : event.payload.content
              // 保留工具调用卡片（tool_calls）+ Skill 标签，不被 DB 正文冲掉；关闭 thinking
              const preservedToolCalls = buf[idx].tool_calls
              const preservedSkillUsed = buf[idx].skill_used || event.payload.skill_used || undefined
              // 证据台账：优先用事件里的（刚算出来的），否则保留已有的
              const grounding = event.payload.grounding ?? buf[idx].grounding
              const groundingNote = event.payload.grounding_note ?? buf[idx].grounding_note
              buf[idx] = {
                ...buf[idx],
                id: event.payload.message_id || buf[idx].id,
                content: finalContent ?? buf[idx].content,
                think_content: 'think_content' in event.payload ? event.payload.think_content || undefined : buf[idx].think_content,
                model_id: buf[idx].model_id || event.payload.model,
                token_input: event.payload.token_input,
                token_output: event.payload.token_output,
                status: event.payload.stopped ? 'canceled' : event.payload.stream_error ? 'error' : 'completed',
                trace_source: buildTraceSource(event.payload.references || []),
                tool_calls: preservedToolCalls,
                skill_used: preservedSkillUsed,
                grounding: grounding || undefined,
                grounding_note: groundingNote || undefined,
                isThinking: false,
                stream_phase: undefined,
                retry_progress: undefined,
              }
              finishDocument(
                !event.payload.stream_error && !event.payload.stopped ? buf[idx] : undefined,
                event.payload.content ?? (docEditRequest === documentRequest ? docStream.value.raw : undefined),
              )
              replacementReady = !!event.payload.message_id && !event.payload.stream_error && !event.payload.stopped
            }
          }
          finishDocument()
          // 流结束：把最终消息快照写回共享缓存，切走再切回不再重拉
          if (buf) sharedMessageCache.set(convId, buf)
          if (replaceMessageId && replacementReady && event.payload.message_id !== replaceMessageId && isActiveStream()) {
            try {
              await abortable(invoke('delete_message', { id: replaceMessageId }), streamLease.signal)
              if (isActiveStream()) {
                const index = streamMessages.findIndex(message => message.id === replaceMessageId)
                if (index >= 0) streamMessages.splice(index, 1)
              }
            } catch (error) {
              if (isActiveStream() && ownerCurrent()) requestOwner.state.chatSubmissionError.value =
                `新回答已保存，旧回答清理失败：${error instanceof Error ? error.message : String(error)}`
            }
          }
          finishStream()
        }))

        unlistens.push(await listen<{ error: string; canceled?: boolean; conversation_id?: string }>('chat-stream-error', async (event) => {
          if (event.payload.conversation_id && event.payload.conversation_id !== convId) return
          const buf = _streamMsgBuffers.get(convId)
          if (buf) {
            const idx = buf.findIndex(m => m.id === aiMsg.id)
            if (idx >= 0) {
              if (event.payload.canceled) {
                // 手动停止：保留已有内容，仅追加停止标记（落库由 stop_stream 负责）
                const currentContent = buf[idx].content || ''
                buf[idx] = {
                  ...buf[idx],
                  status: 'canceled',
                  content: currentContent + '\n\n---\n\n🛑 **已停止**',
                  isThinking: false,
                  stream_phase: undefined,
                  web_searching: false,
                  retry_progress: undefined,
                }
              } else {
                const errText = event.payload.error
                // ★ 401 密钥失效恢复：命中则接管（含清理 + 重取密钥 + 重试一次）
                if (!isActiveStream() || terminalStarted) return
                terminalStarted = true
                const errContent = `错误: ${errText}`
                buf[idx] = { ...buf[idx], status: 'error', content: errContent, isThinking: false, stream_phase: undefined, retry_progress: undefined }
                // 持久化错误回答，刷新/切换后仍可回显
                try {
                  const invokeFn = await getInvoke()
                  if (invokeFn && isActiveStream()) {
                    const savedId = await abortable(invokeFn<string>('save_assistant_message', {
                      conversationId: convId,
                      content: errContent,
                      groupId: aiMsg.group_id || groupId || null,
                      modelId: aiMsg.model_id || modelId || null,
                      status: 'error',
                    }), streamLease.signal)
                    if (isActiveStream() && savedId) buf[idx] = { ...buf[idx], id: savedId }
                  }
                } catch (e) {
                  console.warn('[chat] persist error message failed', e)
                }
              }
            }
          }
          if (!isActiveStream()) { cleanup(); return }
          finishDocument()
          // 流结束：把最终消息快照写回共享缓存，切走再切回不再重拉
          if (buf) sharedMessageCache.set(convId, buf)
          finishStream()
        }))

        // 联网搜索：开始（进入「搜索中」状态）
        unlistens.push(await listen<{ query: string; conversation_id?: string }>('chat-web-search-start', (event) => {
          if (event.payload.conversation_id && event.payload.conversation_id !== convId) return
          const buf = _streamMsgBuffers.get(convId)
          if (!buf) return
          const idx = buf.findIndex(m => m.id === aiMsg.id)
          if (idx < 0) return
          buf[idx] = {
            ...buf[idx],
            web_searching: true,
            web_search_query: event.payload.query,
          }
        }))

        // 联网搜索：命中结果（挂到当前 AI 消息上，供内联指示器 + 右侧来源面板使用）
        unlistens.push(await listen<{ query: string; hits: WebHit[]; error?: string; conversation_id?: string }>('chat-web-search', (event) => {
          if (event.payload.conversation_id && event.payload.conversation_id !== convId) return
          const buf = _streamMsgBuffers.get(convId)
          if (!buf) return
          const idx = buf.findIndex(m => m.id === aiMsg.id)
          if (idx < 0) return
          buf[idx] = {
            ...buf[idx],
            web_searching: false,
            web_search_query: event.payload.query,
            web_hits: event.payload.hits || [],
          }
        }))

        // 最后一个 Tab 可能在监听注册期间已关闭/停止，禁止随后启动孤立请求。
        const stopping = streamStopWaits.get(streamLease)
        if (stopping) await abortable(stopping, streamLease.signal)
        if (!isActiveStream()) {
          finishDocument()
          cleanup()
          return
        }
        _streamCleanups.set(convId, cleanup)

        await invoke('send_message_stream', {
          conversationId: convId,
          content,
          modelId,
          knowledgeBaseIds: knowledgeBaseIds || [],
          fileIds: fileIds || [],
          attachmentFileIds: (attachments || []).filter(a => a.uploaded_file_id).map(a => a.uploaded_file_id),
          groupId: groupId || null,
          skipUserMessage: skipUserMessage || false,
          systemPrompt: systemPrompt || null,
          localKbPaths: localKbPaths || [],
          kbSearchApiUrl: DEFAULT_CONFIG.KB_SEARCH_API_URL,
          webSearch: webSearch ?? false,
          searchProvider: searchProvider ?? null,
          images: images || [],
          imageAttachmentsJson: allAttachmentsJson || null,
          enableThinking: enableThinking ?? false,
          mcpTools: mcpTools ?? true,
          mcpServerIds: mcpServerIds ?? [],
          enabledSkills: enabledSkills ?? [],
          ocrContext: ocrContext || null,
          skillName: skillName || null,
        })
      } catch (err: unknown) {
        if (!isActiveStream()) { cleanup(); return }
        if (terminalStarted) return
        const errMsg = err instanceof Error ? err.message : String(err)
        // ★ 401 密钥失效恢复（防御性兜底；send_message_stream 通常经事件返回 401）
        if (!isActiveStream()) { cleanup(); return }
        if (terminalStarted) return
        terminalStarted = true
        finishDocument()
        cleanup() // 修复 listener 泄漏
        const buf = _streamMsgBuffers.get(convId)
        if (buf) {
          const idx = buf.findIndex(m => m.id === aiMsg.id)
          if (idx >= 0) {
            // 保留 token listener 已累积的内容，追加错误标记
            const existing = buf[idx].content || ''
            const errContent = existing ? `${existing}\n\n---\n⚠️ 错误: ${errMsg}` : `错误: ${errMsg}`
            buf[idx] = { ...buf[idx], status: 'error', content: errContent, isThinking: false, stream_phase: undefined, retry_progress: undefined }
            // 持久化错误回答，刷新/切换后仍可回显
            try {
              const invokeFn = await getInvoke()
              if (invokeFn && isActiveStream()) {
                const savedId = await abortable(invokeFn<string>('save_assistant_message', {
                  conversationId: convId,
                  content: errContent,
                  groupId: aiMsg.group_id || groupId || null,
                  modelId: aiMsg.model_id || modelId || null,
                  status: 'error',
                }), streamLease.signal)
                if (isActiveStream() && savedId) buf[idx] = { ...buf[idx], id: savedId }
              }
            } catch (e) {
              console.warn('[chat] persist error message failed', e)
            }
          }
        }
        finishStream()
      }
    } else {
      finishDocument()
      // const mockResponse = generateMockResponse(content); let i = 0
      // const timer = setInterval(() => {
      //   const idx = messages.value.findIndex(m => m.id === aiMsg.id)
      //   if (idx < 0) { clearInterval(timer); isLoading.value = false; return }
      //   if (i >= mockResponse.length) {
      //     clearInterval(timer)
      //     messages.value[idx] = { ...messages.value[idx], status: 'completed', model_id: 'qwen3.5-397b-a17b', token_input: content.length * 2, token_output: mockResponse.length * 2 }
      clearStreamBuffer(convId)
      _removeLoadingConv(convId); return
      //   }
      //   const chunkSize = Math.min(1 + Math.floor(Math.random() * 4), mockResponse.length - i)
      //   messages.value[idx].content += mockResponse.slice(i, i + chunkSize); i += chunkSize
      // }, 15)
    }
  }

  async function sendMessage(content: string, modelId?: string, knowledgeBaseIds?: string[], attachments?: any[], groupId?: string, systemPrompt?: string, assistantId?: string, mode?: string, localKbPaths?: { path: string; recursive: boolean }[], fileIds?: string[], webSearch?: boolean, searchProvider?: string, images?: string[], enableThinking?: boolean, assistantDisplay?: { agent_name: string; agent_icon?: string | null }, mcpTools?: boolean, mcpServerIds?: string[], ocrContext?: string, skillName?: string, enabledSkills?: string[], providerCode?: string, request?: { signal: AbortSignal }) {
    if (isLoading.value || messageDeletionService.isPending(currentConversationId.value) || request?.signal.aborted) return false
    const pendingConversationId = currentConversationId.value
    const owner = getChatTabState(instanceId)
    const pendingDocument = docEditRequest
    const contextCurrent = owner.capture()
    if (!contextCurrent()) return false
    const controller = new AbortController()
    const draftMessages = messages.value
    const localMessageIds: string[] = []
    let targetId = pendingConversationId
    const activityId = `draft:${instanceId}:${crypto.randomUUID()}`
    const isCurrent = () => !controller.signal.aborted && contextCurrent()
      && currentConversationId.value === targetId && messages.value === draftMessages
    const cancel = () => {
      controller.abort()
      cancelDocumentRequest(pendingDocument)
      // Nothing has reached the transport yet. Do not leave phantom questions on retry.
      for (const id of localMessageIds) {
        const index = draftMessages.findIndex(message => message.id === id)
        if (index >= 0) draftMessages.splice(index, 1)
      }
      if (pendingMessage?.cancel === cancel) { pendingMessage = null; preparingMessage.value = false }
      executionActivity.remove(activityId, 'chat')
    }
    pendingMessage = { conversationId: pendingConversationId, cancel }
    preparingMessage.value = true
    executionActivity.update(activityId, tabPath, 'chat', { phase: 'submitting', protected: true })
    const removeInvalidation = owner.onInvalidate(cancel)
    const unwatch = watch(currentConversationId, id => { if (id !== targetId) cancel() }, { flush: 'sync' })
    request?.signal.addEventListener('abort', cancel, { once: true })
    const isFirstMessage = messages.value.length === 0
    const gid = groupId || crypto.randomUUID()

    // 将文档类附件持久化到消息上（不含一次性附件如图片/截图）
    const persistAttachments = (attachments || [])
      .filter((a) => a.type === 'document' && a.status === 'uploaded')
      .map((a) => ({
        id: a.id,
        name: a.name,
        type: a.type,
        size: a.size,
        uploaded_file_id: a.uploaded_file_id,
        uploaded_kb_id: a.uploaded_kb_id,
        status: a.status,
      }))

    // 图片附件（含 preview_url data URL）用于消息气泡显示
    const imageAttachments = (attachments || [])
      .filter((a) => (a.type === 'image' || a.type === 'screenshot') && a.preview_url)
      .map((a) => ({
        id: a.id,
        name: a.name,
        type: a.type as 'image' | 'screenshot',
        size: a.size,
        preview_url: a.preview_url,
        status: 'uploaded' as const,
      }))

    const allDisplayAttachments = [...persistAttachments, ...imageAttachments]

    // 序列化所有附件（文档+图片）为 JSON，供后端持久化，后续 loadMessages 时按消息精确回显
    const allAttachmentsJson = allDisplayAttachments.length > 0
      ? JSON.stringify(allDisplayAttachments)
      : undefined

    const userMsg: Message = {
      id: crypto.randomUUID(), conversation_id: pendingConversationId || '', role: 'user', content,
      model_id: null, token_input: null, token_output: null, status: 'completed',
      created_at: new Date().toISOString(), group_id: gid,
      attachments: allDisplayAttachments.length > 0 ? allDisplayAttachments : undefined,
      // 按钮选择的 skill 标签直接放在用户消息上，不在 AI 消息上突兀出现
      skill_used: skillName ? { name: skillName } : undefined,
    }
    messages.value.push(userMsg)

    const aiMsg: Message = {
      id: crypto.randomUUID(), conversation_id: pendingConversationId || '', role: 'assistant', content: '',
      model_id: modelId || null, token_input: null, token_output: null, status: 'streaming',
      created_at: new Date().toISOString(), group_id: gid,
      agent_name: assistantDisplay?.agent_name,
      agent_icon: assistantDisplay?.agent_icon || null,
      _providerCode: providerCode,
    }
    messages.value.push(aiMsg)

    // 先插入本地消息，再等待新会话落库。否则首次发送时会在等待
    // create_conversation / get_conversations 期间继续渲染欢迎页，然后突然切换为消息列表，造成闪烁。
    localMessageIds.push(userMsg.id, aiMsg.id)
    try {
      if (!pendingConversationId) {
        // Transfer the expected identity synchronously, only after the guarded create succeeds.
        const conversationId = await abortable(createConversation(firstMessageTitle(content), modelId, assistantId, mode, true,
          isCurrent, id => { targetId = id; if (pendingMessage?.cancel === cancel) pendingMessage.conversationId = id }), controller.signal)
        userMsg.conversation_id = conversationId
        aiMsg.conversation_id = conversationId
      } else if (isFirstMessage) {
        const currentConv = conversations.value.find(c => c.id === pendingConversationId)
        // 占位标题（智能对话 / 规划会话 / 新对话）才用首条问题命名
        if (currentConv && isPlaceholderTitle(currentConv.title)) {
          const title = firstMessageTitle(content)
          currentConv.title = title
          const invoke = await abortable(getInvoke(), controller.signal)
          if (invoke && isCurrent()) { try { await abortable(invoke('update_conversation', { id: pendingConversationId, title }), controller.signal) } catch { } }
        }
      }
      if (!isCurrent() || docEditRequest !== pendingDocument) { cancel(); return false }
      // Runtime establishes its stream buffer synchronously before its first await.
      removeInvalidation(); unwatch()
      request?.signal.removeEventListener('abort', cancel)
      if (pendingMessage?.cancel === cancel) { pendingMessage = null; preparingMessage.value = false }
      executionActivity.remove(activityId, 'chat')
      await streamResponse(aiMsg, content, modelId, knowledgeBaseIds, attachments, gid, false, systemPrompt, localKbPaths, fileIds, webSearch, searchProvider, images, allAttachmentsJson, enableThinking, mcpTools, mcpServerIds, ocrContext, skillName, enabledSkills)
      return true
    } catch (error) {
      cancelDocumentRequest(pendingDocument)
      if (controller.signal.aborted || !contextCurrent()) return false
      // No transport accepted this pair; retain the caller's draft for an explicit retry.
      for (const message of [userMsg, aiMsg]) {
        const index = draftMessages.findIndex(item => item.id === message.id)
        if (index >= 0) draftMessages.splice(index, 1)
      }
      throw error
    } finally {
      removeInvalidation(); unwatch()
      request?.signal.removeEventListener('abort', cancel)
      if (pendingMessage?.cancel === cancel) { pendingMessage = null; preparingMessage.value = false }
      executionActivity.remove(activityId, 'chat')
    }
  }

  // 为已有群组追加一个模型回答（不创建重复的 user 消息）
  async function addModelAnswer(groupId: string, content: string, modelId: string, providerCode?: string, replaceMessageId?: string) {
    if (isLoading.value || messageDeletionService.isPending(currentConversationId.value) || !modelId || !currentConversationId.value || !getChatTabState(instanceId).capture()()) return false
    if (!messages.value.some(message => message.role === 'user' && message.group_id === groupId && message.content === content)) return false
    if (replaceMessageId && !messages.value.some(message => message.id === replaceMessageId
      && message.role === 'assistant' && message.group_id === groupId)) return false

    const aiMsg: Message = {
      id: crypto.randomUUID(), conversation_id: currentConversationId.value!, role: 'assistant', content: '',
      model_id: modelId, token_input: null, token_output: null, status: 'streaming',
      created_at: new Date().toISOString(), group_id: groupId,
      _providerCode: providerCode,
    }
    // 插入到该 group 的最后一个消息之后，确保 groupedMessages 能正确合并
    let insertIdx = messages.value.length
    for (let i = messages.value.length - 1; i >= 0; i--) {
      if (messages.value[i].group_id === groupId) {
        insertIdx = i + 1
        break
      }
    }
    messages.value.splice(insertIdx, 0, aiMsg)

    // ★ 追加/重试必须带上「本次对话」的工具选择：streamResponse 这里全是位置参数，
    //   漏传一个后面全体错位（重试会静默丢掉弹层里的开关）。逐行对齐形参顺序写。
    const toolsSel = useChatToolsStore().selection(currentConversationId.value)
    await streamResponse(
      aiMsg, content, modelId,
      undefined,                       // knowledgeBaseIds
      undefined,                       // attachments
      groupId, true,                   // groupId / skipUserMessage
      undefined,                       // systemPrompt
      undefined,                       // localKbPaths
      undefined,                       // fileIds
      undefined,                       // webSearch
      undefined,                       // searchProvider
      undefined,                       // images
      undefined,                       // allAttachmentsJson
      undefined,                       // enableThinking
      // mcpTools 是「本轮要不要开工具循环」的总闸（只勾了 skill 也要开）
      useMcpChatStore().enabled && (toolsSel.mcpServerIds.length > 0 || toolsSel.skills.length > 0),
      toolsSel.mcpServerIds,           // 空数组 = 本轮一个 MCP 服务都不用
      undefined,                       // ocrContext
      undefined,                       // skillName
      toolsSel.skills,                 // enabledSkills
      false,                           // isRetry
      replaceMessageId,
    )
    return true
  }

  async function updateConversation(id: string, data: { title?: string }) {
    const invoke = await getInvoke()
    if (invoke) {
      try {
        await invoke('update_conversation', { id, ...data })
      } catch (err) {
        console.warn('Failed to update conversation', err)
      }
    }
    const conv = conversations.value.find(c => c.id === id)
    if (conv) {
      if (data.title !== undefined) conv.title = data.title
    }
  }

  // 用「话题命名模型」为对话生成标题。首条消息后调用一次；
  // 不阻塞主流程，失败时静默回退（后端已回退为原文截断）。
  async function maybeNameTopic(conversationId: string | null, content: string) {
    if (!conversationId) return
    const invoke = await getInvoke()
    if (!invoke) return
    try {
      const title = await invoke<string>('generate_topic_title', {
        conversationId,
        userMessage: content,
      })
      const conv = conversations.value.find(c => c.id === conversationId)
      if (conv && title) conv.title = title
    } catch (e) {
      console.warn('generate_topic_title failed', e)
    }
  }

  async function deleteConversation(id: string) {
    const invoke = await getInvoke()
    if (invoke) {
      await invoke('delete_conversation', { id })
    }
    conversations.value = conversations.value.filter(c => c.id !== id)
    forgetConversationActivity(id)
    removeConvFileCache(id)  // 同时清理文件缓存
    cleanupStreamListeners(id)  // 清理该会话的流监听器和缓冲区
    if (currentConversationId.value === id) {
      currentConversationId.value = null
      messages.value = []
      sharedMessageCache.delete(id)
    }
  }

  function deleteMessage(id: string, canStart: () => boolean = () => true): Promise<boolean> {
    const sourceMessages = messages.value
    return messageDeletionService.remove({
      owner: getChatTabState(instanceId),
      store: {
        get currentConversationId() { return currentConversationId.value },
        get messages() { return messages.value },
      },
    }, id, {
      getInvoke, canStart,
      busy: () => isLoading.value || getChatTabState(instanceId).state.sendBtnLock.value
        || (!!currentConversationId.value && executionActivity.isConversationRunning(currentConversationId.value)),
      commit: (conversationId, ids) => {
        const lists = [sourceMessages, sharedMessageCache.get(conversationId),
          _streamMsgBuffers.get(conversationId), webMockMessages.get(conversationId)]
          .filter((list): list is Message[] => !!list)
        for (const definition of chatStoreDefs.values()) {
          const store = definition() as unknown as { currentConversationId: string | null; messages: Message[] }
          if (store.currentConversationId === conversationId) lists.push(store.messages)
        }
        // Mutate captured/cache arrays in place; never redirect the result to the selected conversation.
        deletedMessageLedger.commit(conversationId, ids, lists)
      },
    })
  }

  /** 仅删除单条消息（不级联删除用户消息），用于图像重新生成等场景 */
  async function deleteSingleMessage(id: string) {
    messages.value = messages.value.filter(m => m.id !== id)
    if (currentConversationId.value) sharedMessageCache.set(currentConversationId.value, messages.value)
    const invoke = await getInvoke()
    if (invoke) {
      try {
        await invoke('delete_message', { id })
      } catch (err) {
        console.warn('Failed to delete single message from backend', err)
      }
    }
  }

  function regenerateMessage(id: string, modelId?: string, providerCode?: string) {
    const source = answerSource(messages.value, id)
    if (!source) return Promise.resolve(false)
    return addModelAnswer(source.groupId, source.content, modelId || '', providerCode, id)
  }

  async function clearMessages() {
    if (!currentConversationId.value) return
    const invoke = await getInvoke()
    if (invoke) {
      try {
        await invoke('clear_conversation_messages', { conversationId: currentConversationId.value })
      } catch (err) {
        console.warn('Failed to clear messages', err)
      }
    }
    messages.value = []
    sharedMessageCache.delete(currentConversationId.value)
    // 清空消息时同步清理该对话的文件缓存
    removeConvFileCache(currentConversationId.value)
  }

  function branchConversation(messageId: string, canStart: () => boolean = () => true): Promise<string | null> {
    return conversationBranchService.create({
      owner: getChatTabState(instanceId),
      store: {
        get currentConversationId() { return currentConversationId.value },
        get messages() { return messages.value },
      },
    }, messageId, { getInvoke, canStart, refresh: () => loadConversations(true) })
  }

  function resetToHome() {
    void releaseTabStreams()
    _clearAllLoading()
    currentConversationId.value = null
    messages.value = []
  }

  // 对话级文档附件缓存
  function setConvFileCache(convId: string, files: ChatAttachment[]) {
    convFileCache.value.set(convId, files)
    saveFileCacheToStorage()
  }
  function getConvFileCache(convId: string): ChatAttachment[] {
    return convFileCache.value.get(convId) || []
  }
  // 清除指定对话的缓存（用于删除对话等场景）
  function removeConvFileCache(convId: string) {
    convFileCache.value.delete(convId)
    saveFileCacheToStorage()
  }

  /** IPC 偶发失败时重试，尽量保证后端 active_streams 被释放 */
  async function invokeStopStreamReliable(
    invoke: InvokeFn,
    args: {
      conversationId: string
      messageId: string | null
      content: string
      modelId: string | null
      groupId: string | null
      thinkContent: string | null
    },
    retries = 3,
  ): Promise<boolean> {
    let lastErr: unknown
    for (let i = 0; i < retries; i++) {
      try {
        await invoke('stop_stream', args)
        return true
      } catch (err) {
        lastErr = err
        if (i < retries - 1) {
          await new Promise(r => setTimeout(r, 80 * (i + 1)))
        }
      }
    }
    console.warn('stop_stream failed after retries', args.conversationId, lastErr)
    return false
  }

  // ─── 停止流式输出（不传 convId = 当前会话；可停后台会话） ───
  async function stopStream(convId?: string) {
    if (pendingMessage && (!convId || pendingMessage.conversationId === convId)) pendingMessage.cancel()
    const cid = convId || currentConversationId.value
    if (!cid) return
    const lease = streamLeases.get(cid)
    if (lease && streamStopWaits.has(lease)) return streamStopWaits.get(lease)
    let releaseStop!: () => void
    const stoppedWaiting = new Promise<void>(resolve => { releaseStop = resolve })
    if (lease) streamStopWaits.set(lease, stoppedWaiting)
    const buffer = _streamMsgBuffers.get(cid)
    try {
      const invoke = await getInvoke()
      if (streamLeases.get(cid) !== lease) return
      if (!invoke) {
        if (docStream.value.conversationId === cid) clearDocEditStream()
        cleanupStreamListeners(cid)
        return
      }
      // 找到该会话正在流式输出的 AI 消息，将其部分内容传回后端落库。
      // 当前会话取 messages，后台会话从流缓冲区取（其消息数组由监听器持续写入）。
      const msgs = buffer || sharedMessageCache.get(cid) || (cid === currentConversationId.value ? messages.value : [])
      const streamingMsg = msgs.find(
        m => m.role === 'assistant' && m.status === 'streaming'
      )
      const stopped = await invokeStopStreamReliable(invoke, {
        conversationId: cid,
        messageId: streamingMsg?.id || null,
        content: streamingMsg?.content || '',
        modelId: streamingMsg?.model_id || null,
        groupId: streamingMsg?.group_id || null,
        thinkContent: streamingMsg?.think_content || null,
      })
      if (!stopped) {
        ElMessage.error('停止回答失败，请重试')
        return
      }
      // IPC 返回可能早于结束事件。仅收尾本次流，避免误清理随后启动的新回答。
      if (streamLeases.get(cid) === lease) {
        if (docStream.value.conversationId === cid) clearDocEditStream()
        const msg = msgs.find(m => m.id === streamingMsg?.id && m.status === 'streaming')
        if (msg) {
          msg.status = 'canceled'
          msg.isThinking = false
          msg.stream_phase = undefined
          msg.web_searching = false
          msg.retry_progress = undefined
        }
        cleanupStreamListeners(cid)
      }
    } finally {
      releaseStop()
      if (lease && streamStopWaits.get(lease) === stoppedWaiting) streamStopWaits.delete(lease)
    }
  }

  // 停止所有活跃会话的流（含后台会话）。用于「新建对话 / 返回首页」拆除前端流状态前，
  // 通知后端释放 active_streams 锁，避免会话被永久锁死（切回后无法继续发送）。
  // 内部先「同步」快照各会话正在流式的消息，再逐个通知后端，避免与随后的清理竞态。
  async function stopAllStreams() {
    pendingMessage?.cancel()
    const ids = new Set<string>([
      ...[...streamOwners].filter(([, owner]) => owner === instanceId).map(([id]) => id),
      ...loadingConversationIds.value,
    ])
    if (ids.size === 0) return
    // 同步快照（在任何 await 之前完成），确保即使调用方随后清空缓冲区也能落库
    const snapshots = [...ids].map((id) => {
      const msgs = _streamMsgBuffers.get(id) || sharedMessageCache.get(id) || messages.value
      const msg = msgs.find(m => m.role === 'assistant' && m.status === 'streaming')
      const snapshot = msg ? { ...msg } : undefined
      // 卸监听前同步完成共享气泡状态，避免其他 Tab/重新打开缓存时一直显示三个圆点。
      if (msg) {
        msg.status = 'canceled'
        msg.isThinking = false
        msg.stream_phase = undefined
        msg.web_searching = false
        msg.retry_progress = undefined
      }
      cleanupStreamListeners(id)
      return { id, msg: snapshot }
    })
    const invoke = await getInvoke()
    for (const { id, msg } of snapshots) {
      if (invoke) {
        await invokeStopStreamReliable(invoke, {
          conversationId: id,
          messageId: msg?.id || null,
          content: msg?.content || '',
          modelId: msg?.model_id || null,
          groupId: msg?.group_id || null,
          thinkContent: msg?.think_content || null,
        })
      }
    }
  }

  /** 关闭/重置 Tab：将仍被其他 Tab 打开的会话移交，剩余的才停止。 */
  function releaseTabStreams() {
    pendingMessage?.cancel()
    for (const [cid, owner] of streamOwners) {
      if (owner !== instanceId) continue
      const successor = [...activeChatStores].find(([id, store]) => (
        id !== instanceId && store.currentConversationId.value === cid
      ))
      if (successor) {
        streamOwners.set(cid, successor[0])
        executionActivity.update(cid, chatTabPath(successor[0]), 'chat', {
          phase: 'executing',
          protected: true,
        })
      }
    }
    return stopAllStreams()
  }

  // ─── 新增：编辑消息正文 ───
  async function updateMessageContent(id: string, content: string, isCurrent: () => boolean = () => true): Promise<boolean> {
    const conversationId = currentConversationId.value
    const source = messages.value.find(message => message.id === id && message.conversation_id === conversationId)
    if (!source || !conversationId || !isCurrent()) return false
    const original = source.content
    const valid = () => isCurrent() && currentConversationId.value === conversationId
      && messages.value.some(message => message.id === id && message.conversation_id === conversationId && message.content === original)
    const invoke = await getInvoke()
    if (!valid()) return false
    if (invoke) {
      // Propagate persistence failure: the editor must retain its draft instead of closing as if saved.
      await invoke('update_message_content', { id, content })
    }
    if (!valid()) return false
    const idx = messages.value.findIndex(m => m.id === id)
    if (idx >= 0) {
      messages.value[idx] = {
        ...messages.value[idx],
        content,
        is_edited: true,
        edited_at: new Date().toISOString(),
        ...(content !== original ? { translation: undefined, translation_lang: undefined } : {}),
      }
    }
    return true
  }

  // ─── 新增：翻译消息 ───
  async function translateMessage(messageId: string, targetLang: string, currentModelId?: string): Promise<string | null> {
    const owner = getChatTabState(instanceId), current = owner.capture(), conversationId = currentConversationId.value
    const valid = () => current() && currentConversationId.value === conversationId
    // Compatibility API for non-UI callers; ChatView supplies its stricter presentation lease directly.
    return messageActionService.translate({ owner, store: {
      get currentConversationId() { return currentConversationId.value },
      get messages() { return messages.value },
    } }, messageId, targetLang, currentModelId, {
      getInvoke, canStart: valid, isVisible: valid,
      report: (kind, error) => {
        if (kind === 'unavailable') ElMessage.info('翻译功能需要 Tauri 后端支持')
        else {
          const detail = error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error)
          ElMessage.error(`翻译失败: ${detail}`)
        }
      },
    })
  }

  // ─── 新增：清除上下文（保留显示，但下次 LLM 请求不带这些） ───
  async function clearContext() {
    if (!currentConversationId.value) return
    const invoke = await getInvoke()
    if (invoke) {
      try {
        await invoke('clear_conversation_context', { conversationId: currentConversationId.value })
      } catch (err) {
        console.warn('clear_conversation_context failed', err)
        ElMessage.error('清除上下文失败')
        return
      }
    }
    messages.value = messages.value.map(m => ({ ...m, context_cleared: true }))
    if (currentConversationId.value) sharedMessageCache.set(currentConversationId.value, messages.value)
    ElMessage.success('上下文已清除')
  }

  /**
   * 解析 mxcs JSON 为结构化数组，供模板 v-for 渲染
   * 返回 [{ section, type: 'object'|'array'|'text', items?: [{key, value}], value?: string }]
   */
  const parseMxcs = (mxcs: any) => {
    if (!mxcs) return [];
    try {
      const data = JSON.parse(mxcs);
      return Object.entries(data).map(([section, content]) => {
        if (content !== null && typeof content === 'object' && !Array.isArray(content)) {
          return {
            section,
            type: 'object',
            items: Object.entries(content).map(([key, value]) => ({ key, value: String(value) }))
          };
        } else if (Array.isArray(content)) {
          return { section, type: 'array', value: content.join('、') };
        } else {
          return { section, type: 'text', value: String(content) };
        }
      });
    } catch {
      return [];
    }
  };

  /**
   * 格式化 mxcs 为紧凑 HTML，用于 popover（内联下拉框等场景）
   */
  const formatMxcsContent = (mxcs: any) => {
    if (!mxcs) return '';
    try {
      const data = JSON.parse(mxcs);
      // 文字颜色统一继承面板（跟随主题色），不写死灰/黑；key 用透明度做次级弱化
      const wrapStyle = 'height:100%;overflow-y:auto;padding:10px 14px;box-sizing:border-box;color:inherit;';
      const gridStyle = 'display:grid;grid-template-columns:auto auto;gap:4px 20px;';
      const secTitleStyle = 'grid-column:1/-1;font-weight:600;font-size:11px;color:inherit;margin:4px 0 2px;padding-bottom:3px;border-bottom:1px solid var(--border-color);';
      const cellStyle = 'display:flex;flex-direction:column;padding:3px 0;font-size:11px;line-height:1.4;border-bottom:1px dashed var(--border-color);min-width:0;overflow:hidden;';
      const keyStyle = 'color:inherit;opacity:0.6;font-size:10px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;margin-bottom:1px;';
      const valStyle = 'color:inherit;font-size:11px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;';

      let html = `<div style="${wrapStyle}"><div style="${gridStyle}">`;
      for (const [key, content] of Object.entries(data)) {
        if (content !== null && typeof content === 'object' && !Array.isArray(content)) {
          html += `<div style="${secTitleStyle}">${key}</div>`;
          for (const [k, v] of Object.entries(content)) {
            html += `<div style="${cellStyle}"><span style="${keyStyle}" title="${k}">${k}</span><span style="${valStyle}" title="${String(v)}">${v}</span></div>`;
          }
        } else {
          const val = Array.isArray(content) ? content.join('、') : String(content);
          html += `<div style="${cellStyle}"><span style="${keyStyle}" title="${key}">${key}</span><span style="${valStyle}" title="${val}">${val}</span></div>`;
        }
      }
      html += '</div></div>';
      return html;
    } catch {
      return `<div style="height:100%;overflow-y:auto;padding:10px 14px;box-sizing:border-box;font-size:11px;line-height:1.5;white-space:pre-wrap;color:inherit;">${mxcs}</div>`;
    }
  };

  // 仅保存用户消息到 DB（不触发 AI），供任务规划模式使用
  // attachments：可选图片附件（如以图生图的参考图），随消息一并显示并持久化
  async function addUserMessage(content: string, attachments?: ChatAttachment[]): Promise<string> {
    const pendingConversationId = currentConversationId.value
    const isFirstMessage = messages.value.length === 0
    const gid = crypto.randomUUID()
    const userMsg: Message = {
      id: crypto.randomUUID(),
      conversation_id: pendingConversationId || '',
      role: 'user',
      content,
      model_id: null,
      token_input: null,
      token_output: null,
      status: 'completed',
      created_at: new Date().toISOString(),
      group_id: gid,
      attachments: attachments && attachments.length > 0 ? attachments : undefined,
    }
    messages.value.push(userMsg)

    // 供应商补全、任务规划和图像模式也共用这条入口，同样需要保留已经显示的首条消息。
    if (!pendingConversationId) {
      const conversationId = await createConversation(firstMessageTitle(content), undefined, undefined, undefined, true)
      userMsg.conversation_id = conversationId
    } else if (isFirstMessage) {
      const currentConv = conversations.value.find(c => c.id === pendingConversationId)
      if (currentConv && isPlaceholderTitle(currentConv.title)) {
        const title = firstMessageTitle(content)
        currentConv.title = title
        const invoke = await getInvoke()
        if (invoke) { try { await invoke('update_conversation', { id: pendingConversationId, title }) } catch { } }
      }
    }

    const attachmentsJson = attachments && attachments.length > 0
      ? JSON.stringify(attachments)
      : undefined
    const invoke = await getInvoke()
    if (invoke) {
      try {
        await invoke('save_user_message', {
          conversationId: userMsg.conversation_id,
          content,
          groupId: gid,
          attachments: attachmentsJson ?? null,
        })
      } catch (e) {
        console.warn('save_user_message failed', e)
      }
    }
    return gid
  }

  // Image jobs belong to the tab Store, not the mounted ChatView.
  const activeContextCount = computed(() => messages.value.filter(m => !m.context_cleared).length)

  /** 与后端 topic_title::is_placeholder_title 对齐 */
  function isPlaceholderTitle(title?: string | null): boolean {
    const t = (title || '').trim()
    if (!t) return true
    if (t === '新对话' || t === '智能对话' || t === '规划会话') return true
    return t.toLowerCase() === 'new chat'
  }

  /** 历史标题 = 首条问题原文（过长截断，不做模型概括） */
  function firstMessageTitle(content: string, maxChars = 80): string {
    const text = (content || '').trim().replace(/\s+/g, ' ')
    if (!text) return '智能对话'
    return [...text].length > maxChars ? [...text].slice(0, maxChars).join('') : text
  }

  /**
   * 首条用户消息：把占位标题换成问题原文（不做模型概括）。
   */
  async function applyFirstMessageTitle(conversationId: string | null | undefined, content: string) {
    if (!conversationId) return
    const currentConv = conversations.value.find(c => c.id === conversationId)
    if (!currentConv || !isPlaceholderTitle(currentConv.title)) return

    const title = firstMessageTitle(content)
    currentConv.title = title
    const invoke = await getInvoke()
    if (invoke) {
      try {
        await invoke('update_conversation', { id: conversationId, title })
      } catch { /* ignore */ }
    }
  }

  const store = {
    conversations, currentConversationId, messages, activeContextCount, isLoading, hasMore,
    isConversationStreaming, hasUnreadCompletion, markConversationViewed,
    loadConversations, createConversation, ensureConversation, detachCurrentConversation,
    prepareMessages, commitPreparedMessages, loadMessages, loadMoreMessages, sendMessage, updateConversation, deleteConversation,
    deleteMessage, deleteSingleMessage, regenerateMessage, clearMessages, branchConversation, addModelAnswer,
    resetToHome, stopStream, stopAllStreams, releaseTabStreams, updateMessageContent, translateMessage, clearContext,
    setConvFileCache, getConvFileCache, removeConvFileCache, parseMxcs, formatMxcsContent,
    addUserMessage,
    activateDocEditStream, getDocStreamRaw, clearDocEditStream,
    cleanupStreamListeners,
    removeLoadingConv: _removeLoadingConv,
    addLoadingConv: _addLoadingConv,
    isPlaceholderTitle,
    applyFirstMessageTitle,
    maybeNameTopic,
  }
  // 注册到全局单订阅的活跃实例表：remote 事件（会话删除/截断/新建）需据此遍历实例重置状态。
  activeChatStores.set(instanceId, store)
  return store
}

// ─── 多实例工厂：历史会话目录共享，其余对话状态按 tab 隔离 ───
const chatStoreDefs = new Map<string, ReturnType<typeof defineStore>>()
export function useChatStore(instanceId: string = 'default') {
  const key = `chat:${instanceId}`
  if (!chatStoreDefs.has(key)) {
    chatStoreDefs.set(key, defineStore(key, () => createChatStore(instanceId)))
  }
  return chatStoreDefs.get(key)!()
}
/**
 * 释放某个对话 tab 的 chat store。
 * 先移交仍在其他 Tab 打开的流，其余同步快照并清理，再销毁视图 Store。
 * 会话级监听器继续更新共享缓冲，完成/错误时自行释放；停止 IPC 在后台执行。
 */
export async function disposeChatStore(instanceId: string) {
  releaseChatTabState(instanceId)
  const key = `chat:${instanceId}`
  const def = chatStoreDefs.get(key)
  if (def) {
    try {
      const store = def() as {
        releaseTabStreams: () => Promise<void>
        currentConversationId: string | null
      }
      void store.releaseTabStreams()
      store.currentConversationId = null
    } catch { /* 停流失败仍继续释放，避免 tab 关不掉 */ }
    try { def().$dispose() } catch { /* 已释放则忽略 */ }
    chatStoreDefs.delete(key)
    activeChatStores.delete(instanceId)
  }
  unregisterChatTab(chatTabPath(instanceId))
  setForegroundConversation(chatTabPath(instanceId), null)
  if (typeof sessionStorage !== 'undefined') {
    sessionStorage.removeItem(`fastclaw-active-conversation:${instanceId}`)
  }
}

/** 正在流式生成（或会话仍在 loading）的对话 tab 路径 */
export function listStreamingChatTabPaths(): string[] {
  const paths: string[] = []
  for (const [key, def] of chatStoreDefs) {
    try {
      const store = def() as {
        messages?: { status?: string }[]
        isLoading?: boolean
        loadingConversationIds?: Set<string>
      }
      const instanceId = key.startsWith('chat:') ? key.slice(5) : key
      const path = instanceId === 'default' ? '/chat' : `/chat/${instanceId}`
      const streaming = Array.isArray(store.messages)
        && store.messages.some(m => m?.status === 'streaming')
      const loading =
        store.isLoading === true
        || (store.loadingConversationIds instanceof Set && store.loadingConversationIds.size > 0)
      const ownsBackgroundStream = [...streamOwners.values()].includes(instanceId)
      if (streaming || loading || ownsBackgroundStream) paths.push(path)
    } catch { /* ignore disposed */ }
  }
  return paths
}

function buildTraceSource(refs: KbReference[]): import('@/types').TraceSource | undefined {
  if (!refs || refs.length === 0) return undefined

  const ts: import('@/types').TraceSource = {}

  // 本地向量库溯源
  ts.local = {
    show: false,
    data: refs.map(r => ({
      snippet: r.preview || '',
      fileName: r.document_name || '',
      fileId: r.chunk_id,
    })),
  }

  // 关联文档溯源：按 document_name 聚合
  const fileMap = new Map<string, { fileName: string; snippet: { snippet: string }[] }>()
  refs.forEach(r => {
    const name = r.document_name || '未知文件'
    if (!fileMap.has(name)) {
      fileMap.set(name, { fileName: name, snippet: [] })
    }
    fileMap.get(name)!.snippet.push({ snippet: r.preview || '' })
  })
  ts.file = {
    show: false,
    current: 0,
    data: Array.from(fileMap.values()),
  }

  // 结果质量反馈（初始状态）
  ts.result = {
    show: false,
    rate: 0,
    content: '',
    loading: false,
    isSubmit: false,
    isQuery: false,
  }

  // 综合分析（延迟构建）
  ts.analysis = {
    show: false,
    summary: '',
  }

  return ts
}

