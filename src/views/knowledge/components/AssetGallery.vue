<script setup lang="ts">
import { ref, computed } from 'vue';
import type { AssetElement } from '@/types/knowledge';
import { sanitizeHtml } from '@/utils/display';
import Modal from '@/components/common/Modal.vue';
import LoadingSpinner from '@/components/common/LoadingSpinner.vue';

const props = withDefaults(defineProps<{
  assets: AssetElement[];
  loading?: boolean;
  imageBaseUrl?: string;
  editable?: boolean;
}>(), {
  loading: false,
  editable: false,
});

const emit = defineEmits<{
  (e: 'edit-asset', asset: AssetElement): void;
}>();

// ---- filter ----
const filterType = ref<string>('all');

const typeOptions = [
  { key: 'all',      label: '全部', icon: '' },
  { key: 'table',    label: '表格', icon: '' },
  { key: 'formula',  label: '公式', icon: '' },
  { key: 'image',    label: '图片', icon: '' },
  { key: 'figure',   label: '图表', icon: '' },
];

const typeCounts = computed(() => {
  const c: Record<string, number> = { all: props.assets.length, table: 0, formula: 0, image: 0, figure: 0 };
  props.assets.forEach(a => {
    const t = a.element_type;
    if (t === 'table') c.table++;
    else if (t === 'formula' || t === 'inline_equation' || t === 'interline_equation' || t === 'equation') c.formula++;
    else if (t === 'image' || t === 'page_image' || t === 'text_image') c.image++;
    else if (t === 'figure') c.figure++;
    else c.image++; // fallback
  });
  return c;
});

const filteredAssets = computed(() => {
  if (filterType.value === 'all') return props.assets;
  const ft = filterType.value;
  return props.assets.filter(a => {
    const t = a.element_type;
    if (ft === 'table') return t === 'table';
    if (ft === 'formula') return t === 'formula' || t === 'inline_equation' || t === 'interline_equation' || t === 'equation';
    if (ft === 'image') return t === 'image' || t === 'page_image' || t === 'text_image';
    if (ft === 'figure') return t === 'figure';
    return false;
  });
});

// ---- preview modal ----
const previewAsset = ref<AssetElement | null>(null);
const showPreview = ref(false);

function openPreview(asset: AssetElement) {
  previewAsset.value = asset;
  showPreview.value = true;
}

function closePreview() {
  showPreview.value = false;
  previewAsset.value = null;
}

// ---- helpers ----
function assetImageSrc(asset: AssetElement, useThumbnail = true): string {
  const p = (useThumbnail && asset.thumbnail_path) ? asset.thumbnail_path : asset.asset_path;
  if (!p) return '';
  if (p.startsWith('http')) return p;
  const origin = '';
  if (p.startsWith('/')) return `${origin}${p}`;
  return `${origin}/api/v1/assets/${p}`;
}

function getTypeLabel(t: string): string {
  const m: Record<string, string> = {
    table: '表格',
    formula: '公式',
    equation: '公式',
    inline_equation: '行内公式',
    interline_equation: '块公式',
    image: '图片',
    figure: '图表',
    page_image: '页面截图',
    text_image: '图文',
  };
  return m[t] || t;
}

function getTypeIcon(t: string): string {
  if (t === 'table') return '📊';
  if (t === 'formula' || t === 'inline_equation' || t === 'interline_equation' || t === 'equation') return '📐';
  if (t === 'figure') return '📈';
  if (t === 'page_image') return '📄';
  return '🖼';
}

function getTypeColor(t: string): string {
  if (t === 'table') return 'bg-blue-100 text-blue-700';
  if (t === 'formula' || t === 'inline_equation' || t === 'interline_equation' || t === 'equation') return 'bg-purple-100 text-purple-700';
  if (t === 'figure') return 'bg-amber-100 text-amber-700';
  if (t === 'page_image') return 'bg-teal-100 text-teal-700';
  return 'bg-emerald-100 text-emerald-700';
}

function isTableType(t: string): boolean {
  return t === 'table';
}

