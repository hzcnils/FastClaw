<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, onDeactivated, watch, nextTick } from 'vue'
import { ElMessage } from 'element-plus'
import { useKnowledgeBaseStore } from '@/stores/index'
import { FolderAdd, DocumentAdd } from '@element-plus/icons-vue';
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { kgStarting, kgStartingPhase, kgServiceReady } from '@/api/knowledge'
import type { LocalKnowledgeBase, LocalKbSource } from '@/types/knowledge'
import FilesystemBrowserModal from '@/components/chat/FilesystemBrowserModal.vue'
import type { FilesystemSelection } from '@/components/chat/FilesystemBrowserModal.vue'

const selectedIds = defineModel<string[]>({ default: () => [] })
const props = withDefaults(defineProps<{
  disabled?: boolean
  appearance?: 'select' | 'chip'
}>(), {
  disabled: false,
  appearance: 'select',
})
const kbStore = useKnowledgeBaseStore()
const router = useRouter()
const { t } = useI18n()

const searchStr = ref('')
const kbRenameRef = ref<HTMLInputElement[] | null>(null);

const filteredList = computed(() => {
  const all = kbStore.allKnowledgeBases
  if (!searchStr.value) return all
  const q = searchStr.value.toLowerCase()
  return all.filter(item => {
    const name = item.name || ''
    const code = '_type' in item && item._type === 'remote' ? (item as any).code || '' : ''
    return name.toLowerCase().includes(q) || (code && code.toLowerCase().includes(q))
  })
})

function handleChange(ids: string[]) {
  selectedIds.value = ids
}

function toggleKbItem(id: string) {
  if (props.disabled || kbStore.localKnowledgeBases.some(kb => kb.id === id && !kb.enabled)) return
  const idx = selectedIds.value.indexOf(id)
  if (idx >= 0) {
    selectedIds.value = selectedIds.value.filter(s => s !== id)
  } else {
    selectedIds.value = [...selectedIds.value, id]
  }
}

function goToKnowledge() {
  router.push('/knowledge')
}

// ─── 本地知识库操作 ───────────────────────────

// 文件浏览器弹窗
const filesystemBrowserVisible = ref(false)
const filesystemBrowserForKbId = ref<string | null>(null) // null=新建KB, string=追加到已有KB

function openAddFolder() {
  if (props.disabled) return
  filesystemBrowserForKbId.value = null
  filesystemBrowserVisible.value = true
}

// function openAddFile() {
//   filesystemBrowserForKbId.value = null
//   filesystemBrowserVisible.value = true
// }

function openAddSourceForKb(kbId: string) {
  if (props.disabled) return
  closeContextMenu()
  filesystemBrowserForKbId.value = kbId
  filesystemBrowserVisible.value = true
}

function onFilesystemConfirm(selections: FilesystemSelection[]) {
  if (props.disabled) return
  filesystemBrowserVisible.value = false
  if (selections.length === 0) return

  const targetKbId = filesystemBrowserForKbId.value
  if (targetKbId) {
    // 追加到已有 KB
    kbStore.addSourcesToLocalKb(targetKbId, selections)
    // 确保该 KB 处于选中状态，方便直接问答
    if (!selectedIds.value.includes(targetKbId)) {
      selectedIds.value = [...selectedIds.value, targetKbId]
    }
  } else {
    // 创建新 KB
    const newKb = kbStore.createLocalKnowledgeBase('', selections)
    // 自动选中刚创建的本地知识库，方便用户直接进行问答
    if (newKb && !selectedIds.value.includes(newKb.id)) {
      selectedIds.value = [...selectedIds.value, newKb.id]
    }
  }
  // 刷新状态
  kbStore.validateLocalKbs()
}

function removeLocalKb(id: string) {
  kbStore.removeLocalKnowledgeBase(id)
  // 从已选中列表中移除
  selectedIds.value = selectedIds.value.filter(sid => sid !== id)
}

function toggleEnabled(id: string) {
  const kb = kbStore.localKnowledgeBases.find(k => k.id === id)
  if (kb) {
    kbStore.updateLocalKnowledgeBase(id, { enabled: !kb.enabled })
  }
}

