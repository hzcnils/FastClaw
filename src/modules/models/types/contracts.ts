/**
 * 模型管理模块当前 IPC 契约基线。
 *
 * Raw* 类型描述现有 Rust command 的实际返回；阶段 1 的 API 适配层应负责
 * 将原始响应转换成页面使用的领域对象，不允许 Vue 组件继续自行猜测字段。
 */

export type ModelType = 'chat' | 'embedding' | 'rerank' | 'image' | 'tts' | 'stt' | string
export type SourceCategory = 'local' | 'online' | 'huizhi' | 'independent' | string

export interface RawProviderResponse {
  id: string
  name: string
  provider_type: string
  base_url: string
  /** 管理模块需要返回完整值；数据库字段名 api_key_ref 不应泄漏到页面。 */
  api_key: string | null
  is_local: boolean
  is_enabled: boolean
  provider_code: string | null
  source_category: SourceCategory
  extra_config: string | null
  gateway_id?: string | null
}

export interface ProviderSummary {
  id: string
  name: string
  provider_type: string
  base_url: string
  has_api_key: boolean
  is_local: boolean
  is_enabled: boolean
  provider_code: string | null
  source_category: SourceCategory
}

export interface ProviderRuntimeConfig {
  id: string
  provider_code: string
  base_url: string
  api_key: string | null
}

export interface RawModelResponse {
  id: string
  model_id: string
  model_name?: string
  display_name: string
  model_type: ModelType
  is_enabled: boolean
  model_label: string
  mxcs: string | null
  context_window: number | null
  provider_code: string
  supports_vision: boolean
}

export interface RawGatewayResponse {
  id: string
  name: string
  description: string | null
  sort_order: number
  is_enabled: boolean
}

export interface GatewayProvider extends RawProviderResponse {
  _gateway_id?: string
  _gateway_name?: string
}

export interface GatewayWithProviders extends RawGatewayResponse {
  providers: RawProviderResponse[]
}

export interface RemoteModel {
  id: string
  provider_code: string
  model_id?: string
  model_name: string
  display_name?: string
  model_type: ModelType
  model_label?: string
  context_window?: number
  context_length?: number
  max_output?: number
  input_price?: number | null
  output_price?: number | null
  is_enabled?: boolean
  _exists?: boolean
  tags?: string[]
  extra_config?: string
  mxcs?: string | null
}

export interface ModelRating {
  model_id: string
  provider_code: string
  likes: number
  dislikes: number
  score: number
  tier: string
}

export interface RankedModel extends ModelRating {
  rank: number
}

export interface RawModelExportResponse {
  id: string
  model_id: string
  display_name: string
  model_type: ModelType
  is_enabled: boolean
  model_label: string
  mxcs: string | null
  context_window: number | null
  temperature: number | null
  top_p: number | null
  extra_config: string | null
  provider_code: string | null
  base_url: string
  api_key_ref: string | null
}

/** models.json 的对外导入导出格式，字段名保持既有 camelCase 契约。 */
export interface ModelExportItem {
  id: string
  type: string
  model: string
  provider: string
  name: string
  responseLimit: number | null
  temperature: number | null
  topP: number | null
  apiKey: string | null
  secretKey: string | null
  baseUrl: string
  endpoint: string | null
  geminiLocation: string | null
  geminiProject: string | null
  azureDeploymentName: string | null
  imageSize: string | null
  imageQuality: string | null
  imageStyle: string | null
  dimension: string | null
  modelLabel: string
  source: string
  mxcs: string | null
}

export type ModelImportItem = Partial<ModelExportItem>

export interface SyncResult {
  providers_created: number
  models_inserted: number
  models_updated: number
}

export interface ProviderReuse {
  json_name: string
  db_name: string
  db_display_name: string
  match_level: string
}

export interface ProviderNew {
  name: string
  model_count: number
}

export interface UrlConflict {
  provider_name: string
  provider_display_name: string | null
  urls: string[]
}

export interface MergeHint {
  db_name: string
  db_display_name: string
  json_names: string[]
}

export interface SyncPreview {
  reused_providers: ProviderReuse[]
  new_providers: ProviderNew[]
  models_insert: number
  models_update: number
  base_url_conflicts: UrlConflict[]
  merge_hints: MergeHint[]
}

export interface ProviderUpdatePatch {
  name?: string
  provider_type?: string
  base_url?: string
  api_key_ref?: string | null
  is_local?: boolean
  is_enabled?: boolean
  source_category?: SourceCategory
  extra_config?: string
}

export interface ModelUpdatePatch {
  display_name?: string | null
  model_type?: ModelType
  context_window?: number
  max_output?: number
  input_price?: number
  output_price?: number
  model_label?: string | null
  is_enabled?: boolean
  temperature?: number
  top_p?: number
  mxcs?: string | null
}

export interface AddProviderInput {
  name: string
  providerType: string
  baseUrl: string
  apiKey?: string | null
  isLocal?: boolean
  sourceCategory?: SourceCategory
  gatewayId?: string | null
}

export interface AddGatewayInput {
  name: string
  description?: string | null
}

export interface TestConnectionInput {
  providerCode: string
  testApiUrl?: string | null
  testApiKey?: string | null
}

export interface TestConnectionResult {
  success: boolean
  latency_ms: number
  message: string
}

export interface GetModelRatingsInput {
  scope?: 'global' | 'me' | string
  userId?: string | null
}

export const MODEL_MANAGEMENT_COMMANDS = {
  listProviders: 'list_providers',
  listProviderSummaries: 'list_provider_summaries',
  getProviderRuntimeConfig: 'get_provider_runtime_config',
  addProvider: 'add_provider',
  updateProvider: 'update_provider',
  deleteProvider: 'delete_provider',
  listGateways: 'list_gateways',
  addGateway: 'add_gateway',
  listGatewayProviders: 'list_gateway_providers',
  listGatewaysWithProviders: 'list_gateways_with_providers',
  listModels: 'list_models',
  fetchRemoteModels: 'fetch_remote_models',
  saveRemoteModels: 'save_remote_models',
  updateModels: 'update_models',
  deleteModels: 'delete_models',
  testConnection: 'test_connection',
  syncPreview: 'sync_preview',
  syncExecute: 'sync_execute',
  listModelsForExport: 'list_models_for_export',
  exportModels: 'export_models',
  importModelsJson: 'import_models_json',
  getModelRatings: 'get_model_ratings',
  getModelRanking: 'get_model_ranking',
} as const

/** 数据库和导入冲突判定使用的模型业务键。 */
export function modelIdentityKey(providerCode: string, modelId: string): string {
  return `${providerCode}::${modelId}`
}

/** 页面评分映射沿用现有双下划线格式。 */
export function modelRatingKey(modelId: string | undefined, providerCode: string): string {
  return `${modelId || ''}__${providerCode}`
}
