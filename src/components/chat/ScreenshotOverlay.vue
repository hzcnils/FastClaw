<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import type { MonitorInfo, WindowInfo } from '@/types/screenshot'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { PhysicalPosition, PhysicalSize } from '@tauri-apps/api/dpi'
import { save as saveDialog } from '@tauri-apps/plugin-dialog'
import { writeFile } from '@tauri-apps/plugin-fs'
import ScreenshotMagnifier from './ScreenshotMagnifier.vue'

const props = defineProps<{
  imageWidth: number       // 虚拟桌面总宽度（物理像素）
  imageHeight: number      // 虚拟桌面总高度（物理像素）
  imageOffsetX: number     // 虚拟桌面左上角 X（物理像素）
  imageOffsetY: number     // 虚拟桌面左上角 Y（物理像素）
  monitors: MonitorInfo[]  // 各显示器信息（物理坐标）
  monitorImages: string[]  // 各显示器 JPEG base64 data URL（与 monitors 顺序对应）
  windows: WindowInfo[]
  /**
   * 是否由本组件管理承载窗口的几何（位置/尺寸/置顶/还原）。
   * - true / 省略（默认，旧路径）：在主窗口内展开，需临时把主窗口铺满虚拟桌面再还原。
   * - false（独立遮罩窗口路径）：几何由承载页 ScreenshotWindow 负责，本组件不触碰窗口，避免主窗口抖动。
   */
  manageWindow?: boolean
}>()

/** 当前鼠标所在显示器（用于 SVG 暗色遮罩 + 限制选区 + 裁剪来源） */
interface ActiveMonArea {
  cssX: number; cssY: number; cssW: number; cssH: number
  index: number; info: MonitorInfo
}
const activeMon = ref<ActiveMonArea | null>(null)

const emit = defineEmits<{
  'confirm': [dataUrl: string]
  'cancel': []
}>()

const overlayRef = ref<HTMLDivElement>()
const selSizeRef = ref<HTMLDivElement>()

// ---- Selection state (in image-local pixel coordinates) ----
const selecting = ref(false)
const selection = ref({ x: 0, y: 0, w: 0, h: 0 })
const startPoint = ref({ x: 0, y: 0 })
const cachedScale = ref({ scale: 0, offsetX: 0, offsetY: 0 })

// ---- Resize / Move state ----
type ResizeHandle = 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w' | 'move'
const activeHandle = ref<ResizeHandle | null>(null)
const originSel = ref<{ x: number; y: number; w: number; h: number } | null>(null)
const resizeDragStart = ref<{ imgX: number; imgY: number } | null>(null)

// ---- Hover: auto window detection ----
const hoveredWindow = ref<WindowInfo | null>(null)
// snapshot of hovered window at mousedown, used for bare-click snap (hoveredWindow is cleared while selecting=true)
let hoveredWindowAtClick: WindowInfo | null = null

// ============================================================================
// Coordinate system utilities
// ============================================================================

/// 缓存 overlay 的屏幕位置，避免 mousemove 中反复调用 getBoundingClientRect()
let cachedOverlayLeft = 0
let cachedOverlayTop = 0

function updateOverlayRect() {
  if (!overlayRef.value) return
  const r = overlayRef.value.getBoundingClientRect()
  cachedOverlayLeft = r.left
  cachedOverlayTop = r.top
}

function computeScale() {
  const ovW = window.innerWidth
  const ovH = window.innerHeight
  if (ovW <= 0 || ovH <= 0 || props.imageWidth <= 0 || props.imageHeight <= 0) return
  const scale = Math.max(ovW / props.imageWidth, ovH / props.imageHeight)
  cachedScale.value = {
    scale,
    offsetX: (ovW - props.imageWidth * scale) / 2,
    offsetY: (ovH - props.imageHeight * scale) / 2,
  }
}

/// mousemove 热路径：零 DOM 查询，仅算术运算
/// 点击坐标 clamped 到最近有效图像像素，确保任意位置都能框选
function cssToImage(clientX: number, clientY: number): { x: number; y: number } | null {
  const d = cachedScale.value
  if (d.scale <= 0) return null

  const cssX = clientX - cachedOverlayLeft
  const cssY = clientY - cachedOverlayTop

  return {
    x: Math.round(Math.max(0, Math.min(props.imageWidth - 1, (cssX - d.offsetX) / d.scale))),
    y: Math.round(Math.max(0, Math.min(props.imageHeight - 1, (cssY - d.offsetY) / d.scale))),
  }
}

function imageToDisplayX(imgX: number): number {
  const d = cachedScale.value
  return d.offsetX + imgX * d.scale
}

function imageToDisplayY(imgY: number): number {
  const d = cachedScale.value
  return d.offsetY + imgY * d.scale
}

/** Convert a window's virtual desktop coordinates to image-local pixel rect for display. */
function windowToImageRect(w: WindowInfo): { x: number; y: number; w: number; h: number } {
  return {
    x: w.x - props.imageOffsetX,
    y: w.y - props.imageOffsetY,
    w: w.width,
    h: w.height,
  }
}

/** Find the topmost window that contains the given image pixel coordinate.
 *  Windows are in Z-order (topmost first from EnumWindows). */
