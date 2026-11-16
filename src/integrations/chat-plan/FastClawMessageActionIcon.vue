<script setup lang="ts">
defineProps<{ name: string; active?: boolean; copied?: boolean; label?: string }>()
</script>

<template>
  <span
    class="fastclaw-message-action-icon"
    :class="{ 'is-translating': name === 'loader' && !!label }"
    :data-action-icon="name"
    aria-hidden="true"
  >
  <svg v-if="name === 'loader'" class="fastclaw-action-loader" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
  <svg v-else-if="name === 'copy' && copied" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><polyline points="20 6 9 17 4 12"/></svg>
  <svg v-else-if="name === 'copy'" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
  <svg v-else-if="name === 'custom'" viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><circle cx="5" cy="12" r="1.7"/><circle cx="12" cy="12" r="1.7"/><circle cx="19" cy="12" r="1.7"/></svg>
  <svg v-else-if="name === 'regenerate' || name === 'resend'" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>
  <svg v-else-if="name === 'edit'" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
  <svg v-else-if="name === 'translate'" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><path d="M5 8l6 6M4 14l6-6 2-3M2 5h12M7 2h1m14 20-5-10-5 10M14 18h6"/></svg>
  <svg v-else-if="name === 'favorite'" viewBox="0 0 24 24" width="14" height="14" :stroke="active ? '#faad14' : 'currentColor'" stroke-width="2" :fill="active ? '#faad14' : 'none'"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>
  <svg v-else-if="name === 'edit-as-doc'" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/></svg>
  <svg v-else-if="name === 'switch-model'" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><polyline points="16 3 21 3 21 8"/><line x1="4" y1="20" x2="21" y2="3"/><polyline points="21 16 21 21 16 21"/><line x1="15" y1="15" x2="21" y2="21"/><line x1="4" y1="4" x2="9" y2="9"/></svg>
  <svg v-else-if="name === 'quote' || name === 'feedback'" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
  <svg v-else-if="name === 'tts'" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"/></svg>
  <svg v-else-if="name === 'trace-source'" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
  <svg v-else-if="name === 'export'" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
  <svg v-else-if="name === 'like'" class="fastclaw-like-icon" :class="{ 'is-active': active }" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" :fill="active ? '#faad14' : 'none'"><path d="M14 9V5a3 3 0 0 0-3-3l-4 9v11h11.28a2 2 0 0 0 2-1.7l1.38-9a2 2 0 0 0-2-2.3H14zM7 22H4a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2h3"/></svg>
  <svg v-else-if="name === 'dislike'" class="fastclaw-dislike-icon" :class="{ 'is-active': active }" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" :fill="active ? 'var(--color-danger)' : 'none'"><path d="M10 15v4a3 3 0 0 0 3 3l4-9V2H5.72a2 2 0 0 0-2 1.7l-1.38 9a2 2 0 0 0 2 2.3H10zM17 2h2.67a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H17"/></svg>
  <svg v-else-if="name === 'branch'" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><line x1="6" y1="3" x2="6" y2="15"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="21" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/></svg>
  <svg v-else-if="name === 'delete'" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
  <span v-if="name === 'loader' && label" class="fastclaw-translating-label">{{ label }}</span>
  </span>
</template>

<style scoped>
.fastclaw-message-action-icon,
.fastclaw-message-action-icon svg {
  display: block;
  width: 16px;
  height: 16px;
  flex: 0 0 16px;
}

.fastclaw-message-action-icon.is-translating {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  width: auto;
  height: auto;
  flex: 0 0 auto;
}

.fastclaw-action-loader {
  animation: fastclaw-action-spin 0.8s linear infinite;
}

/* 点赞选中：黄色填充，描边仍走 currentColor（不挡轮廓） */
.fastclaw-like-icon.is-active,
.fastclaw-like-icon.is-active path {
  fill: #faad14 !important;
  stroke: currentColor !important;
}
.fastclaw-dislike-icon.is-active,
.fastclaw-dislike-icon.is-active path {
  fill: var(--color-danger, #ff4d4f) !important;
  stroke: currentColor !important;
}

.fastclaw-translating-label {
  display: inline-block;
  font-size: 12px;
  line-height: 1;
  white-space: nowrap;
  color: inherit;
}

@keyframes fastclaw-action-spin {
  to { transform: rotate(360deg); }
}
</style>
