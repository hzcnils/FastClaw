// FastClaw · WSP 类型定义
// 与数据库表结构一一对应的 Rust 结构体

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;

/// WSP 服务注册信息（对应 wsp_services 表）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WspService {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub version: String,
    pub category: String,
    pub category_name: Option<String>,
    pub sub_category_name: Option<String>,
    /// exe 文件路径
    pub exe_path: String,
    /// 工作目录
    pub work_dir: Option<String>,
    /// 启动参数（JSON 数组）
    pub args: Option<String>,
    /// 环境变量（JSON 对象）
    pub env_vars: Option<String>,
    /// 通信协议：http / websocket / hybrid
    pub protocol: String,
    pub host: String,
    pub port: i32,
    /// API 基础路径
    pub base_path: String,
    /// WebSocket 路径
    pub ws_path: Option<String>,
    /// 健康检查端点
    pub health_endpoint: String,
    // 资源限制
    pub cpu_limit: Option<f64>,
    pub memory_limit_mb: Option<i32>,
    pub gpu_required: bool,
    pub gpu_device_id: Option<i32>,
    // 启动行为
    pub auto_start: bool,
    pub start_delay_ms: i32,
    pub start_priority: i32,
    pub restart_policy: String,
    pub max_restarts: i32,
    pub restart_cooldown_ms: i32,
    // 健康检查
    pub health_check_interval_ms: i32,
    pub health_check_timeout_ms: i32,
    pub startup_timeout_ms: i32,
    /// 依赖的其他服务名（JSON 数组）
    pub depends_on: Option<String>,
    pub is_enabled: bool,
    pub is_installed: bool,
    pub sort_order: i32,
    pub extra_config: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Default for WspService {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            display_name: String::new(),
            description: None,
            version: String::new(),
            category: String::new(),
            category_name: None,
            sub_category_name: None,
            exe_path: String::new(),
            work_dir: None,
            args: None,
            env_vars: None,
            protocol: "http".to_string(),
            host: "127.0.0.1".to_string(),
            port: 0,
            base_path: "/".to_string(),
            ws_path: None,
            health_endpoint: "/health".to_string(),
            cpu_limit: None,
            memory_limit_mb: None,
            gpu_required: false,
            gpu_device_id: None,
            auto_start: false,
            start_delay_ms: 0,
            start_priority: 0,
            restart_policy: "no".to_string(),
            max_restarts: 0,
            restart_cooldown_ms: 0,
            health_check_interval_ms: 5000,
            health_check_timeout_ms: 3000,
            startup_timeout_ms: 30000,
            depends_on: None,
            is_enabled: true,
            is_installed: true,
            sort_order: 0,
            extra_config: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

impl WspService {
    /// 获取完整的服务基地址 URL
    pub fn base_url(&self) -> String {
        format!("http://{}:{}{}", self.host, self.port, self.base_path)
    }

    /// 获取健康检查完整 URL
    pub fn health_url(&self) -> String {
        format!("http://{}:{}{}", self.host, self.port, self.health_endpoint)
    }

    /// 解析启动参数
    pub fn parsed_args(&self) -> Vec<String> {
        self.args
            .as_ref()
            .and_then(|a| serde_json::from_str(a).ok())
            .unwrap_or_default()
    }

    /// 解析环境变量
    pub fn parsed_env_vars(&self) -> std::collections::HashMap<String, String> {
        self.env_vars
            .as_ref()
            .and_then(|e| serde_json::from_str(e).ok())
            .unwrap_or_default()
    }

    /// 解析依赖服务列表
    pub fn parsed_depends_on(&self) -> Vec<String> {
        self.depends_on
            .as_ref()
            .and_then(|d| serde_json::from_str(d).ok())
            .unwrap_or_default()
    }
}

/// WSP 运行状态（对应 wsp_runtime_status 表）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WspStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
    Crashed,
}