function hitTestWindows(imgX: number, imgY: number): WindowInfo | null {
  for (const w of props.windows) {
    const ix = w.x - props.imageOffsetX
    const iy = w.y - props.imageOffsetY
    if (imgX >= ix && imgX < ix + w.width &&
        imgY >= iy && imgY < iy + w.height) {
      return w
    }
  }
  return null
}

/// 通过 CSS 变量批量更新，每帧仅 4 次 setProperty（vs 旧方案 13+ 次独立 style 写入）
function updateSelectionDOM(sel: { x: number; y: number; w: number; h: number }) {
  const d = cachedScale.value
  if (d.scale <= 0 || !overlayRef.value) return

  overlayRef.value.style.setProperty('--msk-x', (d.offsetX + sel.x * d.scale) + 'px')
  overlayRef.value.style.setProperty('--msk-y', (d.offsetY + sel.y * d.scale) + 'px')
  overlayRef.value.style.setProperty('--msk-w', (sel.w * d.scale) + 'px')
  overlayRef.value.style.setProperty('--msk-h', (sel.h * d.scale) + 'px')

  if (selSizeRef.value) {
    selSizeRef.value.textContent = `${Math.round(sel.w)} x ${Math.round(sel.h)}`
  }
}

// ============================================================================
// Event handlers
// ============================================================================

/// 存放最新鼠标位置和选区坐标，RAF 回调读取
let latestMouseX = 0
let latestMouseY = 0
const latestSel = { x: 0, y: 0, w: 0, h: 0 }
let rafPending = false

// 放大镜：响应式的鼠标 CSS 坐标（相对于 overlay）
const magnifierX = ref(0)
const magnifierY = ref(0)
const showMagnifier = ref(false)

// 放大镜参数：使用当前鼠标所在显示器的图像位置
const magnifierProps = computed(() => {
  const d = cachedScale.value
  const am = activeMon.value
  if (d.scale <= 0 || !am) return { scaleX: 0, scaleY: 0, offsetX: 0, offsetY: 0, imageWidth: 0, imageHeight: 0, imageDataUrl: '' }
  return {
    scaleX: 1 / d.scale,
    scaleY: 1 / d.scale,
    offsetX: am.cssX,   // 已含 +2 offset（monitorToSvgCoords 计算）
    offsetY: am.cssY,
    imageWidth: am.info.width,
    imageHeight: am.info.height,
    imageDataUrl: props.monitorImages[am.index] ?? '',
  }
})

/** 将显示器在虚拟桌面中的位置转为 SVG/CSS overlay 坐标（overlay 有 -2px offset，图像从 offsetX+2 开始）*/
function monitorToCssCoords(m: MonitorInfo): { x: number; y: number; w: number; h: number } {
  const d = cachedScale.value
  const mx = m.x - props.imageOffsetX
  const my = m.y - props.imageOffsetY
  return {
    x: d.offsetX + mx * d.scale,
    y: d.offsetY + my * d.scale,
    w: m.width * d.scale,
    h: m.height * d.scale,
  }
}

/** 更新当前鼠标所在的显示器（activeMon） */
function updateActiveMon(clientX: number, clientY: number) {
  const d = cachedScale.value
  if (d.scale <= 0) { activeMon.value = null; return }

  const cssX = clientX - cachedOverlayLeft
  const cssY = clientY - cachedOverlayTop
  const imgX = (cssX - d.offsetX) / d.scale
  const imgY = (cssY - d.offsetY) / d.scale

  for (let i = 0; i < props.monitors.length; i++) {
    const m = props.monitors[i]
    const mx = m.x - props.imageOffsetX
    const my = m.y - props.imageOffsetY
    if (imgX >= mx && imgX < mx + m.width && imgY >= my && imgY < my + m.height) {
      const c = monitorToCssCoords(m)
      activeMon.value = {
        cssX: c.x + 2,  // +2 对应 overlay 的 -2px offset（与图像定位一致）
        cssY: c.y + 2,
        cssW: c.w,
        cssH: c.h,
        index: i,
        info: m,
      }
      return
    }
  }
  activeMon.value = null
}

/** 当前选区在 SVG/overlay 坐标中的位置（用于 SVG mask 挖孔） */
const selBox = computed(() => {
  const d = cachedScale.value
  const sel = selection.value
  if (d.scale <= 0 || sel.w <= 5) return null
  return {
    x: d.offsetX + sel.x * d.scale,
    y: d.offsetY + sel.y * d.scale,
    w: sel.w * d.scale,
    h: sel.h * d.scale,
  }
})

/** 分辨率标签默认在选区上方外侧；选区贴近屏幕顶部（<28px）时翻到选区内部左上角，避免被裁剪 */
const sizeLabelInside = computed(() => {
  const sb = selBox.value
  return !!sb && sb.y < 28
})

/** 选区完成后显示的工具栏位置：默认在选区右下角外侧，底部空间不足时移至选区右上角外侧 */
const floatBtnStyle = computed(() => {
  const sb = selBox.value
  if (!sb) return { display: 'none' as const }
  const TOOLBAR_H = 44
  const GAP = 8

  // 用 CSS right 属性对齐选区右边缘，无需知道工具栏实际宽度
  // overlay 左边缘在 viewport 外 2px，故 overlay 总宽 = innerWidth + 4
  const rightOffset = window.innerWidth + 4 - (sb.x + sb.w)

  let top = sb.y + sb.h + GAP
  if (top + TOOLBAR_H > window.innerHeight) {
    top = sb.y - TOOLBAR_H - GAP
  }

  return { right: rightOffset + 'px', top: top + 'px' }
})

