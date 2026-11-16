import { reactive, ref } from 'vue'
import { defineStore } from 'pinia'
import {
  deleteModels as deleteModelsApi,
  deleteProvider as deleteProviderApi,
  getModelRatings,
  getSyncPreview,
  listGateways,
  listGatewaysWithProviders,
  listModels,
  exportModels,
  listProviders,
  updateModels,
  updateProvider,
} from '../api'
import { modelRatingKey } from '../types/contracts'
import { notifyChatModelCatalogChanged } from '../chatCatalog'
import type {
  GatewayProvider,
  ModelExportItem,
  ModelRating,
  ModelUpdatePatch,
  ProviderUpdatePatch,
  RawGatewayResponse,
  RawModelResponse,
  RawProviderResponse,
  SyncPreview,
} from '../types/contracts'

export type ModelCatalogTab = 'all' | 'gateway'
export type SelectedModelProvider = RawProviderResponse | GatewayProvider

export interface RefreshContextOptions {
  providers?: boolean
  gatewayProviders?: boolean
}

const previewModels: RawModelResponse[] = [
  { id: '1', model_id: 'gpt-4o', display_name: 'GPT-4o', model_type: 'chat', model_label: 'OpenAI,推理', is_enabled: true, mxcs: null, context_window: null, provider_code: 'MOCK', supports_vision: true },
  { id: '2', model_id: 'gpt-4-turbo', display_name: 'GPT-4 Turbo', model_type: 'chat', model_label: 'OpenAI', is_enabled: true, mxcs: null, context_window: null, provider_code: 'MOCK', supports_vision: false },
  { id: '3', model_id: 'text-embedding-3-small', display_name: 'Text Embedding 3 Small', model_type: 'embedding', model_label: '嵌入', is_enabled: true, mxcs: null, context_window: null, provider_code: 'MOCK', supports_vision: false },
]

