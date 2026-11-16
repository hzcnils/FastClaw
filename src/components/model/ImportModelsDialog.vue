<template>
  <div v-if="visible" class="import-overlay">
    <div class="import-dialog">
      <!-- 标题栏 -->
      <div class="import-header">
        <h3>{{ $t('modelManage.importTitle') }}</h3>
        <button class="import-close-btn" @click="$emit('close')">
          <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </div>

      <!-- Phase 1: 上传区 -->
      <template v-if="!compareResult && !parsing">
        <div class="upload-area" @dragover.prevent @drop.prevent="onDrop">
          <input ref="fileInputRef" type="file" accept=".json" class="file-input-hidden" @change="onFileInputChange" />
          <div class="upload-content" @click="fileInputRef?.click()">
            <svg viewBox="0 0 24 24" width="40" height="40" stroke="var(--color-primary)" stroke-width="1.5" fill="none">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
              <polyline points="17 8 12 3 7 8"></polyline>
              <line x1="12" y1="3" x2="12" y2="15"></line>
            </svg>
            <p class="upload-hint">{{ $t('modelManage.importUploadHint') }}</p>
            <p class="upload-sub">.json</p>
          </div>
        </div>
      </template>

      <!-- 解析中 -->
      <template v-if="parsing">
        <div class="parsing-state">
          <div class="parsing-spinner"></div>
          <p>正在分析文件...</p>
        </div>
      </template>

      <!-- Phase 2: 对比结果 -->
      <template v-if="compareResult && !parsing">
        <!-- 超大量时全量覆盖 -->
        <template v-if="isLarge">
          <div class="large-warning">
            <svg viewBox="0 0 24 24" width="18" height="18" stroke="#f0ad4e" stroke-width="2" fill="none">
              <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path>
              <line x1="12" y1="9" x2="12" y2="13"></line>
              <line x1="12" y1="17" x2="12.01" y2="17"></line>
            </svg>
            <span>{{ $t('modelManage.importLargeWarning', { count: importRawData.length }) }}</span>
          </div>
          <div class="large-summary">
            <div class="summary-num-item">
              <span class="num" style="color: var(--color-primary);">{{ importRawData.length }}</span>
              <span class="label">个模型</span>
            </div>
          </div>
        </template>

        <!-- 正常量时可选择 -->
        <template v-else>
          <!-- 摘要栏 -->
          <div class="import-summary">
            <div class="summary-chip new">
              <span class="chip-num">{{ compareResult.newModels.length }}</span>
              <span class="chip-label">新增</span>
            </div>
            <div class="summary-chip conflict">
              <span class="chip-num">{{ compareResult.conflictModels.length }}</span>
              <span class="chip-label">冲突</span>
            </div>
            <div class="summary-chip existing">
              <span class="chip-num">{{ compareResult.existingModels.length }}</span>
              <span class="chip-label">已有</span>
            </div>
          </div>

          <!-- 新建供应商提示 -->
          <div v-if="compareResult.newProviderCodes.length" class="provider-warning">
            <svg viewBox="0 0 24 24" width="14" height="14" stroke="var(--color-primary)" stroke-width="2" fill="none">
              <circle cx="12" cy="12" r="10"></circle>
              <line x1="12" y1="8" x2="12" y2="12"></line>
              <line x1="12" y1="16" x2="12.01" y2="16"></line>
            </svg>
            <span>{{ $t('modelManage.importNewProviders') }}：{{ compareResult.newProviderCodes.join('、') }}</span>
          </div>

          <el-scrollbar class="import-scroll" max-height="400px">
            <el-collapse v-model="activePanels" class="import-collapse">

              <!-- 新增模型 -->
              <el-collapse-item v-if="compareResult.newModels.length" name="new">
                <template #title>
                  <div class="collapse-title">
                    <svg viewBox="0 0 24 24" width="14" height="14" stroke="#20c997" stroke-width="2" fill="none">
                      <line x1="12" y1="5" x2="12" y2="19"></line>
                      <line x1="5" y1="12" x2="19" y2="12"></line>
                    </svg>
                    <span>新增模型</span>
                    <el-tag type="success" size="small">{{ compareResult.newModels.length }}</el-tag>
                    <span class="select-all-link" @click.stop="toggleSelectAll">
                      {{ isAllNewSelected ? '取消全选' : '全选' }}
                    </span>
                  </div>
                </template>
                <div class="model-list">
                  <div v-for="m in compareResult.newModels" :key="importedModelKey(m)" class="model-row">
                    <el-checkbox
                      :model-value="selectedNewIds.has(importedModelKey(m))"
                      @change="(v: boolean) => toggleNewModel(importedModelKey(m), v)"
                    />
                    <div class="model-info">
                      <span class="model-id">{{ m.model }}</span>
                      <span class="model-type-tag">{{ m.type }}</span>
                      <span class="model-provider-tag">{{ m.provider }}</span>
                    </div>
                  </div>
                </div>
              </el-collapse-item>

              <!-- 冲突模型 -->
              <el-collapse-item v-if="compareResult.conflictModels.length" name="conflict">
                <template #title>
                  <div class="collapse-title">
                    <svg viewBox="0 0 24 24" width="14" height="14" stroke="#f0ad4e" stroke-width="2" fill="none">
                      <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path>
                      <line x1="12" y1="9" x2="12" y2="13"></line>
                    </svg>
                    <span>冲突模型</span>
                    <el-tag type="warning" size="small">{{ compareResult.conflictModels.length }}</el-tag>
                  </div>
                </template>
                <div class="conflict-list">
                  <div v-for="(c, idx) in compareResult.conflictModels" :key="String(c.json.id ?? idx)" class="conflict-item">
                    <div class="conflict-model-name">{{ c.json.model }}</div>
                    <div class="conflict-diff">
                      <div class="diff-header">
                        <span class="diff-col-label">字段</span>
                        <span class="diff-col-label">文件版</span>
                        <span class="diff-col-label">数据库版</span>
                      </div>
                      <div v-for="field in c.diffFields" :key="field" class="diff-row">
                        <span class="diff-field">{{ field }}</span>
                        <span class="diff-val json-val">{{ formatDiffVal(c.json[field]) }}</span>
                        <span class="diff-val db-val">{{ formatDiffVal(c.db[field]) }}</span>
                      </div>
                    </div>
                    <div class="conflict-choice">
                      <el-radio-group v-model="c.keepJson" size="small">
                        <el-radio :value="true">{{ $t('modelManage.importKeepJson') }}</el-radio>
                        <el-radio :value="false">{{ $t('modelManage.importKeepDb') }}</el-radio>
                      </el-radio-group>
                    </div>
                  </div>
                </div>
              </el-collapse-item>

              <!-- 已有模型 -->
              <el-collapse-item v-if="compareResult.existingModels.length" name="existing">
                <template #title>
                  <div class="collapse-title">
                    <svg viewBox="0 0 24 24" width="14" height="14" stroke="var(--text-secondary)" stroke-width="2" fill="none">
                      <polyline points="20 6 9 17 4 12"></polyline>
                    </svg>
                    <span style="color: var(--text-secondary);">已有（无变化）</span>
                    <el-tag size="small">{{ compareResult.existingModels.length }}</el-tag>
                  </div>
                </template>
                <div class="model-list">
                  <div v-for="m in compareResult.existingModels" :key="importedModelKey(m)" class="model-row readonly">
                    <div class="model-info">
                      <span class="model-id">{{ m.model }}</span>
                      <span class="model-type-tag">{{ m.type }}</span>
                      <span class="model-provider-tag">{{ m.provider }}</span>
                    </div>
                  </div>
                </div>
              </el-collapse-item>

            </el-collapse>
          </el-scrollbar>
        </template>
      </template>

      <!-- 操作按钮 -->
      <div class="import-actions">
        <button class="btn-cancel-import" @click="$emit('close')">取消</button>
        <button
          v-if="compareResult"
          class="btn-do-import"
          :disabled="importing"
          @click="executeImport"
        >
          <span v-if="importing">导入中...</span>
          <span v-else-if="isLarge">{{ $t('modelManage.importExecuteAll') }}</span>
          <span v-else>{{ $t('modelManage.importExecute') }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { useI18n } from 'vue-i18n'
import { importModelsJson, listModelsForExport, listProviders } from '@/modules/models/api'
import {
  compareImportedModels,
  parseImportedModels,
  type ImportedModel,
  type ModelCompareResult,
  type ModelConflictItem,
} from '@/modules/models/services/modelImportExport'

const props = defineProps<{ visible: boolean }>()
const emit = defineEmits<{
  close: []
  imported: []
}>()

const { t } = useI18n()

const fileInputRef = ref<HTMLInputElement | null>(null)
const parsing = ref(false)
const importing = ref(false)
const importRawData = ref<ImportedModel[]>([])
const activePanels = ref<string[]>(['new', 'conflict'])

const compareResult = ref<ModelCompareResult | null>(null)
const selectedNewIds = ref<Set<string>>(new Set())

const isLarge = computed(() => importRawData.value.length > 500)

function importedModelKey(model: ImportedModel): string {
  return model.id || `${model.provider || ''}::${model.model || ''}`
}

const isAllNewSelected = computed(() => {
  if (!compareResult.value) return false
  return compareResult.value.newModels.every(model => selectedNewIds.value.has(importedModelKey(model)))
})

watch(() => props.visible, (v) => {
  if (!v) resetState()
})

function resetState() {
  compareResult.value = null
  importRawData.value = []
  selectedNewIds.value = new Set()
  parsing.value = false
  importing.value = false
  activePanels.value = ['new', 'conflict']
}

function onFileInputChange(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0]
  if (file) processFile(file)
}