function scheduleUpdate() {
  if (rafPending) return
  rafPending = true
  requestAnimationFrame(() => {
    rafPending = false
    processFrameUpdate()
  })
}

function onMouseDown(e: MouseEvent) {
  if (!overlayRef.value) return
  e.preventDefault()

  const imgCoords = cssToImage(e.clientX, e.clientY)
  if (!imgCoords) return

  // Click inside existing selection → enter move mode
  const sel = selection.value
  if (sel.w > 5 && !selecting.value &&
      imgCoords.x >= sel.x && imgCoords.x <= sel.x + sel.w &&
      imgCoords.y >= sel.y && imgCoords.y <= sel.y + sel.h) {
    startResize(e, 'move')
    return
  }

  // Snapshot hovered window before selecting=true clears it in onMouseMove
  hoveredWindowAtClick = hoveredWindow.value

  // Always start a free drag; a bare click (no drag) snaps to the hovered window in onMouseUp
  const snapped = snapToEdge(imgCoords.x, imgCoords.y)
  startPoint.value = { x: snapped.x, y: snapped.y }
  selection.value = { x: snapped.x, y: snapped.y, w: 0, h: 0 }
  selecting.value = true
}

function onMouseMove(e: MouseEvent) {
  // 阻止浏览器默认拖拽行为（否则鼠标拖动时整个页面会跟着鼠标动）
  if (selecting.value || activeHandle.value) e.preventDefault()

  // 放大镜：始终更新 CSS 坐标（相对于 overlay）
  magnifierX.value = e.clientX - cachedOverlayLeft
  magnifierY.value = e.clientY - cachedOverlayTop

  // 始终更新显示器（SVG 遮罩需要）
  updateActiveMon(e.clientX, e.clientY)
  // 窗口命中检测：仅在无选区的初始状态（未拖拽、选区为空）时运行
  if (!selecting.value && !activeHandle.value && selection.value.w < 5) {
    const imgCoordsForHit = cssToImage(e.clientX, e.clientY)
    hoveredWindow.value = imgCoordsForHit ? hitTestWindows(imgCoordsForHit.x, imgCoordsForHit.y) : null
  } else {
    hoveredWindow.value = null
  }

  // 无选区时显示放大镜（含悬浮窗口状态）；一旦存在选区或正在拖拽则隐藏
  showMagnifier.value = !selecting.value && selection.value.w < 5

  if (selecting.value || activeHandle.value) {
    // 只存原始坐标，计算交给 RAF（鼠标 1000Hz → 渲染 60Hz）
    latestMouseX = e.clientX
    latestMouseY = e.clientY
    scheduleUpdate()
  }
}

/// 在 RAF 中完成坐标转换和 DOM 更新（限速至屏幕刷新率）
function processFrameUpdate() {
  // Resize / Move 模式
  if (activeHandle.value && originSel.value && resizeDragStart.value) {
    const imgCoords = cssToImage(latestMouseX, latestMouseY)
    if (!imgCoords) return
    const dx = imgCoords.x - resizeDragStart.value.imgX
    const dy = imgCoords.y - resizeDragStart.value.imgY
    const newSel = applyResize(activeHandle.value, originSel.value, dx, dy)
    clampSel(newSel)
    selection.value = newSel
    updateSelectionDOM(newSel)
    return
  }

  if (!selecting.value) return

  const imgCoords = cssToImage(latestMouseX, latestMouseY)
  if (!imgCoords) return

  const snapped = snapToEdge(imgCoords.x, imgCoords.y)
  latestSel.x = Math.min(startPoint.value.x, snapped.x)
  latestSel.y = Math.min(startPoint.value.y, snapped.y)
  latestSel.w = Math.abs(snapped.x - startPoint.value.x)
  latestSel.h = Math.abs(snapped.y - startPoint.value.y)

  if (selection.value.w < 5 && latestSel.w > 0) {
    selection.value = { x: latestSel.x, y: latestSel.y, w: latestSel.w, h: latestSel.h }
  }

  updateSelectionDOM(latestSel)
}

