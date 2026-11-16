<script setup lang="ts">
import { computed, inject, nextTick, onMounted, onUnmounted, provide, ref, watch } from 'vue'
import type { ComponentPublicInstance } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useAppStore } from '@/stores/app'
import { useSettingsStore } from '@/stores/settings'
import { useWspStore } from '@/stores/wsp'
import { useDownloadRuntime } from 'tauri-download-center-vue'
import { usePlanningStore } from '@/stores/planning'
import { usePlanActivityStore } from '@/stores/planActivity'
import { useExecutionActivityStore, type ExecutionPhase } from '@/stores/executionActivity'
import { fastPlanSessionManager } from '@/integrations/chat-plan/sessionManager'
import { chatSwitchPerf } from '@/utils/chatSwitchPerf'
import { disposeChatStore, listStreamingChatTabPaths } from '@/stores/chat'
import { getChatTabConversation, isChatTabPath, syncRestoredChatTabs } from '@/utils/chatTabRegistry'
import { TAB_SOFT_WARN_MAX } from '@/config/constants'
import { releaseChatTabState } from '@/modules/chat/tabState'
import SvgIcon from '@/components/svgIcon/index.vue'
import { ElMessageBox } from 'element-plus'
import {
  buildSnapshot,
  dedupeSessionTabs,
  defaultHomeTabs,
  readRestoreModeFromLocalStorage,
  readSnapshotFromLocalStorage,
  resolveTabsFromSnapshot,
  type SessionTab,
  type UnfinishedDetectContext,
} from '@/utils/tabSession'
const { t, te } = useI18n()

function tabDisplayTitle(title: string): string {
  return te(title) ? t(title) : title
}
const appStore = useAppStore()
const settingsStore = useSettingsStore()
const wspStore = useWspStore()
const downloadRuntime = useDownloadRuntime()

const planningStore = usePlanningStore()
const planActivity = usePlanActivityStore()
const executionActivity = useExecutionActivityStore()

const PATH_ICON_MAP: Record<string, string> = {
  '/chat': '智能对话',
  '/alltool': '全部工具',
  '/skills': 'Skills管理',
  '/mcp': 'MCP服务',
  '/wsp-admin': 'WSP管理',
  '/knowledge': '知识管理',
  '/files': 'nav-data',
  '/models': 'nav-model',
  '/dashboard': '调用看板',
  '/settings': 'nav-settings',
  '/tasks': '知识管理',
  '/governance': '知识管理',
  '/chunk-tool': '知识管理',
  '/chunk-viewer': '知识管理',
}
function tabIcon(path: string): string {
  if (PATH_ICON_MAP[path]) return PATH_ICON_MAP[path]
  const prefixMap: [string, string][] = [
    ['/chat/', '智能对话'],
    ['/knowledge-bases/', '知识管理'],
    ['/governance/', '知识管理'],
    ['/settings/', 'nav-settings'],
    ['/chunk-tool/', '知识管理'],
    ['/chunk-viewer/', '知识管理'],
    ['/tasks/', '知识管理'],
    ['/files/', 'nav-data'],
    ['/models/', 'nav-model'],
  ]
  for (const [p, icon] of prefixMap) {
    if (path.startsWith(p)) return icon
  }
  return ''
}

const showMaxTip = ref(false)
const tipPosition = ref({ top: 0, left: 0 })
let maxTipTimer: ReturnType<typeof setTimeout> | null = null
const tipStyle = computed(() => ({
  position: 'fixed' as const,
  top: tipPosition.value.top + 'px',
  left: tipPosition.value.left + 'px',
  transform: 'translateX(-50%)',
}))

function updateTipPosition() {
  if (tabBarRef.value) {
    const rect = tabBarRef.value.getBoundingClientRect()
    tipPosition.value = { top: rect.top - 8, left: rect.left + rect.width / 2 }
  }
}

function showTabSoftWarnTip() {
  updateTipPosition()
  showMaxTip.value = true
  if (maxTipTimer) clearTimeout(maxTipTimer)
  maxTipTimer = setTimeout(() => { showMaxTip.value = false }, 4000)
}

onUnmounted(() => { if (maxTipTimer) clearTimeout(maxTipTimer) })

interface Tab {
  path: string
  title: string
  i18nKey?: string
  closable: boolean
  conversationId?: string | null
  /** 非 title 的路由 query，切 Tab 时带回 */
  query?: Record<string, string>
}

