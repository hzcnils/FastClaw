// FastClaw · 应用配置管理器
// 管理 config.json（exe 同目录）：本地服务 URL + HTTP 服务器端口
// 通过 Arc<RwLock<AppConfig>> 实现运行时即时生效（端口除外，需重启）

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

const CURRENT_CONFIG_VERSION: u32 = 6;

// ─── 远程服务器 URL 默认值（可配置参数的唯一定义源）─────────
// 这些值可通过 config.json → remote_servers 在运行时覆盖。
// 其他模块如需引用默认值，应从此处导入，而非从 constants.rs。

/// KG 服务 / 更新包下载地址（外网 / 汇智网）
pub const DEFAULT_DOWNLOAD_API_BASE_URL: &str = "";
/// 可选的私有下载镜像；公开版默认留空
pub const DEFAULT_DOWNLOAD_API_BASE_URL_PRIVATE: &str = "";
/// 模型同步接口地址
pub const DEFAULT_MODEL_SYNC_URL: &str = "";
/// 模型市场 / FastCI 服务
pub const DEFAULT_FASTCI_BASE_URL: &str = "";
/// 模型下载接口
pub const DEFAULT_FASTCI_MODEL_DOWNLOAD_URL: &str = "";

// ─── WebView2 检测默认值 ────────────────────────────────────

pub const DEFAULT_WEBVIEW2_MIN_VERSION: &str = "148.0.0.0";
pub const DEFAULT_WEBVIEW2_ONLINE_INSTALLER_URL: &str =
    "https://go.microsoft.com/fwlink/p/?LinkId=2124703";
pub const DEFAULT_WEBVIEW2_OFFLINE_INSTALLER_URL: &str = "";

// ─── 默认值函数（供 serde(default) 使用）─────────────────────

fn default_version() -> u32 {
    1
}
fn default_host() -> String {
    "127.0.0.1".into()
}
fn default_port_start() -> u16 {
    5179
}
fn default_port_end() -> u16 {
    5189
}
fn default_fastplan_url() -> String {
    format!(
        "http://127.0.0.1:{}",
        crate::utils::paths::FASTPLAN_DEFAULT_PORT
    )
}
fn default_fastkg_url() -> String {
    format!(
        "http://127.0.0.1:{}",
        crate::utils::paths::FASTKG_DEFAULT_PORT
    )
}
fn default_shimmy_url() -> String {
    "http://127.0.0.1:11435".into()
}
fn default_pinchtab_url() -> String {
    "http://127.0.0.1:9867".into()
}
fn default_workflow_url() -> String {
    "http://127.0.0.1:804".into()
}
fn default_qdrant_url() -> String {
    format!(
        "http://127.0.0.1:{}",
        crate::utils::paths::QDRANT_DEFAULT_PORT
    )
}
fn default_llama_server_url() -> String {
    format!(
        "http://127.0.0.1:{}",
        crate::utils::paths::LLAMA_SERVER_DEFAULT_PORT
    )
}
fn default_asr_url() -> String {
    String::new()
}

fn default_download_api_base_url() -> String {
    DEFAULT_DOWNLOAD_API_BASE_URL.to_string()
}
fn default_download_api_base_url_private() -> String {
    DEFAULT_DOWNLOAD_API_BASE_URL_PRIVATE.to_string()
}
fn default_model_sync_url() -> String {
    DEFAULT_MODEL_SYNC_URL.to_string()
}
fn default_fastci_base_url() -> String {
    DEFAULT_FASTCI_BASE_URL.to_string()
}
fn default_fastci_model_download_url() -> String {
    DEFAULT_FASTCI_MODEL_DOWNLOAD_URL.to_string()
}
fn default_independent_model_data() -> bool {
    false
}
fn default_independent_model_api_url() -> String {
    String::new()
}
fn default_vector_backend() -> String {
    "qdrant".into()
}
fn default_embedding_model() -> String {
    "bge-m3".into()
}
fn default_reranker_model() -> String {
    "bge-reranker-v2-m3".into()
}
fn default_vector_api_key_env() -> String {
    "FASTCLAW_VECTOR_API_KEY".into()
}
fn default_embedding_api_key_env() -> String {
    "FASTCLAW_EMBEDDING_API_KEY".into()
}
fn default_reranker_api_key_env() -> String {
    "FASTCLAW_RERANKER_API_KEY".into()
}
fn default_company_name() -> String {
    "FastClaw".into()
}
fn default_webview2_min_version() -> String {
    DEFAULT_WEBVIEW2_MIN_VERSION.into()
}
fn default_webview2_online_installer_url() -> String {
    DEFAULT_WEBVIEW2_ONLINE_INSTALLER_URL.into()
}
fn default_webview2_offline_installer_url() -> String {
    DEFAULT_WEBVIEW2_OFFLINE_INSTALLER_URL.into()
}
fn default_webview2_network_timeout_secs() -> u64 {
    2
}
fn default_webview2_internet_probes() -> Vec<WebView2NetworkProbeConfig> {
    vec![WebView2NetworkProbeConfig::Tcp {
        target: "go.microsoft.com:443".into(),
    }]
}