function onMouseUp(e: MouseEvent) {
  // 结束 resize / move 操作
  if (activeHandle.value && originSel.value && resizeDragStart.value) {
    const imgCoords = cssToImage(e.clientX, e.clientY)
    if (imgCoords) {
      const dx = imgCoords.x - resizeDragStart.value.imgX
      const dy = imgCoords.y - resizeDragStart.value.imgY
      const newSel = applyResize(activeHandle.value, originSel.value, dx, dy)
      clampSel(newSel)
      selection.value = newSel
      updateSelectionDOM(newSel)
    }
    activeHandle.value = null
    originSel.value = null
    resizeDragStart.value = null
    return
  }

  if (selecting.value) {
    const imgCoords = cssToImage(e.clientX, e.clientY)
    if (imgCoords) {
      const snapped = snapToEdge(imgCoords.x, imgCoords.y)
      latestSel.x = Math.min(startPoint.value.x, snapped.x)
      latestSel.y = Math.min(startPoint.value.y, snapped.y)
      latestSel.w = Math.abs(snapped.x - startPoint.value.x)
      latestSel.h = Math.abs(snapped.y - startPoint.value.y)
      // Bare click (no drag) → snap to the window that was under cursor at mousedown
      if (latestSel.w < 5 && latestSel.h < 5 && hoveredWindowAtClick) {
        const r = windowToImageRect(hoveredWindowAtClick)
        selection.value = { x: r.x, y: r.y, w: r.w, h: r.h }
        updateSelectionDOM(selection.value)
      } else {
        selection.value = { x: latestSel.x, y: latestSel.y, w: latestSel.w, h: latestSel.h }
      }
      hoveredWindowAtClick = null
    }
  }
  selecting.value = false
  if (rafPending) {
    rafPending = false
    processFrameUpdate()
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    // 直接取消，不做防抖（修复 #188 ESC需双击退出）
    emit('cancel')
    return
  }
  if (e.key === 'Enter' && selection.value.w > 10 && selection.value.h > 10) {
    cropAndConfirm()
    return
  }

  // 方向键微调控区（解决屏幕边缘 1px 死区无法被鼠标选中）
  if (!selection.value.w || !selection.value.h) return
  const step = e.shiftKey ? 10 : 1
  const sel = { ...selection.value }

  if (e.key === 'ArrowLeft')  { sel.x = Math.max(0, sel.x - step); sel.w += step }
  if (e.key === 'ArrowRight') { sel.w = Math.min(props.imageWidth - sel.x, sel.w + step) }
  if (e.key === 'ArrowUp')    { sel.y = Math.max(0, sel.y - step); sel.h += step }
  if (e.key === 'ArrowDown')  { sel.h = Math.min(props.imageHeight - sel.y, sel.h + step) }

  if (sel.x !== selection.value.x || sel.y !== selection.value.y ||
      sel.w !== selection.value.w || sel.h !== selection.value.h) {
    e.preventDefault()
    selection.value = sel
    updateSelectionDOM(sel)
  }
}


// ============================================================================
// Crop & Confirm
// ============================================================================

/** 根据选区中心点找到对应的显示器，供 active_window / specified_window 模式下裁剪使用 */
function getMonitorForSelectionCenter(): { idx: number; mon: MonitorInfo } {
  const sel = selection.value
  const cx = sel.x + sel.w / 2
  const cy = sel.y + sel.h / 2
  for (let i = 0; i < props.monitors.length; i++) {
    const m = props.monitors[i]
    const mx = m.x - props.imageOffsetX
    const my = m.y - props.imageOffsetY
    if (cx >= mx && cx < mx + m.width && cy >= my && cy < my + m.height) {
      return { idx: i, mon: m }
    }
  }
  return { idx: 0, mon: props.monitors[0] }
}

function cropAndConfirm() {
  const sel = selection.value
  if (sel.w < 10 || sel.h < 10) return

  // 优先使用当前鼠标所在显示器；若鼠标已离开，则根据选区中心点判断
  const { idx, mon } = activeMon.value
    ? { idx: activeMon.value.index, mon: activeMon.value.info }
    : getMonitorForSelectionCenter()

  if (!mon || !props.monitorImages[idx]) return

  // 选区坐标是虚拟桌面局部坐标（原点 = 虚拟桌面左上角）
  // 显示器在虚拟桌面中的偏移：(m.x - imageOffsetX, m.y - imageOffsetY)
  const localX = sel.x - (mon.x - props.imageOffsetX)
  const localY = sel.y - (mon.y - props.imageOffsetY)

  const img = new Image()
  img.onload = () => {
    const canvas = document.createElement('canvas')
    canvas.width = sel.w
    canvas.height = sel.h
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    ctx.drawImage(img, localX, localY, sel.w, sel.h, 0, 0, sel.w, sel.h)
    emit('confirm', canvas.toDataURL('image/png'))
  }
  img.src = props.monitorImages[idx]
}

function handleConfirm() {
  cropAndConfirm()
}

async function handleSave() {
  const sel = selection.value
  if (sel.w < 10 || sel.h < 10) return
  const { idx, mon } = activeMon.value
    ? { idx: activeMon.value.index, mon: activeMon.value.info }
    : getMonitorForSelectionCenter()
  if (!mon || !props.monitorImages[idx]) return

  const localX = sel.x - (mon.x - props.imageOffsetX)
  const localY = sel.y - (mon.y - props.imageOffsetY)

  // 弹出保存路径选择对话框
  const savePath = await saveDialog({
    defaultPath: `screenshot_${Date.now()}.png`,
    filters: [{ name: 'PNG 图片', extensions: ['png'] }],
  })
  if (!savePath) return  // 用户取消

  const img = new Image()
  img.onload = async () => {
    const canvas = document.createElement('canvas')
    canvas.width = sel.w
    canvas.height = sel.h
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    ctx.drawImage(img, localX, localY, sel.w, sel.h, 0, 0, sel.w, sel.h)

    // canvas → Uint8Array（PNG 二进制），写入成功后退出截图模式
    canvas.toBlob(async (blob) => {
      if (!blob) return
      const buf = await blob.arrayBuffer()
      await writeFile(savePath, new Uint8Array(buf))
      emit('cancel')
    }, 'image/png')
  }
  img.src = props.monitorImages[idx]
}

