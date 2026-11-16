import { describe, expect, it } from 'vitest'
import {
  MODEL_MANAGEMENT_COMMANDS,
  modelIdentityKey,
  modelRatingKey,
  type ModelExportItem,
  type ModelRating,
  type RawGatewayResponse,
  type RawModelResponse,
  type RawProviderResponse,
  type SyncPreview,
  type SyncResult,
} from '../types/contracts'

const providerFixture = {
  id: 'provider-openai',
  name: 'OpenAI',
  provider_type: 'openai',
  base_url: 'https://api.openai.com/v1',
  api_key: 'sk-management-visible',
  is_local: false,
  is_enabled: true,
  provider_code: 'OPENAI',
  source_category: 'online',
  extra_config: null,
  gateway_id: null,
} satisfies RawProviderResponse

const modelFixture = {
  id: 'OPENAI-gpt-4o',
  model_id: 'gpt-4o',
  display_name: 'GPT-4o',
  model_type: 'chat',
  is_enabled: true,
  model_label: '视觉,工具,推理',
  mxcs: null,
  context_window: 128000,
  provider_code: 'OPENAI',
  supports_vision: true,
} satisfies RawModelResponse

const gatewayFixture = {
  id: 'gateway-1',
  name: 'Default Gateway',
  description: null,
  sort_order: 0,
  is_enabled: true,
} satisfies RawGatewayResponse

const ratingFixture = {
  model_id: 'gpt-4o',
  provider_code: 'OPENAI',
  likes: 5,
  dislikes: 1,
  score: 0.72,
  tier: 'good',
} satisfies ModelRating

const exportFixture = {
  id: 'OPENAI-gpt-4o',
  type: 'CHAT',
  model: 'gpt-4o',
  provider: 'OPENAI',
  name: 'GPT-4o',
  responseLimit: null,
  temperature: 0.7,
  topP: 1,
  apiKey: 'sk-management-visible',
  secretKey: null,
  baseUrl: 'https://api.openai.com/v1',
  endpoint: null,
  geminiLocation: null,
  geminiProject: null,
  azureDeploymentName: null,
  imageSize: null,
  imageQuality: null,
  imageStyle: null,
  dimension: null,
  modelLabel: '视觉,工具,推理',
  source: 'OPENAI',
  mxcs: null,
} satisfies ModelExportItem

describe('model management contract baseline', () => {
  it('keeps the full API key in the management provider response', () => {
    expect(providerFixture.api_key).toBe('sk-management-visible')
  })

  it('uses provider_code and model_id as the model business identity', () => {
    expect(modelIdentityKey(modelFixture.provider_code, modelFixture.model_id)).toBe('OPENAI::gpt-4o')
    expect(modelIdentityKey('AZURE', modelFixture.model_id)).not.toBe(
      modelIdentityKey(modelFixture.provider_code, modelFixture.model_id),
    )
  })

  it('keeps the existing rating lookup key format', () => {
    expect(modelRatingKey(ratingFixture.model_id, ratingFixture.provider_code)).toBe('gpt-4o__OPENAI')
    expect(modelRatingKey(undefined, 'OPENAI')).toBe('__OPENAI')
  })

  it('fixes the external models.json camelCase field contract', () => {
    expect(exportFixture).toMatchObject({
      model: 'gpt-4o',
      provider: 'OPENAI',
      apiKey: 'sk-management-visible',
      baseUrl: 'https://api.openai.com/v1',
      topP: 1,
    })
    expect(exportFixture).not.toHaveProperty('api_key_ref')
  })

  it('captures gateway and nullable response fields', () => {
    expect(gatewayFixture.description).toBeNull()
    expect(providerFixture.gateway_id).toBeNull()
    expect(modelFixture.mxcs).toBeNull()
  })

  it('captures sync preview and result counter names', () => {
    const preview = {
      reused_providers: [],
      new_providers: [],
      models_insert: 2,
      models_update: 1,
      base_url_conflicts: [],
      merge_hints: [],
    } satisfies SyncPreview
    const result = {
      providers_created: 1,
      models_inserted: 2,
      models_updated: 1,
    } satisfies SyncResult

    expect(preview.models_insert).toBe(result.models_inserted)
    expect(preview.models_update).toBe(result.models_updated)
  })

  it('pins the command names used by the module', () => {
    expect(MODEL_MANAGEMENT_COMMANDS.listProviders).toBe('list_providers')
    expect(MODEL_MANAGEMENT_COMMANDS.listProviderSummaries).toBe('list_provider_summaries')
    expect(MODEL_MANAGEMENT_COMMANDS.exportModels).toBe('export_models')
    expect(MODEL_MANAGEMENT_COMMANDS.importModelsJson).toBe('import_models_json')
    expect(new Set(Object.values(MODEL_MANAGEMENT_COMMANDS)).size).toBe(
      Object.values(MODEL_MANAGEMENT_COMMANDS).length,
    )
  })
})
