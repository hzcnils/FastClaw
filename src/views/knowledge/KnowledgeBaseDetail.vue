<script setup lang="ts">
import { ref, watch, onMounted, onActivated, onDeactivated, onBeforeUnmount, computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { readFile } from '@tauri-apps/plugin-fs';
import { knowledgeBaseApi, fileApi, chunkApi, chunkToolApi, kbGrantApi, kgServiceReady, ensureKgReady } from '@/api/knowledge';
import type { KnowledgeBase, FileInfo, ChunkInfo, KbGrant, CreateKbGrantRequest, AlgorithmInfo } from '@/types/knowledge';
import { useUIStore } from '@/stores';
import LoadingSpinner from '@/components/common/LoadingSpinner.vue';
import EmptyState from '@/components/common/EmptyState.vue';
import Modal from '@/components/common/Modal.vue';
import ChunkQAModal from './components/ChunkQAModal.vue';
import ChunkViewer from './components/ChunkViewer.vue';
import { useThemeStore } from '@/stores/theme';
import { useDialog } from '@/composables/useDialog';
import { ElMessageBox } from 'element-plus';
const { t } = useI18n();
const { confirm: confirmDialog } = useDialog();

const route = useRoute();
const router = useRouter();
const uiStore = useUIStore();

const knowledgeBase = ref<KnowledgeBase | null>(null);
const files = ref<FileInfo[]>([]);
const chunks = ref<ChunkInfo[]>([]);
const loading = ref(true);
const filesLoading = ref(false);
const chunksLoading = ref(false);

const activeTab = ref<'overview' | 'files' | 'chunks' | 'settings'>('overview');

/** 同页内嵌分块查看（不新开 Tab） */
const chunkViewerTarget = ref<{ fileId: string; fileName: string } | null>(null);

function openChunkViewer(file: FileInfo) {
  chunkViewerTarget.value = {
    fileId: file.id,
    fileName: file.original_name,
  };
  activeTab.value = 'files';
}

function closeChunkViewer() {
  chunkViewerTarget.value = null;
}

// 编辑模态框
const showEditModal = ref(false);
const editForm = ref({
  name: '',
  description: '',
  is_public: false,
});
const editLoading = ref(false);

// 文件上传
const showUploadModal = ref(false);
const uploadFile = ref<File | null>(null);
const uploadFileInput = ref<HTMLInputElement | null>(null);
const uploadDragOver = ref(false);
const uploadProgress = ref(0);
const uploadLoading = ref(false);
/** Tauri 接管系统文件拖入；HTML5 drop 在桌面端通常拿不到本地路径。 */
let tauriUploadDropUnlisten: (() => void) | undefined;

function openUploadFilePicker() {
  uploadFileInput.value?.click();
}

function onUploadFileInputChange(e: Event) {
  const input = e.target as HTMLInputElement;
  uploadFile.value = input.files?.[0] || null;
}

function resetUploadPicker() {
  uploadDragOver.value = false;
  uploadFile.value = null;
  uploadProgress.value = 0;
  if (uploadFileInput.value) uploadFileInput.value.value = '';
}

function fileNameFromPath(path: string) {
  return path.replace(/\\/g, '/').split('/').pop() || 'file';
}

async function fileFromLocalPath(path: string): Promise<File> {
  const bytes = await readFile(path);
  return new File([bytes], fileNameFromPath(path));
}

async function applyDroppedLocalPaths(paths: string[]) {
  const path = (paths || []).find((item) => !!item?.trim());
  if (!path) return;
  try {
    uploadFile.value = await fileFromLocalPath(path);
    if (uploadFileInput.value) uploadFileInput.value.value = '';
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '读取拖入文件失败');
  }
}

function onUploadDragEnter(e: DragEvent) {
  e.preventDefault();
  if (e.dataTransfer?.types?.includes('Files')) uploadDragOver.value = true;
}

function onUploadDragOver(e: DragEvent) {
  e.preventDefault();
  if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy';
  if (e.dataTransfer?.types?.includes('Files')) uploadDragOver.value = true;
}

function onUploadDragLeave(e: DragEvent) {
  e.preventDefault();
  const root = e.currentTarget as HTMLElement | null;
  const related = e.relatedTarget as Node | null;
  if (root && related && root.contains(related)) return;
  uploadDragOver.value = false;
}

function onUploadDrop(e: DragEvent) {
  e.preventDefault();
  uploadDragOver.value = false;
  const file = e.dataTransfer?.files?.[0] || null;
  if (!file) return;
  uploadFile.value = file;
  if (uploadFileInput.value) uploadFileInput.value.value = '';
}

function unbindTauriUploadDrop() {
  tauriUploadDropUnlisten?.();
  tauriUploadDropUnlisten = undefined;
}

async function bindTauriUploadDrop() {
  unbindTauriUploadDrop();
  if (!isTauri()) return;
  try {
    tauriUploadDropUnlisten = await getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (!showUploadModal.value) return;
      if (payload.type === 'enter' || payload.type === 'over') {
        uploadDragOver.value = true;
        return;
      }
      uploadDragOver.value = false;
      if (payload.type === 'drop') {
        void applyDroppedLocalPaths(payload.paths || []);
      }
    });
  } catch (error) {
    console.warn('[KnowledgeBaseDetail] tauri upload drop unavailable', error);
  }
}

watch(showUploadModal, (open) => {
  if (open) {
    void bindTauriUploadDrop();
    return;
  }
  unbindTauriUploadDrop();
  resetUploadPicker();
});

// 分页
const filesTableHeadRef = ref<HTMLElement | null>(null);
const filesTableBodyRef = ref<HTMLElement | null>(null);

function onFilesTableBodyScroll() {
  const head = filesTableHeadRef.value;
  const body = filesTableBodyRef.value;
  if (head && body) head.scrollLeft = body.scrollLeft;
}

const filePage = ref(1);
const filePageSize = ref<number>(Number(localStorage.getItem('kb_file_page_size')) || 20);
const fileTotalCount = ref(0);
const chunkPage = ref(1);
const chunkPageSize = ref<number>(Number(localStorage.getItem('kb_chunk_page_size')) || 20);
const chunkTotalCount = ref(0);
const PAGE_SIZE_OPTIONS = [20, 50, 100, 200, 500, 1000];

// v1.1 §3.2: 切片删除 + 清理无意义切片
const chunkSelectedIds = ref<Set<string>>(new Set());
const chunkBatchDeleting = ref(false);
const showCleanupModal = ref(false);
const cleanupLoading = ref(false);
const cleanupResult = ref<{
  scanned: number;
  matched: number;
  samples: Array<{ id: string; preview: string }>;
} | null>(null);
const cleanupForm = ref({
  min_len: 20,
  scan_limit: 10000,
  mode: 'soft' as 'soft' | 'hard',
});

// 删除知识库确认弹窗
const showDeleteKbModal = ref(false);
const isDeletingKb = ref(false);

async function handleConfirmDeleteKb() {
  if (!knowledgeBase.value) return;
  isDeletingKb.value = true;
  try {
    await knowledgeBaseApi.delete(knowledgeBase.value.id);
    uiStore.showToast('success', '知识库已删除');
    showDeleteKbModal.value = false;
    // 先导航到知识管理界面，再移除当前详情页 tab
    const currentPath = route.path
    await router.push('/knowledge');
    window.dispatchEvent(new CustomEvent('remove-tab', { detail: currentPath }))
  } catch (e: any) {
    uiStore.showToast('error', e.message || '删除失败');
  } finally {
    isDeletingKb.value = false;
  }
}

const themeStore = useThemeStore()

const bgStyle = computed(() => {
  const id = themeStore.currentPreset.id
  if (id == 'simple-dark' || id.startsWith('dark-')) {
    return { 'background-color': 'rgb(255 255 255 / 15%)' }
  } else if (id == 'scene-graphite-cyan') {
    return { 'background-color': '#dfdfe0' }
  } else if (id.startsWith('scene-')) {
    return { 'background-color': 'rgba(255,255,255,0.15)' }
  }
  return {}
})
// 弹窗背景使用主题色 --theme-dailog-color
const modalStyle = computed(() => {
  const id = themeStore.currentPreset.id
  const r: Record<string, string> = {}
  r['background-color'] = 'var(--theme-dailog-color)'
  if (id === 'simple-dark' || id.startsWith('dark-') || (id.startsWith('scene-') && id !== 'scene-graphite-cyan')) {
    r['border'] = '1px solid rgba(255,255,255,0.15)'
    r['color'] = '#fff'
  } else if (id === 'scene-graphite-cyan') {
    r['border'] = '1px solid rgba(0,0,0,0.10)'
    r['color'] = '#1f2937'
  } else if (id === 'simple-light') {
    r['border'] = '1px solid #e5e7eb'
    r['color'] = '#1f2937'
  }  
  return r
})

const algorithms = ref<AlgorithmInfo[]>([])
async function loadAlgorithms() {
  try {
    const response = await chunkToolApi.listAlgorithms()
    algorithms.value = response || []
  } catch (e) {
    console.error('加载切片算法列表失败:', e)
  }
}
function formatAlgorithmName(code: string) {
  if (!code) return '--'
  const algorithm = algorithms.value.find(a => a.code === code)
  return algorithm?.name || code
}

const chunkAllSelected = computed(() =>
  chunks.value.length > 0 && chunks.value.every((c) => chunkSelectedIds.value.has(c.id))
);

function toggleChunkSelectAll() {
  if (chunkAllSelected.value) {
    chunks.value.forEach((c) => chunkSelectedIds.value.delete(c.id));
  } else {
    chunks.value.forEach((c) => chunkSelectedIds.value.add(c.id));
  }
  chunkSelectedIds.value = new Set(chunkSelectedIds.value);
}

function toggleChunkSelect(id: string) {
  if (chunkSelectedIds.value.has(id)) {
    chunkSelectedIds.value.delete(id);
  } else {
    chunkSelectedIds.value.add(id);
  }
  chunkSelectedIds.value = new Set(chunkSelectedIds.value);
}

async function askDeleteChunk(chunkId: string, hard = false) {
  try {
    await ElMessageBox.confirm(
      t('knowledge.confirmDeleteChunkMsg'),
      t('knowledge.confirmDeleteChunkTitle'),
      {
        type: 'warning',
        confirmButtonText: t('knowledge.confirmDelete'),
        cancelButtonText: t('common.cancel'),
        confirmButtonClass: 'el-button--danger',
        cancelButtonClass: 'delete-confirm-cancel-btn',
        customClass: 'delete-confirm-dialog',
      },
    );
  } catch {
    return;
  }
  await handleDeleteChunk(chunkId, hard);
}

async function handleDeleteChunk(chunkId: string, hard = false) {
  try {
    await chunkApi.delete(chunkId, hard);
    uiStore.showToast('success', '切片已删除');
    chunkSelectedIds.value.delete(chunkId);
    // 立即从本地列表移除，避免"统计变了列表没变"的视觉不一致
    chunks.value = chunks.value.filter(c => c.id !== chunkId);
    chunkTotalCount.value = Math.max(0, chunkTotalCount.value - 1);
    // 如果当前页为空且不是第一页，回退一页
    if (chunks.value.length === 0 && chunkPage.value > 1) {
      chunkPage.value--;
    }
    await fetchChunks();
    await fetchKnowledgeBase(); // 同步头部「切片数」统计
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '删除失败');
  }
}

async function askBatchDeleteChunks(mode: 'soft' | 'hard' = 'soft') {
  if (chunkSelectedIds.value.size === 0) {
    uiStore.showToast('warning', '请先勾选要删除的切片');
    return;
  }
  try {
    await ElMessageBox.confirm(
      t('knowledge.confirmBatchDeleteChunks', { count: chunkSelectedIds.value.size }),
      t('knowledge.confirmDeleteChunkTitle'),
      {
        type: 'warning',
        confirmButtonText: t('knowledge.confirmDelete'),
        cancelButtonText: t('common.cancel'),
        confirmButtonClass: 'el-button--danger',
        cancelButtonClass: 'delete-confirm-cancel-btn',
        customClass: 'delete-confirm-dialog',
      },
    );
  } catch {
    return;
  }
  await handleBatchDeleteChunks(mode);
}

