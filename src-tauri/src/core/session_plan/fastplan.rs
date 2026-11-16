//! 会话任务规划（task_plan）桥接 —— 与桌面 ChatView 共用同一套 FastPlan 引擎。
//!
//! 负责：`extra_config` 里 task_plan / execution_mode 的读写、规划会话的创建与复用、
//! 以及规划状态落库。

use serde::Serialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

pub use super::plan_control::{send_plan_control, PlanControl, PlanWaitSlot};




#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanExecutionMode {
    Auto,
    Manual,
    Step,
}

impl PlanExecutionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manual => "manual",
            Self::Step => "step",
        }
    }

    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "manual" => Self::Manual,
            "step" => Self::Step,
            _ => Self::Auto,
        }
    }
}

















fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}




/// Update one desktop-owned FastPlan entry without replacing the rest of the
/// conversation metadata. Used by the application-level session manager after
/// its ChatView has already been evicted.
pub(crate) fn merge_fastplan_entry_status(
    existing: Option<&str>,
    session_id: &str,
    entry_id: &str,
    status: &str,
) -> String {
    let mut root = extra_config_object(existing);
    let Some(obj) = root.as_object_mut() else {
        return root.to_string();
    };
    obj.insert("task_plan".into(), json!(true));
    obj.insert("fastplan_session_id".into(), json!(session_id));

    let updated_at = now_rfc3339();
    let mut fastplan = obj
        .get("fastplan")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let Some(fp_obj) = fastplan.as_object_mut() else {
        return root.to_string();
    };
    let revision = fp_obj
        .get("revision")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        + 1;
    fp_obj.insert("version".into(), json!(2));
    fp_obj.insert("revision".into(), json!(revision));
    fp_obj.insert("writerId".into(), json!("session-manager"));
    fp_obj.insert("updatedAt".into(), json!(updated_at.clone()));
    if fp_obj.get("activeEntryId").and_then(Value::as_str).is_none() {
        fp_obj.insert("activeEntryId".into(), json!(entry_id));
    }

    let mut entries = fp_obj
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(entry) = entries.iter_mut().find(|item| {
        item.get("sessionId").and_then(Value::as_str) == Some(session_id)
            || item.get("id").and_then(Value::as_str) == Some(entry_id)
    }) {
        entry["id"] = json!(entry_id);
        entry["sessionId"] = json!(session_id);
        entry["status"] = json!(status);
    } else {
        entries.push(json!({
            "id": entry_id,
            "sessionId": session_id,
            "status": status,
            "createdAt": updated_at,
        }));
    }
    fp_obj.insert("entries".into(), json!(entries));
    obj.insert("fastplan".into(), fastplan);
    root.to_string()
}

