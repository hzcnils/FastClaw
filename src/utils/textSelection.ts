function collectSelectionRects(sel: Selection): DOMRect[] {
  const rects: DOMRect[] = []
  for (let r = 0; r < sel.rangeCount; r++) {
    const range = sel.getRangeAt(r)
    rects.push(...Array.from(range.getClientRects()))
    if (rects.length === 0) {
      const box = range.getBoundingClientRect()
      if (box.width > 0 || box.height > 0) {
        rects.push(box)
      }
    }
  }
  return rects
}

/** 判断指针是否落在当前页面选区高亮区域内（仅按坐标，避免父容器 intersectsNode 误判） */
export function isPointerInSelection(event: { clientX: number; clientY: number; target: EventTarget | null }): boolean {
  const sel = window.getSelection()
  if (!sel || sel.isCollapsed || !sel.rangeCount) return false

  const { clientX: x, clientY: y } = event
  for (const rect of collectSelectionRects(sel)) {
    if (x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom) {
      return true
    }
  }
  return false
}

/** 点击选区外时清除页面文本选区 */
export function clearDocumentSelectionIfOutside(event: PointerEvent | MouseEvent): void {
  if (event.button !== 0) return

  const sel = window.getSelection()
  if (sel && !sel.isCollapsed && sel.rangeCount && !isPointerInSelection(event)) {
    sel.removeAllRanges()
  }

  // 输入框内选区：点击到外部时折叠光标
  const active = document.activeElement
  if (!(active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement)) return
  const target = event.target
  if (target instanceof Node && (active === target || active.contains(target))) return
  const pos = active.selectionStart ?? 0
  if ((active.selectionEnd ?? 0) > pos) {
    active.setSelectionRange(pos, pos)
  }
}

let installed = false

function onClearSelectionPointer(event: Event) {
  clearDocumentSelectionIfOutside(event as PointerEvent | MouseEvent)
}

/** 全局：左键点击任意非选区位置时取消文本选中 */
export function installClearSelectionOnClickOutside(): void {
  if (installed || typeof document === 'undefined') return
  installed = true
  document.addEventListener('pointerdown', onClearSelectionPointer, true)
  // WebView2 等环境下 pointerdown 可能不稳定，mousedown 兜底
  document.addEventListener('mousedown', onClearSelectionPointer, true)
}

export function uninstallClearSelectionOnClickOutside(): void {
  if (!installed || typeof document === 'undefined') return
  installed = false
  document.removeEventListener('pointerdown', onClearSelectionPointer, true)
  document.removeEventListener('mousedown', onClearSelectionPointer, true)
}
