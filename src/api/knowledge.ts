import { ref } from 'vue'
import axios, { AxiosError, AxiosInstance, AxiosResponse } from 'axios';
import { FASTKG_AUTH_HEADERS } from '@/config/constants';
import { createKgServiceCoordinator } from './kgServiceCoordinator';
import type {
  ApiResponse,
  KnowledgeBase,
  CreateKnowledgeBaseRequest,
  UpdateKnowledgeBaseRequest,
  FileInfo,
  GovernanceDoc,
  UpdateGovernanceRequest,
  ChunkInfo,
  SearchRequest,
  SearchResponse,
  KnowledgeRecallRequest,
  KnowledgeRecallResponse,
  FaqRecallRequest,
  FaqRecallResponse,
  EvaluateSampleRequest,
  EvaluateResponse,
  EvaluateBatchRequest,
  EvaluateBatchResponse,
  EvaluateQueryRequest,
  EvaluateQueryResponse,
  Task,
  KnowledgeStats,
  AlgorithmInfo,
  ChunkCompareRequest,
  ChunkCompareResponse,
  AssetElement,
  FilePreviewResponse,
  ChunkDetail,
  ChunkContext,
  LLMModel,
  FAQ,
  CreateFAQRequest,
  UpdateFAQRequest,
  PaginatedResponse,
  ConfigGroup,
} from '@/types/knowledge';

// ─── 服务就绪状态（模块级共享） ───────────────────────────
const kgCoordinator = createKgServiceCoordinator(30_000)
export const kgServiceReady = kgCoordinator.ready
export const kgServiceState = kgCoordinator.state
export const kgStarting = ref(false)
export const kgStartingPhase = ref('')
/** 最近一次知识服务启动失败原因（供页面展示，避免只显示笼统「未就绪」） */
export const kgLastStartError = ref('')
/** FastKG 正在覆盖安装时禁止拉起服务，避免再次锁住 FastKG.exe */
export const kgStartBlocked = ref(false)
export const kgPortConflicts = ref<Array<{ service: string; port: number; message: string }>>([])
export type SharedOwnership = 'self_managed' | 'shared_managed' | 'external_unmanaged' | 'none'
export const kgOwnership = ref<SharedOwnership>('none')
export const kgStarterApp = ref<string | undefined>()
export const kgLeaseCount = ref(0)
export const kgConsumers = ref<string[]>([])
export const kgLeaseRegistered = ref(false)
export const kgSharedInstance = ref(false)
export const kgPendingRestart = ref(false)

export interface KgSharedStatus {
  ownership: SharedOwnership
  starterApp?: string
  leaseCount: number
  consumers: string[]
  leaseRegistered: boolean
  sharedInstance: boolean
  pendingRestart: boolean
  state?: string
  instanceId?: string
  serviceDir?: string
}

let unlistenKgStatus: (() => void) | null = null

function applyKgSharedStatus(status?: Partial<KgSharedStatus>) {
  if (!status) return
  kgOwnership.value = status.ownership || 'none'
  kgStarterApp.value = status.starterApp
  kgLeaseCount.value = status.leaseCount ?? 0
  kgConsumers.value = status.consumers || []
  kgLeaseRegistered.value = !!status.leaseRegistered
  kgSharedInstance.value = !!status.sharedInstance
  kgPendingRestart.value = !!status.pendingRestart
}

export async function refreshKgSharedStatus(): Promise<KgSharedStatus | null> {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const status = await invoke<KgSharedStatus>('get_kg_shared_status')
    applyKgSharedStatus(status)
    return status
  } catch {
    return null
  }
}

let ensureKgSharedLeasePromise: Promise<KgSharedStatus | null> | null = null

async function doEnsureKgSharedLease(): Promise<KgSharedStatus | null> {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const status = await invoke<KgSharedStatus>('ensure_kg_shared_lease')
    applyKgSharedStatus(status)
    // 租约 registry 可能短暂非 ready / 变成 external；不要因此把已在跑的 FastKG 判成「未就绪」
    if (status.state === 'ready' || status.ownership === 'external_unmanaged') {
      kgCoordinator.markState('ready')
    } else if (status.state === 'starting') {
      // 启动中不改动现有就绪标志，避免闪断
    } else {
      await checkKgServiceHealth()
    }
    return status
  } catch {
    return null
  }
}

// 始终通过 FastClaw axum 代理，避免 KG 实际端口漂移后仍直连 3000。
async function getKgHealthUrl(): Promise<string> {
  if (import.meta.env.DEV) return '/api/v1/health'
  const port = await getHttpPort()
  return `http://127.0.0.1:${port}/api/v1/health`
}

