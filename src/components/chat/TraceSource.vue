<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElDivider, ElRate, ElInput, ElTable, ElTableColumn, ElTag, ElScrollbar, ElPopover, ElButton, ElIcon } from 'element-plus'
import { Close, Download, View } from '@element-plus/icons-vue'
import type { Message } from '@/types'

const { t } = useI18n()

const props = defineProps<{
  message: Message
}>()

const emit = defineEmits<{
  'quality-submit': [messageId: string, score: number, content: string]
  'close': []
}>()

const graphColumns = computed(() => [
  { label: t('chat.graphStartEntity'), prop: 'sourseName' },
  { label: t('chat.graphRelation'), prop: 'relation', width: '170' },
  { label: t('chat.graphTargetEntity'), prop: 'targetName' },
])

const traceData = computed(() => props.message.trace_source)

function sliceDescText(text: string): string {
  if (!text) return ''
  return text.length > 12 ? text.slice(0, 12) + '...' : text
}

// 本地向量库溯源
function toggleLocal() {
  if (!traceData.value || !traceData.value.local) {
    ElMessage.warning(t('chat.noKbLocalTrace'))
    return
  }
  if (props.message.trace_source?.local) {
    props.message.trace_source.local.show = !props.message.trace_source.local.show
  }
}

// 关联文档溯源
function toggleFile() {
  if (!traceData.value || !traceData.value.file) {
    ElMessage.warning(t('chat.noKbDocTrace'))
    return
  }
  if (props.message.trace_source?.file) {
    props.message.trace_source.file.show = !props.message.trace_source.file.show
  }
}

// 知识图谱溯源（暂时保留入口但暂不可用）
function toggleGraph() {
  ElMessage.info(t('chat.graphNotAvailable'))
}

// 结果质量反馈
function toggleResult() {
  if (props.message.trace_source?.result) {
    const tr = props.message.trace_source.result
    if (tr.isQuery === false && !tr.isSubmit) {
      // 首次打开，直接展示
      tr.isQuery = true
      tr.show = true
    } else {
      tr.show = !tr.show
    }
  }
}

// 综合分析
function toggleAnalysis() {
  if (props.message.trace_source?.analysis) {
    props.message.trace_source.analysis.show = !props.message.trace_source.analysis.show
  }
}

// 提交质量反馈
function submitQualityFeedback() {
  const tr = props.message.trace_source?.result
  if (!tr || tr.loading || tr.isSubmit) return
  emit('quality-submit', props.message.id, tr.rate, tr.content)
}

// 下载关联文档
function downloadFile(file: any) {
  ElMessage.info(t('chat.fileDownloadNotImplemented'))
}

// 预览文件
function previewFile(file: any) {
  ElMessage.info(t('chat.filePreviewNotImplemented'))
}

const hasAnyTrace = computed(() => {
  const ts = traceData.value
  if (!ts) return false
  return !!(ts.local?.data?.length || ts.file?.data?.length || ts.graph?.graphTable?.length || ts.result?.isSubmit)
})
</script>