function toggleRecursive(kbId: string, sourcePath: string) {
  const kb = kbStore.localKnowledgeBases.find(k => k.id === kbId)
  if (!kb) return
  const src = kb.sources.find(s => s.path === sourcePath)
  if (src && src.entryType === 'directory') {
    kbStore.updateLocalKbSource(kbId, sourcePath, { recursive: !src.recursive })
    kbStore.validateLocalKbs()
  }
}

async function openLocalFolder(path: string) {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('open_local_source_folder', { path })
  } catch (e) {
    ElMessage.error(t('fileDrop.openFailed') + ': ' + String(e))
  }
}

// ─── 重命名 ───────────────────────────

const editingNameId = ref<string | null>(null)
const editingNameValue = ref('')

function startRename(id: string, currentName: string) {
  editingNameId.value = id
  editingNameValue.value = currentName
  nextTick(() => {
    const input = kbRenameRef.value?.[0];
    if (input) {
      (input as HTMLInputElement).focus()
    }
  })
}

function finishRename(id: string) {
  const name = editingNameValue.value.trim()
  if (name) {
    kbStore.updateLocalKnowledgeBase(id, { name })
  }
  editingNameId.value = null
}

function cancelRename() {
  editingNameId.value = null
}

function onRenameInputFocus(e: FocusEvent) {
  (e.target as HTMLInputElement).select()
}

// ─── 右键菜单 ───────────────────────────

const ctxMenuVisible = ref(false)
const ctxMenuTarget = ref<LocalKnowledgeBase | null>(null)
const ctxMenuX = ref(0)
const ctxMenuY = ref(0)
const ctxMenuRef = ref<HTMLElement | null>(null)

function handleContextMenu(e: MouseEvent, kb: LocalKnowledgeBase) {
  if (props.disabled) return
  e.preventDefault()
  ctxMenuTarget.value = kbStore.localKnowledgeBases.find(item => item.id === kb.id) || kb
  ctxMenuX.value = e.clientX
  ctxMenuY.value = e.clientY
  ctxMenuVisible.value = true
  nextTick(() => adjustMenuPosition())
}

function adjustMenuPosition() {
  const menu = ctxMenuRef.value
  if (!menu) return
  const rect = menu.getBoundingClientRect()
  const vw = window.innerWidth
  const vh = window.innerHeight
  const pad = 4

  if (rect.right > vw) {
    ctxMenuX.value = Math.max(pad, vw - rect.width - pad)
  }
  if (rect.bottom > vh) {
    ctxMenuY.value = Math.max(pad, vh - rect.height - pad)
  }
}

function closeContextMenu() {
  ctxMenuVisible.value = false
  ctxMenuTarget.value = null
}

function onLocalItemMouseDown(e: MouseEvent) {
  // 阻止右键 mousedown 冒泡，避免 el-select popper 被关闭
  if (e.button === 2) e.preventDefault()
}

function ctxOpenFolder() {
  if (ctxMenuTarget.value && ctxMenuTarget.value.sources[0]) {
    openLocalFolder(ctxMenuTarget.value.sources[0].path)
  }
  closeContextMenu()
}

function ctxRefreshStats() {
  cachedFileLists.value = {}
  if (ctxMenuTarget.value) kbStore.validateLocalKbs()
}

function ctxEditName() {
  if (ctxMenuTarget.value) startRename(ctxMenuTarget.value.id, ctxMenuTarget.value.name)
  closeContextMenu()
}

function ctxRemoveKb() {
  if (ctxMenuTarget.value) removeLocalKb(ctxMenuTarget.value.id)
  closeContextMenu()
}

function ctxToggleEnabled() {
  if (ctxMenuTarget.value) toggleEnabled(ctxMenuTarget.value.id)
}

function ctxToggleRecursive() {
  if (!ctxMenuTarget.value) return
  const dirs = ctxMenuTarget.value.sources.filter(s => s.entryType === 'directory')
  const recursive = !dirs.every(s => s.recursive)
  for (const source of dirs) kbStore.updateLocalKbSource(ctxMenuTarget.value.id, source.path, { recursive })
  void kbStore.validateLocalKbs()
}

// ─── 文件列表 Tooltip ───────────────────────────

const cachedFileLists = ref<Record<string, string[]>>({})
const loadingFileList = ref<Record<string, boolean>>({})

