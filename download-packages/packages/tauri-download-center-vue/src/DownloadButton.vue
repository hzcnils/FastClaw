<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { Download } from 'lucide-vue-next'
import { useDownloadRuntime } from './runtime'

const store = useDownloadRuntime()
const activeCount = computed(() => store.activeCount.value)
onMounted(() => { void store.initialize().catch(() => undefined) })
</script>

<template>
  <el-badge :value="activeCount" :hidden="activeCount === 0" :max="9">
    <button class="download-center-button" :class="{ active: store.panelVisible.value }" @click.stop="store.panelVisible.value = !store.panelVisible.value">
      <Download :size="16" />
    </button>
  </el-badge>
</template>

<style scoped>
.download-center-button{display:flex;align-items:center;justify-content:center;width:32px;height:32px;padding:0;border:0;border-radius:6px;color:var(--text-primary,#303133);background:transparent;cursor:pointer}.download-center-button:hover,.download-center-button.active{background:var(--bg-hover,#f5f7fa)}
</style>
