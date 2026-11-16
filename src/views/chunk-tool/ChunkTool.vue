<script setup lang="ts">
import { ref, onActivated } from 'vue';
import { chunkToolApi, kgServiceReady, ensureKgReady } from '@/api/knowledge';
import { useUIStore } from '@/stores/index';
import type { AlgorithmInfo, ChunkCompareResponse, AlgorithmChunkResult, ChunkStats } from '@/types/knowledge';
import LoadingSpinner from '@/components/common/LoadingSpinner.vue';
import EmptyState from '@/components/common/EmptyState.vue';
import MarkdownRenderer from '@/components/markdown/MarkdownRenderer.vue';

const uiStore = useUIStore();

// 算法列表
const algorithms = ref<AlgorithmInfo[]>([]);
const selectedAlgorithms = ref<string[]>(['semantic', 'markdown', 'fixed_size']);
const algorithmsLoading = ref(false);

// 输入
const inputMethod = ref<'text' | 'file'>('text');
const textContent = ref('');
const uploadedFile = ref<File | null>(null);

// 参数配置
const targetSize = ref(500);
const maxSize = ref(1000);
const minSize = ref(100);
const overlap = ref(50);

// 结果
const compareResults = ref<{
  original_char_count: number;
  results: AlgorithmChunkResult[];
  recommended: string;
  recommendation_reason: string;
  total_elapsed_ms: number;
} | null>(null);
const isComparing = ref(false);

// 查看详情
const selectedResult = ref<AlgorithmChunkResult | null>(null);
const selectedChunkIndex = ref(0);

// 加载算法列表
async function loadAlgorithms() {
  algorithmsLoading.value = true;
  try {
    const response = await chunkToolApi.listAlgorithms();
    algorithms.value = response || [];
  } catch (e) {
    console.error('加载算法列表失败:', e);
  } finally {
    algorithmsLoading.value = false;
  }
}

// 执行对比
async function runCompare() {
  if (selectedAlgorithms.value.length === 0) {
    uiStore.showToast('warning', '请至少选择一个算法');
    return;
  }

  if (inputMethod.value === 'text' && !textContent.value.trim()) {
    uiStore.showToast('warning', '请输入文本内容');
    return;
  }

  if (inputMethod.value === 'file' && !uploadedFile.value) {
    uiStore.showToast('warning', '请上传文件');
    return;
  }

  isComparing.value = true;
  compareResults.value = null;

  try {
    let response;
    
    if (inputMethod.value === 'file' && uploadedFile.value) {
      const formData = new FormData();
      formData.append('file', uploadedFile.value);
      formData.append('algorithms', JSON.stringify(selectedAlgorithms.value));
      formData.append('target_size', String(targetSize.value));
      formData.append('max_size', String(maxSize.value));
      formData.append('min_size', String(minSize.value));
      formData.append('overlap', String(overlap.value));
      
      response = await chunkToolApi.compareFile(formData);
    } else {
      response = await chunkToolApi.compare({
        content: textContent.value,
        algorithms: selectedAlgorithms.value,
        target_size: targetSize.value,
        max_size: maxSize.value,
        min_size: minSize.value,
        overlap: overlap.value,
      });
    }

    if (response) {
      compareResults.value = response;
      uiStore.showToast('success', `对比完成，推荐算法: ${response.recommended}`);
    }
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '对比失败');
  } finally {
    isComparing.value = false;
  }
}

// 文件上传处理
function handleFileUpload(event: Event) {
  const input = event.target as HTMLInputElement;
  if (input.files && input.files[0]) {
    uploadedFile.value = input.files[0];
  }
}

function removeFile() {
  uploadedFile.value = null;
}

// 选择算法切换
function toggleAlgorithm(code: string) {
  const index = selectedAlgorithms.value.indexOf(code);
  if (index === -1) {
    selectedAlgorithms.value.push(code);
  } else {
    selectedAlgorithms.value.splice(index, 1);
  }
}

