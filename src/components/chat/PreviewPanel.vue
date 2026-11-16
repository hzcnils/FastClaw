<script setup lang="ts">
// FastClaw · 右侧网页预览面板
// 由 previewPanelStore 驱动：搜索结果点标题 → fetch_web_page → 这里 iframe 渲染。
// 显隐以 previewPanelStore.isOpen 为唯一事实来源（ChatView 据此切 rightPanel）。
import { computed } from 'vue'
import { X, ExternalLink, BookOpen, Monitor } from 'lucide-vue-next'
import { usePreviewPanelStore } from '@/stores/previewPanelStore'

const store = usePreviewPanelStore()

// 阅读模式可用性：抓取成功且有阅读器正文
const readerAvailable = computed(() => !!store.external?.readerHtml)

function openInBrowser() {
  const url = store.activeUrl
  if (url) window.open(url, '_blank', 'noopener,noreferrer')
}
</script>

<template>
  <div class="preview-panel">
    <div class="panel-head">
      <span class="panel-title" :title="store.externalTitle">{{ store.externalTitle || '网页预览' }}</span>
      <div class="panel-actions">
        <template v-if="store.external">
          <button
            type="button"
            class="pp-btn"
            :class="{ on: store.readerMode }"
            :disabled="!readerAvailable"
            :title="readerAvailable ? '阅读模式（正文提取）' : '该页无阅读正文'"
            @click="store.setReaderMode(true)"
          >
            <BookOpen :size="14" /> 阅读
          </button>
          <button
            type="button"
            class="pp-btn"
            :class="{ on: !store.readerMode }"
            title="原始网页"
            @click="store.setReaderMode(false)"
          >
            <Monitor :size="14" /> 原始
          </button>
          <button type="button" class="pp-btn" title="用系统浏览器打开" @click="openInBrowser">
            <ExternalLink :size="14" />
          </button>
        </template>
        <button type="button" class="pp-btn" title="关闭预览" @click="store.close()">
          <X :size="14" />
        </button>
      </div>
    </div>

    <div class="panel-body">
      <!-- 抓取中 -->
      <div v-if="store.loading" class="pp-state">
        <span class="pp-spinner" />
        <span>正在抓取网页…</span>
      </div>

      <!-- 抓取失败 / 非 HTML -->
      <div v-else-if="store.error" class="pp-state pp-error">
        <p>{{ store.error }}</p>
        <button type="button" class="pp-open" @click="openInBrowser">用系统浏览器打开</button>
      </div>

      <!-- iframe 预览：阅读模式 srcdoc（净化正文），原始模式 src（跨域直嵌） -->
      <iframe
        v-else-if="store.external"
        :key="store.reloadKey"
        class="pp-iframe"
        sandbox="allow-same-origin allow-scripts allow-popups allow-forms allow-popups-to-escape-sandbox"
        referrerpolicy="no-referrer"
        :src="store.readerMode ? undefined : store.external.final_url"
        :srcdoc="store.readerMode ? store.external.readerHtml : undefined"
      />

      <!-- 空 -->
      <div v-else class="pp-state">点击搜索结果标题以预览网页</div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.preview-panel {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
}

.panel-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
}
.panel-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
}
.panel-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}
.pp-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: var(--text-secondary);
  font-size: 12px;
  cursor: pointer;

  &:hover {
    color: var(--text-primary);
    background: var(--bg-hover, rgba(0, 0, 0, 0.05));
  }
  &.on {
    color: var(--color-primary);
    background: rgba(59, 130, 246, 0.1);
  }
  &:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
}

.panel-body {
  flex: 1;
  min-height: 0;
  position: relative;
  display: flex;
  background: #fff;
}
.pp-iframe {
  width: 100%;
  height: 100%;
  border: none;
}

.pp-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 24px;
  color: var(--text-secondary);
  font-size: 13px;
  text-align: center;

  &.pp-error { color: #ef4444; }
  p { margin: 0; }
}
.pp-spinner {
  width: 20px;
  height: 20px;
  border: 2px solid var(--border-color);
  border-top-color: var(--color-primary, #3b82f6);
  border-radius: 50%;
  animation: pp-spin 0.8s linear infinite;
}
.pp-open {
  padding: 6px 14px;
  border: none;
  border-radius: 6px;
  background: var(--color-primary, #3b82f6);
  color: #fff;
  font-size: 13px;
  cursor: pointer;

  &:hover { opacity: 0.9; }
}

@keyframes pp-spin {
  to { transform: rotate(360deg); }
}
</style>
