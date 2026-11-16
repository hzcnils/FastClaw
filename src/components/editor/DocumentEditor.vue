<script setup lang="ts">
/**
 * DocumentEditor - 豆包风格的分屏文档编辑器
 * 
 * 复用 fastplan 的 TipTapEditor 组件，提供：
 * - 富文本编辑（标题、加粗、列表、表格等）
 * - 大纲导航
 * - 导出（Markdown / 复制）
 * - 追问（基于文档内容发起新对话）
 */
import { ref, computed, watch, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { exportMarkdown, exportWord, exportPdf } from '@/utils/exportMessage'
import MarkdownIt from 'markdown-it'

const { t } = useI18n()

const md = new MarkdownIt({ html: false, breaks: true, linkify: true })

const props = defineProps<{
  content: string
  title?: string
  versions?: { label: string; content: string; timestamp: string }[]
  activeVersionIdx?: number
}>()

const emit = defineEmits<{
  close: []
  save: [content: string]
  export: [format: string]
  followUp: [question: string]
  'version-change': [idx: number]
}>()

const followUpInput = ref('')
const copied = ref(false)
const sidebarCollapsed = ref(false)
const isPreviewMode = ref(false)
// localContent 是编辑器内容的本地 Markdown 真值
const localContent = ref('')

const renderedHtml = computed(() => md.render(localContent.value))

// 外部内容变化（版本切换/AI 修改）时同步本地状态
watch(() => props.content, (val) => {
  if (val === localContent.value) return
  localContent.value = val
  nextTick(() => {
    sidebarCollapsed.value = outline.value.length === 0
  })
}, { immediate: true })

// Only user input emits a draft update; external version projections must not echo as edits.
function onContentInput(event: Event) {
  const value = (event.target as HTMLTextAreaElement).value
  localContent.value = value
  emit('save', value)
}

function toggleSidebar() {
  sidebarCollapsed.value = !sidebarCollapsed.value
}

function onVersionSelectChange(value: number) {
  emit('version-change', Number(value))
}

// 大纲从 localContent 实时提取
const outline = computed(() => {
  const lines = localContent.value.split('\n')
  const items: { level: number; title: string; id: string }[] = []
  let idx = 0
  for (const line of lines) {
    const match = line.match(/^(#{1,4})\s+(.+)/)
    if (match) {
      items.push({
        level: match[1].length,
        title: match[2],
        id: `heading-${idx++}`
      })
    }
  }
  return items
})

// 复制
const handleCopy = async () => {
  try {
    await navigator.clipboard.writeText(localContent.value)
    copied.value = true
    setTimeout(() => { copied.value = false }, 1500)
  } catch { /* fallback */ }
}

// 下载
const downloadPopoverRef = ref()
const handleExport = async (format: string) => {
  const content = localContent.value
  if (!content || !content.trim()) {
    ElMessage.warning(t('chat.emptyContentCannotExport'))
    return
  }
  downloadPopoverRef.value?.hide()
  try {
    if (format === 'md') {
      const ok = await exportMarkdown(content)
      if (ok) ElMessage.success(`Markdown ${t('chat.downloadSuccess')}`)
    } else if (format === 'docx') {
      const msg = ElMessage.info({ message: t('chat.exportGenerating'), duration: 0 })
      const ok = await exportWord(content, (markdown) => md.render(markdown))
      msg.close()
      if (ok) ElMessage.success(`Word ${t('chat.downloadSuccess')}`)
    } else if (format === 'pdf') {
      const msg = ElMessage.info({ message: t('chat.exportGenerating'), duration: 0 })
      const result = await exportPdf(content, (markdown) => md.render(markdown))
      msg.close()
      if (result.success) {
        ElMessage.success(`PDF ${t('chat.downloadSuccess')}`)
      } else if (!result.canceled) {
        ElMessage.error(t('chat.exportFailed', { error: result.error || '' }))
      }
    }
  } catch (err: any) {
    console.error('Export failed:', err)
    ElMessage.error(t('chat.exportFailed', { error: err?.message || String(err) }))
  }
}

// 追问
const handleFollowUp = () => {
  if (!followUpInput.value.trim()) return
  emit('followUp', followUpInput.value.trim())
  followUpInput.value = ''
}

const handleFollowUpKeydown = (e: KeyboardEvent) => {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    handleFollowUp()
  }
}
</script>

<template>
  <div class="doc-editor">
    <!-- Header -->
    <div class="editor-header">
      <div class="header-left">
        <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none">
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
          <polyline points="14 2 14 8 20 8" />
          <line x1="16" y1="13" x2="8" y2="13" />
          <line x1="16" y1="17" x2="8" y2="17" />
        </svg>
         <span class="header-title">{{ t('chat.aiAnswer') || t('chat.docEdit') }}</span>
        <!-- <span class="header-title">{{ title || t('chat.docEdit') }}</span> -->
        <!-- <span class="header-time">创建时间: {{ new Date().toLocaleString('zh-CN') }}</span> -->
      </div>
      <!-- 版本切换器（有版本记录时显示） -->
      <div v-if="versions && versions.length > 0" class="version-switcher">
        <span>{{ t('chat.versionLabel') }}</span>
        
        <el-select
          :model-value="activeVersionIdx"
          class="version-select"
          @update:model-value="onVersionSelectChange"
          :teleported="false"
          size="small"
        >
          <el-option v-for="(v, i) in versions" :key="v.label" :value="i"
            :label="v.label + ' · ' + new Date(v.timestamp).toLocaleString('zh-CN', { month:'2-digit', day:'2-digit', hour:'2-digit', minute:'2-digit' })">
          </el-option>
        </el-select>

      </div>
      <div class="header-actions">
        <!-- 预览 / 编辑切换 -->
        <button v-if="!isPreviewMode" class="hdr-btn" @click="isPreviewMode = true" :title="t('chat.preview')">
          <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="2" fill="none">
            <path d="M1 12 C1 12 5 4 12 4 C19 4 23 12 23 12 C23 12 19 20 12 20 C5 20 1 12 1 12 Z" />
            <circle cx="12" cy="12" r="3" />
          </svg>
          <span>{{ t('chat.preview') }}</span>
        </button>
        <button v-if="isPreviewMode" class="hdr-btn hdr-btn--active" @click="isPreviewMode = false" :title="t('common.edit')">
          <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="2" fill="none">
            <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
            <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
          </svg>
          <span>{{ t('common.edit') }}</span>
        </button>
        <!-- 复制 -->
        <button class="hdr-btn" @click="handleCopy" :title="copied ? t('chat.copied') : t('common.copy')">
          <svg v-if="!copied" viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="2"
            fill="none">
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
          </svg>
          <svg v-else viewBox="0 0 24 24" width="15" height="15" stroke="#52c41a" stroke-width="2" fill="none">
            <polyline points="20 6 9 17 4 12" />
          </svg>
          <span>{{ copied ? t('chat.copied') : t('common.copy') }}</span>
        </button>
        <!-- 下载 -->
        <el-popover placement="bottom" :width="180" trigger="click" ref="downloadPopoverRef">
          <template #reference>
            <button class="hdr-btn" :title="t('common.download')">
              <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="2" fill="none">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                <polyline points="7 10 12 15 17 10" />
                <line x1="12" y1="15" x2="12" y2="3" />
              </svg>
              <span>{{ t('common.download') }}</span>
            </button>
          </template>
          <div style="display:flex;flex-direction:column;gap:4px;padding:4px">
            <button class="dropdown-item" @click="handleExport('md')">{{ t('chat.downloadAsMD') }}</button>
            <button class="dropdown-item" @click="handleExport('docx')">{{ t('chat.downloadAsDOCX') }}</button>
            <button class="dropdown-item" @click="handleExport('pdf')">{{ t('chat.downloadAsPDF') }}</button>
          </div>
        </el-popover>
        <!-- 分享 -->
          <!-- 
        <button class="hdr-btn" :title="t('chat.share')">
          <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="2" fill="none">
            <circle cx="18" cy="5" r="3" />
            <circle cx="6" cy="12" r="3" />
            <circle cx="18" cy="19" r="3" />
            <line x1="8.59" y1="13.51" x2="15.42" y2="17.49" />
            <line x1="15.41" y1="6.51" x2="8.59" y2="10.49" />
          </svg>
          <span>{{ t('chat.share') }}</span>
        </button>
        -->
        <!-- 更多 -->
<!-- <button class="hdr-btn hdr-btn--more" title="更多">
          <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="2" fill="none">
            <circle cx="12" cy="12" r="1" />
            <circle cx="19" cy="12" r="1" />
            <circle cx="5" cy="12" r="1" />
          </svg>
        </button> -->
        <!-- 关闭 -->
        <button class="hdr-btn hdr-btn--close" @click="emit('close')" :title="t('common.close')">
          <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="2" fill="none">
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </div>
    </div>

    <!-- 主体区域：大纲侧边栏 + 编辑器 -->
    <div class="editor-main">
      <!-- 左侧大纲导航 -->
      <div class="outline-sidebar" :class="{ collapsed: sidebarCollapsed }">
        <div class="outline-header" @click="sidebarCollapsed && toggleSidebar()">
          <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
            <line x1="8" y1="6" x2="21" y2="6" />
            <line x1="8" y1="12" x2="21" y2="12" />
            <line x1="8" y1="18" x2="21" y2="18" />
            <line x1="3" y1="6" x2="3.01" y2="6" />
            <line x1="3" y1="12" x2="3.01" y2="12" />
            <line x1="3" y1="18" x2="3.01" y2="18" />
          </svg>
          <span class="outline-label">{{ t('chat.catalog') }}</span>
          <button class="sidebar-toggle" :title="sidebarCollapsed ? t('chat.expandCatalog') : t('chat.foldCatalog')" @click.stop="toggleSidebar">
            <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"
              :style="{ transform: sidebarCollapsed ? 'rotate(180deg)' : 'none', transition: 'transform 0.2s' }">
              <polyline points="15 18 9 12 15 6" />
            </svg>
          </button>
        </div>
        <div v-show="!sidebarCollapsed" class="outline-list">
          <div v-for="item in outline" :key="item.id" class="outline-item" :class="`level-${item.level}`"
            :style="{ paddingLeft: (item.level - 1) * 16 + 12 + 'px' }">
            <span class="outline-dot"></span>
            <span class="outline-text">{{ item.title }}</span>
          </div>
          <div v-if="!outline.length" class="outline-empty">
            {{ t('chat.noOutline') }}
          </div>
        </div>
      </div>

      <!-- 右侧内容区：编辑态/预览态 -->
      <div class="editor-content">
        <textarea
          v-if="!isPreviewMode"
          :value="localContent"
          @input="onContentInput"
          class="editor-textarea"
          :placeholder="t('chat.inputMarkdown')"
          spellcheck="false"
        />
        <div
          v-else
          class="editor-body editor-preview"
          v-html="renderedHtml"
        />
      </div>
    </div>

    <!-- 底部追问栏 -->
    <!-- <div class="editor-footer">
      <div class="follow-up-bar">
        <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none" class="quote-icon">
          <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
        </svg>
        <input
          v-model="followUpInput"
          type="text"
          placeholder="输入主题和写作要求"
          class="follow-up-input"
          @keydown="handleFollowUpKeydown"
        />
        <button
          class="follow-up-send"
          :disabled="!followUpInput.trim()"
          @click="handleFollowUp"
        >
          <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none">
            <line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/>
          </svg>
        </button>
      </div>
    </div> -->
  </div>
</template>

<style lang="scss" scoped>
.doc-editor {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: auto;
  background: transparent;
}

// ====== Header ======
.editor-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 5px;
  border-bottom: 1px solid var(--chat-divider-color);
  background: transparent;
  gap: 0px;
  min-width: 0;

}

.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-primary);
  flex-shrink: 0;
  min-width: 0;

  .header-title {
    font-size: var(--font-size-base);
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .header-time {
    font-size: 11px;
    color: var(--text-color-light-black);
    margin-left: 8px;
  }
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
  justify-content: flex-end;
}

