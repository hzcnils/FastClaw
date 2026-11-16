//! 桌面端「任务规划」Tauri 命令
//!
//! 规划开关、执行模式、规划确认（confirm/resume/cancel）与桌面事件广播。

use tauri::State;

use crate::core::session_plan::config;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// 远程管理器
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn set_remote_ui_model(
    state: State<'_, AppState>,
    model_id: Option<String>,
    conversation_id: Option<String>,
) -> Result<(), String> {
    let mid = model_id.filter(|s| !s.trim().is_empty());
    config::set_current_ui_model(mid.clone());
    let Some(mid) = mid else {
        return Ok(());
    };
    let Some(cid) = conversation_id.filter(|s| !s.trim().is_empty()) else {
        return Ok(());
    };
    let pool = state.db.pool().await;
    crate::db::repo::conversations::update_model_id(&pool, &cid, &mid)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn map_acp_err((code, msg): (i32, String)) -> String {
    format!("[{code}] {msg}")
}
/// 桌面「任务规划」开关：写 `task_plan`。
/// 故意不标 `fastplan_remote`，桌面仍走本地 FastPlan UI/WS。
#[tauri::command]
pub async fn set_conversation_task_plan(
    state: State<'_, AppState>,
    session_id: Option<String>,
    enabled: bool,
    model_id: Option<String>,
) -> Result<serde_json::Value, String> {
    use crate::core::session_plan::fastplan;

    let pool = state.db.pool().await;
    let sid = session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    if !enabled {
        let sid = sid.ok_or_else(|| "关闭任务规划需要会话 ID".to_string())?;
        let _ = fastplan::persist_conversation_task_plan(&pool, &sid, false).await?;
        return Ok(serde_json::json!({ "session_id": sid, "taskPlan": false }));
    }

    let target = fastplan::open_or_reuse_planning_conversation(
        &pool,
        sid.as_deref(),
        model_id,
        None,
    )
    .await?;

    if let Some(ref old) = target.detached_from {
    } else if sid.is_none() {
    }


    Ok(serde_json::json!({
        "session_id": target.session_id,
        "taskPlan": true,
        "detachedFrom": target.detached_from,
        "title": target.title,
    }))
}

/// 桌面 / App 共用：规划执行模式写入 extra_config 并广播。
#[tauri::command]
pub async fn set_conversation_execution_mode(
    state: State<'_, AppState>,
    session_id: String,
    mode: String,
) -> Result<serde_json::Value, String> {
    use crate::core::session_plan::fastplan;

    let sid = session_id.trim();
    if sid.is_empty() {
        return Err("Missing session_id".into());
    }
    let parsed = fastplan::PlanExecutionMode::parse(&mode);
    let pool = state.db.pool().await;
    let _ = fastplan::persist_conversation_execution_mode(&pool, sid, parsed).await?;
    Ok(serde_json::json!({
        "session_id": sid,
        "executionMode": parsed.as_str(),
    }))
}

/// 桌面远程规划卡确认 / 继续 / 取消：与 App ACP 同一入口。
#[tauri::command]
pub async fn send_conversation_plan_control(
    state: State<'_, AppState>,
    session_id: String,
    action: String,
    expected_wait_id: Option<String>,
) -> Result<serde_json::Value, String> {
    use crate::core::session_plan::handler::AcpHandler;

    let handler = state
        .acp_handler
        .as_ref()
        .ok_or_else(|| "远程控制未初始化".to_string())?;
    let sid = session_id.trim();
    if sid.is_empty() {
        return Err("Missing session_id".into());
    }
    let method = match action.trim() {
        "confirm" | "confirm_plan" => "session/confirm-plan",
        "resume" | "resume_plan" | "resume_tree" | "continue" => "session/resume-plan",
        "cancel" | "cancel_plan" | "stop" => "session/cancel-plan",
        _ => return Err("未知规划操作".into()),
    };
    handler
        .handle_request(
            method,
            Some(serde_json::json!({ "session_id": sid, "expected_wait_id": expected_wait_id })),
        )
        .await
        .map_err(map_acp_err)
}


/// 桌面本地 FastPlan：落库用户句并广播给 App。
#[tauri::command]
pub async fn sync_desktop_plan_user_message(
    state: State<'_, AppState>,
    conversation_id: String,
    content: String,
    group_id: Option<String>,
) -> Result<serde_json::Value, String> {

    let sid = conversation_id.trim();
    let text = content.trim();
    if sid.is_empty() {
        return Err("Missing conversation_id".into());
    }
    if text.is_empty() {
        return Err("消息内容不能为空".into());
    }
    let pool = state.db.pool().await;
    let msg_id = uuid::Uuid::new_v4().to_string();
    let gid = group_id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    crate::db::repo::messages::insert(
        &pool,
        &crate::db::repo::messages::NewMessage {
            id: msg_id.clone(),
            conversation_id: sid.to_string(),
            role: "user".into(),
            content: text.to_string(),
            model_id: None,
            model_instance_id: None,
            parent_id: None,
            group_id: Some(gid.clone()),
            attachments: None,
            token_input: None,
            token_output: None,
        },
    )
    .await
    .map_err(|e| e.to_string())?;
    let _ = crate::core::session_plan::fastplan::persist_conversation_task_plan(&pool, sid, true).await;

    // 首条规划提问：自动命名并广播给 App（默认当前对话模型）
    let prior_user: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE conversation_id = ? AND role = 'user'",
    )
    .bind(sid)
    .fetch_one(&*pool)
    .await
    .unwrap_or(0);
    if prior_user <= 1 {
        if let Ok(Some(title)) = crate::core::topic_title::maybe_name_conversation_topic(
            &pool,
            state.llm.as_ref(),
            sid,
            text,
            None,
            true,
        )
        .await
        {
        }
    }

    Ok(serde_json::json!({
        "message_id": msg_id,
        "group_id": gid,
        "session_id": sid,
    }))
}

