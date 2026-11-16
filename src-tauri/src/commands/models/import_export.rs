use tauri::State;

use crate::core::error::AppError;
use crate::core::models::{
    build_model_file_export, import_catalog_models, ModelFileExportDto, SyncResult,
};
use crate::db::repo;
use crate::state::AppState;

#[tauri::command]
pub async fn import_models_json(
    state: State<'_, AppState>,
    models: Vec<serde_json::Value>,
) -> Result<SyncResult, AppError> {
    import_catalog_models(&*state.db.pool().await, &models).await
}

#[tauri::command]
pub async fn export_models(
    state: State<'_, AppState>,
    include_credentials: bool,
) -> Result<Vec<ModelFileExportDto>, AppError> {
    let rows = repo::models::list_models_for_export(&*state.db.pool().await).await?;
    Ok(rows
        .into_iter()
        .map(|row| build_model_file_export(row, include_credentials))
        .collect())
}