function onDblClick(e: MouseEvent) {
  if (selecting.value || activeHandle.value) return
  const sel = selection.value
  if (sel.w < 10 || sel.h < 10) return
  const imgCoords = cssToImage(e.clientX, e.clientY)
  if (!imgCoords) return
  if (imgCoords.x >= sel.x && imgCoords.x <= sel.x + sel.w &&
      imgCoords.y >= sel.y && imgCoords.y <= sel.y + sel.h) {
    cropAndConfirm()
  }
}

// Window highlight style (specified_window mode)
const highlightStyle = computed(() => {
  if (!hoveredWindow.value) return { display: 'none' }
  const r = windowToImageRect(hoveredWindow.value)
  return {
    display: 'block',
    left: imageToDisplayX(r.x) + 'px',
    top: imageToDisplayY(r.y) + 'px',
    width: imageToDisplayX(r.w) - imageToDisplayX(0) + 'px',
    height: imageToDisplayY(r.h) - imageToDisplayY(0) + 'px',
  }
})

// ============================================================================
// Resize / Move helpers
// ============================================================================

function applyResize(
  handle: ResizeHandle,
  origin: { x: number; y: number; w: number; h: number },
  dx: number, dy: number
): { x: number; y: number; w: number; h: number } {
  let { x, y, w, h } = origin
  switch (handle) {
    case 'nw':   x += dx; y += dy; w -= dx; h -= dy; break
    case 'n':             y += dy;           h -= dy; break
    case 'ne':            y += dy; w += dx;  h -= dy; break
    case 'e':                      w += dx;           break
    case 'se':                     w += dx;  h += dy; break
    case 's':                                h += dy; break
    case 'sw':   x += dx;          w -= dx;  h += dy; break
    case 'w':    x += dx;          w -= dx;           break
    case 'move': x += dx; y += dy;                    break
  }
  return { x, y, w: Math.max(10, w), h: Math.max(10, h) }
}

function clampSel(sel: { x: number; y: number; w: number; h: number }) {
  const am = activeMon.value
  const minX = am ? am.info.x - props.imageOffsetX : 0
  const minY = am ? am.info.y - props.imageOffsetY : 0
  const maxX = am ? minX + am.info.width : props.imageWidth
  const maxY = am ? minY + am.info.height : props.imageHeight
  sel.x = Math.max(minX, sel.x)
  sel.y = Math.max(minY, sel.y)
  sel.w = Math.min(maxX - sel.x, sel.w)
  sel.h = Math.min(maxY - sel.y, sel.h)
  sel.w = Math.max(10, sel.w)
  sel.h = Math.max(10, sel.h)
  if (sel.x + sel.w > maxX) sel.x = maxX - sel.w
  if (sel.y + sel.h > maxY) sel.y = maxY - sel.h
  sel.x = Math.max(minX, sel.x)
  sel.y = Math.max(minY, sel.y)
}

function startResize(e: MouseEvent, handle: string) {
  const imgCoords = cssToImage(e.clientX, e.clientY)
  if (!imgCoords) return
  activeHandle.value = handle as ResizeHandle
  originSel.value = { ...selection.value }
  resizeDragStart.value = { imgX: imgCoords.x, imgY: imgCoords.y }
}

function getHandleStyle(h: string): Record<string, string> {
  return (handleStyles.value as Record<string, Record<string, string>>)[h] ?? {}
}

const HANDLE_KEYS = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w']

// Handle positions (computed, absolute within overlay)
const handleStyles = computed(() => {
  const d = cachedScale.value
  const sel = selection.value
  const HALF = 4  // 锚点尺寸 8px 的一半，居中定位
  if (d.scale <= 0 || sel.w < 5) {
    const hidden = { display: 'none' }
    return { nw: hidden, n: hidden, ne: hidden, e: hidden, se: hidden, s: hidden, sw: hidden, w: hidden }
  }
  const x0 = d.offsetX + sel.x * d.scale
  const y0 = d.offsetY + sel.y * d.scale
  const x1 = x0 + sel.w * d.scale
  const y1 = y0 + sel.h * d.scale
  const xm = (x0 + x1) / 2
  const ym = (y0 + y1) / 2
  const p = (l: number, t: number) => ({ left: (l - HALF) + 'px', top: (t - HALF) + 'px' })
  return {
    nw: p(x0, y0), n: p(xm, y0), ne: p(x1, y0),
    e: p(x1, ym), se: p(x1, y1), s: p(xm, y1),
    sw: p(x0, y1), w: p(x0, ym),
  }
})

// ============================================================================
// Cursor style
// ============================================================================
const HANDLE_CURSORS: Record<ResizeHandle, string> = {
  nw: 'nw-resize', n: 'n-resize', ne: 'ne-resize',
  e: 'e-resize', se: 'se-resize', s: 's-resize',
  sw: 'sw-resize', w: 'w-resize', move: 'move',
}
const cursorStyle = computed(() => {
  if (activeHandle.value) return HANDLE_CURSORS[activeHandle.value]
  return 'crosshair'
})

function onMouseLeave() {
  hoveredWindow.value = null
  showMagnifier.value = false
}

function handleWindowResize() {
  updateOverlayRect()
  computeScale()
}