function tabToLocation(tab: Tab) {
  const query: Record<string, string> = { title: tab.title }
  if (tab.query) Object.assign(query, tab.query)
  return { path: tab.path, query }
}

const router = useRouter()
const route = useRoute()

const ACTIVE_TAB_KEY = 'fastclaw-active-tab'

function initialSession(): { tabs: Tab[]; activePath: string } {
  const mode = settingsStore.getTabSessionRestoreMode?.() ?? readRestoreModeFromLocalStorage()
  const snapshot =
    settingsStore.getTabSessionSnapshot?.() ?? readSnapshotFromLocalStorage()
  const resolved = resolveTabsFromSnapshot(snapshot, mode)
  return {
    tabs: resolved.tabs.length ? resolved.tabs : defaultHomeTabs(),
    activePath: resolved.activePath,
  }
}

const _initial = initialSession()
const tabs = ref<Tab[]>(dedupeSessionTabs(_initial.tabs))
watch(tabs, value => syncRestoredChatTabs(value), { immediate: true, deep: true, flush: 'sync' })
const activeTab = ref(_initial.activePath || '/chat')
const tabBarRef = ref<HTMLDivElement | null>(null)
const tabberRef = ref<HTMLDivElement | null>(null)
const tabItemRefs = new Map<string, HTMLDivElement>()

let persistTimer: ReturnType<typeof setTimeout> | null = null
let sessionReady = false

function buildUnfinishedContext(): UnfinishedDetectContext {
  const runningWspServiceIds = new Set<string>()
  for (const s of wspStore.services || []) {
    if (s.runtime_status === 'running' || s.runtime_status === 'starting') {
      runningWspServiceIds.add(String(s.id))
    }
  }
  try {
    const raw = localStorage.getItem('wsp_services')
    if (raw) {
      const list = JSON.parse(raw) as Array<{ id?: string; runtime_status?: string }>
      for (const s of list) {
        if (s?.id && (s.runtime_status === 'running' || s.runtime_status === 'starting')) {
          runningWspServiceIds.add(String(s.id))
        }
      }
    }
  } catch { /* ignore */ }

  const planNodes = Object.values(planningStore.executionState || {}) as Array<{ status?: string }>
  const planningBusy =
    !!planningStore.isPlanning
    || planActivity.anyBusy
    || (planningStore.stats?.running ?? 0) > 0
    || planNodes.some(n => n.status === 'running' || n.status === 'pending')

  const hasActiveDownloads = downloadRuntime.tasks.value.some(
    t => ['pending', 'downloading', 'pausing', 'verifying', 'installing'].includes(t.status),
  )

  return {
    streamingChatPaths: listStreamingChatTabPaths(),
    planningBusy,
    runningWspServiceIds,
    flaggedUnfinishedPaths: [...new Set([...appStore.unfinishedFlagPaths, ...planActivity.busyPaths])],
    hasActiveDownloads,
  }
}

function persistTabSession(immediate = false): Promise<void> {
  const run = () => {
    const snapshot = buildSnapshot(
      tabs.value.map(tab => isChatTabPath(tab.path)
        ? { ...tab, conversationId: getChatTabConversation(tab.path) } : tab) as SessionTab[],
      activeTab.value,
      buildUnfinishedContext(),
    )
    return settingsStore.saveTabSessionSnapshot(snapshot)
  }
  if (immediate) {
    if (persistTimer) { clearTimeout(persistTimer); persistTimer = null }
    return run()
  }
  if (persistTimer) clearTimeout(persistTimer)
  return new Promise((resolve) => {
    persistTimer = setTimeout(() => { void run().then(resolve) }, 400)
  })
}

/** 供关闭窗口前同步落盘 */
function flushTabSession() {
  return persistTabSession(true)
}
provide('flushTabSession', flushTabSession)

const refreshTabFn = inject<((path: string, options?: { remountWsp?: boolean }) => void) | undefined>('refreshTab', undefined)
const registerTabSessionFlush = inject<
  ((fn: (() => void | Promise<void>) | null) => void) | undefined
>('registerTabSessionFlush', undefined)
const registerCloseTab = inject<
  ((fn: ((path: string) => void | Promise<void>) | null) => void) | undefined
>('registerCloseTab', undefined)

const contextMenu = ref({
  visible: false,
  x: 0,
  y: 0,
  tab: <Tab>{},
})

