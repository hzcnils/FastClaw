// FastClaw · 全局应用状态（整合版）
// 聚合 FastClaw 核心引擎 + FastPlan + FastKg

use crate::core::config::ConfigManager;
use crate::core::file2md::File2MdManager;
use crate::core::intent::IntentRouter;
use crate::core::llm::LlmManager;
use crate::core::mcp::{create_shared_mcp_manager, SharedMcpManager};
use crate::core::session_plan::handler::FastClawAcpHandler;
use crate::core::tools::registry::{create_shared_tool_registry, SharedToolRegistry};
use crate::core::wsp::manager::WspManager;
use crate::db::Database;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use tokio::task::AbortHandle;

/// 全局关闭标记：由前端发起的退出命令设置，用于避免 CloseRequested handler 重复清理
pub static SHUTDOWN_DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 启动完成标记：setup() 完成后置为 true，用于阻止启动阶段中的窗口隐藏
pub static STARTUP_COMPLETE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// FastPlan 侧载进程 PID（0 表示未运行）
/// 由 wsp.rs 中的 start_fastplan_inner / stop_fastplan_inner 维护
pub static FASTPLAN_PID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// KG 服务 PID 追踪 — 仅用于清理自己启动的进程
#[derive(Debug, Default, Clone)]
pub struct KgServicePids {
    pub qdrant: Option<u32>,
    pub fastkg: Option<u32>,
    pub llama_server: Option<u32>,
    pub fastplan: Option<u32>,
    pub fastplan_connection: Option<crate::commands::fastplan_connection::FastPlanConnectionInfo>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    Planning,
}

pub struct AppState {
    pub db: Database,
    pub mode: Arc<RwLock<AppMode>>,
    pub llm: Arc<LlmManager>,
    pub wsp: Arc<WspManager>,
    pub file2md: Arc<File2MdManager>,
    pub mcp: SharedMcpManager,
    pub tool_registry: SharedToolRegistry,

    /// 应用配置管理器（config.json：本地服务 URL + HTTP 端口）
    pub app_config: Arc<ConfigManager>,

    /// 依赖服务运行身份监督器。端口只用于发现冲突，停止授权以这里记录的进程指纹为准。
    pub dependency_services: Arc<crate::core::dependency_services::DependencyServiceSupervisor>,
    pub dependency_recovery: Arc<crate::core::dependency_services::manager::DependencyRecoveryManager>,

    /// 进行中的流式任务 abort 句柄（key = conversation_id）
    /// 由 send_message_stream 注册、stop_stream 触发 abort。
    pub active_streams: Arc<RwLock<HashMap<String, AbortHandle>>>,

    /// BERT 意图路由（None 表示模型未加载或已禁用）
    pub intent_router: Option<Arc<IntentRouter>>,

    /// KG 服务进程 PID 追踪（用于安全清理，只杀自己启动的进程）
    pub kg_pids: Arc<RwLock<KgServicePids>>,

    /// HTTP 服务器实际绑定的端口（5179 被占用时可能漂移到 5180-5189）
    pub http_port: Arc<std::sync::atomic::AtomicU16>,

    /// 两步截图 IPC：capture_start 把每个显示器的 JPEG 字节存这里，capture_get_all_monitor_pixels 取走
    pub pending_screenshots: Arc<Mutex<Vec<Vec<u8>>>>,

    /// 独立遮罩窗口截图：capture_start 把本次截图的元数据（尺寸/偏移/显示器/窗口列表）
    /// 存这里，遮罩窗口就绪后调用 capture_take_pending 一次性取走元数据 + 各屏图像。
    pub pending_metadata: Arc<Mutex<Option<crate::commands::screenshot::CaptureMetadata>>>,

    /// ACP 处理器（桌面端「停止生成」与规划确认）
    pub acp_handler: Option<std::sync::Arc<FastClawAcpHandler>>,
}

impl AppState {
    pub async fn new(
        db: Database,
        kg_pids: Arc<RwLock<KgServicePids>>,
        app_config: Arc<ConfigManager>,
    ) -> anyhow::Result<Self> {
        let llm = Arc::new(LlmManager::new(db.pool().await.clone()).await?);
        let wsp = Arc::new(WspManager::new(db.pool().await.clone()));
        let file2md = Arc::new(File2MdManager::new());
        let mcp = create_shared_mcp_manager();
        let tool_registry = create_shared_tool_registry();

        Ok(Self {
            db,
            mode: Arc::new(RwLock::new(AppMode::Normal)),
            llm,
            wsp,
            file2md,
            mcp,
            tool_registry,
            app_config,
            dependency_services: crate::core::dependency_services::dependency_service_supervisor(),
            dependency_recovery: Arc::new(crate::core::dependency_services::manager::DependencyRecoveryManager::default()),
            active_streams: Arc::new(RwLock::new(HashMap::new())),
            intent_router: None, // 在 main.rs 的 setup 中异步加载
            kg_pids,
            http_port: Arc::new(std::sync::atomic::AtomicU16::new(5179)),
            pending_screenshots: Arc::new(Mutex::new(Vec::new())),
            pending_metadata: Arc::new(Mutex::new(None)),
            acp_handler: None,
        })
    }

    pub async fn set_mode(&self, mode: AppMode) {
        let mut current = self.mode.write().await;
        *current = mode;
    }

    pub async fn get_mode(&self) -> AppMode {
        self.mode.read().await.clone()
    }
}
