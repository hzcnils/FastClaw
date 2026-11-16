import { modelIdentityKey } from '../types/contracts'
import type {
  ModelExportItem,
  ModelImportItem,
  RawModelExportResponse,
  RawProviderResponse,
} from '../types/contracts'

export type ImportedModel = ModelImportItem

export interface ModelConflictItem {
  json: Record<string, unknown>
  db: Record<string, unknown>
  diffFields: string[]
  keepJson: boolean
}

export interface ModelCompareResult {
  newModels: ImportedModel[]
  conflictModels: ModelConflictItem[]
  existingModels: ImportedModel[]
  newProviderCodes: string[]
}

export function parseExtraConfig(value: string | null | undefined): Record<string, unknown> {
  if (!value) return {}
  try {
    const parsed: unknown = JSON.parse(value)
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed)
      ? parsed as Record<string, unknown>
      : {}
  } catch {
    return {}
  }
}

function nullableString(value: unknown): string | null {
  return typeof value === 'string' ? value : null
}

function nullableNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

export function mapModelExportItem(model: RawModelExportResponse): ModelExportItem {
  const extra = parseExtraConfig(model.extra_config)
  return {
    id: model.id,
    type: (model.model_type || 'chat').toUpperCase(),
    model: model.model_id,
    provider: model.provider_code || '',
    name: model.display_name,
    responseLimit: nullableNumber(extra.responseLimit),
    temperature: model.temperature ?? null,
    topP: model.top_p ?? null,
    apiKey: model.api_key_ref ?? null,
    secretKey: nullableString(extra.secretKey),
    baseUrl: model.base_url || '',
    endpoint: nullableString(extra.endpoint),
    geminiLocation: nullableString(extra.geminiLocation),
    geminiProject: nullableString(extra.geminiProject),
    azureDeploymentName: nullableString(extra.azureDeploymentName),
    imageSize: nullableString(extra.imageSize),
    imageQuality: nullableString(extra.imageQuality),
    imageStyle: nullableString(extra.imageStyle),
    dimension: nullableString(extra.dimension),
    modelLabel: model.model_label || '',
    source: nullableString(extra.source) || model.provider_code || '',
    mxcs: model.mxcs ?? null,
  }
}

export function parseImportedModels(text: string): ImportedModel[] {
  const parsed: unknown = JSON.parse(text)
  if (!Array.isArray(parsed) || parsed.length === 0) {
    throw new Error('INVALID_MODELS_JSON')
  }
  if (parsed.some(item => !item || typeof item !== 'object' || Array.isArray(item))) {
    throw new Error('INVALID_MODELS_JSON')
  }
  return parsed as ImportedModel[]
}

function valuesEqual(a: unknown, b: unknown): boolean {
  const left = a ?? null
  const right = b ?? null
  if (left === null && right === null) return true
  if (left === null || right === null) return false
  if (typeof left === 'number' && typeof right === 'number') {
    return Math.abs(left - right) < 0.001
  }
  return String(left) === String(right)
}

export function compareImportedModels(
  importedModels: ImportedModel[],
  dbModels: RawModelExportResponse[],
  dbProviders: RawProviderResponse[],
): ModelCompareResult {
  const dbProviderCodes = new Set(
    dbProviders.flatMap(provider => provider.provider_code ? [provider.provider_code] : []),
  )
  const dbById = new Map(dbModels.map(model => [model.id, model]))
  const dbByKey = new Map(
    dbModels.map(model => [modelIdentityKey(model.provider_code || '', model.model_id), model]),
  )
  const importedProviderCodes = new Set(
    importedModels
      .map(model => typeof model.provider === 'string' ? model.provider.trim() : '')
      .filter(Boolean),
  )

  const newModels: ImportedModel[] = []
  const conflictModels: ModelConflictItem[] = []
  const existingModels: ImportedModel[] = []
  const compareFields = ['model', 'name', 'type', 'baseUrl', 'temperature', 'topP', 'modelLabel', 'mxcs']

  for (const item of importedModels) {
    const providerCode = typeof item.provider === 'string' ? item.provider.trim() : ''
    const modelId = typeof item.model === 'string' ? item.model : ''
    const dbMatch = (item.id ? dbById.get(item.id) : undefined)
      ?? dbByKey.get(modelIdentityKey(providerCode, modelId))

    if (!dbMatch) {
      newModels.push(item)
      continue
    }

    const dbAsJson: Record<string, unknown> = {
      model: dbMatch.model_id,
      name: dbMatch.display_name,
      type: (dbMatch.model_type || 'chat').toUpperCase(),
      baseUrl: dbMatch.base_url || '',
      temperature: dbMatch.temperature ?? null,
      topP: dbMatch.top_p ?? null,
      modelLabel: dbMatch.model_label || '',
      mxcs: dbMatch.mxcs ?? null,
    }
    const itemAsJson: Record<string, unknown> = {
      model: item.model,
      name: item.name,
      type: (typeof item.type === 'string' ? item.type : 'CHAT').toUpperCase(),
      baseUrl: item.baseUrl || '',
      temperature: item.temperature ?? null,
      topP: item.topP ?? null,
      modelLabel: item.modelLabel || '',
      mxcs: item.mxcs ?? null,
    }
    const diffFields = compareFields.filter(field => !valuesEqual(itemAsJson[field], dbAsJson[field]))

    if (diffFields.length === 0) {
      existingModels.push(item)
    } else {
      conflictModels.push({
        json: { ...itemAsJson, id: item.id, provider: providerCode },
        db: { ...dbAsJson, id: dbMatch.id, provider: dbMatch.provider_code },
        diffFields,
        keepJson: true,
      })
    }
  }

  return {
    newModels,
    conflictModels,
    existingModels,
    newProviderCodes: [...importedProviderCodes].filter(code => !dbProviderCodes.has(code)),
  }
}
