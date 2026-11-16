// FastClaw · 系统设置 Tauri Commands
use crate::core::error::AppError;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_settings(
    state: State<'_, AppState>,
    category: Option<String>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let query = if let Some(cat) = category {
        sqlx::query_as::<_, (String, String, String, Option<String>)>(
            "SELECT key, value, category, description FROM settings WHERE category = ?",
        )
        .bind(cat)
        .fetch_all(&*state.db.pool().await)
        .await?
    } else {
        sqlx::query_as::<_, (String, String, String, Option<String>)>(
            "SELECT key, value, category, description FROM settings",
        )
        .fetch_all(&*state.db.pool().await)
        .await?
    };
    let rows: Vec<serde_json::Value> = query.iter().map(|(k, v, c, d)| {
        serde_json::json!({"key": k, "value": v, "category": c, "description": d})
    }).collect();
    Ok(rows)
}
#[tauri::command]
pub async fn update_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), AppError> {
    let pool = &*state.db.pool().await;
    let affected =
        sqlx::query("UPDATE settings SET value = ?, updated_at = datetime('now') WHERE key = ?")
            .bind(&value)
            .bind(&key)
            .execute(pool)
            .await?
            .rows_affected();
    if affected == 0 {
        sqlx::query("INSERT INTO settings (key, value, category, updated_at) VALUES (?, ?, 'general', datetime('now'))")
            .bind(&key).bind(&value).execute(pool).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn update_setting_with_category(
    state: State<'_, AppState>,
    key: String,
    value: String,
    category: String,
) -> Result<(), AppError> {
    sqlx::query("INSERT OR REPLACE INTO settings (key, value, category, updated_at) VALUES (?, ?, ?, datetime('now'))")
        .bind(&key).bind(&value).bind(&category).execute(&*state.db.pool().await).await?;
    Ok(())
}

#[tauri::command]
pub async fn update_setting_with_description(
    state: State<'_, AppState>,
    key: String,
    value: String,
    description: String,
    category: String,
) -> Result<(), AppError> {
    sqlx::query("INSERT OR REPLACE INTO settings (key, value, description, category, updated_at) VALUES (?, ?, ?, ?, datetime('now'))")
        .bind(&key).bind(&value).bind(&description).bind(&category).execute(&*state.db.pool().await).await?;
    Ok(())
}

/// 设置页「测试搜索」按钮用：用当前 websearch 配置跑一次搜索，返回命中
#[tauri::command]
pub async fn web_search_test(
    state: State<'_, AppState>,
    query: String,
    provider: Option<String>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let pool = state.db.pool().await.clone();
    let cfg = crate::services::web_search::load_cfg(&pool, provider).await;
    let hits = crate::services::web_search::web_search(&query, &cfg).await?;
    Ok(hits
        .into_iter()
        .map(|h| serde_json::json!({ "title": h.title, "url": h.url, "snippet": h.snippet }))
        .collect())
}