const setTabItemRef = (
  path: string,
  el: Element | ComponentPublicInstance | null,
) => {
  if (el instanceof HTMLDivElement) {
    tabItemRefs.set(path, el)
    return
  }
  tabItemRefs.delete(path)
}

/** 溢出时两侧翻页；滚轮可左右滚动；不显示滚动条；允许半个 tab */
const showPager = ref(false)
const canScrollPrev = ref(false)
const canScrollNext = ref(false)

const ensureTabVisible = async (
  path = activeTab.value,
  behavior: ScrollBehavior = 'smooth',
) => {
  await nextTick()
  const container = tabberRef.value
  const tab = tabItemRefs.get(path)
  if (!container || !tab) return

  const containerRect = container.getBoundingClientRect()
  const tabRect = tab.getBoundingClientRect()
  const SCROLL_PADDING = 8

  if (tabRect.left < containerRect.left + SCROLL_PADDING) {
    const targetLeft = container.scrollLeft + (tabRect.left - containerRect.left) - SCROLL_PADDING
    container.scrollTo({ left: Math.max(0, targetLeft), behavior })
    updateScrollState()
    return
  }
  if (tabRect.right > containerRect.right - SCROLL_PADDING) {
    const targetLeft = container.scrollLeft + (tabRect.right - containerRect.right) + SCROLL_PADDING
    container.scrollTo({ left: Math.max(0, targetLeft), behavior })
    updateScrollState()
  }
}

watch(
  () => [route.path, route.query.title] as const,
  ([path, queryTitle]) => {
    if (!sessionReady) return
    if (path === '/') {
      activeTab.value = path
      return
    }

    const existingTab = tabs.value.find(tab => tab.path === path)

    activeTab.value = path

    const customTitle = typeof queryTitle === 'string' ? queryTitle : ''
    const defaultTitle =
      typeof route.meta?.title === 'string' ? route.meta.title : 'sidebar.newPage'
    const finalTitle = customTitle || defaultTitle

    if (!existingTab) {
      // 路由监听和异步会话恢复可能在同一轮更新中交错；赋值前再次按 path
      // 建立唯一集合，保证任何来源都不能产生重复 key 的 tab。
      tabs.value = dedupeSessionTabs([
        ...tabs.value,
        { path, title: finalTitle, closable: true },
      ])
      // 仅软提示，不拦截、不关闭已有标签
      if (tabs.value.length > TAB_SOFT_WARN_MAX) {
        showTabSoftWarnTip()
      }
    } else {
      if (customTitle && existingTab.title !== customTitle) {
        existingTab.title = finalTitle
      }
    }

    void ensureTabVisible(path)
  },
  { immediate: true },
)

const switchTab = (tab: Tab) => {
  if (tab.path === activeTab.value) return
  chatSwitchPerf.begin(activeTab.value, tab.path)
  router.push(tabToLocation(tab))
}

const spinningExecutionPhases = new Set<ExecutionPhase>([
  'submitting', 'planning', 'executing', 'reconnecting',
])

function tabExecutionPhase(path: string): ExecutionPhase | undefined {
  return executionActivity.activityByPath(path)?.phase
}

function tabExecutionSpinning(path: string) {
  const phase = tabExecutionPhase(path)
  return !!phase && spinningExecutionPhases.has(phase)
}

function tabExecutionLabel(path: string) {
  const labels: Partial<Record<ExecutionPhase, string>> = {
    submitting: '正在提交',
    planning: '正在规划任务',
    executing: '正在执行',
    awaiting_plan_confirmation: '等待确认规划',
    awaiting_step_confirmation: '等待继续下一步',
    awaiting_human: '等待人工输入',
    paused: '任务已暂停',
    reconnecting: '连接中断，正在恢复',
  }
  const phase = tabExecutionPhase(path)
  return phase ? labels[phase] : undefined
}

async function guardProtectedTabClose(paths: string[]): Promise<boolean> {
  const protectedPaths = [...new Set(paths.filter(path => executionActivity.isTabProtected(path)))]
  if (!protectedPaths.length) return true
  const detail = protectedPaths.length > 1
    ? `将关闭 ${protectedPaths.length} 个执行中的会话，关闭后未完成内容将丢失。`
    : '关闭后未完成内容将丢失'
  try {
    await ElMessageBox.confirm(detail, '任务尚未完成', {
      confirmButtonText: '仍要关闭',
      cancelButtonText: '取消',
      type: 'warning',
    })
    return true
  } catch {
    return false
  }
}