export async function checkKgServiceRunning(): Promise<boolean> {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const status: any = await invoke('check_kg_service_running')
    return status?.running ?? false
  } catch {
    // 无 Tauri 环境（纯 Vite dev）：经 Vite 转发到 FastClaw 动态代理
    if (import.meta.env.DEV) {
      try {
        await fetch('/api/v1/health', {
          headers: FASTKG_AUTH_HEADERS,
          signal: AbortSignal.timeout(1500),
        })
        return true
      } catch { return false }
    }
    return false
  }
}

async function probeKgServiceHealth(): Promise<boolean> {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const result = await invoke<{ ready: boolean }>('check_kg_service_ready')
    return result?.ready ?? false
  } catch {
    // 纯 Vite 环境或旧后端兼容路径。
    try {
      const healthUrl = await getKgHealthUrl()
      const res = await fetch(healthUrl, {
        headers: FASTKG_AUTH_HEADERS,
        signal: AbortSignal.timeout(12_000),
      })
      return res.ok
    } catch (error) {
      console.warn('知识库服务 /health 请求异常:', error)
      return false
    }
  }
}

export function ensureKgSharedLease(): Promise<KgSharedStatus | null> {
  if (ensureKgSharedLeasePromise) return ensureKgSharedLeasePromise
  ensureKgSharedLeasePromise = doEnsureKgSharedLease().finally(() => {
    ensureKgSharedLeasePromise = null
  })
  return ensureKgSharedLeasePromise
}

export function checkKgServiceHealth(force = true): Promise<boolean> {
  return kgCoordinator.check(probeKgServiceHealth, force)
}

async function listenForKgReady() {
  if (unlistenKgStatus) unlistenKgStatus()
  try {
    const { listen } = await import('@tauri-apps/api/event')
    unlistenKgStatus = await listen<Partial<KgSharedStatus> & { phase: string; service?: string; port?: number; pid?: number; message?: string }>(
      'kg-service-status',
      (event) => {
        const { phase } = event.payload
        applyKgSharedStatus(event.payload)
        if (phase === 'starting_qdrant') kgStartingPhase.value = 'starting_qdrant'
        else if (phase === 'starting_fastkg') kgStartingPhase.value = 'starting_fastkg'
        else if (phase === 'starting_llama') kgStartingPhase.value = 'starting_llama'
        else if (phase === 'starting_full_stack') {
          kgStartingPhase.value = 'starting_full_stack'
          kgLastStartError.value = ''
        } else if (phase === 'all_ready') {
          kgStarting.value = false
          kgStartingPhase.value = ''
          kgLastStartError.value = ''
          kgCoordinator.markState('ready')
        } else if (phase === 'stopped') {
          kgStarting.value = false
          kgStartingPhase.value = ''
          kgCoordinator.markState('unavailable')
        } else if (phase === 'port_conflict') {
          kgStartingPhase.value = ''
          const conflict = {
            service: event.payload.service || '',
            port: event.payload.port || 0,
            message: event.payload.message || '',
          }
          kgPortConflicts.value.push(conflict)
          // 使用 Element Plus 弹警告
          import('element-plus').then(({ ElNotification }) => {
            ElNotification.warning({
              title: '端口冲突',
              message: conflict.message,
              duration: 6000,
            })
          }).catch(() => {
            console.warn('[KG 端口冲突]', conflict.message)
          })
        } else if (phase === 'error') {
          const msg = event.payload.message || '知识库服务启动失败'
          kgLastStartError.value = msg
          console.warn('知识服务启动失败:', msg)
          // 明确失败后仍给短暂窗口（并发自动启动可能稍后成功），避免空等满超时
          setTimeout(() => {
            if (kgStarting.value && !kgServiceReady.value) {
              kgStarting.value = false
              kgStartingPhase.value = ''
            }
          }, 12_000)
        }
      }
    )
  } catch { /* 非 Tauri 环境 */ }
}

// 启动服务（幂等：已在运行直接返回，启动中等待完成）
// 返回 Promise，在服务真正就绪（收到 all_ready 事件）时 resolve
let startKgPromise: Promise<void> | null = null

async function doStartKgService(): Promise<void> {
  if (kgStartBlocked.value) return
  kgStarting.value = true
  kgLastStartError.value = ''
  kgCoordinator.markState('starting')
  kgStartingPhase.value = 'starting_service'
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const { waitForDependencyReady } = await import('@/api/dependencyRecovery')
    await invoke('start_knowledge_service_async')
    await waitForDependencyReady('fastkg')
    kgServiceReady.value = true
    kgCoordinator.markState('ready')
    await refreshKgSharedStatus()
  } catch (error: any) {
    kgServiceReady.value = false
    kgLastStartError.value = error?.message || String(error)
    throw new Error(kgLastStartError.value)
  } finally {
    kgStarting.value = false
    kgStartingPhase.value = ''
  }
}