async function loadFileList(kb: LocalKnowledgeBase) {
  if (cachedFileLists.value[kb.id] || loadingFileList.value[kb.id]) return
  loadingFileList.value[kb.id] = true
  const signature = JSON.stringify(kb.sources)
  try {
    const files = await kbStore.getLocalKbFileList(kb.id)
    if (JSON.stringify(kb.sources) === signature) cachedFileLists.value[kb.id] = files
  } catch { /* Store exposes scan errors */ }
  finally { loadingFileList.value[kb.id] = false }
}

// ─── 下拉框打开时校验 ───────────────────────────

const kbSelectRef = ref<any>(null)
const menuOpen = ref(false)
function open() {
  if (props.disabled) return
  searchStr.value = ''
  if (!menuOpen.value) kbSelectRef.value?.toggleMenu?.()
}
function close() {
  closeContextMenu()
  filesystemBrowserVisible.value = false
  editingNameId.value = null
  kbSelectRef.value?.blur?.()
}
defineExpose({ open, close })
onDeactivated(close)
watch(() => props.disabled, disabled => { if (disabled) close() })
watch(() => kbStore.localKnowledgeBases.map(k => k.id), (ids, previous) => {
  const removed = new Set(previous.filter(id => !ids.includes(id)))
  if (removed.size) selectedIds.value = selectedIds.value.filter(id => !removed.has(id))
})
watch(() => JSON.stringify(kbStore.localKnowledgeBases.map(k => ({ id: k.id, sources: k.sources }))), () => {
  cachedFileLists.value = {}
  if (ctxMenuTarget.value) ctxMenuTarget.value = kbStore.localKnowledgeBases.find(k => k.id === ctxMenuTarget.value?.id) || null
  if (!ctxMenuTarget.value) ctxMenuVisible.value = false
})

async function handleVisibleChange(visible: boolean) {
  menuOpen.value = visible
  if (visible) cachedFileLists.value = {}
  if (visible) void kbStore.ensureKnowledgeBases()
  if (visible && kbStore.localKnowledgeBases.length > 0) {
    kbStore.validateLocalKbs()
  }
  // 右键菜单打开时，下拉框自动关闭 → 立即重新打开
  if (!visible && ctxMenuVisible.value) {
    await nextTick()
    kbSelectRef.value?.toggleMenu?.()
  }
}

// ─── 模板辅助（Vue 模板不支持 as 类型断言） ────────────────

function isLocal(item: any): item is LocalKnowledgeBase {
  return item._type === 'local'
}

function toLocal(item: any): LocalKnowledgeBase {
  return item as LocalKnowledgeBase
}

// 未安装KG但有本地KB时，在下拉框底部显示安装提示
const showInstallHint = computed(() =>
  !kgServiceReady.value && !kgStarting.value && kbStore.localKnowledgeBases.length > 0
)

// ─── 服务就绪后自动刷新列表 ───────────────────────────

function onDocumentClick() {
  if (ctxMenuVisible.value) closeContextMenu()
}

onMounted(() => {
  document.addEventListener('click', onDocumentClick)
})
onBeforeUnmount(() => {
  close()
  document.removeEventListener('click', onDocumentClick)
})
</script>

