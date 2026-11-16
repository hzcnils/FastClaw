<script setup lang="ts">
import { onMounted } from 'vue'
import { useDownloadRuntime } from './runtime'
import DownloadTaskItem from './DownloadTaskItem.vue'

const store = useDownloadRuntime()
const downloadCenterText = store.text
onMounted(() => { void store.initialize().catch(() => undefined) })
</script>

<template>
  <Teleport to="body">
    <Transition name="download-panel">
      <section v-if="store.panelVisible.value" class="download-center-panel">
        <header class="dc-header">
          <span class="dc-title">{{ downloadCenterText('title') }}</span>
          <span v-if="store.activeCount.value > 0" class="dc-active-hint">{{ downloadCenterText('active', { n: store.activeCount.value }) }}</span>
          <button class="dc-icon-btn dc-close" type="button" :title="downloadCenterText('close')" @click="store.closePanel()"><svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round"><path d="M18 6 6 18M6 6l12 12" /></svg></button>
        </header>
        <div class="dc-body">
          <div v-if="store.tasks.value.length === 0" class="dc-empty"><svg viewBox="0 0 24 24" width="32" height="32" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><path d="m7 10 5 5 5-5M12 15V3" /></svg><span>{{ downloadCenterText('empty') }}</span></div>
          <DownloadTaskItem v-for="task in store.tasks.value" :key="task.id" :task="task" />
        </div>
      </section>
    </Transition>
  </Teleport>
</template>

<style scoped>
.download-center-panel{position:fixed;top:56px;right:16px;z-index:2000;display:flex;width:360px;max-height:480px;overflow:hidden;flex-direction:column;border:1px solid var(--border-color,#dcdfe6);border-radius:8px;background:var(--theme-dailog-color,var(--dialog-background-color,#fff));box-shadow:0 4px 24px #0000002e}.dc-header{display:flex;align-items:center;flex-shrink:0;gap:8px;padding:10px 14px;border-bottom:1px solid var(--border-color,#dcdfe6)}.dc-title{font-size:13px;font-weight:600;color:var(--text-primary,#303133)}.dc-active-hint{padding:2px 7px;border-radius:10px;background:color-mix(in srgb,var(--color-primary,#409eff) 10%,transparent);color:var(--color-primary,#409eff);font-size:11px}.dc-icon-btn{display:flex;width:26px;height:26px;align-items:center;justify-content:center;flex-shrink:0;border:0;border-radius:5px;background:transparent;color:var(--text-secondary,#909399);cursor:pointer;transition:.15s}.dc-icon-btn:hover{background:var(--bg-hover,#f5f7fa);color:var(--text-primary,#303133)}.dc-close{margin-left:auto}.dc-body{overflow-y:auto;flex:1;padding:2px 0}.dc-empty{display:flex;align-items:center;justify-content:center;flex-direction:column;padding:32px 16px;color:var(--text-secondary,#909399);font-size:13px}.dc-empty svg{margin-bottom:8px;opacity:.3}.download-panel-enter-active,.download-panel-leave-active{transition:opacity .2s ease,transform .2s ease}.download-panel-enter-from,.download-panel-leave-to{opacity:0;transform:translateY(-8px) scale(.97)}
</style>
