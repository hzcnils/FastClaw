<template>
  <div class="space-y-6">
    <!-- <teleport to="#topbar-actions">
      <button
        @click="loadTasks"
        :disabled="loading"
        class="inline-flex items-center gap-2 px-4 py-2 bg-white border border-gray-300 rounded-lg text-sm font-medium text-gray-700 hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 transition-all disabled:opacity-50"
      >
        <svg :class="{ 'animate-spin': loading }" class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
        刷新
      </button>
    </teleport> -->

    <!-- 统计卡片 -->
    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-4">
      <div class="stat-card">
        <div class="stat-icon bg-blue-100">
          <svg class="w-5 h-5 text-blue-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
          </svg>
        </div>
        <div class="stat-content">
          <div class="stat-value">{{ stats.total }}</div>
          <div class="stat-label">总任务数</div>
        </div>
      </div>
      <div class="stat-card">
        <div class="stat-icon bg-yellow-100">
          <svg class="w-5 h-5 text-yellow-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        </div>
        <div class="stat-content">
          <div class="stat-value">{{ stats.pending }}</div>
          <div class="stat-label">等待中</div>
        </div>
      </div>
      <div class="stat-card">
        <div class="stat-icon bg-blue-100">
          <svg class="w-5 h-5 text-blue-600 animate-spin" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
          </svg>
        </div>
        <div class="stat-content">
          <div class="stat-value">{{ stats.running }}</div>
          <div class="stat-label">执行中</div>
        </div>
      </div>
      <div class="stat-card">
        <div class="stat-icon bg-green-100">
          <svg class="w-5 h-5 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
          </svg>
        </div>
        <div class="stat-content">
          <div class="stat-value">{{ stats.completed }}</div>
          <div class="stat-label">已完成</div>
        </div>
      </div>
      <div class="stat-card">
        <div class="stat-icon bg-red-100">
          <svg class="w-5 h-5 text-red-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
        </div>
        <div class="stat-content">
          <div class="stat-value">{{ stats.failed }}</div>
          <div class="stat-label">失败</div>
        </div>
      </div>
    </div>

    <!-- 筛选栏 -->
    <div class="flex flex-wrap items-center gap-4 p-4 bg-white rounded-xl border border-gray-200">
      <div class="flex items-center gap-2">
        <label class="text-sm font-medium text-gray-700">状态</label>
        <select
          v-model="filters.status"
          class="rounded-lg border-gray-300 text-sm focus:ring-blue-500 focus:border-blue-500"
        >
          <option value="">全部</option>
          <option value="pending">等待中</option>
          <option value="running">执行中</option>
          <option value="completed">已完成</option>
          <option value="failed">失败</option>
          <option value="cancelled">已取消</option>
        </select>
      </div>
      <div class="flex items-center gap-2">
        <label class="text-sm font-medium text-gray-700">任务类型</label>
        <select
          v-model="filters.task_type"
          class="rounded-lg border-gray-300 text-sm focus:ring-blue-500 focus:border-blue-500"
        >
          <option value="">全部</option>
          <option value="file_process">文件处理</option>
          <option value="embedding">向量化</option>
          <option value="indexing">建立索引</option>
          <option value="rebuild">重建知识库</option>
        </select>
      </div>
      <div class="flex items-center gap-2">
        <label class="text-sm font-medium text-gray-700">知识库</label>
        <select
          v-model="filters.knowledge_base_id"
          class="rounded-lg border-gray-300 text-sm focus:ring-blue-500 focus:border-blue-500"
        >
          <option value="">全部知识库</option>
          <option v-for="kb in knowledgeBases" :key="kb.id" :value="kb.id">{{ kb.name }}</option>
        </select>
      </div>
    </div>

    <!-- 任务列表 -->
    <div class="bg-white rounded-xl border border-gray-200 overflow-hidden">
      <LoadingSpinner v-if="loading" text="加载中..." />
      <EmptyState
        v-else-if="tasks.length === 0"
        icon="tasks"
        title="暂无任务"
        description="当前没有任何任务记录"
      />
      <div v-else class="overflow-x-auto">
        <table class="min-w-full divide-y divide-gray-200">
          <thead class="bg-gray-50">
            <tr>
              <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">任务</th>
              <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">类型</th>
              <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">状态</th>
              <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">进度</th>
              <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">创建时间</th>
              <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">耗时</th>
              <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">操作</th>
            </tr>
          </thead>
          <tbody class="bg-white divide-y divide-gray-200">
            <tr
              v-for="task in tasks"
              :key="task.id"
              class="hover:bg-gray-50 cursor-pointer transition-colors"
              @click="openTaskDetail(task)"
            >
              <td class="px-6 py-4">
                <div class="flex items-center gap-3">
                  <div :class="['w-10 h-10 rounded-lg flex items-center justify-center', getTaskIconBg(task.task_type)]">
                    <svg class="w-5 h-5 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" :d="getTaskIconPath(task.task_type)" />
                    </svg>
                  </div>
                  <div>
                    <div class="font-medium text-gray-900">{{ task.task_name || task.id.slice(0, 8) }}</div>
                    <div class="text-xs text-gray-500">ID: {{ task.id.slice(0, 12) }}...</div>
                  </div>
                </div>
              </td>
              <td class="px-6 py-4">
                <span :class="['inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium', getTaskTypeBadge(task.task_type)]">
                  {{ getTaskTypeLabel(task.task_type) }}
                </span>
              </td>
              <td class="px-6 py-4">
                <span :class="['inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-xs font-medium', getStatusBadge(task.status)]">
                  <span v-if="task.status === 'running'" class="w-1.5 h-1.5 bg-current rounded-full animate-pulse"></span>
                  {{ getStatusLabel(task.status) }}
                </span>
              </td>
              <td class="px-6 py-4">
                <div class="w-32">
                  <div class="flex items-center justify-between text-xs mb-1">
                    <span class="text-gray-600">{{ task.progress }}%</span>
                  </div>
                  <div class="h-2 bg-gray-200 rounded-full overflow-hidden">
                    <div
                      :class="['h-full transition-all duration-300', getProgressBarColor(task.status)]"
                      :style="{ width: `${task.progress}%` }"
                    ></div>
                  </div>
                  <div v-if="task.progress_message" class="text-xs text-gray-500 mt-1 truncate">
                    {{ task.progress_message }}
                  </div>
                </div>
              </td>
              <td class="px-6 py-4 text-sm text-gray-500">
                {{ formatDateTime(task.created_at) }}
              </td>
              <td class="px-6 py-4 text-sm text-gray-500">
                {{ formatDuration(task.duration_ms) }}
              </td>
              <td class="px-6 py-4" @click.stop>
                <div class="flex items-center gap-2">
                  <button
                    v-if="task.status === 'running'"
                    @click="cancelTask(task)"
                    :disabled="cancelling === task.id"
                    class="inline-flex items-center gap-1 px-2.5 py-1.5 text-xs font-medium text-red-700 bg-red-100 rounded-lg hover:bg-red-200 transition-colors disabled:opacity-50"
                  >
                    <svg v-if="cancelling === task.id" class="w-3 h-3 animate-spin" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                    </svg>
                    取消
                  </button>
                  <button
                    v-if="task.status === 'failed'"
                    @click="retryTask(task)"
                    :disabled="retrying === task.id"
                    class="inline-flex items-center gap-1 px-2.5 py-1.5 text-xs font-medium text-orange-700 bg-orange-100 rounded-lg hover:bg-orange-200 transition-colors disabled:opacity-50"
                  >
                    <svg v-if="retrying === task.id" class="w-3 h-3 animate-spin" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                    </svg>
                    重试
                  </button>
                  <button
                    @click="openTaskDetail(task)"
                    class="inline-flex items-center gap-1 px-2.5 py-1.5 text-xs font-medium text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200 transition-colors"
                  >
                    详情
                  </button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- 分页 -->
      <div v-if="total > pageSize" class="px-6 py-4 border-t border-gray-200 flex items-center justify-between">
        <div class="text-sm text-gray-500">
          共 {{ total }} 条记录
        </div>
        <div class="flex items-center gap-2">
          <button
            @click="page > 1 && (page--, loadTasks())"
            :disabled="page <= 1"
            class="px-3 py-1 rounded-lg border border-gray-300 text-sm disabled:opacity-50 disabled:cursor-not-allowed hover:bg-gray-50"
          >
            上一页
          </button>
          <span class="text-sm text-gray-600">{{ page }} / {{ Math.ceil(total / pageSize) }}</span>
          <button
            @click="page < Math.ceil(total / pageSize) && (page++, loadTasks())"
            :disabled="page >= Math.ceil(total / pageSize)"
            class="px-3 py-1 rounded-lg border border-gray-300 text-sm disabled:opacity-50 disabled:cursor-not-allowed hover:bg-gray-50"
          >
            下一页
          </button>
        </div>
      </div>
    </div>

    <!-- 任务详情模态框 -->
    <Modal v-model:show="showDetailModal" title="任务详情" size="lg">
      <div v-if="selectedTask" class="space-y-6">
        <!-- 基本信息 -->
        <div class="grid grid-cols-2 gap-4">
          <div>
            <label class="text-xs font-medium text-gray-500 uppercase">任务ID</label>
            <p class="mt-1 text-sm text-gray-900 font-mono">{{ selectedTask.id }}</p>
          </div>
          <div>
            <label class="text-xs font-medium text-gray-500 uppercase">任务名称</label>
            <p class="mt-1 text-sm text-gray-900">{{ selectedTask.task_name || '-' }}</p>
          </div>
          <div>
            <label class="text-xs font-medium text-gray-500 uppercase">任务类型</label>
            <p class="mt-1">
              <span :class="['inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium', getTaskTypeBadge(selectedTask.task_type)]">
                {{ getTaskTypeLabel(selectedTask.task_type) }}
              </span>
            </p>
          </div>
          <div>
            <label class="text-xs font-medium text-gray-500 uppercase">状态</label>
            <p class="mt-1">
              <span :class="['inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-xs font-medium', getStatusBadge(selectedTask.status)]">
                <span v-if="selectedTask.status === 'running'" class="w-1.5 h-1.5 bg-current rounded-full animate-pulse"></span>
                {{ getStatusLabel(selectedTask.status) }}
              </span>
            </p>
          </div>
        </div>

        <!-- 进度 -->
        <div>
          <label class="text-xs font-medium text-gray-500 uppercase">执行进度</label>
          <div class="mt-2">
            <div class="flex items-center justify-between text-sm mb-1">
              <span class="text-gray-600">{{ selectedTask.progress_message || '处理中...' }}</span>
              <span class="font-medium">{{ selectedTask.progress }}%</span>
            </div>
            <div class="h-3 bg-gray-200 rounded-full overflow-hidden">
              <div
                :class="['h-full transition-all duration-300', getProgressBarColor(selectedTask.status)]"
                :style="{ width: `${selectedTask.progress}%` }"
              ></div>
            </div>
          </div>
        </div>

        <!-- 时间信息 -->
        <div class="grid grid-cols-2 gap-4">
          <div>
            <label class="text-xs font-medium text-gray-500 uppercase">创建时间</label>
            <p class="mt-1 text-sm text-gray-900">{{ formatDateTime(selectedTask.created_at) }}</p>
          </div>
          <div>
            <label class="text-xs font-medium text-gray-500 uppercase">开始时间</label>
            <p class="mt-1 text-sm text-gray-900">{{ selectedTask.started_at ? formatDateTime(selectedTask.started_at) : '-' }}</p>
          </div>
          <div>
            <label class="text-xs font-medium text-gray-500 uppercase">完成时间</label>
            <p class="mt-1 text-sm text-gray-900">{{ selectedTask.completed_at ? formatDateTime(selectedTask.completed_at) : '-' }}</p>
          </div>
          <div>
            <label class="text-xs font-medium text-gray-500 uppercase">执行耗时</label>
            <p class="mt-1 text-sm text-gray-900">{{ formatDuration(selectedTask.duration_ms) }}</p>
          </div>
        </div>

        <!-- 重试信息 -->
        <div>
          <label class="text-xs font-medium text-gray-500 uppercase">重试次数</label>
          <p class="mt-1 text-sm text-gray-900">{{ selectedTask.retry_count }} 次</p>
        </div>

        <!-- 错误信息 -->
        <div v-if="selectedTask.error_message || selectedTask.error" class="p-4 bg-red-50 rounded-lg border border-red-200">
          <div class="flex items-start gap-3">
            <svg class="w-5 h-5 text-red-500 flex-shrink-0 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
            <div>
              <h4 class="text-sm font-medium text-red-800">错误信息</h4>
              <p class="mt-1 text-sm text-red-700 whitespace-pre-wrap">{{ selectedTask.error_message || selectedTask.error }}</p>
            </div>
          </div>
        </div>

        <!-- 操作按钮 -->
        <div v-if="selectedTask.status === 'failed'" class="flex justify-end">
          <button
            @click="retryTask(selectedTask); showDetailModal = false"
            :disabled="retrying === selectedTask.id"
            class="inline-flex items-center gap-2 px-4 py-2 bg-orange-600 text-white rounded-lg hover:bg-orange-700 transition-colors disabled:opacity-50"
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
            重试任务
          </button>
        </div>
      </div>
    </Modal>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, watch, onMounted, onActivated, onUnmounted } from 'vue';
