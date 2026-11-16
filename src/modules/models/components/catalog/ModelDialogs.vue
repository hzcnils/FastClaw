<script setup lang="ts">
import { computed } from 'vue'
import ConnectionTestDialog from '@/components/model/ConnectionTestDialog.vue'
import ImportModelsDialog from '@/components/model/ImportModelsDialog.vue'
import AddProviderDialog from '@/views/model/AddProviderDialog.vue'
import AddGatewayDialog from '@/views/model/AddGatewayDialog.vue'
import AddGatewayProviderDialog from '@/views/model/AddGatewayProviderDialog.vue'
import EditProviderDialog from '@/views/model/EditProviderDialog.vue'
import EditModelDialog from '@/views/model/EditModelDialog.vue'
import AddModelDialog from '@/views/model/AddModelDialog.vue'
import ManualAddModelDialog from '@/views/model/ManualAddModelDialog.vue'
import SyncPreviewDialog from '@/views/model/SyncPreviewDialog.vue'
import type { RawGatewayResponse, RawModelResponse, SyncPreview, SyncResult } from '../../types/contracts'
import type { SelectedModelProvider } from '../../stores/modelCatalog.store'

interface ProviderEditorTarget {
  id: string
  name: string
  provider_type: string
  base_url: string
  api_key: string
}

const props = defineProps<{
  showAdd: boolean
  showAddGateway: boolean
  showAddGatewayProvider: boolean
  showDeleteProvider: boolean
  showEditProvider: boolean
  showEditModel: boolean
  showAddModel: boolean
  showManualAddModel: boolean
  showSyncPreview: boolean
  showTest: boolean
  showImport: boolean
  selectedProvider: SelectedModelProvider | null
  editTargetProvider: ProviderEditorTarget | null
  editTargetModel: RawModelResponse | null
  testProvider: SelectedModelProvider | null
  testBaseUrl: string
  testApiKey: string
  gateways: RawGatewayResponse[]
  syncPreview: SyncPreview | null
}>()

const emit = defineEmits<{
  'update:showAdd': [value: boolean]
  'update:showAddGateway': [value: boolean]
  'update:showAddGatewayProvider': [value: boolean]
  'update:showDeleteProvider': [value: boolean]
  'update:showEditProvider': [value: boolean]
  'update:showEditModel': [value: boolean]
  'update:showAddModel': [value: boolean]
  'update:showManualAddModel': [value: boolean]
  'update:showSyncPreview': [value: boolean]
  'update:showTest': [value: boolean]
  'update:showImport': [value: boolean]
  providerSaved: []
  gatewaySaved: []
  gatewayProviderSaved: []
  editProviderSaved: [provider: ProviderEditorTarget]
  editModelSaved: [data: { id: string; display_name: string; model_label: string | null; context_window: number | string; mxcs: unknown }]
  addModelSaved: []
  manualModelSaved: []
  syncCompleted: [result: SyncResult]
  testSaved: [apiKey: string]
  imported: []
  confirmDeleteProvider: []
}>()

const addVisible = computed({ get: () => props.showAdd, set: value => emit('update:showAdd', value) })
const addGatewayVisible = computed({ get: () => props.showAddGateway, set: value => emit('update:showAddGateway', value) })
const addGatewayProviderVisible = computed({ get: () => props.showAddGatewayProvider, set: value => emit('update:showAddGatewayProvider', value) })
const editProviderVisible = computed({ get: () => props.showEditProvider, set: value => emit('update:showEditProvider', value) })
const editModelVisible = computed({ get: () => props.showEditModel, set: value => emit('update:showEditModel', value) })
const addModelVisible = computed({ get: () => props.showAddModel, set: value => emit('update:showAddModel', value) })
const manualAddVisible = computed({ get: () => props.showManualAddModel, set: value => emit('update:showManualAddModel', value) })
const syncVisible = computed({ get: () => props.showSyncPreview, set: value => emit('update:showSyncPreview', value) })
const testVisible = computed({ get: () => props.showTest, set: value => emit('update:showTest', value) })
</script>

<template>
  <AddProviderDialog v-model:visible="addVisible" @saved="emit('providerSaved')" />
  <AddGatewayDialog v-model:visible="addGatewayVisible" @saved="emit('gatewaySaved')" />
  <AddGatewayProviderDialog v-model:visible="addGatewayProviderVisible" :gateways="gateways" @saved="emit('gatewayProviderSaved')" />

  <div v-if="showDeleteProvider" class="overlay">
    <div class="dialog">
      <div class="dialog-header"><h3>{{ $t('modelManage.confirmDelete') }}</h3><button class="close-btn" @click="emit('update:showDeleteProvider', false)"><svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg></button></div>
      <p class="delete-message">{{ $t('modelManage.confirmDeleteProvider', { name: selectedProvider?.name }) }}</p>
      <div class="actions"><el-button class="btn-cancel" @click="emit('update:showDeleteProvider', false)">{{ $t('common.cancel') }}</el-button><el-button class="btn-confirm" type="primary" @click="emit('confirmDeleteProvider')">{{ $t('modelManage.delete') }}</el-button></div>
    </div>
  </div>

  <EditProviderDialog v-model:visible="editProviderVisible" :provider="editTargetProvider" @saved="emit('editProviderSaved', $event)" />
  <EditModelDialog v-model:visible="editModelVisible" :model="editTargetModel" @saved="emit('editModelSaved', $event)" />
  <AddModelDialog v-model:visible="addModelVisible" :provider-code="selectedProvider?.provider_code || ''" :provider-name="selectedProvider?.name || ''" @saved="emit('addModelSaved')" />
  <ManualAddModelDialog v-model:visible="manualAddVisible" :provider-code="selectedProvider?.provider_code || ''" :provider-name="selectedProvider?.name || ''" @saved="emit('manualModelSaved')" />
  <SyncPreviewDialog v-model:visible="syncVisible" :preview="syncPreview" @synced="emit('syncCompleted', $event)" />
  <ConnectionTestDialog v-model:visible="testVisible" :provider-id="testProvider?.id || ''" :provider-code="testProvider?.provider_code || ''" :base-url="testBaseUrl" :api-key="testApiKey" @saved="emit('testSaved', $event)" />
  <ImportModelsDialog :visible="showImport" @close="emit('update:showImport', false)" @imported="emit('imported')" />
</template>

<style scoped lang="scss">
@use '../../styles/model-dialogs';
</style>
