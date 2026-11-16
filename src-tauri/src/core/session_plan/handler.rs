//! FastClawAcpHandler — 桌面端会话控制入口
//!
//! 实现 AcpHandler trait，向桌面端提供：
//! - session/cancel：停止生成
//! - session/confirm-plan / resume-plan / cancel-plan：规划确认 / 继续 / 取消

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::core::llm::{LlmManager, ToolDefinition, ToolFunctionDef};
use crate::core::mcp::SharedMcpManager;
use crate::db::Database;
use crate::state::KgServicePids;

use super::protocol::{AcpEvent, SessionEvent};

// ---------------------------------------------------------------------------
// AcpHandler trait (re-export for convenience)
// ---------------------------------------------------------------------------

#[async_trait]
pub trait AcpHandler: Send + Sync {
    async fn handle_request(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, (i32, String)>;
}

// ---------------------------------------------------------------------------
// 会话入站消息
// ---------------------------------------------------------------------------

/// 从手机端发送到会话任务的消息
#[derive(Debug, Clone)]
pub struct SessionInbox {
    pub content: String,
    pub model: Option<String>,
    pub permission_mode: Option<String>,
    pub mcp_server_ids: Vec<String>,
    pub client_msg_id: Option<String>,
    pub images: Vec<String>, // base64 data URLs
    pub attachments: Vec<Value>,
    pub enable_thinking: bool,
    pub task_plan: bool,
}

// ---------------------------------------------------------------------------
// 会话运行时状态
// ---------------------------------------------------------------------------

struct SessionState {
    msg_tx: mpsc::UnboundedSender<SessionInbox>,
    cancel_token: CancellationToken,
    provider_code: String,
    model: String,
    last_activity: std::time::Instant,
    plan_wait: super::fastplan::PlanWaitSlot,
}

/// 会话不活跃超时（30分钟）
const SESSION_IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

// ---------------------------------------------------------------------------
// FastClawAcpHandler
// ---------------------------------------------------------------------------

/// 权限请求状态
struct PendingPermission {
    /// 审批结果发送端
    tx: oneshot::Sender<bool>,
    /// 工具名（日志用）
    tool_name: String,
    /// 是否高风险（仅桌面本地可批；云端 session/permission 一律拒绝）
    high_risk: bool,
}

pub struct FastClawAcpHandler {
    /// 事件广播通道（pub: run_stream 需要访问以同步桌面消息到手机）
    pub event_tx: broadcast::Sender<SessionEvent>,
    /// 活跃会话: session_id → SessionState
    sessions: Arc<Mutex<HashMap<String, SessionState>>>,
    /// 取消令牌注册表（用于 session/cancel）
    cancel_tokens: Arc<Mutex<HashMap<String, CancellationToken>>>,
    /// 待审批的权限请求: request_id → PendingPermission
    pending_permissions: Arc<Mutex<HashMap<String, PendingPermission>>>,
    /// 数据库连接池
    db: Database,
    /// LLM 管理器
    llm: Arc<LlmManager>,
    /// MCP 管理器
    mcp: SharedMcpManager,
    /// Skills 目录路径
    skills_dir: std::path::PathBuf,
    /// 设备 ID
    device_id: String,
    /// 本机 FastPlan sidecar 连接（与桌面规划共用）
    kg_pids: Arc<RwLock<KgServicePids>>,
}

impl FastClawAcpHandler {
    /// 桌面端本地审批远程工具调用（唯一合法审批入口；云端 session/permission 已禁用）
    pub fn resolve_permission_local(
        &self,
        request_id: &str,
        allow: bool,
    ) -> Result<serde_json::Value, String> {
        let pending = {
            let mut perms = self
                .pending_permissions
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            perms.remove(request_id)
        };
        match pending {
            Some(p) => {
                let tool = p.tool_name.clone();
                let _ = p.tx.send(allow);
                info!(
                    request_id = %request_id,
                    allow,
                    tool = %tool,
                    "Local desktop permission resolved"
                );
                Ok(serde_json::json!({
                    "status": "resolved",
                    "request_id": request_id,
                    "allowed": allow,
                    "tool_name": tool,
                }))
            }
            None => Err(format!("权限请求不存在或已过期: {}", request_id)),
        }
    }

