<script setup lang="ts">
import ProviderSidebar from '@/modules/models/components/catalog/ProviderSidebar.vue'
import GatewayProviderTabs from '@/modules/models/components/catalog/GatewayProviderTabs.vue'
import ProviderConfigPanel from '@/modules/models/components/catalog/ProviderConfigPanel.vue'
import ModelToolbar from '@/modules/models/components/catalog/ModelToolbar.vue'
import ModelBatchBar from '@/modules/models/components/catalog/ModelBatchBar.vue'
import ModelCatalogContent from '@/modules/models/components/catalog/ModelCatalogContent.vue'
import ModelDialogs from '@/modules/models/components/catalog/ModelDialogs.vue'
import { useModelsView } from '@/modules/models/composables/useModelsView'
import '@/modules/models/styles/models-view-global.scss'

const {
  router, themeStore, viewMode, selectedProvider, modelRatings, gateways, gatewayProviders,
  activeGatewayProviderId, fixedTab, syncPreview, providerSearchQuery, showMoreMenu, showImportDialog, showSyncPreview,
  showTestDialog, showEditModelDialog, showAdd, showEditProvider, showAddModel,
  showManualAddModel, showAddGatewayDialog, showAddGatewayProviderDialog,
  showDeleteProviderDialog, editTargetProvider, editTargetModel, testDialogProvider,
  testDialogBaseUrl, testDialogApiKey, syncing, search, manageMode, selectedModelIds,
  filteredModels, isAllSelected, filteredProviders, editingBaseUrl, editingBaseUrlValue,
  sourceCategoryOptions, changeSourceCategory, handleDeleteModel, toggleModel,
  handleEditModelLabel, handleEditModelSaved, startEditBaseUrl, confirmBaseUrl,
  cancelEditBaseUrl, resetBaseUrl, onApiKeyFocus, onApiKeyBlur, handleSyncModels,
  exportModels, handleShowImport, selectFixedTab, selectProvider,
  openTestDialog, onTestSaved, handleProviderSaved, handleGatewaySaved,
  handleGatewayProviderSaved, handleEditProviderSaved, handleAddModelSaved,
  handleManualAddSaved, handleModelsImported, handleDeleteGatewayProvider,
  openEditGatewayProvider, handleSyncSynced, shareFn, settingFn, toggleProviderEnabled,
  handleDeleteProvider, confirmDeleteProvider, toggleManageMode, toggleModelSelection,
  toggleSelectAll, batchToggleModels, batchDeleteModels,
} = useModelsView()
</script>

