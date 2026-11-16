<script setup lang="ts">
import { computed, ref } from 'vue'
import type { DownloadStatus, DownloadTask } from 'tauri-download-center-core'
import { useDownloadRuntime } from './runtime'

const props = defineProps<{ task: DownloadTask }>()
const store = useDownloadRuntime()
const downloadCenterText = store.text
const activeStatuses: DownloadStatus[] = ['pending', 'downloading', 'pausing', 'verifying', 'installing']
const pausableStatuses: DownloadStatus[] = ['pending', 'downloading']
const terminalStatuses: DownloadStatus[] = ['success', 'failed', 'cancelled']
const statusKey: Record<DownloadStatus, string> = { pending: 'statusPending', downloading: 'statusDownloading', pausing: 'statusPausing', paused: 'statusPaused', verifying: 'statusVerifying', installing: 'statusInstalling', success: 'statusSuccess', failed: 'statusFailed', cancelled: 'statusCancelled' }
const message = computed(() => props.task.error?.message || (props.task.messageKey ? downloadCenterText(props.task.messageKey, props.task.messageParams) : props.task.phase))
function formatBytes(value?:number){if(value==null||!Number.isFinite(value))return'';const units=['B','KB','MB','GB','TB'];let size=Math.max(0,value),index=0;while(size>=1024&&index<units.length-1){size/=1024;index++}return `${size.toFixed(index===0?0:size>=100?0:size>=10?1:2)} ${units[index]}`}
const sizeText=computed(()=>{const downloaded=formatBytes(props.task.downloadedBytes);const total=formatBytes(props.task.totalBytes);return total?`${downloaded} / ${total}`:downloaded?`${downloadCenterText('downloaded')} ${downloaded}`:''})
const statusText=computed(()=>{const status=`${downloadCenterText(statusKey[props.task.status])}${props.task.status==='downloading'?` ${Math.round(props.task.progress)}%`:''}`;return sizeText.value?`${status} · ${sizeText.value}`:status})
const actionPending=ref(false)
async function runAction(action:()=>Promise<unknown>){if(actionPending.value)return;actionPending.value=true;try{await action()}finally{actionPending.value=false}}
</script>

<template>
  <article class="dc-item">
    <div class="dc-item-main"><div class="dc-item-header"><span class="dc-item-name" :title="task.name">{{ task.name }}</span><span class="dc-item-status" :class="`status-${task.status}`">{{ statusText }}</span></div><el-progress :percentage="Math.round(task.progress)" :show-text="false" :status="task.status === 'failed' ? 'exception' : task.status === 'success' ? 'success' : undefined" :striped="task.status === 'downloading'" :striped-flow="task.status === 'downloading'" :duration="20" :stroke-width="5" class="dc-progress" /><div v-if="message" class="dc-item-msg" :title="message">{{ message }}</div></div>
    <div class="dc-item-actions">
      <button v-if="pausableStatuses.includes(task.status)" class="dc-icon-btn action-primary" type="button" :disabled="actionPending" :title="downloadCenterText('pause')" @click="runAction(()=>store.pause(task.id))"><svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2"><path d="M8 5v14M16 5v14" /></svg></button>
      <button v-if="task.status === 'paused'" class="dc-icon-btn action-primary" type="button" :disabled="actionPending" :title="downloadCenterText('resume')" @click="runAction(()=>store.resume(task.id))"><svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor"><path d="m8 5 11 7-11 7z" /></svg></button>
      <button v-if="task.status === 'failed'" class="dc-icon-btn action-primary" type="button" :disabled="actionPending" :title="downloadCenterText('retry')" @click="runAction(()=>store.retry(task.id))"><svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2"><path d="M1 4v6h6M3.5 15a9 9 0 1 0 .5-4.5" /></svg></button>
      <button v-if="activeStatuses.includes(task.status) || task.status === 'paused'" class="dc-icon-btn action-danger" type="button" :disabled="actionPending" :title="downloadCenterText('cancel')" @click="runAction(()=>store.cancel(task.id))"><svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2"><rect x="4" y="4" width="16" height="16" rx="2" /></svg></button>
      <button v-if="terminalStatuses.includes(task.status)" class="dc-icon-btn action-danger" type="button" :title="downloadCenterText('remove')" @click="store.remove(task.id)"><svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M19 6l-1 14H6L5 6m4 0V4h6v2M10 11v6M14 11v6" /></svg></button>
    </div>
  </article>
</template>

<style scoped>
.dc-item{display:flex;align-items:center;gap:10px;padding:10px 14px;border-bottom:1px solid var(--border-color,#dcdfe6)}.dc-item:last-child{border-bottom:0}.dc-item-main{overflow:hidden;flex:1;min-width:0}.dc-item-header{display:flex;align-items:center;gap:6px;margin-bottom:6px}.dc-item-name{overflow:hidden;flex:1;color:var(--text-primary,#303133);font-size:12px;font-weight:500;text-overflow:ellipsis;white-space:nowrap}.dc-item-status{overflow:hidden;max-width:62%;flex-shrink:0;padding:1px 5px;border-radius:4px;background:var(--bg-secondary,#f5f7fa);color:var(--text-secondary,#909399);font-size:10px;font-variant-numeric:tabular-nums;text-overflow:ellipsis;white-space:nowrap}.status-downloading,.status-verifying,.status-installing{background:color-mix(in srgb,var(--color-primary,#409eff) 10%,transparent);color:var(--color-primary,#409eff)}.status-success{background:#67c23a1a;color:#67c23a}.status-failed{background:#f56c6c1a;color:#f56c6c}.dc-progress{margin-bottom:3px}.dc-item-msg{overflow:hidden;margin-top:3px;color:var(--text-secondary,#909399);font-size:11px;line-height:1.4;text-overflow:ellipsis;white-space:nowrap}.dc-item-actions{display:flex;min-width:58px;align-items:center;justify-content:center;flex-direction:row;flex-shrink:0;flex-wrap:nowrap;gap:3px}.dc-icon-btn{display:flex;width:26px;height:26px;align-items:center;justify-content:center;flex:0 0 26px;border:0;border-radius:5px;background:transparent;color:var(--text-secondary,#909399);cursor:pointer;transition:.15s}.dc-icon-btn:hover{background:var(--bg-hover,#f5f7fa);color:var(--text-primary,#303133)}.dc-icon-btn:disabled{cursor:not-allowed;opacity:.45}.action-danger:hover{background:#f56c6c1a;color:#f56c6c}.action-primary:hover{background:color-mix(in srgb,var(--color-primary,#409eff) 10%,transparent);color:var(--color-primary,#409eff)}
</style>
