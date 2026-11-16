<script setup lang="ts">
// FastClaw · 主布局
// 第一层：顶部标题栏（Logo + 广告条 + 窗口控制）
// 第二层：左侧树形菜单 + 右侧内容区（顶部 Tab 标签栏 + 主内容）

import {
  computed,
  defineComponent,
  h,
  isVNode,
  markRaw,
  nextTick,
  onMounted,
  onUnmounted,
  provide,
  ref,
  watch,
  type Component,
  type VNode,
} from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { ElNotification } from 'element-plus'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { FASTPLAN_RESUME_KEY, provideFastPlanResume } from '@/api/fastPlanResume'
import zhCn from 'element-plus/es/locale/lang/zh-cn'
import en from 'element-plus/es/locale/lang/en'
import { useAppStore } from '@/stores/app'
import { useThemeStore } from '@/stores/theme'
import { useSettingsStore } from '@/stores/settings'
import TreeMenu from '@/components/common/TreeMenu.vue'
import TabBar from '@/components/common/TabBar.vue'
import SettingsView from '@/views/SettingsView.vue'
import ThemeSelectorModal from '@/components/theme/ThemeSelectorModal.vue'
import { checkConnectivity } from '@/api/connectivity'
import { CHAT_VIEW_HOT_CACHE_SIZE } from '@/config/constants'
import AboutModal from '@/components/about/AboutModal.vue'
import { loadBranding, displayName, appLogo } from '@/api/branding'
import logoSvg from '@/assets/logo.svg?raw'
import { DownloadCenterButton as DownloadButton, DownloadCenterPanel as DownloadCenter } from 'tauri-download-center-vue'
import EditableContextMenu from '@/components/common/EditableContextMenu.vue'
import WindowFileDrop from '@/components/common/WindowFileDrop.vue'
import {
  installClearSelectionOnClickOutside,
  uninstallClearSelectionOnClickOutside,
} from '@/utils/textSelection'
import { disposeChatScreenshotEventBridge } from '@/utils/chatScreenshotEventBridge'
import { fastPlanSessionManager } from '@/integrations/chat-plan/sessionManager'
import { chatSwitchPerf, installChatSwitchPerfDiagnostics } from '@/utils/chatSwitchPerf'
import { captureChatViewIdentity, CHAT_VIEW_IDENTITY } from '@/modules/chat/viewIdentity'
import { ensureFastKgPackageDownloaded } from '@/api/fastKgAutoDownload'

const router = useRouter()
const route = useRoute()
const removeChatSwitchPerfGuard = router.beforeEach((to, from) => {
  chatSwitchPerf.begin(from.path, to.path)
})
let recoveryUnlisten: (() => void) | undefined
let recoveryDisposed = false
let resumeUnlisten: (() => void) | undefined
onMounted(async () => {
  // 与入口请求合并；事件系统初始化不能阻塞服务调度。
  void provideFastPlanResume()
  try {
    const unlisten = await listen<{ serviceId: string; message: string }>('dependency-recovery-failed', ({ payload }) => {
      const names: Record<string, string> = { qdrant: '向量库服务', llama_server: '嵌入向量服务', fastkg: '知识库服务', fastplan: 'FastPlan' }
      ElNotification({ title: `${names[payload.serviceId] || payload.serviceId} 恢复失败`, message: `${payload.message}。点击进入依赖服务页面处理。`,
        type: 'error', duration: 0, onClick: () => { void router.push({ path: '/wsp-admin', query: { tab: 'deps' } }) } })
    })
    if (recoveryDisposed) unlisten()
    else recoveryUnlisten = unlisten
    let wasHealthy = false
    const saveResume = async () => {
      try {
        const data = await invoke('get_fastplan_resume_data')
        if (data) localStorage.setItem(FASTPLAN_RESUME_KEY, JSON.stringify(data))
      } catch { /* 存储不可用不影响当前连接，不打印令牌 */ }
    }
    const resumeListener = await listen<{ id: string; recovery_state: string }[]>('dependency-recovery-state', ({ payload }) => {
      const healthy = payload.find(item => item.id === 'fastplan')?.recovery_state === 'healthy'
      if (healthy && !wasHealthy) void saveResume()
      wasHealthy = healthy
    })
    if (recoveryDisposed) resumeListener()
    else resumeUnlisten = resumeListener
    await saveResume()
  } catch { /* 非桌面环境 */ }
})
onUnmounted(() => { recoveryDisposed = true; recoveryUnlisten?.(); resumeUnlisten?.() })
onMounted(installChatSwitchPerfDiagnostics)
onUnmounted(() => {
  removeChatSwitchPerfGuard()
  chatSwitchPerf.dispose()
})
const appStore = useAppStore()
// const menuStore = useMenuStore()
const themeStore = useThemeStore()
const settingsStore = useSettingsStore()