    /// 列出当前待本地审批的工具请求
    pub fn list_pending_permissions(&self) -> Vec<serde_json::Value> {
        let perms = self
            .pending_permissions
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        perms
            .iter()
            .map(|(id, p)| {
                serde_json::json!({
                    "request_id": id,
                    "tool_name": p.tool_name,
                    "high_risk": p.high_risk,
                })
            })
            .collect()
    }

    pub fn new(
        db: Database,
        llm: Arc<LlmManager>,
        mcp: SharedMcpManager,
        kg_pids: Arc<RwLock<KgServicePids>>,
    ) -> (Arc<Self>, broadcast::Receiver<SessionEvent>) {
        let (tx, rx) = broadcast::channel(1024);
        let handler = Arc::new(Self {
            event_tx: tx,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            cancel_tokens: Arc::new(Mutex::new(HashMap::new())),
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
            db,
            llm,
            mcp,
            skills_dir: std::path::PathBuf::from("skills"),
            device_id: String::new(),
            kg_pids,
        });

        // ★ 启动后台收割：每60s清理超过30min不活跃的会话
        let sessions_ref = Arc::clone(&handler.sessions);
        let cancel_tokens_ref = Arc::clone(&handler.cancel_tokens);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                let now = std::time::Instant::now();
                let stale: Vec<String> = {
                    let sessions = sessions_ref.lock().unwrap_or_else(|e| e.into_inner());
                    sessions
                        .iter()
                        .filter(|(_, s)| now.duration_since(s.last_activity) > SESSION_IDLE_TIMEOUT)
                        .map(|(id, _)| id.clone())
                        .collect()
                };
                for sid in &stale {
                    info!(sid = %sid, "Reaping stale session");
                    if let Ok(tokens) = cancel_tokens_ref.lock() {
                        if let Some(token) = tokens.get(sid) {
                            token.cancel();
                        }
                    }
                }
                if !stale.is_empty() {
                    info!(count = stale.len(), "Reaped stale sessions");
                }
            }
        });

        (handler, rx)
    }

    fn emit(&self, session_id: &str, event: AcpEvent) {
        let _ = self.event_tx.send(SessionEvent {
            session_id: session_id.to_string(),
            event,
            origin: "remote",
        });
    }

    fn emit_session_updated(&self, session_id: &str, action: &str, title: Option<&str>) {
        self.emit_session_updated_full(session_id, action, title, None);
    }

    fn emit_session_updated_full(
        &self,
        session_id: &str,
        action: &str,
        title: Option<&str>,
        model: Option<&str>,
    ) {
        self.emit(
            session_id,
            AcpEvent::SessionUpdated {
                action: action.to_string(),
                title: title.map(|s| s.to_string()),
                model: model.map(|s| s.to_string()),
                task_plan: None,
                successor_session_id: None,
                execution_mode: None,
            },
        );
    }

    fn emit_task_plan_updated(&self, session_id: &str, enabled: bool) {
        self.emit(
            session_id,
            AcpEvent::SessionUpdated {
                action: "task_plan".into(),
                title: None,
                model: None,
                task_plan: Some(enabled),
                successor_session_id: None,
                execution_mode: None,
            },
        );
    }

    fn emit_execution_mode_updated(&self, session_id: &str, mode: &str) {
        self.emit(
            session_id,
            AcpEvent::SessionUpdated {
                action: "execution_mode".into(),
                title: None,
                model: None,
                task_plan: None,
                successor_session_id: None,
                execution_mode: Some(mode.to_string()),
            },
        );
    }

    fn interrupt_generation(&self, sid: &str, drop_task: bool) {
        if drop_task {
            if let Ok(mut tokens) = self.cancel_tokens.lock() {
                if let Some(old) = tokens.remove(sid) {
                    old.cancel();
                }
            }
            if let Ok(mut sessions) = self.sessions.lock() {
                sessions.remove(sid);
            }
            return;
        }
        let fresh = CancellationToken::new();
        if let Ok(mut tokens) = self.cancel_tokens.lock() {
            if let Some(old) = tokens.insert(sid.to_string(), fresh.clone()) {
                old.cancel();
            }
        }
        if let Ok(mut sessions) = self.sessions.lock() {
            if let Some(state) = sessions.get_mut(sid) {
                state.cancel_token.cancel();
                state.cancel_token = fresh;
            }
        }
    }

    async fn rename_session(&self, p: &Value) -> Result<Value, (i32, String)> {
        let sid = p["session_id"]
            .as_str()
            .or_else(|| p["id"].as_str())
            .ok_or((-32602, "Missing session_id".into()))?;
        let title = p["title"]
            .as_str()
            .or_else(|| p["name"].as_str())
            .unwrap_or("")
            .trim();
        if title.is_empty() {
            return Err((-32602, "Missing title".into()));
        }
        let pool = self.db.pool().await;
        crate::db::repo::conversations::update(&pool, sid, Some(title.to_string()), None, None)
            .await
            .map_err(|e| (-32603, format!("重命名失败: {e}")))?;
        self.emit_session_updated(sid, "renamed", Some(title));
        Ok(serde_json::json!({ "session_id": sid, "title": title, "renamed": true }))
    }

    async fn destroy_session(&self, p: &Value) -> Result<Value, (i32, String)> {
        let sid = p["session_id"]
            .as_str()
            .or_else(|| p["id"].as_str())
            .ok_or((-32602, "Missing session_id".into()))?;
        self.interrupt_generation(sid, true);
        let pool = self.db.pool().await;
        crate::db::repo::conversations::delete(&pool, sid)
            .await
            .map_err(|e| (-32603, format!("删除对话失败: {e}")))?;
        self.emit_session_updated(sid, "deleted", None);
        Ok(serde_json::json!({ "destroyed": true, "session_id": sid }))
    }

    async fn truncate_session(&self, p: &Value) -> Result<Value, (i32, String)> {
        let sid = p["session_id"]
            .as_str()
            .ok_or((-32602, "Missing session_id".into()))?;
        let from_id = p["from_message_id"]
            .as_str()
            .or_else(|| p["fromMessageId"].as_str());
        let after_id = p["after_message_id"]
            .as_str()
            .or_else(|| p["afterMessageId"].as_str());
        if from_id.unwrap_or("").is_empty() && after_id.unwrap_or("").is_empty() {
            return Err((-32602, "Missing from_message_id or after_message_id".into()));
        }
        self.interrupt_generation(sid, false);
        let pool = self.db.pool().await;
        let deleted =
            crate::db::repo::messages::truncate_conversation(&pool, sid, from_id, after_id)
                .await
                .map_err(|e| (-32603, format!("截断对话失败: {e}")))?;
        self.emit_session_updated(sid, "truncated", None);
        Ok(serde_json::json!({ "truncated": true, "deleted": deleted, "session_id": sid }))
    }

    async fn set_session_model(&self, p: &Value) -> Result<Value, (i32, String)> {
        let sid = p["session_id"]
            .as_str()
            .or_else(|| p["id"].as_str())
            .ok_or((-32602, "Missing session_id".into()))?;
        let model = p["model"]
            .as_str()
            .or_else(|| p["model_id"].as_str())
            .unwrap_or("")
            .trim();
        if model.is_empty() {
            return Err((-32602, "Missing model".into()));
        }
        let pool = self.db.pool().await;
        crate::db::repo::conversations::update_model_id(&pool, sid, model)
            .await
            .map_err(|e| (-32603, format!("更新会话模型失败: {e}")))?;
        super::config::set_current_ui_model(Some(model.to_string()));
        if let Ok(mut sessions) = self.sessions.lock() {
            if let Some(state) = sessions.get_mut(sid) {
                state.model = model.to_string();
            }
        }
        self.emit_session_updated_full(sid, "model", None, Some(model));
        Ok(serde_json::json!({ "session_id": sid, "model": model }))
    }

    async fn set_session_execution_mode(&self, p: &Value) -> Result<Value, (i32, String)> {
        let sid = p["session_id"]
            .as_str()
            .or_else(|| p["id"].as_str())
            .ok_or((-32602, "Missing session_id".into()))?;
        let raw = p
            .get("mode")
            .or_else(|| p.get("execution_mode"))
            .or_else(|| p.get("executionMode"))
            .and_then(|v| v.as_str())
            .ok_or((-32602, "Missing mode".into()))?;
        let mode = super::fastplan::PlanExecutionMode::parse(raw);
        let pool = self.db.pool().await;
        super::fastplan::persist_conversation_execution_mode(&pool, sid, mode)
            .await
            .map_err(|e| (-32603, e))?;
        self.emit_execution_mode_updated(sid, mode.as_str());
        Ok(serde_json::json!({
            "session_id": sid,
            "executionMode": mode.as_str(),
        }))
    }

    fn send_session_plan_control(
        &self,
        p: &Value,
        ctrl: super::fastplan::PlanControl,
    ) -> Result<Value, (i32, String)> {
        let sid = p["session_id"]
            .as_str()
            .or_else(|| p["id"].as_str())
            .ok_or((-32602, "Missing session_id".into()))?;
        let slot = {
            let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
            sessions.get(sid).map(|s| s.plan_wait.clone())
        };
        let expected = match p.get("expected_wait_id") {
            None | Some(Value::Null) => None,
            Some(Value::String(id)) if !id.trim().is_empty() => Some(id.as_str()),
            _ => return Err((-32602, "Invalid expected_wait_id".into())),
        };
        let receipt = super::fastplan::send_plan_control(slot.as_ref(), ctrl, expected)
            .map_err(|error| (-32009, error))?;
        let action = match ctrl {
            super::fastplan::PlanControl::Confirm => "confirm",
            super::fastplan::PlanControl::Resume => "resume",
            super::fastplan::PlanControl::Cancel => "cancel",
        };
        if receipt.is_none() {
            // Legacy desktop-owned execution: forward without fabricating a successful state.
            self.emit(
                sid,
                AcpEvent::Plan {
                    event: serde_json::json!({
                        "type": "plan_control",
                        "action": action,
                        "tree_id": p.get("tree_id"),
                        "node_id": p.get("node_id"),
                    }),
                },
            );
        }
        Ok(serde_json::json!({
            "ok": true,
            "session_id": sid,
            "forwarded": receipt.is_none(),
            "duplicate": receipt == Some(true),
            "expected_wait_id": expected,
        }))
    }

    /// App/远程请求话题自动命名：默认用当前会话模型。
    async fn generate_session_topic_title(&self, p: &Value) -> Result<Value, (i32, String)> {
        let sid = p["session_id"]
            .as_str()
            .or_else(|| p["id"].as_str())
            .ok_or((-32602, "Missing session_id".into()))?;
        let user_message = p["user_message"]
            .as_str()
            .or_else(|| p["userMessage"].as_str())
            .or_else(|| p["content"].as_str())
            .unwrap_or("")
            .trim();
        if user_message.is_empty() {
            return Err((-32602, "Missing user_message".into()));
        }
        let preferred = p["model"]
            .as_str()
            .or_else(|| p["model_id"].as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let pool = self.db.pool().await;
        let title = crate::core::topic_title::maybe_name_conversation_topic(
            &pool,
            self.llm.as_ref(),
            sid,
            user_message,
            preferred.as_deref(),
            true,
        )
        .await
        .map_err(|e| (-32603, e.to_string()))?;
        let title = match title {
            Some(t) => t,
            None => sqlx::query_scalar::<_, String>(
                "SELECT COALESCE(title, '') FROM conversations WHERE id = ?",
            )
            .bind(sid)
            .fetch_one(&*pool)
            .await
            .unwrap_or_default(),
        };
        if !title.is_empty() {
            self.emit_session_updated(sid, "renamed", Some(&title));
        }
        Ok(serde_json::json!({ "session_id": sid, "title": title }))
    }

    async fn like_session_message(&self, p: &Value) -> Result<Value, (i32, String)> {
        let message_id = p["message_id"]
            .as_str()
            .or_else(|| p["messageId"].as_str())
            .unwrap_or("")
            .trim();
        if message_id.is_empty() {
            return Err((-32602, "Missing message_id".into()));
        }
        let is_liked = p["is_liked"]
            .as_i64()
            .or_else(|| p["isLiked"].as_i64())
            .ok_or((-32602, "Missing is_liked".into()))? as i32;
        if !matches!(is_liked, -1 | 0 | 1) {
            return Err((-32602, "is_liked must be -1, 0 or 1".into()));
        }
        let pool = self.db.pool().await;
        let row: Option<(String,)> = sqlx::query_as("SELECT conversation_id FROM messages WHERE id = ?")
            .bind(message_id)
            .fetch_optional(&*pool)
            .await
            .map_err(|e| (-32603, format!("查询消息失败: {e}")))?;
        let conv_id = row.ok_or((-32000, "消息不存在".into()))?.0;
        if let Some(sid) = p["session_id"].as_str().or_else(|| p["sessionId"].as_str()) {
            if !sid.is_empty() && sid != conv_id {
                return Err((-32000, "消息不属于当前会话".into()));
            }
        }
        sqlx::query("UPDATE messages SET is_liked = ? WHERE id = ?")
            .bind(is_liked)
            .bind(message_id)
            .execute(&*pool)
            .await
            .map_err(|e| (-32603, format!("点赞失败: {e}")))?;
        if let Ok(Some((model_instance_id, user_id, stable_model_id, provider_code))) =
            sqlx::query_as::<_, (Option<String>, String, Option<String>, Option<String>)>(
                "SELECT m.model_instance_id, c.user_id, mod.model_id, mod.provider_code \
                 FROM messages m \
                 JOIN conversations c ON c.id = m.conversation_id \
                 LEFT JOIN models mod ON mod.id = m.model_instance_id \
                 WHERE m.id = ?",
            )
            .bind(message_id)
            .fetch_optional(&*pool)
            .await
        {
            if let (Some(mid), Some(smid), Some(pc)) =
                (model_instance_id, stable_model_id, provider_code)
            {
                if is_liked == 1 || is_liked == -1 {
                    let _ = crate::db::repo::model_feedback::upsert(
                        &pool,
                        &user_id,
                        &mid,
                        &smid,
                        &pc,
                        message_id,
                        is_liked,
                    )
                    .await;
                } else {
                    let _ = crate::db::repo::model_feedback::remove(&pool, &user_id, message_id).await;
                }
            }
        }
        Ok(serde_json::json!({
            "ok": true,
            "message_id": message_id,
            "is_liked": is_liked,
            "session_id": conv_id,
        }))
    }

    async fn favorite_session_message(&self, p: &Value) -> Result<Value, (i32, String)> {
        let message_id = p["message_id"]
            .as_str()
            .or_else(|| p["messageId"].as_str())
            .unwrap_or("")
            .trim();
        if message_id.is_empty() {
            return Err((-32602, "Missing message_id".into()));
        }
        let is_favorite = p["is_favorite"]
            .as_bool()
            .or_else(|| p["isFavorite"].as_bool())
            .or_else(|| p["is_favorite"].as_i64().map(|n| n != 0))
            .or_else(|| p["isFavorite"].as_i64().map(|n| n != 0))
            .ok_or((-32602, "Missing is_favorite".into()))?;
        let val: i32 = if is_favorite { 1 } else { 0 };
        let pool = self.db.pool().await;
        let row: Option<(String, String, String, Option<String>)> = sqlx::query_as(
            "SELECT conversation_id, role, content, model_id FROM messages WHERE id = ?",
        )
        .bind(message_id)
        .fetch_optional(&*pool)
        .await
        .map_err(|e| (-32603, format!("查询消息失败: {e}")))?;
        const DEFAULT_USER: &str = "default-user";
        let Some((conv_id, role, content, model_id)) = row else {
            if !is_favorite {
                crate::db::repo::message_favorites::remove(&pool, DEFAULT_USER, message_id)
                    .await
                    .map_err(|e| (-32603, format!("取消收藏失败: {e}")))?;
                return Ok(serde_json::json!({
                    "ok": true,
                    "message_id": message_id,
                    "is_favorite": false,
                }));
            }
            return Err((-32000, "消息不存在".into()));
        };
        if let Some(sid) = p["session_id"].as_str().or_else(|| p["sessionId"].as_str()) {
            if !sid.is_empty() && sid != conv_id {
                return Err((-32000, "消息不属于当前会话".into()));
            }
        }
        sqlx::query("UPDATE messages SET is_favorite = ? WHERE id = ?")
            .bind(val)
            .bind(message_id)
            .execute(&*pool)
            .await
            .map_err(|e| (-32603, format!("收藏失败: {e}")))?;
        if is_favorite {
            let title: String = sqlx::query_scalar(
                "SELECT COALESCE(title, '未命名对话') FROM conversations WHERE id = ?",
            )
            .bind(&conv_id)
            .fetch_optional(&*pool)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "未命名对话".into());
            crate::db::repo::message_favorites::upsert(
                &pool,
                DEFAULT_USER,
                message_id,
                &conv_id,
                &title,
                &role,
                &content,
                model_id.as_deref(),
            )
            .await
            .map_err(|e| (-32603, format!("写入收藏失败: {e}")))?;
        } else {
            crate::db::repo::message_favorites::remove(&pool, DEFAULT_USER, message_id)
                .await
                .map_err(|e| (-32603, format!("取消收藏失败: {e}")))?;
        }
        Ok(serde_json::json!({
            "ok": true,
            "message_id": message_id,
            "is_favorite": is_favorite,
            "session_id": conv_id,
        }))
    }

    async fn list_favorite_messages(&self) -> Result<Value, (i32, String)> {
        let pool = self.db.pool().await;
        let items = crate::db::repo::message_favorites::list(&pool, "default-user")
            .await
            .map_err(|e| (-32603, format!("读取收藏失败: {e}")))?;
        let favorites: Vec<Value> = items
            .into_iter()
            .map(|f| {
                serde_json::json!({
                    "id": f.id,
                    "message_id": f.message_id,
                    "role": f.role,
                    "content": f.content,
                    "created_at": f.created_at,
                    "conversation_id": f.conversation_id,
                    "conversation_title": f.conversation_title,
                    "conversation_deleted": f.conversation_deleted != 0,
                    "model_id": f.model_id,
                })
            })
            .collect();
        Ok(serde_json::json!({ "favorites": favorites, "total": favorites.len() }))
    }
}

