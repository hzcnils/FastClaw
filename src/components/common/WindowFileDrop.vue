<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch } from 'vue'
import { useRoute } from 'vue-router'
import { isTauri } from '@tauri-apps/api/core'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { fileDropRegistry } from '@/utils/fileDropRegistry'

const route = useRoute()
const hovering = ref(false)
let disposed = false
let unlisten: (() => void) | undefined
watch(() => route.path, () => { hovering.value = false })
onMounted(async () => {
  if (!isTauri()) return
  try {
    const stop = await getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === 'enter' || payload.type === 'over') {
        hovering.value = fileDropRegistry.accepts(route.path)
      } else {
        hovering.value = false
        if (payload.type === 'drop') void fileDropRegistry.dispatch(route.path, payload.paths)
      }
    })
    if (disposed) stop()
    else unlisten = stop
  } catch (error) { console.warn('File drop listener unavailable', error) }
})
onBeforeUnmount(() => { disposed = true; unlisten?.() })
</script>

<template>
  <Teleport to="body">
    <div v-if="hovering" class="window-file-drop" role="status">
      <span>{{ $t('fileDrop.hint') }}</span>
    </div>
  </Teleport>
</template>

<style scoped>
.window-file-drop { position: fixed; inset: 52px 12px 12px; z-index: 10000; pointer-events: none; border: 2px dashed var(--color-primary); border-radius: 16px; display: grid; place-items: center; background: color-mix(in srgb, var(--color-primary) 12%, transparent); }
.window-file-drop span { padding: 16px 24px; border-radius: 12px; color: var(--text-primary); background: var(--bg-primary); box-shadow: 0 4px 24px #0003; }
</style>