<template>
  <div class="kb-select-wrapper">
    <el-select ref="kbSelectRef" v-model="selectedIds" placement="top" multiple class="kb-selector"
      :class="{ 'kb-selector--chip': props.appearance === 'chip' }"
      :placeholder="props.appearance === 'chip' ? '' : $t('chat.selectKb')"
      :aria-label="$t('chat.selectKb')"
      :loading="kbStore.loading" popper-class="kb-selector-popper" :teleported="true" collapse-tags
      :popper-style="props.appearance === 'chip' ? { width: '300px', maxWidth: 'calc(100vw - 24px)' } : undefined"
      collapse-tags-tooltip
      :tag-tooltip="{ popperClass: 'kb-collapse-tags-tooltip' }"
      :disabled="props.disabled" @change="handleChange" @visible-change="handleVisibleChange">
      <template v-if="props.appearance === 'chip'" #prefix>
        <svg class="kb-chip-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true">
          <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2Z" />
        </svg>
      </template>
      <template v-if="props.appearance === 'chip'" #tag>
        <span class="kb-chip-label">{{ $t('document.knowledgeBase') }}</span>
        <span v-if="selectedIds.length" class="kb-chip-count">{{ selectedIds.length }}</span>
        <slot name="chip-extra" />
      </template>
      <el-option v-for="item in filteredList" :key="item.id" :label="item.name" :value="item.id"
        :disabled="isLocal(item) && !item.enabled">
        <!-- 远端知识库 -->
        <div v-if="item._type === 'remote'" class="kb-item">
          <el-checkbox
            :model-value="selectedIds.includes(item.id)"
            class="kb-checkbox"
            @click.stop="toggleKbItem(item.id)"
          />
          <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"
            class="kb-item-icon kb-icon-remote">
            <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"></path>
            <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"></path>
          </svg>
          <span class="kb-item-name" :title="item.name">{{ item.name }}</span>
          <span class="kb-item-meta">{{ item.file_count }} {{ $t('knowledge.files') }} · {{ item.chunk_count }} {{
            $t('knowledge.chunks') }}</span>
        </div>
        <!-- 本地知识库 -->
        <div v-else class="kb-item kb-item-local" @contextmenu.prevent.stop="handleContextMenu($event, toLocal(item))"
          @mousedown="onLocalItemMouseDown">
          <el-checkbox
            :model-value="selectedIds.includes(item.id)"
            :disabled="!item.enabled"
            class="kb-checkbox"
            @click.stop="toggleKbItem(item.id)"
          />
          <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"
            class="kb-item-icon kb-icon-local">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
          </svg>
          <!-- 名称 / 编辑 -->
          <span v-if="editingNameId !== item.id" class="kb-item-name" @dblclick.stop="startRename(item.id, item.name)"
            :title="$t('chat.editLocalKbName')">{{ item.name }}</span>
          <input v-else ref="kbRenameRef" v-model="editingNameValue" class="kb-rename-input" @blur="finishRename(item.id)"
            @keydown.enter="finishRename(item.id)" @keydown.escape="cancelRename()" @click.stop
            @focus="onRenameInputFocus" />
          <!-- 失效标记 -->
          <el-tooltip v-if="item.exists === false" :content="$t('chat.localKbNotExists')" placement="top">
            <el-icon>
              <svg viewBox="0 0 24 24" width="12" height="12" stroke="#e6a23c" stroke-width="2" fill="none"
                class="kb-warn-icon">
                <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
                <line x1="12" y1="9" x2="12" y2="13" />
                <line x1="12" y1="17" x2="12.01" y2="17" />
              </svg>
            </el-icon>
          </el-tooltip>
          <!-- sources 摘要 -->
          <span class="kb-item-meta">{{ item.sources?.length || 0 }} {{ $t('chat.localKbSources') }}</span>
          <!-- 文件数量 / hover 列表 -->
          <el-tooltip placement="top" :show-after="300" :hide-after="0" @show="loadFileList(toLocal(item))">
            <template #content>
              <div v-if="loadingFileList[item.id]" class="kb-file-tooltip-loading">...</div>
              <div v-else class="kb-file-tooltip">
                <div v-for="file in (cachedFileLists[item.id] || []).slice(0, 20)" :key="file">• {{ file }}</div>
                <div v-if="!cachedFileLists[item.id]?.length">{{ $t('chat.noneKB') }}</div>
                <div v-if="(cachedFileLists[item.id]?.length || 0) > 20">{{ $t('common.more') }} {{ cachedFileLists[item.id].length - 20 }}</div>
                <div v-if="kbStore.localKbScanStates[item.id]?.truncated">{{ $t('fileDrop.scanLimit') }}</div>
                <div v-for="error in kbStore.localKbScanStates[item.id]?.errors" :key="error">{{ error }}</div>
              </div>
            </template>
            <span class="kb-item-files">
              {{ kbStore.localKbScanStates[item.id]?.loading ? '...' : (item.fileCount ?? '...') }}{{ kbStore.localKbScanStates[item.id]?.truncated ? '+' : '' }} {{ $t('chat.localKbFiles') }}
              <span v-if="kbStore.localKbScanStates[item.id]?.errors.length">⚠</span>
            </span>
          </el-tooltip>
          <!-- 每个 source 的递归开关（仅目录类型） -->
          <template v-for="src in toLocal(item).sources" :key="src.path">
            <el-tooltip
              v-if="src.entryType === 'directory'"
              :content="src.path"
              placement="top"
            >
              <span
                class="kb-recursive-toggle"
                :class="{ active: src.recursive }"
                @click.stop="toggleRecursive(item.id, src.path)"
                :title="$t('chat.localKbRecursive')"
              >
                <svg viewBox="0 0 24 24" width="11" height="11" stroke="currentColor" stroke-width="2" fill="none">
                  <polyline points="17 1 21 5 17 9" />
                  <path d="M3 11V9a4 4 0 0 1 4-4h14" />
                  <polyline points="7 23 3 19 7 15" />
                  <path d="M21 13v2a4 4 0 0 1-4 4H3" />
                </svg>
              </span>
            </el-tooltip>
          </template>
        </div>
      </el-option>
      <!-- 未安装KG但有本地KB时，下拉框底部安装提示 -->
      <el-option v-if="showInstallHint" value="__hint__" disabled class="kb-install-hint-option">
        <div class="kb-hint-inline">
          <span>{{ $t('chat.kbNotInstalled') }}</span>
          <span class="kb-hint-link" @click.stop="goToKnowledge">{{ $t('chat.goInstallKb') }}</span>
        </div>
      </el-option>
      <template #header>
        <div class="kb-header-row">
          <el-input v-model="searchStr" :placeholder="$t('chat.searchKb')" clearable class="kb-search-input">
            <template #append>
              <el-button :disabled="props.disabled" @click="openAddFolder" :title="$t('chat.addLocalFolder')">
                <el-icon :size="17">
                  <FolderAdd />
                </el-icon>
              </el-button>
              <!-- <el-button :disabled="props.disabled" @click="openAddFile" :title="$t('chat.addLocalFile')">
                <el-icon :size="17">
                  <DocumentAdd />
                </el-icon>
              </el-button> -->
            </template>
          </el-input>
        </div>
      </template>
      <template #empty>
        <!-- 服务启动中 -->
        <div v-if="kgStarting" class="kb-empty">
          <div class="kb-empty-hint">{{ kgStartingPhase ? t(`knowledge.${kgStartingPhase}`) : t('chat.startingKb') }}
          </div>
          <div class="kb-empty-loading"><span class="mini-spinner"></span></div>
        </div>
        <!-- 服务未安装且无本地KB数据 -->
        <div v-else-if="!kgServiceReady && kbStore.localKnowledgeBases.length === 0" class="kb-empty">
          <div class="kb-empty-hint">{{ $t('chat.kbNotInstalled') }}</div>
          <div class="kb-empty-action" @click="goToKnowledge">{{ $t('chat.goInstallKb') }}</div>
        </div>
        <!-- 服务就绪但无数据 -->
        <div v-else-if="kgServiceReady" class="kb-empty">{{ $t('chat.noKb') }}</div>
        <!-- 服务未安装但有本地KB（正常显示在列表中，empty 不触发） -->
      </template>
    </el-select>

    <!-- 右键菜单 (teleported to body) -->
    <Teleport to="body">
      <div v-if="ctxMenuVisible" ref="ctxMenuRef" class="kb-ctx-menu" :style="{ left: ctxMenuX + 'px', top: ctxMenuY + 'px' }"
        @click.stop @mouseleave="closeContextMenu">
        <div class="kb-ctx-item" @click="ctxOpenFolder">
          <svg viewBox="0 0 24 24" width="13" height="13" stroke="currentColor" stroke-width="2" fill="none">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
          </svg>
          <span>{{ $t('chat.openLocalKbFolder') }}</span>
        </div>
        <div class="kb-ctx-item" @click="ctxRefreshStats">
          <svg viewBox="0 0 24 24" width="13" height="13" stroke="currentColor" stroke-width="2" fill="none">
            <polyline points="23 4 23 10 17 10" />
            <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" />
          </svg>
          <span>{{ $t('chat.refreshLocalKb') }}</span>
        </div>
        <div class="kb-ctx-item" @click="ctxEditName">
          <svg viewBox="0 0 24 24" width="13" height="13" stroke="currentColor" stroke-width="2" fill="none">
            <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
            <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
          </svg>
          <span>{{ $t('chat.editLocalKbName') }}</span>
        </div>
        <div class="kb-ctx-divider" />
        <div class="kb-ctx-item" @click="ctxMenuTarget && openAddSourceForKb(ctxMenuTarget.id)">
          <svg viewBox="0 0 24 24" width="13" height="13" stroke="currentColor" stroke-width="2" fill="none">
            <line x1="12" y1="5" x2="12" y2="19" />
            <line x1="5" y1="12" x2="19" y2="12" />
          </svg>
          <span>{{ $t('chat.addLocalKbSource') }}</span>
        </div>
        <div class="kb-ctx-item" @click="ctxToggleEnabled">
          <svg viewBox="0 0 24 24" width="13" height="13" stroke="currentColor" stroke-width="2" fill="none">
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="6" x2="12" y2="12" />
            <line x1="12" y1="16" x2="12.01" y2="16" />
          </svg>
          <span>{{ ctxMenuTarget?.enabled ? $t('chat.disable') : $t('chat.enable') }}{{ $t('chat.localKb') }}</span>
        </div>
        <div v-if="ctxMenuTarget?.sources.some(s => s.entryType === 'directory')" class="kb-ctx-item" @click="ctxToggleRecursive">
          <svg viewBox="0 0 24 24" width="13" height="13" stroke="currentColor" stroke-width="2" fill="none">
            <polyline points="17 1 21 5 17 9" />
            <path d="M3 11V9a4 4 0 0 1 4-4h14" />
            <polyline points="7 23 3 19 7 15" />
            <path d="M21 13v2a4 4 0 0 1-4 4H3" />
          </svg>
          <span>{{ $t('chat.localKbRecursive') }}</span>
          <svg v-if="ctxMenuTarget?.sources.filter(s => s.entryType === 'directory').every(s => s.recursive)" viewBox="0 0 24 24" width="12" height="12" stroke="#52c41a"
            stroke-width="3" fill="none" style="margin-left:auto">
            <polyline points="20 6 9 17 4 12" />
          </svg>
        </div>
        <div class="kb-ctx-divider" />
        <div class="kb-ctx-item kb-ctx-danger" @click="ctxRemoveKb">
          <svg viewBox="0 0 24 24" width="13" height="13" stroke="currentColor" stroke-width="2" fill="none">
            <polyline points="3 6 5 6 21 6" />
            <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
          </svg>
          <span>{{ $t('chat.removeLocalKb') }}</span>
        </div>
      </div>
    </Teleport>

    <!-- 文件系统浏览器弹窗 -->
    <FilesystemBrowserModal
      v-model="filesystemBrowserVisible"
      @confirm="onFilesystemConfirm"
    />
  </div>
