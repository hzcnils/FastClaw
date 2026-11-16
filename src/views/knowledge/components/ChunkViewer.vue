<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { chunkToolApi, filePreviewApi, chunkApi, chunkAssetApi, kbGraphApi } from '@/api/knowledge';
import { useUIStore } from '@/stores';
import type { AssetElement } from '@/types/knowledge';
import LoadingSpinner from '@/components/common/LoadingSpinner.vue';
import EmptyState from '@/components/common/EmptyState.vue';
import MarkdownRenderer from './ChunkMarkdownRenderer.vue';
import ChunkMethodDialog from './ChunkMethodDialog.vue';
import Modal from '@/components/common/Modal.vue';
import AssetGallery from './AssetGallery.vue';
import { getKGNodeColor } from './kgColors';

const props = withDefaults(defineProps<{
  /** 嵌入知识库详情时传入；不传则读路由 query（独立页兜底） */
  fileId?: string;
  fileName?: string;
  kbId?: string;
  embedded?: boolean;
}>(), {
  embedded: false,
});

const emit = defineEmits<{ back: [] }>();

const route = useRoute();
const router = useRouter();
const uiStore = useUIStore();
const { t } = useI18n();

const showChunkDialog = ref(false);

const fileId = computed(() => props.fileId || String(route.query.fileId || ''));
const fileName = computed(() => props.fileName || String(route.query.fileName || ''));
const kbId = computed(() => props.kbId || String(route.query.kbId || ''));

const imageBaseUrl = computed(() => {
  if (!kbId.value || !fileId.value) return '';
  const base = '';
  return `${base}/api/v1/assets/governance/${kbId.value}/${fileId.value}`;
});

const loading = ref(false);
const error = ref<string | null>(null);
const fullText = ref('');
const chunks = ref<any[]>([]);
const activeChunkIndex = ref(-1);
const algorithm = ref('recursive');
const algorithms = ref<Array<{ code: string; name: string; category: string; description?: string }>>([]);

/** 下拉宽度按最长算法名估算，保证文案完整显示 */
const algoSelectWidth = computed(() => {
  const labels = algorithms.value.map((a) => a.name || a.code).filter(Boolean);
  if (!labels.length) return '180px';
  const longest = labels.reduce((a, b) => ([...a].length >= [...b].length ? a : b));
  const units = [...longest].reduce((n, ch) => n + (/[\u4e00-\u9fff]/.test(ch) ? 1 : 0.55), 0);
  const px = Math.ceil(units * 14 + 52);
  return `${Math.min(Math.max(px, 180), 420)}px`;
});

function uniqueAlgorithms<T extends { code: string }>(list: T[]): T[] {
  const seen = new Set<string>();
  return list.filter((item) => {
    if (!item?.code || seen.has(item.code)) return false;
    seen.add(item.code);
    return true;
  });
}

function bumpNumber(field: 'targetSize' | 'overlap', delta: number) {
  if (field === 'targetSize') {
    targetSize.value = Math.max(1, (Number(targetSize.value) || 0) + delta);
  } else {
    overlap.value = Math.max(0, (Number(overlap.value) || 0) + delta);
  }
}

const chunkAssets = ref<Record<string, AssetElement[]>>({});
const chunkAssetsLoading = ref<Record<string, boolean>>({});
const showAssetModal = ref(false);
const activeAssetChunkId = ref<string | null>(null);
const activeAssetChunkIndex = ref<number>(-1);

const chunkEntities = ref<Record<string, any[]>>({});
const chunkEntitiesLoading = ref<Record<string, boolean>>({});
const showEntityChunksModal = ref(false);
const entityChunksLoading = ref(false);
const activeEntityChunks = ref<any[]>([]);
const activeEntityName = ref('');
const activeEntityType = ref('');

const targetSize = ref(512);
const overlap = ref(50);
const minSize = ref(100);
const maxSize = ref(2000);

