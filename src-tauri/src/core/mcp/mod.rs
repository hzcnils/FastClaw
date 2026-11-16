// FastClaw · MCP 协议支持
//
// 完整的 MCP 集成模块：
// - McpClient: 支持 Stdio / Legacy SSE / Streamable HTTP 三种传输协议
// - McpManager: 多 Server 连接管理、工具发现、工具调用
// - 从 FastPlan MCP 模块适配，将 FastPlanError → AppError
//
// ★ 锁策略：SharedMcpManager 的写锁只保护 HashMap 突变；
//   connect / call_tool / disconnect 的网络 I/O 必须在锁外执行，
//   避免一个慢 MCP 服务器阻塞所有会话。

pub mod client;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::core::error::AppError;
pub use client::McpClient;

// ═══════════════════════════════════════════════════════════════
//  传输协议 & 工具定义
// ═══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransportType {
    Stdio { command: String, args: Vec<String> },
    Sse { url: String },
    WebSocket { url: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub input_schema: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub transport: TransportType,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub environment: HashMap<String, String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default = "default_true")]
    pub auto_index: bool,
    #[serde(default)]
    pub keywords: Vec<String>,
}

fn default_true() -> bool {
    true
}

impl McpServerConfig {
    pub fn stdio(name: impl Into<String>, command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            name: name.into(),
            transport: TransportType::Stdio {
                command: command.into(),
                args,
            },
            enabled: true,
            environment: HashMap::new(),
            description: String::new(),
            auto_start: false,
            auto_index: true,
            keywords: Vec::new(),
        }
    }
    pub fn sse(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            transport: TransportType::Sse { url: url.into() },
            enabled: true,
            environment: HashMap::new(),
            description: String::new(),
            auto_start: false,
            auto_index: true,
            keywords: Vec::new(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  状态 & 信息
// ═══════════════════════════════════════════════════════════════

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

#[derive(Clone, Debug)]
pub struct McpServerInfo {
    pub config: McpServerConfig,
    pub status: ServerStatus,
    pub tools: Vec<McpTool>,
}

// ═══════════════════════════════════════════════════════════════
//  McpManager（多服务管理器，使用真实 McpClient）
// ═══════════════════════════════════════════════════════════════

pub struct McpManager {
    clients: HashMap<String, Arc<McpClient>>,
    server_infos: HashMap<String, McpServerInfo>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            server_infos: HashMap::new(),
        }
    }

    /// 注册 MCP 服务（仅保存配置，不连接）
    pub fn register_server(&mut self, id: String, config: McpServerConfig) {
        info!("📦 注册 MCP 服务: {} ({})", id, config.name);
        self.server_infos.insert(
            id,
            McpServerInfo {
                config,
                status: ServerStatus::Stopped,
                tools: Vec::new(),
            },
        );
    }

    /// 从运行时彻底移除（需先 disconnect）
    pub fn unregister_server(&mut self, id: &str) {
        self.clients.remove(id);
        if self.server_infos.remove(id).is_some() {
            info!("🗑️ 已从运行时移除 MCP 服务: {}", id);
        }
    }

    /// 获取所有已发现的工具
    pub fn list_all_tools(&self) -> Vec<(String, McpTool)> {
        self.server_infos
            .iter()
            .filter(|(_, info)| info.status == ServerStatus::Running)
            .flat_map(|(sid, info)| info.tools.iter().map(move |t| (sid.clone(), t.clone())))
            .collect()
    }

    pub fn list_servers(&self) -> Vec<String> {
        self.server_infos.keys().cloned().collect()
    }

    /// 供前端展示：所有已注册 Server 及连接状态、工具数量
    pub fn list_server_summaries(&self) -> Vec<serde_json::Value> {
        self.server_infos
            .iter()
            .map(|(id, info)| {
                let (transport_type, command, args, url) = match &info.config.transport {
                    TransportType::Stdio {
                        command: cmd,
                        args: a,
                    } => ("stdio", Some(cmd.clone()), Some(a.clone()), None),
                    TransportType::Sse { url: u } => ("sse", None, None, Some(u.clone())),
                    TransportType::WebSocket { url: u } => {
                        ("websocket", None, None, Some(u.clone()))
                    }
                };
                let (last_status, error_message) = match &info.status {
                    ServerStatus::Running => ("running", None),
                    ServerStatus::Starting => ("connecting", None),
                    ServerStatus::Stopped => ("stopped", None),
                    ServerStatus::Error(msg) => ("error", Some(msg.clone())),
                };
                serde_json::json!({
                    "id": id,
                    "name": info.config.name,
                    "status": info.status,
                    "transport_type": transport_type,
                    "command": command,
                    "args": args,
                    "url": url,
                    "last_status": last_status,
                    "error_message": error_message,
                    "tool_count": info.tools.len(),
                    "description": info.config.description,
                    "enabled": info.config.enabled,
                    "keywords": info.config.keywords,
                    "environment": info.config.environment,
                    "auto_start": info.config.auto_start,
                    "auto_index": info.config.auto_index,
                })
            })
            .collect()
    }
    pub fn get_server_status(&self, id: &str) -> Option<&ServerStatus> {
        self.server_infos.get(id).map(|i| &i.status)
    }
    pub fn get_server_info(&self, id: &str) -> Option<&McpServerInfo> {
        self.server_infos.get(id)
    }
}

