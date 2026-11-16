<template>
  <div class="role-square">
    <!-- 顶部搜索栏 -->
    <div class="square-header">
      <el-input
        v-model="store.squareFilter.search"
        :placeholder="$t('assistant.searchSquare')"
        clearable
        style="width: 300px;"
        @change="handleSearch"
        @clear="handleSearch"
      >
        <template #prefix><el-icon><Search /></el-icon></template>
      </el-input>
      <el-select v-model="store.squareFilter.sortBy" style="width: 130px;" @change="handleSearch">
        <el-option value="newest" :label="$t('assistant.sortNewest')" />
        <el-option value="mostUsed" :label="$t('assistant.sortMostUsed')" />
        <el-option value="mostStarred" :label="$t('assistant.sortMostStarred')" />
      </el-select>
      <span class="result-count">{{ $t('assistant.squareCount', { n: store.squareTotal }) }}</span>
    </div>

    <!-- 主体：左侧分类 + 右侧卡片 -->
    <div class="square-body">
      <!-- 分类侧栏 -->
      <div class="category-sidebar">
        <div
          v-for="c in CATEGORIES"
          :key="c.value"
          class="category-item"
          :class="{ 'is-active': store.squareFilter.category === c.value }"
          @click="selectCategory(c.value)"
        >
          {{ c.label }}
        </div>
      </div>

      <!-- 卡片区域 -->
      <div class="card-area" v-loading="store.squareLoading">
        <el-empty v-if="!store.squareLoading && !store.squareRoles.length"
          :description="$t('assistant.emptyCategory')" :image-size="80" />

        <div class="square-grid">
          <div
            v-for="role in store.squareRoles"
            :key="role.id"
            class="square-card"
          >
            <!-- 卡片头部 -->
            <div class="sq-card-header">
              <div class="sq-avatar" :style="getAvatarStyle(role)">
                <img v-if="role.avatar" :src="role.avatar" alt="" />
                <span v-else class="sq-avatar-letter">{{ getAvatarLetter(role) }}</span>
              </div>
              <div class="sq-card-info">
                <div class="sq-role-name" :title="role.name">{{ role.name }}</div>
                <div class="sq-author">{{ role.authorName || $t('assistant.anonymousUser') }}</div>
              </div>
              <el-tag size="small" class="sq-category-tag">{{ getCategoryLabel(role.category) }}</el-tag>
            </div>

            <!-- 描述 -->
            <div class="sq-desc">{{ role.description || $t('assistant.noDescription') }}</div>

            <!-- 工具摘要 -->
            <div class="sq-tools" v-if="hasTools(role)">
              <el-tag v-if="role.toolSummary?.mcps?.length" size="small" type="warning">
                MCP×{{ role.toolSummary.mcps.length }}
              </el-tag>
              <el-tag v-if="role.toolSummary?.skills?.length" size="small" type="success">
                Skills×{{ role.toolSummary.skills.length }}
              </el-tag>
            </div>

            <!-- 底部操作 -->
            <div class="sq-card-footer">
              <div class="sq-meta">
                <span class="meta-item">
                  <el-icon><User /></el-icon>{{ role.useCount }}
                </span>
                <span class="meta-item">
                  <el-icon><Star /></el-icon>{{ role.starCount }}
                </span>
              </div>
              <div class="sq-actions">
                <el-button size="small" text :loading="starringId === role.id" @click="handleStar(role.id)">
                  <el-icon><Star /></el-icon>
                </el-button>
                <el-button size="small" type="primary" :loading="applyingId === role.id" @click="handleApply(role.id)">
                  {{ $t('assistant.applyRole') }}
                </el-button>
              </div>
            </div>
          </div>
        </div>

        <!-- 分页 -->
        <div class="pagination-bar" v-if="store.squareTotal > store.squareFilter.pageSize">
          <el-pagination
            v-model:current-page="store.squareFilter.page"
            :page-size="store.squareFilter.pageSize"
            :total="store.squareTotal"
            layout="prev, pager, next"
            small
            @current-change="store.loadSquareRoles()"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { Search, User, Star } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import { useAssistantStore } from '@/stores/assistant'
import type { RoleCategory, SquareRole } from '@/types/assistant'

const { t } = useI18n()

const CATEGORIES = computed(() => [
  { value: 'all' as RoleCategory, label: t('assistant.cat_all') },
  { value: 'work' as RoleCategory, label: t('assistant.cat_work') },
  { value: 'writing' as RoleCategory, label: t('assistant.cat_writing') },
  { value: 'coding' as RoleCategory, label: t('assistant.cat_coding') },
  { value: 'education' as RoleCategory, label: t('assistant.cat_education') },
  { value: 'life' as RoleCategory, label: t('assistant.cat_life') },
  { value: 'other' as RoleCategory, label: t('assistant.cat_other') },
])

