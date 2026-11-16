<script setup lang="ts">
withDefaults(defineProps<{
  size?: 'sm' | 'md' | 'lg';
  text?: string;
  fullPage?: boolean;
}>(), {
  size: 'md',
});
</script>

<template>
  <div
    class="fc-loading-spinner"
    :class="[
      `fc-loading-spinner--${size}`,
      fullPage ? 'fc-loading-spinner--fullpage' : '',
    ]"
  >
    <div class="fc-loading-spinner__ring" aria-hidden="true"></div>
    <span v-if="text" class="fc-loading-spinner__text">{{ text }}</span>
  </div>
</template>

<style scoped>
.fc-loading-spinner {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
}

.fc-loading-spinner--fullpage {
  position: fixed;
  inset: 0;
  z-index: 50;
  background: rgba(15, 23, 42, 0.4);
  backdrop-filter: blur(4px);
}

.fc-loading-spinner__ring {
  box-sizing: border-box;
  border-radius: 50%;
  border-style: solid;
  border-color: var(--border-color, rgba(148, 163, 184, 0.35));
  border-top-color: var(--color-primary, #4b6bfc);
  animation: fc-spinner-spin 0.8s linear infinite;
}

.fc-loading-spinner--sm .fc-loading-spinner__ring {
  width: 20px;
  height: 20px;
  border-width: 2px;
}

.fc-loading-spinner--md .fc-loading-spinner__ring {
  width: 32px;
  height: 32px;
  border-width: 3px;
}

.fc-loading-spinner--lg .fc-loading-spinner__ring {
  width: 48px;
  height: 48px;
  border-width: 3px;
}

.fc-loading-spinner__text {
  font-size: var(--font-size-sm, 13px);
  color: var(--text-secondary, #64748b);
}

@keyframes fc-spinner-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
