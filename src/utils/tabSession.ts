/**
 * 启动时恢复标签页会话
 * - unfinished（默认）：仅恢复「未完成任务」标签（见 UnfinishedDetectContext）
 * - all：恢复上次全部未关闭的标签页
 * 会话快照与其它基础参数一样写入 settings 表（并镜像 localStorage 以便同步读）。
 */

import { readStoredChatConversation } from './chatTabRegistry'

export type TabSessionRestoreMode = 'unfinished' | 'all'

export interface SessionTab {
  path: string
  title: string
  i18nKey?: string
  closable: boolean
  conversationId?: string | null
  /** 切 Tab 时带回的路由 query */
  query?: Record<string, string>
}

export interface TabSessionSnapshot {
  tabs: SessionTab[]
  activePath: string
  unfinishedPaths: string[]
  savedAt: number
}

/** 关闭时收集未完成标签所需的运行时上下文 */
export interface UnfinishedDetectContext {
  /** 正在流式生成的对话 tab 路径（/chat、/chat/:iid） */
  streamingChatPaths?: string[]
  /** FastPlan / Agent 规划执行中（running/pending） */
  planningBusy?: boolean
  /** 正在运行/启动中的 WSP 服务 id */
  runningWspServiceIds?: Set<string> | string[]
  /** 组件侧登记的未完成 tab */
  flaggedUnfinishedPaths?: string[]
  /** 是否有进行中的下载任务 */
  hasActiveDownloads?: boolean
}

export const TAB_SESSION_RESTORE_KEY = 'tab_session_restore'
export const TAB_SESSION_SNAPSHOT_KEY = 'tab_session_snapshot'
export const TAB_SESSION_RESTORE_LS = 'fastclaw-tab-session-restore'
export const TAB_SESSION_SNAPSHOT_LS = 'fastclaw-tab-session-snapshot'
/** 兼容旧版 TabBar 本地缓存 */
export const LEGACY_TABS_LS = 'fastclaw-open-tabs'
export const LEGACY_ACTIVE_LS = 'fastclaw-active-tab'

export const DEFAULT_TAB_SESSION_RESTORE: TabSessionRestoreMode = 'unfinished'

const HOME_CHAT = '/chat'

/**
 * 标签页快照来自数据库和 localStorage，旧版本或并发落盘可能留下重复项。
 * Vue 的 v-for 以 path 为 key，重复 path 不仅会重复显示，还会导致 DOM 复用错乱。
 */
export function dedupeSessionTabs(tabs: SessionTab[]): SessionTab[] {
  const result: SessionTab[] = []
  const indexByPath = new Map<string, number>()

  for (const tab of tabs) {
    if (!tab || typeof tab.path !== 'string' || !tab.path.startsWith('/')) continue

    const normalized: SessionTab = {
      path: tab.path,
      title: typeof tab.title === 'string' && tab.title ? tab.title : 'sidebar.newPage',
      closable: tab.closable !== false,
      ...(typeof tab.i18nKey === 'string' ? { i18nKey: tab.i18nKey } : {}),
      ...(isChatTabPath(tab.path) && (typeof tab.conversationId === 'string' || tab.conversationId === null)
        ? { conversationId: tab.conversationId || null } : {}),
      ...(tab.query && typeof tab.query === 'object' && !Array.isArray(tab.query)
        ? {
            query: Object.fromEntries(
              Object.entries(tab.query)
                .filter(([, v]) => typeof v === 'string' && v)
                .map(([k, v]) => [k, String(v)]),
            ),
          }
        : {}),
    }
    const existingIndex = indexByPath.get(normalized.path)
    if (existingIndex === undefined) {
      indexByPath.set(normalized.path, result.length)
      result.push(normalized)
    } else {
      // 保留原来的顺序，同时采用较新的标题等信息。
      result[existingIndex] = { ...result[existingIndex], ...normalized }
    }
  }

  return result
}

/** 仅浏览、默认不算未完成的路径前缀/精确路径 */
const BROWSE_ONLY_EXACT = new Set([
  '/alltool',
  '/skills',
  '/mcp',
  '/wsp-admin',
  '/knowledge',
  '/files',
  '/models',
  '/dashboard',
  '/settings',
  '/tasks',
  '/governance',
  '/chunk-tool',
  '/chunk-viewer',
])

const DOWNLOAD_RELATED = new Set(['/models', '/wsp-admin'])

/** 侧边栏已收掉的页面。旧会话里如果还记着这些路径，恢复时直接丢掉。 */
const REMOVED_ROUTE_PREFIXES = [
  '/compare',
  '/assistants',
  '/agent',
  '/coding',
  '/documentGeneration',
  '/fastoffice',
  '/longform',
  '/model-market',
  '/market',
]

function isRemovedRoute(path: string): boolean {
  return REMOVED_ROUTE_PREFIXES.some((prefix) => path === prefix || path.startsWith(`${prefix}/`))
}