async function handleBatchDeleteChunks(mode: 'soft' | 'hard' = 'soft') {
  if (chunkSelectedIds.value.size === 0) {
    uiStore.showToast('warning', '请先勾选要删除的切片');
    return;
  }
  chunkBatchDeleting.value = true;
  try {
    const ids = Array.from(chunkSelectedIds.value);
    const result = await chunkApi.batchDelete(ids, mode);
    uiStore.showToast('success', `批量删除完成：${result.deleted} 个切片`);
    // 立即从本地列表移除
    const deletedSet = new Set(ids);
    chunks.value = chunks.value.filter(c => !deletedSet.has(c.id));
    chunkTotalCount.value = Math.max(0, chunkTotalCount.value - (result.deleted || ids.length));
    chunkSelectedIds.value = new Set();
    // 回退页码
    if (chunks.value.length === 0 && chunkPage.value > 1) {
      chunkPage.value = 1;
    }
    await fetchChunks();
    await fetchKnowledgeBase(); // 同步头部「切片数」统计
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '批量删除失败');
  } finally {
    chunkBatchDeleting.value = false;
  }
}

async function previewCleanup() {
  cleanupLoading.value = true;
  try {
    const res = await chunkApi.cleanupMeaningless({
      knowledge_base_id: id.value,
      min_len: cleanupForm.value.min_len,
      scan_limit: cleanupForm.value.scan_limit,
      dry_run: true,
      mode: cleanupForm.value.mode,
    });
    cleanupResult.value = {
      scanned: res.scanned,
      matched: res.matched,
      samples: res.samples,
    };
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '预览失败');
  } finally {
    cleanupLoading.value = false;
  }
}

async function executeCleanup() {
  if (!cleanupResult.value || cleanupResult.value.matched === 0) {
    uiStore.showToast('warning', '请先点击"预览扫描"，没有命中无需清理');
    return;
  }
  if (!await confirmDialog(`确定清理 ${cleanupResult.value.matched} 个无意义切片？此操作不可撤销。`, { closeOnClickModal: false })) return;
  cleanupLoading.value = true;
  try {
    const res = await chunkApi.cleanupMeaningless({
      knowledge_base_id: id.value,
      min_len: cleanupForm.value.min_len,
      scan_limit: cleanupForm.value.scan_limit,
      dry_run: false,
      mode: cleanupForm.value.mode,
    });
    uiStore.showToast('success', `清理完成：删除 ${res.deleted} 个切片`);
    cleanupResult.value = null;
    showCleanupModal.value = false;
    chunkPage.value = 1;  // 回到第一页确保列表刷新
    await fetchChunks();
    await fetchKnowledgeBase(); // 同步头部「切片数」统计
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '清理失败');
  } finally {
    cleanupLoading.value = false;
  }
}

function handleChunkPageSizeChange() {
  localStorage.setItem('kb_chunk_page_size', String(chunkPageSize.value));
  chunkPage.value = 1;
  fetchChunks();
}

function handleFilePageSizeChange() {
  localStorage.setItem('kb_file_page_size', String(filePageSize.value));
  filePage.value = 1;
  fetchFiles();
}

// ============================================================
// v1.1 §1.1: 知识库授权管理
// ============================================================

const grants = ref<KbGrant[]>([]);
const grantsLoading = ref(false);
const showGrantModal = ref(false);
const GRANT_USER_ID = 'admin'
const grantForm = ref<CreateKbGrantRequest>({
  subject_type: 'user',
  subject_id: GRANT_USER_ID,
  permission: 'read',
});
const grantSubmitting = ref(false);

// visibility 设置（独立保存）
const visibilityForm = ref<'private' | 'public' | 'restricted'>('private');
const visibilitySaving = ref(false);

async function fetchGrants() {
  grantsLoading.value = true;
  try {
    grants.value = await kbGrantApi.list(id.value);
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '获取授权列表失败');
  } finally {
    grantsLoading.value = false;
  }
}

function resetGrantForm() {
  grantForm.value = { subject_type: 'user', subject_id: GRANT_USER_ID, permission: 'read' };
}

function onGrantSubjectType(type: 'user' | 'role' | 'dept') {
  grantForm.value.subject_type = type;
  grantForm.value.subject_id = type === 'user' ? GRANT_USER_ID : '';
}

async function handleAddGrant() {
  const subjectId = grantForm.value.subject_type === 'user'
    ? GRANT_USER_ID
    : grantForm.value.subject_id.trim();
  if (!subjectId) {
    uiStore.showToast('warning', '请输入 subject ID');
    return;
  }
  grantSubmitting.value = true;
  try {
    await kbGrantApi.grant(id.value, {
      ...grantForm.value,
      subject_id: subjectId,
    });
    uiStore.showToast('success', '授权成功');
    showGrantModal.value = false;
    resetGrantForm();
    await fetchGrants();
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '授权失败');
  } finally {
    grantSubmitting.value = false;
  }
}

async function handleRevokeGrant(grantId: string) {
  if (!await confirmDialog('确定撤销此授权？', { closeOnClickModal: false })) return;
  try {
    await kbGrantApi.revoke(id.value, grantId);
    uiStore.showToast('success', '已撤销');
    await fetchGrants();
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '撤销失败');
  }
}

async function handleSaveVisibility() {
  if (!knowledgeBase.value) return;
  visibilitySaving.value = true;
  try {
    // 复用现有 update 接口，同时把 is_public 兼容写好
    await knowledgeBaseApi.update(id.value, {
      visibility: visibilityForm.value,
      is_public: visibilityForm.value === 'public',
    });
    knowledgeBase.value.visibility = visibilityForm.value;
    knowledgeBase.value.is_public = visibilityForm.value === 'public';
    uiStore.showToast('success', '可见范围已更新');
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '保存失败');
  } finally {
    visibilitySaving.value = false;
  }
}

function subjectTypeLabel(type: string): string {
  const key = 'knowledge.subjectType_' + type
  const translated = t(key)
  return translated !== key ? translated : type
}

function permissionLabel(p: string): string {
  const key = 'knowledge.permission_' + p
  const translated = t(key)
  return translated !== key ? translated : p
}

function permissionBadgeClass(p: string): string {
  switch (p) {
    case 'admin':
      return 'bg-rose-50 text-rose-700 border-rose-200 dark:bg-rose-500/10 dark:text-rose-400 dark:border-rose-500/30';
    case 'write':
      return 'bg-amber-50 text-amber-700 border-amber-200 dark:bg-amber-500/10 dark:text-amber-400 dark:border-amber-500/30';
    case 'read':
    default:
      return 'bg-blue-50 text-blue-700 border-blue-200 dark:bg-blue-500/10 dark:text-blue-400 dark:border-blue-500/30';
  }
}

function openSettingsTab() {
  activeTab.value = 'settings';
  if (knowledgeBase.value) {
    const v = knowledgeBase.value.visibility;
    visibilityForm.value = (v as any) || (knowledgeBase.value.is_public ? 'public' : 'private');
  }
  fetchGrants();
}

// QA Modal
const showChunkQAModal = ref(false);
const selectedChunkId = ref('');

const id = computed(() => route.params.id as string);

function resetDetailState() {
  knowledgeBase.value = null;
  files.value = [];
  chunks.value = [];
  fileTotalCount.value = 0;
  chunkTotalCount.value = 0;
  chunkViewerTarget.value = null;
  loading.value = true;
  stopVectorPoll();
  stopProcessingPoll();
}

async function loadDetailPage() {
  // 从知识库列表进入时复用已确认的应用级就绪状态；仅状态未知/失效时探活。
  if (!kgServiceReady.value) await ensureKgReady({ startIfNeeded: false });
  if (!kgServiceReady.value) {
    loading.value = false;
    uiStore.showToast('error', '知识库服务未就绪');
    return;
  }
  loadAlgorithms();
  await fetchKnowledgeBase();
}

async function fetchKnowledgeBase() {
  try {
    knowledgeBase.value = await knowledgeBaseApi.get(id.value);
    if (knowledgeBase.value) {
      editForm.value = {
        name: knowledgeBase.value.name,
        description: knowledgeBase.value.description || '',
        is_public: knowledgeBase.value.is_public,
      };
    }
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '获取知识库失败');
    // 加载失败回到知识库列表（路由为 /knowledge，此前误写 /knowledge-bases 无匹配 → 空白页）
    router.push('/knowledge');
  } finally {
    loading.value = false;
  }
}

function fileIsInFlight(file: FileInfo): boolean {
  const vector = file.vector_status;
  if (vector === 'running' || vector === 'pending') return true;
  return file.status === 'pending' || file.status === 'processing' || file.status === 'uploaded';
}

function isVectorRunning(file: FileInfo): boolean {
  if (file.vector_status === 'running') return true;
  return !file.vector_status && (file.status === 'processing' || file.status === 'uploaded');
}

let filesRequestPromise: Promise<void> | null = null;
let filesRequestKbId = '';

async function performFetchFiles(kbId: string, silent = false) {
  if (!silent) filesLoading.value = true;
  if (!kbId) {
    files.value = [];
    fileTotalCount.value = 0;
    if (!silent) filesLoading.value = false;
    return;
  }
  try {
    const response = await fileApi.list({
      knowledge_base_id: kbId,
      page: filePage.value,
      page_size: filePageSize.value,
    });
    // 防止慢请求返回时已切换到其他知识库
    if (id.value !== kbId) return;
    files.value = response.items || [];
    fileTotalCount.value = response.total ?? 0;
  } catch (e) {
    if (id.value !== kbId) return;
    // 失败时停掉轮询，避免每 2s 打一次 403 刷屏
    stopVectorPoll();
    if (!silent) {
      files.value = [];
      fileTotalCount.value = 0;
      uiStore.showToast('error', e instanceof Error ? e.message : '获取文件列表失败');
    }
  } finally {
    if (!silent && id.value === kbId) filesLoading.value = false;
  }
}

function fetchFiles(silent = false): Promise<void> {
  const kbId = id.value;
  if (filesRequestPromise && filesRequestKbId === kbId) return filesRequestPromise;

  filesRequestKbId = kbId;
  let request: Promise<void>;
  request = performFetchFiles(kbId, silent).finally(() => {
    if (filesRequestPromise === request) {
      filesRequestPromise = null;
      filesRequestKbId = '';
    }
  });
  filesRequestPromise = request;
  return request;
}

async function fetchChunks() {
  chunksLoading.value = true;
  try {
    const response = await chunkApi.list({
      knowledge_base_id: id.value,
      page: chunkPage.value,
      page_size: chunkPageSize.value,
    });
    chunks.value = response.items || [];
    chunkTotalCount.value = response.total;
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '获取切片列表失败');
  } finally {
    chunksLoading.value = false;
  }
}


async function handleUpdate() {
  if (!editForm.value.name.trim()) {
    uiStore.showToast('warning', '请输入知识库名称');
    return;
  }
  editLoading.value = true;
  try {
    const updated = await knowledgeBaseApi.update(id.value, editForm.value);
    knowledgeBase.value = updated;
    showEditModal.value = false;
    uiStore.showToast('success', '更新成功');
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '更新失败');
  } finally {
    editLoading.value = false;
  }
}

async function handleRefreshStats() {
  try {
    knowledgeBase.value = await knowledgeBaseApi.refreshStats(id.value);
    uiStore.showToast('success', '统计信息已刷新');
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '刷新失败');
  }
}