export const useModelCatalogStore = defineStore('modelCatalog', () => {
  const providers = ref<RawProviderResponse[]>([])
  const selectedProvider = ref<SelectedModelProvider | null>(null)
  const models = ref<RawModelResponse[]>([])
  const modelRatings = ref<Record<string, ModelRating>>({})
  const gateways = ref<RawGatewayResponse[]>([])
  const gatewayProviders = ref<GatewayProvider[]>([])
  const activeGatewayProviderId = ref<string | null>(null)
  const fixedTab = ref<ModelCatalogTab | null>('all')
  const syncPreview = ref<SyncPreview | null>(null)
  const loading = reactive({ providers: false, models: false, gateways: false, mutation: false, sync: false })
  const error = ref<unknown>(null)

  let modelsRequestId = 0

  function replaceProviderLocally(providerId: string, patch: ProviderUpdatePatch) {
    const targets: SelectedModelProvider[] = [
      ...providers.value.filter(provider => provider.id === providerId),
      ...gatewayProviders.value.filter(provider => provider.id === providerId),
    ]
    if (selectedProvider.value?.id === providerId && !targets.includes(selectedProvider.value)) {
      targets.push(selectedProvider.value)
    }
    for (const provider of targets) {
      if (patch.name !== undefined) provider.name = patch.name
      if (patch.provider_type !== undefined) provider.provider_type = patch.provider_type
      if (patch.base_url !== undefined) provider.base_url = patch.base_url
      if (patch.api_key_ref !== undefined) provider.api_key = patch.api_key_ref
      if (patch.is_local !== undefined) provider.is_local = patch.is_local
      if (patch.is_enabled !== undefined) provider.is_enabled = patch.is_enabled
      if (patch.source_category !== undefined) provider.source_category = patch.source_category
      if (patch.extra_config !== undefined) provider.extra_config = patch.extra_config
    }
  }

  function buildRatings(items: ModelRating[], currentModels: RawModelResponse[]) {
    const next: Record<string, ModelRating> = {}
    for (const rating of items) next[modelRatingKey(rating.model_id, rating.provider_code)] = rating
    for (const model of currentModels) {
      const key = modelRatingKey(model.model_id, model.provider_code)
      if (!next[key]) {
        next[key] = {
          model_id: model.model_id,
          provider_code: model.provider_code,
          tier: 'none',
          likes: 0,
          dislikes: 0,
          score: 0,
        }
      }
    }
    return next
  }

  async function refreshProviders() {
    loading.providers = true
    error.value = null
    try {
      const next = await listProviders()
      providers.value = Array.isArray(next) ? next : []
      if (selectedProvider.value && fixedTab.value !== 'gateway') {
        selectedProvider.value = providers.value.find(provider => provider.id === selectedProvider.value?.id) ?? null
      }
    } catch (cause) {
      error.value = cause
      throw cause
    } finally {
      loading.providers = false
    }
  }

  async function refreshCurrentModels() {
    const requestId = ++modelsRequestId
    const providerCode = selectedProvider.value?.provider_code ?? null
    const shouldLoadAll = fixedTab.value === 'all' && !providerCode
    if (!providerCode && !shouldLoadAll) {
      models.value = []
      modelRatings.value = {}
      return
    }

    loading.models = true
    error.value = null
    try {
      const [nextModels, ratings] = await Promise.all([
        listModels(providerCode),
        getModelRatings({ scope: 'global' }),
      ])
      if (requestId !== modelsRequestId) return
      models.value = Array.isArray(nextModels) ? nextModels : []
      modelRatings.value = buildRatings(ratings, models.value)
    } catch (cause) {
      if (requestId === modelsRequestId) error.value = cause
      throw cause
    } finally {
      if (requestId === modelsRequestId) loading.models = false
    }
  }

  async function loadGatewayProviders() {
    loading.gateways = true
    error.value = null
    try {
      const groups = await listGatewaysWithProviders()
      gateways.value = groups.map(({ providers: _providers, ...gateway }) => gateway)
      gatewayProviders.value = groups.flatMap(gateway =>
        gateway.providers.map(provider => ({
            ...provider,
            _gateway_id: gateway.id,
            _gateway_name: gateway.name,
        })),
      )
    } catch (cause) {
      error.value = cause
      throw cause
    } finally {
      loading.gateways = false
    }
  }

  async function refreshCurrentContext(options: RefreshContextOptions = {}) {
    if (options.providers) await refreshProviders()
    if (fixedTab.value === 'gateway' && options.gatewayProviders) {
      const currentId = activeGatewayProviderId.value
      await loadGatewayProviders()
      const refreshed = gatewayProviders.value.find(provider => provider.id === currentId)
        ?? gatewayProviders.value[0]
        ?? null
      selectedProvider.value = refreshed
      activeGatewayProviderId.value = refreshed?.id ?? null
    }
    await refreshCurrentModels()
  }

  async function initialize() {
    try {
      await refreshProviders()
      await refreshCurrentModels()
    } catch (cause) {
      console.warn('Tauri not available', cause)
      models.value = previewModels
      modelRatings.value = buildRatings([], previewModels)
    }
  }

  async function selectFixedTab(tab: ModelCatalogTab) {
    fixedTab.value = tab
    if (tab === 'all') {
      selectedProvider.value = null
    } else {
      selectedProvider.value = null
      await loadGatewayProviders()
      const first = gatewayProviders.value[0] ?? null
      selectedProvider.value = first
      activeGatewayProviderId.value = first?.id ?? null
    }
    await refreshCurrentModels()
  }

  async function selectProvider(provider: RawProviderResponse) {
    fixedTab.value = null
    selectedProvider.value = provider
    await refreshCurrentModels()
  }

  async function selectGatewayProvider(provider: GatewayProvider) {
    selectedProvider.value = provider
    activeGatewayProviderId.value = provider.id
    await refreshCurrentModels()
  }

  async function selectGatewayProviderById(providerId: string | null) {
    activeGatewayProviderId.value = providerId
    if (!providerId || fixedTab.value !== 'gateway') return
    const provider = gatewayProviders.value.find(item => item.id === providerId)
    if (provider && selectedProvider.value?.id !== provider.id) await selectGatewayProvider(provider)
  }

  async function updateProviderFields(providerId: string, patch: ProviderUpdatePatch) {
    loading.mutation = true
    error.value = null
    try {
      await updateProvider(providerId, patch)
      replaceProviderLocally(providerId, patch)
      notifyChatModelCatalogChanged({ kind: 'provider-updated', providerId })
    } catch (cause) {
      error.value = cause
      throw cause
    } finally {
      loading.mutation = false
    }
  }

  async function toggleProviderEnabled(provider: SelectedModelProvider) {
    const next = !provider.is_enabled
    await updateProviderFields(provider.id, { is_enabled: next })
  }

  async function removeProvider(provider: SelectedModelProvider) {
    await deleteProviderApi(provider.id)
    notifyChatModelCatalogChanged({ kind: 'provider-removed', providerId: provider.id })
    if (fixedTab.value === 'gateway' || ('_gateway_id' in provider && provider._gateway_id)) {
      gatewayProviders.value = gatewayProviders.value.filter(item => item.id !== provider.id)
      if (selectedProvider.value?.id === provider.id) {
        const first = gatewayProviders.value[0] ?? null
        selectedProvider.value = first
        activeGatewayProviderId.value = first?.id ?? null
      }
      await refreshCurrentModels()
      return
    }
    if (selectedProvider.value?.id === provider.id) selectedProvider.value = null
    models.value = []
    modelRatings.value = {}
    await refreshProviders()
  }

  async function toggleModelEnabled(model: RawModelResponse) {
    const next = model.is_enabled === false
    await updateModels([model.id], { is_enabled: next })
    model.is_enabled = next
    notifyChatModelCatalogChanged({ kind: 'model-toggled', modelId: model.id })
  }

  async function removeModels(ids: string[]) {
    await deleteModelsApi(ids)
    await refreshCurrentModels()
    notifyChatModelCatalogChanged({ kind: 'models-removed', modelIds: ids })
  }

  async function patchModels(ids: string[], patch: ModelUpdatePatch) {
    await updateModels(ids, patch)
    await refreshCurrentModels()
    notifyChatModelCatalogChanged({ kind: 'models-updated', modelIds: ids })
  }

  function applyModelEdit(data: { id: string; display_name: string; model_label: string | null; context_window: number | string; mxcs: unknown }) {
    const target = models.value.find(model => model.id === data.id)
    if (!target) return
    target.display_name = data.display_name
    target.model_label = data.model_label || ''
    const contextWindow = typeof data.context_window === 'number' ? data.context_window : Number(data.context_window)
    target.context_window = Number.isFinite(contextWindow) ? contextWindow : null
    target.mxcs = typeof data.mxcs === 'string' ? data.mxcs : null
    notifyChatModelCatalogChanged({ kind: 'model-edited', modelId: data.id })
  }

  async function loadSyncPreview() {
    loading.sync = true
    error.value = null
    try {
      syncPreview.value = await getSyncPreview()
      return syncPreview.value
    } catch (cause) {
      error.value = cause
      throw cause
    } finally {
      loading.sync = false
    }
  }

  async function getModelsForExport(includeCredentials = true): Promise<ModelExportItem[]> {
    return exportModels(includeCredentials)
  }

  async function refreshGateways() {
    gateways.value = await listGateways()
  }

  return {
    providers, selectedProvider, models, modelRatings, gateways, gatewayProviders,
    activeGatewayProviderId, fixedTab, syncPreview, loading, error,
    initialize, refreshProviders, refreshCurrentModels, refreshCurrentContext,
    loadGatewayProviders, refreshGateways, selectFixedTab, selectProvider,
    selectGatewayProvider, selectGatewayProviderById, updateProviderFields,
    toggleProviderEnabled, removeProvider, toggleModelEnabled,
    removeModels, patchModels, applyModelEdit, loadSyncPreview, getModelsForExport,
  }
})