const AVATAR_COLORS = [
  '#4b6bfc', '#10b981', '#f59e0b', '#ef4444',
  '#8b5cf6', '#06b6d4', '#ec4899',
]

const store = useAssistantStore()
const applyingId = ref('')
const starringId = ref('')

onMounted(() => {
  store.squareFilter.page = 1
  store.loadSquareRoles()
})

function selectCategory(cat: RoleCategory) {
  store.squareFilter.category = cat
  store.squareFilter.page = 1
  store.loadSquareRoles()
}

function handleSearch() {
  store.squareFilter.page = 1
  store.loadSquareRoles()
}

async function handleApply(id: string) {
  applyingId.value = id
  try {
    const role = await store.applySquareRole(id)
    if (role) ElMessage.success(t('assistant.applySuccess', { name: role.name }))
  } finally {
    applyingId.value = ''
  }
}

async function handleStar(id: string) {
  starringId.value = id
  try {
    await store.starSquareRole(id)
  } finally {
    starringId.value = ''
  }
}

function getAvatarLetter(role: SquareRole) {
  return (role.name || '?').charAt(0).toUpperCase()
}

function getAvatarStyle(role: SquareRole) {
  if (role.avatar) return {}
  const code = role.id.charCodeAt(0) + role.id.charCodeAt(1)
  return { background: AVATAR_COLORS[code % AVATAR_COLORS.length] }
}

function getCategoryLabel(cat: string) {
  return CATEGORIES.value.find(c => c.value === cat)?.label ?? cat
}

function hasTools(role: SquareRole) {
  const t = role.toolSummary
  return (t?.mcps?.length || t?.skills?.length)
}
</script>

<style scoped lang="scss">
.role-square {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow: hidden;
}

.square-header {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
}

.result-count {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
  margin-left: auto;
}

.square-body {
  flex: 1;
  display: flex;
  gap: 14px;
  overflow: hidden;
}

.category-sidebar {
  width: 120px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
  overflow-y: auto;
}

.category-item {
  padding: 8px 12px;
  border-radius: 6px;
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
  white-space: nowrap;

  &:hover {
    background: var(--bg-hover, rgba(0, 0, 0, 0.04));
    color: var(--text-primary);
  }

  &.is-active {
    background: var(--el-color-primary-light-9);
    color: var(--el-color-primary);
    font-weight: 500;
  }
}

.card-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.square-grid {
  flex: 1;
  overflow-y: auto;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(calc(var(--font-size-base) * 14.29), 1fr));
  gap: 12px;
  align-content: start;
  padding-right: 4px;
  padding-bottom: 4px;
}

.square-card {
  background: var(--card-background-tmd);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  transition: box-shadow 0.2s;

  &:hover {
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.08);
  }
}

.sq-card-header {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.sq-avatar {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  flex-shrink: 0;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;

  img { width: 100%; height: 100%; object-fit: cover; }

  .sq-avatar-letter {
    font-size: 15px;
    font-weight: 700;
    color: #fff;
  }
}

.sq-card-info {
  flex: 1;
  min-width: 0;
}

.sq-role-name {
  font-size: var(--font-size-base);
  font-weight: 600;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sq-author {
  font-size: 11px;
  color: var(--text-tertiary, var(--text-secondary));
}

.sq-category-tag {
  flex-shrink: 0;
  font-size: 10px;
}

.sq-desc {
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  line-height: 1.5;
  min-height: 36px;
}

.sq-tools {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.sq-card-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-top: 6px;
  border-top: 1px solid var(--border-color);
}

.sq-meta {
  display: flex;
  gap: 10px;
  font-size: var(--font-size-sm);
  color: var(--text-secondary);
}

.meta-item {
  display: flex;
  align-items: center;
  gap: 3px;
}

.sq-actions {
  display: flex;
  align-items: center;
  gap: 4px;
}

.pagination-bar {
  display: flex;
  justify-content: center;
  padding-top: 10px;
  flex-shrink: 0;
}

:deep(.el-input__wrapper) {
  background: transparent !important;
  border-color: var(--border-color) !important;
  box-shadow: none !important;
  height: 36px !important;
}

:deep(.el-select__wrapper) {
  background: transparent !important;
  border-color: var(--border-color) !important;
  box-shadow: none !important;
  height: 36px !important;
}

:deep(.el-empty) {
  min-height: 40vh;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}
</style>