function onDrop(e: DragEvent) {
  const file = e.dataTransfer?.files?.[0]
  if (file) processFile(file)
}

async function processFile(file: File) {
  parsing.value = true
  compareResult.value = null
  importRawData.value = []
  selectedNewIds.value = new Set()

  try {
    const text = await readFileText(file)
    let parsed: ImportedModel[]
    try {
      parsed = parseImportedModels(text)
    } catch {
      ElMessage.error(t('modelManage.importParseError'))
      parsing.value = false
      return
    }

    importRawData.value = parsed

    // 获取 DB 数据
    const [dbModels, dbProviders] = await Promise.all([
      listModelsForExport(),
      listProviders(),
    ])

    const result = compareImportedModels(parsed, dbModels, dbProviders)
    compareResult.value = result

    // 默认全选新增模型
    selectedNewIds.value = new Set(result.newModels.map(importedModelKey))
  } catch (e: any) {
    ElMessage.error(`分析失败: ${e.message || String(e)}`)
  } finally {
    parsing.value = false
  }
}

function readFileText(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = (e) => resolve(e.target?.result as string)
    reader.onerror = () => reject(new Error('文件读取失败'))
    reader.readAsText(file, 'utf-8')
  })
}

function formatDiffVal(v: unknown): string {
  if (v === null || v === undefined) return '—'
  return String(v)
}

