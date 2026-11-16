<script setup lang="ts">
import { ref, onMounted, onActivated, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { fileApi, knowledgeBaseApi, kgServiceReady, ensureKgReady } from '@/api/knowledge';
import type { FileInfo, KnowledgeBase } from '@/types/knowledge';
import { useUIStore } from '@/stores/index';
import LoadingSpinner from '@/components/common/LoadingSpinner.vue';
import EmptyState from '@/components/common/EmptyState.vue';
import Modal from '@/components/common/Modal.vue';
import { useDialog } from '@/composables/useDialog';

const { t } = useI18n();
const { confirm: confirmDialog } = useDialog();

const uiStore = useUIStore();

const files = ref<FileInfo[]>([]);
const knowledgeBases = ref<KnowledgeBase[]>([]);
const loading = ref(true);
const total = ref(0);
const page = ref(1);
const pageSize = ref(20);

// 筛选
const filterStatus = ref('');
const filterCategory = ref('');
const filterKnowledgeBase = ref('');

// 上传
const showUploadModal = ref(false);
const uploadFile = ref<File | null>(null);
const uploadKnowledgeBase = ref('');
const uploadProgress = ref(0);
const uploadLoading = ref(false);

// 详情
const showDetailModal = ref(false);
const selectedFile = ref<FileInfo | null>(null);

async function fetchFiles() {
  loading.value = true;
  try {
    const response = await fileApi.list({
      status: filterStatus.value || undefined,
      category: filterCategory.value || undefined,
      knowledge_base_id: filterKnowledgeBase.value || undefined,
      page: page.value,
      page_size: pageSize.value,
    });
    files.value = response.items || [];
    total.value = response.total;
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '获取文件列表失败');
  } finally {
    loading.value = false;
  }
}

async function fetchKnowledgeBases() {
  try {
    knowledgeBases.value = await knowledgeBaseApi.list();
  } catch (e) {
    console.error('获取知识库列表失败', e);
  }
}

async function handleUpload() {
  if (!uploadFile.value) {
    uiStore.showToast('warning', '请选择文件');
    return;
  }
  if (!uploadKnowledgeBase.value) {
    uiStore.showToast('warning', '请选择知识库');
    return;
  }
  
  uploadLoading.value = true;
  uploadProgress.value = 0;
  try {
    const formData = new FormData();
    formData.append('file', uploadFile.value);
    formData.append('knowledge_base_id', uploadKnowledgeBase.value);
    
    await fileApi.upload(formData, (progress) => {
      uploadProgress.value = progress;
    });
    
    showUploadModal.value = false;
    uploadFile.value = null;
    uploadKnowledgeBase.value = '';
    uploadProgress.value = 0;
    uiStore.showToast('success', '上传成功');
    fetchFiles();
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '上传失败');
  } finally {
    uploadLoading.value = false;
  }
}

async function handleDelete(fileId: string) {
  if (!await confirmDialog('确定要删除此文件吗？相关的切片也会被删除。')) return;
  try {
    await fileApi.delete(fileId);
    uiStore.showToast('success', '删除成功');
    fetchFiles();
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '删除失败');
  }
}

async function showFileDetail(file: FileInfo) {
  selectedFile.value = file;
  showDetailModal.value = true;
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(2) + ' KB';
  if (bytes < 1024 * 1024 * 1024) return (bytes / 1024 / 1024).toFixed(2) + ' MB';
  return (bytes / 1024 / 1024 / 1024).toFixed(2) + ' GB';
}

function formatDate(dateStr: string): string {
  return new Date(dateStr).toLocaleString('zh-CN');
}

function getStatusClass(status: string): string {
  const classes: Record<string, string> = {
    pending: 'badge-secondary',
    processing: 'badge-warning',
    completed: 'badge-success',
    failed: 'badge-error',
  };
  return classes[status] || 'badge-secondary';
}

function getStatusText(status: string): string {
  const texts: Record<string, string> = {
    pending: '等待中',
    processing: '处理中',
    completed: '已完成',
    failed: '失败',
  };
  return texts[status] || status;
}