// 工具函数
function formatSize(chars: number): string {
  if (chars < 1000) return chars + ' 字符';
  return (chars / 1000).toFixed(1) + 'K 字符';
}

function getCategoryColor(category: string): string {
  const colors: Record<string, string> = {
    'semantic': 'bg-primary-500/20 text-primary-400',
    'structural': 'bg-ocean-500/20 text-ocean-400',
    'fixed': 'bg-warning-500/20 text-warning-400',
  };
  return colors[category] || 'bg-slate-100 text-slate-700 dark:bg-dark-600 dark:text-dark-300';
}

function getDistributionData(stats: ChunkStats) {
  if (!stats.size_distribution) return [];
  return Object.entries(stats.size_distribution).map(([range, count]) => ({
    range,
    count,
    percentage: stats.total_chunks > 0 ? (count / stats.total_chunks) * 100 : 0,
  }));
}

// 初始化（服务就绪时才加载）
function initAlgorithms() {
  ensureKgReady({ startIfNeeded: false }).then(ready => {
    if (ready) loadAlgorithms();
  });
}
initAlgorithms();
onActivated(initAlgorithms);
</script>

<template>
  <div class="space-y-6">
    <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
      <!-- 左侧：配置面板 -->
      <div class="space-y-4">
        <!-- 算法选择 -->
        <div class="card">
          <h3 class="text-lg font-semibold text-slate-900 dark:text-dark-100 mb-4 flex items-center gap-2">
            <svg class="w-5 h-5 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m18-6h-2m2 6h-2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2zM9 9h6v6H9V9z" />
            </svg>
            选择算法
          </h3>

          <div v-if="algorithmsLoading" class="py-4">
            <LoadingSpinner text="加载算法列表..." />
          </div>

          <div v-else class="space-y-2">
            <div
              v-for="algo in algorithms"
              :key="algo.code"
              @click="toggleAlgorithm(algo.code)"
              :class="[
                'p-3 rounded-lg border-2 cursor-pointer transition-all',
                selectedAlgorithms.includes(algo.code)
                  ? 'border-primary-500 bg-primary-500/10'
                  : 'border-slate-200 hover:border-slate-300 dark:border-dark-700 dark:hover:border-dark-600'
              ]"
            >
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-3">
                  <input
                    type="checkbox"
                    :checked="selectedAlgorithms.includes(algo.code)"
                    class="w-4 h-4 rounded accent-primary-500"
                    @click.stop
                  />
                  <div>
                    <p class="font-medium text-slate-900 dark:text-dark-200">{{ algo.name }}</p>
                    <p class="text-xs text-slate-500 dark:text-dark-500">{{ algo.code }}</p>
                  </div>
                </div>
                <span :class="['badge text-xs', getCategoryColor(algo.category)]">
                  {{ algo.category }}
                </span>
              </div>
              <p class="text-sm text-slate-600 dark:text-dark-400 mt-2">{{ algo.description }}</p>
              <div class="flex gap-2 mt-2">
                <span v-if="algo.supports_overlap" class="text-xs text-slate-500 dark:text-dark-500">✓ 支持重叠</span>
                <span v-if="algo.supports_hierarchy" class="text-xs text-slate-500 dark:text-dark-500">✓ 支持层级</span>
              </div>
            </div>
          </div>
        </div>

        <!-- 参数配置 -->
        <div class="card">
          <h3 class="text-lg font-semibold text-slate-900 dark:text-dark-100 mb-4 flex items-center gap-2">
            <svg class="w-5 h-5 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4" />
            </svg>
            参数配置
          </h3>

          <div class="space-y-4">
            <div>
              <label class="block text-sm text-slate-600 dark:text-dark-400 mb-2">目标切片大小</label>
              <input v-model.number="targetSize" type="number" class="input" min="100" max="5000" step="100" />
              <p class="text-xs text-slate-500 dark:text-dark-500 mt-1">推荐: 300-800 字符</p>
            </div>

            <div>
              <label class="block text-sm text-slate-600 dark:text-dark-400 mb-2">最大切片大小</label>
              <input v-model.number="maxSize" type="number" class="input" min="200" max="10000" step="100" />
            </div>

            <div>
              <label class="block text-sm text-slate-600 dark:text-dark-400 mb-2">最小切片大小</label>
              <input v-model.number="minSize" type="number" class="input" min="50" max="1000" step="50" />
            </div>

            <div>
              <label class="block text-sm text-slate-600 dark:text-dark-400 mb-2">重叠大小</label>
              <input v-model.number="overlap" type="number" class="input" min="0" max="500" step="10" />
              <p class="text-xs text-slate-500 dark:text-dark-500 mt-1">建议: 目标大小的 10-20%</p>
            </div>
          </div>
        </div>
      </div>

      <!-- 中间：输入区域 -->
      <div class="space-y-4">
        <div class="card">
          <h3 class="text-lg font-semibold text-slate-900 dark:text-dark-100 mb-4">输入内容</h3>

          <!-- 输入方式切换 -->
          <div class="flex gap-2 mb-4">
            <button
              @click="inputMethod = 'text'"
              :class="[
                'px-4 py-2 rounded-lg text-sm font-medium transition-colors',
                inputMethod === 'text' ? 'bg-primary-600 text-white' : 'bg-slate-100 text-slate-700 hover:bg-slate-200 dark:bg-dark-700 dark:text-dark-300 dark:hover:bg-dark-600'
              ]"
            >
              文本输入
            </button>
            <button
              @click="inputMethod = 'file'"
              :class="[
                'px-4 py-2 rounded-lg text-sm font-medium transition-colors',
                inputMethod === 'file' ? 'bg-primary-600 text-white' : 'bg-slate-100 text-slate-700 hover:bg-slate-200 dark:bg-dark-700 dark:text-dark-300 dark:hover:bg-dark-600'
              ]"
            >
              文件上传
            </button>
          </div>

          <!-- 文本输入 -->
          <div v-if="inputMethod === 'text'">
            <textarea
              v-model="textContent"
              rows="15"
              class="input font-mono text-sm"
              placeholder="粘贴或输入要测试的文本内容...