interface BeforeTabCloseDetail {
  path: string
  cancelExecution: boolean
  waitUntil: (task: Promise<unknown>) => void
}

/** Allow route components to remove Teleport content before KeepAlive invalidation. */
async function prepareTabViewForClose(path: string, cancelExecution = false) {
  const pending: Promise<unknown>[] = []
  const detail: BeforeTabCloseDetail = {
    path,
    cancelExecution,
    waitUntil(task) {
      pending.push(Promise.resolve(task))
    },
  }
  window.dispatchEvent(new CustomEvent<BeforeTabCloseDetail>('fastclaw:before-tab-close', { detail }))
  if (pending.length > 0) await Promise.allSettled(pending)
  await fastPlanSessionManager.closeTabSessions(path, cancelExecution)
  await nextTick()
}

/** 关闭 Tab：共享会话的流移交给其他 Tab，未共享的流自动停止。 */
const closingTabPaths = new Set<string>()
const closeTab = async (path: string) => {
  if (closingTabPaths.has(path)) return
  closingTabPaths.add(path)
  try {
    let idx = tabs.value.findIndex(tab => tab.path === path)
    if (idx < 0) return

    if (!await guardProtectedTabClose([path])) return

    await prepareTabViewForClose(path, executionActivity.isTabProtected(path))
    await disposeTabStore(path)
    // Other tab operations may have changed indices while cleanup was awaited.
    idx = tabs.value.findIndex(tab => tab.path === path)
    if (idx < 0) return
    const wasActive = activeTab.value === path
    tabs.value.splice(idx, 1)
    tabItemRefs.delete(path)

    if (!tabs.value.length) {
      tabs.value = defaultHomeTabs()
      const home = tabs.value[0]
      if (home) {
        await router.push(tabToLocation(home))
      }
      refreshTabFn?.(path, { remountWsp: false })
      return
    }

    if (!wasActive) {
      refreshTabFn?.(path, { remountWsp: false })
      // 关闭非当前 Tab：剪枝卸载可能误清宿主；按当前路由重新对齐
      return
    }

    const next = tabs.value[Math.min(idx, tabs.value.length - 1)]
    if (next) {
      await router.push(tabToLocation(next))
    }
    // Invalidate the old key only after the router no longer renders that view.
    refreshTabFn?.(path, { remountWsp: false })
  } finally {
    closingTabPaths.delete(path)
  }
}

provide('closeTab', closeTab)

/** 仅移除 tab（不跳转）；对话 tab 同样先停流再释放 store */
const removeTab = async (path: string) => {
  if (closingTabPaths.has(path)) return
  closingTabPaths.add(path)
  try {
    if (!await guardProtectedTabClose([path])) return
    await prepareTabViewForClose(path, executionActivity.isTabProtected(path))
    await disposeTabStore(path)
    const idx = tabs.value.findIndex(tab => tab.path === path)
    if (idx < 0) return
    tabs.value.splice(idx, 1)
    tabItemRefs.delete(path)
    refreshTabFn?.(path, { remountWsp: false })
  } finally {
    closingTabPaths.delete(path)
  }
}
provide('removeTab', removeTab)

watch(tabs, () => {
  if (!sessionReady) return
  persistTabSession()
  nextTick(updateScrollState)
}, { deep: true })

watch(activeTab, (val) => {
  try { localStorage.setItem(ACTIVE_TAB_KEY, val) } catch { /* ignore */ }
  if (sessionReady) persistTabSession()
})

watch(() => tabs.value.map(tab => isChatTabPath(tab.path) ? getChatTabConversation(tab.path) : null), () => {
  if (sessionReady) persistTabSession()
})

watch(
  () => appStore.tabTitles,
  (val) => {
    for (const [path, title] of Object.entries(val)) {
      const tab = tabs.value.find(t => t.path === path)
      if (tab) tab.title = title
    }
  },
  { deep: true },
)

async function disposeTabStore(path: string) {
  if (path === '/chat') {
    await disposeChatStore('default')
  } else if (path.startsWith('/chat/')) {
    await disposeChatStore(path.slice('/chat/'.length))
  }
}

function updateScrollState() {
  const el = tabberRef.value
  if (!el) {
    showPager.value = false
    canScrollPrev.value = false
    canScrollNext.value = false
    return
  }
  const maxScroll = el.scrollWidth - el.clientWidth
  const nextShow = maxScroll > 1
  const visibilityChanged = showPager.value !== nextShow
  showPager.value = nextShow
  canScrollPrev.value = el.scrollLeft > 1
  canScrollNext.value = el.scrollLeft < maxScroll - 1
  // 翻页按钮显隐会改变可用宽度，再测一次
  if (visibilityChanged) {
    nextTick(updateScrollState)
  }
}