// ─── 标题栏副标题 ───────────────────────
const subTitle = ref('多工具、自规划的协同任务龙虾')

const menuCollapsed = ref(localStorage.getItem('fastclaw-menu-collapsed') === '1')
const showThemeSelector = ref(false)
const showAboutModal = ref(false)

// 标题栏菜单
async function handleMenuCommand(cmd: string) {
  if (cmd === 'about') {
    showAboutModal.value = true
  }
  // 其他菜单项暂不处理
}

// shuttingDown 已移除：退出时直接隐藏窗口，无需遮罩

// Tab refresh: per-path counter used as component key to force keep-alive remount
const refreshKeys = ref<Record<string, number>>({})
interface RouteCacheRecord {
  name: string
  component: Component
}
const routeCacheRecords = new Map<string, RouteCacheRecord>()
const excludedRouteCacheNames = ref<string[]>([])
const pruningRouteCacheKeys = new Set<string>()
let routeCacheSequence = 0

function routeCacheKey(path: string): string {
  return `${path}:${refreshKeys.value[path] ?? 0}`
}

/**
 * KeepAlive's public include/exclude API works by component name rather than
 * vnode key. Give every tab incarnation a unique named wrapper so a closed tab
 * can be pruned without evicting the other open tabs.
 */
function getRouteCacheComponent(component: Component | VNode, path: string, instanceId?: string | string[]): Component {
  const key = routeCacheKey(path)
  const cached = routeCacheRecords.get(key)
  if (cached) return cached.component

  const name = `FastClawRouteCache${++routeCacheSequence}`
  // RouterView exposes a VNode whose `component` field belongs to that render.
  // Re-cloning it after KeepAlive deactivation also clones the stale internal
  // instance pointer. Cache only the route component type/props and create a
  // clean VNode on every wrapper render.
  const routeComponent = (isVNode(component) ? component.type : component) as Component
  const routeProps = isVNode(component) ? component.props : null
  const routeRef = isVNode(component) ? component.ref : null
  // 在创建缓存记录时冻结身份，不能等异步路由/ChatView 加载完再读共享 useRoute。
  const chatIdentity = captureChatViewIdentity({ path, params: { iid: instanceId } })
  const wrapped = markRaw(defineComponent({
    name,
    inheritAttrs: false,
    setup(_props, { attrs, slots }) {
      if (chatIdentity) provide(CHAT_VIEW_IDENTITY, chatIdentity)
      return () => h(routeComponent, {
        ...(routeProps || {}),
        ...attrs,
        ...(routeRef ? { ref: routeRef } : {}),
      }, slots)
    },
  }))
  routeCacheRecords.set(key, { name, component: wrapped })
  return wrapped
}

function pruneRouteCache(path: string) {
  const key = routeCacheKey(path)
  const record = routeCacheRecords.get(key)
  if (!record || pruningRouteCacheKeys.has(key)) return

  pruningRouteCacheKeys.add(key)
  excludedRouteCacheNames.value = [...excludedRouteCacheNames.value, record.name]
  // KeepAlive applies exclude in a post-flush watcher. Keep the unique name
  // excluded through two render ticks so rapid route changes cannot reactivate
  // the vnode while it is being pruned.
  void nextTick().then(() => nextTick()).then(() => {
    excludedRouteCacheNames.value = excludedRouteCacheNames.value.filter(name => name !== record.name)
    routeCacheRecords.delete(key)
    pruningRouteCacheKeys.delete(key)
  })
}

// ChatView 含有完整时间线、Markdown 和 FastPlan 渲染树。只保留当前
// 以及最近两个会话的重型 DOM；其余 Tab 的 Pinia/会话快照仍保留。
// 流式聊天由 Chat store、FastPlan 由 SessionManager 在视图外继续运行。
let recentChatCachePaths: string[] = []
let chatCacheTrimGeneration = 0

function isChatRoutePath(path: string) {
  return path === '/chat' || path.startsWith('/chat/')
}