async function handleUpload() {
  if (!uploadFile.value) {
    uiStore.showToast('warning', '请选择文件');
    return;
  }
  uploadLoading.value = true;
  uploadProgress.value = 0;
  try {
    const formData = new FormData();
    formData.append('file', uploadFile.value);
    formData.append('knowledge_base_id', id.value);
    // FastKG 私有库要求调用者身份；仅 Authorization Bearer 不够，需显式 user_id
    formData.append('user_id', 'admin');
    await fileApi.upload(formData, (progress) => {
      uploadProgress.value = progress;
    });
    showUploadModal.value = false;
    uploadFile.value = null;
    uploadProgress.value = 0;
    // HTTP 传输完成 ≠ 处理完成：FastKG 的切块/embedding 仍在后台进行，轮询刷新状态
    uiStore.showToast('success', '上传成功，正在后台解析…');
    startProcessingPoll();
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '上传失败');
  } finally {
    uploadLoading.value = false;
  }
}

// 上传后轮询文件处理状态（切块/embedding），直到无 pending/processing 或达上限；
// 失败给出提示，完成后刷新切片统计。避免「显示上传成功但切片数 0、状态卡在处理中」。
let _processingPollTimer: ReturnType<typeof setTimeout> | null = null;
function stopProcessingPoll() {
  if (_processingPollTimer != null) {
    clearTimeout(_processingPollTimer);
    _processingPollTimer = null;
  }
}
async function pollProcessing(attempt = 0) {
  const MAX_ATTEMPTS = 15;
  const inFlightIds = new Set(files.value.filter(fileIsInFlight).map(f => f.id));
  await fetchFiles(true);
  const newlyFailed = files.value.some(f =>
    inFlightIds.has(f.id) && (f.vector_status === 'failed' || f.status === 'failed')
  );
  if (newlyFailed) {
    uiStore.showToast('error', '部分文件解析失败，请删除后重试');
  }
  const hasPending = files.value.some(fileIsInFlight);
  if (hasPending && attempt < MAX_ATTEMPTS) {
    _processingPollTimer = setTimeout(() => pollProcessing(attempt + 1), 2500);
  } else {
    stopProcessingPoll();
    await fetchKnowledgeBase();
    // 处理轮询达到上限但仍有任务时，交给文件标签的轻量串行轮询继续跟踪。
    if (hasPending && activeTab.value === 'files') startVectorPoll();
  }
}
function startProcessingPoll() {
  stopProcessingPoll();
  pollProcessing(0);
}

async function askDeleteFile(fileId: string, fileName: string) {
  try {
    await ElMessageBox.confirm(
      t('knowledge.confirmDeleteFileMsg', { name: fileName }),
      t('knowledge.confirmDeleteFileTitle'),
      {
        type: 'warning',
        confirmButtonText: t('knowledge.confirmDelete'),
        cancelButtonText: t('common.cancel'),
        confirmButtonClass: 'el-button--danger',
        cancelButtonClass: 'delete-confirm-cancel-btn',
        customClass: 'delete-confirm-dialog',
      },
    );
  } catch {
    return;
  }
  await handleDeleteFile(fileId);
}

async function handleDeleteFile(fileId: string) {
  try {
    await fileApi.delete(fileId);
    uiStore.showToast('success', '删除成功');
    fetchFiles();
    fetchKnowledgeBase();
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '删除失败');
  }
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
    active: 'badge-success',
    inactive: 'badge-secondary',
    building: 'badge-warning',
    pending: 'badge-gray',
    processing: 'badge-warning',
    completed: 'badge-success',
    failed: 'badge-danger',
  };
  return classes[status] || 'badge-gray';
}

function getStatusText(status: string): string {
  const key = 'knowledge.status_' + status
  const translated = t(key)
  return translated !== key ? translated : status
}

function vectorStatusText(status?: string): string {
  const key = status ? `knowledge.vectorStatus_${status}` : 'knowledge.vectorStatus_unknown'
  const translated = t(key)
  if (translated !== key) return translated
  return t('knowledge.vectorStatus_unknown')
}

function vectorBadgeClass(status?: string): string {
  const classes: Record<string, string> = {
    pending: 'badge-gray',
    completed: 'badge-success',
    failed: 'badge-danger',
    cancelled: 'badge-gray',
  };
  return (status && classes[status]) || 'badge-gray';
}

function vectorBarClass(status?: string): string {
  const classes: Record<string, string> = {
    running: 'file-vector-bar--running',
    pending: 'file-vector-bar--pending',
    completed: 'file-vector-bar--completed',
    failed: 'file-vector-bar--failed',
    cancelled: 'file-vector-bar--cancelled',
  };
  return (status && classes[status]) || 'file-vector-bar--cancelled';
}

let vectorPollTimer: ReturnType<typeof setTimeout> | null = null;
let vectorPollActive = false;
function stopVectorPoll() {
  vectorPollActive = false;
  if (vectorPollTimer) {
    clearTimeout(vectorPollTimer);
    vectorPollTimer = null;
  }
}

async function runVectorPoll() {
  vectorPollTimer = null;
  if (!vectorPollActive || activeTab.value !== 'files') return;

  if (document.visibilityState === 'visible') await fetchFiles(true);

  if (!vectorPollActive || activeTab.value !== 'files') return;
  if (!files.value.some(fileIsInFlight)) {
    stopVectorPoll();
    return;
  }
  vectorPollTimer = setTimeout(() => { void runVectorPoll(); }, 2000);
}

function startVectorPoll() {
  if (vectorPollActive || !files.value.some(fileIsInFlight)) return;
  vectorPollActive = true;
  vectorPollTimer = setTimeout(() => { void runVectorPoll(); }, 2000);
}

async function openFilesTab() {
  activeTab.value = 'files';
  await fetchFiles();
  startVectorPoll();
}

async function refreshActiveTab() {
  const requests: Promise<unknown>[] = [fetchKnowledgeBase()];
  if (activeTab.value === 'files') requests.push(fetchFiles());
  else if (activeTab.value === 'chunks') requests.push(fetchChunks());
  else if (activeTab.value === 'settings') requests.push(fetchGrants());
  await Promise.all(requests);
}

watch(activeTab, (tab) => {
  if (tab !== 'files') stopVectorPoll();
});

// 切换知识库 id 时必须清空并重载，避免 keep-alive / 慢请求导致串库文件列表
watch(id, async (newId, oldId) => {
  if (!newId || newId === oldId) return;
  resetDetailState();
  await loadDetailPage();
});

onMounted(async () => {
  await loadDetailPage();
});

let hasActivatedOnce = false;
onActivated(async () => {
  // KeepAlive 首次挂载会同时触发 mounted 与 activated；首次 activated 不重复初始化。
  if (!hasActivatedOnce) {
    hasActivatedOnce = true;
    if (showUploadModal.value) void bindTauriUploadDrop();
    return;
  }
  if (!kgServiceReady.value) await ensureKgReady({ startIfNeeded: false });
  if (kgServiceReady.value) {
    await refreshActiveTab();
  }
  if (activeTab.value === 'files') startVectorPoll();
  if (showUploadModal.value) void bindTauriUploadDrop();
});

onDeactivated(() => {
  stopVectorPoll();
  stopProcessingPoll();
  unbindTauriUploadDrop();
});

onBeforeUnmount(() => {
  stopVectorPoll();
  stopProcessingPoll();
  unbindTauriUploadDrop();
});
</script>