export function startKgService(): Promise<void> {
  if (startKgPromise) return startKgPromise
  startKgPromise = doStartKgService().finally(() => {
    startKgPromise = null
  })
  return startKgPromise
}

export interface EnsureKgReadyOptions {
  force?: boolean
  startIfNeeded?: boolean
}

/** 页面入口统一调用：成功状态 30 秒 TTL，所有并发调用复用同一个 Promise。 */
export function ensureKgReady(options: EnsureKgReadyOptions = {}): Promise<boolean> {
  const { force = false, startIfNeeded = true } = options
  return kgCoordinator.ensure(async () => {
    // 正常页面进入只做 PID/监听端口快检，真实 GET 若遇 502/503 再进入
    // 一次完整 readiness 恢复。这样 FastKG 已在运行时不会先阻塞约 4 秒。
    if (!force && await checkKgServiceRunning()) {
      kgCoordinator.markState('ready')
      void refreshKgSharedStatus()
      return true
    }
    if (force && await checkKgServiceHealth(true)) return true
    if (!startIfNeeded || kgStartBlocked.value) return false
    try {
      await startKgService()
      return kgServiceReady.value
    } catch {
      return false
    }
  }, force)
}

let focusSyncInstalled = false
function installKgFocusSync() {
  if (focusSyncInstalled || typeof window === 'undefined') return
  focusSyncInstalled = true
  window.addEventListener('focus', () => {
    void ensureKgReady({ startIfNeeded: false })
  })
}
installKgFocusSync()

/// 轮询等待 kgServiceReady 变为 true（带超时）
/// 注意：不要在 kgStarting=false 时立刻放行——后端自动启动仍可能在跑，
/// 过早结束会导致知识页闪一下「服务未就绪」再变成正常列表。
function waitForKgReady(timeoutMs: number): Promise<void> {
  return new Promise((resolve, reject) => {
    const startTime = Date.now()
    let lastHealthAt = 0
    let errorSeenAt = 0
    const check = async () => {
      if (kgServiceReady.value) {
        kgStarting.value = false
        kgStartingPhase.value = ''
        kgLastStartError.value = ''
        resolve()
        return
      }
      // 启动流程已结束且已有明确错误：再探一轮 health 后尽快结束
      if (!kgStarting.value && kgLastStartError.value) {
        try {
          await checkKgServiceHealth(true)
          if (kgServiceReady.value) {
            kgLastStartError.value = ''
            resolve()
            return
          }
        } catch { /* ignore */ }
        reject(new Error(kgLastStartError.value))
        return
      }
      if (kgLastStartError.value && errorSeenAt === 0) {
        errorSeenAt = Date.now()
      }
      const elapsed = Date.now() - startTime
      // 每隔约 2s 主动探活（覆盖漏发 all_ready / 并发启动跳过的情况）
      if (elapsed - lastHealthAt >= 2000) {
        lastHealthAt = elapsed
        try {
          await checkKgServiceHealth(true)
          if (kgServiceReady.value) {
            kgStarting.value = false
            kgStartingPhase.value = ''
            kgLastStartError.value = ''
            resolve()
            return
          }
        } catch { /* ignore */ }
      }
      // 收到 error 后最多再等 15s（覆盖并发自动启动），避免空等满 timeout
      if (errorSeenAt > 0 && Date.now() - errorSeenAt > 15_000) {
        kgStarting.value = false
        kgStartingPhase.value = ''
        reject(new Error(kgLastStartError.value || '知识库服务启动失败'))
        return
      }
      if (elapsed > timeoutMs) {
        kgStarting.value = false
        kgStartingPhase.value = ''
        reject(new Error(kgLastStartError.value || 'KG 服务启动超时'))
        return
      }
      setTimeout(() => { void check() }, 500)
    }
    void check()
  })
}

// ─── HTTP 端口动态获取 ───────────────────────────
// 后端 axum 端口可能因冲突而漂移（5179→5189），模块级缓存实际端口
let cachedHttpPort = 5179
let portFetched = false

export async function getHttpPort(): Promise<number> {
  if (portFetched) return cachedHttpPort
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const port = await invoke<number>('get_http_port')
    if (port && port > 0) {
      cachedHttpPort = port
    }
  } catch { /* 非 Tauri 环境或命令未注册，使用默认 5179 */ }
  portFetched = true
  return cachedHttpPort
}

// 创建 axios 实例
// 开发与生产均经 axum 动态代理；开发环境由 Vite 的 /api 转发到 axum。
const http: AxiosInstance = axios.create({
  baseURL: import.meta.env.DEV
    ? '/api/v1'
    : `http://127.0.0.1:${cachedHttpPort}/api/v1`,
  timeout: 60000,
  headers: {
    'Content-Type': 'application/json',
    ...FASTKG_AUTH_HEADERS,
  },
});

