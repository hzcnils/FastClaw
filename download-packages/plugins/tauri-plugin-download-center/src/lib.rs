use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tauri::{
    plugin::{Builder as TauriBuilder, TauriPlugin},
    AppHandle, Manager, State, Wry,
};
use tauri_plugin_shared::{
    record_schema_version, schema_version, ApiResult, PluginError, SharedDatabase,
};
use tokio::sync::OnceCell;

pub mod manager;
pub mod model;

const PLUGIN_NAME: &str = "download-center";
const SCHEMA_VERSION: i64 = 2;
pub const IPC_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
pub struct DownloadRequest {
    pub action: String,
    #[serde(default)]
    pub payload: Value,
}

pub struct DownloadPluginState {
    database: Arc<dyn SharedDatabase>,
    manager: manager::DownloadManager,
    ready: Arc<OnceCell<()>>,
}

impl DownloadPluginState {
    pub fn database(&self) -> &Arc<dyn SharedDatabase> {
        &self.database
    }
    pub fn manager(&self) -> &manager::DownloadManager {
        &self.manager
    }
    async fn ensure_ready(&self) -> Result<(), PluginError> {
        initialize_runtime(&self.database, &self.manager, &self.ready).await
    }
}

/// Applies plugin-owned tables to the host pool. Kept public for migrations and
/// diagnostics; normal hosts no longer need to call it manually.
pub async fn initialize_shared_schema(database: &dyn SharedDatabase) -> Result<(), PluginError> {
    let pool = database.sqlite_pool().await?;
    if schema_version(&pool, PLUGIN_NAME).await? >= SCHEMA_VERSION {
        return Ok(());
    }
    sqlx::query("CREATE TABLE IF NOT EXISTS package_download_tasks (id TEXT PRIMARY KEY NOT NULL, resource_type TEXT NOT NULL, resource_id TEXT NOT NULL, name TEXT NOT NULL, status TEXT NOT NULL, phase TEXT NOT NULL DEFAULT '', progress INTEGER NOT NULL DEFAULT 0, downloaded_bytes INTEGER NOT NULL DEFAULT 0, total_bytes INTEGER, message_key TEXT, message_params_json TEXT, error_code TEXT, error_message TEXT, payload_json TEXT NOT NULL DEFAULT '{}', target_path TEXT, temp_path TEXT, etag TEXT, last_modified TEXT, silent INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)").execute(&pool).await.map_err(db_error)?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_package_download_tasks_status ON package_download_tasks(status)").execute(&pool).await.map_err(db_error)?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_package_download_tasks_resource ON package_download_tasks(resource_type,resource_id)").execute(&pool).await.map_err(db_error)?;
    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_package_download_tasks_active_resource ON package_download_tasks(resource_type,resource_id) WHERE status IN ('pending','downloading','pausing','paused','verifying','installing')").execute(&pool).await.map_err(db_error)?;
    record_schema_version(&pool, PLUGIN_NAME, SCHEMA_VERSION).await
}

fn db_error(error: sqlx::Error) -> PluginError {
    PluginError::new("DATABASE_ERROR", error.to_string())
}

async fn initialize_runtime(
    database: &Arc<dyn SharedDatabase>,
    manager: &manager::DownloadManager,
    ready: &Arc<OnceCell<()>>,
) -> Result<(), PluginError> {
    ready
        .get_or_try_init(|| async {
            initialize_shared_schema(database.as_ref()).await?;
            let pool = database.sqlite_pool().await?;
            manager.recover_interrupted(&pool).await.map_err(db_error)?;
            Ok::<(), PluginError>(())
        })
        .await
        .map(|_| ())
}

fn action_error(message: String) -> PluginError {
    let code = if message.contains("not found") {
        "NOT_FOUND"
    } else if message.contains("already")
        || message.contains("current state")
        || message.contains("active task")
    {
        "CONFLICT"
    } else {
        "VALIDATION_ERROR"
    };
    PluginError::new(code, message)
}

