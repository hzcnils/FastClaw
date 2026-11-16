/**
 * 白屏 / 致命错误恢复：纯 DOM，不依赖 Vue / Element Plus 是否还活着。
 * 仅在真正需要时插入 UI，不影响正常业务流程。
 */

const FATAL_ID = 'fastclaw-fatal-recovery'
const HINT_ID = 'fastclaw-reload-hint'

export function reloadApp() {
  try {
    window.location.reload()
  } catch {
    try {
      window.location.href = window.location.href
    } catch { /* ignore */ }
  }
}

export function isAppRootBlank(): boolean {
  const root = document.getElementById('app')
  if (!root) return true
  if (root.childElementCount === 0) return true
  // 仅有注释/空白文本视为空
  const text = (root.textContent || '').replace(/\s+/g, '')
  return text.length === 0 && root.querySelector('*') == null
}

/** 全屏恢复页：#app 已空或根渲染崩溃时用 */
export function showFatalRecovery(message?: string) {
  if (typeof document === 'undefined') return
  if (document.getElementById(FATAL_ID)) return

  hideReloadHint()

  const wrap = document.createElement('div')
  wrap.id = FATAL_ID
  wrap.setAttribute('role', 'alert')
  wrap.style.cssText = [
    'position:fixed', 'inset:0', 'z-index:2147483646',
    'display:flex', 'align-items:center', 'justify-content:center',
    'padding:24px', 'box-sizing:border-box',
    'background:#0f1419', 'color:#e8eef4',
    'font-family:system-ui,-apple-system,sans-serif',
  ].join(';')

  const card = document.createElement('div')
  card.style.cssText = 'max-width:420px;text-align:center;'

  const title = document.createElement('div')
  title.textContent = '界面出错了'
  title.style.cssText = 'font-size:20px;font-weight:600;margin-bottom:10px;'

  const desc = document.createElement('div')
  desc.textContent = message?.trim()
    ? String(message).slice(0, 200)
    : '页面无法继续显示。可点击下方按钮重新加载。'
  desc.style.cssText = 'font-size:13px;line-height:1.5;opacity:.78;margin-bottom:20px;word-break:break-word;'

  const btn = document.createElement('button')
  btn.type = 'button'
  btn.textContent = '重新加载'
  btn.style.cssText = [
    'cursor:pointer', 'border:none', 'border-radius:8px',
    'padding:10px 22px', 'font-size:14px', 'font-weight:600',
    'background:#3b82f6', 'color:#fff',
  ].join(';')
  btn.addEventListener('click', () => reloadApp())

  card.append(title, desc, btn)
  wrap.appendChild(card)
  document.body.appendChild(wrap)
}

/** 轻量提示条：组件渲染异常但壳子还在时，提供重载双保险（不挡操作） */
export function ensureReloadHint(message?: string) {
  if (typeof document === 'undefined') return
  if (document.getElementById(FATAL_ID) || document.getElementById(HINT_ID)) return

  const bar = document.createElement('div')
  bar.id = HINT_ID
  bar.setAttribute('role', 'status')
  bar.style.cssText = [
    'position:fixed', 'right:16px', 'bottom:16px', 'z-index:2147483645',
    'display:flex', 'align-items:center', 'gap:10px',
    'max-width:min(420px,calc(100vw - 32px))',
    'padding:10px 12px', 'border-radius:10px',
    'background:rgba(15,20,25,.92)', 'color:#e8eef4',
    'box-shadow:0 8px 28px rgba(0,0,0,.35)',
    'font-family:system-ui,-apple-system,sans-serif', 'font-size:12px',
  ].join(';')

  const text = document.createElement('span')
  text.textContent = message?.trim()
    ? `界面异常：${String(message).slice(0, 80)}`
    : '界面出现异常'
  text.style.cssText = 'flex:1;line-height:1.4;opacity:.9;word-break:break-word;'

  const reloadBtn = document.createElement('button')
  reloadBtn.type = 'button'
  reloadBtn.textContent = '重新加载'
  reloadBtn.style.cssText = [
    'flex-shrink:0', 'cursor:pointer', 'border:none', 'border-radius:6px',
    'padding:6px 10px', 'font-size:12px', 'font-weight:600',
    'background:#3b82f6', 'color:#fff',
  ].join(';')
  reloadBtn.addEventListener('click', () => reloadApp())

  const closeBtn = document.createElement('button')
  closeBtn.type = 'button'
  closeBtn.textContent = '×'
  closeBtn.title = '关闭'
  closeBtn.style.cssText = [
    'flex-shrink:0', 'cursor:pointer', 'border:none', 'background:transparent',
    'color:#e8eef4', 'opacity:.7', 'font-size:16px', 'line-height:1', 'padding:0 2px',
  ].join(';')
  closeBtn.addEventListener('click', () => hideReloadHint())

  bar.append(text, reloadBtn, closeBtn)
  document.body.appendChild(bar)
}

export function hideReloadHint() {
  document.getElementById(HINT_ID)?.remove()
}

export function isBenignAsyncError(reason: unknown): boolean {
  if (reason == null) return true
  const name = reason instanceof Error ? reason.name : ''
  const msg = reason instanceof Error ? reason.message : String(reason)
  if (name === 'AbortError') return true
  return /abort|cancel|canceled|cancelled|已停止|已取消|navigation cancelled/i.test(msg)
}

/**
 * 浏览器 / 布局库常见的无害运行时噪音。
 * 典型：切换 el-select / v-if 导致布局抖动时，Chrome 会报
 * “ResizeObserver loop completed with undelivered notifications”，
 * 并非业务逻辑错误，不应弹「界面异常 / 重新加载」。
 *
 * Vue 在 Teleport 弹窗打开时做 HMR 热更新，也常出现
 * “Cannot set properties of null (setting '__vnode')”，属开发态噪音。
 */
export function isBenignRuntimeError(err: unknown): boolean {
  const msg = errorToMessage(err)
  if (/ResizeObserver loop (limit exceeded|completed with undelivered notifications)/i.test(msg)) {
    return true
  }
  // Vue HMR / Teleport：节点已卸载仍 patch
  if (/Cannot set properties of null \(setting ['"]__vnode['"]\)/i.test(msg)) {
    return true
  }
  if (/Cannot read propert(?:y|ies) of null \(reading ['"]__vnode['"]\)/i.test(msg)) {
    return true
  }
  return false
}

export function errorToMessage(err: unknown): string {
  if (err instanceof Error) return err.message || err.name
  if (typeof err === 'string') return err
  try {
    return JSON.stringify(err)
  } catch {
    return String(err)
  }
}
