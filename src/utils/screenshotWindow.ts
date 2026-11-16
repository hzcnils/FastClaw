// FastClaw · 独立截图遮罩窗口的创建/预热工具
// 该窗口在应用启动时预创建并常驻隐藏，截图时复用（显示/隐藏），
// 避免每次现建 WebView2 的冷启动延迟，实现"秒开"。

const LABEL = 'screenshot'

/** Register before probing so a cold window and an already-ready window both acknowledge. */
async function waitForScreenshotReady(): Promise<boolean> {
  const { listen, emit } = await import('@tauri-apps/api/event')
  const token = crypto.randomUUID()
  let unlisten: (() => void) | undefined
  let timer: ReturnType<typeof setTimeout> | undefined
  let done!: (value: boolean) => void
  const result = new Promise<boolean>(resolve => { done = resolve })
  try {
    unlisten = await listen<string | null>('screenshot-ready', event => {
      if (event.payload === token || event.payload === null) done(true)
    })
    timer = setTimeout(() => done(false), 2000)
    await emit('screenshot-probe', token)
    return await result
  } catch { return false }
  finally { if (timer) clearTimeout(timer); unlisten?.() }
}

const OPTIONS = {
  url: 'screenshot.html',
  decorations: false,
  transparent: true,
  resizable: false,
  skipTaskbar: true,
  // 预热期不置顶 + 移出屏幕：即便 visible:false 在某些平台（透明窗 + Windows）或
  // tauri-plugin-window-state 启动时恢复可见性时失效，也不会成为挡住桌面点击的
  // "幽灵窗口"。截图时 runCapture 会重新定位、置顶并 show()。
  alwaysOnTop: false,
  shadow: false,
  visible: false,
  focus: false,
  width: 800,
  height: 600,
  x: -10000,
  y: -10000,
  title: '截图',
} as const

function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

/** 预热：若遮罩窗口尚不存在则隐藏创建（供 App 启动时调用一次）。 */
export async function prewarmScreenshotWindow(): Promise<void> {
  if (!isTauri()) return
  try {
    const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow')
    const existing = await WebviewWindow.getByLabel(LABEL)
    if (!existing) {
      const w = new WebviewWindow(LABEL, { ...OPTIONS })
      // 关键修复：visible:false 在部分平台（透明窗 + Windows）或 tauri-plugin-window-state
      // 启动时恢复可见性时可能不生效，导致一个透明置顶窗口停在桌面左上角挡点击。
      // 窗口创建完成后立即强制隐藏；截图时 runCapture 会重新 show() 并铺满屏幕。
      w.once('tauri://created', () => { void w.hide().catch(() => {}) })
    }
  } catch {
    /* 预热失败不影响功能，截图时会走冷路径新建 */
  }
}

/**
 * 确保遮罩窗口存在。
 * @returns existed 表示是否复用已存在窗口；ok=true 表示就绪握手完成。
 *          新旧窗口均需显式发送携带请求 ID 的 screenshot-run，不在预热时取图。
 *          ok=false 表示创建失败，调用方应回退到主窗口内遮罩。
 */
export async function ensureScreenshotWindow(): Promise<{ existed: boolean; ok: boolean }> {
  if (!isTauri()) return { existed: false, ok: false }
  try {
    const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow')
    const existing = await WebviewWindow.getByLabel(LABEL)
    if (existing) return { existed: true, ok: await waitForScreenshotReady() }

    const w = new WebviewWindow(LABEL, { ...OPTIONS, focus: true })
    const ok = await new Promise<boolean>((resolve) => {
      let settled = false
      const done = (v: boolean) => { if (!settled) { settled = true; resolve(v) } }
      w.once('tauri://created', () => done(true))
      w.once('tauri://error', () => done(false))
      setTimeout(() => done(false), 2000)
    })
    return { existed: false, ok: ok && await waitForScreenshotReady() }
  } catch {
    return { existed: false, ok: false }
  }
}
