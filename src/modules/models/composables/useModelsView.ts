import { computed, h, onActivated, onMounted, onUnmounted, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'
import { ElCheckbox, ElMessage, ElMessageBox } from 'element-plus'
import { useI18n } from 'vue-i18n'
import { useThemeStore } from '@/stores/theme'
import { useModelCatalogStore, type ModelCatalogTab } from '../stores/modelCatalog.store'
import { useModelSelection } from './useModelSelection'
import { useModelBatchActions } from './useModelBatchActions'
import { useProviderEditor } from './useProviderEditor'
import { modelRatingKey } from '../types/contracts'
import { notifyChatModelCatalogChanged } from '../chatCatalog'
import type {
  GatewayProvider,
  RawModelResponse,
  RawProviderResponse,
  SyncResult,
} from '../types/contracts'

type ProviderEditorTarget = Pick<RawProviderResponse, 'id' | 'name' | 'provider_type' | 'base_url'> & { api_key: string }

export function useModelsView() {
  const { t } = useI18n()
  const themeStore = useThemeStore()
  const catalog = useModelCatalogStore()
  const {
    providers, selectedProvider, models, modelRatings, gateways, gatewayProviders,
    activeGatewayProviderId, fixedTab, syncPreview, loading,
  } = storeToRefs(catalog)

  const viewMode = ref<'card' | 'ranking'>('card')
  const providerSearchQuery = ref('')
  const activeTab = ref('models')
  const showApiKey = ref(false)
  const showMoreMenu = ref(false)
  const showImportDialog = ref(false)
  const showSyncPreview = ref(false)
  const showTestDialog = ref(false)
  const showEditModelDialog = ref(false)
  const showAdd = ref(false)
  const showEditProvider = ref(false)
  const showAddModel = ref(false)
  const showManualAddModel = ref(false)
  const showAddGatewayDialog = ref(false)
  const showAddGatewayProviderDialog = ref(false)
  const showDeleteProviderDialog = ref(false)
  const editTargetProvider = ref<ProviderEditorTarget | null>(null)
  const editTargetModel = ref<RawModelResponse | null>(null)
  const testDialogProvider = ref<RawProviderResponse | GatewayProvider | null>(null)
  const testDialogBaseUrl = ref('')
  const testDialogApiKey = ref('')

  const hiddenProviderCodes = ['HUIZHI', 'PRIVATE', 'LOCAL', 'OLLAMA', 'LLAMACPP']
  const internetModels = computed(() => models.value.filter(model =>
    !hiddenProviderCodes.includes(model.provider_code || ''),
  ))
  const selection = useModelSelection(internetModels)
  const {
    search, manageMode, selectedModelIds, filteredModels, isAllSelected,
    clearSelection, toggleManageMode, toggleModelSelection, toggleSelectAll,
  } = selection
  const batchActions = useModelBatchActions(catalog, selectedModelIds, manageMode)
  const providerEditor = useProviderEditor(selectedProvider, catalog.updateProviderFields)
  const {
    editingBaseUrl, editingBaseUrlValue, initialBaseUrl, trackInitialBaseUrl,
    startEditBaseUrl, cancelEditBaseUrl, onApiKeyFocus,
  } = providerEditor

  const syncing = computed(() => loading.value.sync)
  const filteredProviders = computed(() => {
    const items = providers.value.filter(provider =>
      !['HUIZHI', 'PRIVATE', 'LOCAL', 'OLLAMA', 'LLAMACPP'].includes(provider.provider_code || '')
      && !provider.gateway_id,
    )
    const query = providerSearchQuery.value.trim().toLowerCase()
    return query ? items.filter(provider => provider.name.toLowerCase().includes(query)) : items
  })

  const sourceCategoryOptions = [
    { value: 'online', label: '互联网', icon: 'internet' },
  ]
  const getSourceCategoryIcon = (category?: string) =>
    sourceCategoryOptions.find(option => option.value === category)?.icon || 'internet'

  function ratingKey(modelId: string | undefined, providerCode: string) {
    return modelRatingKey(modelId, providerCode)
  }

  async function changeSourceCategory(category: string) {
    if (!selectedProvider.value) return
    try {
      await catalog.updateProviderFields(selectedProvider.value.id, { source_category: category })
    } catch (cause) {
      console.warn('Update source_category failed:', cause)
    }
  }

  async function handleDeleteModel(model: RawModelResponse) {
    try {
      await ElMessageBox.confirm(
        t('modelManage.confirmDeleteModel', { name: model.model_id || model.display_name || model.id }),
        t('modelManage.deleteConfirmTitle'),
        {
          confirmButtonText: t('common.confirm'), cancelButtonText: t('common.cancel'),
          type: 'warning', customClass: 'delete-confirm-dialog',
          cancelButtonClass: 'delete-confirm-cancel-btn',
          closeOnClickModal: false,
        },
      )
      await catalog.removeModels([model.id])
      ElMessage.success(t('modelManage.deleteModelSuccess'))
    } catch { /* cancelled or failed: preserve existing behavior */ }
  }

  async function toggleModel(model: RawModelResponse) {
    try {
      await catalog.toggleModelEnabled(model)
    } catch (cause) {
      console.warn('Toggle model failed:', cause)
    }
  }

  function handleEditModelLabel(model: RawModelResponse) {
    if (!model?.id && !model?.model_id) {
      ElMessage.warning('模型数据异常，无法编辑')
      return
    }
    editTargetModel.value = model
    showEditModelDialog.value = true
  }

  function handleEditModelSaved(data: { id: string; display_name: string; model_label: string | null; context_window: number | string; mxcs: unknown }) {
    catalog.applyModelEdit(data)
  }

  async function confirmBaseUrl() {
    try {
      if (await providerEditor.confirmBaseUrl()) ElMessage.success(t('modelManage.updateAddrSuccess'))
    } catch (cause) {
      console.warn('保存 API 地址失败:', cause)
    }
  }

  async function resetBaseUrl() {
    try {
      await providerEditor.resetBaseUrl()
    } catch (cause) {
      console.warn('重置 API 地址失败:', cause)
    }
  }

  async function onApiKeyBlur(newValue: string | null) {
    try {
      const result = await providerEditor.saveApiKey(newValue)
      if (result === 'saved') ElMessage.success(t('modelManage.apiKeySaved'))
    } catch (cause: any) {
      ElMessage.error(cause?.message || t('modelManage.apiKeySaveFailed'))
    }
  }

  function closeMoreMenu(event: MouseEvent) {
    if (!(event.target as Element).closest('.more-menu-wrapper')) showMoreMenu.value = false
  }

  onMounted(async () => {
    await catalog.initialize()
    window.addEventListener('click', closeMoreMenu)
  })
  onActivated(async () => {
    try {
      await catalog.refreshCurrentContext({
        providers: true,
        gatewayProviders: fixedTab.value === 'gateway',
      })
    } catch (cause) {
      console.warn('Refresh model context failed:', cause)
    }
  })
  onUnmounted(() => window.removeEventListener('click', closeMoreMenu))

  async function selectFixedTab(tab: ModelCatalogTab) {
    try {
      await catalog.selectFixedTab(tab)
    } catch (cause) {
      console.warn('Select model tab failed:', cause)
    }
  }

  async function selectProvider(provider: RawProviderResponse) {
    try {
      await catalog.selectProvider(provider)
    } catch (cause) {
      console.warn('Select provider failed:', cause)
    }
  }

  watch(activeGatewayProviderId, async providerId => {
    try {
      await catalog.selectGatewayProviderById(providerId)
    } catch (cause) {
      console.warn('Select gateway provider failed:', cause)
    }
  })

  function openTestDialog(provider: RawProviderResponse | GatewayProvider) {
    testDialogProvider.value = provider
    testDialogBaseUrl.value = provider.base_url || ''
    testDialogApiKey.value = provider.api_key || ''
    showTestDialog.value = true
  }

  function onTestSaved(apiKey: string) {
    if (testDialogProvider.value) testDialogProvider.value.api_key = apiKey
    notifyChatModelCatalogChanged({ kind: 'provider-credentials-updated', providerId: testDialogProvider.value?.id })
    ElMessage.success(t('modelManage.testSuccessWithDialog'))
  }

  async function handleProviderSaved() {
    await catalog.refreshProviders()
    notifyChatModelCatalogChanged({ kind: 'provider-added' })
  }
  async function handleGatewaySaved() {
    await catalog.refreshGateways()
    notifyChatModelCatalogChanged({ kind: 'gateway-added' })
  }
  async function handleAddModelSaved() {
    await catalog.refreshCurrentModels()
    notifyChatModelCatalogChanged({ kind: 'model-added' })
  }
  async function handleManualAddSaved() {
    await catalog.refreshCurrentModels()
    notifyChatModelCatalogChanged({ kind: 'model-added' })
  }
  async function handleModelsImported() {
    await catalog.refreshCurrentContext({ providers: true, gatewayProviders: fixedTab.value === 'gateway' })
    notifyChatModelCatalogChanged({ kind: 'models-imported' })
  }

  async function handleGatewayProviderSaved() {
    await catalog.loadGatewayProviders()
    notifyChatModelCatalogChanged({ kind: 'gateway-provider-added' })
    const last = gatewayProviders.value.at(-1)
    if (last) await catalog.selectGatewayProvider(last)
  }

  async function handleEditProviderSaved(provider: ProviderEditorTarget) {
    await catalog.refreshProviders()
    notifyChatModelCatalogChanged({ kind: 'provider-updated', providerId: provider.id })
    const updated = providers.value.find(item => item.id === provider.id)
    if (updated) selectedProvider.value = updated
  }

  async function handleDeleteGatewayProvider(provider: RawProviderResponse | GatewayProvider) {
    try {
      await catalog.removeProvider(provider)
      ElMessage.success(t('modelManage.deleteSuccess'))
    } catch (cause: any) {
      ElMessage.error(t('modelManage.deleteFailed', { msg: cause.message || String(cause) }))
    }
  }

  function openEditGatewayProvider(provider: RawProviderResponse | GatewayProvider) {
    editTargetProvider.value = {
      id: provider.id, name: provider.name || '',
      provider_type: provider.provider_type || 'openai',
      base_url: provider.base_url || '', api_key: provider.api_key || '',
    }
    showEditProvider.value = true
  }

  async function syncModels() {
    try {
      await catalog.loadSyncPreview()
      showSyncPreview.value = true
    } catch (cause: any) {
      console.error('Sync preview failed:', cause)
      ElMessage.error(t('modelManage.syncFail', { msg: cause.message || String(cause) }))
    }
  }

  async function handleSyncModels() {
    showMoreMenu.value = false
    if (syncing.value) return
    try {
      await ElMessageBox.confirm(t('modelManage.syncConfirm'), '', {
        confirmButtonText: t('common.confirm'), cancelButtonText: t('common.cancel'),
        customClass: 'sync-models-confirm', cancelButtonClass: 'sync-models-confirm-cancel',
        showClose: true,
        closeOnClickModal: false,
        beforeClose: async (action, instance, done) => {
          if (action !== 'confirm') return done()
          instance.confirmButtonLoading = true
          instance.confirmButtonText = t('modelManage.syncing')
          await syncModels()
          done()
          setTimeout(() => { instance.confirmButtonLoading = false }, 300)
        },
      })
    } catch { /* cancelled */ }
  }

  async function handleSyncSynced(result: SyncResult) {
    ElMessage.success(t('modelManage.syncSuccess', {
      providers: result.providers_created,
      inserted: result.models_inserted,
      updated: result.models_updated,
    }))
    await catalog.refreshCurrentContext({ providers: true, gatewayProviders: fixedTab.value === 'gateway' })
  }

  async function exportModels() {
    showMoreMenu.value = false
    try {
      const includeCredentials = ref(true)
      await ElMessageBox.confirm(
        h('div', { class: 'models-export-confirm' }, [
          h(ElCheckbox, {
            modelValue: includeCredentials.value,
            'onUpdate:modelValue': (value: boolean | string | number) => {
              includeCredentials.value = Boolean(value)
            },
          }, () => t('modelManage.exportIncludeCredentials')),
          h('p', { style: 'margin: 10px 0 0; color: var(--text-secondary); line-height: 1.5;' },
            t('modelManage.exportCredentialWarning')),
        ]),
        t('modelManage.exportModels'),
        {
          confirmButtonText: t('common.confirm'),
          cancelButtonText: t('common.cancel'),
          type: 'warning',
          customClass: 'models-manage-msgbox',
          cancelButtonClass: 'btn-cancel',
          closeOnClickModal: false,
        },
      )
      const jsonData = await catalog.getModelsForExport(includeCredentials.value)
      const { save } = await import('@tauri-apps/plugin-dialog')
      const { writeTextFile } = await import('@tauri-apps/plugin-fs')
      const filePath = await save({
        title: t('modelManage.exportModels'), defaultPath: 'models.json',
        filters: [{ name: 'JSON', extensions: ['json'] }],
      })
      if (!filePath) return
      await writeTextFile(filePath, JSON.stringify(jsonData, null, 2))
      ElMessage.success(t('modelManage.exportSuccess', { count: jsonData.length }))
    } catch (cause: any) {
      if (cause === 'cancel' || cause === 'close') return
      ElMessage.error(t('modelManage.exportFail', { msg: cause.message || String(cause) }))
    }
  }

  function handleShowImport() {
    showMoreMenu.value = false
    showImportDialog.value = true
  }

  async function shareFn() {
    if (!selectedProvider.value) return
    const info = JSON.stringify({
      name: selectedProvider.value.name,
      provider_type: selectedProvider.value.provider_type,
      base_url: selectedProvider.value.base_url,
    }, null, 2)
    try {
      await navigator.clipboard.writeText(info)
      ElMessage.success(t('modelManage.copyProviderSuccess'))
    } catch {
      ElMessage.error(t('modelManage.copyFail'))
    }
  }

  function settingFn() {
    if (!selectedProvider.value) return
    editTargetProvider.value = {
      id: selectedProvider.value.id, name: selectedProvider.value.name || '',
      provider_type: selectedProvider.value.provider_type || 'openai',
      base_url: selectedProvider.value.base_url || '', api_key: selectedProvider.value.api_key || '',
    }
    showEditProvider.value = true
  }

  async function toggleProviderEnabled(provider: RawProviderResponse | GatewayProvider) {
    try {
      await catalog.toggleProviderEnabled(provider)
      ElMessage.success(t('modelManage.operationSuccess'))
    } catch (cause: any) {
      ElMessage.error(t('modelManage.operationFailed', { msg: cause.message || String(cause) }))
    }
  }

  function handleDeleteProvider() {
    if (selectedProvider.value) showDeleteProviderDialog.value = true
  }

  async function confirmDeleteProvider() {
    if (!selectedProvider.value) return
    try {
      await catalog.removeProvider(selectedProvider.value)
      clearSelection()
      showDeleteProviderDialog.value = false
      ElMessage.success(t('modelManage.deleteSuccess'))
    } catch (cause: any) {
      ElMessage.error(t('modelManage.deleteFailed', { msg: cause.message || String(cause) }))
    }
  }

  async function batchToggleModels(enabled: boolean) {
    try {
      if (await batchActions.batchToggleModels(enabled)) {
        ElMessage.success(enabled ? t('modelManage.batchEnabled') : t('modelManage.batchDisabled'))
      }
    } catch (cause: any) {
      ElMessage.error(t('modelManage.operationFailed', { msg: cause.message || String(cause) }))
    }
  }

  async function batchDeleteModels() {
    if (selectedModelIds.value.size === 0 || !selectedProvider.value) return
    try {
      await ElMessageBox.confirm(
        t('modelManage.confirmBatchDelete', { n: selectedModelIds.value.size }),
        t('modelManage.deleteConfirmTitle'),
        {
          confirmButtonText: t('common.confirm'), cancelButtonText: t('common.cancel'),
          type: 'warning', customClass: 'delete-confirm-dialog',
          cancelButtonClass: 'delete-confirm-cancel-btn',
          closeOnClickModal: false,
        },
      )
      if (await batchActions.batchDeleteModels()) ElMessage.success(t('modelManage.batchDeleted'))
    } catch { /* cancelled or failed: preserve existing behavior */ }
  }

  return {
    themeStore, viewMode,
    providers, selectedProvider, models, modelRatings, gateways, gatewayProviders,
    activeGatewayProviderId, fixedTab, syncPreview,
    providerSearchQuery, activeTab, showApiKey, showMoreMenu, showImportDialog,
    showSyncPreview, showTestDialog, showEditModelDialog, showAdd, showEditProvider,
    showAddModel, showManualAddModel, showAddGatewayDialog, showAddGatewayProviderDialog,
    showDeleteProviderDialog, editTargetProvider, editTargetModel, testDialogProvider,
    testDialogBaseUrl, testDialogApiKey, syncing,
    search, manageMode, selectedModelIds, filteredModels, isAllSelected, filteredProviders,
    editingBaseUrl, editingBaseUrlValue, initialBaseUrl,
    sourceCategoryOptions, getSourceCategoryIcon, ratingKey,
    changeSourceCategory, handleDeleteModel, toggleModel, handleEditModelLabel,
    handleEditModelSaved, trackInitialBaseUrl, startEditBaseUrl, confirmBaseUrl,
    cancelEditBaseUrl, resetBaseUrl, onApiKeyFocus, onApiKeyBlur,
    handleSyncModels, exportModels, handleShowImport, selectFixedTab,
    selectProvider, openTestDialog, onTestSaved,
    handleProviderSaved, handleGatewaySaved, handleGatewayProviderSaved,
    handleEditProviderSaved, handleAddModelSaved, handleManualAddSaved,
    handleModelsImported, handleDeleteGatewayProvider, openEditGatewayProvider,
    syncModels, handleSyncSynced, shareFn, settingFn, toggleProviderEnabled,
    handleDeleteProvider, confirmDeleteProvider, toggleManageMode,
    toggleModelSelection, toggleSelectAll, batchToggleModels, batchDeleteModels,
  }
}