// ─── 结构体定义 ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_version")]
    pub version: u32,

    #[serde(default)]
    pub http_server: HttpServerConfig,

    #[serde(default)]
    pub services: ServicesConfig,

    /// 菜单可见性配置（空 = 全部可见）
    #[serde(default)]
    pub menus: HashMap<String, bool>,

    /// 设置页可见性配置（空 = 全部可见）
    #[serde(default)]
    pub settings: HashMap<String, bool>,

    /// 远程服务器地址配置（模型同步、更新下载、FastCI 等）
    #[serde(default)]
    pub remote_servers: RemoteServersConfig,

    /// 知识库的可替换外部依赖；URL 为空时使用随 FastKG 下载的本地服务。
    #[serde(default)]
    pub knowledge: KnowledgeServicesConfig,

    /// 系统品牌配置（名称 + 图标，可随 config.json 分发）
    #[serde(default)]
    pub branding: BrandingConfig,

    /// WebView2 版本检测和内外网下载引导配置（修改后需重启生效）
    #[serde(default)]
    pub webview2: WebView2Config,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpServerConfig {
    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_port_start")]
    pub port_start: u16,

    #[serde(default = "default_port_end")]
    pub port_end: u16,
}

impl Default for HttpServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port_start: default_port_start(),
            port_end: default_port_end(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicesConfig {
    #[serde(default = "default_fastplan_url")]
    pub fastplan: String,

    #[serde(default = "default_fastkg_url")]
    pub fastkg: String,

    #[serde(default = "default_shimmy_url")]
    pub shimmy: String,

    #[serde(default = "default_pinchtab_url")]
    pub pinchtab: String,

    #[serde(default = "default_workflow_url")]
    pub workflow: String,

    #[serde(default = "default_qdrant_url")]
    pub qdrant: String,

    #[serde(default = "default_llama_server_url")]
    pub llama_server: String,

    /// ASR 语音识别 WebSocket 地址
    #[serde(default = "default_asr_url")]
    pub asr: String,
}

impl Default for ServicesConfig {
    fn default() -> Self {
        Self {
            fastplan: default_fastplan_url(),
            fastkg: default_fastkg_url(),
            shimmy: default_shimmy_url(),
            pinchtab: default_pinchtab_url(),
            workflow: default_workflow_url(),
            qdrant: default_qdrant_url(),
            llama_server: default_llama_server_url(),
            asr: default_asr_url(),
        }
    }
}

// ─── 远程服务器地址配置 ──────────────────────────────────

/// 远程服务器/云端服务地址配置
/// 对应前端 constants.ts 中原本硬编码的远程 URL，现可运行时修改。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteServersConfig {
    /// KG 服务 / 更新包下载地址（外网 / 汇智网）
    #[serde(default = "default_download_api_base_url")]
    pub download_api_base_url: String,

    /// 下载/更新服务器地址（内网 / 私域网）
    #[serde(default = "default_download_api_base_url_private")]
    pub download_api_base_url_private: String,

    /// 模型同步接口地址
    #[serde(default = "default_model_sync_url")]
    pub model_sync_url: String,

    /// 模型市场 / FastCI 服务
    #[serde(default = "default_fastci_base_url")]
    pub fastci_base_url: String,

    /// 模型下载接口
    #[serde(default = "default_fastci_model_download_url")]
    pub fastci_model_download_url: String,

    /// 是否启用独立模型数据（从独立 API 拉取模型作为 ChatView 模型来源）
    #[serde(default = "default_independent_model_data")]
    pub independent_model_data: bool,

    /// 独立模型数据 API 地址（仅在 independent_model_data=true 时使用）
    #[serde(default = "default_independent_model_api_url")]
    pub independent_model_api_url: String,
}

