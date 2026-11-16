<script setup lang="ts">
import { X } from 'lucide-vue-next'

withDefaults(defineProps<{
  overlay?: boolean
}>(), {
  overlay: false,
})

const emit = defineEmits<{
  clear: []
}>()
</script>

<template>
  <button
    type="button"
    class="skills-search-clear"
    :class="{ 'skills-search-clear--overlay': overlay }"
    :aria-label="$t('common.clear')"
    @mousedown.prevent
    @click.stop="emit('clear')"
  >
    <X :size="15" />
  </button>
</template>