function isFormulaType(t: string): boolean {
  return t === 'formula' || t === 'inline_equation' || t === 'interline_equation' || t === 'equation';
}

function getPreviewImageSrc(asset: AssetElement): string {
  return assetImageSrc(asset, false);
}

function onEditAsset(asset: AssetElement) {
  emit('edit-asset', asset);
}
</script>

<template>
  <div class="asset-gallery">
    <!-- Filter bar -->
    <div class="asset-gallery-filters">
      <button
        v-for="opt in typeOptions"
        :key="opt.key"
        @click="filterType = opt.key"
        :class="['asset-filter-btn', filterType === opt.key ? 'asset-filter-btn--active' : '']"
      >
        {{ opt.icon }} {{ opt.label }}
        <span class="asset-filter-count">{{ typeCounts[opt.key] }}</span>
      </button>
      <span class="asset-filter-total text-xs text-gray-400 ml-auto">{{ filteredAssets.length }} 项</span>
    </div>

    <!-- Loading -->
    <LoadingSpinner v-if="loading" size="sm" text="加载资产..." class="py-16" />

    <!-- Empty -->
    <div v-else-if="filteredAssets.length === 0" class="text-center py-16 text-sm text-gray-400">
      暂无资产
    </div>

    <!-- Grid -->
    <div v-else class="asset-gallery-grid">
      <div
        v-for="asset in filteredAssets"
        :key="asset.id"
        class="asset-card"
        @click="openPreview(asset)"
      >
        <!-- Thumbnail / Preview area -->
        <div class="asset-card-thumb">
          <!-- Table: show image or rendered HTML table -->
          <template v-if="isTableType(asset.element_type)">
            <img
              v-if="asset.asset_path"
              :src="assetImageSrc(asset)"
              :alt="asset.caption || 'table'"
              class="asset-card-img"
              loading="lazy"
            />
            <div
              v-else-if="asset.table_data"
              v-html="sanitizeHtml(asset.table_data)"
              class="asset-card-table-preview"
            ></div>
            <span v-else class="text-xs text-gray-400">无预览</span>
          </template>

          <!-- Formula -->
          <div v-else-if="isFormulaType(asset.element_type)" class="asset-card-formula">
            <code class="text-xs text-blue-600 font-mono break-all line-clamp-4">{{ asset.content_text || asset.ocr_text || '无公式内容' }}</code>
          </div>

          <!-- Image / Figure -->
          <img
            v-else-if="assetImageSrc(asset)"
            :src="assetImageSrc(asset)"
            :alt="asset.caption || 'asset image'"
            class="asset-card-img"
            loading="lazy"
          />
          <span v-else class="text-xs text-gray-400">无预览</span>

          <!-- Page badge -->
          <span v-if="asset.page_number" class="asset-card-page-badge">
            P{{ asset.page_number }}
          </span>
        </div>

        <!-- Card footer -->
        <div class="asset-card-footer">
          <span :class="['asset-card-type-badge', getTypeColor(asset.element_type)]">
            {{ getTypeIcon(asset.element_type) }} {{ getTypeLabel(asset.element_type) }}
          </span>
          <button
            v-if="editable && (isTableType(asset.element_type) || isFormulaType(asset.element_type))"
            @click.stop="onEditAsset(asset)"
            class="asset-card-edit-btn"
          >
            编辑
          </button>
        </div>
        <!-- Caption -->
        <div v-if="asset.caption" class="asset-card-caption" :title="asset.caption">
          {{ asset.caption }}
        </div>
      </div>
    </div>

    <!-- ===== Preview Modal ===== -->
    <Modal
      :show="showPreview"
      :title="previewAsset ? (previewAsset.caption || getTypeLabel(previewAsset.element_type)) : '资产预览'"
      size="xl"
      @close="closePreview"
    >
      <template v-if="previewAsset">
        <div class="asset-preview-container">
          <!-- Table: rendered HTML or image -->
          <template v-if="isTableType(previewAsset.element_type)">
            <div v-if="previewAsset.table_data" class="asset-preview-table-wrap">
              <div v-html="sanitizeHtml(previewAsset.table_data)" class="asset-preview-table"></div>
            </div>
            <div v-else-if="previewAsset.asset_path" class="asset-preview-image">
              <img :src="getPreviewImageSrc(previewAsset)" alt="table" class="max-w-full max-h-[60vh] object-contain" />
            </div>
            <div v-else class="text-center text-gray-400 py-8">表格数据未提取</div>

            <!-- Table raw data -->
            <details v-if="previewAsset.table_data" class="mt-4">
              <summary class="text-xs text-gray-500 cursor-pointer hover:text-gray-700">查看原始HTML</summary>
              <pre class="mt-2 p-3 bg-gray-50 rounded text-xs font-mono whitespace-pre-wrap overflow-auto max-h-40">{{ previewAsset.table_data }}</pre>
            </details>
          </template>

          <!-- Formula -->
          <template v-else-if="isFormulaType(previewAsset.element_type)">
            <div class="asset-preview-formula">
              <div class="formula-display bg-gray-50 rounded-lg p-6 text-center">
                <code v-if="previewAsset.content_text" class="text-base text-blue-700 font-mono whitespace-pre-wrap">{{ previewAsset.content_text }}</code>
                <code v-else-if="previewAsset.ocr_text" class="text-base text-blue-700 font-mono whitespace-pre-wrap">{{ previewAsset.ocr_text }}</code>
                <span v-else class="text-gray-400">公式内容未提取</span>
              </div>
            </div>
          </template>

          <!-- Image / Figure -->
          <template v-else>
            <div class="asset-preview-image">
              <img
                :src="getPreviewImageSrc(previewAsset)"
                :alt="previewAsset.caption || 'asset'"
                class="max-w-full max-h-[60vh] object-contain mx-auto"
              />
            </div>
          </template>

          <!-- Metadata bar -->
          <div class="asset-preview-meta mt-4 pt-3 border-t border-gray-200 flex items-center gap-4 flex-wrap text-xs text-gray-500">
            <span>{{ getTypeIcon(previewAsset.element_type) }} {{ getTypeLabel(previewAsset.element_type) }}</span>
            <span v-if="previewAsset.page_number">第 {{ previewAsset.page_number }} 页</span>
            <span v-if="previewAsset.bbox">区域: {{ previewAsset.bbox }}</span>

            <!-- "查看原始截图" button -->
            <a
              v-if="getPreviewImageSrc(previewAsset)"
              :href="getPreviewImageSrc(previewAsset)"
              target="_blank"
              rel="noopener noreferrer"
              class="asset-preview-raw-btn ml-auto"
            >
              查看原始截图
              <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 13v6a2 2 0 01-2 2H5a2 2 0 01-2-2V8a2 2 0 012-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
            </a>
          </div>

          <!-- OCR text if available -->
          <div v-if="previewAsset.ocr_text && !isFormulaType(previewAsset.element_type)" class="asset-preview-ocr mt-3">
            <p class="text-xs text-gray-400 mb-1">OCR 文本:</p>
            <p class="text-sm text-gray-700 bg-gray-50 rounded p-3 whitespace-pre-wrap">{{ previewAsset.ocr_text }}</p>
          </div>
        </div>
      </template>
    </Modal>
  </div>
