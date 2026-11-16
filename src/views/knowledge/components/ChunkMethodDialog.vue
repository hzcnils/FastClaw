<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { chunkToolApi } from '@/api/knowledge';
import Modal from '@/components/common/Modal.vue';
import LoadingSpinner from '@/components/common/LoadingSpinner.vue';

const props = withDefaults(defineProps<{
  show: boolean;
  content?: string;
  /** 父级已加载的算法列表，优先使用，避免弹窗内重复请求失败导致无选项 */
  algorithms?: Array<{ code: string; name: string; description?: string; category?: string }>;
  initialAlgorithm?: string;
  initialTargetSize?: number;
  initialOverlap?: number;
  initialMinSize?: number;
  initialMaxSize?: number;
}>(), {
  algorithms: () => [],
  initialAlgorithm: 'recursive',
  initialTargetSize: 512,
  initialOverlap: 50,
  initialMinSize: 100,
  initialMaxSize: 2000,
});

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'apply', payload: {
    algorithm: string;
    targetSize: number;
    overlap: number;
    minSize: number;
    maxSize: number;
  }): void;
}>();

const localAlgorithms = ref<Array<{ code: string; name: string; description?: string; category?: string }>>([]);
const algorithm = ref('recursive');
const targetSize = ref(512);
const overlap = ref(50);
const minSize = ref(100);
const maxSize = ref(2000);
const loading = ref(false);
const previewResult = ref<any>(null);
const previewText = ref('');

/** 按 code 去重，避免接口重复项导致 el-option 错乱 */
function uniqueByCode<T extends { code: string }>(list: T[]): T[] {
  const seen = new Set<string>();
  return list.filter((item) => {
    if (!item?.code || seen.has(item.code)) return false;
    seen.add(item.code);
    return true;
  });
}

const algorithms = computed(() =>
  uniqueByCode((props.algorithms?.length ? props.algorithms : localAlgorithms.value) || [])
);

const selectedAlgo = computed(() => algorithms.value.find(a => a.code === algorithm.value));

function ensureValidAlgorithm() {
  const list = algorithms.value;
  if (!list.length) return;
  if (!list.some((a) => a.code === algorithm.value)) {
    algorithm.value = list.find((a) => a.code === 'recursive')?.code || list[0].code;
  }
}

type NumField = 'targetSize' | 'overlap' | 'minSize' | 'maxSize';

const numBounds: Record<NumField, { min: number; max: number }> = {
  targetSize: { min: 50, max: 5000 },
  overlap: { min: 0, max: 1000 },
  minSize: { min: 20, max: 5000 },
  maxSize: { min: 50, max: 10000 },
};

function bumpNumber(field: NumField, delta: number) {
  const bounds = numBounds[field];
  const map = { targetSize, overlap, minSize, maxSize };
  const cur = Number(map[field].value) || 0;
  map[field].value = Math.min(bounds.max, Math.max(bounds.min, cur + delta));
  previewResult.value = null;
}

async function loadAlgorithms() {
  if (props.algorithms?.length) {
    localAlgorithms.value = uniqueByCode(props.algorithms);
    ensureValidAlgorithm();
    return;
  }
  try {
    const res = await chunkToolApi.listAlgorithms();
    localAlgorithms.value = uniqueByCode(Array.isArray(res) ? res : []);
  } catch { /* ignore */ }
  ensureValidAlgorithm();
}

function syncFromProps() {
  algorithm.value = props.initialAlgorithm || 'recursive';
  targetSize.value = props.initialTargetSize ?? 512;
  overlap.value = props.initialOverlap ?? 50;
  minSize.value = props.initialMinSize ?? 100;
  maxSize.value = props.initialMaxSize ?? 2000;
}

async function runPreview() {
  if (!props.content) return;
  loading.value = true;
  previewResult.value = null;
  try {
    const data = await chunkToolApi.preview({
      content: props.content.slice(0, 5000),
      algorithm: algorithm.value,
      common: {
        target_size: targetSize.value,
        overlap: overlap.value,
        min_size: minSize.value,
        max_size: maxSize.value,
      },
    });
    previewResult.value = data;
    previewText.value = (data as any).sample_chunks?.map((c: any) => c.content).join('\n\n---\n\n') || '';
  } catch { /* ignore */ }
  finally { loading.value = false; }
}

function apply() {
  emit('apply', {
    algorithm: algorithm.value,
    targetSize: targetSize.value,
    overlap: overlap.value,
    minSize: minSize.value,
    maxSize: maxSize.value,
  });
}

function onAlgoChange() {
  previewResult.value = null;
}

watch(() => props.show, (v) => {
  if (v) {
    syncFromProps();
    loadAlgorithms();
    previewResult.value = null;
  }
});
</script>

