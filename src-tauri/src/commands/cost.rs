// FastClaw · 费用追踪 Tauri Commands
use crate::core::error::AppError;
use crate::db::repo;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_usage_stats(
    state: State<'_, AppState>,
    period: Option<String>,
) -> Result<serde_json::Value, AppError> {
    let stats =
        repo::cost::get_stats(&*state.db.pool().await, "default-user", period.as_deref()).await?;
    Ok(stats)
}

#[tauri::command]
pub async fn get_budgets(state: State<'_, AppState>) -> Result<Vec<serde_json::Value>, AppError> {
    let rows = repo::cost::list_budgets(&*state.db.pool().await, "default-user").await?;
    Ok(rows)
}

#[tauri::command]
pub async fn set_budget(
    state: State<'_, AppState>,
    budget_type: String,
    limit_usd: f64,
    action: String,
) -> Result<(), AppError> {
    repo::cost::upsert_budget(
        &*state.db.pool().await,
        "default-user",
        &budget_type,
        limit_usd,
        &action,
    )
    .await?;
    Ok(())
}