/** 滚轮转为横向滚动 */
const handleWheel = (event: WheelEvent) => {
  const container = tabberRef.value
  if (!container || container.scrollWidth <= container.clientWidth) return

  event.preventDefault()
  const delta = Math.abs(event.deltaY) > Math.abs(event.deltaX)
    ? event.deltaY
    : event.deltaX
  container.scrollLeft += delta
  updateScrollState()
}

const CONTEXT_MENU_WIDTH = 160
const CONTEXT_MENU_ITEM_HEIGHT = 36

const onContextMenu = (event: MouseEvent, path: string) => {
  event.preventDefault()
  const idx = tabs.value.findIndex(t => t.path === path)
  const closableCount = tabs.value.filter(t => t.closable).length

  let menuItemCount = 1
  const tab = tabs.value[idx]
  if (tab?.closable) menuItemCount++
  let leftClosable = false
  let rightClosable = false
  for (let i = 0; i < idx; i++) {
    if (tabs.value[i].closable) { leftClosable = true; break }
  }
  for (let i = idx + 1; i < tabs.value.length; i++) {
    if (tabs.value[i].closable) { rightClosable = true; break }
  }
  if (leftClosable) menuItemCount++
  if (rightClosable) menuItemCount++
  if (closableCount > 0 && (tab?.closable ? closableCount > 1 : closableCount > 0)) menuItemCount++

  const menuHeight = menuItemCount * CONTEXT_MENU_ITEM_HEIGHT + 4

  let x = event.clientX
  let y = event.clientY

  if (x + CONTEXT_MENU_WIDTH > window.innerWidth) {
    x = window.innerWidth - CONTEXT_MENU_WIDTH - 8
  }
  if (y + menuHeight > window.innerHeight) {
    y = window.innerHeight - menuHeight - 8
  }

  contextMenu.value = { visible: true, x, y, tab: tab }
}

const hideContextMenu = () => {
  contextMenu.value.visible = false
}

const onKeydown = (e: KeyboardEvent) => {
  if (e.key === 'Escape') hideContextMenu()
}

function scrollPrevPage() {
  if (!canScrollPrev.value) return
  const el = tabberRef.value
  if (!el) return
  const page = Math.max(80, Math.floor(el.clientWidth * 0.85))
  el.scrollBy({ left: -page, behavior: 'smooth' })
}

function scrollNextPage() {
  if (!canScrollNext.value) return
  const el = tabberRef.value
  if (!el) return
  const page = Math.max(80, Math.floor(el.clientWidth * 0.85))
  el.scrollBy({ left: page, behavior: 'smooth' })
}

function onTabScroll() {
  updateScrollState()
}

onMounted(async () => {
  registerTabSessionFlush?.(flushTabSession)
  registerCloseTab?.(closeTab)
  tabBarRef.value?.addEventListener('wheel', handleWheel, { passive: false })
  tabberRef.value?.addEventListener('scroll', onTabScroll, { passive: true })
  document.addEventListener('click', hideContextMenu)
  document.addEventListener('keydown', onKeydown)
  nextTick(updateScrollState)
  window.addEventListener('resize', updateScrollState)

  // 等设置库加载完成后按模式恢复（与基础参数同源）
  const routePathBeforeSettingsLoad = route.path
  try {
    await settingsStore.loadSettings()
  } catch { /* ignore */ }

  const mode = settingsStore.getTabSessionRestoreMode()
  const snapshot = settingsStore.getTabSessionSnapshot()
  const resolved = resolveTabsFromSnapshot(snapshot, mode)
  tabs.value = dedupeSessionTabs(resolved.tabs.length ? resolved.tabs : defaultHomeTabs())

  // 设置读取是异步的。在等待期间用户可能已经切换页面或发送了首条消息；
  // 此时迟到的“上次激活 tab”不能再覆盖当前操作，否则会出现页面来回闪切。
  const currentChatIsStreaming = listStreamingChatTabPaths().includes(route.path)
  const currentRouteWasUsed = route.path !== routePathBeforeSettingsLoad || currentChatIsStreaming
  if (currentRouteWasUsed && route.path !== '/') {
    const currentTitle = typeof route.query.title === 'string'
      ? route.query.title
      : (typeof route.meta?.title === 'string' ? route.meta.title : 'sidebar.newPage')
    tabs.value = dedupeSessionTabs([
      ...tabs.value,
      { path: route.path, title: currentTitle, closable: true },
    ])
    activeTab.value = route.path
  } else {
    activeTab.value = resolved.activePath || '/chat'
  }
  sessionReady = true

  const target = tabs.value.find(t => t.path === activeTab.value) || tabs.value[0]
  if (target && route.path !== target.path) {
    try {
      await router.replace(tabToLocation(target))
    } catch { /* ignore */ }
  } else if (target) {
    activeTab.value = target.path
  }
  persistTabSession(true)
  nextTick(() => ensureTabVisible(activeTab.value, 'auto'))
})

