<script setup lang="ts">
import { computed, ref } from 'vue'
import { usePreviewPanelStore } from '@/stores/previewPanelStore'
import { buildReaderHtml } from '@/utils/webReader'
import MarkdownViewer from '@/components/markdown/MarkdownViewer.vue'
import type { ToolCallEvent, SearchResult, FetchWebPageResult } from '@/types'

const props = defineProps<{
  tool: ToolCallEvent
}>()

const store = usePreviewPanelStore()

type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>
let _invoke: InvokeFn | null = null
async function getInvoke(): Promise<InvokeFn | null> {
  if (_invoke) return _invoke
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    _invoke = invoke as InvokeFn
    return _invoke
  } catch {
    return null
  }
}

/** 解析 result_preview（内置工具的 result_preview 是 {content,is_error,metadata} 的 JSON 串） */
function parseResultPreview(): Record<string, any> | null {
  const raw = props.tool.result_preview
  if (!raw) return null
  const s = raw.trim()
  if (!s.startsWith('{')) return null
  try { return JSON.parse(s) } catch { return null }
}

/** 依次从 tool.metadata / result_preview JSON 里挖 key（兼容 toolUseResult 平铺两种形态） */
function collectMeta(key: string): any {
  const m = props.tool.metadata
  if (m && typeof m === 'object') {
    const mm = m as Record<string, any>
    if (key in mm) return mm[key]
    const tu = mm.toolUseResult
    if (tu && typeof tu === 'object' && key in tu) return (tu as Record<string, any>)[key]
  }
  const p = parseResultPreview()
  if (p && typeof p === 'object') {
    if (key === 'content' && typeof p.content === 'string') return p.content
    const meta = p.metadata
    if (meta && typeof meta === 'object' && key in meta) return (meta as Record<string, any>)[key]
    const tu = p.toolUseResult
    if (tu && typeof tu === 'object' && key in tu) return (tu as Record<string, any>)[key]
  }
  return undefined
}

const queries = computed<string[]>(() => {
  const q = collectMeta('queries')
  return Array.isArray(q) ? q.map(String) : []
})

const structuredResults = computed<SearchResult[]>(() => {
  const r = collectMeta('results')
  return Array.isArray(r) ? (r as SearchResult[]) : []
})

/** 无结构化 metadata 时，从 Markdown 正文（render 的编号列表）里兜底解析标题/URL/摘要 */
const markdownContent = computed<string>(() => {
  const c = collectMeta('content')
  if (typeof c === 'string' && c.trim()) return c
  // result_preview 本身可能直接是 Markdown（旧消息 / 非 JSON 工具）
  const p = parseResultPreview()
  if (p) return ''
  return props.tool.result_preview || ''
})

function parseMarkdownResults(text: string): SearchResult[] {
  if (!text) return []
  const out: SearchResult[] = []
  let cur: SearchResult | null = null
  for (const line of text.split('\n')) {
    const m = line.match(/^\s*\d+\.\s*\*\*(.+?)\*\*/)
    if (m) {
      if (cur) out.push(cur)
      cur = { title: m[1].trim(), url: '' }
      continue
    }
    if (!cur) continue
    const url = line.match(/URL:\s*(\S+)/)
    if (url) { cur.url = url[1]; continue }
    const snippet = line.match(/摘要:\s*(.*)/)
    if (snippet) { cur.snippet = snippet[1].trim(); continue }
    const src = line.match(/来源:\s*(.*)/)
    if (src) { cur.engines = src[1].split(',').map((s) => s.trim()).filter(Boolean); continue }
  }
  if (cur) out.push(cur)
  return out.filter((r) => r.url || r.title)
}

const parsedMarkdownResults = computed<SearchResult[]>(() =>
  structuredResults.value.length > 0 ? [] : parseMarkdownResults(markdownContent.value),
)

const results = computed<SearchResult[]>(() =>
  structuredResults.value.length > 0 ? structuredResults.value : parsedMarkdownResults.value,
)

