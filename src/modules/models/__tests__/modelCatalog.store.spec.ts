import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { resetInvokeAdapter, setInvokeAdapter, type InvokeAdapter } from '@/shared/tauri-ipc'
import { useModelCatalogStore } from '../stores/modelCatalog.store'

const provider = {
  id: 'provider-1', name: 'Local', provider_type: 'ollama', base_url: 'old',
  api_key: 'sk-visible', is_local: true, is_enabled: true,
  provider_code: 'LOCAL', source_category: 'local', extra_config: '{}', gateway_id: null,
}
const model = {
  id: 'model-1', model_id: 'gpt-4o', display_name: 'GPT-4o', model_type: 'chat',
  is_enabled: true, model_label: '', mxcs: null, context_window: null,
  provider_code: 'LOCAL', supports_vision: true,
}

interface Call { command: string; args?: Record<string, unknown> }

function install(handler?: (command: string, args?: Record<string, unknown>) => unknown) {
  const calls: Call[] = []
  const adapter: InvokeAdapter = async <T>(command: string, args?: Record<string, unknown>) => {
    calls.push({ command, args })
    if (handler) return await handler(command, args) as T
    const defaults: Record<string, unknown> = {
      list_providers: [provider], list_models: [model], get_model_ratings: [],
      list_gateways: [], list_gateway_providers: [], list_gateways_with_providers: [], update_provider: undefined,
      update_models: undefined, delete_models: undefined, delete_provider: undefined,
    }
    return defaults[command] as T
  }
  setInvokeAdapter(adapter)
  return calls
}

beforeEach(() => setActivePinia(createPinia()))
afterEach(() => resetInvokeAdapter())

describe('model catalog store', () => {
  it('loads providers, models and placeholder ratings through one catalog boundary', async () => {
    install()
    const store = useModelCatalogStore()
    await store.initialize()

    expect(store.providers[0].api_key).toBe('sk-visible')
    expect(store.models).toEqual([model])
    expect(store.modelRatings['gpt-4o__LOCAL']).toMatchObject({ tier: 'none', score: 0 })
  })

  it('annotates gateway providers and restores the selected provider during refresh', async () => {
    const gatewayProvider = { ...provider, id: 'gateway-provider', provider_code: 'GATEWAY_PROVIDER' }
    const calls = install((command) => {
      if (command === 'list_gateways_with_providers') return [{ id: 'gateway-1', name: 'Gateway', description: null, sort_order: 0, is_enabled: true, providers: [gatewayProvider] }]
      if (command === 'list_models') return []
      if (command === 'get_model_ratings') return []
      if (command === 'list_providers') return [provider]
      return undefined
    })
    const store = useModelCatalogStore()
    await store.selectFixedTab('gateway')

    expect(store.gatewayProviders[0]).toMatchObject({
      id: 'gateway-provider', _gateway_id: 'gateway-1', _gateway_name: 'Gateway',
    })
    expect(store.selectedProvider?.id).toBe('gateway-provider')
    expect(calls.filter(call => call.command === 'list_gateways_with_providers')).toHaveLength(1)
    expect(calls.filter(call => call.command === 'list_gateway_providers')).toHaveLength(0)

    await store.refreshCurrentContext({ gatewayProviders: true })
    expect(store.selectedProvider?.id).toBe('gateway-provider')
    expect(calls.filter(call => call.command === 'list_gateways_with_providers')).toHaveLength(2)
  })

  it('refreshes model state after a batch patch', async () => {
    const calls = install()
    const store = useModelCatalogStore()
    await store.initialize()
    await store.patchModels(['model-1'], { is_enabled: false })

    expect(calls).toContainEqual({
      command: 'update_models',
      args: { ids: ['model-1'], updates: { is_enabled: false } },
    })
    expect(calls.filter(call => call.command === 'list_models')).toHaveLength(2)
  })
})