</template>

<style lang="scss">
/* +N 折叠标签悬浮提示（teleport 到 body）内的 tag 样式 */
.kb-collapse-tags-tooltip .el-tag,
.kb-collapse-tags-tooltip .el-tag.el-tag--info,
.kb-collapse-tags-tooltip .el-tag.el-tag--info.el-tag--light,
.kb-collapse-tags-tooltip .el-tag.is-closable.el-tag--info.el-tag--light {
  --el-tag-bg-color: var(--card-background-tmd) !important;
  --el-tag-border-color: var(--border-color) !important;
  --el-tag-text-color: var(--text-secondary) !important;
  --el-tag-hover-color: var(--text-secondary) !important;
  background: var(--card-background-tmd) !important;
  background-color: var(--card-background-tmd) !important;
  border-color: var(--border-color) !important;
  color: var(--text-secondary) !important;
}

.kb-collapse-tags-tooltip .el-tag .el-tag__close,
.kb-collapse-tags-tooltip .el-tag .el-icon,
.kb-collapse-tags-tooltip .el-tag svg {
  color: var(--text-secondary) !important;
  fill: currentColor !important;
}

.kb-collapse-tags-tooltip .el-tag .el-tag__close:hover {
  background-color: transparent !important;
  color: var(--text-primary) !important;
}