function scheduleChatCacheTrim(path: string) {
  if (!isChatRoutePath(path)) return
  recentChatCachePaths = [path, ...recentChatCachePaths.filter(item => item !== path)]
  chatSwitchPerf.setCachedPaths(recentChatCachePaths.slice(0, CHAT_VIEW_HOT_CACHE_SIZE))
  const generation = ++chatCacheTrimGeneration
  void nextTick().then(() => {
    if (generation !== chatCacheTrimGeneration) return
    const keep = new Set(recentChatCachePaths.slice(0, CHAT_VIEW_HOT_CACHE_SIZE))
    for (const cachedPath of recentChatCachePaths) {
      if (!keep.has(cachedPath)) pruneRouteCache(cachedPath)
    }
    recentChatCachePaths = recentChatCachePaths.filter(item => keep.has(item))
    chatSwitchPerf.setCachedPaths(recentChatCachePaths)
  })
}
// TabBar 重载 key。保持不变，避免整栏重载清掉其它标签。
const tabBarKey = ref(0)
/** TabBar 注册的会话落盘回调（关闭窗口前调用） */
let tabSessionFlush: (() => void | Promise<void>) | null = null
/** TabBar：按 path 关闭标签（历史会话删除等场景） */
let closeTabByPath: ((path: string) => void | Promise<void>) | null = null
provide('registerTabSessionFlush', (fn: (() => void | Promise<void>) | null) => {
  tabSessionFlush = fn
})
provide('registerCloseTab', (fn: ((path: string) => void | Promise<void>) | null) => {
  closeTabByPath = fn
})
provide('closeTab', async (path: string) => {
  await closeTabByPath?.(path)
})

provide('refreshTab', (path: string, options?: { remountWsp?: boolean }) => {
  pruneRouteCache(path)
  refreshKeys.value = {
    ...refreshKeys.value,
    [path]: (refreshKeys.value[path] ?? 0) + 1,
  }
  // 关闭 Tab 时只剪枝 keep-alive，不要 remount 常驻 iframe（否则易与当前 Tab 错位）
  if (options?.remountWsp === false) return
})

const { t, locale } = useI18n()
const elLocale = computed(() => locale.value === 'en' ? en : zhCn)
function applyLanguage(lang: string) {
  const v = lang === 'en' ? 'en' : 'zh-CN'
  locale.value = v
  localStorage.setItem('fastclaw-language', v)
  document.querySelectorAll('iframe').forEach(iframe => {
    try { iframe.contentWindow?.postMessage({ type: 'LANGUAGE_CHANGE', lang: v }, '*') } catch (_) { }
  })
}

// 初始化时从设置恢复语言
settingsStore.loadSettings().then(() => {
  applyLanguage(settingsStore.settings.language || 'zh-CN')
})

watch(() => settingsStore.settings.language, (lang) => {
  applyLanguage(lang || 'zh-CN')
})

// 系统名称变化时同步操作系统窗口标题（任务栏显示）
watch(displayName, (name) => {
  try {
    getCurrentWindow().setTitle(name)
  } catch { /* 非 Tauri 环境忽略 */ }
}, { immediate: true })

// 立即初始化主题，确保 CSS 变量在组件渲染前生效
themeStore.initTheme()

// 每次路由切换时重新应用主题变量，解决 keep-alive 缓存组件不更新的问题
watch(() => route.path, (path, previousPath) => {
  themeStore.applyCurrent()
  if (isChatRoutePath(path)) {
    if (previousPath) chatSwitchPerf.ensure(previousPath, path)
    chatSwitchPerf.markRouteCommitted(path)
  }
  scheduleChatCacheTrim(path)
}, { immediate: true })

// 主题变更时重新应用 CSS 变量（不触发全局重排，keep-alive 组件无需强制重绘）
watch(() => themeStore.current, () => {
  themeStore.applyCurrent()
}, { deep: true })

// 外部链接点击拦截处理器引用（模块作用域，供 onMounted/onUnmounted 共用）
let _externalLinkClickHandler: ((e: MouseEvent) => void) | null = null
/** 窗口尺寸变化防抖定时器（onResized 内落盘用） */
let windowStateSaveTimer: ReturnType<typeof setTimeout> | null = null