import { taskApi, knowledgeBaseApi, kgServiceReady, ensureKgReady } from '@/api/knowledge';
import type { Task, KnowledgeBase } from '@/types/knowledge';
import { useUIStore } from '@/stores/index';
import LoadingSpinner from '@/components/common/LoadingSpinner.vue';
import EmptyState from '@/components/common/EmptyState.vue';
import Modal from '@/components/common/Modal.vue';

const uiStore = useUIStore();

// 数据
const tasks = ref<Task[]>([]);
const loading = ref(false);
const page = ref(1);
const pageSize = ref(20);
const total = ref(0);
const retrying = ref<string | null>(null);
const cancelling = ref<string | null>(null);
const knowledgeBases = ref<KnowledgeBase[]>([]);

// 筛选
const filters = reactive({
  status: '',
  task_type: '',
  knowledge_base_id: '',
});

// 统计
const stats = reactive({
  total: 0,
  pending: 0,
  running: 0,
  completed: 0,
  failed: 0,
});

// 详情弹窗
const showDetailModal = ref(false);
const selectedTask = ref<Task | null>(null);

// 自动刷新
let refreshInterval: number | null = null;

// 加载任务列表
const loadTasks = async () => {
  loading.value = true;
  try {
    const result = await taskApi.list({
      status: filters.status || undefined,
      task_type: filters.task_type || undefined,
      knowledge_base_id: filters.knowledge_base_id || undefined,
      page: page.value,
      page_size: pageSize.value,
    });
    tasks.value = result.items || [];
    total.value = result.total || 0;
    
    // 更新统计
    updateStats();
  } catch (err) {
    uiStore.showToast('error', '加载任务列表失败');
    console.error(err);
  } finally {
    loading.value = false;
  }
};

