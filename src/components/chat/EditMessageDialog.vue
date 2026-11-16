<script setup lang="ts">
import { ref, computed, shallowRef, watch, nextTick, onBeforeUnmount } from 'vue'
import { EditorState } from '@codemirror/state'
import { EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection } from '@codemirror/view'
import { history, defaultKeymap, historyKeymap, undo, redo } from '@codemirror/commands'
import { markdown } from '@codemirror/lang-markdown'
import { syntaxHighlighting, defaultHighlightStyle } from '@codemirror/language'
import {
  search, searchKeymap, setSearchQuery, SearchQuery,
  findNext, findPrevious, replaceNext, replaceAll,
} from '@codemirror/search'
import { Search, Code2, Eye, Copy, Check, X, Undo2, Redo2, Download } from 'lucide-vue-next'
import MarkdownIt from 'markdown-it'
import { useI18n } from 'vue-i18n'
import { exportMarkdown, exportWord, exportPdf } from '@/utils/exportMessage'
import { useMarkdown } from '@/composables/useMarkdown'
import { ElMessage, ElMessageBox } from 'element-plus'
const { t } = useI18n()

const props = defineProps<{ modelValue: boolean; content: string; draftContent?: string; loading?: boolean }>()
const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  (e: 'save', content: string): void
  (e: 'draft-change', content: string): void
}>()

const md = new MarkdownIt({ html: false, linkify: true, breaks: true })
const { render } = useMarkdown()

const text = ref('')
const original = ref('')
const dirty = computed(() => text.value !== original.value)
const charCount = computed(() => text.value.length)
const tokenCount = computed(() => {
  const cn = (text.value.match(/[一-龥]/g) || []).length
  return cn + Math.ceil((text.value.length - cn) / 4)
})

/* ---------- view: source / preview ---------- */
const view = ref<'edit' | 'preview'>('edit')
const previewHtml = computed(() => md.render(text.value || ''))

/* ---------- copy ---------- */
const copied = ref(false)
async function copyAll() {
  await navigator.clipboard.writeText(text.value)
  copied.value = true
  setTimeout(() => (copied.value = false), 1500)
}

/* ---------- export ---------- */
const exporting = ref(false)
async function doExport(format: string) {
  const content = text.value
  if (!content?.trim()) {
    ElMessage.warning(t('chat.emptyContentCannotExport'))
    return
  }
  exporting.value = true
  let loadingMsg: ReturnType<typeof ElMessage.info> | null = null
  try {
    if (format === 'md') {
      const ok = await exportMarkdown(content)
      if (ok) ElMessage.success(`Markdown ${t('chat.downloadSuccess')}`)
    } else if (format === 'docx') {
      loadingMsg = ElMessage.info({ message: t('chat.exportGenerating'), duration: 0 })
      const ok = await exportWord(content, render)
      loadingMsg.close()
      if (ok) ElMessage.success(`Word ${t('chat.downloadSuccess')}`)
    } else if (format === 'pdf') {
      loadingMsg = ElMessage.info({ message: t('chat.exportGenerating'), duration: 0 })
      const result = await exportPdf(content, render, (progress) => {
        if (progress.stage === 'render') {
          loadingMsg!.message = t('chat.exportRendering')
        } else {
          loadingMsg!.message = t('chat.exportSlicing', { current: progress.current, total: progress.total })
        }
      })
      loadingMsg.close()
      if (result.success) {
        ElMessage.success({ message: `PDF ${t('chat.downloadSuccess')}`, duration: 2000 })
      } else if (result.canceled) {
        ElMessage.warning({ message: t('chat.exportCancelled'), duration: 2000 })
      } else {
        ElMessage.error({ message: t('chat.exportFailed', { error: result.error || 'Unknown error' }), duration: 3000 })
      }
    }
  } catch (err: any) {
    loadingMsg?.close()
    ElMessage.error({ message: t('chat.exportFailed', { error: err?.message || String(err) }), duration: 3000 })
  } finally {
    exporting.value = false
  }
}

/* ---------- CodeMirror ---------- */
const host = ref<HTMLElement | null>(null)
const cm = shallowRef<EditorView | null>(null)

const cmTheme = EditorView.theme({
  '&': { fontSize: '13px', backgroundColor: 'transparent', height: '100%', color: 'var(--text-primary)' },
  '.cm-scroller': { fontFamily: 'ui-monospace,SFMono-Regular,Menlo,Consolas,monospace', lineHeight: '1.7' },
  // 光标颜色跟随正文，避免暗色主题白字黑光标看不清
  '.cm-content': { padding: '14px 4px', caretColor: 'var(--text-primary)' },
  '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--text-primary)' },
  '.cm-gutters': { background: 'var(--bg-primary)', border: 'none', color: 'var(--text-secondary)' },
  '.cm-activeLine': { background: 'var(--dropdown-hover-color)' },
  '.cm-activeLineGutter': { background: 'transparent' },
  '&.cm-focused': { outline: 'none' },
  '.cm-selectionBackground, ::selection': { background: 'rgba(79,107,237,.18) !important' },
  '.cm-searchMatch': { background: '#fff3bf', borderRadius: '3px' },
  '.cm-searchMatch-selected': { background: '#ffd43b' },
})

