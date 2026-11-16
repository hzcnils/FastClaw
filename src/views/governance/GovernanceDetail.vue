<script setup lang="ts">
import { ref, onMounted, onActivated, computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useI18n } from 'vue-i18n';
import { governanceApi, kgServiceReady, ensureKgReady } from '@/api/knowledge';
import type { GovernanceDoc } from '@/types/knowledge';
import { useUIStore } from '@/stores/index';
import LoadingSpinner from '@/components/common/LoadingSpinner.vue';
import MarkdownRenderer from '@/components/markdown/MarkdownRenderer.vue';
import Modal from '@/components/common/Modal.vue';
import { useDialog } from '@/composables/useDialog';

const { t } = useI18n();
const { confirm: confirmDialog } = useDialog();

const route = useRoute();
const router = useRouter();
const uiStore = useUIStore();

const document = ref<GovernanceDoc | null>(null);
const loading = ref(true);
const saving = ref(false);
const isEditing = ref(false);
const editContent = ref('');
const changeSummary = ref('');
const showPreview = ref(true);

const activeTab = ref<'preview' | 'json' | 'assets' | 'source'>('preview');

// Assets
const assets = ref<any[]>([]);
const assetsLoading = ref(false);
const assetFilter = ref<'all' | 'image' | 'table' | 'formula'>('all');
const editingAsset = ref<any>(null);
const showAssetEditModal = ref(false);
const assetEditValue = ref('');

const id = computed(() => route.params.id as string);

const filteredAssets = computed(() => {
  if (assetFilter.value === 'all') return assets.value;
  if (assetFilter.value === 'image') return assets.value.filter(a => a.element_type === 'image' || a.element_type === 'page_image');
  return assets.value.filter(a => a.element_type === assetFilter.value);
});

const assetCounts = computed(() => {
  const c = { all: assets.value.length, image: 0, table: 0, formula: 0 };
  assets.value.forEach(a => {
    if (a.element_type === 'image' || a.element_type === 'page_image') c.image++;
    else if (a.element_type === 'table') c.table++;
    else if (a.element_type === 'formula') c.formula++;
  });
  return c;
});

async function fetchDocument() {
  loading.value = true;
  try {
    document.value = await governanceApi.get(id.value);
    editContent.value = document.value.md_content || '';
  } catch (e) {
    uiStore.showToast('error', e instanceof Error ? e.message : '获取文档详情失败');
    router.push('/governance');
  } finally { loading.value = false; }
}

async function fetchAssets() {
  assetsLoading.value = true;
  try {
    const result = await governanceApi.getAssets(id.value);
    assets.value = result || [];
  } catch { assets.value = []; }
  finally { assetsLoading.value = false; }
}

async function handleSave() {
  if (!editContent.value.trim()) { uiStore.showToast('warning', '内容不能为空'); return; }
  saving.value = true;
  try {
    await governanceApi.update(id.value, { md_content: editContent.value, change_summary: changeSummary.value || undefined });
    uiStore.showToast('success', '保存成功');
    isEditing.value = false; changeSummary.value = '';
    fetchDocument();
  } catch (e) { uiStore.showToast('error', e instanceof Error ? e.message : '保存失败'); }
  finally { saving.value = false; }
}

async function handleVerify() {
  if (!await confirmDialog('确定标记此文档为已验证吗？')) return;
  try { await governanceApi.verify(id.value); uiStore.showToast('success', '验证成功'); fetchDocument(); }
  catch (e) { uiStore.showToast('error', e instanceof Error ? e.message : '验证失败'); }
}

async function handleRevectorize() {
  if (!await confirmDialog('修改后重新向量化才能在搜索中生效，确定继续？')) return;
  try { await governanceApi.revectorize(id.value); uiStore.showToast('success', '重新向量化任务已提交'); }
  catch (e) { uiStore.showToast('error', e instanceof Error ? e.message : '操作失败'); }
}

function startEditing() { editContent.value = document.value?.md_content || ''; isEditing.value = true; }
async function cancelEditing() {
  if (editContent.value !== document.value?.md_content && !await confirmDialog('有未保存的更改，确定取消？')) return;
  isEditing.value = false; editContent.value = document.value?.md_content || ''; changeSummary.value = '';
}

function openAssetEdit(asset: any) {
  editingAsset.value = asset;
  assetEditValue.value = asset.element_type === 'table' ? (asset.table_data || '') : (asset.content_text || '');
  showAssetEditModal.value = true;
}