<template>
  <div v-if="traceData" class="trace-source">
    <!-- ===== 知识图谱溯源 ===== -->
    <div v-if="traceData.graph" class="trace-section">
      <el-divider class="divider-line" v-show="traceData.graph.show" />
      <div v-show="traceData.graph.show" class="trace-box">
        <div class="box-title">
          <div class="box-title-left">
            <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none">
              <circle cx="12" cy="12" r="3" />
              <circle cx="5" cy="5" r="1.5" />
              <circle cx="19" cy="5" r="1.5" />
              <circle cx="5" cy="19" r="1.5" />
              <circle cx="19" cy="19" r="1.5" />
              <line x1="8" y1="8" x2="9.5" y2="9.5" />
              <line x1="16" y1="8" x2="14.5" y2="9.5" />
              <line x1="8" y1="16" x2="9.5" y2="14.5" />
              <line x1="16" y1="16" x2="14.5" y2="14.5" />
            </svg>
            <span>{{ t('chat.entityTrace') }}</span>
          </div>
          <el-icon :size="16" @click="traceData.graph.show = false" class="close-btn">
            <Close />
          </el-icon>
        </div>
        <div class="box-content">
          <el-table v-if="traceData.graph.graphTable?.length" :data="traceData.graph.graphTable" border stripe
            height="260">
            <el-table-column v-for="(col, colIdx) in graphColumns" :key="colIdx" :label="col.label" :prop="col.prop"
              :width="col.width">
              <template v-if="col.prop === 'sourseName'" #default="{ row }">
                <span>{{ row.head }}</span>
                <el-tag v-if="row.headType" type="danger" size="small">{{ row.headType }}</el-tag>
                <el-tag v-if="row.headDesc" type="info" size="small" :title="row.headDesc">{{
                  sliceDescText(row.headDesc) }}</el-tag>
              </template>
              <template v-else-if="col.prop === 'targetName'" #default="{ row }">
                <span>{{ row.tail }}</span>
                <el-tag v-if="row.tailType" type="danger" size="small">{{ row.tailType }}</el-tag>
                <el-tag v-if="row.tailDesc" type="info" size="small" :title="row.tailDesc">{{
                  sliceDescText(row.tailDesc) }}</el-tag>
              </template>
            </el-table-column>
          </el-table>
          <div v-else class="box-empty">{{ t('chat.noEntityData') }}</div>
        </div>
      </div>
    </div>

    <!-- ===== 本地向量库溯源 ===== -->
    <div v-if="traceData.local" class="trace-section">
      <el-divider class="divider-line" v-show="traceData.local.show" />
      <div v-show="traceData.local.show" class="trace-box">
        <div class="box-title">
          <div class="box-title-left">
            <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none">
              <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
              <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
            </svg>
            <span>{{ t('chat.traceLocal') }}</span>
          </div>
          <el-icon :size="16" @click="traceData.local.show = false" class="close-btn">
            <Close />
          </el-icon>
        </div>
        <div class="box-content">
          <div v-if="traceData.local.data.length">
            <div v-for="(item, idx) in traceData.local.data" :key="idx" class="trace-item">
              <span class="trace-item-index">{{ idx + 1 }}、</span>
              <span class="trace-item-text">{{ item.snippet }}</span>
            </div>
          </div>
          <div v-else class="box-empty">{{ t('chat.noLocalTraceData') }}</div>
        </div>
      </div>
    </div>

    <!-- ===== 关联文档溯源 ===== -->
    <div v-if="traceData.file" class="trace-section">
      <el-divider class="divider-line" v-show="traceData.file.show" />
      <div v-show="traceData.file.show" class="trace-box">
        <div class="box-title">
          <div class="box-title-left">
            <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
              <polyline points="14 2 14 8 20 8" />
              <line x1="16" y1="13" x2="8" y2="13" />
              <line x1="16" y1="17" x2="8" y2="17" />
            </svg>
            <span>{{ t('chat.traceFile') }}</span>
          </div>
          <el-icon :size="16" @click="traceData.file.show = false" class="close-btn">
            <Close />
          </el-icon>
        </div>
        <el-scrollbar>
          <div class="file-tags">
            <el-popover v-for="(file, fileIdx) in traceData.file.data" :key="fileIdx" placement="top-start" :width="150"
              trigger="hover">
              <template #reference>
                <el-tag :type="traceData.file.current === fileIdx ? 'primary' : 'info'" effect="plain"
                  @click="traceData.file.current = fileIdx" class="file-tag-item">
                  {{ file.fileName }}
                </el-tag>
              </template>
              <el-button type="primary" link :icon="Download"  @click="downloadFile(file)">{{ t('chat.traceDownload') }}</el-button>
              <el-button type="primary" link :icon="View" @click="previewFile(file)">{{ t('chat.tracePreview') }}</el-button>
            </el-popover>
          </div>
        </el-scrollbar>
        <div class="box-content" v-if="traceData.file.current !== -1">
          <div v-for="(item, idx) in traceData.file.data[traceData.file.current]?.snippet || []" :key="idx"
            class="trace-item">
            <span class="trace-item-index">{{ idx + 1 }}、</span>
            <span class="trace-item-text">{{ item.snippet }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- ===== 结果质量反馈 ===== -->
    <div v-if="traceData.result" class="trace-section">
      <el-divider class="divider-line" v-show="traceData.result.show" />
      <div v-show="traceData.result.show" class="trace-box">
        <div class="box-title">
          <div class="box-title-left">
            <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none">
              <polygon
                points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
            </svg>
            <span>{{ t('chat.traceResult') }}</span>
          </div>
          <el-icon :size="16" @click="traceData.result.show = false" class="close-btn">
            <Close />
          </el-icon>
        </div>
        <div class="box-content">
          <div class="quality-hint">{{ t('chat.feedbackHint') }}</div>
          <el-rate v-model="traceData.result.rate" show-score :max="10" allow-half
            :disabled="traceData.result.isSubmit || traceData.result.loading" />
          <el-input type="textarea" :readonly="traceData.result.isSubmit || traceData.result.loading" v-model="traceData.result.content" :rows="4"
            :placeholder="t('chat.feedbackPlaceholder')" class="quality-input" />
          <div class="btn-box">
            <el-button v-if="!traceData.result.isSubmit" type="primary" :loading="traceData.result.loading"
              @click="submitQualityFeedback" size="small">{{ t('chat.submitFeedback') }}</el-button>
            <el-tag v-else type="success" size="small">{{ t('chat.feedbackSubmittedTag') }}</el-tag>
          </div>
        </div>
      </div>
    </div>

    <!-- ===== 溯源信息综合分析 ===== -->
    <div v-if="traceData.analysis" class="trace-section">
      <el-divider class="divider-line" v-show="traceData.analysis.show" />
      <div v-show="traceData.analysis.show" class="trace-box">
        <div class="box-title">
          <div class="box-title-left">
            <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none">
              <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
            </svg>
            <span>{{ t('chat.traceAnalysis') }}</span>
          </div>
          <el-icon :size="16" @click="traceData.analysis.show = false" class="close-btn">
            <Close />
          </el-icon>
        </div>
        <div class="box-content">
          <div v-if="traceData.analysis.summary" class="analysis-summary markdown-body">
            <div class="summary-section" v-if="traceData.local && traceData.local.data.length">
              <h5>{{ t('chat.vectorDbTrace') }}</h5>
              <p>{{ t('chat.retrievedFragments', { n: traceData.local.data.length }) }}</p>
            </div>
            <div class="summary-section" v-if="traceData.file && traceData.file.data.length">
              <h5>{{ t('chat.relatedDocs') }}</h5>
              <p>{{t('chat.docFragments', {
                docs: traceData.file.data.length, frags: traceData.file.data.reduce((sum,
                  f) =>
                  sum + (f.snippet?.length || 0), 0)
              })}}</p>
            </div>
            <div class="summary-section" v-if="traceData.graph && traceData.graph.graphTable?.length">
              <h5>{{ t('chat.knowledgeGraph') }}</h5>
              <p>{{ t('chat.graphRelations', { n: traceData.graph.graphTable.length }) }}</p>
            </div>
            <div class="summary-section" v-if="traceData.result && traceData.result.isSubmit">
              <h5>{{ t('chat.qualityFeedback') }}</h5>
              <p>{{ t('chat.scoreLabel', { n: traceData.result.rate }) }}</p>
            </div>
            <div v-if="!hasAnyTrace" class="box-empty">{{ t('chat.noAnswerTrace') }}</div>
          </div>
          <div v-else class="box-empty">{{ t('chat.noAnalysisData') }}</div>
        </div>
      </div>
    </div>
  </div>
</template>

<script lang="ts">
// Trace source toggle buttons exposed for parent (MessageBubble action bar)
export function useTraceSourceActions() {
  // i18n labels are resolved at call site; these are fallback keys
  return [
    { name: 'local', labelKey: 'chat.traceLocal', toggle: (msg: Message) => toggleLocalOnMsg(msg) },
    { name: 'graph', labelKey: 'chat.traceGraph', toggle: (msg: Message) => toggleGraphOnMsg(msg) },
    { name: 'file', labelKey: 'chat.traceFile', toggle: (msg: Message) => toggleFileOnMsg(msg) },
    { name: 'result', labelKey: 'chat.traceResult', toggle: (msg: Message) => toggleResultOnMsg(msg) },
    { name: 'analysis', labelKey: 'chat.traceAnalysis', toggle: (msg: Message) => toggleAnalysisOnMsg(msg) },
  ]
}

function toggleLocalOnMsg(msg: Message) {
  if (!msg.trace_source) return
  if (!msg.trace_source.local) { ElMessage.warning('未选择知识库，无向量库溯源数据'); return }
  msg.trace_source.local.show = !msg.trace_source.local.show
}

function toggleGraphOnMsg(msg: Message) {
  if (!msg.trace_source) return
  if (!msg.trace_source.graph) { ElMessage.info('知识图谱功能暂未开放'); return }
  msg.trace_source.graph.show = !msg.trace_source.graph.show
}

function toggleFileOnMsg(msg: Message) {
  if (!msg.trace_source) return
  if (!msg.trace_source.file) { ElMessage.warning('未选择知识库，无关联文档溯源数据'); return }
  msg.trace_source.file.show = !msg.trace_source.file.show
}

function toggleResultOnMsg(msg: Message) {
  if (!msg.trace_source) return
  if (!msg.trace_source.result) {
    msg.trace_source.result = { show: true, rate: 0, content: '', loading: false, isSubmit: false, isQuery: true }
    return
  }
  msg.trace_source.result.show = !msg.trace_source.result.show
}

function toggleAnalysisOnMsg(msg: Message) {
  if (!msg.trace_source) return
  if (!msg.trace_source.analysis) {
    // Generate analysis from available data
    const summary = buildAnalysisSummary(msg)
    msg.trace_source.analysis = { show: true, summary }
    return
  }
  msg.trace_source.analysis.show = !msg.trace_source.analysis.show
}

function buildAnalysisSummary(msg: Message): string {
  const ts = msg.trace_source
  if (!ts) return ''
  let summary = '# 溯源综合分析\n\n'
  if (ts.local?.data?.length) {
    summary += `**向量库溯源**：检索到 ${ts.local.data.length} 条相关片段\n\n`
  }
  if (ts.file?.data?.length) {
    const totalFragments = ts.file.data.reduce((sum, f) => sum + (f.snippet?.length || 0), 0)
    summary += `**关联文档**：来自 ${ts.file.data.length} 个文档的 ${totalFragments} 条片段\n\n`
  }
  if (ts.graph?.graphTable?.length) {
    summary += `**知识图谱**：包含 ${ts.graph.graphTable.length} 条实体关系\n\n`
  }
  if (ts.result?.isSubmit) {
    summary += `**质量反馈**：用户评分 ${ts.result.rate}/10\n\n`
  }
  return summary
}
</script>


<style scoped lang="scss">
.trace-source {
  width: 100%;
  margin-top: 4px;
}

.trace-section {
  width: 100%;
}

.trace-box {
  width: 100%;
  border: 1px solid var(--border-color);
  border-radius: 10px;
  overflow: hidden;
  background: var(--ai-answer-background) !important;
  // background:  color-mix(in srgb, var(--bg-secondary, #fafafa) 40%, transparent);
}

.box-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  border-bottom: 1px solid var(--border-color);
  // background:  var(--theme-dailog-color);
 
 // background: color-mix(in srgb, var(--bg-secondary, #f5f5f5) 60%, transparent);

  &-left {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--font-size-md);
    font-weight: 500;
    color: var(--text-primary);

    svg {
      color: var(--color-primary);
      flex-shrink: 0;
    }
  }

  .close-btn {
    cursor: pointer;
    color: var(--text-secondary);

    &:hover {
      color: var(--text-primary);
    }
  }
}