onMounted(async () => {
  checkConnectivity()
  loadBranding()        // 加载自定义系统名称/图标
  // 加载应用配置（exe 同目录 config.json：服务 URL + 菜单/设置页显隐）
  import('@/stores/appConfig').then(async ({ useAppConfigStore }) => {
    const configStore = useAppConfigStore()
    if (!configStore.loaded) await configStore.loadConfig()
    void ensureFastKgPackageDownloaded()
  }).catch(() => { })
  // await menuStore.loadMenus()
  // 监听窗口大小变化，同步最大化按钮状态（双击标题栏等方式也会触发）
  try {
    const appWindow = getCurrentWindow()
    isWindowMaximized.value = await appWindow.isMaximized()
    await appWindow.onResized(async () => {
      isWindowMaximized.value = await appWindow.isMaximized()
      // 防抖落盘：即使 tauri:dev 强杀也能保留最近一次尺寸
      if (windowStateSaveTimer) clearTimeout(windowStateSaveTimer)
      windowStateSaveTimer = setTimeout(() => {
        windowStateSaveTimer = null
        void invoke('save_main_window_state').catch(() => { /* ignore */ })
      }, 600)
    })
  } catch { /* 非 Tauri 环境忽略 */ }

  // 拦截外部链接跳转：在系统浏览器中打开，防止覆盖当前 Vue 页面
  _externalLinkClickHandler = (e: MouseEvent) => {
    // 向上查找最近的 <a> 标签
    let el: HTMLElement | null = e.target as HTMLElement
    while (el && el.tagName !== 'A') {
      el = el.parentElement
    }
    if (!el) return

    const anchor = el as HTMLAnchorElement
    const href = anchor.getAttribute('href')
    if (!href) return

    // 跳过内部路由链接、锚点、特殊协议
    if (
      href.startsWith('/') ||
      href.startsWith('#') ||
      href.startsWith('javascript:') ||
      href.startsWith('mailto:') ||
      href.startsWith('tel:')
    ) {
      return
    }

    // 跳过 download 属性（文件下载链接）
    if (anchor.hasAttribute('download')) return

    // 外部 URL（http/https）→ 系统默认浏览器打开
    if (/^https?:\/\//i.test(href)) {
      e.preventDefault()
      e.stopPropagation()
      invoke('open_external', { url: href }).catch(() => {
        // fallback：非 Tauri 环境（开发时浏览器）
        window.open(href, '_blank')
      })
    }
  }

  document.addEventListener('click', _externalLinkClickHandler, true)
  installClearSelectionOnClickOutside()
  // 监听个人资料头像变更事件（本地模式依赖 localStorage，需要事件触发刷新）
  import('@/utils/screenshotWindow')
    .then(({ prewarmScreenshotWindow }) => prewarmScreenshotWindow())
    .catch(() => { /* 预热失败不影响功能 */ })
})

onUnmounted(async () => {
  fastPlanSessionManager.disposeAll()
  await disposeChatScreenshotEventBridge()
  // 移除外部链接点击拦截
  if (_externalLinkClickHandler) {
    document.removeEventListener('click', _externalLinkClickHandler, true)
    _externalLinkClickHandler = null
  }
  uninstallClearSelectionOnClickOutside()
  if (windowStateSaveTimer) {
    clearTimeout(windowStateSaveTimer)
    windowStateSaveTimer = null
  }
  try {
    await invoke('stop_knowledge_service')
  } catch (e) {
    console.log('Stop knowledge service on exit:', e)
  }
})

const toggleMenu = () => {
  menuCollapsed.value = !menuCollapsed.value
  try {
    localStorage.setItem('fastclaw-menu-collapsed', menuCollapsed.value ? '1' : '0')
  } catch {
    // ignore
  }
}

const toggleThemeSelector = () => {
  showThemeSelector.value = !showThemeSelector.value
}

const minimize = async () => {
  try {
    const appWindow = getCurrentWindow();
    await appWindow.minimize()
  } catch (e) { console.log('Minimize window', e) }
}

const isWindowMaximized = ref(false)

const maximize = async () => {
  try {
    const appWindow = getCurrentWindow();
    if (isWindowMaximized.value) {
      await appWindow.unmaximize()
    } else {
      await appWindow.maximize()
    }
    isWindowMaximized.value = !isWindowMaximized.value
  } catch (e) { console.log('Maximize window', e) }
}

/**
 * 标题栏拖拽：不用系统 drag-region，避免右键弹出 Windows 系统菜单
 *
 * click / drag 分离 + 拖拽阈值：
 * mousedown 不再立即 startDragging（快速点击时真实 mouse-up 会先于合成的
 * WM_NCLBUTTONDOWN 派发，导致 Win32 模态移动循环等不到退出信号而"拖拽锁死"），
 * 而是等位移 ≥ 4px 且左键仍按着时，调用带物理左键守卫的 drag_window_if_held。
 */
const TITLEBAR_DRAG_THRESHOLD_PX = 4

const onTitleBarMouseDown = (e: MouseEvent) => {
  if (e.button !== 0) return
  // 双击的第二次 mousedown 不进拖拽路径，留给 onTitleBarDblClick 最大化
  if (e.detail >= 2) return
  const el = e.target as HTMLElement | null
  if (!el) return
  if (el.closest('button, a, input, textarea, select, .el-dropdown, .titlebar-user, [data-tauri-drag-region="false"]')) {
    return
  }

  const startX = e.screenX
  const startY = e.screenY

  const cleanup = () => {
    window.removeEventListener('mousemove', onMouseMove)
    window.removeEventListener('mouseup', onMouseUp)
  }

  const onMouseMove = (ev: MouseEvent) => {
    // 左键已松开（mousemove 可能在 up 之后触发）→ 视为点击，取消
    if ((ev.buttons & 1) === 0) {
      cleanup()
      return
    }
    const dx = ev.screenX - startX
    const dy = ev.screenY - startY
    if (dx * dx + dy * dy < TITLEBAR_DRAG_THRESHOLD_PX * TITLEBAR_DRAG_THRESHOLD_PX) return
    cleanup()
    try {
      void invoke<boolean>('drag_window_if_held').catch(() => { /* 非 Tauri 环境忽略 */ })
    } catch {
      /* 非 Tauri 环境忽略 */
    }
  }

  const onMouseUp = () => cleanup()

  window.addEventListener('mousemove', onMouseMove)
  window.addEventListener('mouseup', onMouseUp)
}

