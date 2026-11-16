<script setup lang="ts">
/**
 * 全局自定义右键菜单（按场景区分）
 * - 输入框：撤销 / 复制 / 粘贴 / 剪切 / 全选
 * - 已选中文字：剪切 / 复制 / 全选（只读选区无剪切）
 * - 标题栏：还原 / 移动 / 大小 / 最小化 / 最大化 / 关闭（样式同输入框菜单）
 * - 普通界面：不弹出（生产环境仍由 main.ts 屏蔽系统默认菜单）
 */
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import { isPointerInSelection } from '@/utils/textSelection'

type MenuMode = 'editable' | 'selection' | 'window'
type MenuAction =
  | 'undo'
  | 'copy'
  | 'paste'
  | 'cut'
  | 'selectAll'
  | 'restore'
  | 'move'
  | 'size'
  | 'minimize'
  | 'maximize'
  | 'close'
  | 'separator'

interface MenuItem {
  key: MenuAction
  label?: string
  shortcut?: string
  disabled?: boolean
  /** 窗口菜单左侧图标 */
  icon?: 'restore' | 'minimize' | 'maximize' | 'close' | 'none'
}

const { t } = useI18n()

const visible = ref(false)
const posX = ref(0)
const posY = ref(0)
const mode = ref<MenuMode>('editable')
const targetEl = ref<HTMLElement | null>(null)
const canEdit = ref(false)
const hasTextSelection = ref(false)
const hasClipboardText = ref(false)
const isMaximized = ref(false)
const menuRef = ref<HTMLElement | null>(null)

function isTitleBarTarget(start: EventTarget | null): boolean {
  const el = start instanceof Element ? start : null
  return !!(el && el.closest('.title-bar'))
}

function windowLabel(key: string, accessKey: string) {
  return `${t(`contextMenu.${key}`)}(${accessKey})`
}

const TEXT_INPUT_TYPES = new Set([
  'text',
  'search',
  'password',
  'email',
  'url',
  'tel',
  'number',
  '',
])

function isEditableElement(el: Element | null): el is HTMLElement {
  if (!(el instanceof HTMLElement)) return false
  if (el instanceof HTMLTextAreaElement) {
    return !el.disabled && !el.readOnly
  }
  if (el instanceof HTMLInputElement) {
    if (el.disabled || el.readOnly) return false
    return TEXT_INPUT_TYPES.has((el.type || '').toLowerCase())
  }
  return el.isContentEditable
}

function findEditable(start: EventTarget | null): HTMLElement | null {
  let cur: Element | null = start instanceof Element ? start : null
  while (cur) {
    if (isEditableElement(cur)) return cur
    cur = cur.parentElement
  }
  return null
}

function getPageSelectedText(): string {
  const sel = window.getSelection()
  if (!sel || sel.isCollapsed) return ''
  return sel.toString()
}

/** 已有独立右键菜单的区域，不弹出全局文本/选区菜单 */
const CUSTOM_CONTEXT_MENU_SELECTORS = [
  '.tab-bar',
  '.tab-context-menu',
  '.kb-ctx-menu',
  '.context-menu-backdrop',
  '.context-menu',
  '.editable-ctx-menu',
]

function isCustomContextMenuZone(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false
  return CUSTOM_CONTEXT_MENU_SELECTORS.some((selector) => !!target.closest(selector))
    || !!target.closest('[data-app-contextmenu]')
}

function getSelectionState(el: HTMLElement | null): boolean {
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
    const start = el.selectionStart ?? 0
    const end = el.selectionEnd ?? 0
    return end > start
  }
  if (el) {
    const sel = window.getSelection()
    if (!sel || sel.isCollapsed || !sel.rangeCount) return false
    const anchor = sel.anchorNode
    return !!(anchor && el.contains(anchor) && sel.toString())
  }
  return !!getPageSelectedText()
}

function getSelectedText(el: HTMLElement | null): string {
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
    const start = el.selectionStart ?? 0
    const end = el.selectionEnd ?? 0
    return el.value.slice(start, end)
  }
  if (el) {
    const sel = window.getSelection()
    if (sel && !sel.isCollapsed && sel.anchorNode && el.contains(sel.anchorNode)) {
      return sel.toString()
    }
  }
  return getPageSelectedText()
}

function dispatchInput(el: HTMLElement) {
  el.dispatchEvent(new Event('input', { bubbles: true }))
  el.dispatchEvent(new Event('change', { bubbles: true }))
}

