// FastClaw · AI 助手 & 角色管理 Tauri Commands
use crate::core::error::AppError;
use crate::db::repo;
use crate::state::AppState;
use tauri::{Emitter, State};

// ─── 旧版 assistants CRUD（保留兼容） ───

#[tauri::command]
pub async fn list_assistants(
    state: State<'_, AppState>,
    category: Option<String>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let rows = repo::assistants::list(&*state.db.pool().await, category.as_deref()).await?;
    Ok(rows)
}

#[tauri::command]
pub async fn create_assistant(
    state: State<'_, AppState>,
    input: serde_json::Value,
) -> Result<String, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    repo::assistants::insert(&*state.db.pool().await, &id, input).await?;
    Ok(id)
}

#[tauri::command]
pub async fn update_assistant(
    state: State<'_, AppState>,
    id: String,
    updates: serde_json::Value,
) -> Result<(), AppError> {
    repo::assistants::update(&*state.db.pool().await, &id, updates).await?;
    Ok(())
}

#[tauri::command]
pub async fn delete_assistant(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    repo::assistants::delete(&*state.db.pool().await, &id).await?;
    Ok(())
}

// ─── 角色管理命令 ───

#[tauri::command]
pub async fn get_roles(
    state: State<'_, AppState>,
    search: Option<String>,
    source_type: Option<String>,
    is_active: Option<bool>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let rows = repo::assistants::list_roles(
        &*state.db.pool().await,
        search.as_deref(),
        source_type.as_deref(),
        is_active,
    )
    .await?;
    Ok(rows)
}

#[tauri::command]
pub async fn get_role(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<serde_json::Value>, AppError> {
    let role = repo::assistants::get_role(&*state.db.pool().await, &id).await?;
    Ok(role)
}

#[tauri::command]
pub async fn create_role(
    state: State<'_, AppState>,
    data: serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    repo::assistants::insert_role(&*state.db.pool().await, &id, &data).await?;
    let role = repo::assistants::get_role(&*state.db.pool().await, &id)
        .await?
        .ok_or_else(|| AppError::NotFound("角色".to_string()))?;
    Ok(role)
}

#[tauri::command]
pub async fn update_role(
    state: State<'_, AppState>,
    id: String,
    data: serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    repo::assistants::update_role(&*state.db.pool().await, &id, &data).await?;
    let role = repo::assistants::get_role(&*state.db.pool().await, &id)
        .await?
        .ok_or_else(|| AppError::NotFound("角色".to_string()))?;
    Ok(role)
}

#[tauri::command]
pub async fn delete_role(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    repo::assistants::delete_role(&*state.db.pool().await, &id).await?;
    Ok(())
}

#[tauri::command]
pub async fn activate_role(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), AppError> {
    repo::assistants::activate_role(&*state.db.pool().await, &id).await?;
    // 通知前端角色已激活
    let _ = app.emit("role-activated", &id);
    Ok(())
}

#[tauri::command]
pub async fn deactivate_role(state: State<'_, AppState>) -> Result<(), AppError> {
    repo::assistants::deactivate_role(&*state.db.pool().await).await?;
    Ok(())
}

#[tauri::command]
pub async fn get_active_role(
    state: State<'_, AppState>,
) -> Result<Option<serde_json::Value>, AppError> {
    let role = repo::assistants::get_active_role(&*state.db.pool().await).await?;
    Ok(role)
}

#[tauri::command]
pub async fn get_role_stats(state: State<'_, AppState>) -> Result<serde_json::Value, AppError> {
    let stats = repo::assistants::get_role_stats(&*state.db.pool().await).await?;
    Ok(stats)
}

#[tauri::command]
pub async fn publish_role(
    state: State<'_, AppState>,
    id: String,
    category: String,
    square_desc: String,
    author_name: Option<String>,
) -> Result<(), AppError> {
    repo::assistants::publish_role(
        &*state.db.pool().await,
        &id,
        &category,
        &square_desc,
        author_name.as_deref().unwrap_or("用户"),
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn unpublish_role(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    repo::assistants::unpublish_role(&*state.db.pool().await, &id).await?;
    Ok(())
}

#[tauri::command]
pub async fn clone_role(
    state: State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, AppError> {
    let new_id = uuid::Uuid::new_v4().to_string();
    let role = repo::assistants::clone_role(&*state.db.pool().await, &id, &new_id).await?;
    Ok(role)
}

#[tauri::command]
pub async fn list_square_roles(
    state: State<'_, AppState>,
    search: Option<String>,
    category: Option<String>,
    sort_by: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<serde_json::Value, AppError> {
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(9).max(1).min(50);
    let (items, total) = repo::assistants::list_square_roles(
        &*state.db.pool().await,
        search.as_deref(),
        category.as_deref(),
        sort_by.as_deref(),
        page,
        page_size,
    )
    .await?;
    Ok(serde_json::json!({
        "items": items,
        "total": total,
        "page": page,
        "pageSize": page_size,
    }))
}

#[tauri::command]
pub async fn apply_square_role(
    state: State<'_, AppState>,
    square_id: String,
) -> Result<serde_json::Value, AppError> {
    let new_id = uuid::Uuid::new_v4().to_string();
    let role =
        repo::assistants::apply_square_role(&*state.db.pool().await, &square_id, &new_id).await?;
    Ok(role)
}

#[tauri::command]
pub async fn star_square_role(
    state: State<'_, AppState>,
    square_id: String,
) -> Result<(), AppError> {
    repo::assistants::star_square_role(&*state.db.pool().await, &square_id).await?;
    Ok(())
}
