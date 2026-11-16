<script setup lang="ts">
import { computed } from 'vue'
import { matchModelIcon, MODEL_ICON_LABELS } from './modelIcons'
import { CI_LOGO_DATA_URL, MODEL_ICON_SVGS } from './modelIconSvgs'

const props = withDefaults(defineProps<{ name?: string; modelId?: string; avatar?: string | null; size?: number }>(), {
  name: '', modelId: '', avatar: null, size: 24,
})
const iconKey = computed(() => matchModelIcon(`${props.name} ${props.modelId}`))
const label = computed(() => MODEL_ICON_LABELS[iconKey.value] || 'AI')
const iconSvg = computed(() => iconKey.value === 'CI' || iconKey.value === 'MODEL'
  ? null
  : MODEL_ICON_SVGS[iconKey.value])
const iconImage = computed(() => iconKey.value === 'CI' ? CI_LOGO_DATA_URL : null)
</script>

<template>
  <img v-if="avatar" class="jcr-model-icon" :src="avatar" alt="" :style="{ width: `${size}px`, height: `${size}px` }" />
  <img v-else-if="iconImage" class="jcr-model-icon jcr-model-icon--brand" :data-model-icon="iconKey"
    :src="iconImage" alt="" :style="{ width: `${size}px`, height: `${size}px` }" />
  <span v-else-if="iconSvg" class="jcr-model-icon jcr-model-icon--brand" :data-model-icon="iconKey"
    :style="{ width: `${size}px`, height: `${size}px` }" role="img" :aria-label="name || 'AI'" v-html="iconSvg" />
  <span v-else class="jcr-model-icon jcr-model-icon--fallback jcr-model-icon--fallback-html" :data-model-icon="iconKey"
    :style="{ width: `${size}px`, height: `${size}px`, fontSize: `${Math.max(8, Math.round(size * 0.42))}px` }"
    role="img" :aria-label="name || label">{{ label }}</span>
</template>