function toggleNewModel(id: string, checked: boolean) {
  const set = new Set(selectedNewIds.value)
  if (checked) set.add(id)
  else set.delete(id)
  selectedNewIds.value = set
}

function toggleSelectAll() {
  if (!compareResult.value) return
  if (isAllNewSelected.value) {
    selectedNewIds.value = new Set()
  } else {
    selectedNewIds.value = new Set(compareResult.value.newModels.map(importedModelKey))
  }
}

async function executeImport() {
  if (!compareResult.value) return
  importing.value = true

  try {
    let items: ImportedModel[]
    if (isLarge.value) {
      items = importRawData.value
    } else {
      const selectedNew = compareResult.value.newModels.filter(model => selectedNewIds.value.has(importedModelKey(model)))
      const selectedConflict = compareResult.value.conflictModels
        .filter((c: ModelConflictItem) => c.keepJson)
        .map((c: ModelConflictItem) => {
          // 还原完整 JSON 格式：从 importRawData 找原始记录
          const original = importRawData.value.find(model => model.id === c.json.id)
          return original || c.json as ImportedModel
        })
      items = [...selectedNew, ...selectedConflict]
      if (items.length === 0) {
        ElMessage.warning(t('modelManage.importNoSelection'))
        importing.value = false
        return
      }
    }

    const result = await importModelsJson(items)
    ElMessage.success(t('modelManage.importSuccess', {
      inserted: result.models_inserted,
      updated: result.models_updated,
    }))
    emit('imported')
    emit('close')
  } catch (e: any) {
    ElMessage.error(t('modelManage.importFail', { msg: e.message || String(e) }))
  } finally {
    importing.value = false
  }
}
</script>