function isChatTabPath(path: string): boolean {
  return path === HOME_CHAT || path.startsWith('/chat/')
}

/**
 * 按约定条件判定未完成任务标签：
 * - 智能对话：流式生成中
 * - 规划/Agent：节点 running/pending 时，已打开的对话标签
 * - 组件登记的未完成路径
 * - 下载：有进行中下载时，关联的模型市场/模型管理/WSP 管理页
 * - 纯浏览页一律不算
 */
export function collectUnfinishedPaths(
  tabs: SessionTab[],
  ctx: UnfinishedDetectContext = {},
): string[] {
  const open = new Set(tabs.map(t => t.path))
  const result = new Set<string>()
  const flagged = new Set(ctx.flaggedUnfinishedPaths || [])
  const streamingChats = new Set(ctx.streamingChatPaths || [])

  for (const tab of tabs) {
    const path = tab.path

    // 1) 智能对话流式
    if (isChatTabPath(path) && streamingChats.has(path)) {
      result.add(path)
      continue
    }

    // 2) 规划执行中 → 已打开的对话 tab
    if (ctx.planningBusy && isChatTabPath(path)) {
      result.add(path)
      continue
    }

    // 3) 组件侧显式登记的未完成路径
    if (flagged.has(path)) {
      result.add(path)
      continue
    }

    // 4) 进行中下载关联页
    if (ctx.hasActiveDownloads && DOWNLOAD_RELATED.has(path)) {
      result.add(path)
      continue
    }
  }

  // 登记了但 tabs 里没有的忽略；仅保留当前仍打开的
  return [...result].filter(p => open.has(p))
}

/** @deprecated 请用 collectUnfinishedPaths；保留给旧调用兼容 */
export function isUnfinishedTab(tab: SessionTab, ctx?: UnfinishedDetectContext): boolean {
  return collectUnfinishedPaths([tab], ctx).includes(tab.path)
}

export function normalizeRestoreMode(value: string | undefined | null): TabSessionRestoreMode {
  return value === 'all' ? 'all' : 'unfinished'
}

export function buildSnapshot(
  tabs: SessionTab[],
  activePath: string,
  ctx: UnfinishedDetectContext = {},
): TabSessionSnapshot {
  const uniqueTabs = dedupeSessionTabs(tabs)
  return {
    tabs: uniqueTabs,
    activePath,
    unfinishedPaths: collectUnfinishedPaths(uniqueTabs, ctx),
    savedAt: Date.now(),
  }
}

export function defaultHomeTabs(): SessionTab[] {
  return [{ path: HOME_CHAT, title: 'sidebar.smartChat', closable: true }]
}

/**
 * 启动首屏路由（同步读 localStorage 镜像）。
 * 避免先挂 /chat 再 replace 到上次会话，造成 ChatView 卸载重挂、中间白屏闪一下。
 */
export function resolveStartupRoute(): { path: string; query?: { title: string } } {
  try {
    const mode = normalizeRestoreMode(readRestoreModeFromLocalStorage())
    const snapshot = readSnapshotFromLocalStorage()
    const resolved = resolveTabsFromSnapshot(snapshot, mode)
    const path = resolved.activePath || HOME_CHAT
    const tab = resolved.tabs.find((t) => t.path === path)
    const title = tab?.title
    if (title && title !== 'sidebar.smartChat' && title !== 'sidebar.newPage') {
      return { path, query: { title } }
    }
    return { path }
  } catch {
    return { path: HOME_CHAT }
  }
}

/** 按恢复模式从快照得到要打开的标签与激活路径 */
export function resolveTabsFromSnapshot(
  snapshot: TabSessionSnapshot | null,
  mode: TabSessionRestoreMode,
): { tabs: SessionTab[]; activePath: string } {
  if (!snapshot?.tabs?.length) {
    return { tabs: defaultHomeTabs(), activePath: HOME_CHAT }
  }

  // 兼容旧版「非首页全算未完成」快照：首次升级后避免仍恢复全部
  const sanitized = sanitizeLegacyUnfinishedSnapshot({
    ...snapshot,
    tabs: dedupeSessionTabs(snapshot.tabs),
    unfinishedPaths: [...new Set(snapshot.unfinishedPaths || [])],
  })

  let tabs: SessionTab[]
  const kept = sanitized.tabs.filter(t => !isRemovedRoute(t.path))
  if (mode === 'all') {
    tabs = kept.map(t => ({ ...t }))
  } else {
    const unfinishedSet = new Set(sanitized.unfinishedPaths || [])
    tabs = kept.filter(t => unfinishedSet.has(t.path) || t.path === HOME_CHAT)
  }

  if (!tabs.some(t => t.path === HOME_CHAT)) {
    tabs.unshift(...defaultHomeTabs())
  }
  if (!tabs.length) tabs = defaultHomeTabs()

  let activePath = isRemovedRoute(sanitized.activePath || '') ? HOME_CHAT : (sanitized.activePath || HOME_CHAT)
  // 旧快照从 sessionStorage 补充 ID；新快照直接保存，懒加载的 Tab 也能参与查重。
  tabs = tabs.map(tab => isChatTabPath(tab.path)
    ? { ...tab, conversationId: tab.conversationId === undefined ? readStoredChatConversation(tab.path) : tab.conversationId }
    : tab)
  const canonicalPaths = new Map<string, string>()
  for (const tab of tabs) {
    if (!tab.conversationId) continue
    const previous = canonicalPaths.get(tab.conversationId)
    if (!previous || tab.path === HOME_CHAT || (tab.path === activePath && previous !== HOME_CHAT)) {
      canonicalPaths.set(tab.conversationId, tab.path)
    }
  }
  tabs = tabs.filter(tab => {
    const canonical = tab.conversationId && canonicalPaths.get(tab.conversationId)
    if (!canonical || canonical === tab.path) return true
    if (activePath === tab.path) activePath = canonical
    return false
  })
  if (!tabs.some(t => t.path === activePath)) {
    activePath = tabs.find(t => t.path !== HOME_CHAT)?.path || HOME_CHAT
  }

  return { tabs, activePath }
}