/** 有结果才走折叠列表；否则（旧消息/空结果）回退 Markdown 平铺 */
const hasMeta = computed(() => results.value.length > 0)

const summaryText = computed(() => {
  const parts: string[] = []
  if (queries.value.length) parts.push(`搜索 ${queries.value.length} 个关键词`)
  if (results.value.length) parts.push(`参考 ${results.value.length} 篇资料`)
  return parts.length ? parts.join(' · ') : '搜索结果'
})

const isOpen = ref(false)
const showRaw = ref(false)
const loadingUrl = ref<string | null>(null)

const activeUrl = computed(() => store.activeUrl)

function hostOf(r: SearchResult): string {
  if (r.host) return r.host
  try { return new URL(r.url).hostname.replace(/^www\./, '') } catch { return '' }
}

async function openResult(r: SearchResult) {
  const url = (r.url || '').trim()
  if (!url) return
  loadingUrl.value = url
  store.openExternalLoading(url, r.title || hostOf(r) || url)
  try {
    const inv = await getInvoke()
    if (!inv) {
      window.open(url, '_blank', 'noopener,noreferrer')
      store.failExternal('当前环境不支持内嵌预览，已用新窗口打开')
      return
    }
    const res = await inv<FetchWebPageResult>('fetch_web_page', { url })
    const finalUrl = res.final_url || url
    if (!res.html) {
      store.failExternal('该链接不是网页（非 HTML 内容），无法预览，请用系统浏览器打开')
      return
    }
    const readerHtml = buildReaderHtml(res.html, finalUrl)
    if (!res.embeddable && !readerHtml) {
      store.failExternal('该站点禁止嵌入且正文抽取失败，请用系统浏览器打开')
      return
    }
    store.openExternal({
      final_url: finalUrl,
      html: res.html,
      embeddable: res.embeddable,
      title: r.title || finalUrl,
      readerHtml,
    })
  } catch (e: any) {
    store.failExternal(`抓取失败：${e?.message || e || '网络错误'}，请用系统浏览器打开`)
  } finally {
    loadingUrl.value = null
  }
}
</script>

<template>
  <!-- 无结构化结果（旧消息 / 空结果）：回退 Markdown 平铺 -->
  <MarkdownViewer v-if="!hasMeta" :content="markdownContent" class="jmis-search-fallback" />

  <div v-else class="jmis-search">
    <!-- 折叠态一行 -->
    <button
      v-if="!isOpen"
      type="button"
      class="jmis-search__bar"
      @click="isOpen = true"
    >
      <span class="jmis-search__icon">🔍</span>
      <span class="jmis-search__summary">{{ summaryText }}</span>
      <span class="jmis-search__hint">展开</span>
    </button>

    <template v-else>
      <!-- 展开态标题行 -->
      <button
        type="button"
        class="jmis-search__bar jmis-search__bar--open"
        @click="isOpen = false"
      >
        <span class="jmis-search__icon">🔍</span>
        <span class="jmis-search__summary">{{ summaryText }}</span>
        <span class="jmis-search__hint">收起</span>
      </button>

      <!-- 结果列表：>20 条内部滚动 -->
      <ul class="jmis-search__list">
        <li v-for="(r, i) in results" :key="r.url || i">
          <button
            type="button"
            class="jmis-search__item"
            :class="{ active: activeUrl === r.url }"
            @click="openResult(r)"
          >
            <span class="jmis-search__num">
              <svg v-if="loadingUrl === r.url" class="jmis-search__spinner" viewBox="0 0 24 24" fill="none">
                <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" class="opacity-25" />
                <path d="M4 12a8 8 0 018-8" stroke="currentColor" stroke-width="3" stroke-linecap="round" class="opacity-75" />
              </svg>
              <span v-else>{{ i + 1 }}</span>
            </span>
            <span class="jmis-search__body">
              <span class="jmis-search__title-row">
                <span class="jmis-search__title">{{ r.title }}</span>
                <span v-if="hostOf(r)" class="jmis-search__host">{{ hostOf(r) }}</span>
              </span>
              <span v-if="r.snippet" class="jmis-search__snippet">{{ r.snippet }}</span>
            </span>
          </button>
        </li>
      </ul>

      <!-- 原始数据 toggle -->
      <button type="button" class="jmis-search__raw-toggle" @click="showRaw = !showRaw">
        <span class="jmis-search__raw-arrow" :class="{ open: showRaw }">›</span>
        原始数据
      </button>
      <pre v-if="showRaw" class="jmis-search__raw">{{ tool.result_preview || '' }}</pre>
    </template>
  </div>
