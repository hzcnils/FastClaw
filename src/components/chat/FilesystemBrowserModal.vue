<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { ElMessage } from 'element-plus'
import { FolderOpen, Folder, File, Search, HardDrive, Eye, EyeOff } from 'lucide-vue-next'
import SearchClearButton from '@/components/common/SearchClearButton.vue'

const { t } = useI18n()

// ── Types ──
interface BrowseEntry {
  path: string
  name: string
  type: 'directory' | 'file'
}

export interface FilesystemSelection {
  path: string
  name: string
  type: 'directory' | 'file'
}

// ── Props / Emits ──
const props = withDefaults(defineProps<{
  modelValue: boolean
  initialPath?: string
}>(), {
  initialPath: '',
})

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  'confirm': [selections: FilesystemSelection[]]
}>()

// ── State ──
const currentPath = ref('')
const pathInput = ref('')
const entries = ref<BrowseEntry[]>([])
const drives = ref<string[]>([])
const loading = ref(false)
const error = ref<string | null>(null)
const searchQuery = ref('')
const showHidden = ref(false)
const checkedPaths = ref<Set<string>>(new Set())
const lastClickedIndex = ref(-1)
const anchorIndex = ref(-1)

// ── Computed ──
const visible = computed({
  get: () => props.modelValue,
  set: (v) => emit('update:modelValue', v),
})

const parentPath = computed(() => {
  if (!currentPath.value) return null
  // Windows: C:\ → null, D:\dir → D:\
  const p = currentPath.value.replace(/\\/g, '/')
  if (p === '/') return null
  // Drive root: "C:/" → null
  if (/^[A-Za-z]:\/$/.test(p)) return null
  const parts = p.replace(/\/+$/, '').split('/')
  parts.pop()
  if (parts.length === 0) return null
  const parent = parts.join('/')
  // If parent is just a drive letter without slash
  if (/^[A-Za-z]:$/.test(parent)) return parent + '/'
  return parent || '/'
})

const visibleEntries = computed(() => {
  let result = entries.value
  // Filter hidden files (dot-prefixed)
  if (!showHidden.value) {
    result = result.filter(e => !e.name.startsWith('.'))
  }
  // Filter by search
  if (searchQuery.value.trim()) {
    const q = searchQuery.value.toLowerCase()
    result = result.filter(e => e.name.toLowerCase().includes(q))
  }
  // Sort: dirs first, then alpha
  return [...result].sort((a, b) => {
    if (a.type !== b.type) return a.type === 'directory' ? -1 : 1
    return a.name.localeCompare(b.name, undefined, { sensitivity: 'base' })
  })
})

const selectedCount = computed(() => checkedPaths.value.size)

const canConfirm = computed(() => selectedCount.value > 0)

// ── Methods ──
async function loadPath(path: string) {
  loading.value = true
  error.value = null
  checkedPaths.value = new Set()
  lastClickedIndex.value = -1
  anchorIndex.value = -1
  try {
    const result = await invoke<{ path: string; suggestions: BrowseEntry[]; drives: string[] }>(
      'browse_filesystem',
      { path: path || undefined },
    )
    currentPath.value = result.path.replace(/\\/g, '/')
    pathInput.value = result.path.replace(/\\/g, '/')
    entries.value = result.suggestions.map(e => ({
      ...e,
      path: e.path.replace(/\\/g, '/'),
    }))
    drives.value = result.drives || []
  } catch (e: any) {
    error.value = typeof e === 'string' ? e : e?.message || t('filesystemBrowser.loadError')
  } finally {
    loading.value = false
  }
}

function navigateTo(newPath: string) {
  loadPath(newPath)
}

function navigateUp() {
  if (parentPath.value) loadPath(parentPath.value)
}

function navigateToPathInput() {
  const p = pathInput.value.trim()
  if (p) loadPath(p)
}

function toggleCheck(entryPath: string) {
  const next = new Set(checkedPaths.value)
  if (next.has(entryPath)) {
    next.delete(entryPath)
  } else {
    next.add(entryPath)
  }
  checkedPaths.value = next
}

function clearSelection() {
  checkedPaths.value = new Set()
  lastClickedIndex.value = -1
  anchorIndex.value = -1
}

