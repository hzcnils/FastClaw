<script setup lang="ts">
import { ref, computed } from 'vue'
import { ElMessage } from 'element-plus'
import { useI18n } from 'vue-i18n'
import { executeSync as runSync } from '@/modules/models/api'
import type { SyncPreview, SyncResult } from '@/modules/models/types/contracts'

const { t } = useI18n()

const props = defineProps<{
  visible: boolean
  preview: SyncPreview | null
}>()

const emit = defineEmits<{
  (e: 'update:visible', value: boolean): void
  (e: 'synced', result: any): void
}>()

const syncing = ref(false)
const activeSyncPanels = ref<string[]>(['reused', 'new'])

const textFormatFn = (str: string) => str.split(':').pop()

function close() {
  emit('update:visible', false)
}

async function executeSync() {
  syncing.value = true
  try {
    const result: SyncResult = await runSync()
    emit('synced', result)
    close()
  } catch (e: any) {
    console.error('Sync execute failed:', e)
    ElMessage.error(t('modelManage.syncFail', { msg: e.message || String(e) }))
  } finally {
    syncing.value = false
  }
}
</script>

<template>
  <div v-if="visible" class="overlay">
    <div class="dialog sync-preview-dialog">
      <div class="dialog-header">
        <h3>{{ $t('modelManage.syncPreviewTitle') }}</h3>
        <button class="close-btn" @click="close">
          <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </div>

      <div class="sync-summary">
        <div class="summary-item">
          <div class="summary-num" style="color: var(--color-primary);">{{ preview?.reused_providers?.length || 0 }}</div>
          <div class="summary-label">{{ $t('modelManage.reusedProviders') }}</div>
        </div>
        <div class="summary-item">
          <div class="summary-num" style="color: #20c997;">{{ preview?.new_providers?.length || 0 }}</div>
          <div class="summary-label">{{ $t('modelManage.newProviders') }}</div>
        </div>
        <div class="summary-item">
          <div class="summary-num" style="color: #20c997;">{{ preview?.models_insert || 0 }}</div>
          <div class="summary-label">{{ $t('modelManage.newModels') }}</div>
        </div>
        <div class="summary-item">
          <div class="summary-num" style="color: var(--color-primary);">{{ preview?.models_update || 0 }}</div>
          <div class="summary-label">{{ $t('modelManage.updateModels_count') }}</div>
        </div>
      </div>

      <el-scrollbar class="sync-detail-scroll" :max-height="360">
        <el-collapse v-model="activeSyncPanels" class="sync-collapse">
          <el-collapse-item v-if="preview?.reused_providers?.length" name="reused">
            <template #title>
              <div class="collapse-title">
                <svg viewBox="0 0 24 24" width="16" height="16" stroke="#20c997" stroke-width="2" fill="none" class="collapse-icon">
                  <polyline points="20 6 9 17 4 12"></polyline>
                </svg>
                <span>{{ $t('modelManage.reusedProviders') }}</span>
                <el-tag type="success">{{ preview.reused_providers.length }}</el-tag>
              </div>
            </template>
            <div class="collapse-content">
              <div v-for="r in preview.reused_providers" :key="r.json_name" class="match-row">
                <div class="match-names">
                  <span class="match-from">{{ r.json_name }}</span>
                  <svg viewBox="0 0 24 24" width="14" height="14" stroke="var(--text-secondary)" stroke-width="2" fill="none" class="match-arrow">
                    <line x1="5" y1="12" x2="19" y2="12"></line>
                    <polyline points="12 5 19 12 12 19"></polyline>
                  </svg>
                  <span class="match-to">{{ r.db_display_name || r.db_name }}</span>
                </div>
                <span class="match-tag" :class="{ 'match-l2': r.match_level?.startsWith('L2') }">
                  {{ textFormatFn(r.match_level) }}
                </span>
              </div>
            </div>
          </el-collapse-item>

          <el-collapse-item v-if="preview?.new_providers?.length" name="new">
            <template #title>
              <div class="collapse-title">
                <svg viewBox="0 0 24 24" width="16" height="16" stroke="#20c997" stroke-width="2" fill="none" class="collapse-icon">
                  <line x1="12" y1="5" x2="12" y2="19"></line>
                  <line x1="5" y1="12" x2="19" y2="12"></line>
                </svg>
                <span>{{ $t('modelManage.newProviders') }}</span>
                <el-tag type="success">{{ preview.new_providers.length }}</el-tag>
              </div>
            </template>
            <div class="collapse-content">
              <div v-for="n in preview.new_providers" :key="n.name" class="new-row">
                <span class="new-name">{{ n.name }}</span>
                <span class="new-count">{{ n.model_count }} {{ $t('modelManage.modelCount') }}</span>
              </div>
            </div>
          </el-collapse-item>

          <el-collapse-item v-if="preview?.base_url_conflicts?.length" name="conflict">
            <template #title>
              <div class="collapse-title">
                <svg viewBox="0 0 24 24" width="16" height="16" stroke="#f0ad4e" stroke-width="2" fill="none" class="collapse-icon">
                  <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path>
                  <line x1="12" y1="9" x2="12" y2="13"></line>
                  <line x1="12" y1="17" x2="12.01" y2="17"></line>
                </svg>
                <span>{{ $t('modelManage.urlConflicts') }}</span>
                <el-tag type="warning">{{ preview.base_url_conflicts.length }}</el-tag>
              </div>
            </template>
            <div class="collapse-content">
              <div v-for="c in preview.base_url_conflicts" :key="c.provider_name" class="conflict-item">
                <div class="conflict-provider">{{ c.provider_display_name || c.provider_name }}</div>
                <div v-for="u in c.urls" :key="u" class="conflict-url">{{ u }}</div>
              </div>
            </div>
          </el-collapse-item>

          <el-collapse-item v-if="preview?.merge_hints?.length" name="merge">
            <template #title>
              <div class="collapse-title">
                <svg viewBox="0 0 24 24" width="16" height="16" stroke="var(--color-primary)" stroke-width="2" fill="none" class="collapse-icon">
                  <circle cx="12" cy="12" r="10"></circle>
                  <line x1="12" y1="16" x2="12" y2="12"></line>
                  <line x1="12" y1="8" x2="12.01" y2="8"></line>
                </svg>
                <span>{{ $t('modelManage.mergeHints') }}</span>
                <el-tag type="info">{{ preview.merge_hints.length }}</el-tag>
              </div>
            </template>
            <div class="collapse-content">
              <div v-for="h in preview.merge_hints" :key="h.db_name" class="merge-row">
                <span class="merge-from">{{ h.json_names.join(', ') }}</span>
                <svg viewBox="0 0 24 24" width="14" height="14" stroke="var(--text-secondary)" stroke-width="2" fill="none" class="match-arrow">
                  <line x1="5" y1="12" x2="19" y2="12"></line>
                  <polyline points="12 5 19 12 12 19"></polyline>
                </svg>
                <span class="merge-to">{{ h.db_display_name || h.db_name }}</span>
              </div>
            </div>
          </el-collapse-item>
        </el-collapse>
      </el-scrollbar>

      <div class="actions">
        <el-button class="btn-cancel" @click="close">{{ $t('common.cancel') }}</el-button>
        <el-button class="btn-confirm" type="primary" @click="executeSync" :loading="syncing">{{ $t('common.confirm') }}</el-button>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* ─── 弹窗基础（复刻 dialog-common）─── */
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(15, 23, 42, 0.15);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.dialog {
  background: var(--theme-dailog-color);
  padding: 20px;
  border-radius: 18px;
  width: 400px;
  border: 1px solid var(--border-color);
  box-shadow: 0 24px 70px rgba(15, 23, 42, 0.24);
}
.dialog-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 15px;
}
.dialog-header h3 {
  font-size: var(--font-size-lg);
  font-weight: 600;
  margin: 0;
}
.close-btn {
  width: 28px;
  height: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  border-radius: 5px;
  cursor: pointer;
  color: var(--text-secondary);
  padding: 0;
  transition: background-color 0.18s ease, color 0.18s ease;
}
.close-btn:hover {
  background: var(--dropdown-hover-color, var(--bg-hover)) !important;
  color: var(--text-primary);
}
.actions {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 15px;
  margin-top: 15px;
}
.btn-cancel,
.btn-confirm {
  display: inline-flex !important;
  align-items: center !important;
  justify-content: center !important;
  gap: 0 !important;
  width: 64px !important;
  height: 37px !important;
  min-width: 64px !important;
  min-height: 37px !important;
  padding: 0 !important;
  margin: 0 !important;
  box-sizing: border-box !important;
  border-radius: 4px;
  cursor: pointer;
  font-size: var(--font-size-md);
  font-weight: 500;
}
.actions .el-button + .el-button {
  margin-left: 0 !important;
}
.btn-confirm {
  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);
  border: 1px solid var(--btn-primary-border, transparent);
}
.btn-confirm:hover {
  background: var(--btn-primary-hover-bg, var(--color-primary));
  color: var(--btn-primary-hover-text, #fff);
  border-color: var(--btn-primary-hover-border, transparent);
}
.btn-cancel {
  color: var(--btn-secondary-text) !important;
  background: var(--cancel-btn-color) !important;
  border: 1px solid var(--btn-secondary-border) !important;
}
.btn-cancel:hover {
  background: var(--cancel-btn-hover-color) !important;
  color: var(--btn-secondary-hover-text) !important;
  border-color: var(--btn-secondary-hover-border) !important;
}

/* ─── 同步预览弹窗（精确复刻原 ModelsView.vue）─── */
.sync-preview-dialog {
  width: 540px;
  height: 60vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: 20px;
}

.sync-summary {
  display: flex;
  gap: 10px;
  padding: 0 4px;
  margin-bottom: 4px;
}

.summary-item {
  flex: 1;
  padding: 12px 8px 10px;
  background: var(--bg-hover);
  border-radius: 10px;
  text-align: center;
  transition: transform 0.15s;
}
.summary-item:hover {
  transform: translateY(-1px);
}

.summary-num {
  font-size: 22px;
  font-weight: 700;
  line-height: 1.2;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}

.summary-label {
  font-size: 11px;
  color: var(--text-secondary);
  margin-top: 4px;
}

.sync-detail-scroll {
  flex: 1;
  min-height: 0;
  margin: 0 -4px;
}

.sync-collapse {
  border: none;
}
.sync-collapse :deep(.el-collapse-item) {
  margin-bottom: 2px;
}
.sync-collapse :deep(.el-collapse-item__header) {
  height: 38px;
  padding: 0 12px;
  border: none;
  background: transparent;
  border-radius: 6px;
  font-size: var(--font-size-base);
  transition: background 0.15s;
}
.sync-collapse :deep(.el-collapse-item__header):hover {
  background: var(--bg-hover);
}
.sync-collapse :deep(.el-collapse-item__wrap) {
  border: none;
  background: transparent;
}
.sync-collapse :deep(.el-collapse-item__content) {
  padding: 0 12px 8px 12px;
}

.collapse-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 600;
  font-size: var(--font-size-sm);
  color: var(--text-primary);
}
.collapse-title .collapse-icon {
  flex-shrink: 0;
}
.collapse-title :deep(.el-tag) {
  margin-left: auto;
  margin-right: 10px;
}

