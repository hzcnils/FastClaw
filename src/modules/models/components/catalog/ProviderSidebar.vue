<script setup lang="ts">
import SvgIcon from '@/components/svgIcon/index.vue'
import type { ModelCatalogTab } from '../../stores/modelCatalog.store'
import type { RawProviderResponse } from '../../types/contracts'

defineProps<{
  providers: RawProviderResponse[]
  selectedProviderId: string | null
  fixedTab: ModelCatalogTab | null
  search: string
  moreMenuOpen: boolean
}>()

const emit = defineEmits<{
  'update:search': [value: string]
  'update:moreMenuOpen': [value: boolean]
  selectTab: [tab: ModelCatalogTab]
  selectProvider: [provider: RawProviderResponse]
  add: []
  export: []
  import: []
}>()
</script>

<template>
  <aside class="provider-sidebar">
    <div class="sidebar-header">
      <div class="search-box">
        <svg viewBox="0 0 24 24" width="16" height="26" stroke="currentColor" stroke-width="2" fill="none"><circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" /></svg>
        <input :value="search" type="text" :placeholder="$t('modelManage.searchProvider')" @input="emit('update:search', ($event.target as HTMLInputElement).value)" />
      </div>
    </div>

    <div class="provider-list">
      <div class="provider-item" :class="{ active: fixedTab === 'all' }" @click="emit('selectTab', 'all')"><el-icon :size="18"><SvgIcon icon-name="all" /></el-icon><span class="p-name">{{ $t('modelManage.all') }}</span></div>
      <div class="provider-item" :class="{ active: fixedTab === 'gateway' }" @click="emit('selectTab', 'gateway')"><el-icon :size="18"><SvgIcon icon-name="gateway" /></el-icon><span class="p-name">{{ $t('modelManage.gateway') }}</span></div>
      <div v-for="provider in providers" :key="provider.id" class="provider-item" :class="{ active: selectedProviderId === provider.id && !fixedTab }" @click="emit('selectProvider', provider)">
        <el-icon :size="18"><SvgIcon :icon-name="provider.provider_code || ''" /></el-icon>
        <span class="p-name">{{ provider.name }}</span>
        <el-icon :size="18" class="source-type-icon"><SvgIcon icon-name="internet" /></el-icon>
        <span class="status-badge" :class="provider.is_enabled ? 'on' : 'off'">{{ provider.is_enabled ? 'ON' : 'OFF' }}</span>
      </div>
    </div>

    <div class="sidebar-footer">
      <button class="btn-add-provider" @click="emit('add')">
        <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
        {{ $t('modelManage.addProvider') }}
      </button>
      <div class="more-menu-wrapper" @click.stop>
        <button class="btn-more-menu" :title="$t('modelManage.moreActions')" @click="emit('update:moreMenuOpen', !moreMenuOpen)">
          <svg viewBox="0 0 5 20" width="5" height="18" fill="currentColor"><circle cx="2.5" cy="2.5" r="2" /><circle cx="2.5" cy="10" r="2" /><circle cx="2.5" cy="17.5" r="2" /></svg>
        </button>
        <div v-if="moreMenuOpen" class="more-menu-popup">
          <div class="more-menu-item" @click="emit('export')"><svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="7 10 12 15 17 10" /><line x1="12" y1="15" x2="12" y2="3" /></svg>{{ $t('modelManage.exportModels') }}</div>
          <div class="more-menu-item" @click="emit('import')"><svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="17 8 12 3 7 8" /><line x1="12" y1="3" x2="12" y2="15" /></svg>{{ $t('modelManage.importModels') }}</div>
        </div>
      </div>
    </div>
  </aside>
</template>

<style scoped lang="scss">
@use '../../styles/provider-sidebar';
</style>
