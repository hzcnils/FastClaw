use async_trait::async_trait;
use sqlx::SqlitePool;
use tauri_plugin_shared::{PluginError, SharedDatabase};

use crate::db::Database;

#[derive(Clone)]
pub struct HostSharedDatabase(pub Database);

#[async_trait]
impl SharedDatabase for HostSharedDatabase {
    async fn sqlite_pool(&self) -> Result<SqlitePool, PluginError> {
        Ok(self.0.pool().await.clone())
    }
}