/// 桌面本地 FastPlan 终态：落库助手回答 + planRuntime，并广播给 App。
#[tauri::command]
pub async fn sync_desktop_plan_assistant_message(
    state: State<'_, AppState>,
    conversation_id: String,
    content: String,
    group_id: Option<String>,
    model_id: Option<String>,
    plan_runtime: Option<serde_json::Value>,
    fastplan_session_id: Option<String>,
    entry_id: Option<String>,
    completion_id: Option<String>,
    terminal_status: Option<String>,
) -> Result<serde_json::Value, String> {
    use sha2::{Digest, Sha256};

    let sid = conversation_id.trim();
    if sid.is_empty() {
        return Err("Missing conversation_id".into());
    }
    let pool = state.db.pool().await;
    let fastplan_session_id = fastplan_session_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let entry_id = entry_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let completion_id = completion_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let terminal_status = match terminal_status.as_deref() {
        Some("failed") => "failed",
        _ => "complete",
    };
    let msg_id = match (&fastplan_session_id, &completion_id) {
        (Some(session), Some(completion)) => {
            let digest = Sha256::digest(format!("{sid}\0{session}\0{completion}"));
            format!("fastplan-{:x}", digest)
        }
        _ => uuid::Uuid::new_v4().to_string(),
    };
    let gid = group_id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let text = content;
    let extra_data = serde_json::json!({
        "planRuntime": plan_runtime.clone(),
        "fastplanCompletionId": completion_id.clone(),
        "fastplanSessionId": fastplan_session_id.clone(),
    })
    .to_string();
    let mut transaction = pool.begin().await.map_err(|e| e.to_string())?;
    let existing_extra = sqlx::query_scalar::<_, Option<String>>(
        "SELECT extra_config FROM conversations WHERE id = ?",
    )
    .bind(sid)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|e| e.to_string())?
    .flatten();
    let merged_extra = match (&fastplan_session_id, &entry_id) {
        (Some(session), Some(entry)) => Some(
            crate::core::session_plan::fastplan::merge_fastplan_entry_status(
                existing_extra.as_deref(),
                session,
                entry,
                terminal_status,
            ),
        ),
        _ => None,
    };
    if let Some(ref extra_config) = merged_extra {
        sqlx::query(
            "UPDATE conversations SET extra_config = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
        )
        .bind(extra_config)
        .bind(sid)
        .execute(&mut *transaction)
        .await
        .map_err(|e| e.to_string())?;
    }
    let already_inserted = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM messages WHERE id = ?",
    )
    .bind(&msg_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|e| e.to_string())?
        > 0;
    if already_inserted {
        transaction.commit().await.map_err(|e| e.to_string())?;
        return Ok(serde_json::json!({
            "message_id": msg_id,
            "group_id": gid,
            "session_id": sid,
            "inserted": false,
            "extra_config": merged_extra,
        }));
    }
    sqlx::query(
        "INSERT INTO messages (id, conversation_id, role, content, model_id, parent_id, group_id, status, extra_data, created_at) \
         VALUES (?, ?, 'assistant', ?, ?, NULL, ?, 'completed', ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
    )
    .bind(&msg_id)
    .bind(sid)
    .bind(&text)
    .bind(model_id.as_deref())
    .bind(&gid)
    .bind(&extra_data)
    .execute(&mut *transaction)
    .await
    .map_err(|e| e.to_string())?;
    transaction.commit().await.map_err(|e| e.to_string())?;

    if let Some(ref snap) = plan_runtime {
    }
    if !text.trim().is_empty() {
    }
    Ok(serde_json::json!({
        "message_id": msg_id,
        "group_id": gid,
        "session_id": sid,
        "inserted": true,
        "extra_config": merged_extra,
    }))
}