function normalizeText(text: string): string {
  return text
    .replace(/\r\n/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .replace(/([^\n])\n([^\n])/g, '$1 $2');
}

function goBack() {
  if (props.embedded) emit('back');
  else router.back();
}

function resetViewerState() {
  error.value = null;
  fullText.value = '';
  chunks.value = [];
  activeChunkIndex.value = -1;
  chunkAssets.value = {};
  chunkAssetsLoading.value = {};
  chunkEntities.value = {};
  chunkEntitiesLoading.value = {};
  showAssetModal.value = false;
  showEntityChunksModal.value = false;
}

async function loadAlgorithms() {
  try {
    const res = await chunkToolApi.listAlgorithms();
    const list = uniqueAlgorithms(Array.isArray(res) ? res : []);
    algorithms.value = list;
    if (list.length && !list.some((a) => a.code === algorithm.value)) {
      algorithm.value = list.find((a) => a.code === 'recursive')?.code || list[0].code;
    }
  } catch { /* ignore */ }
}

async function executeChunking() {
  if (!fileId.value) return;
  loading.value = true;
  error.value = null;
  try {
    const result = await chunkToolApi.chunkFile({
      file_id: fileId.value,
      algorithm: algorithm.value,
      common: {
        target_size: targetSize.value,
        overlap: overlap.value,
        min_size: minSize.value,
        max_size: maxSize.value,
      },
    });
    fullText.value = result.full_text;
    chunks.value = result.chunk_result.chunks;
    if (chunks.value.length > 0) activeChunkIndex.value = 0;
  } catch (e: any) {
    error.value = e.message || t('knowledge.chunkRequestFailed');
  } finally {
    loading.value = false;
  }
}

function selectChunk(index: number) {
  activeChunkIndex.value = index;
  scrollToChunk(index);
}

function scrollToChunk(index: number) {
  nextTick(() => {
    const el = document.getElementById(`chunk-section-${index}`);
    if (el) {
      el.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
  });
}

function getQualityColor(score: number | undefined): string {
  if (!score && score !== 0) return 'var(--color-text-tertiary)';
  if (score >= 0.8) return 'var(--color-success)';
  if (score >= 0.5) return 'var(--color-warning)';
  return 'var(--color-danger)';
}

function getQualityLabel(score: number | undefined): string {
  if (!score && score !== 0) return 'N/A';
  if (score >= 0.8) return t('knowledge.qualityGood');
  if (score >= 0.5) return t('knowledge.qualityMedium');
  return t('knowledge.qualityBad');
}

// ===== chunk asset helpers =====
function getChunkAssetCounts(chunkId: string): { table: number; formula: number; image: number; total: number } {
  const assets = chunkAssets.value[chunkId];
  if (!assets || assets.length === 0) return { table: 0, formula: 0, image: 0, total: 0 };
  const c = { table: 0, formula: 0, image: 0, total: assets.length };
  assets.forEach(a => {
    const t = a.element_type;
    if (t === 'table') c.table++;
    else if (t === 'formula' || t === 'inline_equation' || t === 'interline_equation' || t === 'equation') c.formula++;
    else c.image++;
  });
  return c;
}

async function fetchChunkAssets(chunkId: string) {
  if (chunkAssets.value[chunkId] || chunkAssetsLoading.value[chunkId]) return;
  chunkAssetsLoading.value = { ...chunkAssetsLoading.value, [chunkId]: true };
  try {
    const result = await chunkAssetApi.listByChunk(chunkId);
    chunkAssets.value = { ...chunkAssets.value, [chunkId]: result || [] };
  } catch {
    chunkAssets.value = { ...chunkAssets.value, [chunkId]: [] };
  } finally {
    chunkAssetsLoading.value = { ...chunkAssetsLoading.value, [chunkId]: false };
  }
}

async function openChunkAssets(chunk: any, ci: number) {
  const chunkId = chunk.id || chunk.chunk_id;
  if (!chunkId) return;
  activeAssetChunkId.value = chunkId;
  activeAssetChunkIndex.value = ci;
  showAssetModal.value = true;
  await fetchChunkAssets(chunkId);
}

function closeAssetModal() {
  showAssetModal.value = false;
  activeAssetChunkId.value = null;
}

// ---- active chunk's assets for the modal ----
const activeChunkAssets = computed(() => {
  if (!activeAssetChunkId.value) return [];
  return chunkAssets.value[activeAssetChunkId.value] || [];
});
const activeChunkAssetsLoading = computed(() => {
  if (!activeAssetChunkId.value) return false;
  return chunkAssetsLoading.value[activeAssetChunkId.value] || false;
});

// ===== chunk entity helpers (KB ↔ 图谱融合) =====
function entityName(e: any): string {
  return e?.name || e?.entity_name || e?.mention_text || t('knowledge.unnamedEntity');
}

function entityType(e: any): string {
  return e?.entity_type || 'Entity';
}

function entityColor(e: any): string {
  return getKGNodeColor(entityType(e));
}

function entityTagStyle(e: any): Record<string, string> {
  const color = entityColor(e);
  return {
    borderColor: `${color}55`,
    color,
    background: `${color}14`,
  };
}

function chunkEntityList(chunkId: string): any[] {
  return chunkEntities.value[chunkId] || [];
}

async function fetchChunkEntities(chunkId: string) {
  if (!chunkId) return;
  if (chunkEntities.value[chunkId] !== undefined || chunkEntitiesLoading.value[chunkId]) return;
  chunkEntitiesLoading.value = { ...chunkEntitiesLoading.value, [chunkId]: true };
  try {
    const res: any = await kbGraphApi.chunkEntities(chunkId);
    const items = Array.isArray(res) ? res : res?.entities || [];
    chunkEntities.value = { ...chunkEntities.value, [chunkId]: items };
  } catch {
    chunkEntities.value = { ...chunkEntities.value, [chunkId]: [] };
  } finally {
    chunkEntitiesLoading.value = { ...chunkEntitiesLoading.value, [chunkId]: false };
  }
}

// 以有限并发批量加载所有真实 DB 切片的实体
async function loadAllChunkEntities() {
  const ids = chunks.value
    .map((c: any) => c.id)
    .filter((id: string | undefined): id is string =>
      !!id && chunkEntities.value[id] === undefined && !chunkEntitiesLoading.value[id]
    );
  const CONCURRENCY = 6;
  let idx = 0;
  async function worker() {
    while (idx < ids.length) {
      const id = ids[idx++];
      await fetchChunkEntities(id);
    }
  }
  const workers = Array.from({ length: Math.min(CONCURRENCY, ids.length) }, () => worker());
  await Promise.all(workers);
}

async function openEntityChunks(ent: any) {
  const nodeId = ent?.node_id;
  if (!nodeId) return;
  activeEntityName.value = entityName(ent);
  activeEntityType.value = entityType(ent);
  activeEntityChunks.value = [];
  showEntityChunksModal.value = true;
  entityChunksLoading.value = true;
  try {
    const res: any = await kbGraphApi.nodeChunks(nodeId);
    activeEntityChunks.value = Array.isArray(res) ? res : res?.items || res?.chunks || [];
  } catch (e: any) {
    uiStore.showToast('error', e?.message || t('knowledge.loadEntitySourcesFailed'));
  } finally {
    entityChunksLoading.value = false;
  }
}

function entityChunkDocTitle(c: any): string {
  return c?.doc_title || t('knowledge.docFallback', { id: c?.doc_id || '' });
}

function entityChunkSnippet(c: any): string {
  const s = c?.snippet || '';
  return s.length > 120 ? s.slice(0, 120) + '...' : s;
}

function closeEntityChunksModal() {
  showEntityChunksModal.value = false;
  activeEntityChunks.value = [];
}

async function loadExistingChunks() {
  if (!fileId.value || !kbId.value) return false;
  loading.value = true;
  try {
    const result: any = await chunkApi.list({
      knowledge_base_id: kbId.value,
      page: 1,
      page_size: 999,
    });
    const fileChunks = (result.items || []).filter((c: any) => c.raw_file_id === fileId.value);
    if (fileChunks.length > 0) {
      try {
        const preview = await filePreviewApi.getPreview(fileId.value);
        fullText.value = preview.markdown_content;
      } catch { /* 失败不影响切片显示 */ }
      chunks.value = fileChunks.map((c: any, i: number) => ({
        id: c.id,
        index: i,
        content: c.chunk_content || '',
        char_count: c.chunk_size || c.chunk_content?.length || 0,
        token_count: 0,
        metadata: { has_table: (c.chunk_content || '').includes('|'), has_code: (c.chunk_content || '').includes('```'), has_list: false, headings: [] },
          pageInfo: (() => {
            try {
              const sl = typeof c.source_location === 'string' ? JSON.parse(c.source_location) : (c.source_location || {});
              if (sl.page) return t('knowledge.pageNumber', { page: sl.page });
              if (sl.pages?.length) return t('knowledge.pageRange', { from: sl.pages[0], to: sl.pages[sl.pages.length - 1] });
            } catch { return ''; }
            return '';
          })(),
        quality_score: undefined,
      }));
      loading.value = false;
      loadAllChunkEntities();
      return true;
    }
  } catch { /* 失败走实时分块 */ }
  loading.value = false;
  return false;
}

async function bootstrapViewer() {
  resetViewerState();
  await loadAlgorithms();
  if (!fileId.value) return;
  const loaded = await loadExistingChunks();
  if (!loaded) {
    await executeChunking();
  }
}

watch(
  () => [fileId.value, kbId.value].join('|'),
  () => { void bootstrapViewer(); },
  { immediate: true },
);
</script>

<template>
  <div class="chunk-viewer-root">
    <!-- 工具栏 -->
    <div class="chunk-viewer-toolbar">
      <div class="flex items-center gap-3">
        <button @click="goBack" class="btn btn-ghost btn-sm" :title="$t('common.back')">
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M19 12H5m7-7l-7 7 7 7"/>
          </svg>
        </button>
        <span class="text-lg font-medium">{{ fileName || $t('knowledge.chunkViewTitle') }}</span>
      </div>

      <div class="flex items-center gap-2 cv-toolbar-controls">
        <!-- 算法选择：外观对齐知识管理页 el-select，功能仍为选择 algorithm -->
        <el-select
          v-model="algorithm"
          class="cv-algo-select"
          :style="{ width: algoSelectWidth }"
          :teleported="true"
          popper-class="cv-algo-select-popper"
        >
          <el-option
            v-for="alg in algorithms"
            :key="alg.code"
            :label="alg.name || alg.code"
            :value="alg.code"
          />
        </el-select>
        <!-- 参数：自定义上下箭头，去掉原生白底实心三角 -->
        <div class="cv-number-input-wrap" :title="$t('knowledge.chunkTargetSize')">
          <input v-model.number="targetSize" type="number" class="input cv-number-input" />
          <div class="cv-number-arrows">
            <button type="button" class="cv-number-arrow" @click="bumpNumber('targetSize', 1)">
              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="18 15 12 9 6 15" />
              </svg>
            </button>
            <button type="button" class="cv-number-arrow" @click="bumpNumber('targetSize', -1)">
              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="6 9 12 15 18 9" />
              </svg>
            </button>
          </div>
        </div>
        <div class="cv-number-input-wrap" :title="$t('knowledge.chunkOverlap')">
          <input v-model.number="overlap" type="number" class="input cv-number-input" />
          <div class="cv-number-arrows">
            <button type="button" class="cv-number-arrow" @click="bumpNumber('overlap', 1)">
              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="18 15 12 9 6 15" />
              </svg>
            </button>
            <button type="button" class="cv-number-arrow" @click="bumpNumber('overlap', -1)">
              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="6 9 12 15 18 9" />
              </svg>
            </button>
          </div>
        </div>
        <button @click="showChunkDialog = true" class="btn btn-secondary btn-sm cv-action-btn">
          {{ $t('knowledge.selectAlgorithm') }}
        </button>
        <button @click="executeChunking" :disabled="loading" class="btn bg-[var(--color-primary)] text-white btn-sm disabled:opacity-50 cv-action-btn">
          {{ $t('knowledge.rechunk') }}
        </button>
      </div>
    </div>

    <!-- 主体 -->
    <div class="chunk-viewer-body">
      <div v-if="loading" class="chunk-viewer-loading">
        <LoadingSpinner :text="$t('knowledge.chunkingInProgress')" />
      </div>

      <div v-else-if="error" class="chunk-viewer-loading">
        <EmptyState :title="$t('knowledge.chunkErrorTitle')" :description="error" />
      </div>

      <div v-else-if="!fullText" class="chunk-viewer-loading">
        <EmptyState :title="$t('knowledge.chunkNoContent')" :description="$t('knowledge.chunkNoContentDesc')" />
      </div>

      <template v-else>
        <!-- 左面板：文档原文 -->
        <div class="chunk-viewer-left">
          <div class="chunk-viewer-panel-header">
            {{ $t('knowledge.documentSource') }}
            <span class="text-xs text-[var(--color-text-tertiary)] ml-2">
              {{ $t('knowledge.charsAndChunks', { chars: fullText.length, chunks: chunks.length }) }}
            </span>
          </div>
          <div class="chunk-viewer-doc-scroll" id="doc-scroll">
            <div class="chunk-viewer-doc-content prose max-w-none">
              <div
                v-for="(chunk, ci) in chunks"
                :key="ci"
                :id="`chunk-section-${ci}`"
                :class="['chunk-section', 'cv-chunk-section', ci === activeChunkIndex ? 'chunk-section-active' : '']"
                @click="selectChunk(ci)"
              >
                <span class="chunk-section-badge">#{{ chunk.index }}</span>
                <MarkdownRenderer :content="normalizeText(chunk.content)" :image-base-url="imageBaseUrl" />
              </div>
              <div v-if="chunks.length === 0" class="text-sm text-[var(--color-text-tertiary)] p-6">
                <MarkdownRenderer :content="fullText" :image-base-url="imageBaseUrl" />
              </div>
            </div>
          </div>
        </div>

        <!-- 右面板：分块列表 -->
        <div class="chunk-viewer-right">
          <div class="chunk-viewer-panel-header">
            {{ $t('knowledge.chunkListTitle', { count: chunks.length }) }}
          </div>
          <div class="chunk-viewer-list-scroll">
            <div
              v-for="(chunk, ci) in chunks"
              :key="ci"
              :class="['chunk-card', 'cv-chunk-card', ci === activeChunkIndex ? 'chunk-card-active' : '']"
              @click="selectChunk(ci)"
            >
              <div class="chunk-card-header">
                <span class="chunk-card-index">#{{ chunk.index }}</span>
                <span v-if="chunk.pageInfo" class="chunk-card-page">{{ chunk.pageInfo }}</span>
                <span class="chunk-card-size">{{ chunk.char_count }} {{ $t('knowledge.chars') }}</span>
                <span
                  v-if="chunk.quality_score !== undefined"
                  class="chunk-card-quality"
                  :style="{ color: getQualityColor(chunk.quality_score) }"
                >
                  {{ getQualityLabel(chunk.quality_score) }} {{ (chunk.quality_score * 100).toFixed(0) }}%
                </span>
              </div>
              <div class="chunk-card-meta" v-if="chunk.title_path?.length">
                <svg class="w-3 h-3 inline" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M2 4h20M2 10h20M2 16h20"/>
                </svg>
                {{ chunk.title_path.join(' > ') }}
              </div>
              <div class="chunk-card-meta" v-if="chunk.metadata?.has_table">
                <span class="badge badge-xs bg-blue-100 text-blue-700">{{ $t('knowledge.hasTable') }}</span>
              </div>
              <div class="chunk-card-meta" v-if="chunk.metadata?.has_code">
                <span class="badge badge-xs bg-purple-100 text-purple-700">{{ $t('knowledge.hasCode') }}</span>
              </div>
              <!-- Asset badges: fetched on demand; show a load-trigger -->
              <div class="chunk-card-meta chunk-card-assets" v-if="chunk.id">
                <template v-if="chunkAssets[chunk.id]">
                  <span v-if="getChunkAssetCounts(chunk.id).table > 0" class="badge badge-xs bg-blue-100 text-blue-700">{{ $t('knowledge.hasNTables', { n: getChunkAssetCounts(chunk.id).table }) }}</span>
                  <span v-if="getChunkAssetCounts(chunk.id).formula > 0" class="badge badge-xs bg-purple-100 text-purple-700">{{ $t('knowledge.hasNFormulas', { n: getChunkAssetCounts(chunk.id).formula }) }}</span>
                  <span v-if="getChunkAssetCounts(chunk.id).image > 0" class="badge badge-xs bg-emerald-100 text-emerald-700">{{ $t('knowledge.hasNImages', { n: getChunkAssetCounts(chunk.id).image }) }}</span>
                </template>
                <button
                  @click.stop="openChunkAssets(chunk, ci)"
                  class="text-sm text-blue-500 hover:text-blue-600 hover:underline"
                  :title="$t('knowledge.viewAssets')"
                >
                  {{ $t('knowledge.viewAssets') }}
                </button>
              </div>
              <!-- 实体标签（KB ↔ 图谱融合）：仅真实 DB 切片加载 -->
              <div class="chunk-card-meta chunk-card-assets" v-if="chunk.id">
                <template v-if="chunkEntities[chunk.id] !== undefined">
                  <span v-if="chunkEntityList(chunk.id).length > 0" class="entity-label">
                    <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4"/>
                    </svg>
                    {{ $t('knowledge.entities') }}
                  </span>
                  <span
                    v-for="ent in chunkEntityList(chunk.id).slice(0, 6)"
                    :key="ent.node_id || entityName(ent)"
                    class="entity-tag"
                    :style="entityTagStyle(ent)"
                    :title="`${entityName(ent)} (${entityType(ent)}) · ${$t('knowledge.entityClickHint')}`"
                    @click.stop="openEntityChunks(ent)"
                  >
                    {{ entityName(ent) }}
                    <span class="entity-tag-type">{{ entityType(ent) }}</span>
                  </span>
                  <span v-if="chunkEntityList(chunk.id).length > 6" class="text-[10px] text-[var(--color-text-tertiary)]">
                    +{{ chunkEntityList(chunk.id).length - 6 }}
                  </span>
                </template>
                <template v-else>
                  <button
                    v-if="!chunkEntitiesLoading[chunk.id]"
                    @click.stop="fetchChunkEntities(chunk.id)"
                    class="text-sm text-blue-500 hover:text-blue-600 hover:underline"
                    :title="$t('knowledge.loadEntities')"
                  >
                    {{ $t('knowledge.loadEntities') }}
                  </button>
                  <span v-else class="text-[10px] text-[var(--color-text-tertiary)]">{{ $t('knowledge.loadingEntities') }}</span>
                </template>
              </div>
              <div class="chunk-card-preview">
                <MarkdownRenderer :content="normalizeText(chunk.content)" :image-base-url="imageBaseUrl" class="text-sm" />
              </div>
            </div>
          </div>
        </div>
      </template>
    </div>

    <!-- 分块方法选择对话框 -->
    <ChunkMethodDialog
      :show="showChunkDialog"
      :content="fullText"
      :algorithms="algorithms"
      :initial-algorithm="algorithm"
      :initial-target-size="targetSize"
      :initial-overlap="overlap"
      :initial-min-size="minSize"
      :initial-max-size="maxSize"
      @close="showChunkDialog = false"
      @apply="(cfg) => {
        algorithm = cfg.algorithm;
        targetSize = cfg.targetSize;
        overlap = cfg.overlap;
        minSize = cfg.minSize;
        maxSize = cfg.maxSize;
        showChunkDialog = false;
        executeChunking();
      }"
    />

    <!-- 切片资产预览弹窗 -->
    <Modal
      :show="showAssetModal"
      :title="$t('knowledge.chunkAssetsTitle', { index: activeAssetChunkIndex })"
      size="2xl"
      @close="closeAssetModal"
    >
      <AssetGallery
        :assets="activeChunkAssets"
        :loading="activeChunkAssetsLoading"
        :image-base-url="imageBaseUrl"
      />
    </Modal>

    <!-- 实体来源切片弹窗（KB ↔ 图谱融合） -->
    <Modal
      :show="showEntityChunksModal"
      :title="$t('knowledge.entitySourceChunksTitle', { name: activeEntityName })"
      size="lg"
      @close="closeEntityChunksModal"
    >
      <div class="space-y-2">
        <div v-if="entityChunksLoading" class="py-8">
          <LoadingSpinner size="sm" :text="$t('knowledge.loadingSourceChunks')" />
        </div>
        <template v-else-if="activeEntityChunks.length > 0">
          <div class="text-xs text-[var(--color-text-tertiary)] mb-2">
            {{ $t('knowledge.sourceChunksCount', { count: activeEntityChunks.length }) }}
          </div>
          <div class="max-h-[55vh] overflow-y-auto space-y-2 pr-1">
            <div
              v-for="(c, i) in activeEntityChunks"
              :key="c.chunk_id || i"
              class="p-3 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-elevated)]"
            >
              <div class="flex items-center gap-2 flex-wrap mb-1">
                <span class="badge badge-secondary">{{ activeEntityType }}</span>
                <span class="text-xs font-medium text-[var(--color-text)]">{{ entityChunkDocTitle(c) }}</span>
                <span v-if="c.page" class="chunk-card-page">{{ $t('knowledge.pageNumber', { page: c.page }) }}</span>
                <span class="ml-auto text-[11px] font-mono text-[var(--color-text-tertiary)]">
                  {{ $t('knowledge.confidenceLabel') }} {{ c.confidence !== undefined && c.confidence !== null ? (c.confidence * 100).toFixed(0) + '%' : '-' }}
                </span>
              </div>
              <div class="text-xs text-[var(--color-text-secondary)] leading-relaxed">
                {{ entityChunkSnippet(c) || $t('knowledge.noSnippet') }}
              </div>
            </div>
          </div>
        </template>
        <div v-else class="py-8">
          <EmptyState icon="database" :title="$t('knowledge.noSourceChunks')" :description="$t('knowledge.noSourceChunksDesc')" />
        </div>
      </div>
    </Modal>
  </div>
</template>

<style scoped>

/* FastClaw theme token aliases for migrated FastKG styles */
.chunk-viewer-root {
  --color-bg: var(--bg-primary, #fff);
  --color-bg-elevated: var(--card-background-tmd, var(--bg-primary, #fff));
  --color-bg-sunken: var(--background-color, rgba(0,0,0,0.03));
  --color-border: var(--border-color, #e2e8f0);
  --color-text: var(--text-primary, #1a1a1a);
  --color-text-secondary: var(--text-secondary, #64748b);
  --color-text-tertiary: var(--text-secondary, #94a3b8);
  --color-success: #10b981;
  --color-warning: #f59e0b;
  --color-danger: #ef4444;
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: calc(100vh - 120px);
  background: transparent;
}

.chunk-viewer-root.kb-embedded-chunk-viewer {
  min-height: 0;
  background: transparent;
}

/* 算法下拉尺寸；具体外观见下方非 scoped，避免被 knowledge.css .select / main.css 盖掉 */
.chunk-viewer-root .cv-algo-select {
  width: auto;
  --el-fill-color-blank: transparent;
  --el-bg-color-overlay: transparent;
}

/* 对齐知识管理页 btn-create；高度 36px，字号 --font-size-md */
.chunk-viewer-root .cv-action-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 0 16px;
  height: 36px;
  background: var(--btn-primary-bg) !important;
  color: var(--btn-primary-text) !important;
  border: 1px solid var(--btn-primary-border) !important;
  border-radius: 4px;
  font-size: var(--font-size-md) !important;
  font-weight: 500;
  white-space: nowrap;
  line-height: 1;
  cursor: pointer;
  transition: all 0.2s;
  box-shadow: none;
}

/* 数字参数框：自定义箭头 */
.chunk-viewer-root .cv-number-input-wrap {
  position: relative;
  display: inline-flex;
  align-items: center;
  height: 36px;
}
.chunk-viewer-root .cv-number-input {
  width: 72px;
  height: 36px !important;
  min-height: 36px !important;
  padding: 0 28px 0 10px !important;
  box-sizing: border-box;
  background: transparent !important;
  border: 1px solid var(--border-color) !important;
  border-radius: 6px !important;
  color: var(--text-primary) !important;
  font-size: var(--font-size-md) !important;
  box-shadow: none !important;
  -moz-appearance: textfield;
}
.chunk-viewer-root .cv-number-input::-webkit-inner-spin-button,
.chunk-viewer-root .cv-number-input::-webkit-outer-spin-button {
  -webkit-appearance: none;
  margin: 0;
  display: none;
}
.chunk-viewer-root .cv-number-arrows {
  position: absolute;
  right: 1px;
  top: 1px;
  bottom: 1px;
  width: 22px;
  display: flex;
  flex-direction: column;
  border-radius: 0 5px 5px 0;
  overflow: hidden;
}
.chunk-viewer-root .cv-number-arrow {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  margin: 0;
  border: none;
  background: transparent;
  color: var(--text-primary);
  cursor: pointer;
  line-height: 1;
}
.chunk-viewer-root .cv-number-arrow:hover {
  background: var(--dropdown-hover-color, rgba(128, 128, 128, 0.2));
}
.chunk-viewer-root .cv-number-arrow svg {
  width: 12px;
  height: 12px;
  display: block;
}
.chunk-viewer-root .cv-action-btn:hover:not(:disabled) {
  background: var(--btn-primary-hover-bg) !important;
  color: var(--btn-primary-hover-text) !important;
  border-color: var(--btn-primary-hover-border) !important;
  transform: translateY(-1px);
  box-shadow: 0 2px 8px color-mix(in srgb, var(--btn-primary-hover-bg) 26%, transparent);
}
.chunk-viewer-root .cv-action-btn:disabled {
  background: var(--btn-primary-disabled-bg) !important;
  color: var(--btn-primary-disabled-text) !important;
  border-color: var(--btn-primary-disabled-border) !important;
  opacity: 1;
  cursor: not-allowed;
  transform: none;
  box-shadow: none;
}

/* 返回按钮：图标主色，悬浮用下拉悬浮色 */
.chunk-viewer-root .btn.btn-ghost.btn-sm {
  color: var(--text-primary);
  background: transparent;
  border-color: transparent;
}
.chunk-viewer-root .btn.btn-ghost.btn-sm svg {
  color: var(--text-primary);
  stroke: currentColor;
}
.chunk-viewer-root .btn.btn-ghost.btn-sm:hover {
  background: var(--dropdown-hover-color) !important;
  color: var(--text-primary);
}

.chunk-viewer-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 16px;
  border-bottom: 1px solid var(--color-border);
  background: transparent;
  flex-shrink: 0;
  min-height: 52px;
  gap: 12px;
}

.chunk-viewer-body {
  flex: 1;
  display: flex;
  overflow: hidden;
  min-height: 0;
}

.chunk-viewer-loading {
  flex: 1;
  width: 100%;
  height: 100%;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

.chunk-viewer-left {
  flex: 1;
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--color-border);
  min-width: 0;
}

.chunk-viewer-right {
  width: 400px;
  min-width: 320px;
  display: flex;
  flex-direction: column;
  background: transparent;
}

.chunk-viewer-panel-header {
  padding: 10px 16px;
  font-size: var(--font-size-md);
  font-weight: 600;
  color: var(--color-text-secondary);
  border-bottom: 1px solid var(--color-border);
  background: transparent;
  flex-shrink: 0;
}

.chunk-viewer-doc-scroll {
  flex: 1;
  overflow: auto;
  padding: 24px;
}

.chunk-viewer-doc-content {
  line-height: 1.8;
  font-size: var(--font-size-md);
  white-space: pre-wrap;
  word-break: break-word;
}

.chunk-section {
  position: relative;
  padding: 8px 16px;
  border-left: 3px solid transparent;
  border-radius: 4px;
  margin-bottom: 2px;
  cursor: pointer;
  transition: all 0.15s;
  background: rgba(0,0,0,0.01);
}
.chunk-section:nth-child(even) {
  background: rgba(0,0,0,0.02);
}
.chunk-section:hover {
  background: rgba(59, 130, 246, 0.05);
}
.chunk-section-active {
  background: rgba(59, 130, 246, 0.08) !important;
  border-left-color: var(--color-primary);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-primary) 25%, transparent);
}
.chunk-section + .chunk-section {
  border-top: 1px dashed rgba(148, 163, 184, 0.2);
}
.chunk-section-badge {
  position: absolute;
  top: 4px;
  right: 8px;
  font-size: 10px;
  font-weight: 600;
  color: var(--color-primary);
  background: rgba(59, 130, 246, 0.1);
  padding: 1px 6px;
  border-radius: 8px;
  opacity: 0;
  transition: opacity 0.15s;
}
.chunk-section:hover .chunk-section-badge,
.chunk-section-active .chunk-section-badge {
  opacity: 1;
}

.chunk-viewer-list-scroll {
  flex: 1;
  overflow: auto;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.chunk-card {
  background: transparent;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  padding: 12px;
  cursor: pointer;
  transition: all 0.15s;
}
.chunk-card:hover {
  border-color: var(--color-primary);
  box-shadow: 0 1px 4px rgba(59, 130, 246, 0.1);
}
.chunk-card-active {
  background: rgba(59, 130, 246, 0.08) !important;
  border-color: var(--color-primary);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--color-primary) 25%, transparent);
}

.chunk-card-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}

.chunk-card-index {
  font-size: var(--font-size-sm);
  font-weight: 700;
  color: var(--color-primary);
}

.chunk-card-page {
  font-size:var(--font-size-xs);
  color: var(--color-text-tertiary);
  background: var(--color-bg-sunken);
  padding: 1px 6px;
  border-radius: 4px;
  margin-left: 4px;
}

.chunk-card-size {
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
}

.chunk-card-quality {
  font-size: var(--font-size-xs);
  font-weight: 600;
  margin-left: auto;
}

.chunk-card-meta {
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
  margin-bottom: 4px;
}

.chunk-card-assets {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-wrap: wrap;
}

.entity-label {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: 10px;
  font-weight: 600;
  color: var(--color-text-tertiary);
  margin-right: 2px;
}

.entity-tag {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  max-width: 150px;
  padding: 1px 7px;
  border-radius: 999px;
  border: 1px solid transparent;
  font-size: 11px;
  font-weight: 500;
  line-height: 1.5;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  cursor: pointer;
  transition: transform 0.12s, box-shadow 0.12s;
}
.entity-tag:hover {
  transform: translateY(-1px);
  box-shadow: 0 1px 4px rgba(15, 23, 42, 0.12);
}
.entity-tag-type {
  font-size: 9px;
  font-weight: 600;
  opacity: 0.75;
  flex-shrink: 0;
}

.chunk-card-preview {
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
  line-height: 1.5;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.dialog-enter-active,
.dialog-leave-active {
  transition: opacity 0.2s ease;
}
.dialog-enter-from,
.dialog-leave-to {
  opacity: 0;
}
</style>

<style>
/* 对齐知识管理页 toolbar-left el-select__wrapper（非 scoped，压过 main.css） */
html body .chunk-viewer-root .cv-algo-select.el-select .el-select__wrapper,
html body .chunk-viewer-root .cv-algo-select .el-select__wrapper {
  background: transparent !important;
  background-color: transparent !important;
  border: 1px solid var(--border-color) !important;
  border-width: 1px !important;
  border-radius: 6px !important;
  height: 36px !important;
  min-height: 36px !important;
  padding: 0 12px !important;
  box-shadow: none !important;
  outline: none !important;
  box-sizing: border-box;
}
html body .chunk-viewer-root .cv-algo-select .el-select__wrapper:hover {
  border: 1px solid var(--text-secondary) !important;
  border-width: 1px !important;
  box-shadow: none !important;
  background: transparent !important;
  background-color: transparent !important;
}
html body .chunk-viewer-root .cv-algo-select .el-select__wrapper.is-focused,
html body .chunk-viewer-root .cv-algo-select .el-select__wrapper.is-focus,
html body .chunk-viewer-root .cv-algo-select .el-select__wrapper.is-hovering {
  border: 1px solid var(--border-color) !important;
  border-width: 1px !important;
  box-shadow: none !important;
  outline: none !important;
  background: transparent !important;
  background-color: transparent !important;
}
html body .chunk-viewer-root .cv-algo-select .el-select__selected-item,
html body .chunk-viewer-root .cv-algo-select .el-select__input {
  color: var(--text-primary) !important;
  font-size: var(--font-size-md, 13px) !important;
  line-height: 34px;
}
html body .chunk-viewer-root .cv-algo-select .el-select__placeholder {
  color: var(--text-secondary) !important;
}
html body .chunk-viewer-root .cv-algo-select .el-select__caret {
  color: var(--text-secondary) !important;
}

/* 暗色主题下算法下拉边框，对齐知识管理页 toolbar-left el-select */
.dark .chunk-viewer-root .cv-algo-select .el-select__wrapper {
  border: 1px solid rgba(255, 255, 255, 0.20) !important;
  border-width: 1px !important;
  box-shadow: none !important;
}
.dark .chunk-viewer-root .cv-algo-select .el-select__wrapper:hover {
  border: 1px solid rgba(255, 255, 255, 0.40) !important;
  border-width: 1px !important;
  box-shadow: none !important;
}
.dark .chunk-viewer-root .cv-algo-select .el-select__wrapper.is-focused,
.dark .chunk-viewer-root .cv-algo-select .el-select__wrapper.is-focus {
  border: 1px solid var(--color-primary) !important;
  border-width: 1px !important;
  box-shadow: none !important;
}

/* markdown 默认透明，避免再叠一层底色把面板衬成黑白块 */
html body .chunk-viewer-root .md-body,
html body .chunk-viewer-root .md-body pre code,
html body .chunk-viewer-root .md-body code,
html body .chunk-viewer-root .md-body thead,
html body .chunk-viewer-root .md-body tbody tr:nth-child(even) td,
html body .chunk-viewer-root .md-body blockquote {
  background: transparent !important;
  background-color: transparent !important;
}
html body .chunk-viewer-root .md-body pre,
html body .chunk-viewer-root .md-body pre code {
  color: var(--text-primary) !important;
}
html body .chunk-viewer-root .cv-chunk-section.chunk-section-active {
  border-left-color: var(--color-primary) !important;
}

/* 浅色主题：分块卡仍白底；选中原文块改为原雾灰样式（浅蓝底 + 蓝左边框） */
html.light[data-theme-id="simple-light"] .chunk-viewer-root .cv-chunk-card,
html.light[data-theme-id="simple-light"] .chunk-viewer-root .cv-chunk-card.chunk-card-active {
  background: #ffffff !important;
  background-color: #ffffff !important;
}
html.light[data-theme-id="simple-light"] .chunk-viewer-root .cv-chunk-section.chunk-section-active {
  background: rgba(59, 130, 246, 0.1) !important;
  background-color: rgba(59, 130, 246, 0.1) !important;
  border-left-color: rgba(59, 130, 246, 0.7) !important;
  box-shadow: inset 0 0 0 1px rgba(59, 130, 246, 0.2);
}

/* 雾灰：卡片/代码块浅灰不变；选中原文块改为与其他情景主题一致 */
html.light[data-theme-id="scene-graphite-cyan"] .chunk-viewer-root .cv-chunk-card,
html.light[data-theme-id="scene-graphite-cyan"] .chunk-viewer-root .md-body pre {
  background: #f5f5f5 !important;
  background-color: #f5f5f5 !important;
}
html.light[data-theme-id="scene-graphite-cyan"] .chunk-viewer-root .cv-chunk-section.chunk-section-active {
  background: rgba(255, 255, 255, 0.08) !important;
  background-color: rgba(255, 255, 255, 0.08) !important;
  border-left-color: var(--color-primary) !important;
  box-shadow: none;
}

/* 黑暗主题 & 情景主题（除雾灰，含 custom-scene）：半透明浅灰 */
html.dark .chunk-viewer-root .cv-chunk-card,
html.dark .chunk-viewer-root .cv-chunk-card.chunk-card-active,
html[data-category="scene"]:not([data-theme-id="scene-graphite-cyan"]) .chunk-viewer-root .cv-chunk-card,
html[data-category="scene"]:not([data-theme-id="scene-graphite-cyan"]) .chunk-viewer-root .cv-chunk-card.chunk-card-active,
html.dark .chunk-viewer-root .md-body pre,
html[data-category="scene"]:not([data-theme-id="scene-graphite-cyan"]) .chunk-viewer-root .md-body pre {
  background: rgba(255, 255, 255, 0.08) !important;
  background-color: rgba(255, 255, 255, 0.08) !important;
}
html.dark .chunk-viewer-root .cv-chunk-section.chunk-section-active,
html[data-category="scene"]:not([data-theme-id="scene-graphite-cyan"]) .chunk-viewer-root .cv-chunk-section.chunk-section-active {
  background: rgba(255, 255, 255, 0.08) !important;
  background-color: rgba(255, 255, 255, 0.08) !important;
  border-left-color: var(--color-primary) !important;
}

/* 情景主题（除雾灰，含自定义情景）：序号 / 角标 / 质量 白字。
   调节透明度后 themeId 会变成 custom-scene，不能只用 ^=scene- */
html[data-category="scene"]:not([data-theme-id="scene-graphite-cyan"]) .chunk-viewer-root .chunk-card-index,
html[data-category="scene"]:not([data-theme-id="scene-graphite-cyan"]) .chunk-viewer-root .chunk-card-quality,
html[data-category="scene"]:not([data-theme-id="scene-graphite-cyan"]) .chunk-viewer-root .chunk-section-badge {
  color: #ffffff !important;
}
html[data-category="scene"]:not([data-theme-id="scene-graphite-cyan"]) .chunk-viewer-root .chunk-card-quality {
  padding: 1px 6px;
  border-radius: 4px;
}

/* 浅色 / 雾灰：返回 ghost 按钮图标黑色（压过 KnowledgeBaseDetail .btn svg 全白） */
html.light[data-theme-id="simple-light"] .chunk-viewer-root .btn.btn-ghost.btn-sm,
html.light[data-theme-id="scene-graphite-cyan"] .chunk-viewer-root .btn.btn-ghost.btn-sm,
html.light[data-theme-id="simple-light"] .chunk-viewer-root .btn.btn-ghost.btn-sm svg,
html.light[data-theme-id="scene-graphite-cyan"] .chunk-viewer-root .btn.btn-ghost.btn-sm svg {
  color: #000000 !important;
  stroke: #000000 !important;
  fill: none !important;
}

/* 文档原文正文：跟随主题字号 */
.chunk-viewer-root .chunk-viewer-doc-content,
.chunk-viewer-root .md-body {
  font-size: var(--font-size-md) !important;
}

/* 黑暗 & 情景（除雾灰，含 custom-scene）：紫色 badge 用卡片底/边框/白字，不受透明度调节打断 */
html.dark .chunk-viewer-root .badge.badge-xs.bg-purple-100,
html.dark .chunk-viewer-root .badge.badge-xs.text-purple-700,
html[data-category="scene"]:not([data-theme-id="scene-graphite-cyan"]) .chunk-viewer-root .badge.badge-xs.bg-purple-100,
html[data-category="scene"]:not([data-theme-id="scene-graphite-cyan"]) .chunk-viewer-root .badge.badge-xs.text-purple-700 {
  background: var(--card-background-tmd) !important;
  background-color: var(--card-background-tmd) !important;
  border-color: var(--border-color) !important;
  color: #ffffff !important;
}

/* 黑暗 & 情景（除雾灰，含 custom-scene）：蓝色 badge 同上 */
html.dark .chunk-viewer-root .badge.badge-xs.bg-blue-100,
html.dark .chunk-viewer-root .badge.badge-xs.text-blue-700,
html[data-category="scene"]:not([data-theme-id="scene-graphite-cyan"]) .chunk-viewer-root .badge.badge-xs.bg-blue-100,
html[data-category="scene"]:not([data-theme-id="scene-graphite-cyan"]) .chunk-viewer-root .badge.badge-xs.text-blue-700 {
  background: var(--card-background-tmd) !important;
  background-color: var(--card-background-tmd) !important;
  border-color: var(--border-color) !important;
  color: #ffffff !important;
}
</style>
