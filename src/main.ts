// FastClaw · Vue 3 入口
import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { provideFastPlanResume } from './api/fastPlanResume'
import ElementPlus, { ElMessage } from 'element-plus'
import zhCn from 'element-plus/es/locale/lang/zh-cn'
import en from 'element-plus/es/locale/lang/en'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import App from './App.vue'
import router from './router'
import i18n from './i18n'
import DownloadCenterVuePlugin, { createDownloadRuntime, resolveDownloadCenterUiConfig } from 'tauri-download-center-vue'
import { handleFastClawDownloadCompleted, handleFastClawDownloadFailed, handleFastClawDownloadUpdated, setHostDownloadRuntime } from './integrations/download-center'
import {
  ensureReloadHint,
  errorToMessage,
  isAppRootBlank,
  isBenignAsyncError,
  isBenignRuntimeError,
  showFatalRecovery,
} from '@/utils/fatalRecovery'

import './styles/main.css';     // Tailwind 配置须在最前，确保 @custom-variant 等设置先生效
import "./styles/knowledge.css";
import './styles/global.scss'
import * as ElementPlusIconsVue from '@element-plus/icons-vue'

void provideFastPlanResume()
const app = createApp(App)

// 页面依赖 Element Plus 图标全局注册
for (const [key, component] of Object.entries(ElementPlusIconsVue)) {
  app.component(key, component)
}

const elLocale = (i18n.global.locale.value === 'en') ? en : zhCn

const pinia = createPinia()
app.use(pinia)
app.use(router)
app.use(i18n)
app.use(ElementPlus, { locale: elLocale })
const downloadConfig = resolveDownloadCenterUiConfig()
const downloadRuntime = createDownloadRuntime({
  config: downloadConfig,
  onTaskUpdated: handleFastClawDownloadUpdated,
  onTaskCompleted: handleFastClawDownloadCompleted,
  onTaskFailed: handleFastClawDownloadFailed,
})
setHostDownloadRuntime(downloadRuntime)
app.use(DownloadCenterVuePlugin, {
  i18n,
  runtimeInstance: downloadRuntime,
})

let lastSoftToastMsg = ''
let lastSoftToastAt = 0

function softToast(message: string) {
  const now = Date.now()
  if (message === lastSoftToastMsg && now - lastSoftToastAt < 4000) return
  lastSoftToastMsg = message
  lastSoftToastAt = now
  try {
    ElMessage.error(message.slice(0, 180))
  } catch { /* Vue/Element 已挂则忽略，靠 DOM 恢复 UI */ }
}

/** Vue 渲染错误 / 全局未捕错误 → toast；根已空则全屏恢复，渲染类错误再给轻量重载入口 */
function handleCaughtError(
  err: unknown,
  source: string,
  info?: string,
  opts?: { offerReloadHint?: boolean },
) {
  if (isBenignRuntimeError(err)) return
  const msg = errorToMessage(err)
  // eslint-disable-next-line no-console
  console.error(`[${source}]`, info || '', err)
  softToast(source.startsWith('Vue') ? `界面渲染异常：${msg}` : `运行异常：${msg}`)
  if (isAppRootBlank()) {
    showFatalRecovery(msg)
    return
  }
  if (opts?.offerReloadHint !== false) {
    ensureReloadHint(msg)
  }
}

// 全局错误兜底：避免单个组件渲染异常导致整页白屏、且 Production 下无法查看控制台
app.config.errorHandler = (err, _instance, info) => {
  handleCaughtError(err, 'Vue render error', info, { offerReloadHint: true })
}

// 同步脚本错误 + 未处理 Promise（errorHandler 捕不到异步）
if (typeof window !== 'undefined') {
  window.addEventListener('error', (event) => {
    // 资源加载失败（img/script src）一般不白屏，避免刷恢复条
    if (event.target && event.target !== window) return
    const payload = event.error || event.message
    if (isBenignRuntimeError(payload)) {
      // 阻止浏览器默认把 ResizeObserver 噪音刷到控制台/被当成未处理错误
      event.preventDefault()
      return
    }
    handleCaughtError(payload, 'window.error', undefined, { offerReloadHint: true })
  })
  window.addEventListener('unhandledrejection', (event) => {
    if (isBenignAsyncError(event.reason) || isBenignRuntimeError(event.reason)) return
    // 业务 Promise 失败很常见：只 toast；仅在 #app 已空时升为全屏恢复，避免干扰正常功能
    handleCaughtError(event.reason, 'unhandledrejection', undefined, { offerReloadHint: false })
  })
}

// 全局注入 $TOOL（jmis 组件依赖）
app.provide('$TOOL', {
  data: {
    get: () => null,
  },
  dateFormat: (ts: string | number) => {
    const d = new Date(typeof ts === 'string' ? (ts.length <= 10 ? Number(ts) * 1000 : Number(ts)) : ts)
    return d.toLocaleString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' })
  },
})

// 全局注入 $API（递归 proxy，无限层级属性访问，最终调用返回空数据）
function createAPIMock(): any {
  const fn = () => Promise.resolve({ code: 200, success: true, data: { items: [], total: 0 } })
  return new Proxy(fn, {
    get(_target, prop: string) {
      if (prop === 'then') return undefined
      return createAPIMock()
    },
  })
}
app.provide('$API', createAPIMock())

// 刷新时同步恢复字体大小
try {
  const saved = localStorage.getItem('fastclaw-font-size')
  if (saved) {
    const px = Math.min(Math.max(parseInt(saved) || 14, 10), 27)
    document.documentElement.style.setProperty('--font-size-base', `${px}px`)
  }
} catch { /* ignore */ }

try {
  app.mount('#app')
} catch (err) {
  handleCaughtError(err, 'app.mount')
  showFatalRecovery(errorToMessage(err))
}

// 生产环境安全限制（保留自救刷新，继续禁 DevTools）
if (import.meta.env.PROD) {
  // 禁用 WebView 默认右键菜单（复制/检查等）；应用内自定义菜单仍可在自身 @contextmenu 里弹出
  document.addEventListener('contextmenu', (e) => {
    e.preventDefault()
  })

  document.addEventListener('keydown', (e) => {
    // ── 仍禁用 DevTools / 查看源码等 ──
    if (e.key === 'F12') {
      e.preventDefault()
      return
    }
    // F5 仍禁（与历史行为一致）；自救请用 Ctrl+R 或恢复 UI 按钮
    if (e.key === 'F5') {
      e.preventDefault()
      return
    }
    if (e.ctrlKey && e.shiftKey) {
      if (['i', 'I', 'j', 'J', 'c', 'C'].includes(e.key)) {
        e.preventDefault() // DevTools / 元素选择
        return
      }
      if (['r', 'R'].includes(e.key)) {
        e.preventDefault() // 强制刷新仍禁
        return
      }
    }
    if (e.ctrlKey) {
      // ★ 放行 Ctrl+R：白屏时可键盘刷新自救
      if (['r', 'R'].includes(e.key)) return
      if (['u', 'U', 's', 'S', 'p', 'P'].includes(e.key)) {
        e.preventDefault() // 查看源码 / 保存 / 打印
      }
    }
  })
}