const onTitleBarDblClick = async (e: MouseEvent) => {
  const el = e.target as HTMLElement | null
  if (!el) return
  if (el.closest('button, a, input, textarea, select, .el-dropdown, .titlebar-user')) return
  await maximize()
}

const close = async () => {
  try {
    // 关闭前落盘标签会话（未完成任务检测结果一并写入 settings）
    try { await tabSessionFlush?.() } catch { /* ignore */ }

    const isTrayEnabled = await invoke<boolean>('get_system_tray_enabled');
    const appWindow = getCurrentWindow();

    // ★ 必须在 hide 之前保存窗口尺寸；hide 后可能变成 0 并覆盖存档
    try { await invoke('save_main_window_state') } catch { /* ignore */ }

    if (isTrayEnabled) {
      await appWindow.hide();
    } else {
      // 立即隐藏窗口，后台清理子进程后自动退出
      await appWindow.hide();
      try {
        await invoke('exit_app'); // 等待退出命令完成（修复 #133）
      } catch (e) {
        console.log('exit_app failed, falling back to window.close()', e);
        await appWindow.close();
      }
    }
  } catch (e) {
    console.log('Close window', e)
    try {
      const appWindow = getCurrentWindow();
      await appWindow.close()
    } catch (e2) { }
  }
}

// ── 全局快捷键监听（已禁用）──
/*
function getShortcutMap(): Record<string, string> {
  try { const raw = localStorage.getItem('fastclaw-shortcuts'); if (raw) return JSON.parse(raw) } catch {}
  return {}
}
const defaultShortcuts: Record<string, string> = {
  zoomIn: 'Ctrl+=', zoomOut: 'Ctrl+-', zoomReset: 'Ctrl+0',
  showApp: 'Ctrl+B', newTopic: 'Ctrl+A', clearMessage: 'Ctrl+L', clearContext: 'Ctrl+K',
}
function matchShortcut(e: KeyboardEvent, key: string): boolean {
  if (!key) return false
  const parts = key.replace(/ctrl/i,'Ctrl').replace(/alt/i,'Alt').replace(/shift/i,'Shift').replace(/\s+/g,'').split('+')
  const mods = parts.slice(0,-1), k = parts[parts.length-1]
  if ((e.ctrlKey||e.metaKey) !== mods.includes('Ctrl')) return false
  if (e.altKey !== mods.includes('Alt')) return false
  if (e.shiftKey !== mods.includes('Shift')) return false
  const ek = e.key.toUpperCase(), mk = k.toUpperCase()
  if (ek===mk) return true
  if (mk==='=' && (ek==='='||ek==='+')) return true
  if (mk==='-' && (ek==='-'||ek==='_')) return true
  return false
}
async function executeShortcut(id: string) {
  switch (id) {
    case 'zoomIn': try { await invoke('maximize_window') } catch(e){} break
    case 'zoomOut': try { await invoke('restore_window') } catch(e){} break
    case 'zoomReset': try { const w = getCurrentWindow(); await w.minimize() } catch(e){} break
    case 'showApp': try {
      const w = getCurrentWindow()
      await w.show()
      await w.setFocus()
      await w.unminimize()
    } catch(e){} break
    case 'newTopic': try {
      const { useChatStore } = await import('@/stores/chat')
      const iid = route.path.startsWith('/chat') ? (route.params.iid as string || 'default') : 'default'
      useChatStore(iid).createConversation()
    } catch(e){} break
  }
}
document.addEventListener('keydown', async (e: KeyboardEvent) => {
  const map = getShortcutMap()
  const all = { ...defaultShortcuts, ...map }
  for (const [id, key] of Object.entries(all)) {
    if (matchShortcut(e, key)) { e.preventDefault(); e.stopPropagation(); await executeShortcut(id); return }
  }
}, true)
*/
</script>

