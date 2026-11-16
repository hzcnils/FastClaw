// ===== API 响应类型 =====
export interface ApiResponse<T> {
  success: boolean;
  data: T;
  message?: string;
  error?: string;
}

export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  page_size: number;
}

// ===== 知识库相关 =====
export interface KnowledgeBase {
  id: string;
  name: string;
  code: string;
  description?: string;
  embedding_model: string;
  embedding_dimension: number;
  index_type: string;
  user_id: string;
  file_count: number;
  chunk_count: number;
  total_size: number;
  status: 'active' | 'inactive' | 'building';
  is_public: boolean;
  default_algorithm?: string;
  visibility?: 'public' | 'private' | 'restricted';
  created_at: string;
  updated_at: string;
}

// ===== 本地文件夹知识库 =====
export interface LocalKnowledgeBase {
  id: string;           // UUID
  name: string;         // 显示名称（用户可自定义，默认取文件夹名）
  path: string;         // 绝对路径
  type: 'local';        // 与远程 KB 区分
  enabled: boolean;     // 启用/禁用开关
  recursive: boolean;   // 是否递归读取子目录（默认 false）
  fileCount?: number;   // 可读文件数（打开下拉框时刷新）
  exists?: boolean;     // 路径是否有效（打开下拉框时检测）
}

export interface CreateKnowledgeBaseRequest {
  name: string;
  code: string;
  description?: string;
  embedding_model?: string;
  embedding_dimension?: number;
  index_type?: string;
  is_public?: boolean;
  default_algorithm?: string;
  user_id?: string;
  chunking_config?: ChunkingConfig;
}

export interface UpdateKnowledgeBaseRequest {
  name?: string;
  code?: string;
  description?: string;
  embedding_model?: string;
  embedding_dimension?: number;
  index_type?: string;
  is_public?: boolean;
  visibility?: 'public' | 'private' | 'restricted';
}

export interface ChunkingConfig {
  strategy?: string;
  size?: number;
  overlap?: number;
  min_size?: number;
  max_size?: number;
}

// ===== 文件相关 =====
export interface FileInfo {
  id: string;
  original_name: string;
  file_ext: string;
  file_size: number;
  file_category: string;
  status: 'pending' | 'uploaded' | 'processing' | 'completed' | 'failed';
  upload_time: string;
  mime_type?: string;
  knowledge_base_id?: string;
  uploader?: string;
  governance_status?: string;
  governance_error?: string;
  vector_status?: string;
  vector_progress?: number;
  vector_progress_message?: string;
  graph_extract_status?: string;
  graph_extract_mention_count?: number;
}

// ===== 治理文档相关 =====
export interface GovernanceDoc {
  id: string;
  raw_file_id: string;
  knowledge_base_id?: string;
  title?: string;
  summary?: string;
  md_content?: string;
  assets_folder?: string;
  process_log?: string;
  word_count: number;
  is_human_verified: boolean;
  verified_by?: string;
  verified_at?: string;
  version: number;
  status: string;
  process_method?: string;
  created_at: string;
  updated_at: string;
}

export interface AssetElement {
  id: string;
  governance_id: string;
  raw_file_id: string;
  element_type: string;
  sort_order: number;
  page_number: number;
  bbox?: string;
  asset_path: string;
  thumbnail_path?: string;
  table_data?: string;
  content_text?: string;
  caption?: string;
  ocr_text?: string;
  created_at: string;
  updated_at: string;
}

export interface UpdateGovernanceRequest {
  md_content: string;
  change_summary?: string;
}

// ===== 切片相关 =====
export interface ChunkInfo {
  id: string;
  governance_id: string;
  raw_file_id: string;
  chunk_index: number;
  chunk_content: string;
  chunk_size?: number;
  has_embedding: boolean;
  source_location?: unknown;
  created_at: string;
}

export interface ChunkPreview {
  index: number;
  content: string;
  char_count: number;
  title_path: string[];
  has_table: boolean;
  has_code: boolean;
  prev_context?: string;
  next_context?: string;
}

export interface ChunkStats {
  total_chunks: number;
  total_chars: number;
  avg_size: number;
  min_size: number;
  max_size: number;
  std_dev: number;
  size_distribution: Record<string, number>;
  oversized_count: number;
  undersized_count: number;
}

export interface AlgorithmChunkResult {
  algorithm: string;
  description: string;
  chunk_count: number;
  chunks: ChunkPreview[];
  stats: ChunkStats;
  elapsed_ms: number;
  error?: string;
}

export interface ChunkCompareRequest {
  algorithms: string[];
  content?: string;
  target_size?: number;
  min_size?: number;
  max_size?: number;
  overlap?: number;
}

export interface ChunkCompareResponse {
  original_char_count: number;
  results: AlgorithmChunkResult[];
  recommended: string;
  recommendation_reason: string;
  total_elapsed_ms: number;
}

export interface AlgorithmInfo {
  code: string;
  name: string;
  description: string;
  category: string;
  supports_overlap: boolean;
  supports_hierarchy: boolean;
}

export interface ChunkDetail {
  chunk_id: string;
  content: string;
  chunk_index: number;
  total_chunks: number;
  source: ChunkSourceInfo;
  metadata?: unknown;
  media_refs: MediaRef[];
}

