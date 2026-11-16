use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiResult<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<ApiError>,
}

impl<T: Serialize> ApiResult<T> {
    pub fn ok(data: T) -> Self {
        Self { success: true, data: Some(data), error: None }
    }

    pub fn fail(error: impl Into<ApiError>) -> Self {
        Self { success: false, data: None, error: Some(error.into()) }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct PluginError {
    pub code: String,
    pub message: String,
    pub details: Option<Value>,
}

impl PluginError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into(), details: None }
    }
}

impl From<PluginError> for ApiError {
    fn from(value: PluginError) -> Self {
        Self { code: value.code, message: value.message, details: value.details }
    }
}

/// The host implements this trait and returns a clone of its existing SQLite pool.
/// Plugins never derive a second database path or create another database file.
#[async_trait]
pub trait SharedDatabase: Send + Sync {
    async fn sqlite_pool(&self) -> Result<SqlitePool, PluginError>;
}

pub async fn ensure_schema_registry(pool: &SqlitePool) -> Result<(), PluginError> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS package_plugin_schema_versions (\
         plugin_name TEXT PRIMARY KEY NOT NULL,\
         version INTEGER NOT NULL,\
         updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)",
    )
    .execute(pool)
    .await
    .map_err(|error| PluginError::new("DATABASE_ERROR", error.to_string()))?;
    Ok(())
}

pub async fn schema_version(pool: &SqlitePool, plugin_name: &str) -> Result<i64, PluginError> {
    ensure_schema_registry(pool).await?;
    sqlx::query_scalar::<_, i64>(
        "SELECT version FROM package_plugin_schema_versions WHERE plugin_name = ?",
    )
    .bind(plugin_name)
    .fetch_optional(pool)
    .await
    .map(|value| value.unwrap_or(0))
    .map_err(|error| PluginError::new("DATABASE_ERROR", error.to_string()))
}

pub async fn record_schema_version(pool: &SqlitePool, plugin_name: &str, version: i64) -> Result<(), PluginError> {
    sqlx::query(
        "INSERT INTO package_plugin_schema_versions(plugin_name, version) VALUES(?, ?) \
         ON CONFLICT(plugin_name) DO UPDATE SET version = excluded.version, updated_at = CURRENT_TIMESTAMP",
    )
    .bind(plugin_name)
    .bind(version)
    .execute(pool)
    .await
    .map_err(|error| PluginError::new("DATABASE_ERROR", error.to_string()))?;
    Ok(())
}