<template>
  <Modal :show="show" title="分块方法选择" size="2xl" @close="emit('close')">
    <div class="chunk-method-dialog space-y-4">
      <!-- 算法选择：对齐创建知识库弹窗 el-select（勿加 .select，会叠双箭头） -->
      <div>
        <label class="cmd-label">切片算法</label>
        <el-select
          v-model="algorithm"
          class="cmd-algo-select"
          teleported
          popper-class="chunk-method-select-popper"
          @change="onAlgoChange"
        >
          <el-option
            v-for="alg in algorithms"
            :key="alg.code"
            :label="`${alg.name} — ${alg.description || alg.category || alg.code}`"
            :value="alg.code"
          />
        </el-select>
        <p v-if="selectedAlgo?.description" class="cmd-hint">
          {{ selectedAlgo.description }}
        </p>
      </div>

      <!-- 参数：自定义上下箭头（透明底，对齐创建知识库） -->
      <div class="grid grid-cols-4 gap-3">
        <div>
          <label class="cmd-field-label">目标大小</label>
          <div class="cmd-number-wrap">
            <input
              v-model.number="targetSize"
              type="number"
              min="50"
              max="5000"
              class="input w-full text-sm cmd-number-input"
              @change="previewResult = null"
            />
            <div class="cmd-number-arrows">
              <button type="button" class="cmd-number-arrow" @click="bumpNumber('targetSize', 1)">
                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                  <polyline points="18 15 12 9 6 15" />
                </svg>
              </button>
              <button type="button" class="cmd-number-arrow" @click="bumpNumber('targetSize', -1)">
                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                  <polyline points="6 9 12 15 18 9" />
                </svg>
              </button>
            </div>
          </div>
        </div>
        <div>
          <label class="cmd-field-label">重叠</label>
          <div class="cmd-number-wrap">
            <input
              v-model.number="overlap"
              type="number"
              min="0"
              max="1000"
              class="input w-full text-sm cmd-number-input"
              @change="previewResult = null"
            />
            <div class="cmd-number-arrows">
              <button type="button" class="cmd-number-arrow" @click="bumpNumber('overlap', 1)">
                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                  <polyline points="18 15 12 9 6 15" />
                </svg>
              </button>
              <button type="button" class="cmd-number-arrow" @click="bumpNumber('overlap', -1)">
                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                  <polyline points="6 9 12 15 18 9" />
                </svg>
              </button>
            </div>
          </div>
        </div>
        <div>
          <label class="cmd-field-label">最小</label>
          <div class="cmd-number-wrap">
            <input
              v-model.number="minSize"
              type="number"
              min="20"
              max="5000"
              class="input w-full text-sm cmd-number-input"
              @change="previewResult = null"
            />
            <div class="cmd-number-arrows">
              <button type="button" class="cmd-number-arrow" @click="bumpNumber('minSize', 1)">
                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                  <polyline points="18 15 12 9 6 15" />
                </svg>
              </button>
              <button type="button" class="cmd-number-arrow" @click="bumpNumber('minSize', -1)">
                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                  <polyline points="6 9 12 15 18 9" />
                </svg>
              </button>
            </div>
          </div>
        </div>
        <div>
          <label class="cmd-field-label">最大</label>
          <div class="cmd-number-wrap">
            <input
              v-model.number="maxSize"
              type="number"
              min="50"
              max="10000"
              class="input w-full text-sm cmd-number-input"
              @change="previewResult = null"
            />
            <div class="cmd-number-arrows">
              <button type="button" class="cmd-number-arrow" @click="bumpNumber('maxSize', 1)">
                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                  <polyline points="18 15 12 9 6 15" />
                </svg>
              </button>
              <button type="button" class="cmd-number-arrow" @click="bumpNumber('maxSize', -1)">
                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round">
                  <polyline points="6 9 12 15 18 9" />
                </svg>
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- 预览按钮 -->
      <div v-if="content" class="flex items-center gap-3">
        <button
          type="button"
          @click="runPreview"
          :disabled="loading"
          class="btn btn-secondary btn-sm disabled:opacity-50 cmd-preview-btn"
        >
          预览分块效果
        </button>
        <LoadingSpinner v-if="loading" size="sm" />
      </div>

      <!-- 预览结果：字号加大一号 -->
      <div
        v-if="previewResult"
        class="border rounded-lg p-3 max-h-64 overflow-auto cmd-preview-box"
      >
        <div class="cmd-preview-meta text-sm mb-2">
          {{ previewResult.total_chunks }} 块 · 平均 {{ previewResult.avg_quality ? (previewResult.avg_quality * 100).toFixed(0) + '%' : 'N/A' }} 质量 · {{ previewResult.total_chars }} 字符
        </div>
        <pre class="cmd-preview-text text-sm whitespace-pre-wrap">{{ previewText }}</pre>
      </div>
    </div>

    <template #footer>
      <button type="button" class="cmd-btn-cancel" @click="emit('close')">取消</button>
      <button type="button" class="cmd-btn-apply ml-2" @click="apply">应用</button>
    </template>
  </Modal>
</template>

<style scoped>
.chunk-method-dialog {
  font-size: var(--font-size-md);
}

.cmd-label {
  display: block;
  margin-bottom: 8px;
  font-size: var(--font-size-md);
  font-weight: 600;
  color: var(--text-primary);
}

