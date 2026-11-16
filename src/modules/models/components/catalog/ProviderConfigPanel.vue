<script setup lang="ts">
import { ref, watch } from 'vue'
import type { ModelCatalogTab, SelectedModelProvider } from '../../stores/modelCatalog.store'

const props = defineProps<{
  provider: SelectedModelProvider
  fixedTab: ModelCatalogTab | null
  sourceCategoryOptions: Array<{ value: string; label: string; icon: string }>
  editingBaseUrl: boolean
  editingBaseUrlValue: string
}>()
const emit = defineEmits<{
  sourceCategory: [value: string]
  share: []
  editProvider: []
  deleteProvider: []
  toggleProvider: [provider: SelectedModelProvider]
  apiKeyFocus: [value: string | null]
  apiKeyBlur: [value: string | null]
  test: [provider: SelectedModelProvider]
  'update:editingBaseUrlValue': [value: string]
  startBaseUrlEdit: []
  confirmBaseUrl: []
  cancelBaseUrl: []
  resetBaseUrl: []
}>()

const apiKeyDraft = ref(props.provider.api_key || '')
const showApiKey = ref(false)
watch(() => [props.provider.id, props.provider.api_key] as const, () => { apiKeyDraft.value = props.provider.api_key || '' })
</script>

<template>
  <div class="provider-config-wrap">
    <div class="detail-header">
      <div class="title-area">
        <span class="provider-title">{{ provider.name }} API</span>
        <el-select :model-value="provider.source_category" class="source-category-select" popper-class="source-category-popper" @update:model-value="emit('sourceCategory', String($event))">
          <el-option v-for="option in sourceCategoryOptions" :key="option.value" :label="option.label" :value="option.value" />
        </el-select>
        <button class="share-button" @click="emit('share')"><svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" /><polyline points="15 3 21 3 21 9" /><line x1="10" y1="14" x2="21" y2="3" /></svg></button>
      </div>
      <div class="header-actions">
        <button v-if="!fixedTab" class="btn-icon" :title="$t('modelManage.editProvider')" @click="emit('editProvider')"><svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" /><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" /></svg></button>
        <button v-if="!fixedTab" class="btn-icon btn-delete-provider" :title="$t('modelManage.deleteProvider')" @click="emit('deleteProvider')"><svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none"><polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /></svg></button>
        <label class="toggle-switch"><input type="checkbox" :checked="provider.is_enabled" @change="emit('toggleProvider', provider)" /><span class="slider" /></label>
      </div>
    </div>

    <div class="config-section">
      <div class="config-item">
        <div class="config-label"><span>{{ $t('modelManage.apiKey') }}</span></div>
        <div class="config-input-group api-key-group">
          <input v-model="apiKeyDraft" :type="showApiKey ? 'text' : 'password'" class="secret-input" :placeholder="$t('modelManage.apiKeyPlaceholder')" @focus="emit('apiKeyFocus', apiKeyDraft)" @blur="emit('apiKeyBlur', apiKeyDraft)" />
          <button class="btn-toggle-key" type="button" :title="showApiKey ? $t('modelManage.hideKey') : $t('modelManage.showKey')" @click="showApiKey = !showApiKey">
            <svg v-if="showApiKey" viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" /><line x1="1" y1="1" x2="23" y2="23" /></svg>
            <svg v-else viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8z" /><circle cx="12" cy="12" r="3" /></svg>
          </button>
          <button class="btn-check" @click="emit('test', provider)"><svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" /><polyline points="22 4 12 14.01 9 11.01" /></svg>{{ $t('modelManage.testConn') }}</button>
        </div>
        <div class="config-hint">{{ $t('modelManage.multiKeyHint') }}</div>
      </div>
      <div class="config-item">
        <div class="config-label"><span>{{ $t('modelManage.apiAddr') }}</span></div>
        <div class="config-input-group">
          <el-input :model-value="editingBaseUrlValue" :disabled="!editingBaseUrl" class="address-input flex-1" :class="{ 'address-input-editing': editingBaseUrl }" :placeholder="$t('modelManage.apiAddrPlaceholder')" @update:model-value="emit('update:editingBaseUrlValue', $event)" />
          <template v-if="editingBaseUrl"><button class="btn-confirm" @click="emit('confirmBaseUrl')">{{ $t('modelManage.confirm') }}</button><button class="btn-cancel" @click="emit('cancelBaseUrl')">{{ $t('modelManage.cancel') }}</button></template>
          <template v-else><button class="btn-edit" @click="emit('startBaseUrlEdit')">{{ $t('modelManage.edit') }}</button><button class="btn-reset" @click="emit('resetBaseUrl')">{{ $t('modelManage.reset') }}</button></template>
        </div>
        <div class="config-hint">{{ $t('modelManage.proxyHint') }}</div>
      </div>
    </div>
  </div>
</template>

<style scoped lang="scss">
@use '../../styles/provider-config-panel';
</style>
