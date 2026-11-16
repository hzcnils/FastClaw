<script setup lang="ts">
// FastClaw · 插件市场
</script>
<template>
  <div class="page-view">
    <div class="page-header"><h2>🛒 {{ $t('settings.pluginMarket') }}</h2></div>
    <div class="page-body">
      <div class="placeholder">
        <span style="font-size:48px">🛒</span>
        <p>{{ $t('market.placeholder') }}</p>
      </div>
    </div>
  </div>
</template>
<style lang="scss" scoped>
.page-view { height: 100%; display: flex; flex-direction: column; }
.page-header { margin-bottom: 16px; h2 { font-size: 18px; } }
.page-body { flex: 1; display: flex; align-items: center; justify-content: center; }
.placeholder { text-align: center; color: var(--text-secondary); p { margin-top: 12px; } }
</style>
