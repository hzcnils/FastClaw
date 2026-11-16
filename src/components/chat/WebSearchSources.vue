<script setup lang="ts">
import { computed, inject, onActivated, onDeactivated, onBeforeUnmount, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { CHAT_SOURCE_LINKS } from '@/modules/chat/sourceInteractionBinding'
import type { WebHit } from '@/types'

const props = defineProps<{
  hits: WebHit[]
  query?: string
  messageId?: string
}>()

const { t } = useI18n()

function hostname(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, '')
  } catch {
    return url
  }
}

// 用域名首字母作为来源图标，避免依赖外部 favicon 服务
function initial(url: string): string {
  const h = hostname(url)
  return (h[0] || '?').toUpperCase()
}

// 根据域名生成稳定的柔和底色
function badgeColor(url: string): string {
  const h = hostname(url)
  let hash = 0
  for (let i = 0; i < h.length; i++) hash = (hash * 31 + h.charCodeAt(i)) >>> 0
  const hue = hash % 360
  return `hsl(${hue}, 55%, 55%)`
}

const count = computed(() => props.hits?.length || 0)

const links = inject(CHAT_SOURCE_LINKS, null)
const active = ref(true), generation = ref(0)
onActivated(() => { active.value = true })
onDeactivated(() => { active.value = false; generation.value++ })
onBeforeUnmount(() => { active.value = false; generation.value++ })
// Build captured row callbacks while rendering; a stale DOM event cannot borrow a newer source.
const rows = computed(() => {
  const epoch = generation.value, id = props.messageId, hits = props.hits, query = props.query
  return hits.map(hit => {
    const url = hit.url
    const displayed = () => active.value && generation.value === epoch && props.messageId === id
      && props.hits === hits && props.query === query && hits.includes(hit) && hit.url === url
    return { hit, open: active.value && id ? links?.captureLink(id, url, displayed) : undefined }
  })
})
</script>

<template>
  <div class="websearch-sources">
    <div v-if="query" class="ws-query-row">
      <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
        <circle cx="11" cy="11" r="8"></circle>
        <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
      </svg>
      <span class="ws-query-text">{{ query }}</span>
    </div>

    <div class="ws-list">
      <div v-for="({ hit, open }, i) in rows" :key="i" class="ws-item" :title="hit.title || hit.url" @click="open">
        <div class="ws-item-head">
          <span class="ws-badge" :style="{ background: badgeColor(hit.url) }">{{ initial(hit.url) }}</span>
          <span class="ws-host">{{ hostname(hit.url) }}</span>
          <span class="ws-index">{{ i + 1 }}</span>
        </div>
        <div class="ws-title">{{ hit.title || hostname(hit.url) }}</div>
        <div v-if="hit.snippet" class="ws-snippet">{{ hit.snippet }}</div>
      </div>

      <div v-if="count === 0" class="ws-empty">{{ t('chat.webNoResults') }}</div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.websearch-sources {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.ws-query-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  color: var(--text-secondary);
  font-size: 13px;
  border-bottom: 1px solid var(--border-color);

  .ws-query-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
}

.ws-list {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
}

.ws-item {
  padding: 12px;
  border-radius: 10px;
  cursor: pointer;
  transition: background 0.15s;

  &:hover {
    background: var(--bg-hover, #f3f4f6);
  }

  & + .ws-item {
    margin-top: 2px;
  }
}

.ws-item-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}

.ws-badge {
  width: 18px;
  height: 18px;
  border-radius: 5px;
  color: #fff;
  font-size: 11px;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.ws-host {
  font-size: 12px;
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  min-width: 0;
}

.ws-index {
  font-size: 11px;
  color: var(--text-tertiary, #9ca3af);
  flex-shrink: 0;
}

.ws-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  line-height: 1.4;
  margin-bottom: 4px;
  transition: color 0.15s;
}

.ws-item:hover .ws-title {
  color: var(--color-primary, #3b82f6);
  text-decoration: underline;
  text-underline-offset: 2px;
}

.ws-snippet {
  font-size: 12.5px;
  color: var(--text-secondary);
  line-height: 1.5;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.ws-empty {
  padding: 24px;
  text-align: center;
  color: var(--text-secondary);
  font-size: 13px;
}
</style>