.hdr-btn {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 6px;
  border: none;
  background: transparent;
  border-radius: 6px;
  cursor: pointer;
  font-size: var(--font-size-sm);
  color: var(--text-color-light-black);
  transition: all 0.15s;

  &:hover {
    background: var(--chat-divider-color);
    color: var(--text-color-light-black);
  }

  &--active {
    background: color-mix(in srgb, #5B6AF0 10%, transparent);
    color: #5B6AF0;
  }

  &--close {
    margin-left: 8px;
    flex-shrink: 0;

    &:hover {
      background: rgba(255, 77, 79, 0.08);
      color: #ff4d4f;
    }
  }

  &--more {
    padding: 6px;
  }
}

// ====== Main: Outline + Editor ======
.editor-main {

  flex: 1;
  display: flex;
  overflow: hidden;
}

.outline-sidebar {
  width: 200px;
  min-width: 200px;
  border-right: 1px solid var(--chat-divider-color);
  display: flex;
  flex-direction: column;
  background: #fafbfc;
  transition: width 0.2s ease, min-width 0.2s ease;

  &.collapsed {
    width: 36px;
    min-width: 36px;
    overflow: hidden;

    .outline-header {
      padding: 14px 0 10px;
      justify-content: center;
      cursor: pointer;

      &>svg {
        display: none;
      }
    }

    .outline-label {
      display: none;
    }


    .sidebar-toggle {
      margin-left: 0;
    }
  }

  background: transparent;
}

.outline-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 14px 16px 10px;
  font-size: var(--font-size-sm);
  font-weight: 600;
  color: var(--text-color-light-black);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.sidebar-toggle {
  margin-left: auto;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border: none;
  background: transparent;
  border-radius: 4px;
  cursor: pointer;

  flex-shrink: 0;
  padding: 0;

  &:hover {
    background: var(--dropdown-hover-color);
    
  }
}

// 版本切换器
.version-switcher {
  display: flex;
  align-items: center;
  gap: 5px;
  margin: 0 12px;
  flex-shrink: 0;
    /* 新增：文字单行不换行 */
  white-space: nowrap;
 
}

.version-icon {
  color: #5B6AF0;
  flex-shrink: 0;
}

.version-select {
  min-width: 145px;
}
.version-select :deep(.el-select__wrapper),
.version-select :deep(.el-input__wrapper) {
  background: transparent !important;
  border: 1px solid var(--border-color) !important;
  border-radius: 6px !important;
  box-shadow: none !important;
}
.version-select :deep(.el-select__wrapper:hover),
.version-select :deep(.el-input__wrapper:hover) {
  border-color: var(--border-color) !important;
}
.version-select :deep(.el-select__wrapper.is-focus),
.version-select :deep(.el-input__wrapper.is-focus) {
  border-color: var(--border-color) !important;
  box-shadow: 0 0 0 1px var(--border-color) !important;
}

.outline-list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 8px;

  scrollbar-width: none;

  &::-webkit-scrollbar {
    display: none;
  }
}