function getFileIcon(ext: string): string {
  const icons: Record<string, string> = {
    pdf: 'M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z',
    docx: 'M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z',
    doc: 'M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z',
    txt: 'M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z',
    md: 'M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z',
  };
  return icons[ext.toLowerCase()] || icons.txt;
}

const totalPages = computed(() => Math.ceil(total.value / pageSize.value));

function handlePageChange(newPage: number) {
  if (newPage >= 1 && newPage <= totalPages.value) {
    page.value = newPage;
    fetchFiles();
  }
}

function handleFilter() {
  page.value = 1;
  fetchFiles();
}

async function loadData() {
  await ensureKgReady({ startIfNeeded: false });
  if (!kgServiceReady.value) return;
  fetchFiles();
  fetchKnowledgeBases();
}

onMounted(loadData);
onActivated(loadData);
</script>

<template>
  <div class="space-y-6">
    <!-- <teleport to="#topbar-actions">
      <button @click="showUploadModal = true" class="btn btn-primary shadow-lg shadow-blue-500/20">
        <svg class="w-5 h-5 mr-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
        </svg>
        上传文件
      </button>
    </teleport> -->

    <!-- 筛选栏 -->
    <div class="card p-5 border-blue-500/10">
      <div class="flex flex-wrap gap-6">
        <div class="flex-1 min-w-[200px]">
          <label class="text-xs text-slate-500 mb-1.5 block font-medium uppercase tracking-wider">所属知识库</label>
          <select v-model="filterKnowledgeBase" @change="handleFilter" class="select w-full">
            <option value="">全部知识库</option>
            <option v-for="kb in knowledgeBases" :key="kb.id" :value="kb.id">
              {{ kb.name }}
            </option>
          </select>
        </div>
        <div class="flex-1 min-w-[200px]">
          <label class="text-xs text-slate-500 mb-1.5 block font-medium uppercase tracking-wider">处理状态</label>
          <select v-model="filterStatus" @change="handleFilter" class="select w-full">
            <option value="">全部状态</option>
            <option value="pending">等待中</option>
            <option value="processing">处理中</option>
            <option value="completed">已完成</option>
            <option value="failed">失败</option>
          </select>
        </div>
        <div class="flex-1 min-w-[200px]">
          <label class="text-xs text-slate-500 mb-1.5 block font-medium uppercase tracking-wider">文件类型</label>
          <select v-model="filterCategory" @change="handleFilter" class="select w-full">
            <option value="">全部类型</option>
            <option value="document">文档</option>
            <option value="image">图片</option>
            <option value="other">其他</option>
          </select>
        </div>
      </div>
    </div>

    <!-- 文件列表 -->
    <div class="card p-0 overflow-hidden">
      <div v-if="loading" class="p-12 flex justify-center">
        <LoadingSpinner size="lg" :text="t('common.loading')" />
      </div>
      <div v-else-if="files.length === 0" class="p-12">
        <EmptyState
          icon="document"
          title="暂无文件"
          description="点击上方按钮上传文件"
        >
          <button @click="showUploadModal = true" class="btn btn-primary mt-4">
            上传第一个文件
          </button>
        </EmptyState>
      </div>
      <div v-else class="table-container border-0 bg-transparent rounded-none">
        <table class="table">
          <thead>
            <tr>
              <th class="pl-6">文件名</th>
              <th>所属知识库</th>
              <th>类型</th>
              <th>大小</th>
              <th>状态</th>
              <th>上传时间</th>
              <th class="pr-6 text-right">操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="file in files" :key="file.id" class="group">
              <td class="pl-6">
                <div class="flex items-center gap-4">
                  <div class="w-10 h-10 rounded-lg bg-blue-500/10 flex items-center justify-center text-blue-400 group-hover:scale-110 transition-transform duration-300">
                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" :d="getFileIcon(file.file_ext)" />
                    </svg>
                  </div>
                  <div>
                    <div class="font-medium truncate max-w-xs text-slate-900 dark:text-slate-200 group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors">{{ file.original_name }}</div>
                    <div class="text-xs text-slate-500 font-mono mt-0.5">{{ file.id.slice(0, 8) }}...</div>
                  </div>
                </div>
              </td>
              <td>
                <span v-if="file.knowledge_base_id" class="text-sm px-2.5 py-1 rounded-full bg-slate-100 dark:bg-white/5 border border-slate-200 dark:border-white/5 text-slate-700 dark:text-slate-300">
                  {{ knowledgeBases.find(kb => kb.id === file.knowledge_base_id)?.name || file.knowledge_base_id.slice(0, 8) + '...' }}
                </span>
                <span v-else class="text-slate-500">-</span>
              </td>
              <td><span class="badge badge-gray font-mono">.{{ file.file_ext }}</span></td>
              <td class="text-slate-600 dark:text-slate-400 font-mono text-sm">{{ formatFileSize(file.file_size) }}</td>
              <td><span :class="['badge', getStatusClass(file.status)]">{{ getStatusText(file.status) }}</span></td>
              <td class="text-slate-600 dark:text-slate-400 text-sm">{{ formatDate(file.upload_time) }}</td>
              <td class="pr-6 text-right">
                <div class="flex items-center justify-end gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
                  <button
                    @click="showFileDetail(file)"
                    class="p-2 rounded-lg text-slate-500 hover:text-slate-900 dark:text-slate-400 dark:hover:text-white hover:bg-slate-100 dark:hover:bg-white/10 transition-colors"
                    title="查看详情"
                  >
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
                    </svg>
                  </button>
                  <button
                    @click="handleDelete(file.id)"
                    class="p-2 rounded-lg text-slate-500 hover:text-red-600 dark:text-slate-400 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-500/10 transition-colors"
                    title="删除"
                  >
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                    </svg>
                  </button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- 分页 -->
      <div v-if="files.length > 0" class="flex justify-between items-center p-4 border-t border-slate-200 dark:border-white/5 bg-slate-50 dark:bg-white/5">
        <div class="text-sm text-slate-600 dark:text-slate-400">
          共 <span class="text-slate-900 dark:text-white font-medium">{{ total }}</span> 个文件
        </div>
        <div class="flex items-center gap-2">
          <button
            @click="handlePageChange(page - 1)"
            :disabled="page === 1"
            class="btn btn-secondary btn-sm disabled:opacity-50 disabled:cursor-not-allowed"
          >
            上一页
          </button>
          <span class="px-4 py-1.5 text-sm bg-white dark:bg-[#0f172a] rounded-lg border border-slate-200 dark:border-white/10 text-slate-700 dark:text-slate-300 font-mono">
            {{ page }} / {{ totalPages }}
          </span>
          <button
            @click="handlePageChange(page + 1)"
            :disabled="page >= totalPages"
            class="btn btn-secondary btn-sm disabled:opacity-50 disabled:cursor-not-allowed"
          >
            下一页
          </button>
        </div>
      </div>
    </div>

    <!-- 上传模态框 -->
    <Modal v-model="showUploadModal" title="上传文件" size="md">
      <div class="space-y-6">
        <div>
          <label class="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-2">选择知识库</label>
          <select v-model="uploadKnowledgeBase" class="select w-full">
            <option value="">请选择知识库</option>
            <option v-for="kb in knowledgeBases" :key="kb.id" :value="kb.id">
              {{ kb.name }}
            </option>
          </select>
        </div>
        <div
          class="border-2 border-dashed border-slate-200 dark:border-white/10 rounded-xl p-8 text-center hover:border-blue-500/50 hover:bg-blue-50 dark:hover:bg-blue-500/5 transition-all cursor-pointer group"
          @click="($refs.fileInput as HTMLInputElement)?.click()"
          @dragover.prevent
          @drop.prevent="(e) => { uploadFile = e.dataTransfer?.files[0] || null }"
        >
          <input
            ref="fileInput"
            type="file"
            class="hidden"
            accept=".pdf,.doc,.docx,.txt,.md"
            @change="(e) => { uploadFile = (e.target as HTMLInputElement).files?.[0] || null }"
          />
          <div class="w-16 h-16 mx-auto bg-slate-100 dark:bg-slate-800 rounded-full flex items-center justify-center mb-4 group-hover:scale-110 transition-transform duration-300 border border-slate-200 dark:border-white/10">
            <svg class="w-8 h-8 text-slate-400 group-hover:text-blue-400 transition-colors" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
            </svg>
          </div>
          <p v-if="!uploadFile" class="text-slate-600 dark:text-slate-400 group-hover:text-slate-900 dark:group-hover:text-slate-200 transition-colors">点击或拖拽文件到这里</p>
          <p v-else class="text-blue-400 font-medium bg-blue-500/10 py-1 px-3 rounded-full inline-block">{{ uploadFile.name }}</p>
          <p class="text-xs text-slate-500 mt-2">支持 PDF、Word、TXT、Markdown 文件</p>
        </div>
        <div v-if="uploadLoading" class="space-y-2">
          <div class="progress">
            <div class="progress-bar" :style="{ width: uploadProgress + '%' }"></div>
          </div>
          <p class="text-sm text-center text-slate-600 dark:text-slate-400">上传进度: {{ uploadProgress }}%</p>
        </div>
      </div>
      <template #footer>
        <button @click="showUploadModal = false" class="btn btn-secondary">取消</button>
        <button @click="handleUpload" :disabled="uploadLoading || !uploadFile || !uploadKnowledgeBase" class="btn btn-primary">
          <LoadingSpinner v-if="uploadLoading" size="sm" />
          <span v-else>上传</span>
        </button>
      </template>
    </Modal>

    <!-- 详情模态框 -->
    <Modal v-model="showDetailModal" title="文件详情" size="md">
      <div v-if="selectedFile" class="space-y-6">
        <div class="flex items-center gap-5 p-5 bg-slate-50 dark:bg-[#0f172a] rounded-xl border border-slate-200 dark:border-white/5">
          <div class="w-16 h-16 rounded-xl bg-blue-500/10 flex items-center justify-center shrink-0">
            <svg class="w-8 h-8 text-blue-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" :d="getFileIcon(selectedFile.file_ext)" />
            </svg>
          </div>
          <div class="min-w-0">
            <div class="font-bold text-lg text-slate-900 dark:text-slate-200 truncate">{{ selectedFile.original_name }}</div>
            <div class="text-sm text-slate-500 font-mono mt-0.5">{{ formatFileSize(selectedFile.file_size) }}</div>
          </div>
        </div>
        <div class="space-y-4 bg-white dark:bg-[#0f172a]/30 p-5 rounded-xl border border-slate-200 dark:border-white/5">
          <div class="flex justify-between py-2 border-b border-slate-200 dark:border-white/5">
            <span class="text-slate-600 dark:text-slate-400">文件 ID</span>
            <span class="font-mono text-sm text-slate-900 dark:text-slate-200">{{ selectedFile.id }}</span>
          </div>
          <div class="flex justify-between py-2 border-b border-slate-200 dark:border-white/5">
            <span class="text-slate-600 dark:text-slate-400">文件类型</span>
            <span class="badge badge-gray font-mono">.{{ selectedFile.file_ext }}</span>
          </div>
          <div class="flex justify-between py-2 border-b border-slate-200 dark:border-white/5">
            <span class="text-slate-600 dark:text-slate-400">MIME 类型</span>
            <span class="text-slate-900 dark:text-slate-200 font-mono text-sm">{{ selectedFile.mime_type || '-' }}</span>
          </div>
          <div class="flex justify-between py-2 border-b border-slate-200 dark:border-white/5">
            <span class="text-slate-600 dark:text-slate-400">状态</span>
            <span :class="['badge', getStatusClass(selectedFile.status)]">{{ getStatusText(selectedFile.status) }}</span>
          </div>
          <div class="flex justify-between py-2 border-b border-slate-200 dark:border-white/5">
            <span class="text-slate-600 dark:text-slate-400">上传者</span>
            <span class="text-slate-900 dark:text-slate-200">{{ selectedFile.uploader || '-' }}</span>
          </div>
          <div class="flex justify-between py-2 border-b border-slate-200 dark:border-white/5">
            <span class="text-slate-600 dark:text-slate-400">上传时间</span>
            <span class="text-slate-900 dark:text-slate-200">{{ formatDate(selectedFile.upload_time) }}</span>
          </div>
        </div>
      </div>
      <template #footer>
        <button @click="showDetailModal = false" class="btn btn-secondary">关闭</button>
      </template>
    </Modal>
  </div>
</template>
