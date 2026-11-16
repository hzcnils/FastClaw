<script setup lang="ts">
import { ref, onMounted, onActivated, computed } from 'vue';
import { useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { governanceApi, kgServiceReady, ensureKgReady } from '@/api/knowledge';
import type { GovernanceDoc } from '@/types/knowledge';
import { useUIStore, useKnowledgeBaseStore } from '@/stores/index';
import LoadingSpinner from '@/components/common/LoadingSpinner.vue';
import EmptyState from '@/components/common/EmptyState.vue';
import { useDialog } from '@/composables/useDialog';

const { t } = useI18n();
const { confirm: confirmDialog } = useDialog();

const router = useRouter();
const uiStore = useUIStore();
const kbStore = useKnowledgeBaseStore();

const documents = ref<GovernanceDoc[]>([]);
const loading = ref(true);
const total = ref(0);
const page = ref(1);
const pageSize = ref(20);

// 筛选
const filterStatus = ref('');
const filterVerified = ref<string>('');
const filterKnowledgeBase = ref('');

async function fetchDocuments() {
  loading.value = true;
  try {
    const verified = filterVerified.value === '' ? undefined : filterVerified.value === 'true';
    const response = await governanceApi.list({
      status: filterStatus.value || undefined,
      verified,
      knowledge_base_id: filterKnowledgeBase.value || undefined,
      page: page.value,
      page_size: pageSize.value,
    });
    documents.value = response.items || [];
    total.value = response.total;
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '获取治理文档列表失败');
  } finally {
    loading.value = false;
  }
}

async function handleVerify(doc: GovernanceDoc) {
  if (!await confirmDialog('确定标记此文档为已验证吗？')) return;
  try {
    await governanceApi.verify(doc.id);
    uiStore.showToast('success', '验证成功');
    fetchDocuments();
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '验证失败');
  }
}