onUnmounted(() => {
  registerTabSessionFlush?.(null)
  registerCloseTab?.(null)
  if (persistTimer) { clearTimeout(persistTimer); persistTimer = null }
  flushTabSession()
  tabBarRef.value?.removeEventListener('wheel', handleWheel)
  tabberRef.value?.removeEventListener('scroll', onTabScroll)
  document.removeEventListener('click', hideContextMenu)
  document.removeEventListener('keydown', onKeydown)
  window.removeEventListener('resize', updateScrollState)
})

// --- Computed helpers for menu visibility ---
const ctxIdx = computed(() => tabs.value.findIndex(t => t.path === contextMenu.value.tab.path))
const ctxTab = computed(() => tabs.value[ctxIdx.value])

const hasLeftTabs = computed(() => {
  for (let i = 0; i < ctxIdx.value; i++) {
    if (tabs.value[i].closable) return true
  }
  return false
})

const hasRightTabs = computed(() => {
  for (let i = ctxIdx.value + 1; i < tabs.value.length; i++) {
    if (tabs.value[i].closable) return true
  }
  return false
})

const hasOtherClosable = computed(() => {
  return tabs.value.some(t => t.closable && t.path !== contextMenu.value.tab.path)
})

// --- Context menu actions ---
async function refreshCurrentTab() {
  const path = contextMenu.value.tab.path
  hideContextMenu()
  await nextTick()

  if (path !== activeTab.value) {
    await router.push(tabToLocation(contextMenu.value.tab))
    await nextTick()
  }
  if (!await guardProtectedTabClose([path])) return
  await prepareTabViewForClose(path, executionActivity.isTabProtected(path))
  if (isChatTabPath(path)) releaseChatTabState(path === '/chat' ? 'default' : path.slice('/chat/'.length))
  refreshTabFn?.(path)
}

async function closeCurrentTab() {
  const path = contextMenu.value.tab.path
  hideContextMenu()
  await closeTab(path)
}

async function closeLeftTabs() {
  const path = contextMenu.value.tab.path
  hideContextMenu()
  const idx = ctxIdx.value
  const closed = tabs.value.filter((t, i) => t.closable && i < idx)
  if (!await guardProtectedTabClose(closed.map(t => t.path))) return
  await Promise.all(closed.map(async t => {
    await prepareTabViewForClose(t.path, executionActivity.isTabProtected(t.path))
    await disposeTabStore(t.path)
  }))
  tabs.value = tabs.value.filter((t, i) => !t.closable || i >= idx)
  for (const [key] of tabItemRefs) {
    if (!tabs.value.find(t => t.path === key)) tabItemRefs.delete(key)
  }
  if (!tabs.value.find(t => t.path === activeTab.value)) {
    await router.push(tabToLocation(contextMenu.value.tab))
  }
  closed.forEach(t => {
    refreshTabFn?.(t.path, { remountWsp: false })
  })
}

async function closeRightTabs() {
  const path = contextMenu.value.tab.path
  hideContextMenu()
  const idx = ctxIdx.value
  const closed = tabs.value.filter((t, i) => t.closable && i > idx)
  if (!await guardProtectedTabClose(closed.map(t => t.path))) return
  await Promise.all(closed.map(async t => {
    await prepareTabViewForClose(t.path, executionActivity.isTabProtected(t.path))
    await disposeTabStore(t.path)
  }))
  tabs.value = tabs.value.filter((t, i) => !t.closable || i <= idx)
  for (const [key] of tabItemRefs) {
    if (!tabs.value.find(t => t.path === key)) tabItemRefs.delete(key)
  }
  if (!tabs.value.find(t => t.path === activeTab.value)) {
    await router.push(tabToLocation(contextMenu.value.tab))
  }
  closed.forEach(t => {
    refreshTabFn?.(t.path, { remountWsp: false })
  })
}