// ── Selection (Windows Explorer style) ──
function handleEntryClick(entry: BrowseEntry, index: number, event: MouseEvent) {
  if (event.shiftKey && anchorIndex.value >= 0) {
    // Shift+Click: 范围选择（保留批量多选）
    const [from, to] = [anchorIndex.value, index].sort((a, b) => a - b)
    const rangePaths = visibleEntries.value.slice(from, to + 1).map(e => e.path)
    checkedPaths.value = new Set(rangePaths)
    lastClickedIndex.value = index
  } else {
    // 普通单击（含 Ctrl/Cmd，无需快捷键）：切换选中，再点取消
    toggleCheck(entry.path)
    lastClickedIndex.value = index
    if (checkedPaths.value.has(entry.path)) {
      anchorIndex.value = index
    }
  }
}

function handleEntryDoubleClick(entry: BrowseEntry) {
  if (entry.type === 'directory') {
    loadPath(entry.path)
  }
}

function isInputFocused(): boolean {
  const el = document.activeElement
  if (!el) return false
  const tag = el.tagName.toLowerCase()
  if (tag === 'input' || tag === 'textarea') return true
  // Element Plus 的 el-input 内部包裹了一个 input
  if ((el as HTMLElement).closest?.('.el-input__wrapper') || (el as HTMLElement).closest?.('.el-textarea__wrapper')) return true
  return false
}

function handleKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === 'a') {
    if (isInputFocused()) return  // 输入框中 Ctrl+A 走浏览器默认全选文本
    e.preventDefault()
    checkedPaths.value = new Set(visibleEntries.value.map(e => e.path))
  }
  if (e.key === 'Escape') {
    handleClose()
  }
}

function handleConfirm() {
  const selected: FilesystemSelection[] = []
  for (const entry of entries.value) {
    if (checkedPaths.value.has(entry.path)) {
      selected.push({
        path: entry.path,
        name: entry.name,
        type: entry.type,
      })
    }
  }
  emit('confirm', selected)
  visible.value = false
}

function handleClose() {
  error.value = null
  searchQuery.value = ''
  checkedPaths.value = new Set()
  visible.value = false
}

function selectDrive(drive: string) {
  loadPath(drive)
}

function getEntryIconClass(entry: BrowseEntry) {
  return entry.type === 'directory' ? 'entry-icon--folder' : 'entry-icon--file'
}

function getEntryIcon(entry: BrowseEntry) {
  return entry.type === 'directory' ? FolderOpen : File
}

// ── Watch for open ──
watch(() => props.modelValue, (isOpen) => {
  if (isOpen) {
    const initial = props.initialPath || ''
    loadPath(initial)
  }
})

// Keyboard listener
function onWindowKeydown(e: KeyboardEvent) {
  if (props.modelValue) {
    handleKeydown(e)
  }
}

onMounted(() => {
  window.addEventListener('keydown', onWindowKeydown)
})
onUnmounted(() => {
  window.removeEventListener('keydown', onWindowKeydown)
})
</script>

