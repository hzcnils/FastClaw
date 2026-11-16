<script setup lang="ts">
import { computed } from 'vue'
import { resolveComposerIconName } from './composerIcons'
const props = defineProps<{ name: string; customIcon?: string }>()
const icon = computed(() => resolveComposerIconName(props.name))
const paths = {
  file: 'M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z M14 2v6h6 M12 18v-6m-3 3 3-3 3 3',
  task: 'M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z',
  voice: 'M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3z M19 10v2a7 7 0 0 1-14 0v-2 M12 19v3',
  clear: 'M18 6 6 18 M6 6l12 12',
  thinking: 'M9.5 4.5A3.5 3.5 0 0 0 6 8v1a3 3 0 0 0 0 6v1a3.5 3.5 0 0 0 6 2.6V5.4a3.5 3.5 0 0 0-2.5-.9z M14.5 4.5A3.5 3.5 0 0 1 18 8v1a3 3 0 0 1 0 6v1a3.5 3.5 0 0 1-6 2.6',
  planning: 'M9 6h11 M9 12h11 M9 18h11 M4 6l1 1 2-2 M4 12l1 1 2-2 M4 18l1 1 2-2',
  'context-clear': 'M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z M14 2v6h6 M9 13l6 6 M15 13l-6 6',
  'conversation-clear': 'M3 6h18 M8 6V4h8v2 M19 6l-1 15H6L5 6 M10 11v6 M14 11v6',
  export: 'M12 3v12 M7 10l5 5 5-5 M5 21h14',
  more: 'M5 12h.01 M12 12h.01 M19 12h.01',
} as const
</script>

<template>
  <span v-if="customIcon" class="jcr-composer-icon__custom" aria-hidden="true">{{ customIcon }}</span>
  <svg v-else-if="icon === 'image'" class="jcr-composer-icon" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"><path d="M14 2H2a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V3a1 1 0 0 0-1-1ZM4.5 4.5a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3ZM2 13l3.5-4 2.3 2.5 2.7-3 3.5 4.5H2Z" /></svg>
  <svg v-else-if="icon === 'send'" class="jcr-composer-icon jcr-composer-icon--send" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5 12h14M12 5l7 7-7 7" /></svg>
  <span v-else-if="icon === 'stop'" class="jcr-composer-icon__stop" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none"><circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="2.5" opacity=".25" /><path fill="currentColor" d="M4 12a8 8 0 0 1 8-8V0C5.373 0 0 5.373 0 12Z" /></svg><i /></span>
  <svg v-else class="jcr-composer-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path :d="paths[icon]" /></svg>
</template>