impl Default for RemoteServersConfig {
    fn default() -> Self {
        Self {
            download_api_base_url: default_download_api_base_url(),
            download_api_base_url_private: default_download_api_base_url_private(),
            model_sync_url: default_model_sync_url(),
            fastci_base_url: default_fastci_base_url(),
            fastci_model_download_url: default_fastci_model_download_url(),
            independent_model_data: default_independent_model_data(),
            independent_model_api_url: default_independent_model_api_url(),
        }
    }
}

// ─── 知识库外部依赖配置 ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeServicesConfig {
    /// qdrant | milvus | pinecone | chroma | faiss
    #[serde(default = "default_vector_backend")]
    pub vector_backend: String,
    /// 向量库地址；为空时使用自动下载包中的本地 Qdrant。
    #[serde(default)]
    pub vector_url: String,
    /// 向量库密钥所在环境变量名，配置文件中不保存密钥本身。
    #[serde(default = "default_vector_api_key_env")]
    pub vector_api_key_env: String,
    /// Embedding OpenAI 兼容接口；为空时使用自动下载包中的本地服务。
    #[serde(default)]
    pub embedding_url: String,
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
    #[serde(default = "default_embedding_api_key_env")]
    pub embedding_api_key_env: String,
    /// 重排序接口；为空时禁用。
    #[serde(default)]
    pub reranker_url: String,
    #[serde(default = "default_reranker_model")]
    pub reranker_model: String,
    #[serde(default = "default_reranker_api_key_env")]
    pub reranker_api_key_env: String,
}

impl Default for KnowledgeServicesConfig {
    fn default() -> Self {
        Self {
            vector_backend: default_vector_backend(),
            vector_url: String::new(),
            vector_api_key_env: default_vector_api_key_env(),
            embedding_url: String::new(),
            embedding_model: default_embedding_model(),
            embedding_api_key_env: default_embedding_api_key_env(),
            reranker_url: String::new(),
            reranker_model: default_reranker_model(),
            reranker_api_key_env: default_reranker_api_key_env(),
        }
    }
}

// ─── 系统品牌配置 ────────────────────────────────────────

/// 系统名称/图标自定义配置，可在 config.json 中分发。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandingConfig {
    /// 公司名称；关于页和版权信息使用此值
    #[serde(default = "default_company_name")]
    pub company_name: String,

    /// 自定义系统名称（空 = 使用默认 "FastClaw"）
    #[serde(default)]
    pub app_name: String,

    /// 自定义系统图标 data URL（空 = 使用打包内置 logo）
    #[serde(default)]
    pub app_logo: String,

    /// 自定义对话模型图标 data URL（空 = 使用默认模型图标）
    #[serde(default)]
    pub chat_model_icon: String,
}

impl Default for BrandingConfig {
    fn default() -> Self {
        Self {
            company_name: default_company_name(),
            app_name: String::new(),
            app_logo: String::new(),
            chat_model_icon: String::new(),
        }
    }
}

// ─── WebView2 检测配置 ─────────────────────────────────────

