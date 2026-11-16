<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { PhysicalPosition, PhysicalSize } from '@tauri-apps/api/dpi'
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event'
import ScreenshotOverlay from '@/components/chat/ScreenshotOverlay.vue'
import type { CaptureMetadata } from '@/types/screenshot'
import { createNativeScreenshotOverlay } from '@/modules/chat/nativeScreenshotOverlay'

const win = getCurrentWindow()
const overlay = createNativeScreenshotOverlay({
  async take(requestId) {
    const pending = await invoke<{ metadata: CaptureMetadata; images: string[] }>('capture_take_pending', { requestId })
    return { ...pending.metadata, monitorImages: pending.images }
  },
  async show(meta) {
    await win.setPosition(new PhysicalPosition(meta.offset_x, meta.offset_y))
    await win.setSize(new PhysicalSize(meta.width, meta.height))
    await win.setAlwaysOnTop(true)
    await win.show()
    await win.setFocus()
  },
  async hide() { await win.hide() },
  async reply(confirm, payload) {
    try { await emit(confirm ? 'screenshot-confirm' : 'screenshot-cancel', payload) }
    catch { await invoke('show_main_window').catch(() => {}) }
  },
})
const data = overlay.data
const overlayEvents = overlay.events
const unlisteners: UnlistenFn[] = []
let disposed = false
let ready = false
function retain(unlisten: UnlistenFn) { if (disposed) unlisten(); else unlisteners.push(unlisten) }

onMounted(async () => {
  try {
    retain(await listen<string>('screenshot-run', event => {
      if (!disposed && typeof event.payload === 'string') void overlay.run(event.payload).catch(() => {})
    }))
    retain(await listen<string>('screenshot-probe', event => {
      if (!disposed && ready) void emit('screenshot-ready', event.payload).catch(() => {})
    }))
    retain(await win.onCloseRequested(event => {
      event.preventDefault()
      void overlay.cancel().catch(() => {})
    }))
    if (!disposed) { ready = true; await emit('screenshot-ready', null) }
  } catch {
    await invoke('show_main_window').catch(() => {})
  }
  // Prewarming never consumes pending pixels. Only an explicit request may start capture.
})
onUnmounted(() => {
  disposed = true
  unlisteners.splice(0).forEach(fn => fn())
  void overlay.dispose().catch(() => {})
})
</script>

<template>
  <ScreenshotOverlay
    v-if="data"
    :key="data.request_id ?? undefined"
    :imageWidth="data.width"
    :imageHeight="data.height"
    :imageOffsetX="data.offset_x"
    :imageOffsetY="data.offset_y"
    :monitors="data.monitors"
    :monitorImages="data.monitorImages"
    :windows="data.windows"
    :manageWindow="false"
    v-on="overlayEvents"
  />
</template>