function insertTextAtCursor(el: HTMLElement, text: string) {
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
    const start = el.selectionStart ?? el.value.length
    const end = el.selectionEnd ?? el.value.length
    const value = el.value
    el.value = value.slice(0, start) + text + value.slice(end)
    const pos = start + text.length
    el.setSelectionRange(pos, pos)
    dispatchInput(el)
    return
  }
  el.focus()
  if (!document.execCommand('insertText', false, text)) {
    const sel = window.getSelection()
    if (sel && sel.rangeCount) {
      const range = sel.getRangeAt(0)
      range.deleteContents()
      range.insertNode(document.createTextNode(text))
      range.collapse(false)
      sel.removeAllRanges()
      sel.addRange(range)
    }
  }
  dispatchInput(el)
}

function deleteSelection(el: HTMLElement) {
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
    const start = el.selectionStart ?? 0
    const end = el.selectionEnd ?? 0
    if (end <= start) return
    el.value = el.value.slice(0, start) + el.value.slice(end)
    el.setSelectionRange(start, start)
    dispatchInput(el)
    return
  }
  document.execCommand('delete')
  dispatchInput(el)
}

const menuItems = computed<MenuItem[]>(() => {
  const selected = hasTextSelection.value
  const editable = canEdit.value

  if (mode.value === 'window') {
    return [
      {
        key: 'restore',
        label: windowLabel('restore', 'R'),
        icon: 'restore',
        disabled: !isMaximized.value,
      },
      {
        key: 'move',
        label: windowLabel('move', 'M'),
        icon: 'none',
        disabled: isMaximized.value,
      },
      {
        key: 'size',
        label: windowLabel('size', 'S'),
        icon: 'none',
        disabled: true,
      },
      {
        key: 'minimize',
        label: windowLabel('minimize', 'N'),
        icon: 'minimize',
        disabled: false,
      },
      {
        key: 'maximize',
        label: windowLabel('maximize', 'X'),
        icon: 'maximize',
        disabled: isMaximized.value,
      },
      { key: 'separator' },
      {
        key: 'close',
        label: windowLabel('close', 'C'),
        icon: 'close',
        shortcut: 'Alt+F4',
        disabled: false,
      },
    ]
  }

  if (mode.value === 'selection') {
    const items: MenuItem[] = []
    // 只读选区不提供剪切
    if (editable) {
      items.push({
        key: 'cut',
        label: t('contextMenu.cut'),
        shortcut: 'Ctrl+X',
        disabled: !selected,
      })
    }
    items.push(
      {
        key: 'copy',
        label: t('contextMenu.copy'),
        shortcut: 'Ctrl+C',
        disabled: !selected,
      },
      {
        key: 'selectAll',
        label: t('contextMenu.selectAll'),
        shortcut: 'Ctrl+A',
        disabled: false,
      },
    )
    return items
  }

  // editable（未选中文本）
  return [
    {
      key: 'undo',
      label: t('contextMenu.undo'),
      shortcut: 'Ctrl+Z',
      disabled: false,
    },
    {
      key: 'copy',
      label: t('contextMenu.copy'),
      shortcut: 'Ctrl+C',
      disabled: !selected,
    },
    {
      key: 'paste',
      label: t('contextMenu.paste'),
      shortcut: 'Ctrl+V',
      // 剪贴板有内容才可粘贴
      disabled: !hasClipboardText.value,
    },
    {
      key: 'cut',
      label: t('contextMenu.cut'),
      shortcut: 'Ctrl+X',
      disabled: !selected,
    },
    {
      key: 'selectAll',
      label: t('contextMenu.selectAll'),
      shortcut: 'Ctrl+A',
      disabled: false,
    },
  ]
})

function placeMenu(clientX: number, clientY: number) {
  posX.value = clientX
  posY.value = clientY
  nextTick(() => {
    const menu = menuRef.value
    if (!menu) return
    const rect = menu.getBoundingClientRect()
    const pad = 8
    let x = clientX
    let y = clientY
    if (x + rect.width > window.innerWidth - pad) x = window.innerWidth - rect.width - pad
    if (y + rect.height > window.innerHeight - pad) y = window.innerHeight - rect.height - pad
    if (x < pad) x = pad
    if (y < pad) y = pad
    posX.value = x
    posY.value = y
  })
}

function hideMenu() {
  visible.value = false
  targetEl.value = null
  canEdit.value = false
  hasClipboardText.value = false
}

async function refreshClipboardState() {
  try {
    const text = await navigator.clipboard.readText()
    hasClipboardText.value = !!(text && text.length > 0)
  } catch {
    // 无权限或不可读时视为无内容，禁用粘贴
    hasClipboardText.value = false
  }
}