.kb-selector-popper {
  border-radius: 8px !important;

  .el-input-group__append .el-button,
  .el-input-group__append .el-button .el-icon,
  .el-input-group__append .el-button svg {
    color: var(--text-secondary) !important;
  }

  .el-select-dropdown__item {
    font-size: 13px;
    padding: 4px 8px;

    // 隐藏 el-option 默认右侧对勾（使用自定义复选框替代）
    &.is-selected,
    &.is-checked {
      &::after {
        display: none !important;
      }
      .el-select-dropdown__item-icon {
        display: none !important;
      }
    }
  }

  .el-select-dropdown__empty {
    font-size: 13px;
    padding: 12px;
  }

  .el-select-dropdown__search {
    .el-input__wrapper {
      box-shadow: none !important;
    }
  }
}

/* 右键菜单全局样式（Teleported to body） */
.kb-ctx-menu {
  position: fixed;
  z-index: 9999;
  background: var(--bg-submenu, var(--bg-primary));
  border: 1px solid var(--border-color);
  border-radius: 6px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  padding: 4px 0;
  min-width: 160px;
}

.kb-ctx-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  font-size: 12px;
  color: var(--text-primary);
  cursor: pointer;
  transition: background 0.15s;

  &:hover {
    background: var(--bg-hover);
  }
}

