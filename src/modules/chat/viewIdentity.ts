import { inject, type InjectionKey } from 'vue'
import { useRoute } from 'vue-router'

/** 创建时的视图身份，不是响应式的当前路由，也不是后台命令的执行目标。 */
export interface ChatViewIdentity {
  readonly path: string
  readonly instanceId: string
}

export const CHAT_VIEW_IDENTITY: InjectionKey<ChatViewIdentity> = Symbol('chat-view-identity')

interface ChatRouteSnapshot {
  path: string
  params: { iid?: string | string[] }
}

/** 只复制身份，不保存 query、消息或 Store；保持 Router 已解码的 iid 语义。 */
export function captureChatViewIdentity(route: ChatRouteSnapshot): ChatViewIdentity | null {
  if (!/^\/chat(?:\/[^/]+)?\/?$/.test(route.path)) return null
  const iid = route.params.iid
  if (Array.isArray(iid)) return null // /chat/:iid 不是 repeatable 参数。
  const root = route.path === '/chat' || route.path === '/chat/'
  if (!root && !iid) return null
  return Object.freeze({ path: route.path, instanceId: root ? 'default' : iid! })
}

/** 路由外壳在异步加载前提供身份；直接挂载的兼容调用才读取当前路由。 */
export function useChatViewIdentity(): ChatViewIdentity {
  const provided = inject(CHAT_VIEW_IDENTITY, null)
  if (provided) return provided
  const identity = captureChatViewIdentity(useRoute())
  if (!identity) throw new Error('ChatView requires a valid Chat route identity')
  return identity
}