.cmd-field-label {
  display: block;
  margin-bottom: 6px;
  font-size: var(--font-size-md);
  color: var(--text-secondary);
}

.cmd-hint {
  margin-top: 6px;
  font-size: var(--font-size-md);
  color: var(--text-secondary);
}

.cmd-algo-select {
  width: 100%;
}

.cmd-algo-select :deep(.el-select__wrapper) {
  background: transparent !important;
  border: 1px solid var(--border-color) !important;
  border-radius: 6px !important;
  height: 38px !important;
  min-height: 38px !important;
  box-shadow: none !important;
  box-sizing: border-box;
}
.cmd-algo-select :deep(.el-select__wrapper:hover),
.cmd-algo-select :deep(.el-select__wrapper.is-focused),
.cmd-algo-select :deep(.el-select__wrapper.is-focus),
.cmd-algo-select :deep(.el-select__wrapper.is-hovering) {
  border-color: var(--border-color) !important;
  box-shadow: none !important;
}
.cmd-algo-select :deep(.el-select__selected-item),
.cmd-algo-select :deep(.el-select__placeholder),
.cmd-algo-select :deep(.el-select__input) {
  color: var(--text-primary) !important;
  font-size: var(--font-size-md) !important;
}
.cmd-algo-select :deep(.el-select__caret) {
  color: var(--text-secondary) !important;
}

.cmd-number-wrap {
  position: relative;
  display: flex;
  align-items: center;
  width: 100%;
}
.cmd-number-input {
  width: 100%;
  height: 38px;
  padding-right: 28px !important;
  box-sizing: border-box;
  background: transparent !important;
  border: 1px solid var(--border-color) !important;
  border-radius: 6px !important;
  color: var(--text-primary) !important;
  font-size: var(--font-size-md) !important;
  box-shadow: none !important;
  -moz-appearance: textfield;
}
.cmd-number-input::-webkit-inner-spin-button,
.cmd-number-input::-webkit-outer-spin-button {
  -webkit-appearance: none;
  margin: 0;
  display: none;
}
.cmd-number-arrows {
  position: absolute;
  right: 1px;
  top: 1px;
  bottom: 1px;
  width: 26px;
  display: flex;
  flex-direction: column;
  background: transparent;
  border-radius: 0 5px 5px 0;
  overflow: hidden;
}
.cmd-number-arrow {
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
.cmd-number-arrow:hover {
  background: color-mix(in srgb, var(--text-primary) 12%, transparent);
}
.cmd-number-arrow + .cmd-number-arrow {
  border-top: 1px solid var(--border-color);
}
.cmd-number-arrow svg {
  width: 12px;
  height: 12px;
  display: block;
}

.cmd-preview-btn {
  padding: 8px 18px !important;
  border-radius: 4px !important;
  font-size: var(--font-size-md) !important;
  font-weight: 400 !important;
  cursor: pointer;
  transition: all 0.18s ease;
  color: var(--btn-secondary-text) !important;
  background: var(--cancel-btn-color) !important;
  border: 1px solid var(--btn-secondary-border) !important;
  box-shadow: none !important;
}
.cmd-preview-btn:hover:not(:disabled) {
  background: var(--cancel-btn-hover-color) !important;
  color: var(--btn-secondary-hover-text) !important;
  border-color: var(--btn-secondary-hover-border) !important;
}

.cmd-preview-box {
  border-color: var(--border-color);
  background: var(--background-color, rgba(0, 0, 0, 0.03));
}
.cmd-preview-meta {
  color: var(--text-secondary);
  font-size: var(--font-size-md) !important;
}
.cmd-preview-text {
  color: var(--text-primary);
  font-size: var(--font-size-md) !important;
  margin: 0;
  font-family: inherit;
}

/* 对齐创建知识库弹窗：取消 / 确认 */
.cmd-btn-cancel {
  padding: 8px 18px;
  border-radius: 4px;
  font-size: var(--font-size-md);
  cursor: pointer;
  transition: all 0.18s ease;
  color: var(--btn-secondary-text);
  background: var(--cancel-btn-color);
  border: 1px solid var(--btn-secondary-border);
}
.cmd-btn-cancel:hover {
  background: var(--cancel-btn-hover-color);
  color: var(--btn-secondary-hover-text);
  border-color: var(--btn-secondary-hover-border);
}

.cmd-btn-apply {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 8px 18px;
  background: var(--btn-primary-bg);
  color: var(--btn-primary-text);
  border: 1px solid var(--btn-primary-border);
  border-radius: 4px;
  font-size: var(--font-size-md);
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}
.cmd-btn-apply:hover {
  background: var(--btn-primary-hover-bg);
  color: var(--btn-primary-hover-text);
  border-color: var(--btn-primary-hover-border);
  transform: translateY(-1px);
  box-shadow: 0 2px 8px color-mix(in srgb, var(--btn-primary-hover-bg) 26%, transparent);
}
</style>

<style>
/* 分块方法弹窗下拉层级高于 Modal */
.chunk-method-select-popper {
  z-index: 10060 !important;
}
</style>