export interface ChunkSourceInfo {
  file_id: string;
  file_name: string;
  page_numbers: number[];
  governance_id: string;
  knowledge_base_id?: string;
  knowledge_base_name?: string;
}

export interface ChunkContext {
  current: ChunkContextItem;
  previous?: ChunkContextItem;
  next?: ChunkContextItem;
  total_chunks: number;
}

export interface ChunkContextItem {
  chunk_id: string;
  chunk_index: number;
  content: string;
  page_number?: number;
}

export interface MediaRef {
  id: string;
  type: string;
  url: string;
}

// ===== 检索相关 =====
export interface SearchRequest {
  query: string;
  knowledge_base_id?: string;
  top_k?: number;
  strategy?: 'hybrid' | 'vector' | 'keyword';
  rerank?: boolean;
}

export interface SearchResponse {
  results: SearchResultItem[];
  total: number;
  elapsed_ms: number;
}

export interface SearchResultItem {
  chunk_id: string;
  content: string;
  score: number;
  source: ChunkSourceInfo;
}

// ===== 召回 =====
export interface KnowledgeRecallRequest {
  query: string;
  knowledge_base_ids?: string[];
  top_k?: number;
}

export interface KnowledgeRecallResponse {
  results: any[];
  elapsed_ms: number;
}

export interface FaqRecallRequest {
  query: string;
  knowledge_base_id?: string;
  top_k?: number;
}

export interface FaqRecallResponse {
  results: any[];
  elapsed_ms: number;
}

// ===== 知识统计 =====
export interface KnowledgeStats {
  total_chunks: number;
  total_files: number;
  total_knowledge_bases: number;
  embedding_coverage: number;
}

// ===== 评估 =====
export interface EvaluateSampleRequest {
  query: string;
  knowledge_base_id?: string;
  expected_answer?: string;
}
export interface EvaluateResponse {
  score: number;
  details: any;
}
export interface EvaluateBatchRequest {
  items: EvaluateSampleRequest[];
}
export interface EvaluateBatchResponse {
  results: EvaluateResponse[];
  avg_score: number;
}
export interface EvaluateQueryRequest {
  query: string;
  knowledge_base_id?: string;
}
export interface EvaluateQueryResponse {
  answer: string;
  references: any[];
}

// ===== 任务 =====
export interface Task {
  id: string;
  task_type: string;
  status: string;
  progress: number;
  result?: any;
  error?: string;
  knowledge_base_id?: string;
  created_at: string;
  updated_at: string;
  task_name?: string;
  progress_message?: string;
  duration_ms?: number;
  started_at?: string;
  completed_at?: string;
  retry_count?: number;
  error_message?: string;
}

export interface SliceTask {
  id: string;
  name: string;
  data_source_name?: string;
  description?: string;
  knowledge_base_id?: string;
  status?: string;
  total_chunks?: number;
  created_at?: string;
  updated_at?: string;
}

export interface DeleteFileOptions {
  knowledgeBaseId?: string;
  originalName?: string;
}

// ===== 模型 =====
export interface LLMModel {
  id: string;
  name: string;
  model_type: string;
  provider: string;
  api_key?: string;
  base_url?: string;
  is_active: boolean;
  is_default: boolean;
}

// ===== FAQ =====
export interface FAQ {
  id: string;
  knowledge_base_id?: string;
  question: string;
  answer: string;
  tags?: string[];
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateFAQRequest {
  knowledge_base_id?: string;
  question: string;
  answer: string;
  tags?: string[];
}

export interface UpdateFAQRequest {
  question?: string;
  answer?: string;
  tags?: string[];
  is_active?: boolean;
}

// ===== 配置 =====
export interface ConfigGroup {
  group: string;
  configs: { key: string; value: unknown; description?: string; type?: string }[];
}

// ===== 聊天附件 =====
export interface ChatAttachment {
  id: string;
  name: string;
  type: 'image' | 'document' | 'archive' | 'other';
  size: number;
  file: File;
  preview?: string;
  uploading?: boolean;
  progress?: number;
  uploaded_file_id?: string;
}

// ===== 审计日志 =====
export interface AuditLogEntry {
  id: string;
  timestamp: string;
  user_id?: string;
  action: string;
  resource_type: string;
  resource_id?: string;
  status: 'success' | 'failure';
  details?: string;
  ip_address?: string;
  created_at: string;
}

export interface AuditFilter {
  action?: string;
  resource_type?: string;
  status?: string;
  user_id?: string;
  from?: string;
  to?: string;
  page?: number;
  page_size?: number;
}

export interface AuditSummary {
  total: number;
  by_action: Record<string, number>;
  by_status: Record<string, number>;
}

// ===== 知识库授权 =====
export interface KbGrant {
  id: string;
  knowledge_base_id: string;
  subject_type: 'user' | 'group' | 'role';
  subject_id: string;
  subject_name?: string;
  permission: 'read' | 'write' | 'admin';
  granted_by?: string;
  granted_at: string;
  expires_at?: string;
  created_at: string;
}

export interface CreateKbGrantRequest {
  subject_type: 'user' | 'group' | 'role';
  subject_id: string;
  permission: 'read' | 'write' | 'admin';
  expires_at?: string;
}