// 请求拦截器
http.interceptors.request.use(
  async (config) => {
    // 开发环境禁止直连 :3000（跨域 + 旧模块热更新残留）
    if (import.meta.env.DEV) {
      config.baseURL = '/api/v1'
      http.defaults.baseURL = '/api/v1'
    } else if (!portFetched) {
      const port = await getHttpPort()
      http.defaults.baseURL = `http://127.0.0.1:${port}/api/v1`
      config.baseURL = http.defaults.baseURL
    }
    // KG 服务启动中 → 等待就绪后再发送请求（最多 30s），避免请求超时
    if (kgStarting.value) {
      try {
        await waitForKgReady(30_000)
      } catch {
        // 超时后放行，让后端代理层返回具体错误
      }
    }

    // 共享状态由服务启动流程、kg-service-status 事件和后端租约心跳维护。
    // 普通业务请求不再同步刷新 registry；旧实现还会隐式触发完整
    // FastKG /health，曾让每个知识库请求额外阻塞约 4 秒。
    // 租约由后端恢复管理器维护，业务请求不触发注册副作用。

    return config;
  },
  (error) => {
    return Promise.reject(error);
  }
);

// 响应拦截器
http.interceptors.response.use(
  (response: AxiosResponse) => {
    return response;
  },
  async (error: AxiosError<ApiResponse<unknown>>) => {
    const status = error.response?.status
    const retryConfig = error.config as (typeof error.config & { _kgRecoveryRetried?: boolean }) | undefined
    const method = retryConfig?.method?.toLowerCase()
    const isHealthRequest = retryConfig?.url?.includes('/health')

    // 仅幂等 GET 在代理报告服务暂不可用时恢复一次。写请求绝不自动重放，
    // 避免创建、上传、删除等操作在服务端已成功时产生重复副作用。
    if (
      retryConfig
      && method === 'get'
      && !isHealthRequest
      && (status === 502 || status === 503)
      && !retryConfig._kgRecoveryRetried
    ) {
      retryConfig._kgRecoveryRetried = true
      const ready = await ensureKgReady({ force: true })
      if (ready) return http.request(retryConfig)
    }

    const message = error.response?.data?.message || error.message || '请求失败';
    
    console.error('API Error:', message);
    return Promise.reject(new Error(message));
  }
);

// 通用响应处理
async function handleResponse<T>(promise: Promise<AxiosResponse>): Promise<T> {
  const response = await promise;
  if (response.data.success) {
    return response.data.data as T;
  }
  throw new Error(response.data.message || '操作失败');
}

const KG_USER_ID = 'admin'

// ===== 知识库管理 API =====
export const knowledgeBaseApi = {
  // 获取知识库列表。发布版无登录，调用身份固定为管理员。
  list: (params?: { user_id?: string; status?: string }) =>
    handleResponse<KnowledgeBase[]>(http.get('/knowledge-bases', {
      params: { ...params, user_id: KG_USER_ID },
    })),

  // 获取单个知识库（私有库需带 user_id，否则 FastKG 返回 403，详情页会立刻退回列表）
  get: (id: string, _userId = KG_USER_ID) =>
    handleResponse<KnowledgeBase>(http.get(`/knowledge-bases/${id}`, {
      params: { user_id: KG_USER_ID },
    })),

  // 创建知识库
  create: (data: CreateKnowledgeBaseRequest) =>
    handleResponse<KnowledgeBase>(http.post('/knowledge-bases', data)),

  // 更新知识库
  update: (id: string, data: UpdateKnowledgeBaseRequest) =>
    handleResponse<KnowledgeBase>(http.put(`/knowledge-bases/${id}`, data)),

  // 删除知识库
  delete: (id: string) =>
    handleResponse<void>(http.delete(`/knowledge-bases/${id}`)),

  // 刷新知识库统计
  refreshStats: (id: string) =>
    handleResponse<KnowledgeBase>(http.post(`/knowledge-bases/${id}/refresh-stats`)),
};