#[async_trait]
impl AcpHandler for FastClawAcpHandler {
    async fn handle_request(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, (i32, String)> {
        match method {
            "session/cancel" => {
                let p = params.ok_or((-32602, "Missing params".into()))?;
                let sid = p["session_id"]
                    .as_str()
                    .ok_or((-32602, "Missing session_id".into()))?;

                let fresh = CancellationToken::new();
                {
                    let mut tokens = self.cancel_tokens.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(old) = tokens.insert(sid.to_string(), fresh.clone()) {
                        old.cancel();
                    }
                }
                if let Ok(mut sessions) = self.sessions.lock() {
                    if let Some(state) = sessions.get_mut(sid) {
                        state.cancel_token.cancel();
                        state.cancel_token = fresh;
                    }
                }
                self.emit(
                    sid,
                    AcpEvent::Complete {
                        exit_code: 0,
                        is_new_session: Some(false),
                    },
                );

                Ok(serde_json::json!({ "cancelled": true }))
            }

            "session/confirm-plan" => {
                let p = params.ok_or((-32602, "Missing params".into()))?;
                self.send_session_plan_control(&p, super::fastplan::PlanControl::Confirm)
            }

            "session/resume-plan" => {
                let p = params.ok_or((-32602, "Missing params".into()))?;
                self.send_session_plan_control(&p, super::fastplan::PlanControl::Resume)
            }

            "session/cancel-plan" => {
                let p = params.ok_or((-32602, "Missing params".into()))?;
                self.send_session_plan_control(&p, super::fastplan::PlanControl::Cancel)
            }

            other => Err((-32601, format!("Method not found: {}", other))),
        }
    }

}