async function onContextMenu(e: MouseEvent) {
  // 标题栏：替换 Windows 系统菜单，样式与输入框右键菜单一致
  if (isTitleBarTarget(e.target)) {
    e.preventDefault()
    e.stopPropagation()
    targetEl.value = null
    canEdit.value = false
    hasTextSelection.value = false
    hasClipboardText.value = false
    mode.value = 'window'
    try {
      isMaximized.value = await getCurrentWindow().isMaximized()
    } catch {
      isMaximized.value = false
    }
    visible.value = true
    placeMenu(e.clientX, e.clientY)
    return
  }

  // Tab 栏等已有专属右键菜单的区域：不弹出全局选区菜单
  if (isCustomContextMenuZone(e.target)) {
    return
  }

  const editable = findEditable(e.target)
  const selected = getSelectionState(editable)

  targetEl.value = editable
  canEdit.value = !!editable
  hasTextSelection.value = selected
  hasClipboardText.value = false

  if (editable) {
    mode.value = selected ? 'selection' : 'editable'
    try {
      editable.focus({ preventScroll: true })
    } catch {
      editable.focus()
    }
    // 未选中时需要根据剪贴板决定粘贴是否可用
    if (!selected) {
      await refreshClipboardState()
    }
  } else if (selected && isPointerInSelection(e)) {
    mode.value = 'selection'
  } else {
    // 普通界面，或选区与点击位置无关时不弹菜单
    return
  }

  e.preventDefault()
  visible.value = true
  placeMenu(e.clientX, e.clientY)
}

function onPointerDown(e: PointerEvent) {
  if (!visible.value) return
  const menu = menuRef.value
  if (menu && e.target instanceof Node && menu.contains(e.target)) return
  hideMenu()
}

function onKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape' && visible.value) {
    hideMenu()
    return
  }
  // 窗口菜单加速键：R / M / N / X / C
  if (!visible.value || mode.value !== 'window') return
  const key = e.key.toUpperCase()
  const map: Record<string, MenuAction> = {
    R: 'restore',
    M: 'move',
    S: 'size',
    N: 'minimize',
    X: 'maximize',
    C: 'close',
  }
  const action = map[key]
  if (!action) return
  const item = menuItems.value.find((it) => it.key === action)
  if (!item || item.disabled) return
  e.preventDefault()
  void runAction(action)
}

async function runAction(action: MenuAction) {
  if (action === 'separator') return

  const el = targetEl.value
  if (el && (action === 'undo' || action === 'paste' || action === 'cut' || action === 'selectAll' || action === 'copy')) {
    try {
      el.focus({ preventScroll: true })
    } catch {
      el.focus()
    }
  }

  try {
    switch (action) {
      case 'restore': {
        const win = getCurrentWindow()
        if (await win.isMaximized()) await win.unmaximize()
        break
      }
      case 'move': {
        // 关闭菜单后再进入拖拽，避免菜单挡住
        hideMenu()
        await nextTick()
        try {
          await getCurrentWindow().startDragging()
        } catch {
          /* 非 Tauri 环境忽略 */
        }
        return
      }
      case 'size':
        break
      case 'minimize':
        await getCurrentWindow().minimize()
        break
      case 'maximize': {
        const win = getCurrentWindow()
        if (!(await win.isMaximized())) await win.maximize()
        break
      }
      case 'close': {
        try {
          const isTrayEnabled = await invoke<boolean>('get_system_tray_enabled')
          const win = getCurrentWindow()
          try { await invoke('save_main_window_state') } catch { /* ignore */ }
          await win.hide()
          if (!isTrayEnabled) {
            try {
              await invoke('exit_app')
            } catch {
              await win.close()
            }
          }
        } catch {
          try {
            await getCurrentWindow().close()
          } catch {
            /* ignore */
          }
        }
        break
      }
      case 'undo':
        if (el) {
          document.execCommand('undo')
          dispatchInput(el)
        }
        break
      case 'copy': {
        const text = getSelectedText(el)
        if (!text) break
        try {
          await navigator.clipboard.writeText(text)
        } catch {
          document.execCommand('copy')
        }
        break
      }
      case 'cut': {
        if (!el) break
        const text = getSelectedText(el)
        if (!text) break
        if (!document.execCommand('cut')) {
          await navigator.clipboard.writeText(text)
          deleteSelection(el)
        } else {
          dispatchInput(el)
        }
        break
      }
      case 'paste': {
        if (!el) break
        try {
          const text = await navigator.clipboard.readText()
          if (text) insertTextAtCursor(el, text)
        } catch {
          document.execCommand('paste')
          dispatchInput(el)
        }
        break
      }
      case 'selectAll': {
        if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
          el.select()
        } else {
          document.execCommand('selectAll')
        }
        break
      }
    }
  } catch (err) {
    console.warn('[EditableContextMenu] action failed:', action, err)
  }

  hideMenu()
}

onMounted(() => {
  document.addEventListener('contextmenu', onContextMenu, true)
  document.addEventListener('pointerdown', onPointerDown, true)
  document.addEventListener('keydown', onKeyDown)
  window.addEventListener('blur', hideMenu)
  window.addEventListener('resize', hideMenu)
  window.addEventListener('scroll', hideMenu, true)
})