<template>
  <div class="models-view-root">
  <div class="model-view-container" :data-theme-id="themeStore.draftPreset.id">
      <ProviderSidebar
        v-model:search="providerSearchQuery"
        v-model:more-menu-open="showMoreMenu"
        :providers="filteredProviders"
        :selected-provider-id="selectedProvider?.id ?? null"
        :fixed-tab="fixedTab"
        @add="showAdd = true"
        @export="exportModels"
        @import="handleShowImport"
        @select-tab="selectFixedTab"
        @select-provider="selectProvider"
      />

      <main v-if="selectedProvider || fixedTab" class="provider-detail">
        <GatewayProviderTabs
          v-if="fixedTab === 'gateway'"
          v-model="activeGatewayProviderId"
          :providers="gatewayProviders"
          :selected-provider="selectedProvider"
          @add="showAddGatewayProviderDialog = true"
          @edit="openEditGatewayProvider"
          @delete="handleDeleteGatewayProvider"
          @toggle="toggleProviderEnabled"
        />

        <ProviderConfigPanel
          v-if="fixedTab !== 'all' && fixedTab !== 'gateway' && selectedProvider"
          v-model:editing-base-url-value="editingBaseUrlValue"
          :provider="selectedProvider"
          :fixed-tab="fixedTab"
          :source-category-options="sourceCategoryOptions"
          :editing-base-url="editingBaseUrl"
          @source-category="changeSourceCategory"
          @share="shareFn"
          @edit-provider="settingFn"
          @delete-provider="handleDeleteProvider"
          @toggle-provider="toggleProviderEnabled"
          @api-key-focus="onApiKeyFocus"
          @api-key-blur="onApiKeyBlur"
          @test="openTestDialog"
          @start-base-url-edit="startEditBaseUrl"
          @confirm-base-url="confirmBaseUrl"
          @cancel-base-url="cancelEditBaseUrl"
          @reset-base-url="resetBaseUrl"
        />

        <section v-if="fixedTab === 'all' || selectedProvider" class="models-section">
          <div class="section-title"><h3>{{ $t('modelManage.model') }}</h3></div>
          <ModelToolbar
            v-model:view-mode="viewMode"
            v-model:search="search"
            :can-manage="fixedTab !== 'all'"
            :can-sync="false"
            :syncing="syncing"
            @manage="toggleManageMode"
            @fetch="showAddModel = true"
            @manual-add="showManualAddModel = true"
            @sync="handleSyncModels"
          />
          <ModelBatchBar
            v-if="manageMode"
            :selected-count="selectedModelIds.size"
            :all-selected="isAllSelected"
            @select-all="toggleSelectAll"
            @toggle="batchToggleModels"
            @delete="batchDeleteModels"
          />
          <ModelCatalogContent
            :view-mode="viewMode"
            :models="filteredModels"
      :provider-code="selectedProvider?.provider_code || ''"
            :manage-mode="manageMode"
            :selected-model-ids="selectedModelIds"
            :ratings="modelRatings"
            @edit="handleEditModelLabel"
            @delete="handleDeleteModel"
            @toggle="toggleModel"
            @select="toggleModelSelection"
          />
        </section>
      </main>
  </div>

    <ModelDialogs
      v-model:show-add="showAdd"
      v-model:show-add-gateway="showAddGatewayDialog"
      v-model:show-add-gateway-provider="showAddGatewayProviderDialog"
      v-model:show-delete-provider="showDeleteProviderDialog"
      v-model:show-edit-provider="showEditProvider"
      v-model:show-edit-model="showEditModelDialog"
      v-model:show-add-model="showAddModel"
      v-model:show-manual-add-model="showManualAddModel"
      v-model:show-sync-preview="showSyncPreview"
      v-model:show-test="showTestDialog"
      v-model:show-import="showImportDialog"
      :selected-provider="selectedProvider"
      :edit-target-provider="editTargetProvider"
      :edit-target-model="editTargetModel"
      :test-provider="testDialogProvider"
      :test-base-url="testDialogBaseUrl"
      :test-api-key="testDialogApiKey"
      :gateways="gateways"
      :sync-preview="syncPreview"
      @provider-saved="handleProviderSaved"
      @gateway-saved="handleGatewaySaved"
      @gateway-provider-saved="handleGatewayProviderSaved"
      @edit-provider-saved="handleEditProviderSaved"
      @edit-model-saved="handleEditModelSaved"
      @add-model-saved="handleAddModelSaved"
      @manual-model-saved="handleManualAddSaved"
      @sync-completed="handleSyncSynced"
      @test-saved="onTestSaved"
      @imported="handleModelsImported"
      @confirm-delete-provider="confirmDeleteProvider"
    />
  </div>
</template>

<style scoped lang="scss">
.models-view-root {
  height: 100%;
  min-height: 0;
}

.model-view-container {
  display: flex;
      width: 100%;
  height: 100%;
  min-height: 0;
      color: var(--text-primary);
  /* 与智能对话 chatview-shell 一致：容器透明，二级侧栏仅叠 --bg-submenu */
  background: transparent;
}

.provider-detail {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: 24px 32px;
  background: var(--bg-primary);
}

.models-section {
  margin-top: 20px;
}

  .section-title {
    display: flex;
    align-items: center;
    justify-content: space-between;
  margin-bottom: 16px;
}

.section-title h3 {
        margin: 0;
      color: var(--text-primary);
      font-size: var(--font-size-lg);
}
</style>
