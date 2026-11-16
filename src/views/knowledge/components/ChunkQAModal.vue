<script setup lang="ts">
import { ref, watch, computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { ElMessageBox } from 'element-plus';
import { qaPairApi } from '@/api/knowledge';
import Modal from '@/components/common/Modal.vue';
import LoadingSpinner from '@/components/common/LoadingSpinner.vue';
import EmptyState from '@/components/common/EmptyState.vue';
import SearchClearButton from '@/components/common/SearchClearButton.vue';

const { t, locale } = useI18n();
const props = defineProps<{
  show: boolean;
  chunkId: string;
}>();

const emit = defineEmits(['update:show']);

// mock uiStore
const uiStore = {
  showToast: (type: string, msg: string) => {
    console.log(`[${type}] ${msg}`);
    if (type === 'error') alert(msg);
  }
};
const loading = ref(false);
const qaPairs = ref<any[]>([]);
const isAdding = ref(false);
const newQA = ref({ question: '', answer: '' });
const addingLoading = ref(false);

const editingId = ref<string | null>(null);
const editForm = ref({ question: '', answer: '' });

const searchQuery = ref('');

const dateLocale = computed(() => (locale.value === 'en' ? 'en-US' : 'zh-CN'));

const filteredQAPairs = computed(() => {
  if (!searchQuery.value) return qaPairs.value;
  const query = searchQuery.value.toLowerCase();
  return qaPairs.value.filter(qa => 
    qa.question.toLowerCase().includes(query) || 
    qa.answer.toLowerCase().includes(query)
  );
});

watch(() => props.show, (val) => {
  if (val && props.chunkId) {
    fetchQAPairs();
    isAdding.value = false;
    editingId.value = null;
  }
}, { immediate: true });

async function fetchQAPairs() {
  loading.value = true;
  try {
    const res = await qaPairApi.listByChunk(props.chunkId);
    qaPairs.value = res || [];
  } catch (e) {
    uiStore.showToast('error', t('knowledge.chunkQa.fetchFailed'));
  } finally {
    loading.value = false;
  }
}

async function handleAdd() {
  if (!newQA.value.question || !newQA.value.answer) {
    uiStore.showToast('warning', t('knowledge.chunkQa.fillRequired'));
    return;
  }
  addingLoading.value = true;
  try {
    await qaPairApi.create(props.chunkId, newQA.value);
    uiStore.showToast('success', t('knowledge.chunkQa.addOk'));
    newQA.value = { question: '', answer: '' };
    isAdding.value = false;
    fetchQAPairs();
  } catch (e) {
    uiStore.showToast('error', t('knowledge.chunkQa.addFailed'));
  } finally {
    addingLoading.value = false;
  }
}

async function handleDelete(qa: { id: string; question?: string }) {
  try {
    await ElMessageBox.confirm(
      t('knowledge.chunkQa.confirmDeleteMsg', { question: qa.question || '' }),
      t('knowledge.chunkQa.confirmDeleteTitle'),
      {
        type: 'warning',
        confirmButtonText: t('knowledge.confirmDelete'),
        cancelButtonText: t('common.cancel'),
        confirmButtonClass: 'el-button--danger',
        cancelButtonClass: 'delete-confirm-cancel-btn',
        customClass: 'delete-confirm-dialog',
        modalClass: 'fc-msgbox-above-modal',
        closeOnClickModal: false,
      },
    );
  } catch {
    return;
  }
  try {
    await qaPairApi.delete(qa.id);
    uiStore.showToast('success', t('knowledge.chunkQa.deleteOk'));
    fetchQAPairs();
  } catch (e) {
    uiStore.showToast('error', t('knowledge.chunkQa.deleteFailed'));
  }
}

function startEdit(qa: any) {
  editingId.value = qa.id;
  editForm.value = { question: qa.question, answer: qa.answer };
}

async function handleUpdate() {
  if (!editingId.value) return;
  try {
    await qaPairApi.update(editingId.value, editForm.value);
    uiStore.showToast('success', t('knowledge.chunkQa.updateOk'));
    editingId.value = null;
    fetchQAPairs();
  } catch (e) {
    uiStore.showToast('error', t('knowledge.chunkQa.updateFailed'));
  }
}

function formatCreatedAt(value: string | number | Date) {
  return new Date(value).toLocaleDateString(dateLocale.value);
}
</script>

<template>
  <Modal :show="show" :title="t('knowledge.chunkQa.modalTitle')" size="lg" :close-on-overlay="false" @close="emit('update:show', false)">
    <div class="chunk-qa-modal space-y-4 min-h-[400px]">
      <!-- 头部操作 -->
      <div class="flex justify-between items-center">
        <h3 class="chunk-qa-section-label text-sm font-medium">{{ t('knowledge.chunkQa.currentChunkLabel') }}</h3>
        <button v-if="!isAdding" @click="isAdding = true" class="btn btn-primary btn-sm">
          <svg class="w-4 h-4 mr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
          </svg>
          {{ t('knowledge.chunkQa.addBtn') }}
        </button>
      </div>

      <!-- 添加表单 -->
      <div v-if="isAdding" class="chunk-qa-form p-4 rounded-lg space-y-3">
        <div class="chunk-qa-form-header flex justify-between items-center mb-2">
          <span class="text-sm font-bold">{{ t('knowledge.chunkQa.newLabel') }}</span>
          <button @click="isAdding = false" class="chunk-qa-form-close">
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
        <div>
          <input v-model="newQA.question" class="input chunk-qa-input w-full mb-2" :placeholder="t('knowledge.chunkQa.questionPlaceholder')" />
          <textarea v-model="newQA.answer" class="input chunk-qa-input w-full h-24 resize-none" :placeholder="t('knowledge.chunkQa.answerPlaceholder')"></textarea>
        </div>
        <div class="flex justify-end">
          <button @click="handleAdd" :disabled="addingLoading" class="btn btn-primary btn-sm">
            <LoadingSpinner v-if="addingLoading" size="sm" />
            <span v-else>{{ t('common.save') }}</span>
          </button>
        </div>
      </div>

      <!-- 搜索框 -->
      <div v-if="!isAdding && qaPairs.length > 0" class="relative">
        <input
          v-model="searchQuery"
          type="text"
          :placeholder="t('knowledge.chunkQa.searchPlaceholder')"
          class="input chunk-qa-input w-full pl-10"
          :class="searchQuery ? 'pr-9' : 'pr-3'"
        />
        <svg class="w-4 h-4 chunk-qa-muted absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
        </svg>
        <SearchClearButton v-if="searchQuery" overlay @clear="searchQuery = ''" />
      </div>

      <!-- 列表 -->
      <div v-if="loading" class="py-10">
        <LoadingSpinner :text="t('common.loading')" />
      </div>
      <EmptyState
        v-else-if="qaPairs.length === 0 && !isAdding"
        icon="chat"
        :title="t('knowledge.chunkQa.emptyTitle')"
        :description="t('knowledge.chunkQa.emptyDesc')"
      />
      <div v-else class="space-y-3 max-h-[60vh] overflow-y-auto pr-1 custom-scrollbar">
        <div v-if="filteredQAPairs.length === 0" class="chunk-qa-empty text-center py-8">
          {{ t('knowledge.chunkQa.noMatch') }}
        </div>
        <div v-for="qa in filteredQAPairs" :key="qa.id" class="chunk-qa-card p-4 rounded-lg border transition-colors group">
          <div v-if="editingId === qa.id" class="space-y-3">
            <input v-model="editForm.question" class="input chunk-qa-input w-full" />
            <textarea v-model="editForm.answer" class="input chunk-qa-input w-full h-24 resize-none"></textarea>
            <div class="flex justify-end gap-2">
              <button @click="editingId = null" class="btn btn-secondary btn-sm">{{ t('common.cancel') }}</button>
              <button @click="handleUpdate" class="btn btn-primary btn-sm">{{ t('common.save') }}</button>
            </div>
          </div>
          <template v-else>
            <div class="flex justify-between items-start">
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2 mb-1">
                  <span class="chunk-qa-badge-q bg-blue-500/10 text-blue-500 dark:text-blue-400 text-xs px-1.5 py-0.5 rounded">Q</span>
                  <div class="chunk-qa-card-text font-medium">{{ qa.question }}</div>
                </div>
                <div class="flex items-start gap-2 mt-2">
                  <span class="chunk-qa-badge-a bg-emerald-500/10 text-emerald-500 dark:text-emerald-400 text-xs px-1.5 py-0.5 rounded mt-0.5">A</span>
                  <div class="chunk-qa-card-text text-sm whitespace-pre-wrap">{{ qa.answer }}</div>
                </div>
              </div>
              <div class="flex gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
                <button type="button" class="chunk-qa-icon-btn" @click="startEdit(qa)">
                  <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                  </svg>
                </button>
                <button type="button" class="chunk-qa-icon-btn" @click="handleDelete(qa)">
                  <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                  </svg>
                </button>
              </div>
            </div>
            <div class="chunk-qa-card-meta mt-2 pt-2 border-t flex gap-4 text-xs">
              <span>{{ t('knowledge.chunkQa.qualityScore') }}: {{ qa.quality_score }}</span>
              <span>{{ t('knowledge.chunkQa.createdAt') }}: {{ formatCreatedAt(qa.created_at) }}</span>
            </div>
          </template>
        </div>
      </div>
    </div>
  </Modal>
</template>

<style scoped>
.chunk-qa-modal :deep(.btn.btn-primary.btn-sm),
.chunk-qa-modal .btn.btn-primary.btn-sm,
.chunk-qa-modal :deep(.btn.btn-secondary.btn-sm),
.chunk-qa-modal .btn.btn-secondary.btn-sm {
  border-radius: 4px !important;
}

.chunk-qa-section-label,
.chunk-qa-empty,
.chunk-qa-muted {
  color: var(--text-secondary) !important;
}

.chunk-qa-form {
  background: transparent !important;
  border: 1px solid var(--border-color) !important;
}

.chunk-qa-form-header,
.chunk-qa-form-header span {
  color: var(--text-primary) !important;
}

.chunk-qa-form-close {
  color: var(--text-secondary);
  background: transparent;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  padding: 4px;
  transition: background 0.15s ease, color 0.15s ease;
}
.chunk-qa-form-close:hover {
  color: var(--text-primary);
  background: var(--dropdown-hover-color);
}

.chunk-qa-input.input,
.chunk-qa-modal :deep(.chunk-qa-input.input) {
  background: transparent !important;
  border: 1px solid var(--border-color) !important;
  color: var(--text-primary) !important;
  box-shadow: none !important;
}
.chunk-qa-input.input::placeholder,
.chunk-qa-modal :deep(.chunk-qa-input.input)::placeholder {
  color: var(--text-three) !important;
  opacity: 1;
}

.chunk-qa-card {
  background: var(--card-background-tmd) !important;
  border-color: var(--border-color) !important;
}
.chunk-qa-card:hover {
  border-color: color-mix(in srgb, var(--color-primary, #3b82f6) 40%, var(--border-color)) !important;
}

.chunk-qa-card-text,
.chunk-qa-card-meta,
.chunk-qa-card-meta span {
  color: var(--text-secondary) !important;
}

.chunk-qa-card-meta {
  border-top-color: var(--border-color) !important;
}

.chunk-qa-icon-btn {
  padding: 6px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--text-three) !important;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}
.chunk-qa-icon-btn:hover {
  background: var(--dropdown-hover-color) !important;
  color: var(--text-three) !important;
}
.chunk-qa-icon-btn svg {
  color: inherit;
}

.custom-scrollbar::-webkit-scrollbar {
  width: 4px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.1);
  border-radius: 4px;
}
.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.2);
}

/* 取消按钮与主题选择「恢复/取消」一致，不改尺寸 */
.btn.btn-secondary {
  color: var(--btn-secondary-text) !important;
  background: var(--cancel-btn-color) !important;
  border: 1px solid var(--btn-secondary-border) !important;
  border-radius: 4px !important;
  box-shadow: none !important;
}
.btn.btn-secondary:hover:not(:disabled) {
  background: var(--cancel-btn-hover-color) !important;
  color: var(--btn-secondary-hover-text) !important;
  border-color: var(--btn-secondary-hover-border) !important;
  transform: none;
  box-shadow: none !important;
}
</style>

<style>
/* 情景主题（除雾灰调）：Q/A 标签背景为白色 45% 透明 */
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .chunk-qa-badge-q,
html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .chunk-qa-badge-a,
html[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .chunk-qa-badge-q,
html[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .chunk-qa-badge-a {
  background: rgba(255, 255, 255, 0.45) !important;
}
</style>
