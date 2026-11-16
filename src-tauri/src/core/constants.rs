// FastClaw · 后端编译期常量
// 与 src/config/constants.ts 保持同步。
//
// ⚠️ 此处仅保留不可通过 config.json 配置的编译期常量。
//    可配置的远程服务器 URL 默认值已移至 core/config.rs，
//    请从 crate::core::config 导入 DEFAULT_* 常量。

// ─── 网络连通性检测 ──────────────────────────────────────────

/// 网络连通性检测地址
pub const CONNECTIVITY_CHECK_URL: &str = "https://www.baidu.com/favicon.ico";

/// 备用连通性检测地址
pub const CONNECTIVITY_CHECK_URL_FALLBACK: &str = "";

// ─── 搜索 API ────────────────────────────────────────────────

pub const TAVILY_API_URL: &str = "https://api.tavily.com/search";
pub const EXA_API_URL: &str = "https://api.exa.ai/search";
pub const BOCHA_API_URL: &str = "https://api.bocha.cn/v1/web-search";
pub const GOOGLE_SEARCH_URL: &str = "https://www.google.com/search";
pub const BING_SEARCH_URL: &str = "https://www.bing.com/search";
pub const BAIDU_SEARCH_URL: &str = "https://www.baidu.com/s";

/// FastKG 服务间鉴权请求头
pub const FASTKG_SERVICE_KEY_HEADER: &str = "X-FastKG-Service-Key";
pub const FASTKG_SERVICE_KEY: &str = "fastkg-service-key";

// ─── 内置工具名称常量 ────────────────────────

pub const TOOL_NAME_FILE_READ: &str = "Read";
pub const TOOL_NAME_FILE_WRITE: &str = "Write";
pub const TOOL_NAME_FILE_EDIT: &str = "Edit";
pub const TOOL_NAME_BATCH_EDIT: &str = "BatchEdit";
pub const TOOL_NAME_APPLY_PATCH: &str = "ApplyPatch";
pub const TOOL_NAME_GLOB: &str = "Glob";
pub const TOOL_NAME_GREP: &str = "Grep";
pub const TOOL_NAME_WEB_FETCH: &str = "WebFetch";
pub const TOOL_NAME_WEB_SEARCH: &str = "WebSearch";
pub const TOOL_NAME_BASH: &str = "Bash";
pub const TOOL_NAME_POWERSHELL: &str = "PowerShell";
pub const TOOL_NAME_TODO_WRITE: &str = "TodoWrite";
pub const TOOL_NAME_TASK_CREATE: &str = "TaskCreate";
pub const TOOL_NAME_SLEEP: &str = "Sleep";
pub const TOOL_NAME_SKILL: &str = "Skill";
pub const TOOL_NAME_ASK_USER: &str = "AskUserQuestion";
