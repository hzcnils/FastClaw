use tauri::State;

use crate::core::error::AppError;
use crate::core::models::{GatewayDto, GatewayUpdatePatch, GatewayWithProvidersDto, ProviderDto};
use crate::db::repo;
use crate::state::AppState;

// ─── 模型网关 CRUD ───

#[tauri::command]
pub async fn list_gateways(state: State<'_, AppState>) -> Result<Vec<GatewayDto>, AppError> {
    let rows = repo::models::list_gateways(&*state.db.pool().await).await?;
    Ok(rows)
}

#[tauri::command]
pub async fn add_gateway(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
) -> Result<String, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    repo::models::insert_gateway(
        &*state.db.pool().await,
        &repo::models::NewGateway {
            id: id.clone(),
            name,
            description,
        },
    )
    .await?;
    Ok(id)
}

#[tauri::command]
pub async fn update_gateway(
    state: State<'_, AppState>,
    id: String,
    updates: GatewayUpdatePatch,
) -> Result<(), AppError> {
    repo::models::update_gateway(&*state.db.pool().await, &id, &updates).await?;
    Ok(())
}

#[tauri::command]
pub async fn delete_gateway(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    repo::models::delete_gateway(&*state.db.pool().await, &id).await?;
    Ok(())
}

#[tauri::command]
pub async fn list_gateway_providers(
    state: State<'_, AppState>,
    gateway_id: String,
) -> Result<Vec<ProviderDto>, AppError> {
    let rows =
        repo::models::list_providers_by_gateway(&*state.db.pool().await, &gateway_id).await?;
    Ok(rows)
}

#[tauri::command]
pub async fn list_gateways_with_providers(
    state: State<'_, AppState>,
) -> Result<Vec<GatewayWithProvidersDto>, AppError> {
    Ok(repo::models::gateways::list_with_providers(&*state.db.pool().await).await?)
}