function viewDetail(doc: GovernanceDoc) {
  router.push(`/governance/${doc.id}`);
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

const totalPages = computed(() => Math.ceil(total.value / pageSize.value));

function handlePageChange(newPage: number) {
  if (newPage >= 1 && newPage <= totalPages.value) {
    page.value = newPage;
    fetchDocuments();
  }
}

function handleFilter() {
  page.value = 1;
  fetchDocuments();
}

async function loadData() {
  await ensureKgReady({ startIfNeeded: false });
  if (kgServiceReady.value) {
    kbStore.fetchKnowledgeBases();
    fetchDocuments();
  }
}

onMounted(loadData);
onActivated(loadData);
</script>

<template>
  <div class="space-y-6">
    <!-- 统计卡片 -->
    <div class="grid grid-cols-4 gap-4">
      <div class="stat-card">
        <div class="stat-card-icon bg-primary/20 text-primary">
          <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
          </svg>
        </div>
        <div>
          <div class="stat-card-value">{{ total }}</div>
          <div class="stat-card-label">总文档数</div>
        </div>
      </div>
      <div class="stat-card">
        <div class="stat-card-icon bg-success/20 text-success">
          <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        </div>
        <div>
          <div class="stat-card-value">{{ documents.filter(d => d.is_human_verified).length }}</div>
          <div class="stat-card-label">已验证</div>
        </div>
      </div>
      <div class="stat-card">
        <div class="stat-card-icon bg-warning/20 text-warning">
          <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        </div>
        <div>
          <div class="stat-card-value">{{ documents.filter(d => !d.is_human_verified).length }}</div>
          <div class="stat-card-label">待验证</div>
        </div>
      </div>
      <div class="stat-card">
        <div class="stat-card-icon bg-ocean/20 text-ocean">
          <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z" />
          </svg>
        </div>
        <div>
          <div class="stat-card-value">{{ documents.reduce((sum, d) => sum + d.word_count, 0).toLocaleString() }}</div>
          <div class="stat-card-label">总字数</div>
        </div>
      </div>
    </div>

    <!-- 筛选栏 -->
    <div class="card p-4">
      <div class="flex flex-wrap gap-4">
        <div class="flex-1 min-w-48">
          <select v-model="filterKnowledgeBase" @change="handleFilter" class="select w-full">
            <option value="">全部知识库</option>
            <option v-for="kb in kbStore.knowledgeBases" :key="kb.id" :value="kb.id">{{ kb.name }}</option>
          </select>
        </div>
        <div class="flex-1 min-w-48">
          <select v-model="filterStatus" @change="handleFilter" class="select w-full">
            <option value="">全部状态</option>
            <option value="pending">等待中</option>
            <option value="processing">处理中</option>
            <option value="completed">已完成</option>
            <option value="failed">失败</option>
          </select>
        </div>
        <div class="flex-1 min-w-48">
          <select v-model="filterVerified" @change="handleFilter" class="select w-full">
            <option value="">全部验证状态</option>
            <option value="true">已验证</option>
            <option value="false">未验证</option>
          </select>
        </div>
      </div>
    </div>

    <!-- 文档列表 -->
    <div class="card">
      <div v-if="loading" class="p-8">
        <LoadingSpinner size="lg" :text="t('common.loading')" />
      </div>
      <div v-else-if="documents.length === 0" class="p-8">
        <EmptyState
          icon="document"
          title="暂无治理文档"
          description="上传文件后会自动生成治理文档"
        />
      </div>
      <div v-else class="overflow-x-auto">
        <table class="table">
          <thead>
            <tr>
              <th>标题</th>
              <th>字数</th>
              <th>版本</th>
              <th>状态</th>
              <th>验证状态</th>
              <th>更新时间</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="doc in documents" :key="doc.id" class="hover:bg-dark-800/50 transition-colors cursor-pointer" @click="viewDetail(doc)">
              <td>
                <div class="flex items-center gap-3">
                  <div class="w-10 h-10 rounded-lg bg-dark-700 flex items-center justify-center">
                    <svg class="w-5 h-5 text-primary" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                    </svg>
                  </div>
                  <div>
                    <div class="font-medium">{{ doc.title || '未命名文档' }}</div>
                    <div class="text-xs text-dark-500 font-mono">{{ doc.id.slice(0, 8) }}...</div>
                  </div>
                </div>
              </td>
              <td>{{ doc.word_count.toLocaleString() }}</td>
              <td><span class="badge badge-secondary">v{{ doc.version }}</span></td>
              <td><span :class="['badge', getStatusClass(doc.status)]">{{ getStatusText(doc.status) }}</span></td>
              <td>
                <span v-if="doc.is_human_verified" class="badge badge-success">
                  <svg class="w-3 h-3 mr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                  </svg>
                  已验证
                </span>
                <span v-else class="badge badge-warning">待验证</span>
              </td>
              <td class="text-dark-400 text-sm">{{ formatDate(doc.updated_at) }}</td>
              <td>
                <div class="flex items-center gap-2" @click.stop>
                  <button
                    @click="viewDetail(doc)"
                    class="btn btn-ghost btn-sm"
                    title="查看详情"
                  >
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
                    </svg>
                  </button>
                  <button
                    v-if="!doc.is_human_verified"
                    @click="handleVerify(doc)"
                    class="btn btn-ghost btn-sm text-success hover:bg-success/10"
                    title="标记为已验证"
                  >
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                  </button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- 分页 -->
      <div v-if="documents.length > 0" class="flex justify-between items-center p-4 border-t border-dark-700">
        <div class="text-sm text-dark-400">
          共 {{ total }} 个文档
        </div>
        <div class="flex items-center gap-2">
          <button
            @click="handlePageChange(page - 1)"
            :disabled="page === 1"
            class="btn btn-ghost btn-sm"
          >
            上一页
          </button>
          <span class="px-3 text-sm">
            {{ page }} / {{ totalPages }}
          </span>
          <button
            @click="handlePageChange(page + 1)"
            :disabled="page >= totalPages"
            class="btn btn-ghost btn-sm"
          >
            下一页
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