</template>

<style scoped>
.asset-gallery {
  width: 100%;
  --color-bg-elevated: var(--card-background-tmd, var(--bg-primary, #fff));
  --color-bg-sunken: var(--background-color, rgba(0,0,0,0.03));
  --color-border: var(--border-color, #e2e8f0);
  --color-text-secondary: var(--text-secondary, #64748b);
  --color-text-tertiary: var(--text-secondary, #94a3b8);
  --color-primary-soft: color-mix(in srgb, var(--color-primary, #3b82f6) 12%, transparent);
  --radius-md: 8px;
  --radius-sm: 4px;
}

/* ---- filters ---- */
.asset-gallery-filters {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 16px;
  flex-wrap: wrap;
}

.asset-filter-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 12px;
  border-radius: var(--radius-md);
  font-size: 12px;
  color: var(--color-text-tertiary);
  background: transparent;
  border: 1px solid transparent;
  cursor: pointer;
  transition: all 0.15s;
}
.asset-filter-btn:hover {
  color: var(--color-text-secondary);
  background: var(--color-bg-sunken);
}
.asset-filter-btn--active {
  color: var(--color-primary);
  background: var(--color-primary-soft);
  border-color: rgba(37, 99, 235, 0.2);
  font-weight: 500;
}

.asset-filter-count {
  font-size: 10px;
  opacity: 0.6;
  margin-left: 2px;
}

.asset-filter-total {
  flex-shrink: 0;
}

/* ---- grid ---- */
.asset-gallery-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 12px;
}

/* ---- card ---- */
.asset-card {
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  overflow: hidden;
  background: var(--color-bg-elevated);
  cursor: pointer;
  transition: all 0.15s;
  display: flex;
  flex-direction: column;
}
.asset-card:hover {
  border-color: var(--color-primary);
  box-shadow: 0 2px 8px rgba(37, 99, 235, 0.1);
  transform: translateY(-1px);
}

.asset-card-thumb {
  position: relative;
  aspect-ratio: 16 / 11;
  background: var(--color-bg-sunken);
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}

.asset-card-img {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}

.asset-card-table-preview {
  width: 100%;
  height: 100%;
  overflow: auto;
  padding: 6px;
  transform-origin: top left;
  transform: scale(0.55);
}
.asset-card-table-preview :deep(table) {
  font-size: 7px;
  border-collapse: collapse;
}
.asset-card-table-preview :deep(td),
.asset-card-table-preview :deep(th) {
  border: 1px solid #d1d5db;
  padding: 1px 3px;
}
.asset-card-table-preview :deep(th) {
  background: #f3f4f6;
}

.asset-card-formula {
  padding: 8px;
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.asset-card-page-badge {
  position: absolute;
  top: 6px;
  right: 6px;
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
  font-size: 10px;
  font-weight: 500;
  padding: 1px 6px;
  border-radius: 4px;
}

.asset-card-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 8px;
  border-top: 1px solid var(--color-border);
  min-height: 32px;
}

.asset-card-type-badge {
  font-size: 10px;
  font-weight: 500;
  padding: 1px 6px;
  border-radius: 4px;
}

.asset-card-edit-btn {
  font-size: 10px;
  color: var(--color-primary);
  background: none;
  border: none;
  cursor: pointer;
  padding: 1px 4px;
  border-radius: 3px;
}
.asset-card-edit-btn:hover {
  background: var(--color-primary-soft);
}

.asset-card-caption {
  font-size: 11px;
  color: var(--color-text-tertiary);
  padding: 4px 8px 8px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* ---- preview modal ---- */
.asset-preview-container {
  max-height: 70vh;
  overflow-y: auto;
}

.asset-preview-image {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  background: var(--color-bg-sunken);
  border-radius: var(--radius-md);
  min-height: 120px;
}

.asset-preview-table-wrap {
  overflow: auto;
  max-height: 50vh;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  padding: 12px;
  background: #fff;
}

.asset-preview-table :deep(table) {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}
.asset-preview-table :deep(td),
.asset-preview-table :deep(th) {
  border: 1px solid #e2e8f0;
  padding: 6px 12px;
  text-align: left;
}
.asset-preview-table :deep(th) {
  background: #f8fafc;
  font-weight: 600;
  color: var(--color-text-secondary);
}
.asset-preview-table :deep(tr:nth-child(even)) {
  background: #fafbfc;
}

.asset-preview-formula {
  padding: 8px 0;
}

.formula-display {
  min-height: 80px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.asset-preview-meta {
  display: flex;
  align-items: center;
}

.asset-preview-raw-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--color-primary);
  text-decoration: none;
  padding: 4px 10px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-sm);
  transition: all 0.15s;
}
.asset-preview-raw-btn:hover {
  background: var(--color-primary-soft);
  border-color: var(--color-primary);
}

.asset-preview-ocr {
  border-top: 1px solid #f1f5f9;
  padding-top: 12px;
}
</style>