<template>
  <el-config-provider :locale="elLocale">
    <div class="app-container">
      <!-- 第一层：标题栏 -->
      <header class="title-bar" @mousedown="onTitleBarMouseDown" @dblclick="onTitleBarDblClick">
        <div class="title-bar__left">
          <!-- <img :src="appLogo || logoImg" alt="logo" class="logo-img" />  -->
          <img v-if="appLogo" :src="appLogo" alt="logo" class="logo-img" />
          <span v-else v-html="logoSvg" class="logo-img logo-svg" />
          <span class="brand">{{ displayName }}</span>
          <span class="subtitle">{{ subTitle }}</span>
        </div>
        <div class="title-bar__center">
        </div>
        <div class="title-bar__right">
          <!-- 菜单 -->
          <el-dropdown trigger="click" @command="handleMenuCommand" popper-class="titlebar-menu-popper">
            <button class="win-btn" :title="$t('app.menu')" @click.stop>
              <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="2" fill="none"
                stroke-linecap="round" stroke-linejoin="round">
                <line x1="3" y1="6" x2="21" y2="6"></line>
                <line x1="3" y1="12" x2="21" y2="12"></line>
                <line x1="3" y1="18" x2="21" y2="18"></line>
              </svg>
            </button>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="devTools">{{ $t('app.devTools') }}</el-dropdown-item>
                <el-dropdown-item command="reportIssue">{{ $t('app.reportIssue') }}</el-dropdown-item>
                <el-dropdown-item command="helpDoc">{{ $t('app.helpDoc') }}</el-dropdown-item>
                <el-dropdown-item command="trainingVideo">{{ $t('app.trainingVideo') }}</el-dropdown-item>
                <el-dropdown-item command="contactUs">{{ $t('app.contactUs') }}</el-dropdown-item>
                <el-dropdown-item command="viewLog">{{ $t('app.viewLog') }}</el-dropdown-item>
                <el-dropdown-item command="about">{{ $t('app.aboutSystem') }}</el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>

          <!-- 独立下载中心入口 -->
          <DownloadButton />

          <button class="win-btn theme-switch-btn" :class="{ active: showThemeSelector }" :title="$t('app.themeSwitch')"
            @click="toggleThemeSelector">
            <svg viewBox="0 0 1021 1021" width="18" height="18" fill="currentColor">
              <path d="M737.1008 203.6992l146.89792 149.4016-70.20032 71.3984a54.76352 54.76352 0 0 0-24.00256-5.4016c-30.69952 0-55.70048 25.00096-55.70048 55.79776v345.90208H291.90144V474.79808c0-30.80192-25.00096-55.79776-55.70048-55.79776-8.4992 0-16.59904 1.89952-24.00256 5.4016L141.99808 353.1008l146.89792-149.4016h44.60032c40.00256 45.10208 128.50176 65.39776 179.50208 65.39776s139.40224-20.3008 179.50208-65.39776h44.60032m2.2016-49.9968h-49.30048c-13.10208 0-25.6 5.60128-34.0992 15.69792-25.29792 30.19776-97.50016 49.69984-143.0016 49.69984s-117.69856-19.50208-143.0016-49.69984c-8.4992-10.10176-21.00224-15.69792-34.0992-15.69792h-49.30048a44.6464 44.6464 0 0 0-32 13.50144l-163.59936 166.5024c-10.50112 10.7008-10.50112 28.00128 0 38.70208L190.80192 474.0096c5.2992 5.2992 12.20096 8.00256 19.00032 8.00256s13.7984-2.69824 19.00032-8.00256l3.3024-3.39968c1.19808-1.19808 2.60096-1.69984 3.99872-1.69984 2.89792 0 5.69856 2.29888 5.69856 5.80096v354.29888c0 22.99904 18.29888 41.7024 41.00096 41.7024h460.29824c22.59968 0 41.00096-18.69824 41.00096-41.7024v-354.2016c0-3.50208 2.80064-5.80096 5.69856-5.80096 1.39776 0 2.80064 0.50176 3.99872 1.69984l3.3024 3.39968c5.2992 5.2992 12.20096 8.00256 19.00032 8.00256s13.7984-2.69824 19.00032-8.00256l99.90144-101.60128c10.50112-10.7008 10.50112-28.00128 0-38.70208l-163.70176-166.70208a44.9536 44.9536 0 0 0-32-13.39904z" />
            </svg>
          </button>

          <button class="win-btn" :title="$t('settings.title')" data-tauri-drag-region="false" style="-webkit-app-region: no-drag" @click="appStore.showSettings = true">
            <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="2"
              fill="none" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="12" cy="12" r="3" />
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.6a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
            </svg>
          </button>

          <!-- 分割线：用户区域与窗口控制按钮 -->
          <span class="titlebar-divider"></span>

          <button class="win-btn" :title="$t('app.minimize')" data-tauri-drag-region="false" @click="minimize">
            <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"
              stroke-linecap="round" stroke-linejoin="round">
              <line x1="5" y1="12" x2="19" y2="12"></line>
            </svg>
          </button>
          <button class="win-btn" :title="isWindowMaximized ? $t('app.restore') : $t('app.maximize')"
            data-tauri-drag-region="false" style="-webkit-app-region: no-drag" @click="maximize">
            <svg v-if="isWindowMaximized" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor"
              stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
              <path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3">
              </path>
            </svg>
            <svg v-else viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"
              stroke-linecap="round" stroke-linejoin="round">
              <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
            </svg>
          </button>
          <button class="win-btn win-btn--close" :title="$t('app.close')" data-tauri-drag-region="false" @click="close">
            <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"
              stroke-linecap="round" stroke-linejoin="round">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
      </header>

      <!-- 第二层：左侧菜单 + 右侧内容区 -->
      <div class="main-body">
        <!-- 左侧树形菜单 -->
        <aside class="sidebar" :class="{ collapsed: menuCollapsed }">
          <TreeMenu :collapsed="menuCollapsed" @toggle-collapse="toggleMenu" />
        </aside>

        <!-- 右侧内容区 -->
        <main class="content">
          <!-- Tab 标签栏 -->
          <TabBar :key="tabBarKey" />
          <div id="topbar-actions" class="topbar-actions"></div>
          <div class="content-box">
            <router-view v-slot="{ Component, route: slotRoute }">
              <keep-alive :exclude="excludedRouteCacheNames">
                <component
                  :is="getRouteCacheComponent(Component, slotRoute.path, slotRoute.params.iid)"
                  :key="`${slotRoute.path}:${refreshKeys[slotRoute.path] ?? 0}`"
                />
              </keep-alive>
            </router-view>
          </div>
        </main>
      </div>

      <!-- 全局设置弹窗 -->
      <SettingsView v-if="appStore.showSettings" @close="appStore.showSettings = false" />
      <ThemeSelectorModal v-model="showThemeSelector" />

      <!-- 关于我们 -->
      <AboutModal v-model="showAboutModal" />

      <!-- 下载中心悬浮面板 -->
      <DownloadCenter />

      <!-- 输入框自定义右键菜单 -->
      <EditableContextMenu />
      <WindowFileDrop />

    </div>
  </el-config-provider>
