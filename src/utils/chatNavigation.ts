import type { RouteLocationNormalized, Router } from 'vue-router'
import { claimConversationTab, findConversationTab, getChatTabConversation, isChatTabPath } from './chatTabRegistry'

type ConversationOpenOptions = { messageId?: string; preferredPath?: string; replace?: boolean }

/** 旧链接和直接带 convId 的路由也在挂载页面/创建 Tab 前重定向。 */
export function redirectDuplicateConversation(to: RouteLocationNormalized) {
  if (!isChatTabPath(to.path) || typeof to.query.convId !== 'string') return
  const existing = findConversationTab(to.query.convId)
  if (existing && existing !== to.path) return { path: existing, query: to.query, hash: to.hash, replace: true }
}

/** 收藏/外部入口先查重，再决定是否创建 Tab，避免覆盖现有 Tab 的草稿。 */
export async function navigateToConversation(router: Router, conversationId: string, options: ConversationOpenOptions = {}) {
  const path = findConversationTab(conversationId) ?? options.preferredPath ?? `/chat/${crypto.randomUUID()}`
  const claim = claimConversationTab(path, conversationId)
  try {
    const location = {
      path: claim.path,
      query: { convId: conversationId, ...(options.messageId ? { msgId: options.messageId } : {}) },
    }
    const failure = await (options.replace ? router.replace(location) : router.push(location))
    if (failure) claim.release()
    // 导航成功后由目标页面加载时接管预占；避免懒加载期间第二次点击新建 Tab。
    return claim.path
  } catch (error) {
    claim.release()
    throw error
  }
}

/** 所有页面内的会话选择都在修改草稿、附件、规划状态之前经过这里。 */
export async function openConversationInTab(
  router: Router,
  path: string,
  conversationId: string,
  load: () => Promise<void>,
  options: { hydrate?: boolean; messageId?: string } = {},
): Promise<boolean> {
  const currentId = getChatTabConversation(path)
  const claim = claimConversationTab(path, conversationId, true)
  if (claim.path !== path) {
    await navigateToConversation(router, conversationId, { messageId: options.messageId })
    // 目标 Tab 已存在时应跳转过去；若路由没动（重复导航被忽略、守卫拦截等），
    // 不能直接 return，否则历史列表点击会看起来完全没反应。
    if (router.currentRoute.value.path !== path) {
      claim.release()
      return false
    }
    // 仍停在当前 Tab：强制加载被点中的会话
    options = { ...options, hydrate: true }
  }
  try {
    if (currentId !== conversationId || options.hydrate) {
      await load()
    }
    return true
  } catch (error) {
    throw error
  } finally {
    claim.release()
  }
}