.kb-ctx-danger {
  color: #e74c3c;

  &:hover {
    background: rgba(231, 76, 60, 0.08);
  }
}

.kb-ctx-divider {
  height: 1px;
  background: var(--border-color);
  margin: 4px 0;
}

/* 下拉框内安装提示（禁用选项样式） */
.kb-install-hint-option {
  border-top: 1px solid var(--border-color) !important;
  margin-top: 4px !important;
  padding-top: 8px !important;
  cursor: default !important;
  opacity: 1 !important;
}

.kb-hint-inline {
  height: 100%;
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--text-secondary);
  pointer-events: auto;
}

.kb-hint-link {
  color: var(--color-primary);
  cursor: pointer;

  &:hover {
    text-decoration: underline;
  }
}
</style>

<style lang="scss" scoped>
.kb-select-wrapper {
  position: relative;
  display: inline-flex;
  align-items: center;
}

.kb-selector {
  width: 200px;

  :deep(.el-select__wrapper .el-tag),
  :deep(.el-select__wrapper .el-tag.el-tag--info),
  :deep(.el-select__wrapper .el-tag.el-tag--info.el-tag--light),
  :deep(.el-select__wrapper .el-tag.is-closable.el-tag--info.el-tag--light) {
    --el-tag-border-radius: 8px;
    --el-tag-bg-color: var(--card-background-tmd) !important;
    --el-tag-border-color: var(--border-color) !important;
    --el-tag-text-color: var(--text-secondary) !important;
    --el-tag-hover-color: var(--text-secondary) !important;
    background: var(--card-background-tmd) !important;
    background-color: var(--card-background-tmd) !important;
    border-color: var(--border-color) !important;
    color: var(--text-secondary) !important;
  }

  :deep(.el-select__wrapper .el-tag .el-tag__close),
  :deep(.el-select__wrapper .el-tag .el-icon),
  :deep(.el-select__wrapper .el-tag svg) {
    color: var(--text-secondary) !important;
    fill: currentColor !important;
  }

  :deep(.el-select__wrapper .el-tag .el-tag__close:hover) {
    background-color: transparent !important;
    color: var(--text-primary) !important;
  }
}