// ===== 文件管理 API =====
export const fileApi = {
  // 获取文件列表（v1.1 §4.2: 支持文件名 / 扩展名 / 上传人 / 时间范围搜索）
  // 私有库必须带 user_id，否则 FastKG 返回 403
  list: (params?: {
    status?: string;
    category?: string;
    knowledge_base_id?: string;
    user_id?: string;
    /** v1.1 §4.2: 文件名模糊匹配 LIKE %?% */
    filename?: string;
    /** v1.1 §4.2: 文件扩展名精确匹配（小写，如 'pdf'） */
    file_ext?: string;
    /** v1.1 §4.2: 上传人精确匹配 */
    uploader?: string;
    /** v1.1 §4.2: 上传时间范围 'YYYY-MM-DD' 或 'YYYY-MM-DD HH:MM:SS' */
    uploaded_from?: string;
    uploaded_to?: string;
    page?: number;
    page_size?: number;
  }) => handleResponse<PaginatedResponse<FileInfo>>(http.get('/files', {
    params: { ...params, user_id: KG_USER_ID },
  })),

  // 获取文件详情
  get: (id: string) =>
    handleResponse<FileInfo>(http.get(`/files/${id}`)),

  // 上传文件（私有库需在 FormData 中带 user_id，否则 403「缺少调用者身份」）
  upload: (formData: FormData, onProgress?: (percent: number) => void) => {
    formData.set('user_id', KG_USER_ID);
    return handleResponse<FileInfo>(
      http.post('/files/upload', formData, {
        headers: { 'Content-Type': 'multipart/form-data' },
        onUploadProgress: (event) => {
          if (onProgress && event.total) {
            onProgress(Math.round((event.loaded * 100) / event.total));
          }
        },
      })
    );
  },

  // 删除文件（私有库需带 user_id，否则 403「缺少调用者身份」）
  delete: (id: string, _userId = KG_USER_ID) =>
    handleResponse<void>(http.delete(`/files/${id}`, {
      params: { user_id: KG_USER_ID },
    })),

  /** v1.1 §4.3: 批量删除文件 */
  batchDelete: (ids: string[], cascade = true, _userId = KG_USER_ID) =>
    handleResponse<{
      requested: number;
      succeeded: number;
      failed: Array<{ id: string; reason: string }>;
    }>(http.post('/files/batch-delete', { ids, cascade, user_id: KG_USER_ID }, {
      params: { user_id: KG_USER_ID },
    })),
};

// ===== 治理文档 API =====
export const governanceApi = {
  // 获取治理文档列表
  list: (params?: {
    status?: string;
    verified?: boolean;
    knowledge_base_id?: string;
    page?: number;
    page_size?: number;
  }) => handleResponse<PaginatedResponse<GovernanceDoc>>(http.get('/governance', { params })),

  // 获取治理文档详情
  get: (id: string) =>
    handleResponse<GovernanceDoc>(http.get(`/governance/${id}`)),

  // 更新治理文档
  update: (id: string, data: UpdateGovernanceRequest) =>
    handleResponse<GovernanceDoc>(http.put(`/governance/${id}`, data)),

  // 标记为人工验证
  verify: (id: string) =>
    handleResponse<GovernanceDoc>(http.post(`/governance/${id}/verify`)),

  // 获取 WSP 结构化结果
  getWspResult: (id: string) =>
    handleResponse<any>(http.get(`/governance/${id}/wsp-result`)),

  // 获取治理文档的素材列表
  getAssets: (id: string, params?: { element_type?: string }) =>
    handleResponse<any[]>(http.get(`/governance/${id}/assets`, { params })),

  // 更新素材（表格HTML/公式LaTeX）
  updateAsset: (assetId: string, data: {
    table_data?: string;
    content_text?: string;
    caption?: string;
    ocr_text?: string;
  }) => handleResponse<void>(http.put(`/governance/assets/${assetId}`, data)),

  // 重新向量化
  revectorize: (id: string) =>
    handleResponse<any>(http.post(`/governance/${id}/revectorize`)),
};

