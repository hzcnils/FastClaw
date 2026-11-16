import { afterEach, describe, expect, it } from 'vitest'
import { resetInvokeAdapter, setInvokeAdapter } from '@/shared/tauri-ipc'
import {
  addProvider,
  exportModels,
  getProviderRuntimeConfig,
  getModelRatings,
  importModelsJson,
  listModels,
  listProviders,
  listProviderSummaries,
  testConnection,
  updateModels,
  updateProvider,
} from '../api'
import type { InvokeAdapter } from '@/shared/tauri-ipc'

interface InvokeCall {
  command: string
  args: Record<string, unknown> | undefined
}

function installAdapter(responses: Record<string, unknown> = {}) {
  const calls: InvokeCall[] = []
  const adapter: InvokeAdapter = async <T>(command: string, args?: Record<string, unknown>) => {
    calls.push({ command, args })
    return responses[command] as T
  }
  setInvokeAdapter(adapter)
  return calls
}

afterEach(() => resetInvokeAdapter())

describe('model management API adapter', () => {
  it('uses the pinned provider command and preserves the full management response', async () => {
    const provider = {
      id: 'provider-1',
      name: 'Provider',
      provider_type: 'openai',
      base_url: '',
      api_key: 'sk-visible',
      is_local: false,
      is_enabled: true,
      provider_code: 'PROVIDER',
      source_category: 'online',
      extra_config: null,
      gateway_id: null,
    }
    const calls = installAdapter({ list_providers: [provider] })

    await expect(listProviders()).resolves.toEqual([provider])
    expect(calls).toEqual([{ command: 'list_providers', args: {} }])
  })

  it('passes Tauri camelCase arguments when adding a provider', async () => {
    const calls = installAdapter({ add_provider: 'provider-id' })
    const input = {
      name: 'Provider',
      providerType: 'openai',
      baseUrl: '',
      apiKey: 'sk-key',
      isLocal: false,
      sourceCategory: 'online',
      gatewayId: null,
    }

    await expect(addProvider(input)).resolves.toBe('provider-id')
    expect(calls[0]).toEqual({ command: 'add_provider', args: input })
  })

  it('separates provider summaries, targeted runtime credentials and export policy', async () => {
    const summary = { id: 'provider-1', provider_code: 'OPENAI', has_api_key: true }
    const runtime = { id: 'provider-1', provider_code: 'OPENAI', base_url: '', api_key: 'sk-key' }
    const exported = [{ id: 'OPENAI-gpt', apiKey: null, secretKey: null }]
    const calls = installAdapter({
      list_provider_summaries: [summary],
      get_provider_runtime_config: runtime,
      export_models: exported,
    })

    await expect(listProviderSummaries()).resolves.toEqual([summary])
    await expect(getProviderRuntimeConfig('OPENAI')).resolves.toEqual(runtime)
    await expect(exportModels(false)).resolves.toEqual(exported)
    expect(calls).toEqual([
      { command: 'list_provider_summaries', args: {} },
      { command: 'get_provider_runtime_config', args: { providerCode: 'OPENAI' } },
      { command: 'export_models', args: { includeCredentials: false } },
    ])
  })

  it('uses null providerCode to request all models', async () => {
    const calls = installAdapter({ list_models: [] })
    await listModels(null)
    expect(calls[0]).toEqual({ command: 'list_models', args: { providerCode: null } })
  })

  it('keeps api_key_ref inside the provider update patch', async () => {
    const calls = installAdapter()
    await updateProvider('provider-1', { api_key_ref: 'sk-next' })
    expect(calls[0]).toEqual({
      command: 'update_provider',
      args: { id: 'provider-1', updates: { api_key_ref: 'sk-next' } },
    })
  })

  it('passes batch model updates without changing the patch', async () => {
    const calls = installAdapter()
    await updateModels(['one', 'two'], { is_enabled: false })
    expect(calls[0]).toEqual({
      command: 'update_models',
      args: { ids: ['one', 'two'], updates: { is_enabled: false } },
    })
  })

  it('normalizes optional rating arguments at the API boundary', async () => {
    const calls = installAdapter({ get_model_ratings: [] })
    await getModelRatings()
    expect(calls[0]).toEqual({
      command: 'get_model_ratings',
      args: { scope: 'global', userId: null },
    })
  })

  it('passes connection-test and import results through unchanged', async () => {
    const connectionResult = { success: true, latency_ms: 12, message: 'ok' }
    const syncResult = { providers_created: 0, models_inserted: 1, models_updated: 0 }
    const calls = installAdapter({
      test_connection: connectionResult,
      import_models_json: syncResult,
    })

    await expect(testConnection({ providerCode: 'OPENAI', testApiUrl: '', testApiKey: 'key' }))
      .resolves.toEqual(connectionResult)
    await expect(importModelsJson([{ id: 'one', model: 'gpt-4o' }])).resolves.toEqual(syncResult)
    expect(calls.map(call => call.command)).toEqual(['test_connection', 'import_models_json'])
  })

  it('does not swallow transport errors', async () => {
    setInvokeAdapter(async () => { throw new Error('transport failed') })
    await expect(listProviders()).rejects.toThrow('transport failed')
  })
})