impl std::fmt::Display for WspStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WspStatus::Stopped => write!(f, "stopped"),
            WspStatus::Starting => write!(f, "starting"),
            WspStatus::Running => write!(f, "running"),
            WspStatus::Stopping => write!(f, "stopping"),
            WspStatus::Error => write!(f, "error"),
            WspStatus::Crashed => write!(f, "crashed"),
        }
    }
}

/// WSP 运行状态更新（写入 wsp_runtime_status 表）
#[derive(Debug, Clone)]
pub struct WspRuntimeUpdate {
    pub status: String,
    pub pid: Option<i32>,
    pub cpu_percent: Option<f64>,
    pub memory_mb: Option<f64>,
    pub gpu_memory_mb: Option<f64>,
    pub started_at: Option<String>,
    pub stopped_at: Option<String>,
    pub error_message: Option<String>,
}

/// WSP 服务 + 运行状态联合查询结果（前端展示用）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WspServiceWithStatus {
    // 服务基本信息
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub version: String,
    pub category: String,
    pub category_name: Option<String>,
    pub sub_category_name: Option<String>,
    pub exe_path: String, // ← 新增
    pub port: i32,
    pub protocol: String,
    pub auto_start: bool,
    pub is_enabled: bool,
    pub host: String,      // ← 新增
    pub base_path: String, // ← 新增
    #[sqlx(default)]
    pub computed_url: String, // 后端预计算的完整服务基础 URL（host + base_path）
    // 运行状态（来自 LEFT JOIN wsp_runtime_status）
    pub pid: Option<i32>,
    pub runtime_status: Option<String>,
    pub cpu_percent: Option<f64>,
    pub memory_mb: Option<f64>,
    pub gpu_memory_mb: Option<f64>,
    pub request_count: Option<i32>,
    pub error_count: Option<i32>,
    pub avg_latency_ms: Option<f64>,
    pub restart_count: Option<i32>,
    pub runtime_started_at: Option<String>,
    pub runtime_error: Option<String>,
}

/// WSP 事件日志（对应 wsp_event_logs 表）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WspEventLog {
    pub id: String,
    pub service_id: String,
    pub event_type: String,
    pub pid: Option<i32>,
    pub exit_code: Option<i32>,
    pub crash_reason: Option<String>,
    pub cpu_percent: Option<f64>,
    pub memory_mb: Option<f64>,
    pub duration_ms: Option<i64>,
    pub detail: Option<String>,
    pub created_at: String,
}

/// WSP 调用日志（对应 wsp_call_logs 表）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WspCallLog {
    pub id: String,
    pub service_id: String,
    pub endpoint_id: Option<String>,
    pub user_id: String,
    pub conversation_id: Option<String>,
    pub method: String,
    pub path: String,
    pub status_code: Option<i32>,
    pub request_size: Option<i64>,
    pub response_size: Option<i64>,
    pub latency_ms: Option<i64>,
    pub is_success: bool,
    pub error_message: Option<String>,
}

/// 快速启动配置（用于命令+端口 或 压缩包路径）
#[derive(Debug, Clone)]
pub struct QuickStartConfig {
    pub command: String,
    pub port: i32,
    pub work_dir: Option<String>,
    pub args: Option<Vec<String>>,
    pub env_vars: Option<HashMap<String, String>>,
}

/// WSP 崩溃原因分类
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrashReason {
    /// 内存溢出
    Oom,
    /// 端口被占用
    PortConflict,
    /// 配置错误
    ConfigError,
    /// 依赖服务未启动
    DependencyFail,
    /// exe 文件不存在
    ExeNotFound,
    /// 启动超时
    Timeout,
    /// 未知原因
    Unknown,
}

impl std::fmt::Display for CrashReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CrashReason::Oom => write!(f, "oom"),
            CrashReason::PortConflict => write!(f, "port_conflict"),
            CrashReason::ConfigError => write!(f, "config_error"),
            CrashReason::DependencyFail => write!(f, "dependency_fail"),
            CrashReason::ExeNotFound => write!(f, "exe_not_found"),
            CrashReason::Timeout => write!(f, "timeout"),
            CrashReason::Unknown => write!(f, "unknown"),
        }
    }
}