// ===== 知识切片 API =====
export const chunkApi = {
  // 获取切片列表
  list: (params?: {
    governance_id?: string;
    knowledge_base_id?: string;
    user_id?: string;
    status?: string;
    page?: number;
    page_size?: number;
  }) => handleResponse<PaginatedResponse<ChunkInfo>>(http.get('/knowledge/chunks', {
    params: { ...params, user_id: KG_USER_ID },
  })),

  // 更新切片内容
  update: (id: string, data: { content: string }) =>
    handleResponse<ChunkInfo>(http.put(`/knowledge/chunks/${id}`, data)),

  // 恢复切片原始内容
  restore: (id: string) =>
    handleResponse<ChunkInfo>(http.post(`/knowledge/chunks/${id}/restore`)),

  /** v1.1 §3.2: 删除单个切片
   *  - hard=false（默认）：软删除（status='deleted'）
   *  - hard=true：物理删除
   *  级联清理向量库 / Tantivy / QA 对
   */
  delete: (id: string, hard = false) =>
    handleResponse<void>(http.delete(`/knowledge/chunks/${id}`, { params: { hard } })),

  /** v1.1 §3.2: 批量删除切片 */
  batchDelete: (ids: string[], mode: 'soft' | 'hard' = 'soft') =>
    handleResponse<{ requested: number; deleted: number; mode: string }>(
      http.post('/knowledge/chunks/batch-delete', { ids, mode })
    ),

  /** v1.1 §3.2: 清理无意义切片
   *  - dry_run=true（默认）：仅返回命中样例，不实际删除
   *  - dry_run=false：执行删除
   */
  cleanupMeaningless: (params: {
    knowledge_base_id: string;
    min_len?: number;
    dry_run?: boolean;
    scan_limit?: number;
    mode?: 'soft' | 'hard';
  }) =>
    handleResponse<{
      kb_id: string;
      scanned: number;
      matched: number;
      deleted: number;
      dry_run: boolean;
      samples: Array<{ id: string; preview: string }>;
    }>(http.post('/knowledge/chunks/cleanup-meaningless', params)),

  // 获取切片详情
  getDetail: (chunkId: string) =>
    handleResponse<ChunkDetail>(http.get(`/chunks/${chunkId}`)),

  // 获取切片上下文
  getContext: (chunkId: string) =>
    handleResponse<ChunkContext>(http.get(`/chunks/${chunkId}/context`)),

  // 重建知识库
  rebuild: (data: { knowledge_base_id?: string; force?: boolean }) =>
    handleResponse<void>(http.post('/knowledge/rebuild', data)),

  // 获取统计信息
  getStats: () =>
    handleResponse<KnowledgeStats>(http.get('/knowledge/stats')),
};

// ===== 切片工具 API =====
export const chunkToolApi = {
  // 获取可用算法列表
  listAlgorithms: () =>
    handleResponse<AlgorithmInfo[]>(http.get('/chunk/algorithms')),

  // 切片对比 - JSON
  compare: (data: ChunkCompareRequest) =>
    handleResponse<ChunkCompareResponse>(http.post('/chunk/compare', data)),

  // 切片对比 - 文件上传
  compareFile: (formData: FormData) =>
    handleResponse<ChunkCompareResponse>(
      http.post('/chunk/compare-file', formData, {
        headers: { 'Content-Type': 'multipart/form-data' },
      })
    ),

  /** 分块预览（选择算法弹窗） */
  preview: (data: { algorithm: string; content: string; common?: Record<string, unknown> }) =>
    handleResponse<{
      total_chunks?: number;
      avg_quality?: number;
      total_chars?: number;
      sample_chunks?: Array<{ content: string }>;
    }>(http.post('/chunking/preview', data)),

  /** 文件切片：先解析再分块（分块查看「重新分块」） */
  chunkFile: (data: {
    file_id: string;
    algorithm: string;
    common?: Record<string, unknown>;
    params?: Record<string, unknown>;
  }) =>
    handleResponse<{
      full_text: string;
      chunk_result: { chunks: any[] };
    }>(http.post('/chunking/chunk-file', data)),
};

/** 文件 Markdown 预览 */
export const filePreviewApi = {
  getPreview: (fileId: string) =>
    handleResponse<FilePreviewResponse>(http.get(`/files/${fileId}/preview`)),
};

/** 切片关联资产 */
export const chunkAssetApi = {
  listByChunk: (chunkId: string) =>
    handleResponse<AssetElement[]>(http.get(`/chunks/${chunkId}/assets`)),
};

/** KB ↔ 图谱融合（实体标签，可选） */
export const kbGraphApi = {
  chunkEntities: (chunkId: string) =>
    handleResponse<any>(http.get(`/chunks/${chunkId}/entities`)),
  nodeChunks: (nodeId: string) =>
    handleResponse<any>(http.get(`/nodes/${nodeId}/chunks`)),
};

// ===== 检索 API =====
export const searchApi = {
  // 混合搜索
  hybrid: (data: SearchRequest) =>
    handleResponse<SearchResponse>(http.post('/search', data)),

  // 向量搜索
  vector: (data: SearchRequest) =>
    handleResponse<SearchResponse>(http.post('/search/vector', data)),

  // 关键词搜索
  keyword: (data: SearchRequest) =>
    handleResponse<SearchResponse>(http.post('/search/keyword', data)),
};

// ===== 召回测试 API =====
export const recallApi = {
  // 知识库召回测试
  knowledge: (data: KnowledgeRecallRequest) =>
    handleResponse<KnowledgeRecallResponse>(http.post('/retrieval/knowledge', data)),

  // FAQ召回测试
  faq: (data: FaqRecallRequest) =>
    handleResponse<FaqRecallResponse>(http.post('/retrieval/faq', data)),

  // 切片预览
  chunkPreview: (data: {
    knowledge_base_id?: string;
    file_id?: string;
    keyword?: string;
    page?: number;
    page_size?: number;
  }) => handleResponse<PaginatedResponse<unknown>>(http.post('/retrieval/chunk/preview', data)),
};