// 更新统计信息
const updateStats = () => {
  stats.total = total.value;
  stats.pending = tasks.value.filter(t => t.status === 'pending').length;
  stats.running = tasks.value.filter(t => t.status === 'running').length;
  stats.completed = tasks.value.filter(t => t.status === 'completed').length;
  stats.failed = tasks.value.filter(t => t.status === 'failed').length;
};

// 重试任务
const retryTask = async (task: Task) => {
  retrying.value = task.id;
  try {
    await taskApi.retry(task.id);
    uiStore.showToast('success', '任务重试已启动');
    loadTasks();
  } catch (err) {
    uiStore.showToast('error', '重试失败');
    console.error(err);
  } finally {
    retrying.value = null;
  }
};

// 取消任务
const cancelTask = async (task: Task) => {
  cancelling.value = task.id;
  try {
    await taskApi.cancel(task.id);
    uiStore.showToast('success', '任务已取消');
    loadTasks();
  } catch (err) {
    uiStore.showToast('error', '取消失败');
    console.error(err);
  } finally {
    cancelling.value = null;
  }
};

// 打开任务详情
const openTaskDetail = (task: Task) => {
  selectedTask.value = task;
  showDetailModal.value = true;
};

// 格式化日期时间
const formatDateTime = (dateStr: string) => {
  if (!dateStr) return '-';
  const date = new Date(dateStr);
  return date.toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
};