.collapse-content {
  padding-top: 4px;
}

.match-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  border-radius: 6px;
  font-size: var(--font-size-sm);
  transition: background 0.12s;
}
.match-row:hover {
  background: var(--bg-hover);
}

.match-names {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.match-from {
  color: var(--text-primary);
  font-weight: 500;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12px;
}

.match-arrow {
  flex-shrink: 0;
}

.match-to {
  color: var(--color-primary);
  font-weight: 500;
}

.match-tag {
  font-size: 11px;
  padding: 1px 8px;
  border-radius: 3px;
  background: rgba(32, 201, 151, 0.1);
  color: #20c997;
  white-space: nowrap;
  flex-shrink: 0;
}
.match-tag.match-l2 {
  background: rgba(240, 173, 78, 0.12);
  color: #e6a23c;
}

.new-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  border-radius: 6px;
  font-size: var(--font-size-sm);
  transition: background 0.12s;
}
.new-row:hover {
  background: var(--bg-hover);
}

.new-name {
  color: var(--text-primary);
  font-weight: 500;
}

.new-count {
  font-size: 11px;
  color: var(--text-secondary);
  white-space: nowrap;
}

.conflict-item {
  padding: 8px 10px;
  margin-bottom: 6px;
  background: rgba(240, 173, 78, 0.06);
  border-left: 3px solid #f0ad4e;
  border-radius: 0 6px 6px 0;
  font-size: var(--font-size-sm);
}

.conflict-provider {
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 4px;
}

.conflict-url {
  color: var(--text-secondary);
  font-size: 11px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  word-break: break-all;
  line-height: 1.5;
}

.merge-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: 6px;
  font-size: var(--font-size-sm);
  background: rgba(99, 140, 255, 0.04);
  border-left: 3px solid var(--color-primary);
  border-radius: 0 6px 6px 0;
  margin-bottom: 4px;
  transition: background 0.12s;
}
.merge-row:hover {
  background: rgba(99, 140, 255, 0.08);
}

.merge-from {
  color: var(--text-primary);
  font-weight: 500;
  font-size: 12px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}

.merge-to {
  color: var(--color-primary);
  font-weight: 600;
}
</style>