</template>

<style lang="scss" scoped>
.app-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
  background: var(--app-background);
  background-position: center;
  background-repeat: no-repeat;
  background-size: cover;
  color: var(--text-primary);
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
}

.title-bar {
  display: flex;
  align-items: center;
  height: 56px;
  // background: var(--bg-titlebar);
  background: var(--bg-sidebar);
  padding: 0 16px;
  user-select: none;
  border-bottom: 1px solid var(--chat-divider-color) !important; //顶部与tab之间的分割线
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.04);

  &__left {
    display: flex;
    align-items: center;
    width: 370px;

    // padding-left: 16px;
    .logo-img {
      width: 20px;
      height: 20px;
      margin-right: 8px;
      object-fit: contain;
    }

    .logo-svg {
      color: var(--color-primary);

      :deep(svg) {
        width: 100%;
        height: 100%;
        display: block;
      }
    }

    .brand {
      font-size: var(--font-size-base);
      font-weight: 600;
      color: var(--text-primary);
      letter-spacing: 0.5px;
    }

    .subtitle {
      margin-left: 12px;
      font-size: var(--font-size-sm);
      color: var(--text-secondary);
      letter-spacing: 0.3px;
    }
  }

  &__center {
    flex: 1;
    display: flex;
    justify-content: center;
  }

  &__right {
    display: flex;
    align-items: center;
    gap: 8px;
    -webkit-app-region: no-drag;
    padding-right: 1px;
    /* 偏移按钮位置，阻止 Windows 识别为原生窗口按钮 */

    // 用户区域
    .titlebar-user {
      display: flex;
      align-items: center;
      gap: 6px;
      padding: 3px 3px;
      border-radius: 6px;
      cursor: pointer;
      transition: background 0.2s;
      max-width: 140px;

      &:hover {
        background: var(--bg-hover);
      }

      .titlebar-user-avatar {
        width: 26px;
        height: 26px;
        border-radius: 50%;
        flex-shrink: 0;
        overflow: hidden;
        // background: var(--color-primary, #3b82f6);
        display: flex;
        align-items: center;
        justify-content: center;
      }

      .titlebar-user-avatar-img {
        width: 100%;
        height: 100%;
        object-fit: cover;
      }

      .titlebar-user-icon {
        color: var(--text-primary);
        display: flex;
        align-items: center;
        justify-content: center;

        &>svg {
          width: 15px;
          height: 15px;
        }
      }

      .titlebar-user-name {
        font-size: var(--font-size-md);
        color: var(--text-primary);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        max-width: 80px;
      }
    }

    // 分割线
    .titlebar-divider {
      width: 1px;
      height: 20px;
      background: var(--border-color);
      opacity: 0.5;
      margin: 0 2px;
      flex-shrink: 0;
    }

    .win-btn {
      width: 32px;
      height: 32px;
      border: none;
      background: transparent;
      cursor: pointer;
      color: var(--text-primary);
      font-size: var(--font-size-base);
      border-radius: 6px;
      display: flex;
      align-items: center;
      justify-content: center;
      transition: all 0.2s;

      &:hover {
        background: var(--bg-hover);
      }

      &.active {
        // color: var(--color-primary);
        background: var(--dropdown-hover-color);
      }

      &--close:hover {
        background: #e81123;
        color: white;
      }
    }

    .theme-switch-btn:hover {
      // color: var(--color-primary);
    }

    .download-badge-wrapper {
      line-height: 0;

      :deep(.el-badge__content) {
        top: 2px;
        right: 4px;
        font-size: 10px;
        height: 16px;
        line-height: 16px;
        padding: 0 4px;
        min-width: 16px;
        border: none;
        background: var(--el-color-danger);
      }
    }
  }
}