/// 公网连通性探针。仅用于决定使用在线地址还是内网地址。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum WebView2NetworkProbeConfig {
    Tcp { target: String },
    Dns { host: String },
    Http { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebView2Config {
    #[serde(default = "default_webview2_min_version")]
    pub min_version: String,

    #[serde(default = "default_webview2_online_installer_url")]
    pub online_installer_url: String,

    /// 无法直连互联网时向用户提供的内网下载地址。
    #[serde(default = "default_webview2_offline_installer_url")]
    pub offline_installer_url: String,

    #[serde(default = "default_webview2_network_timeout_secs")]
    pub network_timeout_secs: u64,

    #[serde(default = "default_webview2_internet_probes")]
    pub internet_probes: Vec<WebView2NetworkProbeConfig>,
}

impl Default for WebView2Config {
    fn default() -> Self {
        Self {
            min_version: default_webview2_min_version(),
            online_installer_url: default_webview2_online_installer_url(),
            offline_installer_url: default_webview2_offline_installer_url(),
            network_timeout_secs: default_webview2_network_timeout_secs(),
            internet_probes: default_webview2_internet_probes(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CURRENT_CONFIG_VERSION,
            http_server: HttpServerConfig::default(),
            services: ServicesConfig::default(),
            menus: HashMap::new(),    // 空 = 全部可见
            settings: HashMap::new(), // 空 = 全部可见
            remote_servers: RemoteServersConfig::default(),
            knowledge: KnowledgeServicesConfig::default(),
            branding: BrandingConfig::default(),
            webview2: WebView2Config::default(),
        }
    }
}

// ─── ConfigManager ───────────────────────────────────────────

/// 线程安全的配置管理器
/// - 启动时从 config.json 加载，文件不存在则用默认值创建
/// - 修改后保存到文件，运行时通过 Arc<RwLock<AppConfig>> 即时生效
pub struct ConfigManager {
    config: Arc<RwLock<AppConfig>>,
    file_path: PathBuf,
}

impl ConfigManager {
    /// 创建配置管理器：加载配置文件，不存在则创建默认文件
    pub fn new(exe_dir: &Path) -> anyhow::Result<Self> {
        let file_path = exe_dir.join("config.json");

        let config = if file_path.exists() {
            let content = std::fs::read_to_string(&file_path)?;
            match serde_json::from_str::<AppConfig>(&content) {
                Ok(mut cfg) => {
                    let migrated = migrate_legacy_config(&mut cfg);
                    if migrated {
                        Self::save_to_file(&cfg, &file_path)?;
                        log::info!(
                            "[Config] 已迁移依赖服务默认端口并保存: {}",
                            file_path.display()
                        );
                    }
                    log::info!("[Config] 已加载配置文件: {}", file_path.display());
                    cfg
                }
                Err(e) => {
                    log::warn!("[Config] 配置文件解析失败 ({}), 使用默认值并覆盖", e);
                    let defaults = AppConfig::default();
                    // 备份损坏的文件
                    let backup = file_path.with_extension("json.bak");
                    let _ = std::fs::rename(&file_path, &backup);
                    Self::save_to_file(&defaults, &file_path)?;
                    defaults
                }
            }
        } else {
            let defaults = AppConfig::default();
            Self::save_to_file(&defaults, &file_path)?;
            log::info!("[Config] 已创建默认配置文件: {}", file_path.display());
            defaults
        };

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            file_path,
        })
    }

    /// 读取配置（异步读锁）
    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, AppConfig> {
        self.config.read().await
    }

    /// 获取配置的克隆副本
    pub async fn get_clone(&self) -> AppConfig {
        self.config.read().await.clone()
    }

    /// 启动异步运行时前读取初始配置。创建后尚未共享，不会发生锁竞争。
    pub fn get_initial_clone(&self) -> AppConfig {
        self.config
            .try_read()
            .expect("initial app config lock should be available")
            .clone()
    }

    /// 按 JSON 点号路径更新单个字段 + 保存到文件
    /// 例如：`update_field("services.fastplan.url", "\"http://...\"")`
    pub async fn update_field(
        &self,
        dot_path: &str,
        value: serde_json::Value,
    ) -> anyhow::Result<()> {
        let mut cfg = self.config.write().await;

        // 按路径更新
        apply_dot_path(&mut *cfg, dot_path, value)?;

        // 保存到文件
        Self::save_to_file(&cfg, &self.file_path)?;
        log::info!("[Config] 配置已更新并保存: {} ", dot_path);
        Ok(())
    }

    /// 全量更新配置
    pub async fn update_full(&self, new_config: AppConfig) -> anyhow::Result<()> {
        let mut cfg = self.config.write().await;
        *cfg = new_config;
        Self::save_to_file(&cfg, &self.file_path)?;
        log::info!("[Config] 配置已全量更新");
        Ok(())
    }

    /// 获取配置文件路径
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    /// 写入配置（获取写锁）
    pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, AppConfig> {
        self.config.write().await
    }

    /// 保存当前配置到文件
    pub fn save(&self, config: &AppConfig) -> anyhow::Result<()> {
        Self::save_to_file(config, &self.file_path)
    }

    // ─── 内部方法 ─────────────────────────────────────────

    fn save_to_file(config: &AppConfig, path: &Path) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(config)?;
        // 确保父目录存在
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // 写入临时文件后原子 rename
        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, &json)?;
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }
}