.kb-selector--chip {
  width: auto;
  flex-shrink: 0;

  :deep(.el-select__wrapper) {
    min-height: 28px;
    padding: 4px 10px;
    gap: 6px;
    border: 1px solid var(--border-color) !important;
    border-radius: 14px;
    background: transparent !important;
    background-color: transparent !important;
    box-shadow: none !important;
    color: inherit;
    font-size: var(--font-size-sm, 14px);
    line-height: 20px;
  }

  :deep(.el-select__wrapper.is-hovering:not(.is-disabled)) {
    background: var(--dropdown-hover-color, #E5E6EB);
  }

  :deep(.el-select__wrapper.is-focused) {
    box-shadow: 0 0 0 1px var(--color-primary) inset;
  }

  :deep(.el-select__selection) {
    flex: 0 0 auto;
    flex-wrap: nowrap;
    gap: 5px;
  }

  :deep(.el-select__prefix),
  :deep(.el-select__caret) {
    color: inherit;
  }

  :deep(.el-select__caret) { font-size: 12px; }
  :deep(.el-select__wrapper.is-disabled) { opacity: 0.5; }
}

.kb-chip-icon { width: 14px; height: 14px; flex-shrink: 0; }
.kb-chip-label { white-space: nowrap; }
.kb-chip-count {
  min-width: 16px;
  height: 16px;
  padding: 0 4px;
  border-radius: 8px;
  background: var(--color-primary);
  color: var(--btn-primary-text, #fff);
  font-size: 11px;
  line-height: 16px;
  text-align: center;
}

:global(html.dark .kb-selector--chip .el-select__wrapper),
:global(html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .kb-selector--chip .el-select__wrapper),
:global(html.light[data-theme-id="custom-scene"] .kb-selector--chip .el-select__wrapper) {
  background: transparent !important;
  background-color: transparent !important;
  border: 1px solid var(--border-color) !important;
  box-shadow: none !important;
}

.kb-item {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  gap: 8px;
  overflow: hidden;
  padding-right: 10px; // 缩小右侧留白（默认对勾已隐藏）

  // 自定义复选框（替代默认对勾）
  .kb-checkbox {
    flex-shrink: 0;
    margin-right: 0;

    :deep(.el-checkbox__input) {
      // 缩小复选框尺寸使其与图标协调
      .el-checkbox__inner {
        width: 15px;
        height: 15px;
      }
    }

    :deep(.el-checkbox__label) {
      display: none; // 只显示复选框，不显示 label
    }
  }

  .kb-item-icon {
    flex-shrink: 0;
  }

  .kb-icon-remote {
    color: #52c41a;
  }

  .kb-icon-local {
    color: #faad14;
  }

  .kb-item-name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100px;
  }

  .kb-item-meta {
    font-size: 11px;
    color: var(--text-secondary);
    flex-shrink: 0;
    margin-left: auto;
  }

  .kb-item-files {
    font-size: 11px;
    color: var(--text-secondary);
    flex-shrink: 0;
    cursor: default;
  }

  .kb-warn-icon {
    flex-shrink: 0;
  }

  .kb-recursive-toggle {
    flex-shrink: 0;
    cursor: pointer;
    color: var(--text-secondary);
    opacity: 0.4;
    transition: all 0.15s;
    padding: 2px;

    &.active {
      color: var(--color-primary);
      opacity: 1;
    }

    &:hover {
      opacity: 1;
    }
  }

  .kb-rename-input {
    flex: 1;
    height: 20px;
    font-size: 13px;
    border: 1px solid var(--color-primary);
    border-radius: 3px;
    padding: 0 4px;
    background: var(--bg-primary);
    color: var(--text-primary);
    outline: none;
    min-width: 60px;
  }
}

.kb-item-local {
  cursor: default;
}

/* header 区域 */
.kb-header-row {
  display: flex;
  align-items: center;
  gap: 6px;

  .kb-search-input {
    flex: 1;
  }

  .btn-add-kb {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    transition: all 0.15s;
    flex-shrink: 0;

    &:hover {
      background: var(--bg-hover);
      color: var(--color-primary);
    }
  }
}

/* empty 状态 */
.kb-empty {
  text-align: center;
  color: var(--text-secondary);
  padding: 20px;
  font-size: 13px;
}

.kb-empty-hint {
  margin-bottom: 8px;
}

.kb-empty-action {
  color: var(--color-primary);
  cursor: pointer;
  font-size: 12px;

  &:hover {
    text-decoration: underline;
  }
}

.kb-empty-loading {
  margin-top: 6px;
}

.mini-spinner {
  display: inline-block;
  width: 14px;
  height: 14px;
  border: 2px solid var(--border-color);
  border-top-color: var(--color-primary);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}


/* tooltip 文件列表样式 */
.kb-file-tooltip {
  max-height: 260px;
  overflow-y: auto;
  font-size: 11px;
  line-height: 1.6;
}

.kb-file-tooltip-loading {
  font-size: 12px;
  color: var(--text-secondary);
}
</style>