/// 窗口边缘死区（约 5px 鼠标事件无法到达 webview），吸附阈值
const EDGE_SNAP = 10

function snapToEdge(ix: number, iy: number): { x: number; y: number } {
  const am = activeMon.value
  const minX = am ? am.info.x - props.imageOffsetX : 0
  const minY = am ? am.info.y - props.imageOffsetY : 0
  const maxX = am ? minX + am.info.width : props.imageWidth
  const maxY = am ? minY + am.info.height : props.imageHeight
  return {
    x: ix < minX + EDGE_SNAP ? minX : ix > maxX - EDGE_SNAP ? maxX - 1 : ix,
    y: iy < minY + EDGE_SNAP ? minY : iy > maxY - EDGE_SNAP ? maxY - 1 : iy,
  }
}

/** 每个显示器图像的定位样式（在 overlay 中） */
function monitorImgStyle(i: number): Record<string, string> {
  const d = cachedScale.value
  if (d.scale <= 0) return { display: 'none' }
  const m = props.monitors[i]
  const c = monitorToCssCoords(m)
  return {
    position: 'absolute',
    left: (c.x + 2) + 'px',  // +2 对应 overlay 的 -2px offset
    top: (c.y + 2) + 'px',
    width: c.w + 'px',
    height: c.h + 'px',
  }
}

/// 选区分辨率标签始终显示在选区内部底部居中，避免被 overflow:hidden 裁剪

// ============================================================================
// Lifecycle
// ============================================================================
let resizeObserver: ResizeObserver | null = null
/// 存储被修改的元素引用，unmount 时恢复
let titleBarElement: HTMLElement | null = null
let titleBarHadDragRegion = false
let appContainer: HTMLElement | null = null
/// 窗口原始位置/尺寸（物理像素），unmount 时恢复
let savedPos: { x: number; y: number } | null = null
let savedSize: { width: number; height: number } | null = null

onMounted(async () => {
  document.addEventListener('keydown', onKeydown)
  window.addEventListener('resize', handleWindowResize)
  document.body.style.overflow = 'hidden'
  document.documentElement.style.overflow = 'hidden'
  document.documentElement.style.overscrollBehavior = 'none'

  // 让整个应用容器对鼠标事件透明，确保只有截图覆盖层能接收鼠标事件
  // 这样彻底解决 title-bar drag region 在 WebView2 合成器层拦截事件的问题
  appContainer = document.querySelector('.app-container') as HTMLElement | null
  if (appContainer) {
    appContainer.style.pointerEvents = 'none'
  }

  titleBarElement = document.querySelector('.title-bar') as HTMLElement | null
  if (titleBarElement) {
    titleBarElement.style.display = 'none'
    titleBarHadDragRegion = titleBarElement.hasAttribute('data-tauri-drag-region')
    if (titleBarHadDragRegion) {
      titleBarElement.removeAttribute('data-tauri-drag-region')
    }
  }

  updateOverlayRect()
  computeScale()
  resizeObserver = new ResizeObserver(() => {
    updateOverlayRect()
    computeScale()
  })
  if (overlayRef.value) resizeObserver.observe(overlayRef.value)

  const win = getCurrentWindow()
  if (props.manageWindow === false) {
    // 独立遮罩窗口路径：承载页 ScreenshotWindow 已把窗口铺满虚拟桌面，
    // 本组件不触碰窗口几何。仅在 webview 尺寸稳定后重算一次缩放。
    setTimeout(() => { updateOverlayRect(); computeScale() }, 50)
  } else if (props.imageWidth > 0 && props.imageHeight > 0) {
    try {
      // 保存当前窗口位置和尺寸（物理像素），用于退出截图后恢复
      const p = await win.outerPosition()
      const s = await win.outerSize()
      savedPos = { x: p.x, y: p.y }
      savedSize = { width: s.width, height: s.height }

      // 将窗口扩展至覆盖所有被捕获的显示器
      // imageOffsetX/Y 是虚拟桌面左上角（物理像素坐标），imageWidth/Height 是合成图像尺寸
      // tauri.conf.json 已设 decorations:false，无需修改边框
      await win.setAlwaysOnTop(true)
      await win.setPosition(new PhysicalPosition(props.imageOffsetX, props.imageOffsetY))
      await win.setSize(new PhysicalSize(props.imageWidth, props.imageHeight))
      // 等待 webview viewport 更新后重算缩放
      setTimeout(() => { updateOverlayRect(); computeScale() }, 50)
    } catch {
      // 降级：使用当前显示器全屏（多屏时图像会被压缩，但至少可用）
      savedPos = null
      savedSize = null
      try { await win.setFullscreen(true) } catch {}
      try { await overlayRef.value?.requestFullscreen() } catch {}
    }
  } else {
    try { await win.setFullscreen(true) } catch {}
    try { await overlayRef.value?.requestFullscreen() } catch {}
  }
})