<template>
  <el-dialog v-model="visible" :title="$t('filesystemBrowser.title')" width="700px" top="5vh"
    :close-on-click-modal="false" @close="handleClose" class="fsb-dialog" append-to-body>
    <div class="fsb-container">
      <!-- Drives -->
      <div v-if="drives.length > 0" class="fsb-drives">
        <span class="fsb-section-label">{{ $t('filesystemBrowser.drives') }}:</span>
        <div class="fsb-drive-chips">
          <button v-for="drive in drives" :key="drive" class="fsb-drive-chip"
            :class="{ active: currentPath.toUpperCase().startsWith(drive.toUpperCase()) }" @click="selectDrive(drive)">
            <HardDrive :size="12" />
            <span>{{ drive.replace(/[:\/\\]+$/, '') }}</span>
          </button>
        </div>
      </div>

      <!-- Search + hidden toggle -->
      <div class="fsb-toolbar">
        <el-input v-model="searchQuery" :placeholder="$t('filesystemBrowser.searchPlaceholder')"
          class="fsb-search">
          <template #prefix>
            <Search :size="13" />
          </template>
          <template #suffix>
            <SearchClearButton v-if="searchQuery" @clear="searchQuery = ''" />
          </template>
        </el-input>
        <el-button :type="showHidden ? 'primary' : 'default'" :icon="showHidden ? EyeOff : Eye"
          @click="showHidden = !showHidden"
          :title="showHidden ? $t('filesystemBrowser.hideHidden') : $t('filesystemBrowser.showHidden')" />
      </div>

      <!-- Error -->
      <el-alert v-if="error" :title="error" type="error" show-icon :closable="true" @close="error = null"
        class="fsb-error" />

      <!-- Entry list -->
      <div class="fsb-entries" @keydown="handleKeydown">
        <!-- Loading -->
        <div v-if="loading" class="fsb-state">
          <el-icon class="is-loading" :size="20">
            <component :is="'Loading'" />
          </el-icon>
          <span>{{ $t('filesystemBrowser.loading') }}</span>
        </div>

        <!-- Empty -->
        <div v-else-if="visibleEntries.length === 0 && !loading" class="fsb-state">
          <span>{{ $t('filesystemBrowser.emptyDir') }}</span>
        </div>

        <!-- Parent directory ".." -->
        <div v-if="parentPath !== null && !loading" class="fsb-entry fsb-entry--parent" @dblclick="navigateUp()">
          <span class="fsb-entry-check-placeholder" />
          <FolderOpen :size="16" class="entry-icon--folder" />
          <span class="fsb-entry-name">..</span>
        </div>

        <!-- Entries -->
        <div v-for="(entry, idx) in visibleEntries" :key="entry.path" class="fsb-entry" :class="{
          'fsb-entry--checked': checkedPaths.has(entry.path),
          'fsb-entry--dir': entry.type === 'directory',
        }" @click="handleEntryClick(entry, idx, $event)" @dblclick="handleEntryDoubleClick(entry)">
          <el-checkbox :model-value="checkedPaths.has(entry.path)" @click.stop @change="toggleCheck(entry.path)"
            class="fsb-entry-check" />
          <component :is="getEntryIcon(entry)" :size="16" :class="getEntryIconClass(entry)" />
          <span class="fsb-entry-name">{{ entry.name }}</span>
          <span v-if="entry.type === 'directory'" class="fsb-entry-type-tag">{{ $t('filesystemBrowser.folder') }}</span>
        </div>
      </div>

      <!-- Path bar (bottom) -->
      <div class="fsb-path-bar">
        <el-input v-model="pathInput" :placeholder="$t('filesystemBrowser.pathLabel')" size="small"
          @keydown.enter="navigateToPathInput" class="fsb-path-input">
          <template #prefix>
            <FolderOpen :size="14" class="fsb-path-icon" />
          </template>
        </el-input>
        <el-button size="small" @click="navigateToPathInput">{{ $t('filesystemBrowser.jump') }}</el-button>
      </div>

      <!-- Footer -->
      <div class="fsb-footer">
        <div class="fsb-footer-left">
          <span v-if="selectedCount > 0" class="fsb-selected-count">
            {{ $t('filesystemBrowser.selectedCount', { n: selectedCount }) }}
          </span>
          <span v-else class="fsb-selected-count fsb-selected-count--empty">
            {{ $t('filesystemBrowser.noSelection') }}
          </span>
          <el-button v-if="selectedCount > 0" link type="primary" size="small" @click="clearSelection">
            {{ $t('filesystemBrowser.clearSelection') }}
          </el-button>
        </div>
        <div class="fsb-footer-actions">
          <el-button class="fsb-btn-cancel" @click="handleClose">{{ $t('common.cancel') }}</el-button>
          <el-button type="primary" :disabled="!canConfirm" @click="handleConfirm">
            {{ $t('filesystemBrowser.confirmBtn', { n: selectedCount }) }}
          </el-button>
        </div>
      </div>
    </div>
  </el-dialog>
</template>

<style lang="scss" scoped>
.fsb-dialog {
  :deep(.el-dialog__header) {
    border-bottom: 1px solid var(--border-color);
    padding: 16px 20px;
    margin: 0;
  }

  :deep(.el-dialog__body) {
    padding: 0;
  }
}

.fsb-container {
  display: flex;
  flex-direction: column;
  height: 75vh;
}

// ── Drives ──
.fsb-drives {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 20px;
  border-bottom: 1px solid var(--border-color);
  flex-wrap: wrap;
}

.fsb-section-label {
  font-size: 12px;
  color: var(--text-secondary);
  flex-shrink: 0;
}

.fsb-drive-chips {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
}