支持 Markdown 格式，系统会根据文本结构分析最佳切片方案。

建议测试内容至少 1000 字符以上，以便更好地评估各算法效果。"
            ></textarea>
            <p class="text-xs text-slate-500 dark:text-dark-500 mt-2">
              当前字符数: <strong class="text-slate-700 dark:text-dark-300">{{ textContent.length.toLocaleString() }}</strong>
            </p>
          </div>

          <!-- 文件上传 -->
          <div v-else>
            <div
              v-if="!uploadedFile"
              class="border-2 border-dashed border-slate-200 dark:border-dark-600 rounded-xl p-8 text-center hover:border-primary-500 hover:bg-slate-50 dark:hover:bg-transparent transition-colors cursor-pointer"
              @click="($refs.fileInput as HTMLInputElement).click()"
              @dragover.prevent
              @drop.prevent="handleFileUpload"
            >
              <input
                ref="fileInput"
                type="file"
                accept=".txt,.md,.doc,.docx,.pdf"
                class="hidden"
                @change="handleFileUpload"
              />
              <svg class="w-12 h-12 text-slate-500 dark:text-dark-500 mx-auto mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
              </svg>
              <p class="text-slate-700 dark:text-dark-300">点击或拖拽文件到此处上传</p>
              <p class="text-xs text-slate-500 dark:text-dark-500 mt-2">支持 .txt, .md, .doc, .docx, .pdf</p>
            </div>

            <div v-else class="p-4 rounded-lg bg-slate-50 dark:bg-dark-700/50 flex items-center justify-between border border-slate-200 dark:border-transparent">
              <div class="flex items-center gap-3">
                <div class="w-10 h-10 rounded-lg bg-primary-500/20 flex items-center justify-center">
                  <svg class="w-5 h-5 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                  </svg>
                </div>
                <div>
                  <p class="font-medium text-slate-900 dark:text-dark-200">{{ uploadedFile.name }}</p>
                  <p class="text-xs text-slate-500 dark:text-dark-500">{{ (uploadedFile.size / 1024).toFixed(1) }} KB</p>
                </div>
              </div>
              <button @click="removeFile" class="p-2 rounded-lg text-slate-500 dark:text-dark-400 hover:text-danger-600 dark:hover:text-danger-400 hover:bg-danger-50 dark:hover:bg-danger-500/10">
                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
          </div>

          <!-- 执行按钮 -->
          <button
            @click="runCompare"
            :disabled="isComparing || selectedAlgorithms.length === 0"
            class="btn btn-primary w-full mt-4"
          >
            <LoadingSpinner v-if="isComparing" size="sm" />
            <template v-else>
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-3 7h3m-3 4h3m-6-4h.01M9 16h.01" />
              </svg>
              开始对比
            </template>
          </button>
        </div>
      </div>

      <!-- 右侧：结果展示 -->
      <div class="space-y-4">
        <div v-if="!compareResults && !isComparing" class="card">
          <EmptyState
            icon="task"
            title="等待对比"
            description="配置参数并输入内容后，点击开始对比按钮"
          />
        </div>

        <div v-else-if="isComparing" class="card">
          <div class="py-12">
            <LoadingSpinner size="lg" text="正在分析切片效果..." />
          </div>
        </div>

        <template v-else-if="compareResults">
          <!-- 推荐算法 -->
          <div class="card bg-gradient-to-br from-primary-500/20 to-ocean-500/20 border-primary-500/30">
            <div class="flex items-center gap-4">
              <div class="w-14 h-14 rounded-xl bg-primary-500/30 flex items-center justify-center">
                <svg class="w-7 h-7 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4M7.835 4.697a3.42 3.42 0 001.946-.806 3.42 3.42 0 014.438 0 3.42 3.42 0 001.946.806 3.42 3.42 0 013.138 3.138 3.42 3.42 0 00.806 1.946 3.42 3.42 0 010 4.438 3.42 3.42 0 00-.806 1.946 3.42 3.42 0 01-3.138 3.138 3.42 3.42 0 00-1.946.806 3.42 3.42 0 01-4.438 0 3.42 3.42 0 00-1.946-.806 3.42 3.42 0 01-3.138-3.138 3.42 3.42 0 00-.806-1.946 3.42 3.42 0 010-4.438 3.42 3.42 0 00.806-1.946 3.42 3.42 0 013.138-3.138z" />
                </svg>
              </div>
              <div class="flex-1">
                <p class="text-sm text-slate-600 dark:text-dark-400">推荐算法</p>
                <p class="text-xl font-bold text-slate-900 dark:text-dark-100">{{ compareResults.recommended }}</p>
                <p class="text-sm text-slate-600 dark:text-dark-400 mt-1">{{ compareResults.recommendation_reason }}</p>
              </div>
            </div>
          </div>

          <!-- 统计概览 -->
          <div class="card">
            <div class="grid grid-cols-3 gap-4 text-center">
              <div>
                <p class="text-2xl font-bold text-slate-900 dark:text-dark-100">{{ formatSize(compareResults.original_char_count) }}</p>
                <p class="text-xs text-slate-500 dark:text-dark-500">原文长度</p>
              </div>
              <div>
                <p class="text-2xl font-bold text-slate-900 dark:text-dark-100">{{ compareResults.results.length }}</p>
                <p class="text-xs text-slate-500 dark:text-dark-500">对比算法数</p>
              </div>
              <div>
                <p class="text-2xl font-bold text-slate-900 dark:text-dark-100">{{ compareResults.total_elapsed_ms }}ms</p>
                <p class="text-xs text-slate-500 dark:text-dark-500">总耗时</p>
              </div>
            </div>
          </div>

          <!-- 各算法结果 -->
          <div class="space-y-3">
            <div
              v-for="result in compareResults.results"
              :key="result.algorithm"
              :class="[
                'card cursor-pointer transition-all',
                result.algorithm === compareResults.recommended ? 'ring-2 ring-primary-500' : 'hover:bg-slate-50 dark:hover:bg-dark-750'
              ]"
              @click="selectedResult = result; selectedChunkIndex = 0"
            >
              <div class="flex items-center justify-between mb-3">
                <div class="flex items-center gap-3">
                  <span class="font-semibold text-slate-900 dark:text-dark-100">{{ result.algorithm }}</span>
                  <span v-if="result.algorithm === compareResults.recommended" class="badge badge-success text-xs">
                    推荐
                  </span>
                </div>
                <span class="text-sm text-slate-500 dark:text-dark-500">{{ result.elapsed_ms }}ms</span>
              </div>

              <p class="text-sm text-slate-600 dark:text-dark-400 mb-3">{{ result.description }}</p>

              <div v-if="result.error" class="text-sm text-danger-400 mb-3">
                ❌ {{ result.error }}
              </div>

              <div v-else class="grid grid-cols-4 gap-3 text-center">
                <div>
                  <p class="text-lg font-semibold text-slate-900 dark:text-dark-200">{{ result.chunk_count }}</p>
                  <p class="text-xs text-slate-500 dark:text-dark-500">切片数</p>
                </div>
                <div>
                  <p class="text-lg font-semibold text-slate-900 dark:text-dark-200">{{ result.stats.avg_size.toFixed(0) }}</p>
                  <p class="text-xs text-slate-500 dark:text-dark-500">平均大小</p>
                </div>
                <div>
                  <p class="text-lg font-semibold text-slate-900 dark:text-dark-200">{{ result.stats.std_dev.toFixed(1) }}</p>
                  <p class="text-xs text-slate-500 dark:text-dark-500">标准差</p>
                </div>
                <div>
                  <p :class="[
                    'text-lg font-semibold',
                    result.stats.oversized_count > 0 ? 'text-danger-400' : 'text-success-400'
                  ]">{{ result.stats.oversized_count }}</p>
                  <p class="text-xs text-slate-500 dark:text-dark-500">超标切片</p>
                </div>
              </div>

              <!-- 大小分布条 -->
              <div v-if="!result.error" class="mt-4">
                <div class="flex gap-1 h-4 rounded-full overflow-hidden bg-slate-200 dark:bg-dark-700">
                  <div
                    v-for="(item, idx) in getDistributionData(result.stats).slice(0, 5)"
                    :key="idx"
                    :style="{ width: item.percentage + '%' }"
                    :class="[
                      'h-full',
                      idx === 0 ? 'bg-success-500' : idx === 1 ? 'bg-primary-500' : idx === 2 ? 'bg-ocean-500' : idx === 3 ? 'bg-warning-500' : 'bg-danger-500'
                    ]"
                    :title="`${item.range}: ${item.count}个 (${item.percentage.toFixed(1)}%)`"
                  ></div>
                </div>
              </div>
            </div>
          </div>
        </template>
      </div>
    </div>

    <!-- 切片详情面板 -->
    <div v-if="selectedResult" class="card">
      <div class="flex items-center justify-between mb-4">
        <h3 class="text-lg font-semibold text-slate-900 dark:text-dark-100">
          {{ selectedResult.algorithm }} - 切片详情
        </h3>
        <button @click="selectedResult = null" class="btn btn-secondary text-sm">
          关闭
        </button>
      </div>

      <!-- 切片导航 -->
      <div class="flex items-center gap-4 mb-4">
        <button
          @click="selectedChunkIndex = Math.max(0, selectedChunkIndex - 1)"
          :disabled="selectedChunkIndex === 0"
          class="btn btn-secondary"
        >
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
          </svg>
        </button>

        <div class="flex-1">
          <div class="flex gap-1 overflow-x-auto py-2">
            <button
              v-for="idx in selectedResult.chunks.length"
              :key="idx"
              @click="selectedChunkIndex = idx - 1"
              :class="[
                'flex-shrink-0 w-8 h-8 rounded-lg text-xs font-medium transition-colors',
                selectedChunkIndex === idx - 1
                  ? 'bg-primary-600 text-white'
                  : 'bg-slate-100 text-slate-700 hover:bg-slate-200 dark:bg-dark-700 dark:text-dark-400 dark:hover:bg-dark-600'
              ]"
            >
              {{ idx }}
            </button>
          </div>
        </div>

        <button
          @click="selectedChunkIndex = Math.min(selectedResult.chunks.length - 1, selectedChunkIndex + 1)"
          :disabled="selectedChunkIndex === selectedResult.chunks.length - 1"
          class="btn btn-secondary"
        >
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
          </svg>
        </button>
      </div>

      <!-- 当前切片内容 -->
      <div v-if="selectedResult.chunks[selectedChunkIndex]" class="space-y-4">
        <div class="flex items-center gap-4 text-sm text-slate-600 dark:text-dark-400">
          <span>切片 {{ selectedChunkIndex + 1 }} / {{ selectedResult.chunks.length }}</span>
          <span>|</span>
          <span>{{ selectedResult.chunks[selectedChunkIndex].char_count }} 字符</span>
          <span v-if="selectedResult.chunks[selectedChunkIndex].has_table" class="badge badge-ocean text-xs">含表格</span>
          <span v-if="selectedResult.chunks[selectedChunkIndex].has_code" class="badge badge-warning text-xs">含代码</span>
        </div>

        <div v-if="selectedResult.chunks[selectedChunkIndex].title_path?.length" class="flex items-center gap-2 text-sm">
          <span class="text-slate-500 dark:text-dark-500">标题路径:</span>
          <span v-for="(title, i) in selectedResult.chunks[selectedChunkIndex].title_path" :key="i" class="flex items-center gap-1">
            <span v-if="i > 0" class="text-slate-400 dark:text-dark-600">/</span>
            <span class="text-slate-700 dark:text-dark-300">{{ title }}</span>
          </span>
        </div>

        <div class="p-4 rounded-lg bg-slate-50 dark:bg-dark-800/50 max-h-96 overflow-y-auto border border-slate-200 dark:border-transparent">
          <MarkdownRenderer :content="selectedResult.chunks[selectedChunkIndex].content" />
        </div>

        <!-- 上下文预览 -->
        <div class="grid grid-cols-2 gap-4">
          <div v-if="selectedResult.chunks[selectedChunkIndex].prev_context" class="p-3 rounded-lg bg-white dark:bg-dark-700/50 border border-slate-200 dark:border-transparent">
            <p class="text-xs text-slate-500 dark:text-dark-500 mb-2">上文上下文:</p>
            <p class="text-sm text-slate-600 dark:text-dark-400 line-clamp-3">{{ selectedResult.chunks[selectedChunkIndex].prev_context }}</p>
          </div>
          <div v-if="selectedResult.chunks[selectedChunkIndex].next_context" class="p-3 rounded-lg bg-white dark:bg-dark-700/50 border border-slate-200 dark:border-transparent">
            <p class="text-xs text-slate-500 dark:text-dark-500 mb-2">下文上下文:</p>
            <p class="text-sm text-slate-600 dark:text-dark-400 line-clamp-3">{{ selectedResult.chunks[selectedChunkIndex].next_context }}</p>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