<template>
  <div class="kb-detail-page space-y-6">
    <!-- 同页分块可视化：不离开知识库详情 Tab -->
    <ChunkViewer
      v-if="chunkViewerTarget"
      class="kb-embedded-chunk-viewer"
      embedded
      :file-id="chunkViewerTarget.fileId"
      :file-name="chunkViewerTarget.fileName"
      :kb-id="id"
      @back="closeChunkViewer"
    />

    <!-- 加载状态 -->
    <div v-else-if="loading" class="flex items-center justify-center min-h-96">
      <LoadingSpinner size="lg" :text="t('common.loading')" />
    </div>

    <template v-else-if="knowledgeBase">
      <!-- 头部 -->
      <div class="flex items-start justify-between">
        <div class="flex items-center gap-4">
          <button @click="router.push('/knowledge')"
            class="p-2 rounded-lg hover:bg-slate-30 dark:hover:bg-dark-700 transition-colors cursor-pointer">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
            </svg>
          </button>
          <div>
            <div class="flex items-center gap-3">
              <h1 class="text-2xl font-bold">{{ knowledgeBase.name }}</h1>
              <span :class="['badge', getStatusClass(knowledgeBase.status)]">
                {{ getStatusText(knowledgeBase.status) }}
              </span>
              <span v-if="knowledgeBase.is_public" class="badge badge-info">{{ $t('knowledge.visPublic') }}</span>
            </div>
            
            <p class="text-slate-600 dark:text-dark-400 mt-1">{{ knowledgeBase.description || '暂无描述' }}</p>
          </div>
        </div>
        <div class="flex items-center gap-3">
          <button @click="handleRefreshStats" class="btn btn-primary">
            <svg class="w-4 h-4 mr-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
            {{ $t('knowledge.refreshStats') }}
          </button>
          <button @click="showEditModal = true" class="btn btn-primary">
            <svg class="w-4 h-4 mr-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
            </svg>
            {{ $t('common.edit') }}
          </button>
        </div>
      </div>

      <!-- 统计卡片 -->
      <div class="kb-detail-stats">
        <div class="stat-card">
          <div class="stat-card-icon" style="background: #e6f4ff !important; color: #1e7cff !important">
            <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="#000">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
            </svg>
          </div>
            <div class="stat-card-label">{{ $t('knowledge.fileCount') }}</div>
            <div class="stat-card-value">{{ knowledgeBase.file_count }}</div>
        </div>
        <div class="stat-card">
          <div class="stat-card-icon" style="background: #f6ffed !important; color: #60c92b !important">
            <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="#000">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4" />
            </svg>
          </div>
          <!-- <div> -->
            <div class="stat-card-label">{{ $t('knowledge.chunkCount') }}</div>
            <div class="stat-card-value">{{ knowledgeBase.chunk_count.toLocaleString() }}</div>
          <!-- </div> -->
        </div>
        <div class="stat-card">
          <div class="stat-card-icon" style="background: #f0f5ff !important; color: #2f54eb !important">
            <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="#000">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z" />
            </svg>
          </div>
            <div class="stat-card-label">{{ $t('knowledge.totalSize') }}</div>
            <div class="stat-card-value">{{ formatFileSize(knowledgeBase.total_size) }}</div>
        </div>
        <div class="stat-card">
          <div class="stat-card-icon" style="background: #fffbe6 !important; color: #faad14 !important">
            <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="#000">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
            </svg>
          </div>
            <div class="stat-card-label">{{ $t('knowledge.vectorDim') }}</div>
            <div class="stat-card-value">{{ knowledgeBase.embedding_dimension }}</div>
        </div>
      </div>

      <!-- 标签页 -->
      <div class="card">
        <div class="tabs flex border-b border-slate-200 dark:border-dark-700">
          <button
            :class="['tab px-4 py-2 text-base font-bold transition-colors', activeTab === 'overview' ? 'tab-active border-b-2 border-[var(--color-primary)] bg-[var(--color-primary)]' : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200']"
            @click="activeTab = 'overview'">
            {{ $t('knowledge.overview') }}
          </button>
          <button
            :class="['tab px-4 py-2 text-base font-bold transition-colors', activeTab === 'files' ? 'tab-active border-b-2 border-[var(--color-primary)] bg-[var(--color-primary)]' : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200']"
            @click="openFilesTab">
            {{ $t('knowledge.filesTab') }} ({{ fileTotalCount || knowledgeBase.file_count }})
          </button>
          <button
            :class="['tab px-4 py-2 text-base font-bold transition-colors', activeTab === 'chunks' ? 'tab-active border-b-2 border-[var(--color-primary)] bg-[var(--color-primary)]' : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200']"
            @click="activeTab = 'chunks'; chunkPage = 1; fetchChunks()">
            {{ $t('knowledge.chunks') }}
          </button>
          <button
            :class="['tab px-4 py-2 text-base font-bold transition-colors', activeTab === 'settings' ? 'tab-active border-b-2 border-[var(--color-primary)] bg-[var(--color-primary)]' : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200']"
            @click="openSettingsTab">
            {{ $t('knowledge.settingsTab') }}
          </button>
        </div>

        <div class="content-box1">
          <!-- 概览面板 -->
          <div v-if="activeTab === 'overview'" class="p-6 space-y-6">
            <div class="grid grid-cols-2 gap-6">
              <div class="space-y-4">
                <h3 class="text-lg font-semibold">{{ $t("knowledge.basicInfo") }}</h3>
                <div class="space-y-3">
                  <!-- <div class="flex justify-between py-2 border-b border-slate-200 dark:border-dark-700">
                    <span class="text-slate-600 dark:text-dark-400 text-base">{{ $t("knowledge.kbId") }}</span>
                    <span class="font-mono text-base">{{ knowledgeBase.id }}</span>
                  </div> -->
                  <div class="flex justify-between py-2 border-b border-slate-200 dark:border-dark-700">
                    <span class="text-slate-600 dark:text-dark-400 text-base">{{ $t("knowledge.detailKbCode") }}</span>
                    <span class="font-mono text-base ">{{ knowledgeBase.code }}</span>
                  </div>
                  <div class="flex justify-between py-2 border-b border-slate-200 dark:border-dark-700">
                    <span class="text-slate-600 dark:text-dark-400 text-base">{{ $t("knowledge.detailEmbeddingModel") }}</span>
                    <span class="text-base">{{ knowledgeBase.embedding_model }}</span>
                  </div>
                  <div class="flex justify-between py-2 border-b border-slate-200 dark:border-dark-700">
                    <span class="text-slate-600 dark:text-dark-400  text-base">{{ $t("knowledge.detailIndexType") }}</span>
                    <span class="text-base">{{ knowledgeBase.index_type }}</span>
                  </div>
                  <div v-if="knowledgeBase.default_algorithm"
                    class="flex justify-between py-2 border-b border-slate-200 dark:border-dark-700">
                    <span class="text-slate-600 dark:text-dark-400 text-base ">{{ $t("knowledge.defaultAlgorithm") }}</span>
                    <span class="text-base">{{ formatAlgorithmName(knowledgeBase.default_algorithm) }}</span>
                  </div>
                </div>
              </div>
              <div class="space-y-4">
                <h3 class="text-lg font-semibold">{{ $t("knowledge.timeInfo") }}</h3>
                <div class="space-y-3">
                  <div class="flex justify-between py-2 border-b border-slate-200 dark:border-dark-700">
                    <span class="text-slate-600 dark:text-dark-400 text-base">{{ $t("knowledge.createdAt") }}</span>
                    <span class="text-base">{{ formatDate(knowledgeBase.created_at) }}</span>
                  </div>
                  <div class="flex justify-between py-2 border-b border-slate-200 dark:border-dark-700">
                    <span class="text-slate-600 dark:text-dark-400 text-base">{{ $t("knowledge.updatedAt") }}</span>
                    <span class="text-base">{{ formatDate(knowledgeBase.updated_at) }}</span>
                  </div>
                  <div class="flex justify-between py-2 border-b border-slate-200 dark:border-dark-700">
                    <span class="text-slate-600 dark:text-dark-400 text-base">{{ $t("knowledge.owner") }}</span>
                    <span class="text-base">admin</span>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- 文件面板 -->
          <div v-if="activeTab === 'files'" class="files-tab-panel">
            <div class="files-header flex justify-between items-center">
              <h3 class="text-lg font-semibold">{{ $t('knowledge.fileList') }}</h3>
              <button @click="showUploadModal = true" class="btn btn-primary">
                <svg class="w-4 h-4 mr-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                    d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
                </svg>
                {{ $t('knowledge.uploadFile') }}
              </button>
            </div>

            <div v-if="filesLoading" class="py-8 flex-1 flex items-center justify-center">
              <LoadingSpinner :text="t('common.loading')" />
            </div>
            <div v-else-if="files.length === 0" class="flex-1">
              <EmptyState icon="document" title="暂无文件" description="点击上方按钮上传文件" />
            </div>
            <template v-else>
              <div class="files-table-card">
                <div ref="filesTableHeadRef" class="files-table-head">
                  <table class="files-table">
                    <colgroup>
                      <col class="col-name" />
                      <col class="col-type" />
                      <col class="col-size" />
                      <col class="col-status" />
                      <col class="col-time" />
                      <col class="col-action" />
                    </colgroup>
                    <thead>
                      <tr>
                        <th class="text-left">{{ $t('knowledge.fileName') }}</th>
                        <th class="text-center">{{ $t('knowledge.detailType') }}</th>
                        <th class="text-center">{{ $t('knowledge.detailSize') }}</th>
                        <th class="text-center">{{ $t('knowledge.statusLabel') }}</th>
                        <th class="text-center">{{ $t('knowledge.uploadTime') }}</th>
                        <th class="text-right">{{ $t('knowledge.action') }}</th>
                      </tr>
                    </thead>
                  </table>
                </div>
                <div ref="filesTableBodyRef" class="files-table-body-scroll" @scroll="onFilesTableBodyScroll">
                  <table class="files-table">
                    <colgroup>
                      <col class="col-name" />
                      <col class="col-type" />
                      <col class="col-size" />
                      <col class="col-status" />
                      <col class="col-time" />
                      <col class="col-action" />
                    </colgroup>
                    <tbody>
                      <tr v-for="file in files" :key="file.id">
                        <td>
                          <div class="flex items-center gap-2 min-w-0">
                            <svg class="w-5 h-5 text-slate-500 dark:text-dark-400 flex-shrink-0" fill="none" viewBox="0 0 24 24"
                              stroke="currentColor">
                              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                                d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                            </svg>
                            <span class="truncate" :title="file.original_name">{{ file.original_name }}</span>
                          </div>
                        </td>
                        <td class="is-center"><span class="badge badge-secondary">.{{ file.file_ext }}</span></td>
                        <td class="is-center">{{ formatFileSize(file.file_size) }}</td>
                        <td class="file-status-cell">
                          <template v-if="isVectorRunning(file)">
                            <div class="file-vector-status">
                              <div class="file-vector-status__row">
                                <span class="file-vector-status__label">{{ vectorStatusText('running') }}</span>
                                <span v-if="file.vector_progress != null" class="file-vector-status__pct">
                                  {{ file.vector_progress }}%
                                </span>
                              </div>
                              <div class="file-vector-status__track">
                                <div
                                  class="file-vector-status__bar"
                                  :class="vectorBarClass('running')"
                                  :style="{ width: (file.vector_progress ?? 0) + '%' }"
                                ></div>
                              </div>
                              <span
                                v-if="file.vector_progress_message"
                                class="file-vector-status__msg"
                                :title="file.vector_progress_message"
                              >
                                {{ file.vector_progress_message }}
                              </span>
                            </div>
                          </template>
                          <span v-else-if="file.vector_status" :class="['badge', vectorBadgeClass(file.vector_status)]">
                            {{ vectorStatusText(file.vector_status) }}
                          </span>
                          <span v-else :class="['badge', getStatusClass(file.status)]">{{ getStatusText(file.status) }}</span>
                        </td>
                        <td class="is-center text-slate-600 dark:text-dark-400 text-sm">{{ formatDate(file.upload_time) }}</td>
                        <td class="text-right">
                          <div class="file-row-actions">
                            <button
                              type="button"
                              class="file-view-chunks-btn"
                              :title="$t('knowledge.viewChunks')"
                              @click="openChunkViewer(file)"
                            >
                              <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M4 6h16M4 10h16M4 14h16M4 18h16" />
                              </svg>
                            </button>
                            <button
                              type="button"
                              class="btn btn-ghost btn-sm text-error"
                              @click="askDeleteFile(file.id, file.original_name)"
                            >
                              {{ $t('common.delete') }}
                            </button>
                          </div>
                        </td>
                      </tr>
                    </tbody>
                  </table>
                </div>
              </div>
              <!-- 文件分页 -->
              <div class="files-pager">
                <el-pagination
                  v-model:current-page="filePage"
                  v-model:page-size="filePageSize"
                  :page-sizes="PAGE_SIZE_OPTIONS"
                  :total="fileTotalCount"
                  layout="total, sizes, prev, pager, next, jumper"
                  @size-change="handleFilePageSizeChange"
                  @current-change="fetchFiles"
                />
              </div>
            </template>
          </div>

          <!-- 切片面板 -->
          <div v-if="activeTab === 'chunks'" class="chunks-tab-panel chunks-panel">
            <!-- 工具条（固定不滚动） -->
            <div class="chunks-toolbar flex items-center justify-between flex-wrap gap-2">
              <div class="flex items-center gap-3">
                <!-- <label v-if="chunks.length > 0"
                 class="filter-check">
                  <input type="checkbox" :checked="chunkAllSelected" @change="toggleChunkSelectAll"
                    class="w-4 h-4 cursor-pointer accent-[var(--color-primary)]" />           
                  {{ $t('knowledge.selectAllCurrentPage') }}
                  
                </label> -->
                <label v-if="chunks.length > 0" class="filter-check">
                <!-- 原有input逻辑完全不动 -->
                <input
                type="checkbox"
               :checked="chunkAllSelected"
               @change="toggleChunkSelectAll"
              class="origin-checkbox"
             />
               <!-- 自定义勾选方块 -->
             <span class="check-box "></span>
            <span class="check-text">{{ $t('knowledge.selectAllCurrentPage') }}</span>
             </label>
                <span v-if="chunkSelectedIds.size > 0" class="text-md text-orange-600 dark:text-orange-400">
                  已选 {{ chunkSelectedIds.size }} 项
                </span>
              </div>
              
              <div class="flex items-center gap-2">
                <button
                  v-if="chunkSelectedIds.size > 0"
                  type="button"
                  :disabled="chunkBatchDeleting"
                  class="btn-primarydelete"
                  @click="askBatchDeleteChunks('soft')"
                >
                  {{ $t('knowledge.detailBatchDelete') }}
                </button>
                <button @click="showCleanupModal = true; cleanupResult = null" class="btn btn-secondary cleanup-btn"
                  :title="$t('knowledge.cleanupDesc')">
                  <svg class="w-4 h-4 mr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                      d="M19.428 15.428a2 2 0 00-1.022-.547l-2.387-.477a6 6 0 00-3.86.517l-.318.158a6 6 0 01-3.86.517L6.05 15.21a2 2 0 00-1.806.547M8 4h8l-1 1v5.172a2 2 0 00.586 1.414l5 5c1.26 1.26.367 3.414-1.415 3.414H4.828c-1.782 0-2.674-2.154-1.414-3.414l5-5A2 2 0 009 10.172V5L8 4z" />
                  </svg>
                  {{ $t('knowledge.cleanupMeaningless') }}
                </button>
              </div>
            </div>

            <!-- 卡片滚动区域 -->
            <div class="chunks-scroller">
              <div v-if="chunksLoading" class="py-8">
                <LoadingSpinner :text="t('common.loading')" />
              </div>
              <div v-else-if="chunks.length === 0">
                <EmptyState icon="database" title="暂无切片" description="上传文件后会自动生成切片" />
              </div>
              <div v-else class="space-y-4">
                <div v-for="chunk in chunks" :key="chunk.id"
                  class="p-4 rounded-lg border border-slate-200 dark:border-slate-700 hover:border-blue-500/30 transition-colors chunk-card"
                  :class="{ 'ring-1 ring-white-500/80': chunkSelectedIds.has(chunk.id) }">
                  <div class="flex justify-between items-start mb-2 chunk-card-header"
                    style="background: transparent !important;">
                    <div class="flex items-center gap-3 !bg-transparent">
                      <label class="filter-check">
                      <input type="checkbox" :checked="chunkSelectedIds.has(chunk.id)"
                        @change="toggleChunkSelect(chunk.id)" class="w-4 h-4 cursor-pointer accent-[var(--color-primary)]" />
                          <!-- 自定义勾选框容器 -->
                       <span class="check-box"></span> 
                       </label>
                      <span class="badge badge-secondary">{{ $t('knowledge.chunkNumber', { index: chunk.chunk_index + 1 }) }}</span>
                      <span class="text-sm text-slate-500">{{ chunk.chunk_size || chunk.chunk_content.length }} {{ $t('knowledge.chars') }}</span>
                    </div>
                    <div class="flex items-center gap-2">
                      <button type="button" @click.stop="selectedChunkId = chunk.id; showChunkQAModal = true"
                        class="relative z-10 cursor-pointer pointer-events-auto text-sm flex items-center gap-1 text-blue-400 hover:text-blue-300 transition-colors px-2 py-1 rounded bg-blue-500/10 hover:bg-blue-500/20">
                        <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                            d="M8 10h.01M12 10h.01M16 10h.01M9 16H5a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v8a2 2 0 01-2 2h-5l-5 5v-5z" />
                        </svg>
                        {{ $t('knowledge.qaManagement') }}
                      </button>
                      <button type="button"
                        class="text-sm flex cursor-pointer items-center gap-1 text-red-500 hover:text-red-400 transition-colors px-2 py-1 rounded bg-red-500/10 hover:bg-red-500/20"
                        :title="$t('knowledge.deleteSoft')"
                        @click.stop="askDeleteChunk(chunk.id, false)">
                        <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                            d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                        </svg>
                        {{ $t('common.delete') }}
                      </button>
                    </div>
                  </div>
                  <p class="text-base text-slate-700 dark:text-slate-300 line-clamp-3 font-mono leading-relaxed">{{
                    chunk.chunk_content }}</p>
                </div>
              </div>
            </div>

            <!-- 分页（固定在底部，始终可见） -->
            <div class="chunks-pager">
              <el-pagination
                v-model:current-page="chunkPage"
                v-model:page-size="chunkPageSize"
                :page-sizes="PAGE_SIZE_OPTIONS"
                :total="chunkTotalCount"
                layout="total, sizes, prev, pager, next, jumper"
                @size-change="handleChunkPageSizeChange"
                @current-change="fetchChunks"
              />
            </div>
          </div>

          <!-- 设置面板 -->
          <div v-if="activeTab === 'settings'" class="p-6 space-y-8">
            <!-- v1.1 §1.1: 可见范围 -->
            <div class="space-y-3">
              <div class="flex items-center justify-between">
                <div>
                  <h3 class="text-lg font-semibold text-slate-900 dark:text-slate-100">{{ $t('knowledge.visibility') }}</h3>
                  <p class="text-base text-slate-500 mt-1">{{ $t('knowledge.visibilityDesc') }}</p>
                </div>
                <button @click="handleSaveVisibility" :disabled="visibilitySaving"
                  class="btn btn-primary disabled:opacity-50">
                  <LoadingSpinner v-if="visibilitySaving" size="sm" />
                  <span v-else>{{ $t('common.save') }}</span>
                </button>
              </div>
              <div class="grid grid-cols-3 gap-3">
                <label v-for="opt in [
                  { val: 'private', label: $t('knowledge.visPrivate'), desc: $t('knowledge.visPrivateDesc') },
                  { val: 'public', label: $t('knowledge.visPublic'), desc: $t('knowledge.visPublicDesc') },
                  { val: 'restricted', label: $t('knowledge.visRestricted'), desc: $t('knowledge.visRestrictedDesc') },
                ]" :key="opt.val" :class="[
                  'cursor-pointer p-4 rounded-lg border-2 transition-all',
                  visibilityForm === opt.val
                    ? 'border-[var(--color-primary)]'
                    : 'border-[var(--border-color)] hover:border-[var(--color-primary)]'
                ]">
                  <input type="radio" v-model="visibilityForm" :value="opt.val" class="sr-only" />
                  <div class="font-medium text-slate-900 dark:text-slate-100">{{ opt.label }}</div>
                  <div class="text-sm text-slate-500 mt-1">{{ opt.desc }}</div>
                </label>
              </div>
            </div>

            <!-- v1.1 §1.1: 授权管理（仅 restricted 时显示） -->
            <div v-if="visibilityForm === 'restricted' || grants.length > 0"
              class="space-y-3 pt-6 border-t border-slate-200 dark:border-white/10">
              <div class="flex items-center justify-between">
                <div>
                  <h3 class="text-lg font-semibold text-slate-900 dark:text-slate-100">{{ $t('knowledge.grantManagement') }}</h3>
                  <p class="text-base text-slate-500 mt-1">{{ $t('knowledge.grantDesc') }}</p>
                </div>
                <button @click="showGrantModal = true" class="btn btn-primary">
                  <svg class="w-4 h-4 mr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
                  </svg>
                  {{ $t('knowledge.addGrant') }}
                </button>
              </div>

              <div v-if="grantsLoading" class="py-6">
                <LoadingSpinner size="sm" />
              </div>
              <div v-else-if="grants.length === 0"
                class="text-sm text-slate-500 py-4 text-center border border-dashed border-slate-200 dark:border-slate-700 rounded">
                {{ $t('knowledge.noGrantRecords') }}
              </div>
              <div v-else class="overflow-hidden rounded-lg border border-[var(--border-color)] dark:border-slate-700">
                <table class="w-full">
                  <thead class=" dark:bg-slate-800/50">
                    <tr>
                      <th class="px-3 py-2 text-left text-sm font-medium text-slate-500 uppercase">{{ $t('knowledge.detailType') }}</th>
                      <th class="px-3 py-2 text-left text-sm font-medium text-slate-500 uppercase">{{ $t('knowledge.subjectId') }}</th>
                      <th class="px-3 py-2 text-left text-sm font-medium text-slate-500 uppercase">{{ $t('knowledge.grantPermission') }}</th>
                      <th class="px-3 py-2 text-left text-sm font-medium text-slate-500 uppercase">{{ $t('knowledge.grantedBy') }}</th>
                      <th class="px-3 py-2 text-left text-sm font-medium text-slate-500 uppercase">{{ $t('knowledge.grantTime') }}</th>
                      <th class="px-3 py-2 text-right text-sm font-medium text-slate-500 uppercase">{{ $t('knowledge.action') }}</th>
                    </tr>
                  </thead>
                  <tbody class="divide-y divide-slate-200 dark:divide-slate-700">
                    <tr v-for="g in grants" :key="g.id" class="hover:bg-[var(--dropdown-hover-color)] dark:hover:bg-white/5">
                      <td class="px-3 py-2 text-base text-slate-500 font-mono">
                        <span class="badge badge-secondary">{{ subjectTypeLabel(g.subject_type) }}</span>
                      </td>
                      <td class="px-3 py-2 text-base font-mono text-slate-700 dark:text-slate-300">{{ g.subject_type === 'user' ? 'admin' : g.subject_id }}
                      </td>
                      <td class="px-3 py-2 text-base text-slate-500 font-mono ">
                        <span
                          :class="['inline-block px-2 py-0.5 text-xs rounded border  ']">
                          {{ permissionLabel(g.permission) }}
                        </span>
                      </td>
                      <td class="px-3 py-2 text-base text-slate-500 font-mono">{{ g.granted_by || '-' }}</td>
                      <td class="px-3 py-2 text-base text-slate-500 font-mono">{{ g.created_at?.slice(0, 19) }}</td>
                      <td class="px-3 py-2 text-right">
                        <el-popconfirm
                          :title="$t('knowledge.confirmRevokeGrant')"
                          :confirm-button-text="$t('knowledge.confirmDelete')"
                          :cancel-button-text="$t('common.cancel')"
                          @confirm="handleRevokeGrant(g.id)"
                        >
                          <template #reference>
                            <button
                              class="text-sm text-red-500 hover:text-red-400 px-2 py-1 rounded hover:bg-red-500/10">
                              {{ $t('knowledge.revokeGrant') }}
                            </button>
                          </template>
                        </el-popconfirm>
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>

            <!-- 危险操作 -->
            <div class="space-y-4 pt-6 border-t border-slate-200 dark:border-white/10">
              <h3 class="text-lg font-semibold text-warning">{{ $t('knowledge.dangerZone') }}</h3>
              <div class="p-4 border border-error/30 rounded-lg bg-error/5">
                <div class="flex justify-between items-center">
                  <div>
                    <div class="font-medium text-error">{{ $t('knowledge.deleteKb') }}</div>
                    <div class="text-sm text-slate-600 dark:text-dark-400 mt-1">{{ $t('knowledge.deleteKbDesc') }}</div>
                  </div>
                  <button @click="showDeleteKbModal = true" class="btn"
                    style="background:#dc2626;color:#fff;border-radius:4px;border-color:#dc2626;"
                    onmouseenter="this.style.background='#b91c1c'" onmouseleave="this.style.background='#dc2626'">
                    {{ $t('knowledge.deleteKb') }}
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </template>

    <!-- 编辑模态框 -->
    <Modal v-model="showEditModal" :title="$t('knowledge.editKb')" size="md" :close-on-overlay="false">
      <form @submit.prevent="handleUpdate" class="space-y-4">
        <div>
          <label class="block text-base font-bold  dark:text-slate-300 mb-2">{{ $t('knowledge.formName') }}</label>
          <input v-model="editForm.name" type="text" class="input" :placeholder="$t('knowledge.editNamePlaceholder')" />
        </div>
        <div>
          <label class="block text-base font-bold  dark:text-slate-300 mb-2">{{ $t('knowledge.formDesc') }}</label>
          <textarea v-model="editForm.description" class="input h-24 resize-none" :placeholder="$t('knowledge.editDescPlaceholder')"></textarea>
        </div>
        <div class="flex items-center gap-3 pt-0">
          <label class="relative inline-flex items-center cursor-pointer">
            <input v-model="editForm.is_public" type="checkbox" id="is_public" class="sr-only peer" />
            <div
              class="w-11 h-6 bg-[var(--border-color)] peer-focus:ring-0 peer-focus:ring-blue-500/30 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-[var(--color-primary)] ">
            </div>
            <span class="ml-3 text-sm  dark:text-slate-300">{{ $t('knowledge.publicKb') }}</span>
          </label>
        </div>
      </form>
      <template #footer>
        <button @click="showEditModal = false" class="btn btn-secondary">{{ $t('common.cancel') }}</button>
        <button @click="handleUpdate" :disabled="editLoading" class="btn btn-primary">
          <LoadingSpinner v-if="editLoading" size="sm" />
          <span v-else>{{ $t('common.save') }}</span>
        </button>
      </template>
    </Modal>

    <!-- 上传模态框 -->
    <Modal v-model="showUploadModal" :title="$t('knowledge.uploadFile')" size="md" :close-on-overlay="false">
      <div class="space-y-4">
        <div
          class="kb-upload-dropzone border-2 border-dashed rounded-xl p-8 text-center transition-all cursor-pointer group"
          :class="{ 'is-dragover': uploadDragOver }"
          @click="openUploadFilePicker"
          @dragenter="onUploadDragEnter"
          @dragover="onUploadDragOver"
          @dragleave="onUploadDragLeave"
          @drop="onUploadDrop"
        >
          <input
            ref="uploadFileInput"
            type="file"
            class="hidden"
            @change="onUploadFileInputChange"
            @click.stop
          />
          <div
            class="w-16 h-16 mx-auto bg-slate-100 dark:bg-slate-800 rounded-full flex items-center justify-center mb-4 group-hover:scale-110 transition-transform duration-300">
            <svg class="w-8 h-8 text-slate-400 group-hover:text-blue-400 transition-colors" fill="none"
              viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
            </svg>
          </div>
          <p v-if="!uploadFile" class="text-slate-400 group-hover: transition-colors">{{ $t('knowledge.dragOrClickToUpload') }}</p>
          <p v-else class="text-blue-400 font-medium bg-blue-500/10 py-1 px-3 rounded-full inline-block">{{
            uploadFile.name
          }}</p>
        </div>
        <div v-if="uploadLoading" class="space-y-2">
          <div class="progress">
            <div class="progress-bar" :style="{ width: uploadProgress + '%' }"></div>
          </div>
          <p class="text-sm text-center text-slate-400">{{ $t('knowledge.uploadProgress', { progress: uploadProgress }) }}</p>
        </div>
      </div>
      <template #footer>
        <button @click="showUploadModal = false" class="btn btn-secondary">{{ $t('common.cancel') }}</button>
        <button @click="handleUpload" :disabled="uploadLoading || !uploadFile" class="btn btn-primary">
          <LoadingSpinner v-if="uploadLoading" size="sm" />
          <span v-else>{{ $t('common.upload') }}</span>
        </button>
      </template>
    </Modal>

    <!-- QA对管理模态框 -->
    <ChunkQAModal v-model:show="showChunkQAModal" :chunk-id="selectedChunkId" />

    <!-- v1.1 §3.2: 清理无意义切片模态框 -->
    <Modal v-model="showCleanupModal" :title="$t('knowledge.cleanupMeaningless')" size="xl" :close-on-overlay="false">
      <div class="space-y-4">
        <div
          class="p-3 bg-blue-50 dark:bg-blue-500/10 border border-blue-200 dark:border-blue-500/30 rounded-lg text-sm  dark:text-slate-300">
          <strong>{{ $t('knowledge.cleanupRuleTitle') }}</strong>
          {{ $t('knowledge.cleanupRuleDesc') }}
          <code class="text-sm px-1 py-0.5  dark:bg-slate-800 rounded">## Page 13 ## Page 13</code>
          {{ $t('knowledge.cleanupRuleDetail') }}<strong>{{ $t('knowledge.cleanupRuleNote') }}</strong>
        </div>
        <div class="grid grid-cols-3 gap-4">
          <div>
            <label class="text-md  mb-2 block font-bold">{{ $t('knowledge.cleanupMinLen') }}</label>
            <div class="number-input-wrap">
              <input v-model.number="cleanupForm.min_len" type="number" min="1" max="500" />
              <div class="number-arrows">
                <button class="number-arrow number-arrow-up" type="button" @click="cleanupForm.min_len = Math.min(500, (cleanupForm.min_len || 1) + 1)">
                  <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round"><polyline points="18 15 12 9 6 15"></polyline></svg>
                </button>
                <button class="number-arrow number-arrow-down" type="button" @click="cleanupForm.min_len = Math.max(1, (cleanupForm.min_len || 1) - 1)">
                  <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"></polyline></svg>
                </button>
              </div>
            </div>
          </div>
          <div>
            <label class="text-md  mb-2 block font-bold">{{ $t('knowledge.cleanupScanLimit') }}</label>
            <div class="number-input-wrap">
              <input v-model.number="cleanupForm.scan_limit" type="number" min="100" max="100000" />
              <div class="number-arrows">
                <button class="number-arrow number-arrow-up" type="button" @click="cleanupForm.scan_limit = Math.min(100000, (cleanupForm.scan_limit || 100) + 100)">
                  <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round"><polyline points="18 15 12 9 6 15"></polyline></svg>
                </button>
                <button class="number-arrow number-arrow-down" type="button" @click="cleanupForm.scan_limit = Math.max(100, (cleanupForm.scan_limit || 100) - 100)">
                  <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"></polyline></svg>
                </button>
              </div>
            </div>
          </div>
          <div>
            <label class="text-md mb-2 block font-bold">{{ $t('knowledge.cleanupDeleteMode') }}</label>
            <el-select v-model="cleanupForm.mode" style="max-width: 200px; text-xs " class="cleanup-mode-select" :teleported="false">
              <el-option value="soft" :label="$t('knowledge.cleanupModeSoft')"></el-option>
              <el-option value="hard" :label="$t('knowledge.cleanupModeHard')"></el-option>
            </el-select>


          </div>
        </div>

        <div v-if="cleanupResult" class="space-y-3">
          <div class="flex items-center gap-4 p-3  ">
            <div class="text-md">{{ $t('knowledge.cleanupScanned', { count: cleanupResult.scanned }) }}</div>
            <div class="text-md">{{ $t('knowledge.cleanupMatched', { count: cleanupResult.matched }) }}</div>
          </div>
          <div v-if="cleanupResult.samples.length > 0">
            <div class="text-xs text-slate-500 mb-2">{{ $t('knowledge.cleanupSamples') }}</div>
            <div
              class="max-h-64 overflow-y-auto space-y-1.5 border border-slate-200 dark:border-slate-700 rounded p-2 bg-slate-50/50 dark:bg-slate-900/30">
              <div v-for="s in cleanupResult.samples" :key="s.id"
                class="text-xs font-mono bg-white dark:bg-slate-800 p-1.5 rounded border border-slate-200 dark:border-slate-700">
                <span class="text-slate-400">{{ s.id.slice(0, 8) }}...</span>
                <span class="ml-2 text-slate-700 dark:text-slate-300">{{ s.preview }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>
      <template #footer>
        <button @click="showCleanupModal = false" class="btn btn-secondary">{{ $t('common.close') }}</button>
        <button @click="previewCleanup" :disabled="cleanupLoading" class="btn btn-primary">
          <LoadingSpinner v-if="cleanupLoading" size="sm" />
          <span v-else>{{ $t('knowledge.cleanupPreview') }}</span>
        </button>
        <button @click="executeCleanup" :disabled="cleanupLoading || !cleanupResult || cleanupResult.matched === 0"
          class="btn btn-secondary">
          {{ $t('knowledge.cleanupConfirm') }} <span v-if="cleanupResult">({{ cleanupResult.matched }})</span>
        </button>
      </template>
    </Modal>
    <!-- 删除知识库确认弹窗 -->
    <Modal v-model="showDeleteKbModal" :title="$t('knowledge.deleteKb')" size="sm" :contentStyle="modalStyle" :close-on-overlay="false">
      <div class="space-y-4">
        <p class="text-sm">{{ $t('knowledge.confirmDeleteKbMsg', { name: knowledgeBase ? knowledgeBase.name : '' }) }}</p>
        <p class="text-sm text-red-500">{{ $t('knowledge.deleteKbWarning') }}</p>
      </div>
      <template #footer>
        <button @click="showDeleteKbModal = false" class="btn btn-secondary">{{ $t('common.cancel') }}</button>
        <button @click="handleConfirmDeleteKb" :disabled="isDeletingKb"
          class="btn btn-primary">
          <LoadingSpinner v-if="isDeletingKb" size="sm" />
          <span v-else>{{ $t('common.confirm') }}</span>
        </button>
      </template>
    </Modal>
    <!-- v1.1 §1.1: 新增授权 Modal -->
    <Modal v-model="showGrantModal" :title="$t('knowledge.addGrant')" size="md" :contentStyle="modalStyle" :close-on-overlay="false">
      <div class="space-y-4" >
        <div>
          <label class="block text-base font-medium dark:text-slate-300 mb-2">{{ $t('knowledge.subjectTypeLabel') }}</label>
          <div class="grid grid-cols-3 gap-2">
            <label v-for="t in ['user', 'role', 'dept']" :key="t" :class="[
              'cursor-pointer p-3 text-center rounded border-2 transition-all text-base',
              grantForm.subject_type === t
                ? 'border-[var(--color-primary)] dark:bg-blue-500/10  dark:text-blue-300'
                : 'border-[var(--border-color)] dark:border-slate-700 hover:border-slate-300'
            ]" @click="onGrantSubjectType(t as 'user' | 'role' | 'dept')">
              <input type="radio" :checked="grantForm.subject_type === t" class="sr-only" />
              {{ subjectTypeLabel(t) }}
            </label>
          </div>
        </div>
        <div>
          <label class="block text-base font-medium  dark:text-slate-300 mb-2">
            Subject ID
            <span class="text-base font-normal ml-2">{{ $t('knowledge.subjectIdHint') }}</span>
          </label>
          <div v-if="grantForm.subject_type === 'user'" class="input w-full">admin</div>
          <input v-else v-model="grantForm.subject_id" type="text" class="input w-full"
            :placeholder="$t('knowledge.subjectIdPlaceholder')" />
        </div>
        <div>
          <label class="block text-base font-medium dark:text-slate-300 mb-2">{{ $t('knowledge.grantPermission') }}</label>
          <div class="grid grid-cols-3 gap-2">
            <label v-for="p in ['read', 'write', 'admin']" :key="p" :class="[
              'cursor-pointer p-3 rounded border-2 transition-all text-base',
              grantForm.permission === p
                ? 'border-[var(--color-primary)]  dark:bg-blue-500/10'
                : 'border-[var(--border-color)] dark:border-slate-700 hover:border-slate-300'
            ]">
              <input type="radio" v-model="grantForm.permission" :value="p" class="sr-only" />
              <div class="font-medium  dark:text-slate-100">{{ permissionLabel(p) }}</div>
              <div class="text-xs text-slate-400 mt-0.5">
                {{ p === 'read' ? $t('knowledge.permDescRead') : p === 'write' ? $t('knowledge.permDescWrite') : $t('knowledge.permDescAdmin') }}
              </div>
            </label>
          </div>
        </div>
      </div>
      <template #footer>
        <button @click="showGrantModal = false" class="btn btn-secondary">{{ $t('common.cancel') }}</button>
        <button @click="handleAddGrant" :disabled="grantSubmitting || (grantForm.subject_type !== 'user' && !grantForm.subject_id.trim())"
          class="btn btn-primary disabled:opacity-50">
          <LoadingSpinner v-if="grantSubmitting" size="sm" />
          <span v-else>{{ $t('knowledge.confirmGrant') }}</span>
        </button>
      </template>
    </Modal>
  </div>