// ===== 评估 API =====
export const evaluateApi = {
  // 单样本评估
  sample: (data: EvaluateSampleRequest) =>
    handleResponse<EvaluateResponse>(http.post('/evaluate/sample', data)),

  // 批量评估
  batch: (data: EvaluateBatchRequest) =>
    handleResponse<EvaluateBatchResponse>(http.post('/evaluate/batch', data)),

  // 在线评估
  query: (data: EvaluateQueryRequest) =>
    handleResponse<EvaluateQueryResponse>(http.post('/evaluate/query', data)),
};

// ===== 任务管理 API =====
export const taskApi = {
  // 获取任务列表
  list: (params?: {
    status?: string;
    task_type?: string;
    knowledge_base_id?: string;
    page?: number;
    page_size?: number;
  }) => handleResponse<PaginatedResponse<Task>>(http.get('/tasks', { params })),

  // 获取任务详情
  get: (id: string) =>
    handleResponse<Task>(http.get(`/tasks/${id}`)),

  // 重试任务
  retry: (id: string) =>
    handleResponse<Task>(http.post(`/tasks/${id}/retry`)),

  /** 取消任务（v1.1: 先通知 TaskExecutor 再更新 DB） */
  cancel: (id: string) =>
    handleResponse<void>(http.post(`/tasks/${id}/cancel`)),

  /** v1.1 §2.2: 暂停任务 */
  pause: (id: string) =>
    handleResponse<void>(http.post(`/tasks/${id}/pause`)),

  /** v1.1 §2.2: 恢复任务 */
  resume: (id: string) =>
    handleResponse<void>(http.post(`/tasks/${id}/resume`)),

  /** v1.1 §2.2: 强制终止 */
  terminate: (id: string) =>
    handleResponse<void>(http.post(`/tasks/${id}/terminate`)),

  /** v1.1 §2.2: 修改优先级 */
  reprioritize: (id: string, priority: number) =>
    handleResponse<void>(http.post(`/tasks/${id}/priority`, { priority })),

  /** v1.1 §2.2: 批量操作 */
  batchAction: (ids: string[], action: 'cancel' | 'retry' | 'pause' | 'resume') =>
    handleResponse<{ succeeded: number; failed: number }>(
      http.post('/tasks/batch', { ids, action })
    ),
};

// ===== 配置管理 API =====
export const configApi = {
  // 获取所有配置
  list: () =>
    handleResponse<any>(http.get('/config')).then((data) => {
      return Object.entries(data).map(([group, groupData]: [string, any]) => ({
        group,
        configs: Object.entries(groupData.default || {}).map(([key, value]) => ({
          key,
          value,
          description: '',
          type: typeof value
        }))
      })) as ConfigGroup[];
    }),

  // 获取配置分组列表
  listGroups: () =>
    handleResponse<string[]>(http.get('/config/groups')),

  // 获取指定分组配置
  getGroup: (group: string) =>
    handleResponse<ConfigGroup>(http.get(`/config/${group}`)),

  // 更新配置
  update: (group: string, key: string, value: unknown) => {
    // Axios treats strings as raw body, but backend expects JSON-encoded value (e.g. "string")
    // 对于简单类型，直接包装为 { "schema": value } 格式发送
    const data = { schema: value };
    return handleResponse<void>(http.put(`/config/${group}/${key}`, data));
  },

  // 批量更新配置
  batchUpdate: (updates: { group: string; key: string; value: unknown }[]) =>
    handleResponse<void>(http.put('/config/batch', { updates })),

  // 重置配置
  reset: (group: string, key: string) =>
    handleResponse<void>(http.post(`/config/${group}/${key}/reset`)),
};

// ===== 索引管理 API =====
export const indexApi = {
  // 列出索引文档
  listDocuments: (params?: {
    page?: number;
    page_size?: number;
    query?: string;
  }) => handleResponse<PaginatedResponse<unknown>>(http.get('/index/documents', { params })),
};

// ===== 大模型管理 API =====
export const llmApi = {
  // 获取模型列表
  list: () =>
    handleResponse<LLMModel[]>(http.get('/llm/models')),

  // 创建模型
  create: (data: Partial<LLMModel>) =>
    handleResponse<LLMModel>(http.post('/llm/models', data)),

  // 更新模型
  update: (id: string, data: Partial<LLMModel>) =>
    handleResponse<LLMModel>(http.put(`/llm/models/${id}`, data)),

  // 删除模型
  delete: (id: string) =>
    handleResponse<void>(http.delete(`/llm/models/${id}`)),

  // 测试模型连接
  test: (id: string) =>
    handleResponse<{ success: boolean; message: string }>(http.post(`/llm/models/${id}/test`)),

  // 设置默认模型
  setDefault: (id: string) =>
    handleResponse<void>(http.put(`/llm/models/${id}/default`)),
};