function mountEditor() {
  if (!host.value) return
  const state = EditorState.create({
    doc: text.value,
    extensions: [
      lineNumbers(), highlightActiveLine(), drawSelection(),
      history(),
      markdown(),
      syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
      search({ top: true }),
      keymap.of([...defaultKeymap, ...historyKeymap, ...searchKeymap]),
      EditorView.lineWrapping,
      cmTheme,
      EditorView.updateListener.of((u) => {
        if (u.docChanged) {
          text.value = u.state.doc.toString()
          emit('draft-change', text.value)
        }
      }),
    ],
  })
  cm.value = new EditorView({ state, parent: host.value })
}
function destroyEditor() { cm.value?.destroy(); cm.value = null }

// A lazily mounted, already-open el-dialog creates its body after the first tick.
// Follow the real host instead of assuming that a fixed number of ticks is enough.
watch(host, element => {
  if (element && props.modelValue && !cm.value) mountEditor()
}, { flush: 'post' })

/* ---------- find / replace bar ---------- */
const showFind = ref(false)
const findText = ref('')
const replaceText = ref('')
const matchCount = computed(() => {
  if (!findText.value) return 0
  try { return (text.value.match(new RegExp(findText.value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'g')) || []).length }
  catch { return 0 }
})
function syncQuery() {
  if (!cm.value) return
  const q = new SearchQuery({ search: findText.value, replace: replaceText.value, caseSensitive: false })
  cm.value.dispatch({ effects: setSearchQuery.of(q) })
}
watch([findText, replaceText], syncQuery)
function next() { cm.value && (findNext(cm.value), cm.value.focus()) }
function prev() { cm.value && (findPrevious(cm.value), cm.value.focus()) }
function repl() { cm.value && (replaceNext(cm.value), cm.value.focus()) }
function replAll() { cm.value && (replaceAll(cm.value), cm.value.focus()) }
function toggleFind() {
  showFind.value = !showFind.value
  if (showFind.value) nextTick(syncQuery)
}

/* ---------- undo / redo ---------- */
function doUndo() { cm.value && (undo(cm.value), cm.value.focus()) }
function doRedo() { cm.value && (redo(cm.value), cm.value.focus()) }

/* ---------- open / close / save ---------- */
watch(() => props.modelValue, async (open, _previous, onCleanup) => {
  let canceled = false
  onCleanup(() => { canceled = true })
  if (open) {
    text.value = props.draftContent ?? props.content ?? ''
    original.value = props.content ?? ''
    view.value = 'edit'; showFind.value = false; copied.value = false
    await nextTick()
    if (canceled || !props.modelValue) return
    destroyEditor(); mountEditor()
  } else {
    destroyEditor()
  }
}, { immediate: true })
let disposed = false
async function close() {
  if (dirty.value) {
    try {
      await ElMessageBox.confirm(t('chat.unsavedConfirm'), t('common.tip'), {
        confirmButtonText: t('common.confirm'),
        cancelButtonText: t('common.cancel'),
        type: 'warning',
        customClass: 'custom-confirm-dialog',
      })
    } catch {
      return
    }
  }
  if (!disposed && props.modelValue) emit('update:modelValue', false)
}
function save() {
  if (disposed || !props.modelValue || props.loading) return
  if (!dirty.value) return emit('update:modelValue', false)
  emit('save', text.value)
}
// A submitted save may finish while the user continues editing. Only advance the saved baseline.
watch(() => props.content, content => { original.value = content ?? '' })
function onKeydown(e: KeyboardEvent) {
  if (!props.modelValue) return
  const meta = e.metaKey || e.ctrlKey
  if (meta && e.key === 'Enter') { e.preventDefault(); save() }
  else if (e.key === 'Escape') {
    if (showFind.value) { showFind.value = false; return }
    e.preventDefault(); close()
  } else if (meta && (e.key === 'f' || e.key === 'F')) { e.preventDefault(); toggleFind() }
}
window.addEventListener('keydown', onKeydown)
onBeforeUnmount(() => { disposed = true; window.removeEventListener('keydown', onKeydown); destroyEditor() })
</script>