</template>
<style lang="scss" scoped>
.kb-detail-page {
  width: 100%;
  height: 100%;
  min-height: 0;
  margin: 0 auto;
  box-sizing: border-box;
  /* 四边等距，窗口缩放时底部间距与左右保持一致 */
  padding: 20px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  background: var(--bg-primary);

  > .flex.items-start,
  > .kb-detail-stats {
    flex-shrink: 0;
  }
}

.kb-embedded-chunk-viewer {
  flex: 1;
  min-height: 0;
  height: 100%;
  background: transparent;
}

.btn-secondary {
  //   flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0px;
  padding: 6px 16px;
  /* 与主题选择弹窗「恢复/取消」一致，不改尺寸 */
  color: var(--btn-secondary-text) !important;
  background: var(--cancel-btn-color) !important;
  border: 1px solid var(--btn-secondary-border) !important;
  border-radius: 4px;
  cursor: pointer;
  font-size: var(--font-size-md);
  font-weight: 500;
  transition: all 0.2s;
  width: 110px;
  outline: none;
  box-shadow: none !important;

  &:hover:not(:disabled) {
    background: var(--cancel-btn-hover-color) !important;
    color: var(--btn-secondary-hover-text) !important;
    border-color: var(--btn-secondary-hover-border) !important;
    transform: none;
    box-shadow: none !important;
  }

  &:focus,
  &:focus-visible {
    outline: none;
    box-shadow: none !important;
  }
}

