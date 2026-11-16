<script setup lang="ts">
defineProps<{
  icon?: string;
  title: string;
  description?: string;
}>();

const icons: Record<string, string> = {
  document: 'M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z',
  search: 'M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z',
  folder: 'M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z',
  database: 'M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4',
  chat: 'M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z',
  task: 'M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-6 9l2 2 4-4',
  error: 'M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z',
};
</script>

<template>
  <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; height: 100%; width: 100%; padding: 32px; box-sizing: border-box;">
    <div style="width: 64px; height: 64px; margin-bottom: 16px; flex-shrink: 0;">
      <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="var(--text-color-light-black)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path :d="icons[icon || 'document']" />
      </svg>
    </div>
    <h3 style="font-size: 18px; font-weight: 500; margin: 0 0 8px 0;">{{ title }}</h3>
    <p v-if="description" style="font-size: var(--font-size-base);  max-width: 384px; margin: 0 0 24px 0;">{{ description }}</p>
    <slot />
  </div>
</template>