<template>
  <el-dialog
    :model-value="modelValue"
    @update:model-value="(v: boolean) => v ? null : close()"
    :show-close="false" width="720px" align-center append-to-body
    class="qedit-dialog"
  >
    <template #header>
      <div class="qe-head">
        <span class="qe-title">{{ t('chat.editMessage') }}</span>
        <div class="qe-acts">
          <button class="qe-ib" :class="{ on: showFind }" @click="toggleFind" :title="t('chat.findReplace')"><Search :size="16" /></button>
          <button class="qe-ib" @click="doUndo" :title="t('chat.undo')"><Undo2 :size="16" /></button>
          <button class="qe-ib" @click="doRedo" :title="t('chat.redo')"><Redo2 :size="16" /></button>
          <span class="qe-sep" />
          <button class="qe-ib" :class="{ on: view === 'edit' }" @click="view = 'edit'" :title="t('chat.sourceCode')"><Code2 :size="16" /></button>
          <button class="qe-ib" :class="{ on: view === 'preview' }" @click="view = 'preview'" :title="t('chat.preview')"><Eye :size="16" /></button>
          <span class="qe-sep" />
          <button class="qe-ib" @click="copyAll" :title="copied ? t('chat.copied') : t('chat.copyAll')">
            <Check v-if="copied" :size="16" /><Copy v-else :size="16" />
          </button>
          <button class="qe-ib" @click="close" :title="t('common.close')"><X :size="16" /></button>
        </div>
      </div>
    </template>

    <!-- find / replace bar -->
    <div v-show="showFind && view === 'edit'" class="qe-find">
      <input v-model="findText" :placeholder="t('chat.find')" @keydown.enter.prevent="next" />
      <span class="qe-cnt">{{ matchCount }} {{ t('chat.matchCount') }}</span>
      <button class="qe-fb" @click="prev">{{ t('chat.prevMatch') }}</button>
      <button class="qe-fb" @click="next">{{ t('chat.nextMatch') }}</button>
      <input v-model="replaceText" :placeholder="t('chat.replaceWith')" class="qe-rep" />
      <button class="qe-fb" @click="repl">{{ t('chat.replaceWord') }}</button>
      <button class="qe-fb solid" @click="replAll">{{ t('chat.replaceAll') }}</button>
    </div>

    <!-- editor / preview -->
    <div class="qe-body">
      <div v-show="view === 'edit'" ref="host" class="qe-cm" />
      <div v-show="view === 'preview'" class="qe-preview md-body" v-html="previewHtml" />
    </div>

    <template #footer>
      <div class="qe-foot">
        <div class="qe-meta">
          <span>{{ charCount }} {{ t('chat.charCount') }}</span><span class="dot">·</span>
          <span>≈ {{ tokenCount }} tokens</span>
          <span v-if="dirty" class="qe-dirty">· 未保存</span>
        </div>
        <div class="qe-fr">
          <!-- 导出按钮组 -->
          <div class="qe-export-group">
            <button class="qe-ib" @click="doExport('md')" :disabled="exporting" :title="t('chat.downloadAsMD')">
              <Download :size="12" /><span class="qe-export-label">MD</span>
            </button>
            <button class="qe-ib" @click="doExport('docx')" :disabled="exporting" :title="t('chat.downloadAsDOCX')">
              <Download :size="12" /><span class="qe-export-label">DOCX</span>
            </button>
            <button class="qe-ib" @click="doExport('pdf')" :disabled="exporting" :title="t('chat.downloadAsPDF')">
              <Download :size="12" /><span class="qe-export-label">PDF</span>
            </button>
          </div>
          <span class="qe-kbd">{{ t('chat.editKbdHint') }}</span>
          <el-button size="small" class="qe-cancel-btn" @click="close">{{ t('common.cancel') }}</el-button>
          <el-button size="small" type="primary" class="qe-save-btn" :loading="loading" :disabled="!dirty" @click="save">{{ t('common.save') }}</el-button>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
.qe-head { display: flex; align-items: center; justify-content: space-between }
.qe-title { font-size: 16px; font-weight: 700; color: var(--text-primary) }
.qe-acts { display: flex; align-items: center; gap: 2px }
.qe-ib { width: 30px; height: 30px; display: flex; align-items: center; justify-content: center; border: none; background: transparent; border-radius: 8px; cursor: pointer; transition: all .15s }
.qe-ib:hover { background: var(--dropdown-hover-color);  }
.qe-ib.on { background: rgba(79,107,237,.12); color: var(--color-primary) }
.qe-sep { width: 1px; height: 16px; background: var(--border-color); margin: 0 5px }