// ===== FAQ 管理 API =====
export const faqApi = {
  // 获取FAQ列表
  list: (params?: {
    knowledge_base_id?: string;
    keyword?: string;
    page?: number;
    page_size?: number;
  }) => handleResponse<PaginatedResponse<FAQ>>(http.get('/faq', { params })),

  // 获取FAQ详情
  get: (id: string) =>
    handleResponse<FAQ>(http.get(`/faq/${id}`)),

  // 创建FAQ
  create: (data: CreateFAQRequest) =>
    handleResponse<FAQ>(http.post('/faq', data)),

  // 更新FAQ
  update: (id: string, data: UpdateFAQRequest) =>
    handleResponse<FAQ>(http.put(`/faq/${id}`, data)),

  // 删除FAQ
  delete: (id: string) =>
    handleResponse<void>(http.delete(`/faq/${id}`)),

  // 批量导入
  import: (formData: FormData) =>
    handleResponse<{ imported: number; failed: number }>(
      http.post('/faq/import', formData, {
        headers: { 'Content-Type': 'multipart/form-data' },
      })
    ),
};

// ===== 聊天 API =====
export const chatApi = {
  // 发送聊天消息（SSE流式）
  send: async function* (data: {
    question: string;
    model?: string;
    knowledge_base_ids?: string[];
    use_faq?: boolean;
    faq_threshold?: number;
    top_k?: number;
    rerank?: boolean;
  }): AsyncGenerator<{
    type: 'references' | 'delta' | 'done' | 'error';
    content?: string;
    references?: unknown[];
    error?: string;
  }> {
    const response = await fetch('http://127.0.0.1:5179/api/v1/chat/chat', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...FASTKG_AUTH_HEADERS },
      body: JSON.stringify(data),
    });

    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }

    const reader = response.body?.getReader();
    if (!reader) throw new Error('No reader');

    const decoder = new TextDecoder();
    let buffer = '';

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split('\n');
      buffer = lines.pop() || '';

      for (const line of lines) {
        if (line.startsWith('data: ')) {
          const jsonStr = line.slice(6).trim();
          if (!jsonStr) continue;

          try {
            const data = JSON.parse(jsonStr);
            yield data;
          } catch {
            console.warn('Failed to parse SSE data:', jsonStr);
          }
        }
      }
    }
  },
};

// ===== QA对管理 API =====
export const qaPairApi = {
  // 获取切片的QA对列表
  listByChunk: (chunkId: string) =>
    handleResponse<any[]>(http.get(`/chunks/${chunkId}/qa-pairs`)),

  // 为切片添加QA对
  create: (chunkId: string, data: { question: string; answer: string }) =>
    handleResponse<any>(http.post(`/chunks/${chunkId}/qa-pairs`, data)),

  // 删除QA对
  delete: (id: string) =>
    handleResponse<void>(http.delete(`/qa-pairs/${id}`)),

  // 更新QA对
  update: (id: string, data: any) =>
    handleResponse<any>(http.put(`/qa-pairs/${id}`, data)),
};

// 导出 axios 实例和通用处理函数供其他模块使用
export { http, handleResponse };

// 导出 EdgeQuake API
export { edgequakeApi } from './edgequake';

// ============================================================
// v1.1 §7: 审计日志 API
// ============================================================
import type {
  AuditLogEntry,
  AuditFilter,
  AuditSummary,
  KbGrant,
  CreateKbGrantRequest,
} from '@/types/knowledge';

export const auditApi = {
  /** 查询审计日志列表（支持多条件过滤） */
  list: (params?: AuditFilter) =>
    handleResponse<PaginatedResponse<AuditLogEntry>>(http.get('/audit/logs', { params })),

  /** 获取审计日志详情 */
  get: (id: string) =>
    handleResponse<AuditLogEntry>(http.get(`/audit/logs/${id}`)),

  /** 24h 内按 action / status 聚合统计 */
  summary: () =>
    handleResponse<AuditSummary>(http.get('/audit/summary')),
};

// ============================================================
// v1.1 §1.1: 知识库授权 API
// ============================================================

export const kbGrantApi = {
  /** 列出某 kb 的全部授权记录 */
  list: (kbId: string) =>
    handleResponse<KbGrant[]>(http.get(`/kb/${kbId}/grants`)),

  /** 新增一条授权 */
  grant: (kbId: string, data: CreateKbGrantRequest) =>
    handleResponse<KbGrant>(http.post(`/kb/${kbId}/grants`, data)),

  /** 撤销一条授权 */
  revoke: (kbId: string, grantId: string) =>
    handleResponse<void>(http.delete(`/kb/${kbId}/grants/${grantId}`)),
};