async function saveAssetEdit() {
  if (!editingAsset.value) return;
  try {
    const data: any = {};
    if (editingAsset.value.element_type === 'table') data.table_data = assetEditValue.value;
    else data.content_text = assetEditValue.value;
    await governanceApi.updateAsset(editingAsset.value.id, data);
    uiStore.showToast('success', '素材已更新'); showAssetEditModal.value = false; fetchAssets();
  } catch (e) { uiStore.showToast('error', e instanceof Error ? e.message : '更新失败'); }
}

function getAssetUrl(asset: any): string {
  if (asset.asset_path.startsWith('/')) return `/api/v1${asset.asset_path}`;
  return `/api/v1/assets/${asset.asset_path}`;
}

function formatDate(dateStr: string): string { return new Date(dateStr).toLocaleString('zh-CN'); }

function getElementTypeLabel(t: string): string {
  return { page_image: '📄 页面', image: '🖼️ 图片', table: '📊 表格', formula: '📐 公式' }[t] || t;
}

function formatProcessLog(): string {
  try { return JSON.stringify(JSON.parse(document.value?.process_log || '{}'), null, 2); }
  catch { return document.value?.process_log || '无数据'; }
}

async function loadData() {
  await ensureKgReady({ startIfNeeded: false });
  if (!kgServiceReady.value) return;
  fetchDocument();
  fetchAssets();
}

onMounted(loadData);
onActivated(loadData);
</script>