async fn dispatch(
    app: AppHandle,
    state: &DownloadPluginState,
    action: &str,
    payload: Value,
) -> Result<Value, PluginError> {
    state.ensure_ready().await?;
    let pool = state.database.sqlite_pool().await?;
    match action {
        "list" => serde_json::to_value(state.manager.list(&pool).await.map_err(db_error)?)
            .map_err(|error| PluginError::new("INTERNAL_ERROR", error.to_string())),
        "start" => {
            let input: model::StartDownloadInput = serde_json::from_value(payload)
                .map_err(|error| PluginError::new("VALIDATION_ERROR", error.to_string()))?;
            let row = state
                .manager
                .create(&pool, input)
                .await
                .map_err(action_error)?;
            let task: model::DownloadTask = row.clone().into();
            state.manager.spawn(pool, app, row).await;
            serde_json::to_value(task)
                .map_err(|error| PluginError::new("INTERNAL_ERROR", error.to_string()))
        }
        "pause" => {
            let input: model::TaskInput = serde_json::from_value(payload)
                .map_err(|error| PluginError::new("VALIDATION_ERROR", error.to_string()))?;
            let task = state
                .manager
                .request_pause(&pool, &app, &input.task_id)
                .await
                .map_err(action_error)?;
            serde_json::to_value(task)
                .map_err(|error| PluginError::new("INTERNAL_ERROR", error.to_string()))
        }
        "resume" | "retry" => {
            let input: model::TaskInput = serde_json::from_value(payload)
                .map_err(|error| PluginError::new("VALIDATION_ERROR", error.to_string()))?;
            let task = state
                .manager
                .resume(pool, app, &input.task_id)
                .await
                .map_err(action_error)?;
            serde_json::to_value(task)
                .map_err(|error| PluginError::new("INTERNAL_ERROR", error.to_string()))
        }
        "cancel" => {
            let input: model::TaskInput = serde_json::from_value(payload)
                .map_err(|error| PluginError::new("VALIDATION_ERROR", error.to_string()))?;
            let task = state
                .manager
                .request_cancel(&pool, &app, &input.task_id)
                .await
                .map_err(action_error)?;
            serde_json::to_value(task)
                .map_err(|error| PluginError::new("INTERNAL_ERROR", error.to_string()))
        }
        "remove" => {
            let input: model::TaskInput = serde_json::from_value(payload)
                .map_err(|error| PluginError::new("VALIDATION_ERROR", error.to_string()))?;
            state
                .manager
                .remove(&pool, &input.task_id)
                .await
                .map_err(action_error)?;
            Ok(Value::Null)
        }
        _ => Err(PluginError::new(
            "NOT_FOUND",
            format!("Unsupported download action: {action}"),
        )),
    }
}

#[tauri::command]
async fn execute(
    app: AppHandle,
    state: State<'_, DownloadPluginState>,
    request: DownloadRequest,
) -> Result<ApiResult<Value>, String> {
    Ok(
        match dispatch(app, &state, &request.action, request.payload).await {
            Ok(value) => ApiResult::ok(value),
            Err(error) => ApiResult::fail(error),
        },
    )
}

pub fn init(database: Arc<dyn SharedDatabase>) -> TauriPlugin<Wry> {
    init_with_config(database, manager::DownloadManagerConfig::default())
}

pub fn init_with_config(
    database: Arc<dyn SharedDatabase>,
    config: manager::DownloadManagerConfig,
) -> TauriPlugin<Wry> {
    init_with_install_hook(database, config, None)
}

pub fn init_with_install_hook(
    database: Arc<dyn SharedDatabase>,
    config: manager::DownloadManagerConfig,
    hook: Option<Arc<dyn manager::InstallHook>>,
) -> TauriPlugin<Wry> {
    let manager = manager::DownloadManager::new(config);
    if let Some(hook) = hook { manager.set_install_hook(hook); }
    let ready = Arc::new(OnceCell::new());
    let eager_database = database.clone();
    let eager_manager = manager.clone();
    let eager_ready = ready.clone();
    let state = DownloadPluginState {
        database,
        manager,
        ready,
    };
    TauriBuilder::new(PLUGIN_NAME)
        .invoke_handler(tauri::generate_handler![execute])
        .setup(move |app, _api| {
            app.manage(state);
            tauri::async_runtime::spawn(async move {
                let _ = initialize_runtime(&eager_database, &eager_manager, &eager_ready).await;
            });
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    struct TestDatabase(SqlitePool);
    #[async_trait]
    impl SharedDatabase for TestDatabase {
        async fn sqlite_pool(&self) -> Result<SqlitePool, PluginError> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn runtime_initialization_creates_schema_and_is_idempotent() {
        tauri::async_runtime::block_on(async {
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .unwrap();
            let database: Arc<dyn SharedDatabase> = Arc::new(TestDatabase(pool.clone()));
            let manager = manager::DownloadManager::default();
            let ready = Arc::new(OnceCell::new());
            initialize_runtime(&database, &manager, &ready)
                .await
                .unwrap();
            initialize_runtime(&database, &manager, &ready)
                .await
                .unwrap();
            let table:Option<String>=sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table' AND name='package_download_tasks'").fetch_optional(&pool).await.unwrap();
            let version:i64=sqlx::query_scalar("SELECT version FROM package_plugin_schema_versions WHERE plugin_name='download-center'").fetch_one(&pool).await.unwrap();
            assert_eq!(table.as_deref(), Some("package_download_tasks"));
            assert_eq!(version, SCHEMA_VERSION);
        });
    }
}