.cleanup-btn {
  flex: none;
  width: 150px;
  background: #fa9a02 !important;
  color: var(--btn-primary-text, #fff) !important;
  border-color: transparent !important;
  border-radius: 4px;
  cursor: pointer;
  font-size: var(--font-size-md);
  font-weight: 500;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0px;
  padding: 6px 16px;
}

/* 覆盖 .btn-secondary:hover 的取消按钮样式，恢复清理按钮悬浮色 */
.btn-secondary.cleanup-btn:hover:not(:disabled),
.cleanup-btn:hover {
  background: #dfa13d !important;
  color: var(--btn-primary-hover-text, #fff) !important;
  border-color: transparent !important;
  transform: none;
  box-shadow: none !important;
}

/* 分页按钮样式 - 与编辑按钮一致 */
.pagination-btns .btn-primary {
  background: var(--btn-primary-bg, var(--color-primary)) !important;
  color: var(--btn-primary-text, #fff) !important;
  // border-color: var(--btn-primary-border, var(--color-primary)) !important;
  opacity: 1 !important;
}

.pagination-btns .btn-primary:hover:not(:disabled) {
  background: var(--btn-primary-hover-bg, var(--color-primary)) !important;
  color: var(--btn-primary-hover-text, #fff) !important;
  // border-color: var(--btn-primary-hover-border, var(--color-primary)) !important;
}

.btn-primary:disabled,
.btn-secondary:disabled {
  cursor: not-allowed !important;
}

.pagination-btns .btn-primary:disabled {
  background: var(--btn-primary-disabled-bg, #ccc) !important;
  color: var(--btn-primary-disabled-text, #666) !important;
  cursor: not-allowed;
}
.btn-primarydelete
{
  display: flex !important;
  align-items: center !important;
  justify-content: center !important;
  gap: 0px;
  padding: 6px 16px !important;
  background: var(--btn-primary-bg, var(--color-primary)) !important;
  color: var(--btn-primary-text, #fff) !important;
  border: 1px solid var(--btn-primary-border, transparent) !important;
  border-radius: 4px !important;
  cursor: pointer !important;
  font-size: var(--font-size-md) !important;
  font-weight: 500 !important;
  transition: all 0.2s !important;
  width: 110px !important;

  &:hover {
    background: var(--btn-primary-hover-bg, var(--color-primary)) !important;
    color: var(--btn-primary-hover-text, #fff) !important;
    border-color: var(--btn-primary-hover-border, transparent) !important;
  }
}
.btn-primary {
  // flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0px;
  padding: 6px 16px;
  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);
  border: 1px solid var(--btn-primary-border, transparent);
  border-radius: 4px;
  cursor: pointer;
  font-size: var(--font-size-md);
  font-weight: 500;
  transition: all 0.2s;
  width: 110px;

  &:hover {
    background: var(--btn-primary-hover-bg, var(--color-primary));
    color: var(--btn-primary-hover-text, #fff);
    border-color: var(--btn-primary-hover-border, transparent);
  }
}
.stat-card-icon
{
  width: 34px;
  height: 34px;
  border-radius: 8px;
  display: flex !important;
  align-items: center !important;
  justify-content: center !important;
  margin: 0 !important;
  flex-shrink: 0;
}

.stat-card-icon svg {
  display: block;
  flex-shrink: 0;
}

.kb-detail-stats {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 16px;
}

.stat-card {
  display: flex !important;
  flex-direction: row !important;
  align-items: center !important;
  gap: 14px;
  padding: 9px 10px;
  min-width: 0;
  box-sizing: border-box;
  white-space: nowrap;
  background: var(--background-color);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  transition: box-shadow 0.2s;

  &:hover {
    background: var(--background-color);
    box-shadow: 0 2px 12px rgba(0, 0, 0, 0.04);
  }
}

.card {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 18px 18px 5px 18px;
  background: transparent;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  transition: box-shadow 0.2s;
  flex: 1;
  min-height: 0;

  &:hover {
    box-shadow: 0 2px 12px rgba(0, 0, 0, 0.04);
  }

  .content-box1 {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    /* 垂直滚动条，内容超出才显示 */
    overflow-y: auto;
    .table-content {
      height: calc(100% - 36px - 16px);
      
    }
  }

  .tabs {
    background: transparent;
    margin-bottom: 0;
    flex-shrink: 0;
  }
}

.stat-card-value {
  font-size: var(--font-size-3xl);
  font-weight: 600;
  color: var(--text-primary);
  line-height: 1.2;
  margin: 0 !important;
  display: flex;
  align-items: center;
}

.stat-card-label {
  font-size: var(--font-size-lm);
  margin: 0 !important;
  display: flex;
  align-items: center;
  line-height: 1.2;
}
</style>

<style>
/* 统一所有文字颜色跟随主题 */
.space-y-6,
.space-y-6 * {
  color: var(--text-primary) ;
}

/* tabs 区域背景 */
.tabs {
  background: var(--bg-primary);
  margin-bottom: -20px;
}

/* 选中 tab：覆盖 .space-y-6 * 的强制主题色，否则字色=背景色看不见 */
.kb-detail-page .tabs .tab.tab-active,
.kb-detail-page .tabs .tab.border-b-2 {
  color: #ffffff !important;
}

/* tabs 鼠标悬浮效果（排除选中tab） */
.tabs .tab:hover:not(.border-b-2) {
  background: var(--bg-hover);
}

/* 基本信息/时间信息表格分割线 */
.space-y-6 .border-b {
  border-color: var(--chat-divider-color) !important;
}

/* 文件管理区域分割线 */
.space-y-4.border-t,
.space-y-3.border-t {
  border-color: var(--chat-divider-color) !important;
}

/* 错误提示区域边框 */
.border-error\/30 {
  border-color: var(--border-color) !important;
}

/* 虚线边框区域 */
.border-dashed {
  border-color: var(--border-color) !important;
}

.kb-upload-dropzone {
  border-color: color-mix(in srgb, var(--border-color) 80%, transparent) !important;
  background: transparent;
}
.kb-upload-dropzone:hover {
  border-color: color-mix(in srgb, var(--color-primary, #3b82f6) 50%, transparent) !important;
  background: color-mix(in srgb, var(--color-primary, #3b82f6) 5%, transparent);
}
.kb-upload-dropzone.is-dragover {
  border-color: var(--color-primary, #3b82f6) !important;
  background: color-mix(in srgb, var(--color-primary, #3b82f6) 10%, transparent);
}

/* badge 背景 */
.badge-success {
  background: var(--background-color) !important;
  border-color: var(--chat-divider-color) !important;
}

/* 文件列表"已完成"标签 */
.status-completed-tag.el-tag--success,
.status-completed-tag.el-tag--success .el-tag__content,
.status-completed-tag.el-tag--success * {
  --el-tag-bg-color: var(--el-color-success-light-8);
  --el-tag-border-color: var(--el-color-success-light-8);
  --el-tag-hover-color: var(--el-color-success);
  --el-tag-text-color: var(--el-color-success) !important;
  color: var(--el-color-success) !important;
  font-size: var(--font-size-md) !important;
}

/* 按钮文字和图标白色 */
/* .btn,
.btn *,
.btn svg {
  color: #fff !important;
} */

/* 浅色/雾灰调主题下选中tab文字白色 */
.light[data-theme-id="simple-light"] .tabs .border-b-2,
.light[data-theme-id="scene-graphite-cyan"] .tabs .border-b-2 {
  color: #fff !important;
}

/* 非 ghost 按钮图标白色；浅色/雾灰下 ghost 图标黑色 */
.btn:not(.btn-ghost) svg {
  color: #fff !important;
  stroke: #fff !important;
  fill: #fff !important;
}
.light[data-theme-id="simple-light"] .btn.btn-ghost svg,
.light[data-theme-id="scene-graphite-cyan"] .btn.btn-ghost svg {
  color: #000000 !important;
  stroke: #000000 !important;
  fill: none !important;
}

/* 暗色/情景主题下表格样式 */
.dark .overflow-hidden,
.dark .overflow-hidden table,
.dark .overflow-hidden th,
.dark .overflow-hidden td,
.dark .overflow-hidden tr,
.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .overflow-hidden,
.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .overflow-hidden table,
.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .overflow-hidden th,
.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .overflow-hidden td,
.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .overflow-hidden tr {
  border-color: var(--border-color) !important;
}

.dark .overflow-hidden tbody tr:hover td,
.dark .overflow-hidden tbody tr:hover,
.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .overflow-hidden tbody tr:hover td,
.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .overflow-hidden tbody tr:hover,
.dark .w-full tbody tr:hover td,
.dark .w-full tbody tr:hover,
.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .w-full tbody tr:hover td,
.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .w-full tbody tr:hover {
  background: rgba(255, 255, 255, 0.10) !important;
}

.table thead th,
.w-full thead th {
  font-size: var(--font-size-base) !important;
  background: rgba(255, 255, 255, 0.10) !important;
  border-bottom-color: rgba(255, 255, 255, 0.10) !important;
}
.table thead,
.w-full thead {
  position: relative;
  z-index: 1;
  background: var(--dropdown-hover-color)!important;
}
.table tbody td,
.w-full tbody td {
  font-size: var(--font-size-base) !important;
  border-bottom-color: rgba(255, 255, 255, 0.10) !important;
}

.table .btn-ghost {
  color: #ef4444 !important;
  background: transparent !important;
  border: none;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 4px;
  opacity: 1 !important;
}

.table .badge {
  font-size: var(--font-size-sm) !important;
}

.table .btn-ghost:hover {
  background: var(--dropdown-hover-color) !important;
}

.space-y-4,
.space-y-4 span {
  font-size: var(--font-size-md) !important;
}

.btn-primary,
.btn-primary * {
  color: #fff !important;
}

/* 切片卡片背景（仅切片 Tab，避免污染分块查看 ChunkViewer） */
.chunks-panel .chunk-card,
.chunks-tab-panel .chunk-card {
  background: var(--bg-primary) !important;
  border: none !important;
}

/* 暗色主题下切片卡片背景 */
.dark .chunks-panel .chunk-card,
.dark .chunks-tab-panel .chunk-card {
  background: rgba(255, 255, 255, 0.05) !important;
}

/* 情景主题（除雾灰调）下切片卡片背景 */
.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .chunks-panel .chunk-card,
.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .chunks-tab-panel .chunk-card {
  background: rgba(255, 255, 255, 0.10) !important;
}

/* 雾灰调主题下切片卡片背景 */
.light[data-theme-id="scene-graphite-cyan"] .chunks-panel .chunk-card,
.light[data-theme-id="scene-graphite-cyan"] .chunks-tab-panel .chunk-card {
  background: rgba(0, 0, 0, 0.05) !important;
}

/* 切片卡片内部区域背景透明 */
.chunks-panel .chunk-card .chunk-card-header,
.chunks-tab-panel .chunk-card .chunk-card-header,
.chunks-panel .chunk-card p.text-base.text-slate-700.dark\:text-slate-300.line-clamp-3.font-mono.leading-relaxed,
.chunks-tab-panel .chunk-card p.text-base.text-slate-700.dark\:text-slate-300.line-clamp-3.font-mono.leading-relaxed {
  background: transparent !important;
}

/* 切片面板背景透明 */
/* .chunks-panel {
  background: transparent !important;
} */

/* 切片列表容器背景透明 */
.chunks-panel .space-y-4 {
  background: transparent !important;
}

/* 强制所有切片相关元素背景透明 */
.chunks-panel *,
.chunks-panel *::before,
.chunks-panel *::after {
  background-color: transparent !important;
}

/* 切片卡片恢复背景色 */
.chunks-panel .chunk-card,
.chunks-panel .chunk-card * {
   background-color: rgba(255, 255, 255, 0.10) !important;
}

/* 暗色主题下切片卡片 */
.dark .chunks-panel .chunk-card,
.dark .chunks-panel .chunk-card * {
 background-color: rgba(255, 255, 255, 0.05) !important;
}

/* 情景主题（除雾灰调）下切片卡片 */
/* .light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .chunks-panel .chunk-card,
.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .chunks-panel .chunk-card * {
  background-color: rgba(255, 255, 255, 0.10) !important;
} */

/* 雾灰调主题下切片卡片 */
/* .light[data-theme-id="scene-graphite-cyan"] .chunks-panel .chunk-card,
.light[data-theme-id="scene-graphite-cyan"] .chunks-panel .chunk-card * {
  background-color: rgba(0, 0, 0, 0.05) !important;
} */
 
.light[data-theme-id="simple-light"] .chunks-panel .chunk-card,
.light[data-theme-id="simple-light"] .chunks-panel .chunk-card * {
  background-color: #fff !important;

}
/* ===== 文件列表面板 ===== */
.files-tab-panel {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.files-header {
  flex-shrink: 0;
  padding: 16px 24px 12px;
}

/* 文件表：表头固定，仅表体滚动（与全部工具一致） */
.files-table-card {
  flex: 1;
  min-height: 0;
  margin: 0 24px;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  background: transparent;
}

.files-table-head {
  flex-shrink: 0;
  background: var(--table-header-bg-color);
  overflow: hidden;
  padding-right: 6px;
  box-sizing: border-box;
}

.files-table-body-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--scrollbar-thumb, rgba(0, 0, 0, 0.15)) transparent;
  padding-bottom: 2px;
}

.files-table-body-scroll::-webkit-scrollbar { width: 6px; height: 6px; }
.files-table-body-scroll::-webkit-scrollbar-track { background: transparent; }
.files-table-body-scroll::-webkit-scrollbar-thumb {
  background: var(--scrollbar-thumb, rgba(0, 0, 0, 0.15));
  border-radius: 3px;
}

.files-table-card .files-table {
  width: 100%;
  min-width: 680px;
  table-layout: fixed;
  border-collapse: collapse;
  font-size: var(--font-size-base) !important;
  color: var(--text-primary) !important;
  background: transparent !important;
}

.files-table .col-name { width: 22%; }
.files-table .col-type { width: 12%; }
.files-table .col-size { width: 12%; }
.files-table .col-status { width: 22%; }
.files-table .col-time { width: 20%; }
.files-table .col-action { width: 12%; }

.files-table-card .files-table thead th {
  background: var(--table-header-bg-color) !important;
  font-weight: 600 !important;
  font-size: var(--font-size-base) !important;
  color: var(--text-primary) !important;
  padding: 8px 12px !important;
  text-align: left;
  border: none !important;
  white-space: nowrap;
}

.files-table-card .files-table thead th.text-center { text-align: center; }
.files-table-card .files-table thead th.text-right { text-align: right; }

.files-table-card .files-table tbody td {
  padding: 8px 12px !important;
  border: none !important;
  color: var(--text-primary) !important;
  background-color: transparent !important;
  vertical-align: middle;
  overflow: hidden;
  text-overflow: ellipsis;
}

.files-table-card .files-table tbody td.is-center {
  text-align: center;
}

.files-table-card .files-table tbody td.file-status-cell {
  text-align: center;
  overflow: visible;
  text-overflow: unset;
  white-space: normal;
  vertical-align: middle;
}

.files-table-card .files-table tbody td.file-status-cell > * {
  margin-left: auto;
  margin-right: auto;
}

.file-vector-status {
  display: flex;
  flex-direction: column;
  gap: 4px;
  width: 150px;
  padding: 2px 0;
  text-align: left;
}

.file-vector-status__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  font-size: 12px;
  line-height: 1.2;
}

.file-vector-status__label {
  color: var(--color-primary, #3b82f6);
  font-weight: 500;
}

.file-vector-status__pct {
  color: var(--text-secondary, #64748b);
  flex-shrink: 0;
}

.file-vector-status__track {
  height: 6px;
  width: 100%;
  background: var(--border-color, #e2e8f0);
  border-radius: 999px;
  overflow: hidden;
}

.file-vector-status__bar {
  height: 100%;
  border-radius: 999px;
  transition: width 0.5s ease;
}

.file-vector-bar--running {
  background: var(--color-primary, #3b82f6);
}

.file-vector-bar--pending {
  background: #f59e0b;
}

.file-vector-bar--completed {
  background: #10b981;
}

.file-vector-bar--failed {
  background: #ef4444;
}

.file-vector-bar--cancelled {
  background: var(--text-secondary, #94a3b8);
}

/* 情景主题（除雾灰调，含 custom-scene）：处理中白字，进度条主题色 */
:is(
  html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]),
  html.light[data-theme-id="custom-scene"],
  html[data-category="scene"]:not([data-theme-id="scene-graphite-cyan"])
) .file-status-cell .file-vector-status__label {
  color: #ffffff !important;
}

:is(
  html.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]),
  html.light[data-theme-id="custom-scene"],
  html[data-category="scene"]:not([data-theme-id="scene-graphite-cyan"])
) .file-status-cell .file-vector-bar--running {
  background: var(--color-primary) !important;
}

.file-vector-status__msg {
  display: block;
  width: 100%;
  font-size: 12px;
  line-height: 1.2;
  color: var(--text-secondary, #94a3b8);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
}

.file-status-cell .badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 48px;
}

.file-status-cell .badge-danger {
  background: #fef2f2 !important;
  color: #b91c1c !important;
  border-color: #fecaca !important;
}

/* 黑暗主题 & 情景主题（除雾灰调）：在现有浅红底上加 65% 不透明度，不换色 */
.dark .file-status-cell .badge-danger,
.light[data-theme-id^="scene-"]:not([data-theme-id="scene-graphite-cyan"]) .file-status-cell .badge-danger {
  background: rgba(254, 242, 242, 0.65) !important; /* #fef2f2 @ 65% */
  color: #b91c1c !important;
  border-color: rgba(254, 202, 202, 0.65) !important; /* #fecaca @ 65% */
}

.file-status-cell .badge-success {
  background: #ecfdf5 !important;
  color: #047857 !important;
  border-color: #a7f3d0 !important;
}

.file-status-cell .badge-gray {
  background: #f1f5f9 !important;
  color: #475569 !important;
  border-color: #e2e8f0 !important;
}

.files-table-card .files-table tbody td.is-center > * {
  margin-left: auto;
  margin-right: auto;
}

.files-table-card .files-table tbody td.text-right {
  text-align: right;
}

.file-row-actions {
  display: inline-flex;
  align-items: center;
  justify-content: flex-end;
  gap: 4px;
}

.file-view-chunks-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-secondary, #64748b);
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.file-view-chunks-btn:hover {
  background: var(--dropdown-hover-color);
  color: var(--text-primary, #1a1a1a);
}

.files-table-card .files-table tbody tr:nth-child(even) td {
  background-color: var(--card-background-tmd) !important;
}

.files-table-card .files-table tbody tr:hover td {
  background-color: var(--dropdown-hover-color) !important;
}

.files-table-card .files-table tbody tr:nth-child(even):hover td {
  background-color: var(--dropdown-hover-color) !important;
}

.files-pager {
  flex-shrink: 0;
  padding: 10px 24px;
  border-top: 1px solid var(--border-color, rgba(0, 0, 0, 0.08));
}

/* ===== 切片面板 ===== */
.chunks-tab-panel {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.chunks-toolbar {
  flex-shrink: 0;
  padding: 10px 24px;
  border-bottom: 1px solid var(--border-color, rgba(0, 0, 0, 0.08));
}

.chunks-scroller {
  flex: 1;
  overflow-y: auto;
  padding: 10px 16px;
  border: none !important;
}

.chunks-pager {
  flex-shrink: 0;
  padding: 10px 24px;
  border-top: 1px solid var(--border-color, rgba(0, 0, 0, 0.08));
}

/* 分页按钮背景透明 */
.el-pagination .btn-prev,
.el-pagination .btn-next,
.el-pagination .el-pager li {
  background: transparent !important;
}

/* 当前页 / 上一页 / 下一页：固定 30×30 */
.el-pagination .btn-prev,
.el-pagination .btn-next,
.el-pagination .el-pager li.is-active {
  width: 30px !important;
  height: 30px !important;
  min-width: 30px !important;
  padding: 0 !important;
  margin: 0 4px !important;
  box-sizing: border-box !important;
  display: inline-flex !important;
  align-items: center !important;
  justify-content: center !important;
}

/* 上一页 / 下一页：边框 + 悬停背景；内部图标强制居中 */
.el-pagination .btn-prev,
.el-pagination .btn-next {
  border: 1px solid var(--border-color) !important;
  border-radius: 4px !important;
  line-height: 1 !important;
  font-size: 0 !important; /* 避免文字基线把图标顶偏 */
}
.el-pagination .btn-prev .el-icon,
.el-pagination .btn-next .el-icon {
  margin: 0 !important;
  width: 14px !important;
  height: 14px !important;
  display: inline-flex !important;
  align-items: center !important;
  justify-content: center !important;
}
.el-pagination .btn-prev .el-icon svg,
.el-pagination .btn-next .el-icon svg {
  width: 14px !important;
  height: 14px !important;
  display: block !important;
}
.el-pagination .btn-prev:hover:not(:disabled),
.el-pagination .btn-next:hover:not(:disabled) {
  background: var(--dropdown-hover-color) !important;
}

/* 当前页：主题色背景 + 白字 */
.el-pagination .el-pager li.is-active {
  background: var(--color-primary) !important;
  color: #fff !important;
  border-radius: 4px !important;
}
.el-pagination .el-pager li.is-active:hover {
  color: #fff !important;
}

/* 普通页码：悬停背景，文字颜色不变 */
.el-pagination .el-pager li:not(.is-active):hover {
  background: var(--dropdown-hover-color) !important;
  color: var(--text-primary) !important;
}

/* 清理弹窗下拉框高度与输入框一致 */
.number-input-wrap {
  position: relative;
  display: flex;
  align-items: center;
  width: 100%;

  input[type="number"] {
    width: 100%;
    height: 38px;
    padding: 0 28px 0 12px;
    border: 1px solid var(--border-color);
    border-radius: 6px;
    background: transparent;
    color: var(--text-primary);
    font-size: var(--font-size-base);
    outline: none;
    -moz-appearance: textfield;

    &::-webkit-inner-spin-button,
    &::-webkit-outer-spin-button {
      -webkit-appearance: none;
      margin: 0;
    }

    &:focus {
      border-color: var(--color-primary);
    }
  }

  .number-arrows {
    position: absolute;
    right: 4px;
    top: 0;
    bottom: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 0;
    opacity: 0;
    transition: opacity 0.15s;
  }

  &:hover .number-arrows {
    opacity: 1;
  }

  .number-arrow {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 16px;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;

    &:hover {
      color: var(--text-primary);
    }
  }
}

.cleanup-mode-select .el-select__wrapper {
  min-height: 36px!important;
  background: transparent !important;
}

/* 清理弹窗下拉框不被遮罩层挡住 */
.cleanup-mode-popper {
  z-index: 9999 !important;
}



.filter-check {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  user-select: none;
}
/* 隐藏原生勾选框 */
.filter-check input[type="checkbox"] {
  display: none;
}
/* 自定义方块基础样式 */
.check-box {
  width: 15px;
  height: 15px;
  border: 2px solid var(--text-secondary);
  border-radius: 3px;
  position: relative;
  transition: all 0.2s;
}
/* 勾选后的背景+对勾 */
.filter-check input[type="checkbox"]:checked + .check-box {
  background-color: var(--color-primary) !important; /* 主题蓝，可自行改色 */
  border:none;
}
/* 绘制对勾 */
.filter-check input[type="checkbox"]:checked + .check-box::after {
  content: "";
  position: absolute;
  left: 6px;
  top: 3px;
  width: 4px;
  height: 8px;
  border-width: 0 2px 2px 0;
  transform: rotate(45deg);
  color:white;
}
/* hover效果 */
.check-box:hover {
  border-color: var(--color-primary) !important;
}


</style>

<style>
/* 文件列表表格 */
.files-table-card .files-table .btn-ghost {
  color: #ef4444 !important;
  background: transparent !important;
  border: none;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 4px;
  opacity: 1 !important;
}

.files-table-card .files-table .btn-ghost:hover {
  background: var(--dropdown-hover-color) !important;
}

.files-table-card .files-table .badge {
  font-size: var(--font-size-sm) !important;
}

.badge.badge-info {
  border-color: var(--border-color) !important;
}

/* 文件分页下拉框、输入框：透明背景 + --border-color 边框 */
.files-pager .el-select__wrapper,
.files-pager .el-input__wrapper,
.files-pager .el-pagination__editor .el-input__wrapper {
  background: transparent !important;
  border: 1px solid var(--border-color) !important;
  box-shadow: none !important;
}

/* 删除确认弹窗：加宽 + 取消 / 确认同一行 */
.kb-file-delete-popconfirm {
  min-width: 320px !important;
}
.kb-file-delete-popconfirm .el-popconfirm__main {
  white-space: normal;
  word-break: break-word;
}
.kb-file-delete-popconfirm .el-popconfirm__action {
  display: flex !important;
  flex-direction: row !important;
  flex-wrap: nowrap !important;
  justify-content: flex-end;
  align-items: center;
  gap: 8px;
}
.kb-file-delete-popconfirm .el-popconfirm__action .el-button {
  margin: 0 !important;
  flex-shrink: 0;
}
</style>