.fsb-drive-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px 10px;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  background: transparent;
  color: var(--text-primary);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.15s;

  &:hover {
    background: var(--bg-hover);
  }

  &.active {
    border-color: var(--color-primary);
    color: var(--color-primary);
    background: rgba(24, 144, 255, 0.06);
  }
}

// ── Toolbar (search + hidden toggle) ──
.fsb-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 20px;
  border-bottom: 1px solid var(--border-color);

  .fsb-search {
    flex: 1;
  }
}

// ── Error ──
.fsb-error {
  margin: 8px 20px;
}

// ── Entries list ──
.fsb-entries {
  flex: 1;
  overflow-y: auto;
  min-height: 200px;
  // max-height: 300px;
  padding: 4px 0;
  outline: none;
}

.fsb-state {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  height: 160px;
  color: var(--text-secondary);
  font-size: 13px;
}

.fsb-entry {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 20px;
  cursor: pointer;
  transition: background 0.1s;
  user-select: none;

  &:hover {
    background: var(--bg-hover);
  }

  &.fsb-entry--checked {
    background: rgba(24, 144, 255, 0.06);
  }
}

.fsb-entry-check {
  flex-shrink: 0;
}

.fsb-entry-check-placeholder {
  width: 14px; // match el-checkbox size
  flex-shrink: 0;
}

.fsb-entry--parent {
  font-weight: 600;
  border-bottom: 1px solid var(--border-color);
}

.fsb-entry-name {
  flex: 1;
  font-size: 13px;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.fsb-entry-type-tag {
  font-size: 10px;
  color: var(--text-tertiary);
  background: var(--bg-hover);
  padding: 1px 6px;
  border-radius: 3px;
  flex-shrink: 0;
}

// ── Icons ──
.entry-icon--folder {
  color: #faad14;
  flex-shrink: 0;
}

.entry-icon--file {
  color: var(--text-secondary);
  flex-shrink: 0;
}

// ── Path bar (bottom) ──
.fsb-path-bar {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 20px;
  border-top: 1px solid var(--border-color);

  .fsb-path-input {
    flex: 1;
  }

  .fsb-path-icon {
    color: var(--text-secondary);
  }
}

// ── Footer ──
.fsb-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 20px;
  border-top: 1px solid var(--border-color);
}

.fsb-footer-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.fsb-selected-count {
  font-size: 12px;
  color: var(--color-primary);

  &--empty {
    color: var(--text-tertiary);
  }
}

.fsb-footer-actions {
  display: flex;
  gap: 8px;
}
</style>

<!-- append-to-body：弹层挂到 body，用非 scoped 保证样式生效，且仅限 .fsb-dialog -->
<style lang="scss">
html body .fsb-dialog .el-input__wrapper,
html body .el-dialog.fsb-dialog .el-input__wrapper {
  background: transparent !important;
  background-color: transparent !important;
}

/* 搜索框始终透明：去掉悬浮背景色（与图2一致） */
html body .fsb-dialog .fsb-search .el-input__wrapper,
html body .el-dialog.fsb-dialog .fsb-search .el-input__wrapper,
html body .fsb-dialog .fsb-search .el-input__wrapper:hover,
html body .el-dialog.fsb-dialog .fsb-search .el-input__wrapper:hover,
html body .fsb-dialog .fsb-search.el-input .el-input__wrapper:hover,
html body .el-dialog.fsb-dialog .fsb-search.el-input .el-input__wrapper:hover,
html body .fsb-dialog .fsb-search .el-input__wrapper.is-focus,
html body .el-dialog.fsb-dialog .fsb-search .el-input__wrapper.is-focus,
html body .fsb-dialog .fsb-search .el-input__wrapper.is-focus:hover,
html body .el-dialog.fsb-dialog .fsb-search .el-input__wrapper.is-focus:hover,
html body .fsb-dialog .fsb-search.el-input.is-focus .el-input__wrapper,
html body .el-dialog.fsb-dialog .fsb-search.el-input.is-focus .el-input__wrapper,
html body .fsb-dialog .fsb-search.el-input.is-focus .el-input__wrapper:hover,
html body .el-dialog.fsb-dialog .fsb-search.el-input.is-focus .el-input__wrapper:hover {
  background: transparent !important;
  background-color: transparent !important;
}

