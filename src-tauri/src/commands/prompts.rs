// FastClaw · 提示词模板 Tauri Commands

use crate::core::error::AppError;
use crate::db::repo;
use crate::state::AppState;
use tauri::State;

const DEFAULT_USER: &str = "default-user";

#[tauri::command]
pub async fn list_prompts(
    state: State<'_, AppState>,
    category: Option<String>,
    search: Option<String>,
    source: Option<String>,
) -> Result<Vec<repo::prompts::PromptRow>, AppError> {
    let rows = repo::prompts::list(
        &*state.db.pool().await,
        category.as_deref(),
        search.as_deref(),
        source.as_deref(),
    )
    .await
    .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    Ok(rows)
}

#[tauri::command]
pub async fn list_my_prompts(
    state: State<'_, AppState>,
    user_id: Option<String>,
) -> Result<Vec<repo::prompts::PromptRow>, AppError> {
    let uid = user_id.unwrap_or_else(|| DEFAULT_USER.to_string());
    let rows = repo::prompts::list_my(&*state.db.pool().await, &uid)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    Ok(rows)
}

#[tauri::command]
pub async fn create_prompt(
    state: State<'_, AppState>,
    title: String,
    content: String,
    description: Option<String>,
    category: Option<String>,
) -> Result<String, AppError> {
    if title.trim().is_empty() || content.trim().is_empty() {
        return Err(AppError::InvalidInput("标题和正文不能为空".into()));
    }
    let id = uuid::Uuid::new_v4().to_string();
    repo::prompts::insert(
        &*state.db.pool().await,
        &repo::prompts::NewPrompt {
            id: id.clone(),
            title,
            content,
            description,
            category,
            source: "user".into(),
            author: Some(DEFAULT_USER.into()),
        },
    )
    .await
    .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    Ok(id)
}

#[tauri::command]
pub async fn update_prompt(
    state: State<'_, AppState>,
    id: String,
    title: Option<String>,
    content: Option<String>,
    description: Option<String>,
    category: Option<String>,
) -> Result<(), AppError> {
    repo::prompts::update(
        &*state.db.pool().await,
        &id,
        title.as_deref(),
        content.as_deref(),
        description.as_deref(),
        category.as_deref(),
    )
    .await
    .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn delete_prompt(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let affected = repo::prompts::delete(&*state.db.pool().await, &id)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    if affected == 0 {
        return Err(AppError::InvalidInput(
            "无法删除：提示词不存在或属于系统内置".into(),
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn download_prompt(
    state: State<'_, AppState>,
    prompt_id: String,
) -> Result<(), AppError> {
    repo::prompts::record_download(&*state.db.pool().await, &prompt_id, DEFAULT_USER)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    Ok(())
}

/// 取出提示词正文，前端决定塞入输入框还是作为 system_prompt
#[tauri::command]
pub async fn apply_prompt(
    state: State<'_, AppState>,
    prompt_id: String,
) -> Result<String, AppError> {
    let row = repo::prompts::get(&*state.db.pool().await, &prompt_id)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("提示词不存在: {}", prompt_id)))?;
    Ok(row.content)
}
