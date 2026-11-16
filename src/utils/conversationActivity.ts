import { reactive } from 'vue'

// 仅保存本次应用运行期间的完成提醒，不写入消息、数据库或浏览器存储。
const unreadCompletions = reactive(new Set<string>())
const foregroundViews = new Map<string, string>()

export function hasUnreadCompletion(conversationId: string): boolean {
  return unreadCompletions.has(conversationId)
}

export function markConversationViewed(conversationId: string) {
  unreadCompletions.delete(conversationId)
}

export function setForegroundConversation(viewId: string, conversationId: string | null) {
  if (!conversationId) {
    foregroundViews.delete(viewId)
    return
  }
  foregroundViews.set(viewId, conversationId)
  markConversationViewed(conversationId)
}

export function markConversationCompleted(conversationId: string) {
  if ([...foregroundViews.values()].includes(conversationId)) {
    markConversationViewed(conversationId)
  } else {
    unreadCompletions.add(conversationId)
  }
}

export function forgetConversationActivity(conversationId: string) {
  unreadCompletions.delete(conversationId)
  for (const [viewId, id] of foregroundViews) {
    if (id === conversationId) foregroundViews.delete(viewId)
  }
}