.qe-find { display: flex; align-items: center; gap: 8px; margin-bottom: 10px; padding: 7px 10px; background: var(--bg-hover); border: 1px solid var(--border-color); border-radius: 10px }
.qe-find input { flex: 1; min-width: 0; border: 1px solid var(--border-color); background: var(--bg-primary); border-radius: 7px; padding: 6px 9px; font-size: 13px; color: var(--text-primary); outline: none }
.qe-find input:focus { border-color: var(--color-primary) }
.qe-find .qe-rep { flex: .8 }
.qe-cnt { font-size: 12px; color: var(--text-secondary); white-space: nowrap }
.qe-fb { font-size: 12px; color: var(--text-secondary); border: 1px solid var(--border-color); background: var(--bg-primary); border-radius: 7px; padding: 6px 10px; cursor: pointer; white-space: nowrap }
.qe-fb:hover { border-color: var(--color-primary); color: var(--color-primary) }
.qe-fb.solid { background: var(--color-primary); color: #fff; border-color: var(--color-primary) }

.qe-body { border: 1px solid var(--border-color); border-radius: 12px; overflow: hidden; height: 52vh }
.qe-cm { height: 100%; overflow: auto }
.qe-cm :deep(.cm-content) { caret-color: var(--text-primary); }
.qe-cm :deep(.cm-cursor),
.qe-cm :deep(.cm-dropCursor) { border-left-color: var(--text-primary) !important; }
.qe-preview { height: 100%; overflow: auto; padding: 16px 18px; color: var(--text-primary) }

.qe-foot { display: flex; align-items: center; justify-content: space-between; gap: 8px; min-width: 0 }
.qe-meta { font-size: 12px; color: var(--text-secondary); display: flex; gap: 6px; align-items: center; flex-shrink: 0 }
.qe-meta .dot { opacity: .45 }
.qe-dirty { color: #ea7517 }
.qe-fr { display: flex; align-items: center; gap: 6px; flex-wrap: nowrap; min-width: 0; justify-content: flex-end }
.qe-kbd { font-size: 11px; color: var(--text-secondary); opacity: .75; border: 1px solid var(--border-color); border-radius: 5px; padding: 2px 6px; line-height: 1.4; display: inline-flex; align-items: center; box-sizing: border-box; white-space: nowrap; flex-shrink: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis }
.qe-export-group { display: flex; align-items: center; gap: 4px; flex-shrink: 0 }
.qe-export-group .qe-ib {
  width: auto;
  height: auto;
  min-height: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 3px;
  box-sizing: border-box;
  padding: 2px 6px;
  line-height: 1.4;
  border-radius: 5px;
  border: 1px solid var(--btn-primary-border, var(--color-primary));
  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  transition: opacity .15s, filter .15s;
  white-space: nowrap;
  flex-shrink: 0;
}
.qe-export-group .qe-ib:hover:not(:disabled) {
  background: var(--btn-primary-hover-bg, var(--btn-primary-bg, var(--color-primary)));
  color: var(--btn-primary-hover-text, var(--btn-primary-text, #fff));
  border-color: var(--btn-primary-hover-border, var(--btn-primary-border, var(--color-primary)));
  filter: brightness(1.05);
  opacity: 1;
}
.qe-export-group .qe-ib:disabled {
  opacity: .55;
  cursor: not-allowed;
}
.qe-export-label { font-size: 11px; font-weight: 600; margin-left: 0 }
.qe-fr :deep(.qe-cancel-btn),
.qe-fr :deep(.qe-save-btn) {
  margin: 0 !important;
  min-width: 56px !important;
  height: 28px !important;
  padding: 0 12px !important;
  font-size: 12px !important;
  font-weight: 600 !important;
}
.qe-fr :deep(.qe-cancel-btn) {
  color: var(--btn-secondary-text) !important;
  background: var(--cancel-btn-color, var(--btn-secondary-bg)) !important;
  border-color: var(--btn-secondary-border) !important;
}
.qe-fr :deep(.qe-cancel-btn:hover) {
  background: var(--cancel-btn-hover-color, var(--btn-secondary-hover-bg)) !important;
  color: var(--btn-secondary-hover-text) !important;
  border-color: var(--btn-secondary-hover-border) !important;
}
.qe-fr :deep(.qe-save-btn) {
  color: var(--btn-primary-text) !important;
  background: var(--btn-primary-bg) !important;
  border-color: var(--btn-primary-border) !important;
}
</style>

<!-- global (non-scoped) to style el-dialog root -->
<style>
.qedit-dialog.el-dialog { border-radius: 16px; overflow: hidden; padding: 16px 18px 14px }
.qedit-dialog .el-dialog__header { margin-right: 0; padding: 0 0 12px }
.qedit-dialog .el-dialog__body { padding: 0 }
.qedit-dialog .el-dialog__footer { padding: 14px 0 0 }
</style>
