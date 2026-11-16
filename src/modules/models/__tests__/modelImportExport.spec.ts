import { describe, expect, it } from 'vitest'
import {
  compareImportedModels,
  mapModelExportItem,
  parseImportedModels,
} from '../services/modelImportExport'
import type { RawModelExportResponse, RawProviderResponse } from '../types/contracts'

const dbModel: RawModelExportResponse = {
  id: 'OPENAI-gpt-4o',
  model_id: 'gpt-4o',
  display_name: 'GPT-4o',
  model_type: 'chat',
  is_enabled: true,
  model_label: '视觉,工具,推理',
  mxcs: null,
  context_window: 128000,
  temperature: 0.7,
  top_p: 1,
  extra_config: JSON.stringify({ responseLimit: 4096, source: 'OPENAI', dimension: '1024' }),
  provider_code: 'OPENAI',
  base_url: 'https://api.openai.com/v1',
  api_key_ref: 'sk-exported',
}

const provider: RawProviderResponse = {
  id: 'provider-openai',
  name: 'OpenAI',
  provider_type: 'openai',
  base_url: dbModel.base_url,
  api_key: 'sk-management-visible',
  is_local: false,
  is_enabled: true,
  provider_code: 'OPENAI',
  source_category: 'online',
  extra_config: null,
  gateway_id: null,
}

describe('model import/export service', () => {
  it('maps the joined export response without a second provider lookup', () => {
    expect(mapModelExportItem(dbModel)).toMatchObject({
      model: 'gpt-4o',
      provider: 'OPENAI',
      baseUrl: 'https://api.openai.com/v1',
      apiKey: 'sk-exported',
      responseLimit: 4096,
      dimension: '1024',
    })
  })

  it('tolerates malformed extra_config', () => {
    expect(mapModelExportItem({ ...dbModel, extra_config: '{bad' })).toMatchObject({
      responseLimit: null,
      source: 'OPENAI',
    })
  })

  it('parses a non-empty models array and rejects invalid roots', () => {
    expect(parseImportedModels('[{"id":"one"}]')).toHaveLength(1)
    expect(() => parseImportedModels('{}')).toThrow('INVALID_MODELS_JSON')
    expect(() => parseImportedModels('[]')).toThrow('INVALID_MODELS_JSON')
  })

  it('classifies existing, changed and new imported models', () => {
    const unchanged = mapModelExportItem(dbModel)
    const changed = { ...unchanged, id: undefined, temperature: 0.2 }
    const added = { ...unchanged, id: 'NEW-new-model', provider: 'NEW', model: 'new-model' }
    const result = compareImportedModels([unchanged, changed, added], [dbModel], [provider])

    expect(result.existingModels).toHaveLength(1)
    expect(result.conflictModels).toHaveLength(1)
    expect(result.conflictModels[0].diffFields).toContain('temperature')
    expect(result.newModels).toEqual([added])
    expect(result.newProviderCodes).toEqual(['NEW'])
  })
})
