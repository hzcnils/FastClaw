use tauri::State;

use crate::core::error::AppError;
use crate::core::models::{
    fetch_openai_models_payload, infer_model_info, parse_remote_models, ModelDto, ModelExportDto,
    ModelUpdatePatch, RemoteModelDto,
};
use crate::db::repo;
use crate::state::AppState;

#[tauri::command]
pub async fn list_models(
    state: State<'_, AppState>,
    provider_code: Option<String>,
) -> Result<Vec<ModelDto>, AppError> {
    let rows = repo::models::list_models(&*state.db.pool().await, provider_code.as_deref()).await?;
    Ok(rows)
}

#[tauri::command]
pub async fn fetch_remote_models(
    state: State<'_, AppState>,
    provider_code: String,
) -> Result<Vec<RemoteModelDto>, AppError> {
    let pool = state.db.pool().await;
    let provider = repo::models::get_provider(&pool, &provider_code)
        .await?
        .ok_or_else(|| AppError::NotFound("供应商不存在".to_string()))?;
    let payload =
        fetch_openai_models_payload(&provider.base_url, provider.api_key_ref.as_deref()).await?;
    let existing_ids = repo::models::existing_model_ids(&pool, &provider_code)
        .await?
        .into_iter()
        .collect();
    parse_remote_models(&provider_code, &payload, &existing_ids)
}

#[tauri::command]
pub async fn save_remote_models(
    state: State<'_, AppState>,
    provider_code: String,
    models: Vec<RemoteModelDto>,
) -> Result<(), AppError> {
    let pool = state.db.pool().await;
    for model in models {
        let model_name = model.model_name;
        let (model_type, model_label) = infer_model_info(&model_name, Some(&model.model_type));
        let extra_config = model.extra_config.map(|value| match value {
            serde_json::Value::String(value) => value,
            value => value.to_string(),
        });
        repo::models::upsert_model(
            &pool,
            &repo::models::NewModel {
                id: model.id,
                provider_code: provider_code.clone(),
                model_id: model_name.clone(),
                display_name: model.display_name.unwrap_or(model_name),
                model_type,
                model_label,
                context_window: model
                    .context_window
                    .and_then(|value| i32::try_from(value).ok()),
                max_output: model.max_output.and_then(|value| i32::try_from(value).ok()),
                input_price: model.input_price,
                output_price: model.output_price,
                is_enabled: true,
                is_local_model: false,
                temperature: None,
                top_p: None,
                extra_config,
                mxcs: model.mxcs,
            },
        )
        .await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn update_models(
    state: State<'_, AppState>,
    ids: Vec<String>,
    updates: ModelUpdatePatch,
) -> Result<(), AppError> {
    repo::models::update_models(&*state.db.pool().await, &ids, &updates).await?;
    Ok(())
}

#[tauri::command]
pub async fn delete_models(state: State<'_, AppState>, ids: Vec<String>) -> Result<(), AppError> {
    repo::models::delete_models(&*state.db.pool().await, &ids).await?;
    Ok(())
}

// ─── 导出模型（全量，含供应商信息） ───

#[tauri::command]
pub async fn list_models_for_export(
    state: State<'_, AppState>,
) -> Result<Vec<ModelExportDto>, AppError> {
    let rows = repo::models::list_models_for_export(&*state.db.pool().await).await?;
    Ok(rows)
}