pub type SharedMcpManager = Arc<RwLock<McpManager>>;

pub fn create_shared_mcp_manager() -> SharedMcpManager {
    Arc::new(RwLock::new(McpManager::new()))
}

/// 连接指定服务：网络 I/O 在写锁外；仅短暂持锁更新状态 / 插入 client
pub async fn connect_server_shared(
    mcp: &SharedMcpManager,
    server_id: &str,
) -> Result<(), AppError> {
    {
        let guard = mcp.read().await;
        if guard.clients.contains_key(server_id) {
            return Ok(());
        }
        if !guard.server_infos.contains_key(server_id) {
            return Err(AppError::NotFound(format!("MCP 服务不存在: {}", server_id)));
        }
    }

    let config = {
        let mut guard = mcp.write().await;
        if guard.clients.contains_key(server_id) {
            return Ok(());
        }
        let info = guard
            .server_infos
            .get_mut(server_id)
            .ok_or_else(|| AppError::NotFound(format!("MCP 服务不存在: {}", server_id)))?;
        info.status = ServerStatus::Starting;
        info!("🔌 连接 MCP 服务: {} ({})", server_id, info.config.name);
        info.config.clone()
    };

    let connect_result = McpClient::connect(&config).await;

    match connect_result {
        Ok(client) => {
            let tools = match client.list_tools().await {
                Ok(tools) => {
                    info!("🔧 发现 {} 个工具 ({})", tools.len(), server_id);
                    tools
                }
                Err(e) => {
                    warn!("工具发现失败 {}: {}", server_id, e);
                    Vec::new()
                }
            };
            let client = Arc::new(client);
            {
                let mut guard = mcp.write().await;
                if guard.clients.contains_key(server_id) {
                    // 竞态：另一任务已连上 — 丢弃本次连接
                    drop(guard);
                    client.disconnect().await;
                    return Ok(());
                }
                if let Some(info) = guard.server_infos.get_mut(server_id) {
                    info.tools = tools;
                    info.status = ServerStatus::Running;
                }
                guard.clients.insert(server_id.to_string(), client);
            }
            info!("✅ MCP 服务已连接: {}", server_id);
            Ok(())
        }
        Err(e) => {
            let mut guard = mcp.write().await;
            let still_starting = guard
                .server_infos
                .get(server_id)
                .map(|info| matches!(info.status, ServerStatus::Starting))
                .unwrap_or(false)
                && !guard.clients.contains_key(server_id);
            if still_starting {
                if let Some(info) = guard.server_infos.get_mut(server_id) {
                    info.status = ServerStatus::Error(e.to_string());
                }
            }
            Err(e)
        }
    }
}

/// 断开连接：先短暂写锁摘掉 client，再在锁外 disconnect
pub async fn disconnect_server_shared(mcp: &SharedMcpManager, server_id: &str) {
    let client = {
        let mut guard = mcp.write().await;
        let client = guard.clients.remove(server_id);
        if let Some(info) = guard.server_infos.get_mut(server_id) {
            info.status = ServerStatus::Stopped;
            info.tools.clear();
        }
        client
    };
    if let Some(client) = client {
        client.disconnect().await;
    }
    info!("🔌 已断开 MCP 服务: {}", server_id);
}

/// 断开全部：逐个 disconnect_server_shared（I/O 不占全局写锁）
pub async fn disconnect_all_shared(mcp: &SharedMcpManager) {
    let ids: Vec<String> = {
        let guard = mcp.read().await;
        guard.clients.keys().cloned().collect()
    };
    for id in ids {
        disconnect_server_shared(mcp, &id).await;
    }
}

/// 调用工具：懒连接 + 网络 I/O 均在写锁外
pub async fn call_tool_shared(
    mcp: &SharedMcpManager,
    server_id: &str,
    tool_name: &str,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    let client = {
        let guard = mcp.read().await;
        guard.clients.get(server_id).cloned()
    };

    let client = match client {
        Some(c) => c,
        None => {
            // 未注册时给出明确错误；已注册则懒连接
            {
                let guard = mcp.read().await;
                if !guard.server_infos.contains_key(server_id) {
                    return Err(AppError::NotFound(format!(
                        "MCP 服务 '{}' 未注册",
                        server_id
                    )));
                }
            }
            info!("懒连接 MCP 服务: {}", server_id);
            connect_server_shared(mcp, server_id).await?;
            let guard = mcp.read().await;
            guard
                .clients
                .get(server_id)
                .cloned()
                .ok_or_else(|| AppError::Mcp(format!("连接后仍找不到客户端: {}", server_id)))?
        }
    };

    info!("🔧 调用 MCP 工具: {}:{}", server_id, tool_name);
    client.call_tool(tool_name, arguments).await
}