/// 迁移旧版配置并提升 schema 版本。
/// 依赖服务 URL 只匹配完整旧值，用户自定义的主机或端口不会被修改；
/// serde 默认值补齐的新配置节会在迁移保存时一并写回 config.json。
fn migrate_legacy_config(config: &mut AppConfig) -> bool {
    if config.version >= CURRENT_CONFIG_VERSION {
        return false;
    }

    let replacements = [
        (
            &mut config.services.qdrant,
            "http://127.0.0.1:6333",
            default_qdrant_url(),
        ),
        (
            &mut config.services.llama_server,
            "http://127.0.0.1:8089",
            default_llama_server_url(),
        ),
        (
            &mut config.services.fastkg,
            "http://127.0.0.1:3000",
            default_fastkg_url(),
        ),
        (
            &mut config.services.fastplan,
            "http://127.0.0.1:8080",
            default_fastplan_url(),
        ),
    ];

    for (current, legacy, replacement) in replacements {
        if (config.version < 3 || legacy == "http://127.0.0.1:8090") && current.as_str() == legacy {
            *current = replacement;
        }
    }
    config.version = CURRENT_CONFIG_VERSION;
    true
}

// ─── 按点号路径更新 serde_json::Value ────────────────────────

fn apply_dot_path<T: Serialize + serde::de::DeserializeOwned>(
    config: &mut T,
    dot_path: &str,
    value: serde_json::Value,
) -> anyhow::Result<()> {
    let mut current = serde_json::to_value(&*config)?;
    let segments: Vec<&str> = dot_path.split('.').collect();

    // 定位到目标父节点并更新
    set_json_path(&mut current, &segments, value)?;

    // 反序列化回结构体
    *config = serde_json::from_value(current)?;
    Ok(())
}