onUnmounted(async () => {
  document.removeEventListener('keydown', onKeydown)
  window.removeEventListener('resize', handleWindowResize)
  document.body.style.overflow = ''
  document.documentElement.style.overflow = ''
  document.documentElement.style.overscrollBehavior = ''

  // 恢复应用容器鼠标事件
  if (appContainer) {
    appContainer.style.pointerEvents = ''
  }
  // 恢复 title-bar
  if (titleBarElement) {
    titleBarElement.style.display = ''
    if (titleBarHadDragRegion) {
      titleBarElement.setAttribute('data-tauri-drag-region', '')
    }
  }

  resizeObserver?.disconnect()

  // 独立遮罩窗口路径：几何/关闭由承载页 ScreenshotWindow 负责，本组件不还原窗口。
  if (props.manageWindow === false) return

  const win = getCurrentWindow()
  try {
    await win.setAlwaysOnTop(false)
    if (savedPos && savedSize) {
      // 恢复窗口到截图前的位置和尺寸
      await win.setPosition(new PhysicalPosition(savedPos.x, savedPos.y))
      await win.setSize(new PhysicalSize(savedSize.width, savedSize.height))
    }
    savedPos = null
    savedSize = null
    // 确保窗口可见并获得焦点（不论截图是确认还是取消，覆盖"隐藏窗口截图"最小化路径）
    await win.show()
    await win.setFocus()
  } catch {}
  // 若降级路径用了 setFullscreen，一并退出
  try { await win.setFullscreen(false) } catch {}
  if (document.fullscreenElement) {
    try { await document.exitFullscreen() } catch {}
  }
})
</script>

<template>
  <Teleport to="body">
    <div
      class="screenshot-overlay"
      ref="overlayRef"
      :style="{ cursor: cursorStyle }"
      @mousedown="onMouseDown"
      @mousemove="onMouseMove"
      @mouseup="onMouseUp"
      @dblclick="onDblClick"
      @mouseleave="onMouseLeave"
      @contextmenu.prevent="emit('cancel')"
      @dragstart.prevent
      @selectstart.prevent
    >
      <!-- 各显示器独立背景图，按虚拟桌面坐标定位 -->
      <img
        v-for="(url, i) in monitorImages"
        :key="i"
        :src="url"
        class="monitor-bg"
        alt=""
        draggable="false"
        @dragstart.prevent
        :style="monitorImgStyle(i)"
      />

      <!-- 空闲（无选区、未悬浮窗口）时的整屏暗色层；一旦开始选区/悬浮窗口，
           改由 .selection-box / .window-highlight 的 box-shadow 挖亮，避免 SVG 每帧重栅格化 -->
      <div v-if="!selecting && selection.w < 5 && !hoveredWindow" class="dim-layer"></div>

      <!-- Hint when no selection -->
      <div v-if="!selecting && selection.w < 5" class="overlay-hint">
        <div class="hint-text">
          {{ hoveredWindow ? '点击选中当前窗口' : '拖拽选择区域，或悬浮到窗口后点击' }}
        </div>
        <div class="hint-keys">
          <kbd>Enter</kbd> 确认 · <kbd>Esc</kbd> 取消 · <kbd>↑↓←→</kbd> 微调控区
        </div>
      </div>

      <!-- Window highlight -->
      <div v-if="hoveredWindow" class="window-highlight" :style="highlightStyle">
        <div class="window-highlight-label">{{ hoveredWindow.title }}</div>
      </div>

      <!-- 选区：微信风格绿色细边框 + 顶部分辨率标签 -->
      <div v-show="selection.w > 5 || selecting"
        class="selection-box">
        <div ref="selSizeRef" class="sel-size" :class="{ 'sel-size--inside': sizeLabelInside }"></div>
      </div>

      <!-- Resize handles (8 handles around selection, visible when selection exists) -->
      <template v-if="selection.w > 5 && !selecting">
        <div v-for="h in HANDLE_KEYS" :key="h"
             class="sel-handle" :class="`sel-handle--${h}`"
             :style="getHandleStyle(h)"
             @mousedown.stop="startResize($event, h)" />
      </template>

      <!-- 浮动工具栏（选区完成后居中显示在选区下方） -->
      <div
        v-if="selBox && !selecting && !activeHandle"
        class="float-toolbar"
        :style="floatBtnStyle"
        @mousedown.stop
        @click.stop
      >
        <button class="ftb-btn" title="保存图片" @click="handleSave">
          <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <polyline points="7 10 12 15 17 10" />
            <line x1="12" y1="15" x2="12" y2="3" />
          </svg>
        </button>
        <span class="ftb-sep" />
        <button class="ftb-btn ftb-cancel" title="取消" @click="emit('cancel')">
          <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2.5" fill="none">
            <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
        <button class="ftb-btn ftb-confirm" title="确认截图" @click="handleConfirm">
          <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2.5" fill="none">
            <polyline points="20 6 9 17 4 12" />
          </svg>
        </button>
      </div>

      <!-- 放大镜（自由选区模式，未开始选区时显示） -->
      <ScreenshotMagnifier
        v-if="showMagnifier && magnifierProps.imageDataUrl"
        :mouseX="magnifierX"
        :mouseY="magnifierY"
        :imageDataUrl="magnifierProps.imageDataUrl"
        :imageWidth="magnifierProps.imageWidth"
        :imageHeight="magnifierProps.imageHeight"
        :scaleX="magnifierProps.scaleX"
        :scaleY="magnifierProps.scaleY"
        :offsetX="magnifierProps.offsetX"
        :offsetY="magnifierProps.offsetY"
      />
    </div>
  </Teleport>