<style scoped lang="scss">
.import-overlay {
  position: fixed;
  inset: 0;
  background: rgba(15, 23, 42, 0.15);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 200;
}

.import-dialog {
  background:
    linear-gradient(135deg, rgba(255, 255, 255, 0.06), transparent),
    var(--theme-dailog-color);
  border-radius: 18px;
  border: 1px solid var(--border-color);
  box-shadow: 0 24px 70px rgba(15, 23, 42, 0.24);
  width: 680px;
  max-width: 95vw;
  max-height: 90vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: 20px;
}

.import-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;

  h3 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: var(--text-primary);
  }
}

.import-close-btn {
  background: none;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  padding: 4px;
  border-radius: 6px;
  display: flex;
  align-items: center;

  &:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
}

/* 上传区 */
.upload-area {
  border: 2px dashed var(--border-color);
  border-radius: 12px;
  cursor: pointer;
  transition: border-color 0.2s;
  margin-bottom: 16px;

  &:hover {
    border-color: var(--color-primary);
  }
}

.upload-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px 20px;
  gap: 10px;
}

.upload-hint {
  margin: 0;
  font-size: 14px;
  color: var(--text-primary);
}

.upload-sub {
  margin: 0;
  font-size: 12px;
  color: var(--text-secondary);
}

.file-input-hidden {
  display: none;
}

/* 解析中 */
.parsing-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 60px 20px;
  gap: 16px;
  color: var(--text-secondary);
  font-size: 14px;
}

.parsing-spinner {
  width: 28px;
  height: 28px;
  border: 3px solid var(--border-color);
  border-top-color: var(--color-primary);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* 大量数据警告 */
.large-warning {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  background: rgba(240, 173, 78, 0.1);
  border: 1px solid rgba(240, 173, 78, 0.3);
  border-radius: 8px;
  font-size: 13px;
  color: #f0ad4e;
  margin-bottom: 12px;
}

.large-summary {
  display: flex;
  gap: 24px;
  padding: 12px 0;
  margin-bottom: 12px;
}

.summary-num-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;

  .num {
    font-size: 24px;
    font-weight: 700;
  }

  .label {
    font-size: 12px;
    color: var(--text-secondary);
  }
}

/* 摘要栏 */
.import-summary {
  display: flex;
  gap: 12px;
  margin-bottom: 12px;
}

.summary-chip {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border-radius: 20px;
  font-size: 13px;

  &.new {
    background: rgba(32, 201, 151, 0.12);
    color: #20c997;
  }

  &.conflict {
    background: rgba(240, 173, 78, 0.12);
    color: #f0ad4e;
  }

  &.existing {
    background: var(--bg-hover);
    color: var(--text-secondary);
  }

  .chip-num {
    font-weight: 700;
    font-size: 15px;
  }
}