.box-content {
  max-height: 300px;
  overflow-y: auto;
  padding: 12px 14px;
}

.box-empty {
  color: var(--text-secondary);
  font-size: var(--font-size-md);
  text-align: center;
  padding: 20px 0;
}

.trace-item {
  padding-bottom: 10px;
  line-height: 1.6;
  font-size: var(--font-size-md);

  &:last-child {
    padding-bottom: 0;
  }

  &-index {
    color: var(--text-secondary);
  }

  &-text {
    color: var(--text-primary);
  }
}

.file-tags {
  display: flex;
  gap: 8px;
  padding: 8px 14px 0;
  overflow-x: auto;
  scrollbar-width: none;

  &::-webkit-scrollbar {
    display: none;
  }

  .file-tag-item {
    cursor: pointer;
    flex-shrink: 0;
  }
}

.quality-hint {
  font-size: var(--font-size-md);
  color: var(--text-secondary);
  margin-bottom: 10px;
}

.quality-input {
  margin-top: 10px;
  :deep(.el-textarea__inner) {
     background: transparent !important;
     border: 1px solid var(--border-color) !important;
     box-shadow: none !important;
     &:focus, &:hover {
       border-color: var(--border-color) !important;
       box-shadow: none !important;
     }
  }
}

.btn-box {
  display: flex;
  justify-content: flex-end;
  margin-top: 8px;
}

.analysis-summary {
  font-size: var(--font-size-md);
  line-height: 1.7;

  .summary-section {
    margin-bottom: 12px;

    h5 {
      font-size: var(--font-size-md);
      font-weight: 600;
      margin: 0 0 4px;
      color: var(--text-primary);
    }

    p {
      margin: 0;
      color: var(--text-secondary);
    }
  }
}

.divider-line {
  margin: 6px 0;
}
</style>
