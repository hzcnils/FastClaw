<script setup lang="ts">
import type { GatewayProvider } from '../../types/contracts'
import type { SelectedModelProvider } from '../../stores/modelCatalog.store'

defineProps<{
  providers: GatewayProvider[]
  modelValue: string | null
  selectedProvider: SelectedModelProvider | null
}>()
const emit = defineEmits<{
  'update:modelValue': [value: string]
  add: []
  edit: [provider: SelectedModelProvider]
  delete: [provider: SelectedModelProvider]
  toggle: [provider: SelectedModelProvider]
}>()
</script>

<template>
  <div class="gateway-tabs">
    <div class="gateway-tabs-header">
      <el-tabs :model-value="modelValue" class="gateway-tabs-el" @update:model-value="emit('update:modelValue', String($event))">
        <el-tab-pane v-for="provider in providers" :key="provider.id" :name="provider.id">
          <template #label><span class="tab-label-content"><span class="tab-name">{{ provider.name }}</span><el-tag v-if="provider._gateway_name">{{ provider._gateway_name }}</el-tag></span></template>
        </el-tab-pane>
      </el-tabs>
      <button class="btn-add-tab" :title="$t('modelManage.addGatewayProvider')" @click="emit('add')">
        <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
      </button>
    </div>
    <div v-if="selectedProvider" class="gateway-actions-row">
      <span class="current-provider-name">{{ selectedProvider.name }}</span>
      <div class="gateway-actions-right">
        <button class="btn-icon" :title="$t('modelManage.editProvider')" @click="emit('edit', selectedProvider)">
          <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" /><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" /></svg>
        </button>
        <el-popconfirm :title="$t('modelManage.confirmDeleteProvider', { name: selectedProvider.name })" :confirm-button-text="$t('common.confirm')" :cancel-button-text="$t('common.cancel')" @confirm="emit('delete', selectedProvider)">
          <template #reference><button class="btn-icon btn-delete-provider" :title="$t('modelManage.deleteProvider')"><svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none"><polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /></svg></button></template>
        </el-popconfirm>
        <label class="toggle-switch"><input type="checkbox" :checked="selectedProvider.is_enabled" @change="emit('toggle', selectedProvider)" /><span class="slider" /></label>
      </div>
    </div>
    <div v-if="!providers.length" class="gateway-empty-hint">{{ $t('modelManage.noGatewayProviders') }}</div>
  </div>
</template>

<style scoped lang="scss">
@use '../../styles/gateway-provider-tabs';
</style>