/** 旧逻辑把几乎所有非首页都写入 unfinishedPaths，升级后当作「无未完成」 */
function sanitizeLegacyUnfinishedSnapshot(snapshot: TabSessionSnapshot): TabSessionSnapshot {
  const nonHome = snapshot.tabs.filter(t => t.path !== HOME_CHAT).map(t => t.path)
  const unfinished = snapshot.unfinishedPaths || []
  if (
    nonHome.length >= 2
    && unfinished.length >= nonHome.length
    && nonHome.every(p => unfinished.includes(p))
  ) {
    return { ...snapshot, unfinishedPaths: [] }
  }
  return snapshot
}

export function parseSnapshot(raw: string | undefined | null): TabSessionSnapshot | null {
  if (!raw) return null
  try {
    const data = JSON.parse(raw)
    if (!data || !Array.isArray(data.tabs)) return null
    return {
      tabs: dedupeSessionTabs(data.tabs),
      activePath: typeof data.activePath === 'string' ? data.activePath : HOME_CHAT,
      unfinishedPaths: Array.isArray(data.unfinishedPaths) ? data.unfinishedPaths : [],
      savedAt: typeof data.savedAt === 'number' ? data.savedAt : 0,
    }
  } catch {
    return null
  }
}

/** 从旧版 localStorage tabs 结构迁移 */
export function snapshotFromLegacyLocalStorage(): TabSessionSnapshot | null {
  try {
    const raw = localStorage.getItem(LEGACY_TABS_LS)
    if (!raw) return null
    const tabs = JSON.parse(raw) as SessionTab[]
    if (!Array.isArray(tabs) || !tabs.length) return null
    const activePath = localStorage.getItem(LEGACY_ACTIVE_LS) || HOME_CHAT
    // 旧数据无可靠未完成标记 → 视为无未完成，避免「全开」
    return {
      tabs: tabs.map(t => ({ ...t })),
      activePath,
      unfinishedPaths: [],
      savedAt: 0,
    }
  } catch {
    return null
  }
}

export function readSnapshotFromLocalStorage(): TabSessionSnapshot | null {
  try {
    const fromNew = parseSnapshot(localStorage.getItem(TAB_SESSION_SNAPSHOT_LS))
    if (fromNew) return fromNew
  } catch { /* ignore */ }
  return snapshotFromLegacyLocalStorage()
}

export function mirrorSnapshotToLocalStorage(snapshot: TabSessionSnapshot) {
  try {
    localStorage.setItem(TAB_SESSION_SNAPSHOT_LS, JSON.stringify(snapshot))
    localStorage.setItem(LEGACY_TABS_LS, JSON.stringify(snapshot.tabs))
    localStorage.setItem(LEGACY_ACTIVE_LS, snapshot.activePath)
  } catch { /* ignore */ }
}

export function mirrorRestoreModeToLocalStorage(mode: TabSessionRestoreMode) {
  try {
    localStorage.setItem(TAB_SESSION_RESTORE_LS, mode)
  } catch { /* ignore */ }
}

export function readRestoreModeFromLocalStorage(): TabSessionRestoreMode {
  try {
    return normalizeRestoreMode(localStorage.getItem(TAB_SESSION_RESTORE_LS))
  } catch {
    return DEFAULT_TAB_SESSION_RESTORE
  }
}

/** 是否为纯浏览路径（供调试/文档） */
export function isBrowseOnlyPath(path: string): boolean {
  if (BROWSE_ONLY_EXACT.has(path)) return true
  if (path.startsWith('/knowledge-bases/')) return true
  if (path.startsWith('/settings/')) return true
  return false
}