/* 仅红框搜索框右侧清空按钮：复用 Skills 管理 skills-search-clear 样式 */
html body .fsb-dialog .fsb-search button.skills-search-clear,
html body .el-dialog.fsb-dialog .fsb-search button.skills-search-clear {
  padding: 6px;
  border-radius: 8px;
  background: transparent;
  color: var(--text-secondary);
}

html body .fsb-dialog .fsb-search button.skills-search-clear:hover,
html body .el-dialog.fsb-dialog .fsb-search button.skills-search-clear:hover {
  background: var(--dropdown-hover-color) !important;
  background-color: var(--dropdown-hover-color) !important;
  color: var(--text-secondary);
}

/* default / small / primary 按钮高度与 el-input__wrapper（32px）一致 */
html body .fsb-dialog .el-button.el-button--default:not(.is-link),
html body .el-dialog.fsb-dialog .el-button.el-button--default:not(.is-link),
html body .fsb-dialog .el-button.el-button--small:not(.is-link),
html body .el-dialog.fsb-dialog .el-button.el-button--small:not(.is-link),
html body .fsb-dialog .el-button.el-button--primary:not(.is-link),
html body .el-dialog.fsb-dialog .el-button.el-button--primary:not(.is-link) {
  height: 32px !important;
  min-height: 32px !important;
  box-sizing: border-box !important;
}

/* default / small 按钮悬浮（不含 primary / 取消） */
html body .fsb-dialog .el-button.el-button--default:not(.el-button--primary):not(.fsb-btn-cancel):hover,
html body .el-dialog.fsb-dialog .el-button.el-button--default:not(.el-button--primary):not(.fsb-btn-cancel):hover,
html body .fsb-dialog .el-button.el-button--small:not(.el-button--primary):not(.is-link):not(.fsb-btn-cancel):hover,
html body .el-dialog.fsb-dialog .el-button.el-button--small:not(.el-button--primary):not(.is-link):not(.fsb-btn-cancel):hover {
  background: var(--dropdown-hover-color) !important;
  background-color: var(--dropdown-hover-color) !important;
  border-color: var(--border-color) !important;
  color: var(--text-primary) !important;
}

/* 取消按钮：与主题选择弹窗 .theme-action--secondary 一致 */
html body .fsb-dialog .fsb-btn-cancel.el-button,
html body .el-dialog.fsb-dialog .fsb-btn-cancel.el-button {
  min-width: 76px !important;
  color: var(--btn-secondary-text) !important;
  background: var(--cancel-btn-color) !important;
  background-color: var(--cancel-btn-color) !important;
  border-color: var(--btn-secondary-border) !important;
  border-radius: 4px !important;
  font-weight: 600 !important;
}

html body .fsb-dialog .fsb-btn-cancel.el-button:hover,
html body .el-dialog.fsb-dialog .fsb-btn-cancel.el-button:hover {
  color: var(--btn-secondary-hover-text) !important;
  background: var(--cancel-btn-hover-color) !important;
  background-color: var(--cancel-btn-hover-color) !important;
  border-color: var(--btn-secondary-hover-border) !important;
}

/* 关闭按钮：与主题选择弹窗 .theme-modal__close 一致 */
html body .fsb-dialog .el-dialog__headerbtn,
html body .el-dialog.fsb-dialog .el-dialog__headerbtn {
  width: 32px !important;
  height: 32px !important;
  top: 16px !important;
  right: 16px !important;
  display: inline-flex !important;
  align-items: center !important;
  justify-content: center !important;
  border: none !important;
  background: transparent !important;
  border-radius: 8px !important;
  padding: 0 !important;
  box-shadow: none !important;
  outline: none !important;
  transition: all 0.18s ease;
}

html body .fsb-dialog .el-dialog__headerbtn:hover,
html body .el-dialog.fsb-dialog .el-dialog__headerbtn:hover {
  background: var(--bg-hover) !important;
  background-color: var(--bg-hover) !important;
}

html body .fsb-dialog .el-dialog__headerbtn .el-dialog__close,
html body .el-dialog.fsb-dialog .el-dialog__headerbtn .el-dialog__close {
  color: var(--text-secondary) !important;
  font-size: 18px !important;
}

html body .fsb-dialog .el-dialog__headerbtn:hover .el-dialog__close,
html body .el-dialog.fsb-dialog .el-dialog__headerbtn:hover .el-dialog__close {
  color: var(--text-primary) !important;
}
</style>