</template>

<style lang="scss" scoped>
.jmis-search {
  width: 100%;
  margin-top: 4px;
}

.jmis-search__bar {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 6px 10px;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  background: var(--bg-hover, rgba(0, 0, 0, 0.04));
  color: var(--text-secondary);
  font-size: 12px;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;

  &:hover {
    background: var(--bg-active, rgba(0, 0, 0, 0.07));
    color: var(--text-primary);
  }

  &--open {
    border-bottom-left-radius: 0;
    border-bottom-right-radius: 0;
    border-bottom: none;
  }
}

.jmis-search__icon { flex-shrink: 0; }

.jmis-search__summary {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.jmis-search__hint {
  margin-left: auto;
  flex-shrink: 0;
  font-size: 11px;
  opacity: 0.65;
}

.jmis-search__list {
  list-style: none;
  margin: 0;
  padding: 2px;
  max-height: 280px;
  overflow-y: auto;
  border: 1px solid var(--border-color);
  border-top: none;
  border-bottom-left-radius: 8px;
  border-bottom-right-radius: 8px;
  background: var(--bg-primary);
}

.jmis-search__item {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  width: 100%;
  padding: 6px 8px;
  border: none;
  background: transparent;
  text-align: left;
  cursor: pointer;
  border-radius: 6px;
  transition: background 0.15s;

  &:hover { background: var(--bg-hover, rgba(0, 0, 0, 0.04)); }
  &.active { background: var(--bg-hover, rgba(0, 0, 0, 0.06)); }
}

.jmis-search__num {
  flex-shrink: 0;
  width: 18px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-tertiary, var(--text-secondary));
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}

.jmis-search__spinner {
  width: 13px;
  height: 13px;
  animation: jmis-spin 0.8s linear infinite;
  color: var(--text-secondary);
}

.jmis-search__body {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.jmis-search__title-row {
  display: flex;
  align-items: baseline;
  gap: 8px;
}

.jmis-search__title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  font-weight: 500;
  color: var(--text-primary);
}

.jmis-search__item:hover .jmis-search__title { color: var(--color-primary); }

.jmis-search__host {
  flex-shrink: 0;
  font-size: 10px;
  color: var(--text-tertiary, var(--text-secondary));
  opacity: 0.7;
}

.jmis-search__snippet {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 11px;
  line-height: 1.5;
  color: var(--text-secondary);
}

.jmis-search__raw-toggle {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  margin-top: 6px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: 11px;
  cursor: pointer;

  &:hover { color: var(--text-primary); }
}

.jmis-search__raw-arrow {
  display: inline-block;
  transition: transform 0.15s;
  &.open { transform: rotate(90deg); }
}

.jmis-search__raw {
  margin: 4px 0 0;
  padding: 8px 10px;
  max-height: 180px;
  overflow-y: auto;
  background: var(--bg-hover, rgba(0, 0, 0, 0.04));
  border-radius: 6px;
  font-family: 'Cascadia Code', 'Fira Code', 'JetBrains Mono', monospace;
  font-size: 11px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-all;
  color: var(--text-secondary);
}

@keyframes jmis-spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
</style>