fn extra_config_object(existing: Option<&str>) -> Value {
    existing
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

fn extra_config_has_task_plan_key(extra_config: Option<&str>) -> bool {
    let root = extra_config_object(extra_config);
    root.get("task_plan").or_else(|| root.get("taskPlan")).is_some()
}

pub(crate) fn extra_config_task_plan_enabled(extra_config: Option<&str>) -> bool {
    let root = extra_config_object(extra_config);
    let raw = root.get("task_plan").or_else(|| root.get("taskPlan"));
    match raw {
        Some(Value::Bool(true)) => true,
        Some(Value::Number(n)) => n.as_i64() == Some(1) || n.as_u64() == Some(1),
        Some(Value::String(s)) => {
            let s = s.trim().to_ascii_lowercase();
            s == "true" || s == "1"
        }
        _ => false,
    }
}

pub fn extra_config_execution_mode(extra_config: Option<&str>) -> PlanExecutionMode {
    let root = extra_config_object(extra_config);
    let raw = root
        .get("execution_mode")
        .or_else(|| root.get("executionMode"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    PlanExecutionMode::parse(raw)
}

pub fn merge_execution_mode(existing: Option<&str>, mode: PlanExecutionMode) -> String {
    let mut root = extra_config_object(existing);
    if let Some(obj) = root.as_object_mut() {
        obj.insert("execution_mode".into(), json!(mode.as_str()));
    }
    root.to_string()
}

pub async fn persist_conversation_execution_mode(
    pool: &SqlitePool,
    conversation_id: &str,
    mode: PlanExecutionMode,
) -> Result<bool, String> {
    let conv = crate::db::repo::conversations::get_by_id(pool, conversation_id)
        .await
        .map_err(|e| format!("读取对话失败: {e}"))?;
    let extra = conv.as_ref().and_then(|c| c.extra_config.clone());
    if extra_config_execution_mode(extra.as_deref()) == mode {
        return Ok(false);
    }
    crate::db::repo::conversations::update_extra_config(
        pool,
        conversation_id,
        &merge_execution_mode(extra.as_deref(), mode),
    )
    .await
    .map_err(|e| format!("保存规划执行模式失败: {e}"))?;
    Ok(true)
}




/// 已是规划会话：mode=planning，或 extra_config 已打开任务规划。
pub(crate) fn is_planning_conversation(mode: &str, extra_config: Option<&str>) -> bool {
    mode.trim().eq_ignore_ascii_case("planning") || extra_config_task_plan_enabled(extra_config)
}

/// 普通对话里已有消息时，点开规划必须另开一条会话，不能写回原对话。
pub(crate) fn should_open_separate_planning_conversation(
    already_planning: bool,
    message_count: i64,
) -> bool {
    !already_planning && message_count > 0
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanningSessionTarget {
    pub session_id: String,
    pub detached_from: Option<String>,
    pub title: String,
}

async fn insert_planning_conversation(
    pool: &SqlitePool,
    model_id: Option<String>,
    assistant_id: Option<String>,
    detached_from: Option<String>,
) -> Result<PlanningSessionTarget, String> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let title = "规划会话".to_string();
    crate::db::repo::conversations::insert(
        pool,
        &crate::db::repo::conversations::NewConversation {
            id: session_id.clone(),
            user_id: "default-user".to_string(),
            title: title.clone(),
            model_id,
            assistant_id,
            mode: "planning".to_string(),
        },
    )
    .await
    .map_err(|e| format!("创建规划会话失败: {e}"))?;
    persist_conversation_task_plan(pool, &session_id, true).await?;
    Ok(PlanningSessionTarget {
        session_id,
        detached_from,
        title,
    })
}

fn nonempty_model_id(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|s| !s.is_empty())
}

async fn lookup_enabled_model_pk(pool: &SqlitePool, raw: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT id FROM models WHERE is_enabled = 1 AND (id = ? OR model_id = ?) \
         ORDER BY (id = ?) DESC LIMIT 1",
    )
    .bind(raw)
    .bind(raw)
    .bind(raw)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

async fn lookup_default_model_pk(pool: &SqlitePool) -> Option<String> {
    if let Ok(Some(raw)) =
        sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'default_model'")
            .fetch_optional(pool)
            .await
    {
        let model = raw.trim().trim_matches('"');
        if model != "null" && !model.is_empty() {
            if let Some(id) = lookup_enabled_model_pk(pool, model).await {
                return Some(id);
            }
        }
    }
    if let Ok(Some(id)) = sqlx::query_scalar::<_, String>(
        "SELECT id FROM models WHERE is_default = 1 AND is_enabled = 1 \
         ORDER BY sort_order ASC, id ASC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    {
        return Some(id);
    }
    sqlx::query_scalar::<_, String>(
        "SELECT id FROM models WHERE is_enabled = 1 ORDER BY sort_order ASC, id ASC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

/// 规划必须用 `models.id`。依次尝试显式值、桌面当前模型、会话模型、系统默认模型。
pub(crate) async fn resolve_planning_model_pk(
    pool: &SqlitePool,
    candidates: &[Option<&str>],
) -> Option<String> {
    for raw in candidates.iter().copied().filter_map(nonempty_model_id) {
        if let Some(id) = lookup_enabled_model_pk(pool, raw).await {
            return Some(id);
        }
    }
    lookup_default_model_pk(pool).await
}

/// 桌面规划按钮和龙虾 `set_task_plan` 共用：空会话/已是规划则原地打开，普通会话有消息则另开「规划会话」。
pub(crate) async fn open_or_reuse_planning_conversation(
    pool: &SqlitePool,
    from_session_id: Option<&str>,
    fallback_model_id: Option<String>,
    fallback_assistant_id: Option<String>,
) -> Result<PlanningSessionTarget, String> {
    let ui_model = super::config::current_ui_model();
    let from = from_session_id
        .map(str::trim)
        .filter(|sid| !sid.is_empty());
    let Some(sid) = from else {
        let model_id = resolve_planning_model_pk(
            pool,
            &[fallback_model_id.as_deref(), ui_model.as_deref()],
        )
        .await
        .or(fallback_model_id);
        return insert_planning_conversation(pool, model_id, fallback_assistant_id, None).await;
    };

    let conv = crate::db::repo::conversations::get_by_id(pool, sid)
        .await
        .map_err(|e| format!("读取对话失败: {e}"))?
        .ok_or_else(|| "Session not found".to_string())?;
    let msg_count = crate::db::repo::conversations::message_count(pool, sid)
        .await
        .unwrap_or(0);
    let already = is_planning_conversation(&conv.mode, conv.extra_config.as_deref());
    let model_id = resolve_planning_model_pk(
        pool,
        &[
            fallback_model_id.as_deref(),
            ui_model.as_deref(),
            conv.model_id.as_deref(),
        ],
    )
    .await
    .or(conv.model_id.clone())
    .or(fallback_model_id);
    if !should_open_separate_planning_conversation(already, msg_count) {
        persist_conversation_task_plan(pool, sid, true).await?;
        let _ = crate::db::repo::conversations::update_mode(pool, sid, "planning").await;
        if let Some(ref mid) = model_id {
            if conv.model_id.as_deref() != Some(mid.as_str()) {
                let _ = crate::db::repo::conversations::update_model_id(pool, sid, mid).await;
            }
        }
        return Ok(PlanningSessionTarget {
            session_id: sid.to_string(),
            detached_from: None,
            title: conv.title,
        });
    }

    let source_mode = extra_config_execution_mode(conv.extra_config.as_deref());
    let target = insert_planning_conversation(
        pool,
        model_id,
        conv.assistant_id.or(fallback_assistant_id),
        Some(sid.to_string()),
    )
    .await?;
    if source_mode != PlanExecutionMode::Auto {
        let _ = persist_conversation_execution_mode(pool, &target.session_id, source_mode).await;
    }
    Ok(target)
}

pub(crate) fn merge_task_plan_enabled(existing: Option<&str>, enabled: bool) -> String {
    let mut root = extra_config_object(existing);
    if let Some(obj) = root.as_object_mut() {
        obj.insert("task_plan".into(), json!(enabled));
    }
    root.to_string()
}

pub(crate) async fn persist_conversation_task_plan(
    pool: &SqlitePool,
    conversation_id: &str,
    enabled: bool,
) -> Result<bool, String> {
    let conv = crate::db::repo::conversations::get_by_id(pool, conversation_id)
        .await
        .map_err(|e| format!("读取对话失败: {e}"))?;
    let extra = conv.as_ref().and_then(|c| c.extra_config.clone());
    let was = extra_config_task_plan_enabled(extra.as_deref());
    let has_key = extra_config_has_task_plan_key(extra.as_deref());
    // 关闭时即使 extra 里还没有 task_plan（旧会话只写了 mode=planning），也要落显式 false，否则 App 列表会继续亮着
    if enabled {
        if was {
            return Ok(false);
        }
    } else if has_key && !was {
        return Ok(false);
    }
    let merged = merge_task_plan_enabled(extra.as_deref(), enabled);
    crate::db::repo::conversations::update_extra_config(pool, conversation_id, &merged)
        .await
        .map_err(|e| format!("保存规划开关失败: {e}"))?;
    Ok(true)
}


























#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_status_merge_preserves_other_metadata() {
        let existing = r#"{
            "mcp":true,
            "fastplan_session_id":"session-1",
            "fastplan":{"version":2,"revision":4,"activeEntryId":"entry-1","entries":[
                {"id":"entry-1","sessionId":"session-1","status":"active","prompt":"plan"},
                {"id":"entry-0","sessionId":"session-0","status":"complete"}
            ]}
        }"#;
        let merged = merge_fastplan_entry_status(
            Some(existing),
            "session-1",
            "entry-1",
            "failed",
        );
        let value: Value = serde_json::from_str(&merged).unwrap();
        assert_eq!(value["mcp"], json!(true));
        assert_eq!(value["fastplan"]["revision"], json!(5));
        assert_eq!(value["fastplan"]["writerId"], json!("session-manager"));
        assert_eq!(value["fastplan"]["entries"][0]["status"], json!("failed"));
        assert_eq!(value["fastplan"]["entries"][0]["prompt"], json!("plan"));
        assert_eq!(value["fastplan"]["entries"][1]["sessionId"], json!("session-0"));
    }

    #[test]
    fn snapshot_settles_running_stages_when_turn_ends() {
        let mut snap = PlanRuntimeSnapshot::default();
        apply_plan_event_to_snapshot(&mut snap, &json!({ "type": "planning_start" }));
        apply_plan_event_to_snapshot(
            &mut snap,
            &json!({
                "type": "route_stage_update",
                "stage": "intent",
                "status": "done",
                "summary": "数据查询: 判定为多步骤"
            }),
        );
        apply_plan_event_to_snapshot(&mut snap, &json!({ "type": "tree_started", "tree_id": "t1" }));
        assert!(snap.is_planning);
        settle_plan_snapshot_after_turn(&mut snap);
        assert!(!snap.is_planning);
    }

    #[test]
    fn extracts_final_answer_and_auto_confirm() {
        let event = json!({ "type": "assistant_message", "content": "  周报已写  " });
        assert_eq!(extract_final_answer(&event).as_deref(), Some("周报已写"));
        let nested = json!({
            "type": "final_answer",
            "final_output": { "content": "陕西以高原山地为主，新疆盆地与山脉相间。" }
        });
        assert_eq!(
            extract_final_answer(&nested).as_deref(),
            Some("陕西以高原山地为主，新疆盆地与山脉相间。")
        );
        let checkpoint = json!({
            "type": "waiting_for_human",
            "tree_id": "t1",
            "node_id": "n1",
            "prompt": "Continue?"
        });
        assert_eq!(
            should_auto_continue_human(&checkpoint)
                .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(str::to_string))
                .as_deref(),
            Some("human_response")
        );
        assert!(should_auto_continue_human(&json!({
            "type": "waiting_for_human",
            "interaction": { "kind": "input" }
        }))
        .is_none());
        let plan = json!({ "type": "plan_generated", "tree": { "id": "t1" } });
        assert_eq!(
            should_auto_confirm_plan(&plan, false),
            Some(json!({ "id": "t1" }))
        );
        assert!(should_auto_confirm_plan(&plan, true).is_none());
    }

    #[test]
    fn protocol_frames_are_not_plan_events() {
        assert!(is_ws_protocol_event("authenticated"));
        assert!(is_ws_protocol_event("pong"));
        assert!(!is_ws_protocol_event("plan_generated"));
        let wrapped = normalize_plan_event(json!({
            "type": "event",
            "event": { "type": "plan_generated", "tree": { "id": "t1" } }
        }));
        assert_eq!(event_type(&wrapped), "plan_generated");
        assert_eq!(
            incomplete_plan_error(false, false),
            "规划通道已断开，请确认电脑端任务规划服务已启动"
        );
        let mut snap = PlanRuntimeSnapshot::default();
        let mut confirm = false;
        let mut final_content = String::new();
        let mut err = None;
        let mut saw_auth = false;
        let mut saw_plan = false;
        let mut chat_sent = false;
        let mut emitted = Vec::new();
        let commands = apply_ws_text(
            r#"{"type":"authenticated"}"#,
            &mut |e| emitted.push(e),
            &mut snap,
            &mut confirm,
            &mut final_content,
            &mut err,
            &mut saw_auth,
            &mut saw_plan,
            &mut chat_sent,
        );
        assert!(saw_auth);
        assert!(!saw_plan);
        assert!(commands.confirm_tree.is_none());
        assert!(commands.human_continue.is_none());
        assert!(emitted.is_empty());
    }

    #[test]
    fn snapshot_follows_thinking_and_nodes() {
        let mut snap = PlanRuntimeSnapshot::default();
        apply_plan_event_to_snapshot(&mut snap, &json!({ "type": "planning_start" }));
        apply_plan_event_to_snapshot(
            &mut snap,
            &json!({
                "type": "thinking_update",
                "data": { "kind": "plan_stage", "stage": "intent", "status": "done", "content": "查天气" }
            }),
        );
        apply_plan_event_to_snapshot(
            &mut snap,
            &json!({
                "type": "plan_generated",
                "tree": { "id": "t1", "root": { "children": [] } }
            }),
        );
        apply_plan_event_to_snapshot(&mut snap, &json!({ "type": "tree_started", "tree_id": "t1" }));
        apply_plan_event_to_snapshot(&mut snap, &json!({ "type": "node_started", "node_id": "n1" }));
        apply_plan_event_to_snapshot(
            &mut snap,
            &json!({ "type": "final_answer", "content": "今天晴" }),
        );
        apply_plan_event_to_snapshot(&mut snap, &json!({ "type": "tree_completed", "success": true }));
        assert_eq!(snap.current_tree_id, "t1");
        assert!(snap.tree_completed);
        assert_eq!(snap.execution_state["n1"]["status"], "running");
        assert_eq!(snap.final_result.unwrap()["content"], "今天晴");
        assert_eq!(snap.validation_flow[0]["stage"], "intent");
    }

    #[test]
    fn chinese_error_copy() {
        assert_eq!(
            user_facing_plan_error("HTTP 403", Some(403)),
            "规划服务拒绝访问"
        );
        assert_eq!(
            user_facing_plan_error("connection refused", None),
            "电脑端未启动任务规划服务"
        );
    }

    #[test]
    fn task_plan_flag_roundtrip_keeps_fastplan() {
        let merged = merge_task_plan_enabled(Some(r#"{"mcp":true,"fastplan_session_id":"s1"}"#), true);
        let value: Value = serde_json::from_str(&merged).unwrap();
        assert_eq!(value["mcp"], true);
        assert_eq!(value["fastplan_session_id"], "s1");
        assert_eq!(value["task_plan"], true);
        assert!(extra_config_task_plan_enabled(Some(&merged)));
        let off = merge_task_plan_enabled(Some(&merged), false);
        assert!(!extra_config_task_plan_enabled(Some(&off)));
        assert!(!extra_config_task_plan_enabled(Some(r#"{"mcp":true}"#)));
    }

    #[test]
    fn ordinary_chat_with_messages_opens_a_separate_planning_conversation() {
        assert!(!is_planning_conversation("normal", None));
        assert!(is_planning_conversation("planning", None));
        assert!(is_planning_conversation(
            "normal",
            Some(r#"{"task_plan":true}"#)
        ));
        assert!(should_open_separate_planning_conversation(false, 2));
        assert!(!should_open_separate_planning_conversation(false, 0));
        assert!(!should_open_separate_planning_conversation(true, 8));
    }

    #[test]
    fn planning_session_target_uses_camel_case_for_phone_sync() {
        let target = PlanningSessionTarget {
            session_id: "plan-1".into(),
            detached_from: Some("chat-1".into()),
            title: "规划会话".into(),
        };
        let json = serde_json::to_value(&target).unwrap();
        assert_eq!(json["sessionId"], "plan-1");
        assert_eq!(json["detachedFrom"], "chat-1");
        assert_eq!(json["title"], "规划会话");
    }

    async fn model_pool() -> sqlx::SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE models (id TEXT, model_id TEXT, is_enabled INTEGER, is_default INTEGER, sort_order INTEGER)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO models VALUES ('pk-flash','deepseek-v4-flash',1,1,1),('pk-plus','deepseek-v4-plus',1,0,2)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn planning_model_prefers_explicit_then_falls_back_to_default() {
        let pool = model_pool().await;
        assert_eq!(
            resolve_planning_model_pk(&pool, &[Some("pk-plus")]).await.as_deref(),
            Some("pk-plus")
        );
        assert_eq!(
            resolve_planning_model_pk(&pool, &[Some("deepseek-v4-plus")]).await.as_deref(),
            Some("pk-plus")
        );
        assert_eq!(
            resolve_planning_model_pk(&pool, &[None, Some(""), Some("missing")]).await.as_deref(),
            Some("pk-flash")
        );
    }
}