/* 供应商警告 */
.provider-warning {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: rgba(var(--color-primary-rgb, 99, 102, 241), 0.08);
  border: 1px solid rgba(var(--color-primary-rgb, 99, 102, 241), 0.2);
  border-radius: 8px;
  font-size: 12px;
  color: var(--color-primary);
  margin-bottom: 10px;
}

/* 折叠区 */
.import-scroll {
  flex: 1;
  min-height: 0;
}

.import-collapse {
  border: none;

  :deep(.el-collapse-item__header) {
    background: transparent;
    border-bottom: none;
    font-size: 13px;
  }

  :deep(.el-collapse-item__content) {
    padding-bottom: 0;
  }

  :deep(.el-collapse-item__wrap) {
    background: transparent;
    border-bottom: none;
  }
}

.collapse-title {
  display: flex;
  align-items: center;
  gap: 8px;
}

.select-all-link {
  margin-left: 8px;
  font-size: 12px;
  color: var(--color-primary);
  cursor: pointer;

  &:hover {
    text-decoration: underline;
  }
}

/* 模型列表 */
.model-list {
  padding: 4px 0;
}

.model-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 5px 8px;
  border-radius: 6px;
  font-size: 13px;

  &:hover {
    background: var(--bg-hover);
  }

  &.readonly {
    padding-left: 24px;
  }
}

.model-info {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
  min-width: 0;
}

.model-id {
  font-family: monospace;
  font-size: 12px;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.model-type-tag,
.model-provider-tag {
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 4px;
  white-space: nowrap;
  flex-shrink: 0;
}

.model-type-tag {
  background: rgba(var(--color-primary-rgb, 99, 102, 241), 0.1);
  color: var(--color-primary);
}

.model-provider-tag {
  background: var(--bg-hover);
  color: var(--text-secondary);
}

/* 冲突列表 */
.conflict-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 4px 0;
}

.conflict-item {
  border: 1px solid rgba(240, 173, 78, 0.2);
  border-radius: 8px;
  padding: 10px 12px;
  background: rgba(240, 173, 78, 0.04);
}

.conflict-model-name {
  font-size: 13px;
  font-weight: 600;
  font-family: monospace;
  color: var(--text-primary);
  margin-bottom: 8px;
}

.conflict-diff {
  font-size: 12px;
  margin-bottom: 8px;
}

.diff-header {
  display: grid;
  grid-template-columns: 120px 1fr 1fr;
  gap: 8px;
  padding: 3px 0;
  border-bottom: 1px solid var(--border-color);
  margin-bottom: 4px;
}

.diff-col-label {
  font-weight: 600;
  color: var(--text-secondary);
}

.diff-row {
  display: grid;
  grid-template-columns: 120px 1fr 1fr;
  gap: 8px;
  padding: 3px 0;
  border-bottom: 1px solid var(--border-color);

  &:last-child {
    border-bottom: none;
  }
}

.diff-field {
  color: var(--text-secondary);
}

.diff-val {
  font-family: monospace;
  word-break: break-all;
}

.json-val {
  color: #20c997;
}

.db-val {
  color: var(--text-secondary);
}

.conflict-choice {
  margin-top: 8px;
}

/* 操作按钮 */
.import-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 16px;
  padding-top: 12px;
  border-top: 1px solid var(--border-color);
}

.btn-cancel-import {
  padding: 7px 20px;
  background: var(--bg-hover);
  color: var(--text-primary);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  cursor: pointer;
  font-size: 13px;
  transition: all 0.15s;

  &:hover {
    background: var(--bg-secondary);
  }
}

.btn-do-import {
  padding: 7px 20px;
  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);
  border: 1px solid transparent;
  border-radius: 6px;
  cursor: pointer;
  font-size: 13px;
  transition: all 0.15s;

  &:hover:not(:disabled) {
    background: var(--btn-primary-hover-bg, var(--color-primary));
    filter: brightness(1.1);
  }

  &:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
}
</style>