async function closeOtherTabs() {
  const path = contextMenu.value.tab.path
  hideContextMenu()
  const closed = tabs.value.filter(t => t.closable && t.path !== path)
  if (!await guardProtectedTabClose(closed.map(t => t.path))) return
  await Promise.all(closed.map(async t => {
    await prepareTabViewForClose(t.path, executionActivity.isTabProtected(t.path))
    await disposeTabStore(t.path)
  }))
  tabs.value = tabs.value.filter(t => !t.closable || t.path === path)
  for (const [key] of tabItemRefs) {
    if (!tabs.value.find(t => t.path === key)) tabItemRefs.delete(key)
  }
  if (!tabs.value.find(t => t.path === activeTab.value)) {
    await router.push(tabToLocation(contextMenu.value.tab))
  }
  closed.forEach(t => {
    refreshTabFn?.(t.path, { remountWsp: false })
  })
}

</script>

<template>
  <div ref="tabBarRef" class="tab-bar" data-app-contextmenu>
    <Teleport to="body">
      <Transition name="max-tip-fade">
        <div v-if="showMaxTip" class="tab-max-tip" :style="tipStyle">{{ $t('sidebar.tabMaxTip') }}</div>
      </Transition>
    </Teleport>

    <button
      v-show="showPager"
      type="button"
      class="tab-nav-btn tab-nav-btn--prev"
      :disabled="!canScrollPrev"
      :title="t('tab.prevPage')"
      @click="scrollPrevPage"
    >
      <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2.2" fill="none" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="15 18 9 12 15 6" />
      </svg>
    </button>

    <div ref="tabberRef" class="tab-bar-scroll">
      <div
        v-for="tab in tabs"
        :key="tab.path"
        :ref="el => setTabItemRef(tab.path, el)"
        class="tab-item"
        :class="{ active: activeTab === tab.path }"
        @click="switchTab(tab)"
        @contextmenu.prevent.stop="onContextMenu($event, tab.path)"
      >
        <span v-if="tabIcon(tab.path)" class="tab-icon"><SvgIcon :iconName="tabIcon(tab.path)" /></span>
        <span class="tab-title">{{ tabDisplayTitle(tab.title) }}</span>
        <span v-if="tabExecutionLabel(tab.path)" class="tab-execution-status"
          :class="{ 'is-spinning': tabExecutionSpinning(tab.path) }"
          :title="tabExecutionLabel(tab.path)" role="status"
          :aria-label="tabExecutionLabel(tab.path)">
          <span class="tab-execution-dot" aria-hidden="true" />
        </span>
        <span
          class="tab-close"
          :class="{ 'is-disabled': tabs.length <= 1 }"
          @click.stop="tabs.length > 1 ? closeTab(tab.path) : undefined"
          v-html="'&times;'"
        />
      </div>
    </div>

    <button
      v-show="showPager"
      type="button"
      class="tab-nav-btn tab-nav-btn--next"
      :disabled="!canScrollNext"
      :title="t('tab.nextPage')"
      @click="scrollNextPage"
    >
      <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2.2" fill="none" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="9 18 15 12 9 6" />
      </svg>
    </button>

    <!-- Context menu -->
    <Teleport to="body">
      <div
        v-if="contextMenu.visible"
        class="tab-context-menu"
        :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }"
        @click.stop
      >
        <div class="context-menu-item" @click="refreshCurrentTab">
          <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="1 4 1 10 7 10" />
            <polyline points="23 20 23 14 17 14" />
            <path d="M20.49 9A9 9 0 0 0 5.64 5.64L1 10m22 4l-4.64 4.36A9 9 0 0 1 3.51 15" />
          </svg>
          <span>{{ t('tab.refresh') }}</span>
        </div>
        <div v-if="ctxTab?.closable" class="context-menu-item" @click="closeCurrentTab">
          <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
          <span>{{ t('tab.closeCurrent') }}</span>
        </div>
        <div v-if="hasLeftTabs" class="context-menu-item" @click="closeLeftTabs">
          <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="15 18 9 12 15 6" />
          </svg>
          <span>{{ t('tab.closeLeft') }}</span>
        </div>
        <div v-if="hasRightTabs" class="context-menu-item" @click="closeRightTabs">
          <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="9 18 15 12 9 6" />
          </svg>
          <span>{{ t('tab.closeRight') }}</span>
        </div>
        <div v-if="hasOtherClosable" class="context-menu-item" @click="closeOtherTabs">
          <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10" />
            <line x1="4.93" y1="4.93" x2="19.07" y2="19.07" />
          </svg>
          <span>{{ t('tab.closeOther') }}</span>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style lang="scss" scoped>
