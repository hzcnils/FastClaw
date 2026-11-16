<template>
  <div class="role-list">
    <!-- 统计卡片 -->
    <div class="stats-row">
      <div class="stat-card">
        <div class="stat-value">{{ store.stats.total }}</div>
        <div class="stat-label">{{ $t('assistant.totalRoles') }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-value">{{ store.stats.published }}</div>
        <div class="stat-label">{{ $t('assistant.publishedCount') }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-value stat-active">{{ store.stats.activeName && store.stats.activeName !== '无' ? store.stats.activeName : $t('assistant.noActiveRole') }}</div>
        <div class="stat-label">{{ $t('assistant.activeRole') }}</div>
      </div>
    </div>

    <!-- 工具栏 -->
    <div class="toolbar">
      <el-input
        v-model="store.filter.search"
        class="role-search-input"
        :placeholder="$t('assistant.searchRole')"
        style="width: 300px;"
        @change="store.loadRoles()"
      >
        <template #prefix><el-icon><Search /></el-icon></template>
        <template #suffix>
          <SearchClearButton v-if="store.filter.search" @clear="clearRoleSearch" />
        </template>
      </el-input>

      <el-select v-model="store.filter.sourceType" :placeholder="$t('assistant.source')" clearable style="width: 120px;"
        @change="store.loadRoles()">
        <el-option value="user" :label="$t('assistant.myCreated')" />
        <el-option value="square" :label="$t('assistant.fromSquare')" />
      </el-select>

      <el-select v-model="store.filter.isActive" :placeholder="$t('assistant.status')" clearable style="width: 100px;"
        @change="store.loadRoles()">
        <el-option value="true" :label="$t('assistant.activatedStatus')" />
        <el-option value="false" :label="$t('assistant.inactiveStatus')" />
      </el-select>

      <div style="flex: 1;" />
      <el-button v-if="store.activeRole" class="btn-close-role" type="danger" plain size="small" @click="store.deactivateRole()">
        <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="2" fill="none">
        <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
        {{ $t('assistant.deactivate') }}
      </el-button>
      <el-button class="btn-create-role" type="primary" @click="openCreate">
        <el-icon><Plus /></el-icon>{{ $t('assistant.createRole') }}
      </el-button>
    </div>

    <!-- 卡片网格 -->
    <div class="card-grid" v-loading="store.loading">
      <el-empty v-if="!store.loading && !store.roles.length"
        :description="$t('assistant.noRolesHint')" :image-size="80" />
      <RoleCard
        v-for="role in store.roles"
        :key="role.id"
        :role="role"
        @edit="openEdit"
        @activate="handleActivate"
        @delete="handleDelete"
        @clone="handleClone"
        @publish="openPublish"
        @unpublish="handleUnpublish"
      />
    </div>

    <!-- 发布弹窗 -->
    <el-dialog v-model="publishVisible" :title="$t('assistant.publishTitle')" width="440px"
      :append-to-body="true" :close-on-click-modal="false">
      <el-form label-width="80px">
        <el-form-item :label="$t('assistant.category')">
          <el-select v-model="publishForm.category" style="width: 100%">
            <el-option v-for="c in CATEGORIES" :key="c.value" :value="c.value" :label="c.label" />
          </el-select>
        </el-form-item>
        <el-form-item :label="$t('assistant.squareDesc')">
          <el-input v-model="publishForm.squareDesc" type="textarea" :rows="3"
            :placeholder="$t('assistant.squareDescPlaceholder')" maxlength="300" show-word-limit />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button class="theme-action--secondary" @click="publishVisible = false">{{ $t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="publishLoading" @click="doPublish">{{ $t('assistant.confirmPublish') }}</el-button>
      </template>
    </el-dialog>

    <!-- 创建/编辑弹窗 -->
    <RoleCreateDialog
      v-model="createVisible"
      :mode="dialogMode"
      :initial-data="editingRole"
      @success="handleSaved"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { Search, Plus } from '@element-plus/icons-vue'
import { useAssistantStore } from '@/stores/assistant'
import type { Role, CreateRoleData } from '@/types/assistant'
import RoleCard from './components/RoleCard.vue'
import RoleCreateDialog from './components/RoleCreateDialog.vue'
import SearchClearButton from '@/components/common/SearchClearButton.vue'

const { t } = useI18n()

const CATEGORIES = computed(() => [
  { value: 'work', label: t('assistant.cat_work') },
  { value: 'writing', label: t('assistant.cat_writing') },
  { value: 'coding', label: t('assistant.cat_coding') },
  { value: 'education', label: t('assistant.cat_education') },
  { value: 'life', label: t('assistant.cat_life') },
  { value: 'other', label: t('assistant.cat_other') },
])

const store = useAssistantStore()

const createVisible = ref(false)
const dialogMode = ref<'add' | 'edit'>('add')
const editingRole = ref<Role | null>(null)

const publishVisible = ref(false)
const publishLoading = ref(false)
const publishingId = ref('')
const publishForm = ref({ category: 'work', squareDesc: '' })

onMounted(async () => {
  await Promise.all([store.loadRoles(), store.loadStats(), store.loadActiveRole()])
})

function openCreate() {
  dialogMode.value = 'add'
  editingRole.value = null
  createVisible.value = true
}

async function openEdit(role: Role) {
  dialogMode.value = 'edit'
  try {
    editingRole.value = await invoke<Role | null>('get_role', { id: role.id }) ?? role
  } catch {
    editingRole.value = role
  }
  createVisible.value = true
}

async function handleSaved(data: CreateRoleData & { id?: string }) {
  if (dialogMode.value === 'edit' && data.id) {
    const ok = await store.updateRole(data.id, data)
    if (ok) createVisible.value = false
  } else {
    const ok = await store.createRole(data)
    if (ok) createVisible.value = false
  }
}

async function handleActivate(id: string) {
  await store.activateRole(id)
}

async function handleDelete(id: string) {
  await store.deleteRole(id)
}

async function handleClone(id: string) {
  await store.cloneRole(id)
}

function openPublish(id: string) {
  publishingId.value = id
  publishForm.value = { category: 'work', squareDesc: '' }
  publishVisible.value = true
}

async function doPublish() {
  publishLoading.value = true
  try {
    const ok = await store.publishRole(publishingId.value, publishForm.value.category, publishForm.value.squareDesc)
    if (ok) publishVisible.value = false
  } finally {
    publishLoading.value = false
  }
}

async function handleUnpublish(id: string) {
  await store.unpublishRole(id)
}

function clearRoleSearch() {
  store.filter.search = ''
  store.loadRoles()
}
</script>

<style scoped lang="scss">
.role-list {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 14px;
  overflow: hidden;
}

.stats-row {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 12px;
  flex-shrink: 0;
}

.stat-card {
 // background: var(--bg-secondary, var(--bg-primary));
  background:transparent;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 12px 16px;
  text-align: center;
}

.stat-value {
  font-size: 22px;
  font-weight: 700;
  color: var(--text-primary);
  line-height: 1.2;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.stat-active {
  font-size: 15px;
  color: var(--el-color-success);
}

.stat-label {
  font-size: var(--font-size-base);
  color: var(--text-secondary);
  margin-top: 4px;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}


.card-grid {
  flex: 1;
  overflow-y: auto;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(calc(var(--font-size-base) * 17.14), 1fr));
  gap: 14px;
  align-content: start;
  padding-bottom: 8px;
  padding-right: 4px;
}

:deep(.el-input__wrapper) {
  background: transparent !important;
  border-color: var(--border-color) !important;
  box-shadow: none !important;
  height: 36px !important;
}

.role-search-input :deep(button.skills-search-clear) {
  width: 24px;
  height: 24px;
  min-width: 24px;
  max-width: 24px;
  padding: 0 !important;
  gap: 0 !important;
  border: none !important;
  border-radius: 8px;
  flex: none;
  box-sizing: border-box;
}


:deep(.el-select__wrapper) {
  background: transparent !important;
  border-color: var(--border-color) !important;
  box-shadow: none !important;
  height: 36px !important;
}

:deep(.el-select-dropdown__item:hover) {
  background: var(--dropdown-hover-color) !important;
}

// 修改这个关闭角色按钮（通过 type="danger" 配合 plain 定位）
.btn-close-role {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 36px;
    padding: 0 12px;
    border-radius: 4px;
    background: #dc2626;
    color: var(--btn-primary-text, #fff);
    font-size: var(--font-size-md);
    cursor: pointer;
    white-space: nowrap;
    transition: all 0.15s;
    border:none;
    &:hover {
    background: #c72424;
    color: var(--btn-primary-hover-text, #fff);
    }
}

.btn-create-role {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 36px;
  padding: 0 12px;
  border-radius: 4px;
  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);
  border-color: var(--btn-primary-border, transparent);
  font-size: var(--font-size-md);
  cursor: pointer;
  white-space: nowrap;
  transition: all 0.15s;

  &:hover {
    background: var(--btn-primary-hover-bg, var(--color-primary));
    color: var(--btn-primary-hover-text, #fff);
    border-color: var(--btn-primary-hover-border, transparent);
  }
}


.theme-action--secondary {
  color: var(--btn-secondary-text) !important;
  background: var(--btn-secondary-bg) !important;
  border-color: var(--btn-secondary-border) !important;

  &:hover {
    background: var(--btn-secondary-hover-bg) !important;
    color: var(--btn-secondary-hover-text) !important;
    border-color: var(--btn-secondary-hover-border) !important;
  }
}

:deep(.theme-action--primary) {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 36px;
  padding: 0 12px;
  border-radius: 4px;
  background: var(--btn-primary-bg, var(--color-primary));
  color: var(--btn-primary-text, #fff);
  border-color: var(--btn-primary-border, transparent);
  font-size: var(--font-size-md);
  cursor: pointer;
  white-space: nowrap;
  transition: all 0.15s;

  &:hover {
    background: var(--btn-primary-hover-bg, var(--color-primary));
    color: var(--btn-primary-hover-text, #fff);
    border-color: var(--btn-primary-hover-border, transparent);
  }
}

:deep(.el-empty) {
  grid-column: 1 / -1;
  min-height: 40vh;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}

:deep(.el-textarea__inner) {
  background: transparent !important;
  border: 1px solid var(--border-color) !important;
  box-shadow: none !important;

  &:hover, &:focus {
    border: 1px solid var(--border-color) !important;
    box-shadow: none !important;
  }
}
</style>