onUnmounted(() => {
  document.removeEventListener('contextmenu', onContextMenu, true)
  document.removeEventListener('pointerdown', onPointerDown, true)
  document.removeEventListener('keydown', onKeyDown)
  window.removeEventListener('blur', hideMenu)
  window.removeEventListener('resize', hideMenu)
  window.removeEventListener('scroll', hideMenu, true)
})
</script>

<template>
  <Teleport to="body">
    <div
      v-if="visible"
      ref="menuRef"
      class="editable-ctx-menu"
      :class="{ 'editable-ctx-menu--window': mode === 'window' }"
      :style="{ left: posX + 'px', top: posY + 'px' }"
      @contextmenu.prevent
    >
      <template v-for="(item, idx) in menuItems" :key="item.key + '-' + idx">
        <div v-if="item.key === 'separator'" class="editable-ctx-menu__sep" />
        <button
          v-else
          type="button"
          class="editable-ctx-menu__item"
          :disabled="item.disabled"
          @click="runAction(item.key)"
        >
          <span v-if="mode === 'window'" class="editable-ctx-menu__icon" aria-hidden="true">
            <!-- 还原：重叠方块 -->
            <svg v-if="item.icon === 'restore'" viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.2">
              <rect x="4.5" y="1.5" width="9" height="9" rx="0.5" />
              <path d="M3.5 4.5H2.5a1 1 0 0 0-1 1v7a1 1 0 0 0 1 1h7a1 1 0 0 0 1-1v-1" />
            </svg>
            <!-- 最小化：横线 -->
            <svg v-else-if="item.icon === 'minimize'" viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.4">
              <line x1="3" y1="8" x2="13" y2="8" />
            </svg>
            <!-- 最大化：方框 -->
            <svg v-else-if="item.icon === 'maximize'" viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.2">
              <rect x="2.5" y="2.5" width="11" height="11" rx="0.5" />
            </svg>
            <!-- 关闭：X -->
            <svg v-else-if="item.icon === 'close'" viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.4">
              <line x1="4" y1="4" x2="12" y2="12" />
              <line x1="12" y1="4" x2="4" y2="12" />
            </svg>
          </span>
          <span class="editable-ctx-menu__label">{{ item.label }}</span>
          <span v-if="item.shortcut" class="editable-ctx-menu__shortcut">{{ item.shortcut }}</span>
        </button>
      </template>
    </div>
  </Teleport>
</template>

<style scoped>
.editable-ctx-menu {
  position: fixed;
  z-index: 100000;
  min-width: 220px;
  padding: 6px 0;
  border-radius: 8px;
  background: var(--theme-dailog-color, var(--bg-primary, #fff));
  border: 1px solid var(--border-color, rgba(0, 0, 0, 0.08));
  box-shadow: 0 10px 28px rgba(15, 23, 42, 0.16);
  color: var(--text-primary, #1f2937);
  user-select: none;
}

.editable-ctx-menu--window {
  min-width: 200px;
}

.editable-ctx-menu__sep {
  height: 1px;
  margin: 6px 10px;
  background: var(--border-color, rgba(0, 0, 0, 0.08));
}

.editable-ctx-menu--window .editable-ctx-menu__sep {
  margin-left: 36px;
}

.editable-ctx-menu__item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 28px;
  width: 100%;
  padding: 8px 16px;
  border: none;
  background: transparent;
  text-align: left;
  font-size: var(--font-size-base, 14px);
  line-height: 1.4;
  color: inherit;
  cursor: pointer;
}

.editable-ctx-menu--window .editable-ctx-menu__item {
  gap: 8px;
  padding: 7px 14px 7px 8px;
}

.editable-ctx-menu__icon {
  flex-shrink: 0;
  width: 28px;
  height: 18px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--text-primary, #1f2937);
}

.editable-ctx-menu__label {
  flex: 1;
  min-width: 0;
}

.editable-ctx-menu__shortcut {
  flex-shrink: 0;
  font-size: 12px;
  color: var(--text-secondary, #98a2b3);
  letter-spacing: 0.02em;
}

.editable-ctx-menu--window .editable-ctx-menu__shortcut {
  font-size: var(--font-size-base, 14px);
  color: inherit;
  letter-spacing: 0;
}

.editable-ctx-menu__item:hover:not(:disabled) {
  background: var(--dropdown-hover-color, rgba(24, 144, 255, 0.08));
  color: var(--color-primary, #1890ff);
}

.editable-ctx-menu__item:hover:not(:disabled) .editable-ctx-menu__shortcut {
  color: inherit;
  opacity: 0.75;
}

.editable-ctx-menu__item:hover:not(:disabled) .editable-ctx-menu__icon {
  color: inherit;
}

.editable-ctx-menu__item:disabled {
  opacity: 0.42;
  cursor: not-allowed;
}
</style>