.outline-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.15s;

  &:hover {
    background: rgba(91, 106, 240, 0.06);
  }

  &.level-1 .outline-text {
    font-weight: 600;
    font-size: var(--font-size-md);
    color: var(--text-color-light-black);
  }

  &.level-2 .outline-text {
    font-size: var(--font-size-sm);
    color: var(--text-color-light-black);
  }

  &.level-3 .outline-text {
    font-size: var(--font-size-sm);
    color: var(--text-color-light-black);
  }

  &.level-4 .outline-text {
    font-size: 11px;
    color: var(--text-color-light-black);
  }
}

.outline-dot {
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: #bbb;
  flex-shrink: 0;
}

.outline-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.outline-empty {
  padding: 20px 16px;
  text-align: center;
  font-size: var(--font-size-sm);
  color: var(--text-color-light-black);
}

// ====== Editor Content ======
.editor-content {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.editor-preview {
  flex: 1;
  overflow-y: auto;
  padding: 32px 48px;
  scrollbar-width: thin;
  scrollbar-color: #ddd transparent;

  &::-webkit-scrollbar { width: 6px; }
  &::-webkit-scrollbar-thumb { background: #ddd; border-radius: 3px; }
}

// ====== Markdown 编辑 textarea ======
.editor-textarea {
  width: 100%;
  height: 100%;
  padding: 32px 48px;
  border: none;
  outline: none;
  resize: none;
  font-family: 'JetBrains Mono', 'Cascadia Code', Consolas, monospace;
  font-size: 14px;
  line-height: 1.8;
  color: var(--text-primary, #333);
  background: transparent;
  box-sizing: border-box;

  &::placeholder {
    color: #ccc;
  }

  scrollbar-width: thin;
  scrollbar-color: #ddd transparent;

  &::-webkit-scrollbar {
    width: 6px;
  }
  &::-webkit-scrollbar-thumb {
    background: #ddd;
    border-radius: 3px;
  }
}

.editor-body {
  width: 100%;
  // max-width: 720px;
  margin: 0 auto;
  font-size: 15px;
  line-height: 1.8;


  :deep(h1) {
    font-size: 1.8em;
    font-weight: 700;
    margin: 24px 0 16px;
  }

  :deep(h2) {
    font-size: 1.4em;
    font-weight: 600;
    margin: 20px 0 12px;
  }

  :deep(h3) {
    font-size: 1.15em;
    font-weight: 600;
    margin: 16px 0 8px;
  }

  :deep(h4) {
    font-size: 1em;
    font-weight: 600;
    margin: 12px 0 6px;
  }

  :deep(p) {
    margin: 10px 0;
  }

  :deep(strong) {
    font-weight: 600;
  }

  :deep(ul),
  :deep(ol) {
    padding-left: 1.5em;
    margin: 10px 0;
  }

  :deep(li) {
    margin: 4px 0;
  }

  :deep(blockquote) {
    border-left: 3px solid #5B6AF0;
    margin: 14px 0;
    padding: 8px 16px;
    color: #666;
    background: rgba(91, 106, 240, 0.04);
    border-radius: 0 8px 8px 0;
  }

  :deep(code) {
    background: rgba(0, 0, 0, 0.05);
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 0.9em;
    font-family: 'JetBrains Mono', Consolas, monospace;
  }

  :deep(pre) {
    background: #1e1e2e;
    border-radius: 10px;
    padding: 16px;
    overflow-x: auto;
    margin: 14px 0;

    code {
      background: transparent;
      color: #d4d4d8;
      padding: 0;
    }
  }

  :deep(table) {
    width: 100%;
    border-collapse: collapse;
    margin: 14px 0;
    font-size: var(--font-size-base);
  }

  :deep(th) {
    background: #f5f5f5;
    font-weight: 600;
    text-align: left;
    padding: 10px 14px;
    border: 1px solid #e8e8e8;
  }

  :deep(td) {
    padding: 8px 14px;
    border: 1px solid #e8e8e8;
  }
}

// ====== Footer: Follow-up ======
.editor-footer {
  padding: 12px 20px;
  border-top: 1px solid var(--chat-divider-color);
  // background: #fafafa;
  background: transparent;
}

.follow-up-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  background: var(--chat-input-skill-chip);
  border: 1px solid var(--border-color);
  border-radius: 10px;
  padding: 6px 12px;
  transition: border-color 0.2s;


  &:focus-within {
    border: 1px solid var(--chat-divider-color);
    box-shadow: 0 0 0 1px rgba(91, 106, 240, 0.1);
  }
}

.quote-icon {
  color: rgba(var(--text-color-light-black), 0.4);
  flex-shrink: 0;
}

.follow-up-input {
  flex: 1;
  border: none;
  outline: none;
  font-size: var(--font-size-md);

  background: transparent;

  &::placeholder {
    color: var(--text-color-light-black);
  }
}

.follow-up-send {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: var(--btn-primary-bg, var(--color-primary));
  border-radius: 999px;
  cursor: pointer;
  color: white;
  transition: all 0.15s;
  flex-shrink: 0;

  &:hover {
    background: var(--btn-primary-hover-bg, var(--color-primary));
    box-shadow: 0 0 0 1px rgba(91, 106, 240, 0.1);
  }

  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
}




// html.dark .hdr-btn,
// html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .hdr-btn,

// html.dark .header-time,
// html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .header-time{
// color:#ddd;
// }

.dropdown-item {
  display: block;
  width: 100%;
  padding: 8px 12px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-primary);
  font-size: 13px;
  text-align: left;
  cursor: pointer;
  &:hover {
    background: var(--bg-hover);
  }
}
</style>

<style>
.version-select .el-select__wrapper,
.version-select .el-input__wrapper {
  background: transparent !important;
  border: 1px solid var(--border-color) !important;
  border-radius: 6px !important;
  box-shadow: none !important;
}
.version-select .el-select__wrapper:hover,
.version-select .el-input__wrapper:hover {
  border-color: var(--border-color) !important;
}
.version-select .el-select__wrapper.is-focus,
.version-select .el-input__wrapper.is-focus {
  border-color: var(--border-color) !important;
  box-shadow: 0 0 0 1px var(--border-color) !important;
}
</style>
