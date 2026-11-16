// FastClaw · 智能体管理 Tauri Commands
use crate::core::error::AppError;
use crate::db::repo::agents;
use crate::db::repo::agents::AgentFilters;
use crate::state::AppState;
use anyhow::Error;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{Manager, State};

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentInput {
    pub agent_name: String,
    pub agent_description: Option<String>,
    pub agent_icon: Option<String>,
    pub agent_prompt: Option<String>,
    pub agent_type: Option<String>,
    pub agent_model: Option<String>,
    pub agent_knowledgebase: Option<String>,
    pub agent_status: Option<String>,
    pub agent_visibility: Option<String>,
    pub agent_visibility_roles: Option<Vec<String>>,
    pub agent_tags: Option<Vec<String>>,
    pub agent_creator: Option<String>,
    pub agent_cover: Option<String>,
    pub agent_function_desc: Option<String>,
    pub agent_input_params: Option<serde_json::Value>,
    pub agent_output_results: Option<serde_json::Value>,
    /// 偏好参数（采样配置）：{temperature, top_p, max_tokens, presence_penalty, frequency_penalty}
    pub agent_preferences: Option<serde_json::Value>,
    pub json_data: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListAgentsParams {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub search: Option<String>,
    pub agent_type: Option<String>,
    pub agent_status: Option<String>,
    pub agent_visibility: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CheckNameParams {
    pub name: String,
    pub exclude_id: Option<String>,
}

fn vec_to_json_string(vec: &Option<Vec<String>>) -> Option<String> {
    vec.as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default())
}

#[tauri::command]
pub async fn list_agents(
    state: State<'_, AppState>,
    params: ListAgentsParams,
) -> Result<serde_json::Value, AppError> {
    let filters = AgentFilters {
        search: params.search,
        agent_type: params.agent_type,
        agent_status: params.agent_status,
        agent_visibility: params.agent_visibility,
    };

    let pool = state.db.pool().await;
    let (items, total) = tokio::try_join!(
        agents::get_agents(&*pool, params.page, params.page_size, Some(&filters)),
        agents::get_agent_count(&*pool, Some(&filters)),
    )?;
    drop(pool);

    Ok(json!({
        "items": items,
        "total": total,
        "page": params.page.unwrap_or(1),
        "page_size": params.page_size.unwrap_or(20),
    }))
}

#[tauri::command]
pub async fn get_agent(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<serde_json::Value>, AppError> {
    let row = agents::get_agent_by_id(&*state.db.pool().await, &id).await?;
    Ok(row)
}

#[tauri::command]
pub async fn create_agent(
    state: State<'_, AppState>,
    id: String,
    input: AgentInput,
) -> Result<(), AppError> {
    let roles_json = vec_to_json_string(&input.agent_visibility_roles);
    let tags_json = vec_to_json_string(&input.agent_tags);
    let input_params_json = input.agent_input_params.as_ref().map(|v| v.to_string());
    let output_results_json = input.agent_output_results.as_ref().map(|v| v.to_string());
    let preferences_json = input.agent_preferences.as_ref().map(|v| v.to_string());

    agents::insert_agent(
        &*state.db.pool().await,
        &id,
        &input.agent_name,
        input.agent_description.as_deref(),
        input.agent_icon.as_deref(),
        input.agent_prompt.as_deref(),
        input.agent_type.as_deref(),
        input.agent_model.as_deref(),
        input.agent_knowledgebase.as_deref(),
        input.agent_status.as_deref(),
        input.agent_visibility.as_deref(),
        roles_json.as_deref(),
        tags_json.as_deref(),
        input.agent_creator.as_deref(),
        input.agent_cover.as_deref(),
        input.agent_function_desc.as_deref(),
        input_params_json.as_deref(),
        output_results_json.as_deref(),
        preferences_json.as_deref(),
        input.json_data.as_deref(),
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn update_agent(
    state: State<'_, AppState>,
    id: String,
    input: AgentInput,
) -> Result<(), AppError> {
    let roles_json = vec_to_json_string(&input.agent_visibility_roles);
    let tags_json = vec_to_json_string(&input.agent_tags);
    let input_params_json = input.agent_input_params.as_ref().map(|v| v.to_string());
    let output_results_json = input.agent_output_results.as_ref().map(|v| v.to_string());
    let preferences_json = input.agent_preferences.as_ref().map(|v| v.to_string());

    agents::update_agent(
        &*state.db.pool().await,
        &id,
        &input.agent_name,
        input.agent_description.as_deref(),
        input.agent_icon.as_deref(),
        input.agent_prompt.as_deref(),
        input.agent_type.as_deref(),
        input.agent_model.as_deref(),
        input.agent_knowledgebase.as_deref(),
        input.agent_status.as_deref(),
        input.agent_visibility.as_deref(),
        roles_json.as_deref(),
        tags_json.as_deref(),
        input.agent_creator.as_deref(),
        input.agent_cover.as_deref(),
        input.agent_function_desc.as_deref(),
        input_params_json.as_deref(),
        output_results_json.as_deref(),
        preferences_json.as_deref(),
        input.json_data.as_deref(),
    )
    .await?;
    Ok(())
}

/// 批量导入智能体
/// - 自动为缺省 id 生成 UUID
/// - 同名（agent_name）已存在时跳过，避免重复
/// - 返回 {imported, skipped, errors}
#[tauri::command]
pub async fn import_agents(
    state: State<'_, AppState>,
    agents_input: Vec<AgentInput>,
) -> Result<serde_json::Value, AppError> {
    let pool = state.db.pool().await.clone();
    let mut imported = 0u32;
    let mut skipped = 0u32;
    let mut errors: Vec<serde_json::Value> = Vec::new();

    for input in &agents_input {
        if input.agent_name.trim().is_empty() {
            errors.push(json!({ "name": "", "error": "agent_name 不能为空" }));
            continue;
        }

        match agents::check_agent_name_exists(&pool, &input.agent_name, None).await {
            Ok(true) => {
                skipped += 1;
                continue;
            }
            Ok(false) => {}
            Err(e) => {
                errors.push(json!({ "name": input.agent_name, "error": e.to_string() }));
                continue;
            }
        }

        let id = uuid::Uuid::new_v4().to_string();
        let roles_json = vec_to_json_string(&input.agent_visibility_roles);
        let tags_json = vec_to_json_string(&input.agent_tags);
        let input_params_json = input.agent_input_params.as_ref().map(|v| v.to_string());
        let output_results_json = input.agent_output_results.as_ref().map(|v| v.to_string());
        let preferences_json = input.agent_preferences.as_ref().map(|v| v.to_string());

        let res = agents::insert_agent(
            &pool,
            &id,
            &input.agent_name,
            input.agent_description.as_deref(),
            input.agent_icon.as_deref(),
            input.agent_prompt.as_deref(),
            input.agent_type.as_deref(),
            input.agent_model.as_deref(),
            input.agent_knowledgebase.as_deref(),
            input.agent_status.as_deref(),
            input.agent_visibility.as_deref(),
            roles_json.as_deref(),
            tags_json.as_deref(),
            input.agent_creator.as_deref(),
            input.agent_cover.as_deref(),
            input.agent_function_desc.as_deref(),
            input_params_json.as_deref(),
            output_results_json.as_deref(),
            preferences_json.as_deref(),
            input.json_data.as_deref(),
        )
        .await;

        match res {
            Ok(_) => imported += 1,
            Err(e) => errors.push(json!({ "name": input.agent_name, "error": e.to_string() })),
        }
    }

    Ok(json!({
        "imported": imported,
        "skipped": skipped,
        "errors": errors,
    }))
}

#[tauri::command]
pub async fn delete_agent(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    agents::delete_agent(&*state.db.pool().await, &id).await?;
    Ok(())
}

#[tauri::command]
pub async fn check_agent_name(
    state: State<'_, AppState>,
    params: CheckNameParams,
) -> Result<bool, AppError> {
    let exists = agents::check_agent_name_exists(
        &*state.db.pool().await,
        &params.name,
        params.exclude_id.as_deref(),
    )
    .await?;
    Ok(exists)
}

#[derive(Debug, Deserialize)]
pub struct SaveIconInput {
    pub icon_data: String,
    pub file_name: String,
}

#[tauri::command]
pub async fn save_agent_icon(
    app: tauri::AppHandle,
    agent_id: String,
    input: SaveIconInput,
) -> Result<String, AppError> {
    let icon_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Internal(Error::msg(e.to_string())))?
        .join("agent_icons");
    std::fs::create_dir_all(&icon_dir)
        .map_err(|e| AppError::Internal(Error::msg(e.to_string())))?;

    let ext = std::path::Path::new(&input.file_name)
        .extension()
        .and_then(|e: &std::ffi::OsStr| e.to_str())
        .unwrap_or("png");
    let icon_path = icon_dir.join(format!("{}.{}", agent_id, ext));

    let data_url = &input.icon_data;
    let base64_data = if data_url.contains(",") {
        data_url.split(',').nth(1).unwrap_or(&data_url)
    } else {
        data_url
    };

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(base64_data)
        .map_err(|e: base64::DecodeError| AppError::Internal(Error::msg(e.to_string())))?;

    std::fs::write(&icon_path, decoded)
        .map_err(|e| AppError::Internal(Error::msg(e.to_string())))?;

    let relative_path = format!("agent_icons/{}.{}", agent_id, ext);
    Ok(relative_path)
}

#[tauri::command]
pub async fn get_agent_icon(app: tauri::AppHandle, icon_path: String) -> Result<String, AppError> {
    let full_path = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Internal(Error::msg(e.to_string())))?
        .join(&icon_path);

    if !full_path.exists() {
        return Err(AppError::NotFound("图标文件不存在".into()));
    }

    let data =
        std::fs::read(&full_path).map_err(|e| AppError::Internal(Error::msg(e.to_string())))?;

    let ext = full_path
        .extension()
        .and_then(|e: &std::ffi::OsStr| e.to_str())
        .unwrap_or("png");
    let mime = match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        _ => "image/png",
    };

    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);
    Ok(format!("data:{};base64,{}", mime, encoded))
}
