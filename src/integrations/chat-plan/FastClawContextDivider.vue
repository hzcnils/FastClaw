<script setup lang="ts">
defineProps<{ data?: { label?: string } }>()
</script>

<template>
  <div class="fastclaw-context-divider" role="separator">
    <span>{{ data?.label || '上下文已清除' }}</span>
  </div>
</template>

<style scoped>
.fastclaw-context-divider { display:flex; align-items:center; gap:12px; color:var(--text-secondary); font-size:12px; }
.fastclaw-context-divider::before,.fastclaw-context-divider::after { height:1px; flex:1; background:var(--border-color); content:""; }
</style>

