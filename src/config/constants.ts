// FastClaw · 前端默认值唯一定义文件
// 与 src-tauri/src/core/constants.rs 保持同步。
//
//    这是前端所有出厂默认值的唯一来源。其他文件（appConfig、index.js 等）
//    均从此文件导入，不得在其他文件中重复定义相同的默认值。
//
// 运行时实际值来源：config.json（通过 appConfigStore 读取）。
// 此文件中的值仅在 config.json 缺失/加载失败时作为回退使用。

// ─── HTTP 服务器默认配置 ────────────────────────────────────

export const HTTP_SERVER_DEFAULTS = {
  host: '127.0.0.1',
  port_start: 5179,
  port_end: 5189,
} as const

// ─── 本地服务 URL 默认值 ─────────────────────────────────────

export const SERVICE_URL_DEFAULTS: Record<string, string> = {
  fastplan: 'http://127.0.0.1:23180',
  fastkg: 'http://127.0.0.1:23140',
  shimmy: 'http://127.0.0.1:11435',
  pinchtab: 'http://127.0.0.1:9867',
  workflow: 'http://127.0.0.1:804',
  qdrant: 'http://127.0.0.1:23060',
  llama_server: 'http://127.0.0.1:23100',
  asr: '',
}

/** 本地服务中文名称映射（仅在设置页中显示的服务需列出） */
export const SERVICE_LABELS: Record<string, string> = {
  // fastplan: '任务规划服务',
  // fastkg: '知识库服务',
  shimmy: 'WSP模型服务',
  pinchtab: 'Pinchtab 服务',
  // workflow: 'Agent 服务',
  // qdrant: '向量库接口',
  // llama_server: 'Llama 嵌入服务',
}

// ─── 远程服务器 URL 默认值 ───────────────────────────────────

export const REMOTE_SERVER_DEFAULTS = {
  /** KG 服务下载地址。公开版不内置地址，由使用方自行配置。 */
  download_api_base_url: '',
  /** 可选的私有下载镜像；公开版默认留空 */
  download_api_base_url_private: '',
  /** 模型同步接口地址。公开版不内置地址。 */
  model_sync_url: '',
  /** FastCI 服务。公开版不内置地址。 */
  fastci_base_url: '',
  /** 模型下载接口。公开版不内置地址。 */
  fastci_model_download_url: '',
  /** 是否启用独立模型数据 */
  independent_model_data: false,
  /** 独立模型数据接口地址 */
  independent_model_api_url: '',
} as const

export const KNOWLEDGE_SERVICE_DEFAULTS = {
  vector_backend: 'qdrant',
  vector_url: '',
  vector_api_key_env: 'FASTCLAW_VECTOR_API_KEY',
  embedding_url: '',
  embedding_model: 'bge-m3',
  embedding_api_key_env: 'FASTCLAW_EMBEDDING_API_KEY',
  reranker_url: '',
  reranker_model: 'bge-reranker-v2-m3',
  reranker_api_key_env: 'FASTCLAW_RERANKER_API_KEY',
} as const

/** 远程服务器配置中文名称映射 */
export const REMOTE_SERVER_LABELS: Record<string, string> = {
  download_api_base_url: '知识库下载地址(外网)',
  download_api_base_url_private: '知识库下载地址(内网)',
  model_sync_url: '模型同步接口地址',
  fastci_base_url: 'FastCI地址',
  fastci_model_download_url: '模型下载接口地址',
  independent_model_data: '启用独立模型数据',
  independent_model_api_url: '独立模型数据接口地址',
}

/** 所有远程服务器配置 key 列表 */
export const REMOTE_SERVER_KEYS = Object.keys(REMOTE_SERVER_LABELS)

// ─── 向后兼容的独立导出 ──────────────────────────────────────
// 旧代码可能直接 import { DOWNLOAD_API_BASE_URL } from '@/config/constants'

export const DOWNLOAD_API_BASE_URL = REMOTE_SERVER_DEFAULTS.download_api_base_url
export const DOWNLOAD_API_BASE_URL_PRIVATE = REMOTE_SERVER_DEFAULTS.download_api_base_url_private
export const MODEL_SYNC_URL = REMOTE_SERVER_DEFAULTS.model_sync_url
export const FASTCI_BASE_URL = REMOTE_SERVER_DEFAULTS.fastci_base_url
export const FASTCI_MODEL_DOWNLOAD_URL = REMOTE_SERVER_DEFAULTS.fastci_model_download_url

// ─── 系统品牌默认值 ──────────────────────────────────────────

export const BRANDING_DEFAULTS = {
  /** 公司名称（关于页、版权信息共用） */
  company_name: 'FastClaw',
  /** 自定义系统名称（空 = 使用默认 FastClaw） */
  app_name: '',
  /** 自定义系统图标（空 = 使用打包内置 logo） */
  app_logo: '',
  /** 自定义对话模型图标（空 = 使用默认图标） */
  chat_model_icon: '',
} as const

// ─── 标签页缓存 ───────────────────────────────────────────────
// Tab 数量不设硬上限：用户可继续打开，且不自动关闭已打开的页。
// KeepAlive 不设 max，避免 LRU 静默卸载执行中会话导致半恢复/断连。
// 对话页重型 DOM 另由 CHAT_VIEW_HOT_CACHE_SIZE 控制（见 App.vue）。
/** Soft warn only：超过该数量仍可继续打开，仅提示建议关闭闲置页。 */
export const TAB_SOFT_WARN_MAX = 20
/**
 * Heavy ChatView DOM cache. Three avoids the common A → B → C → A thrash
 * measured with a two-entry cache, while the remaining open tabs keep only
 * their Pinia/runtime state and can be recreated on demand.
 */
export const CHAT_VIEW_HOT_CACHE_SIZE = 3

// ─── 网络连通性检测 ──────────────────────────────────────────

export const CONNECTIVITY_CHECK_URL = 'https://www.baidu.com/favicon.ico'
export const CONNECTIVITY_CHECK_URL_FALLBACK = ''

// ─── WebView2 检测默认值 ────────────────────────────────────

export const WEBVIEW2_DEFAULTS = {
  min_version: '148.0.0.0',
  online_installer_url: 'https://go.microsoft.com/fwlink/p/?LinkId=2124703',
  /** 可选的离线安装地址；公开版默认留空 */
  offline_installer_url: '',
  network_timeout_secs: 2,
  internet_probes: [
    { type: 'tcp' as const, target: 'go.microsoft.com:443' },
  ],
} as const

// ─── 搜索 API ────────────────────────────────────────────────

export const TAVILY_API_URL = 'https://api.tavily.com/search'
export const EXA_API_URL = 'https://api.exa.ai/search'
export const BOCHA_API_URL = 'https://api.bocha.cn/v1/web-search'
export const GOOGLE_SEARCH_URL = 'https://www.google.com/search'
export const BING_SEARCH_URL = 'https://www.bing.com/search'
export const BAIDU_SEARCH_URL = 'https://www.baidu.com/s'

/** FastKG 服务间鉴权 */
export const FASTKG_SERVICE_KEY_HEADER = 'X-FastKG-Service-Key'
export const FASTKG_SERVICE_KEY = 'fastkg-service-key'
export const FASTKG_AUTH_HEADERS = {
  [FASTKG_SERVICE_KEY_HEADER]: FASTKG_SERVICE_KEY,
} as const
