<script setup lang="ts">
defineProps<{
  title: string
  versionLabel: string
  timestamp: string
}>()

defineEmits<{ click: [] }>()

function formatTime(ts: string) {
  const d = new Date(ts)
  const mm = String(d.getMonth() + 1).padStart(2, '0')
  const dd = String(d.getDate()).padStart(2, '0')
  const hh = String(d.getHours()).padStart(2, '0')
  const min = String(d.getMinutes()).padStart(2, '0')
  return `${mm}-${dd} ${hh}:${min}`
}
</script>

<template>
  <div class="doc-version-card" @click="$emit('click')">
    <!-- 左侧图标 + 版本徽章 -->
    <div class="card-icon-wrap">
      <svg viewBox="0 0 24 24" width="22" height="22" stroke="#5B6AF0" stroke-width="1.8" fill="none">
        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
        <polyline points="14 2 14 8 20 8"/>
        <line x1="16" y1="13" x2="8" y2="13"/>
        <line x1="16" y1="17" x2="8" y2="17"/>
      </svg>
      <span class="version-badge">{{ versionLabel }}</span>
    </div>

    <!-- 中间：标题 + 时间 -->
    <div class="card-body">
      <div class="card-title">{{ title }}</div>
      <div class="card-meta">创建时间：{{ formatTime(timestamp) }}</div>
    </div>

    <!-- 右侧：文档预览纹理 -->
    <div class="card-preview" aria-hidden="true">
      <div class="preview-line" v-for="i in 4" :key="i" :style="{ width: [80, 60, 72, 45][i - 1] + '%' }" />
    </div>
  </div>
</template>

<style lang="scss" scoped>
.doc-version-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  background: #fff;
  border: 1px solid #e8e8f0;
  border-radius: 10px;
  cursor: pointer;
  transition: box-shadow 0.15s, border-color 0.15s;
  max-width: 420px;
  user-select: none;

  &:hover {
    border-color: #5B6AF0;
    box-shadow: 0 2px 12px rgba(91, 106, 240, 0.12);
  }

  &:active {
    transform: scale(0.99);
  }
}

.card-icon-wrap {
  position: relative;
  flex-shrink: 0;
  width: 36px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(91, 106, 240, 0.08);
  border-radius: 8px;
}

.version-badge {
  position: absolute;
  bottom: -4px;
  right: -6px;
  font-size: 9px;
  font-weight: 700;
  color: #fff;
  background: #5B6AF0;
  border-radius: 6px;
  padding: 1px 4px;
  line-height: 1.4;
  border: 1.5px solid #fff;
}

.card-body {
  flex: 1;
  min-width: 0;
}

.card-title {
  font-size: 13px;
  font-weight: 600;
  color: #1a1a2e;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.card-meta {
  margin-top: 3px;
  font-size: 11px;
  color: #999;
}

.card-preview {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex-shrink: 0;
  width: 72px;
  padding: 6px 8px;
  background: #f7f8fc;
  border-radius: 6px;
  border: 1px solid #eee;
}

.preview-line {
  height: 3px;
  background: #d8daf0;
  border-radius: 2px;
}
</style>
