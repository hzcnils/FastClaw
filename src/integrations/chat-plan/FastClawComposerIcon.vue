<script setup lang="ts">
import { DocumentDelete, Microphone, Plus } from '@element-plus/icons-vue'
import { FileUp } from 'lucide-vue-next'
import SvgIcon from '@/components/svgIcon/index.vue'

defineProps<{ name: string }>()
</script>

<template>
  <span class="fastclaw-composer-icon" :class="[
    `fastclaw-composer-icon--${name}`,
    { 'fastclaw-composer-icon--filled': ['image', 'screenshot', 'thinking', 'planning'].includes(name) },
  ]" aria-hidden="true">
    <SvgIcon v-if="name === 'image'" icon-name="image" />
    <FileUp v-else-if="name === 'document'" />
    <SvgIcon v-else-if="name === 'screenshot'" icon-name="screenshot" />
    <SvgIcon v-else-if="name === 'thinking'" icon-name="think" />
    <SvgIcon v-else-if="name === 'planning'" icon-name="taskPlan" />
    <DocumentDelete v-else-if="name === 'clear-context'" />
    <Microphone v-else-if="name === 'voice'" />
    <Plus v-else />
  </span>
</template>

<style scoped>
.fastclaw-composer-icon,
.fastclaw-composer-icon :deep(svg) {
  display: block;
  width: 17px;
  height: 17px;
  flex: 0 0 17px;
}

.fastclaw-composer-icon--skill,
.fastclaw-composer-icon--skill :deep(svg),
.fastclaw-composer-icon--document,
.fastclaw-composer-icon--document :deep(svg),
.fastclaw-composer-icon--planning,
.fastclaw-composer-icon--planning :deep(svg) {
  width: 18px;
  height: 18px;
  flex-basis: 18px;
}

.fastclaw-composer-icon--filled :deep(svg),
.fastclaw-composer-icon--filled :deep(svg path),
.fastclaw-composer-icon--filled :deep(svg g) {
  color: inherit;
  fill: currentColor !important;
}
</style>
