<script setup lang="ts">
import { onErrorCaptured, ref, watch } from 'vue'

const props = defineProps<{ resetKey?: string }>()
const failed = ref(false)
onErrorCaptured(() => { failed.value = true; return false })
watch(() => props.resetKey, () => { failed.value = false })
</script>

<template><slot v-if="!failed" /><slot v-else name="fallback" /></template>
