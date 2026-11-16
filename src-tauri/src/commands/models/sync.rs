use tauri::State;

use crate::core::error::AppError;
use crate::core::models::{
    execute_catalog_sync, preview_catalog_sync, synchronize_independent_models,
    IndependentSyncResult, SyncPreview, SyncResult,
};
use crate::state::AppState;

#[tauri::command]
pub async fn sync_execute(state: State<'_, AppState>) -> Result<SyncResult, AppError> {
    let model_download_url = state
        .app_config
        .read()
        .await
        .remote_servers
        .fastci_model_download_url
        .clone();
    let items = super::load_mock_items(&model_download_url).await?;
    execute_catalog_sync(&*state.db.pool().await, &items).await
}

#[tauri::command]
pub async fn sync_preview(state: State<'_, AppState>) -> Result<SyncPreview, AppError> {
    let model_download_url = state
        .app_config
        .read()
        .await
        .remote_servers
        .fastci_model_download_url
        .clone();
    let items = super::load_mock_items(&model_download_url).await?;
    preview_catalog_sync(&*state.db.pool().await, &items).await
}

#[tauri::command]
pub async fn sync_independent_models(
    state: State<'_, AppState>,
    token: String,
    api_url: String,
) -> Result<IndependentSyncResult, AppError> {
    log::info!("[独立模型] 开始同步: {}", api_url);
    let response = reqwest::Client::new()
        .get(&api_url)
        .header("Authorization", format!("Bearer {}", token))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|error| AppError::Network(error.to_string()))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Network(format!("HTTP {} | {}", status, body)));
    }
    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|error| AppError::Parse(error.to_string()))?;
    let items = payload
        .get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| AppError::Parse("响应中缺少 data 字段".to_string()))?;
    synchronize_independent_models(&*state.db.pool().await, items).await
}