.main-body {
  display: flex;
  flex: 1;
  overflow: hidden;
  // background: var(--app-background); //智能对话子菜单区域背景
  background: transparent;
}

.sidebar {
  width: 240px;
  min-width: 240px;
  background: var(--bg-sidebar);
  border-right: 1px solid var(--chat-divider-color) !important; //菜单和历史会话之间的分割线
  display: flex;
  flex-direction: column;
  transition: width 0.3s cubic-bezier(0.4, 0, 0.2, 1), min-width 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  overflow: hidden;
  position: relative;
  z-index: 2;

  &.collapsed {
    width: 69px;
    min-width: 68px;
  }
}

.content {
  flex: 1;
  overflow: hidden;
  min-height: 0;
  padding: 0;
  // background:var(--bg-submenu);
  display: flex;
  flex-direction: column;

  .topbar-actions {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    min-height: 0;

    &:empty {
      display: none;
    }
  }

  .content-box {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    height: 100%;
    position: relative;
  }
}

.route-loading {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-secondary, #64748b);
  font-size: 13px;
  background: var(--bg-primary, transparent);
}
</style>

<style lang="scss">
.shutdown-overlay {
  position: fixed;
  inset: 0;
  z-index: 99999;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(4px);
  -webkit-backdrop-filter: blur(4px);
}

.shutdown-box {
  text-align: center;
  color: #fff;
}

.shutdown-spinner {
  width: 40px;
  height: 40px;
  margin: 0 auto 16px;
  border: 3px solid rgba(255, 255, 255, 0.25);
  border-top-color: #fff;
  border-radius: 50%;
  animation: shutdown-spin 0.7s linear infinite;
}

@keyframes shutdown-spin {
  to {
    transform: rotate(360deg);
  }
}

.shutdown-text {
  font-size: 15px;
  letter-spacing: 0.5px;
}

.shutdown-fade-enter-active,
.shutdown-fade-leave-active {
  transition: opacity 0.2s ease;
}

.shutdown-fade-enter-from,
.shutdown-fade-leave-to {
  opacity: 0;
}

/* 标题栏更多菜单：确保不与被拖拽区域重叠 */
.titlebar-menu-popper {
  z-index: 10000 !important;
  margin-top: 4px !important;
  background: var(--theme-dailog-color) !important;
  border-color: var(--border-color) !important;

  .el-dropdown-menu {
    background-color: var(--theme-dailog-color) !important;
    border-color: var(--border-color) !important;
  }

  /* 顶部小三角与菜单背景同色 */
  .el-popper__arrow::before {
    background: var(--theme-dailog-color) !important;
    border: 1px solid var(--border-color) !important;
  }

  .el-dropdown-menu__item {
    color: var(--text-primary) !important;

    &:hover,
    &:focus {
      background-color: var(--dropdown-hover-color) !important;
      color: var(--text-primary) !important;
    }

    &.is-disabled {
      color: var(--text-secondary) !important;
      opacity: 0.6;
    }
  }
}

/* 标题栏用户菜单 */
.titlebar-user-popper {
  z-index: 10000 !important;
  margin-top: 4px !important;
  background: var(--theme-dailog-color) !important;
  border-color: var(--border-color) !important;

  .el-dropdown-menu {
    background-color: var(--theme-dailog-color) !important;
    border-color: var(--border-color) !important;
  }

  .el-popper__arrow::before {
    background: var(--theme-dailog-color) !important;
    border: 1px solid var(--border-color) !important;
  }

  .el-dropdown-menu__item {
    color: var(--text-primary) !important;

    &:hover,
    &:focus {
      background-color: var(--dropdown-hover-color) !important;
      color: var(--text-primary) !important;
    }

    &.is-disabled {
      color: var(--text-secondary) !important;
      opacity: 0.6;
    }
  }
}
</style>