</template>

<style scoped>
.screenshot-overlay {
  position: fixed;
  top: -2px;
  left: -2px;
  right: -2px;
  bottom: -2px;
  background: #000;
  z-index: 100001;
  user-select: none;
  overflow: hidden;
  touch-action: none;
  --msk-x: 0px;
  --msk-y: 0px;
  --msk-w: 0px;
  --msk-h: 0px;
}

.monitor-bg {
  opacity: 1;
  user-select: none;
  -webkit-user-drag: none;
  pointer-events: none;
}

/* 空闲时整屏暗色层（GPU 合成，静态无重绘） */
.dim-layer {
  position: absolute;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  pointer-events: none;
  z-index: 10;
}

/* ---- Hint text ---- */
.overlay-hint {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  text-align: center;
  color: rgba(255, 255, 255, 0.9);
  pointer-events: none;
  z-index: 10;
}

.hint-text {
  font-size: 18px;
  font-weight: 500;
  margin-bottom: 10px;
}

.hint-keys {
  font-size: var(--font-size-md, 13px);
  color: rgba(255, 255, 255, 0.55);
}

.hint-keys kbd {
  background: rgba(255, 255, 255, 0.12);
  padding: 2px 8px;
  border-radius: 4px;
  margin: 0 2px;
}

/* ---- Window highlight (specified_window mode) ---- */
.window-highlight {
  position: absolute;
  z-index: 15;
  pointer-events: none;
  outline: 2px solid #00c060;
  outline-offset: -1px;
  box-shadow: 0 0 0 200vmax rgba(0, 0, 0, 0.4);
}

.window-highlight-label {
  position: absolute;
  top: -28px;
  left: 0;
  background: #00c060;
  color: #fff;
  font-size: 12px;
  padding: 3px 10px;
  border-radius: 4px;
  white-space: nowrap;
  max-width: 320px;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* ---- Selection box（微信风格：绿色细边框；暗色遮罩由 SVG mask 负责）---- */
.selection-box {
  position: absolute;
  left: var(--msk-x);
  top: var(--msk-y);
  width: var(--msk-w);
  height: var(--msk-h);
  outline: 1px solid #00c060;
  outline-offset: -1px;
  /* box-shadow 挖孔：选区四周整屏压暗，选区内保持全亮。
     GPU 合成、随 CSS 变量移动零重栅格化，取代原 SVG mask 消除拖拽卡顿 */
  box-shadow: 0 0 0 200vmax rgba(0, 0, 0, 0.4);
  z-index: 20;
  pointer-events: none;
}

/* 分辨率标签：小巧、淡雅的半透明黑标签，默认贴在选区上方外侧左对齐 */
.sel-size {
  position: absolute;
  top: -24px;
  left: 0;
  background: rgba(0, 0, 0, 0.6);
  color: #fff;
  font-size: 12px;
  line-height: 1.5;
  padding: 1px 7px;
  border-radius: 4px;
  white-space: nowrap;
  letter-spacing: 0.3px;
  pointer-events: none;
}

/* 选区贴近屏幕顶部时，标签翻入选区内部左上角，避免被裁剪 */
.sel-size--inside {
  top: 3px;
  left: 3px;
}

/* ---- Floating toolbar（微信风格：白色圆角悬浮框 + 精致阴影）---- */
.float-toolbar {
  position: absolute;
  display: flex;
  align-items: center;
  gap: 2px;
  z-index: 30;
  pointer-events: all;
  background: #ffffff;
  border: 1px solid rgba(0, 0, 0, 0.06);
  border-radius: 8px;
  padding: 4px 6px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}

.ftb-btn {
  width: 34px;
  height: 34px;
  border-radius: 6px;
  border: none;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  color: #5a5a5a;
  transition: background 0.15s, color 0.15s;
}

.ftb-btn:hover {
  background: rgba(0, 0, 0, 0.06);
  color: #1a1a1a;
}

.ftb-sep {
  width: 1px;
  height: 18px;
  background: rgba(0, 0, 0, 0.1);
  margin: 0 3px;
  flex-shrink: 0;
}

.ftb-cancel {
  color: #ff5f57;
}
.ftb-cancel:hover {
  background: rgba(255, 95, 87, 0.12);
  color: #ff5f57;
}

.ftb-confirm {
  color: #00c060;
}
.ftb-confirm:hover {
  background: rgba(0, 192, 96, 0.12);
  color: #00c060;
}

/* ---- Resize handles（8 个精致小方块锚点）---- */
.sel-handle {
  position: absolute;
  width: 8px;
  height: 8px;
  background: #fff;
  border: 1.5px solid #00c060;
  border-radius: 2px;
  z-index: 25;
  pointer-events: all;
  box-sizing: border-box;
  box-shadow: 0 0 2px rgba(0, 0, 0, 0.25);
}

.sel-handle--nw { cursor: nw-resize; }
.sel-handle--n  { cursor: n-resize;  }
.sel-handle--ne { cursor: ne-resize; }
.sel-handle--e  { cursor: e-resize;  }
.sel-handle--se { cursor: se-resize; }
.sel-handle--s  { cursor: s-resize;  }
.sel-handle--sw { cursor: sw-resize; }
.sel-handle--w  { cursor: w-resize;  }
</style>