<template>
  <div class="space-y-6">
    <div v-if="loading" class="flex items-center justify-center min-h-96"><LoadingSpinner size="lg" :text="t('common.loading')" /></div>

    <template v-else-if="document">
      <!-- Header -->
      <div class="flex items-start justify-between">
        <div class="flex items-center gap-4">
          <button @click="router.push('/governance')" class="p-2 rounded-lg hover:bg-dark-700 transition-colors">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" /></svg>
          </button>
          <div>
            <h1 class="text-2xl font-bold">{{ document.title || '未命名文档' }}</h1>
            <div class="flex items-center gap-3 mt-2">
              <span class="badge badge-secondary">v{{ document.version }}</span>
              <span v-if="document.is_human_verified" class="badge badge-success">已验证</span>
              <span v-else class="badge badge-warning">待验证</span>
              <span class="text-sm text-dark-400">{{ document.word_count.toLocaleString() }} 字</span>
              <span v-if="document.process_method" class="text-xs text-dark-500">{{ document.process_method }}</span>
            </div>
          </div>
        </div>
        <div class="flex items-center gap-3">
          <button v-if="!document.is_human_verified && !isEditing" @click="handleVerify" class="btn btn-secondary">标记已验证</button>
          <button v-if="!isEditing" @click="handleRevectorize" class="btn btn-secondary">重新向量化</button>
          <button v-if="!isEditing" @click="startEditing" class="btn btn-primary">编辑</button>
          <template v-if="isEditing">
            <button @click="cancelEditing" class="btn btn-secondary">取消</button>
            <button @click="handleSave" :disabled="saving" class="btn btn-primary">
              <LoadingSpinner v-if="saving" size="sm" /><span v-else>保存</span>
            </button>
          </template>
        </div>
      </div>

      <!-- Stats -->
      <div class="grid grid-cols-4 gap-4">
        <div class="stat-card"><div class="stat-card-icon bg-primary/20 text-primary"><svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" /></svg></div><div><div class="stat-card-value">{{ document.word_count.toLocaleString() }}</div><div class="stat-card-label">总字数</div></div></div>
        <div class="stat-card"><div class="stat-card-icon bg-ocean/20 text-ocean"><svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" /></svg></div><div><div class="stat-card-value">{{ assetCounts.image }}</div><div class="stat-card-label">图片</div></div></div>
        <div class="stat-card"><div class="stat-card-icon bg-success/20 text-success"><svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 10h18M3 14h18m-9-4v8m-7 0h14a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z" /></svg></div><div><div class="stat-card-value">{{ assetCounts.table }}</div><div class="stat-card-label">表格</div></div></div>
        <div class="stat-card"><div class="stat-card-icon bg-warning/20 text-warning"><svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 7h6m0 10v-3m-3 3h.01M9 17h.01M9 14h.01M12 14h.01M15 11h.01M12 11h.01M9 11h.01M7 21h10a2 2 0 002-2V5a2 2 0 00-2-2H7a2 2 0 00-2 2v14a2 2 0 002 2z" /></svg></div><div><div class="stat-card-value">{{ assetCounts.formula }}</div><div class="stat-card-label">公式</div></div></div>
      </div>

      <!-- Edit mode -->
      <div v-if="isEditing" class="card">
        <div class="flex justify-between items-center p-4 border-b border-dark-700">
          <h3 class="text-lg font-semibold">编辑内容</h3>
          <div class="flex items-center gap-2">
            <button @click="showPreview = false" :class="['btn btn-ghost btn-sm', { 'bg-dark-700': !showPreview }]">编辑</button>
            <button @click="showPreview = true" :class="['btn btn-ghost btn-sm', { 'bg-dark-700': showPreview }]">预览</button>
          </div>
        </div>
        <div class="grid grid-cols-2 divide-x divide-dark-700" :class="{ 'grid-cols-1': !showPreview }">
          <div class="p-4"><textarea v-model="editContent" class="w-full h-[600px] bg-dark-900 border border-dark-700 rounded-lg p-4 font-mono text-sm resize-none focus:outline-none focus:border-primary" placeholder="Markdown..."></textarea></div>
          <div v-if="showPreview" class="p-4 overflow-auto max-h-[650px]"><MarkdownRenderer :content="editContent" /></div>
        </div>
        <div class="p-4 border-t border-dark-700">
          <label class="label">变更说明</label>
          <input v-model="changeSummary" type="text" class="input" placeholder="描述修改..." />
        </div>
      </div>

      <!-- View mode tabs -->
      <div v-else class="card">
        <div class="flex items-center gap-1 p-4 border-b border-dark-700">
          <button v-for="tab in [{key:'preview',label:'👁 预览'},{key:'json',label:'</> JSON'},{key:'assets',label:'🖼 资产库'},{key:'source',label:'📄 源码'}]" :key="tab.key"
            @click="activeTab = tab.key as any"
            :class="['px-4 py-2 text-sm rounded-lg transition-colors', activeTab === tab.key ? 'bg-primary text-white' : 'text-dark-400 hover:text-white hover:bg-dark-700']">
            {{ tab.label }}
          </button>
        </div>

        <div v-if="activeTab === 'preview'" class="p-6 overflow-auto max-h-[700px]">
          <MarkdownRenderer v-if="document.md_content" :content="document.md_content" />
          <div v-else class="text-center text-dark-400 py-12">暂无内容</div>
        </div>

        <div v-if="activeTab === 'json'" class="p-6 overflow-auto max-h-[700px]">
          <pre v-if="document.process_log" class="bg-dark-900 rounded-lg p-4 text-xs font-mono text-slate-300 whitespace-pre-wrap">{{ formatProcessLog() }}</pre>
          <div v-else class="text-center text-dark-400 py-12">无 WSP 解析结果</div>
        </div>

        <div v-if="activeTab === 'assets'" class="p-6">
          <div class="flex items-center gap-3 mb-6">
            <button v-for="f in [{key:'all',label:'📁 全部',count:assetCounts.all},{key:'image',label:'🖼 图片',count:assetCounts.image},{key:'table',label:'📊 表格',count:assetCounts.table},{key:'formula',label:'√x 公式',count:assetCounts.formula}]" :key="f.key"
              @click="assetFilter = f.key as any"
              :class="['px-4 py-2 rounded-lg text-sm flex items-center gap-2 transition-colors', assetFilter === f.key ? 'bg-primary text-white' : 'bg-dark-700 text-dark-300 hover:bg-dark-600']">
              {{ f.label }} <span class="bg-black/20 px-1.5 py-0.5 rounded text-xs">{{ f.count }}</span>
            </button>
            <span class="ml-auto text-sm text-dark-400">共 {{ filteredAssets.length }} 个</span>
          </div>

          <LoadingSpinner v-if="assetsLoading" text="加载..." />
          <div v-else-if="filteredAssets.length === 0" class="text-center py-16">
            <div class="w-20 h-20 mx-auto mb-4 bg-dark-700 rounded-2xl flex items-center justify-center opacity-40">
              <svg class="w-10 h-10" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" /></svg>
            </div>
            <p class="text-dark-400">暂无资产</p>
          </div>

          <div v-else class="grid grid-cols-2 md:grid-cols-3 gap-4">
            <div v-for="asset in filteredAssets" :key="asset.id" class="border border-dark-700 rounded-xl overflow-hidden hover:border-primary/50 transition-colors">
              <div class="aspect-video bg-dark-900 flex items-center justify-center overflow-hidden">
                <div v-if="asset.element_type === 'table' && asset.table_data" v-html="asset.table_data" class="text-xs p-2 [&_table]:w-full [&_td]:border [&_td]:border-dark-600 [&_td]:px-1 [&_th]:border [&_th]:border-dark-600 [&_th]:px-1 [&_th]:bg-dark-700 overflow-auto max-h-full w-full"></div>
                <code v-else-if="asset.element_type === 'formula' && asset.content_text" class="text-sm text-blue-300 p-4 break-all">{{ asset.content_text }}</code>
                <img v-else :src="getAssetUrl(asset)" :alt="asset.caption || ''" class="max-w-full max-h-full object-contain" loading="lazy" />
              </div>
              <div class="p-3 bg-dark-800/50 flex items-center justify-between">
                <div>
                  <span class="text-xs text-dark-400">{{ getElementTypeLabel(asset.element_type) }}</span>
                  <span class="text-xs text-dark-500 ml-2">P{{ asset.page_number + 1 }}</span>
                </div>
                <button v-if="asset.element_type === 'table' || asset.element_type === 'formula'" @click="openAssetEdit(asset)" class="text-xs text-primary hover:underline">编辑</button>
              </div>
            </div>
          </div>
        </div>

        <div v-if="activeTab === 'source'" class="p-6 overflow-auto max-h-[700px]">
          <pre class="bg-dark-900 rounded-lg p-4 text-sm font-mono text-slate-300 whitespace-pre-wrap">{{ document.md_content }}</pre>
        </div>
      </div>

      <!-- Detail info -->
      <div class="card p-6">
        <h3 class="text-lg font-semibold mb-4">详细信息</h3>
        <div class="grid grid-cols-2 gap-6">
          <div class="space-y-3">
            <div class="flex justify-between py-2 border-b border-dark-700"><span class="text-dark-400">文档 ID</span><span class="font-mono text-sm">{{ document.id }}</span></div>
            <div class="flex justify-between py-2 border-b border-dark-700"><span class="text-dark-400">知识库 ID</span><span class="font-mono text-sm">{{ document.knowledge_base_id || '-' }}</span></div>
            <div class="flex justify-between py-2 border-b border-dark-700"><span class="text-dark-400">处理方式</span><span>{{ document.process_method || '-' }}</span></div>
            <div class="flex justify-between py-2 border-b border-dark-700"><span class="text-dark-400">资产目录</span><span class="font-mono text-sm">{{ document.assets_folder || '-' }}</span></div>
          </div>
          <div class="space-y-3">
            <div class="flex justify-between py-2 border-b border-dark-700"><span class="text-dark-400">创建时间</span><span>{{ formatDate(document.created_at) }}</span></div>
            <div class="flex justify-between py-2 border-b border-dark-700"><span class="text-dark-400">更新时间</span><span>{{ formatDate(document.updated_at) }}</span></div>
            <div v-if="document.verified_at" class="flex justify-between py-2 border-b border-dark-700"><span class="text-dark-400">验证时间</span><span>{{ formatDate(document.verified_at) }}</span></div>
          </div>
        </div>
      </div>
    </template>

    <!-- Asset Edit Modal -->
    <Modal :show="showAssetEditModal" :title="editingAsset?.element_type === 'table' ? '编辑表格 HTML' : '编辑公式 LaTeX'" size="lg" @close="showAssetEditModal = false">
      <div class="space-y-4">
        <textarea v-model="assetEditValue" rows="15" class="w-full bg-dark-900 border border-dark-700 rounded-lg p-4 font-mono text-sm text-slate-300 resize-none focus:outline-none focus:border-primary"></textarea>
        <div v-if="editingAsset?.element_type === 'table' && assetEditValue" class="border border-dark-700 rounded-lg p-4 overflow-auto max-h-60">
          <p class="text-xs text-dark-400 mb-2">预览：</p>
          <div v-html="assetEditValue" class="text-sm [&_table]:w-full [&_td]:border [&_td]:border-dark-600 [&_td]:px-2 [&_td]:py-1 [&_th]:border [&_th]:border-dark-600 [&_th]:px-2 [&_th]:py-1 [&_th]:bg-dark-700"></div>
        </div>
      </div>
      <template #footer>
        <button @click="showAssetEditModal = false" class="btn btn-secondary">取消</button>
        <button @click="saveAssetEdit" class="btn btn-primary">保存</button>
      </template>
    </Modal>
  </div>
</template>