// 格式化耗时
const formatDuration = (ms?: number) => {
  if (!ms) return '-';
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  if (ms < 3600000) return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`;
  return `${Math.floor(ms / 3600000)}h ${Math.floor((ms % 3600000) / 60000)}m`;
};

// 获取状态标签样式
const getStatusBadge = (status: string) => {
  const styles: Record<string, string> = {
    pending: 'bg-yellow-100 text-yellow-800',
    running: 'bg-blue-100 text-blue-800',
    completed: 'bg-green-100 text-green-800',
    failed: 'bg-red-100 text-red-800',
    cancelled: 'bg-gray-100 text-gray-800',
  };
  return styles[status] || 'bg-gray-100 text-gray-800';
};

// 获取状态文本
const getStatusLabel = (status: string) => {
  const labels: Record<string, string> = {
    pending: '等待中',
    running: '执行中',
    completed: '已完成',
    failed: '失败',
    cancelled: '已取消',
  };
  return labels[status] || status;
};

// 获取任务类型样式
const getTaskTypeBadge = (type: string) => {
  const styles: Record<string, string> = {
    file_process: 'bg-purple-100 text-purple-800',
    embedding: 'bg-blue-100 text-blue-800',
    indexing: 'bg-teal-100 text-teal-800',
    rebuild: 'bg-orange-100 text-orange-800',
  };
  return styles[type] || 'bg-gray-100 text-gray-800';
};

// 获取任务类型文本
const getTaskTypeLabel = (type: string) => {
  const labels: Record<string, string> = {
    file_process: '文件处理',
    embedding: '向量化',
    indexing: '建立索引',
    rebuild: '重建知识库',
  };
  return labels[type] || type;
};

// 获取任务图标背景
const getTaskIconBg = (type: string) => {
  const bgs: Record<string, string> = {
    file_process: 'bg-purple-500',
    embedding: 'bg-blue-500',
    indexing: 'bg-teal-500',
    rebuild: 'bg-orange-500',
  };
  return bgs[type] || 'bg-gray-500';
};

// 获取任务图标路径
const getTaskIconPath = (type: string) => {
  const paths: Record<string, string> = {
    file_process: 'M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z',
    embedding: 'M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4',
    indexing: 'M4 6h16M4 10h16M4 14h16M4 18h16',
    rebuild: 'M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15',
  };
  return paths[type] || 'M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2';
};

// 获取进度条颜色
const getProgressBarColor = (status: string) => {
  const colors: Record<string, string> = {
    pending: 'bg-yellow-400',
    running: 'bg-blue-500',
    completed: 'bg-green-500',
    failed: 'bg-red-500',
    cancelled: 'bg-gray-400',
  };
  return colors[status] || 'bg-gray-400';
};

// 监听筛选条件变化
watch(filters, () => {
  page.value = 1;
  loadTasks();
});

// 启动自动刷新（当有运行中的任务时）
const startAutoRefresh = () => {
  if (refreshInterval) return;
  refreshInterval = window.setInterval(() => {
    if (tasks.value.some(t => t.status === 'running' || t.status === 'pending')) {
      loadTasks();
    }
  }, 5000);
};

function initPage() {
  ensureKgReady({ startIfNeeded: false }).then(ready => {
    if (!ready) return;
    loadTasks();
    startAutoRefresh();
    knowledgeBaseApi.list().then(kbs => { knowledgeBases.value = kbs; }).catch(() => {});
  });
}

onMounted(initPage);
onActivated(initPage);

onUnmounted(() => {
  if (refreshInterval) {
    clearInterval(refreshInterval);
  }
});
</script>

<style scoped>
@reference "tailwindcss";

.stat-card {
  @apply flex items-center gap-4 p-4 bg-white rounded-xl border border-gray-200;
}

.stat-icon {
  @apply w-12 h-12 rounded-lg flex items-center justify-center flex-shrink-0;
}

.stat-content {
  @apply flex-1 min-w-0;
}

.stat-value {
  @apply text-2xl font-bold text-gray-900;
}

.stat-label {
  @apply text-sm text-gray-500;
}
</style>