.tab-bar {
  display: flex;
  align-items: center;
  height: 36px;
  background: var(--bg-sidebar);
  border-bottom: 1px solid var(--chat-divider-color)!important;
  padding: 0 4px;
  gap: 2px;
  position: relative;
}

.tab-nav-btn {
  flex-shrink: 0;
  width: 24px;
  height: 24px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  padding: 0;

  &:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  &:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }
}

.tab-bar-scroll {
  display: flex;
  align-items: center;
  flex: 1;
  min-width: 0;
  height: 100%;
  gap: 2px;
  overflow-x: auto;
  overflow-y: hidden;
  position: relative;
  overscroll-behavior-x: contain;

  // 隐藏滚动条，保留滚轮横向滚动
  scrollbar-width: none;
  -ms-overflow-style: none;
  &::-webkit-scrollbar {
    display: none;
  }
}

.tab-item {
  display: flex;
  align-items: center;
  height: 28px;
  padding: 0 10px;
  font-size: var(--font-size-base);
  cursor: pointer;
  border-radius: 6px 6px 0 0;
  color: var(--text-secondary);
  gap: 6px;
  white-space: nowrap;
  flex-shrink: 0;
  transition: background 0.15s, color 0.15s;

  &:hover {
    background: var(--bg-hover);
  }

  &.active {
    background: var(--main-meun-active-background);
    color: var(--meum-wenzhi-active-color);
    font-weight: 700;
  }
}

.tab-icon {
  width: 15px;
  height: 15px;
  flex-shrink: 0;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;

  :deep(svg) {
    width: 15px;
    height: 15px;
  }
}

.tab-execution-status {
  width: 14px;
  height: 14px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  color: #e6a23c;
}

.tab-execution-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: currentColor;
}

.tab-execution-status.is-spinning {
  color: #409eff;
}

.tab-execution-status.is-spinning .tab-execution-dot {
  width: 10px;
  height: 10px;
  border: 2px solid color-mix(in srgb, currentColor 25%, transparent);
  border-top-color: currentColor;
  background: transparent;
  animation: tab-execution-spin 1s linear infinite;
}

@keyframes tab-execution-spin {
  to { transform: rotate(360deg); }
}

@media (prefers-reduced-motion: reduce) {
  .tab-execution-status.is-spinning .tab-execution-dot { animation: none; }
}

.tab-close {
  font-size: var(--font-size-base);
  width: 16px;
  height: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 3px;

  &:hover {
    background: rgba(0, 0, 0, 0.1);
  }

  &.is-disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }
}

.tab-add {
  width: 24px;
  height: 24px;
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 16px;
  color: var(--text-secondary);
  border-radius: 4px;

  &:hover {
    background: var(--bg-hover);
  }
}

.tab-max-tip {
  z-index: 100;
  padding: 6px 16px;
  background: #fffbe6;
  color: #d48806;
  font-size: 13px;
  border-radius: 8px;
  white-space: nowrap;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
  border: 1px solid #ffe58f;
  pointer-events: none;
}

.max-tip-fade-enter-active,
.max-tip-fade-leave-active {
  transition: opacity 0.3s ease, transform 0.3s ease;
}
.max-tip-fade-enter-from,
.max-tip-fade-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(4px);
}
</style>

<style lang="scss">
.tab-context-menu {
  position: fixed;
  z-index: 9999;
  min-width: 160px;
  background: var(--theme-dailog-color) ;
  border: 1px solid var(--border-color, #e5e7eb);
  border-radius: 8px;
  padding: 4px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
}

.context-menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 36px;
  padding: 0 10px;
  font-size: 13px;
  color: var(--text-primary);
  cursor: pointer;
  border-radius: 6px;
  white-space: nowrap;

  svg {
    flex-shrink: 0;
    opacity: 0.7;
  }

  &:hover {
    background: var(--bg-hover, rgba(0, 0, 0, 0.05));
  }
}
</style>

<style>
.tab-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  overflow: hidden;
  width: 22px;
  height: 22px;
}
.tab-icon svg {
  width: 100% !important;
  height: 100% !important;
}
</style>

