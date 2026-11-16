import { reactive } from 'vue'

export function chatTabPath(instanceId: string): string {
  return instanceId === 'default' ? '/chat' : `/chat/${instanceId}`
}

export function isChatTabPath(path: string): boolean {
  return path === '/chat' || path.startsWith('/chat/')
}

export function chatTabStorageKey(path: string): string {
  return `fastclaw-active-conversation:${path === '/chat' ? 'default' : path.slice('/chat/'.length)}`
}

// 已挂载的 Store 是当前选择的权威来源；未挂载的恢复 Tab 也参与查重。
const liveTabs = reactive(new Map<string, string | null>())
const restoredTabs = new Map<string, string | null>()
const pendingTabs = new Map<string, { conversationId: string; token: symbol }>()

export function readStoredChatConversation(path: string): string | null {
  try { return sessionStorage.getItem(chatTabStorageKey(path)) } catch { return null }
}

export function setChatTabConversation(path: string, conversationId: string | null) {
  liveTabs.set(path, conversationId)
}

export function getChatTabConversation(path: string): string | null {
  if (liveTabs.has(path)) return liveTabs.get(path) ?? null
  if (restoredTabs.has(path)) return restoredTabs.get(path) ?? null
  return readStoredChatConversation(path)
}

export function syncRestoredChatTabs(tabs: { path: string; conversationId?: string | null }[]) {
  restoredTabs.clear()
  for (const tab of tabs) {
    if (isChatTabPath(tab.path)) {
      restoredTabs.set(tab.path, tab.conversationId === undefined
        ? readStoredChatConversation(tab.path) : tab.conversationId)
    }
  }
}

export function unregisterChatTab(path: string) {
  liveTabs.delete(path)
  restoredTabs.delete(path)
  pendingTabs.delete(path)
}

export function cancelPendingConversation(path: string, conversationId: string) {
  if (pendingTabs.get(path)?.conversationId === conversationId) pendingTabs.delete(path)
}

export function findConversationTab(conversationId: string): string | undefined {
  for (const [path, pending] of pendingTabs) {
    if (pending.conversationId === conversationId) return path
  }
  for (const path of new Set([...liveTabs.keys(), ...restoredTabs.keys()])) {
    // 正在切换到新会话的 Tab 已不再占用旧会话。
    if (!pendingTabs.has(path) && getChatTabConversation(path) === conversationId) return path
  }
  return undefined
}

/** 必须在 await 前预占；旧请求完成时只能释放自己的预占。 */
export function claimConversationTab(path: string, conversationId: string, takeOver = false) {
  const existing = findConversationTab(conversationId)
  const targetPath = existing ?? path
  const token = Symbol(conversationId)
  const pending = pendingTabs.get(path)
  if ((!existing || existing === path) && (takeOver || pending?.conversationId !== conversationId)) {
    pendingTabs.set(path, { conversationId, token })
  }
  return {
    path: targetPath,
    release() {
      if (pendingTabs.get(path)?.token === token) pendingTabs.delete(path)
    },
  }
}