fn set_json_path(
    root: &mut serde_json::Value,
    segments: &[&str],
    value: serde_json::Value,
) -> anyhow::Result<()> {
    if segments.is_empty() {
        *root = value;
        return Ok(());
    }

    let mut current = root;
    for (i, seg) in segments.iter().enumerate() {
        if i == segments.len() - 1 {
            // 最后一个段：设置值
            if let Some(obj) = current.as_object_mut() {
                obj.insert(seg.to_string(), value.clone());
            } else {
                anyhow::bail!("路径段 '{}' 不是 JSON 对象", seg);
            }
        } else {
            // 中间段：进入子对象（必要时创建）
            if let Some(obj) = current.as_object_mut() {
                if !obj.contains_key(*seg) {
                    obj.insert(seg.to_string(), serde_json::json!({}));
                }
                // 安全：刚插入或已存在
                current = obj.get_mut(*seg).unwrap();
            } else {
                anyhow::bail!("路径段 '{}' 不是 JSON 对象", seg);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_roundtrip() {
        let cfg = AppConfig::default();
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        let parsed: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, CURRENT_CONFIG_VERSION);
        assert_eq!(parsed.http_server.port_start, 5179);
        assert_eq!(parsed.http_server.port_end, 5189);
        assert_eq!(parsed.services.fastplan, "http://127.0.0.1:23180");
        assert_eq!(parsed.services.fastkg, "http://127.0.0.1:23140");
        assert_eq!(parsed.services.qdrant, "http://127.0.0.1:23060");
        assert_eq!(parsed.services.llama_server, "http://127.0.0.1:23100");
        assert_eq!(parsed.branding.company_name, "FastClaw");
        assert_eq!(parsed.webview2.min_version, "148.0.0.0");
        assert_eq!(parsed.webview2.network_timeout_secs, 2);
        assert_eq!(
            parsed.webview2.internet_probes,
            vec![WebView2NetworkProbeConfig::Tcp {
                target: "go.microsoft.com:443".into()
            }]
        );
    }

    #[test]
    fn test_legacy_branding_config_gets_company_name_default() {
        let parsed: BrandingConfig = serde_json::from_value(serde_json::json!({
            "app_name": "Custom App",
            "app_logo": "",
            "chat_model_icon": ""
        }))
        .unwrap();
        assert_eq!(parsed.company_name, "FastClaw");
        assert_eq!(parsed.app_name, "Custom App");
    }

    #[test]
    fn test_config_manager_persists_company_name_for_v3_config() {
        let directory = tempfile::tempdir().expect("create temp config directory");
        let config_path = directory.path().join("config.json");
        let mut previous = serde_json::to_value(AppConfig::default()).unwrap();
        previous["version"] = serde_json::json!(3);
        previous["branding"]
            .as_object_mut()
            .unwrap()
            .remove("company_name");
        std::fs::write(
            &config_path,
            serde_json::to_string_pretty(&previous).unwrap(),
        )
        .expect("write v3 config");

        ConfigManager::new(directory.path()).expect("load and migrate v3 config");

        let saved: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(config_path).expect("read migrated config"),
        )
        .expect("parse migrated config");
        assert_eq!(saved["version"], CURRENT_CONFIG_VERSION);
        assert_eq!(
            saved["branding"]["company_name"],
            "FastClaw"
        );
    }

    #[test]
    fn test_config_manager_adds_webview2_section_for_v4_config() {
        let directory = tempfile::tempdir().expect("create temp config directory");
        let config_path = directory.path().join("config.json");
        let mut previous = serde_json::to_value(AppConfig::default()).unwrap();
        previous["version"] = serde_json::json!(4);
        previous.as_object_mut().unwrap().remove("webview2");
        std::fs::write(
            &config_path,
            serde_json::to_string_pretty(&previous).unwrap(),
        )
        .expect("write v4 config");

        ConfigManager::new(directory.path()).expect("load and migrate v4 config");

        let saved: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(config_path).expect("read migrated config"),
        )
        .expect("parse migrated config");
        assert_eq!(saved["version"], CURRENT_CONFIG_VERSION);
        assert_eq!(saved["webview2"]["min_version"], "148.0.0.0");
        assert_eq!(
            saved["webview2"]["internet_probes"][0]["target"],
            "go.microsoft.com:443"
        );
    }

    #[test]
    fn test_config_manager_adds_knowledge_section_for_v5_config() {
        let directory = tempfile::tempdir().expect("create temp config directory");
        let config_path = directory.path().join("config.json");
        let mut previous = serde_json::to_value(AppConfig::default()).unwrap();
        previous["version"] = serde_json::json!(5);
        previous.as_object_mut().unwrap().remove("knowledge");
        std::fs::write(
            &config_path,
            serde_json::to_string_pretty(&previous).unwrap(),
        )
        .expect("write v5 config");

        ConfigManager::new(directory.path()).expect("load and migrate v5 config");

        let saved: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(config_path).expect("read migrated config"),
        )
        .expect("parse migrated config");
        assert_eq!(saved["version"], CURRENT_CONFIG_VERSION);
        assert_eq!(saved["knowledge"]["vector_backend"], "qdrant");
        assert_eq!(saved["knowledge"]["vector_url"], "");
        assert_eq!(saved["knowledge"]["reranker_url"], "");
    }

    #[test]
    fn test_migrate_legacy_dependency_service_ports() {
        let mut cfg = AppConfig::default();
        cfg.version = 1;
        cfg.services.qdrant = "http://127.0.0.1:6333".into();
        cfg.services.llama_server = "http://127.0.0.1:8089".into();
        cfg.services.fastkg = "http://127.0.0.1:3000".into();
        cfg.services.fastplan = "http://127.0.0.1:8080".into();

        assert!(migrate_legacy_config(&mut cfg));
        assert_eq!(cfg.version, CURRENT_CONFIG_VERSION);
        assert_eq!(cfg.services.qdrant, "http://127.0.0.1:23060");
        assert_eq!(cfg.services.llama_server, "http://127.0.0.1:23100");
        assert_eq!(cfg.services.fastkg, "http://127.0.0.1:23140");
        assert_eq!(cfg.services.fastplan, "http://127.0.0.1:23180");
    }

    #[test]
    fn test_migrate_dependency_ports_preserves_custom_urls() {
        let mut cfg = AppConfig::default();
        cfg.version = 1;
        cfg.services.qdrant = "http://192.168.1.20:6333".into();
        cfg.services.fastplan = "http://127.0.0.1:28080".into();

        assert!(migrate_legacy_config(&mut cfg));
        assert_eq!(cfg.services.qdrant, "http://192.168.1.20:6333");
        assert_eq!(cfg.services.fastplan, "http://127.0.0.1:28080");
    }

    #[test]
    fn test_migrate_v2_restored_fastplan_port() {
        let mut cfg = AppConfig::default();
        cfg.version = 2;
        cfg.services.fastplan = "http://127.0.0.1:8080".into();

        assert!(migrate_legacy_config(&mut cfg));
        assert_eq!(cfg.version, CURRENT_CONFIG_VERSION);
        assert_eq!(cfg.services.fastplan, "http://127.0.0.1:23180");
    }

    #[test]
    fn test_config_manager_persists_v2_fastplan_port_recovery() {
        let directory = tempfile::tempdir().expect("create temp config directory");
        let config_path = directory.path().join("config.json");
        let mut previous = AppConfig::default();
        previous.version = 2;
        previous.services.fastplan = "http://127.0.0.1:8080".into();
        std::fs::write(
            &config_path,
            serde_json::to_string_pretty(&previous).unwrap(),
        )
        .expect("write v2 config");

        ConfigManager::new(directory.path()).expect("load and migrate v2 config");

        let saved: AppConfig = serde_json::from_str(
            &std::fs::read_to_string(config_path).expect("read migrated config"),
        )
        .expect("parse migrated config");
        assert_eq!(saved.version, CURRENT_CONFIG_VERSION);
        assert_eq!(saved.services.fastplan, "http://127.0.0.1:23180");
    }

    #[test]
    fn test_config_manager_persists_legacy_port_migration() {
        let directory = tempfile::tempdir().expect("create temp config directory");
        let config_path = directory.path().join("config.json");
        let mut legacy = AppConfig::default();
        legacy.version = 1;
        legacy.services.qdrant = "http://127.0.0.1:6333".into();
        legacy.services.llama_server = "http://127.0.0.1:8089".into();
        legacy.services.fastkg = "http://127.0.0.1:3000".into();
        legacy.services.fastplan = "http://127.0.0.1:8080".into();
        std::fs::write(&config_path, serde_json::to_string_pretty(&legacy).unwrap())
            .expect("write legacy config");

        ConfigManager::new(directory.path()).expect("load and migrate legacy config");

        let saved: AppConfig = serde_json::from_str(
            &std::fs::read_to_string(config_path).expect("read migrated config"),
        )
        .expect("parse migrated config");
        assert_eq!(saved.version, CURRENT_CONFIG_VERSION);
        assert_eq!(saved.services.qdrant, "http://127.0.0.1:23060");
        assert_eq!(saved.services.llama_server, "http://127.0.0.1:23100");
        assert_eq!(saved.services.fastkg, "http://127.0.0.1:23140");
        assert_eq!(saved.services.fastplan, "http://127.0.0.1:23180");
    }

    #[test]
    fn test_update_field() {
        let mut cfg = AppConfig::default();
        let new_url = serde_json::json!("http://127.0.0.1:9999");
        apply_dot_path(&mut cfg, "services.fastplan", new_url).unwrap();
        assert_eq!(cfg.services.fastplan, "http://127.0.0.1:9999");
    }

    #[test]
    fn test_set_json_path() {
        let mut root = serde_json::json!({"a": {"b": 1}});
        set_json_path(&mut root, &["a", "b"], serde_json::json!(42)).unwrap();
        assert_eq!(root["a"]["b"], 42);
    }
}
