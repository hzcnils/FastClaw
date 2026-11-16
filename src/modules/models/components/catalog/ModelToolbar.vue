<script setup lang="ts">
import { Search } from '@element-plus/icons-vue'
import SearchClearButton from '@/components/common/SearchClearButton.vue'

defineProps<{
  viewMode: 'card' | 'ranking'
  search: string
  canManage: boolean
  canSync: boolean
  syncing: boolean
}>()

const emit = defineEmits<{
  'update:viewMode': [value: 'card' | 'ranking']
  'update:search': [value: string]
  manage: []
  fetch: []
  manualAdd: []
  sync: []
}>()
</script>

<template>
  <div class="model-actions">
    <el-input
      v-if="viewMode === 'card'"
      :model-value="search"
      :placeholder="$t('modelManage.searchModelPlaceholder')"
      style="width: 300px"
      :prefix-icon="Search"
      class="model-search-input"
      @update:model-value="emit('update:search', $event)"
    >
      <template #suffix>
        <SearchClearButton v-if="search" @clear="emit('update:search', '')" />
      </template>
    </el-input>
    <div class="view-toggle">
      <button class="view-toggle-btn" :class="{ active: viewMode === 'card' }" :title="$t('modelManage.cardView')" @click="emit('update:viewMode', 'card')">
        <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
          <rect x="3" y="3" width="7" height="7" rx="1" /><rect x="14" y="3" width="7" height="7" rx="1" />
          <rect x="3" y="14" width="7" height="7" rx="1" /><rect x="14" y="14" width="7" height="7" rx="1" />
        </svg>
      </button>
      <button class="view-toggle-btn" :class="{ active: viewMode === 'ranking' }" :title="$t('modelManage.ranking')" @click="emit('update:viewMode', 'ranking')">
        <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none">
          <line x1="18" y1="20" x2="18" y2="10" /><line x1="12" y1="20" x2="12" y2="4" /><line x1="6" y1="20" x2="6" y2="14" />
        </svg>
        {{ $t('modelManage.ranking') || '排行榜' }}
      </button>
    </div>
    <button v-if="canManage" class="btn-manage" @click="emit('manage')">
      <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none">
        <line x1="8" y1="6" x2="21" y2="6" /><line x1="8" y1="12" x2="21" y2="12" /><line x1="8" y1="18" x2="21" y2="18" />
        <line x1="3" y1="6" x2="3.01" y2="6" /><line x1="3" y1="12" x2="3.01" y2="12" /><line x1="3" y1="18" x2="3.01" y2="18" />
      </svg>
      {{ $t('modelManage.manageModels') }}
    </button>
    <span v-if="canManage" class="btn-split-group">
      <button type="button" class="btn-add-model" @click="emit('fetch')">
        <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
        {{ $t('modelManage.fetchModelList') }}
      </button>
      <button type="button" class="btn-add-model btn-add-model--icon" :title="$t('modelManage.manualAdd')" @click="emit('manualAdd')">
        <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
      </button>
    </span>
    <button v-if="canSync" type="button" class="btn-manage" :disabled="syncing" @click="emit('sync')">
      <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none">
        <polyline points="23 4 23 10 17 10" /><polyline points="1 20 1 14 7 14" />
        <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
      </svg>
      {{ $t('modelManage.syncModels') }}
    </button>
  </div>
</template>

<style scoped lang="scss">
@use '../../styles/model-toolbar';
</style>
