// FastClaw · 数据库层
// 使用 sqlx + SQLite，WAL 模式，异步访问

pub mod repo;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::ops::Deref;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 轻量池子句柄：只持有 SqlitePool，不锁定，允许多个操作并发访问数据库。
pub struct DbPool(SqlitePool);

impl Deref for DbPool {
    type Target = SqlitePool;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// 数据库连接池封装（池子可在线替换，用于恢复操作）
#[derive(Clone)]
pub struct Database {
    /// Mutex 仅用于 close_pool / recreate_pool 这类替换池子的操作。
    /// 日常 pool() 调用只在克隆池子时短暂持锁（微秒级），不会串行化业务操作。
    inner: Arc<Mutex<SqlitePool>>,
}

impl Database {
    pub async fn init() -> anyhow::Result<Self> {
        let pool = Self::create_pool().await?;
        Ok(Self {
            inner: Arc::new(Mutex::new(pool)),
        })
    }

    async fn create_pool() -> anyhow::Result<SqlitePool> {
        let db_path = Self::db_path();
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
        let options = SqliteConnectOptions::from_str(&db_url)?
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .foreign_keys(true)
            .busy_timeout(std::time::Duration::from_secs(30));
        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect_with(options)
            .await?;
        let migrator = sqlx::migrate!("./migrations");

        // 兜底：清除数据库中"已应用但迁移目录里已不存在"的孤立迁移记录。
        // 否则 sqlx 会因 "migration ... was previously applied but is missing
        // in the resolved migrations" 而直接失败，导致程序无法启动。
        //
        // ★ 禁止静默改写已应用迁移的 checksum：那会让 sqlx 完整性校验失效，
        //   导致「旧库按旧 SQL 建表 / 新装机按改过的历史迁移建表」却都显示校验通过。
        //   历史迁移文件一旦发布/落库就不可改；schema 变更请新增迁移。
        //   开发期改了已应用迁移：删本地库后重启，或回滚文件内容。
        let migrations_table_exists: bool = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations'",
        )
        .fetch_one(&pool)
        .await?
            > 0;
        if migrations_table_exists {
            let known_versions: std::collections::HashSet<i64> =
                migrator.iter().map(|m| m.version).collect();
            let applied_versions: Vec<i64> =
                sqlx::query_scalar::<_, i64>("SELECT version FROM _sqlx_migrations")
                    .fetch_all(&pool)
                    .await?;
            for version in &applied_versions {
                if !known_versions.contains(version) {
                    tracing::warn!("移除孤立的迁移记录(迁移文件已不存在): {}", version);
                    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ?")
                        .bind(version)
                        .execute(&pool)
                        .await?;
                }
            }

            // 显式检测 checksum 漂移，给出可操作的错误（不改写库内 checksum）
            let applied_ok: std::collections::HashSet<i64> = sqlx::query_scalar::<_, i64>(
                "SELECT version FROM _sqlx_migrations WHERE success = 1",
            )
            .fetch_all(&pool)
            .await?
            .into_iter()
            .collect();
            for m in migrator.iter() {
                if !applied_ok.contains(&m.version) {
                    continue;
                }
                let stored: Option<Vec<u8>> = sqlx::query_scalar::<_, Vec<u8>>(
                    "SELECT checksum FROM _sqlx_migrations WHERE version = ?",
                )
                .bind(m.version)
                .fetch_optional(&pool)
                .await?;
                if let Some(stored) = stored {
                    if stored.as_slice() != m.checksum.as_ref() {
                        anyhow::bail!(
                            "迁移 {} 的内容与数据库记录的 checksum 不一致（历史迁移不可修改）。\
                             请新增迁移文件做 schema 变更；开发环境可删除本地库后重启：{}",
                            m.version,
                            db_path.display()
                        );
                    }
                }
            }
        }

        migrator.run(&pool).await?;

        // 确保新字段存在（sqlx迁移可能未执行时兜底）
        sqlx::query("ALTER TABLE wsp_services ADD COLUMN categoryName TEXT")
            .execute(&pool)
            .await
            .ok();
        sqlx::query("ALTER TABLE wsp_services ADD COLUMN subCategoryName TEXT")
            .execute(&pool)
            .await
            .ok();
        sqlx::query("ALTER TABLE wsp_model_market ADD COLUMN categoryName TEXT")
            .execute(&pool)
            .await
            .ok();
        sqlx::query("ALTER TABLE wsp_model_market ADD COLUMN subCategoryName TEXT")
            .execute(&pool)
            .await
            .ok();

        // 幂等清理旧模型能力字段（替代 20260701000001_drop_model_support_flags.sql，
        // 避免 ALTER TABLE DROP COLUMN 在列不存在时致命报错导致启动失败）
        sqlx::query("ALTER TABLE models DROP COLUMN support_thinking")
            .execute(&pool)
            .await
            .ok();
        sqlx::query("ALTER TABLE models DROP COLUMN support_image_parse")
            .execute(&pool)
            .await
            .ok();
        sqlx::query("ALTER TABLE models DROP COLUMN support_text_to_image")
            .execute(&pool)
            .await
            .ok();

        tracing::info!("数据库初始化完成: {}", db_path.display());
        Ok(pool)
    }

    /// 获取数据库连接池（只短暂持锁克隆，不影响并发业务操作）
    pub async fn pool(&self) -> DbPool {
        DbPool(self.inner.lock().await.clone())
    }

    /// 关闭连接池（重置/恢复前调用）。
    /// 先克隆出池再释放 Mutex，在锁外 `close`，避免长时间阻塞所有 `pool()`。
    /// `close` 设 5s 上限，防止被占着的连接拖死 reset/restore。
    pub async fn close_pool(&self) -> anyhow::Result<()> {
        let pool = {
            let guard = self.inner.lock().await;
            guard.clone()
        };

        match tokio::time::timeout(std::time::Duration::from_secs(5), pool.close()).await {
            Ok(()) => tracing::info!("数据库连接池已关闭"),
            Err(_) => tracing::warn!(
                "关闭连接池超时(5s)，继续执行重置/恢复（后续 recreate_pool 会换新池）"
            ),
        }
        // 给 Windows 一点时间释放文件句柄
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        Ok(())
    }

    /// 重建连接池（恢复/重置后调用）。
    /// 建池在锁外进行；仅交换指针时短暂持锁。
    pub async fn recreate_pool(&self) -> anyhow::Result<()> {
        let new_pool = Self::create_pool().await?;
        let old_pool = {
            let mut guard = self.inner.lock().await;
            std::mem::replace(&mut *guard, new_pool)
        };
        // 旧池在锁外尽量关掉，超时不阻塞恢复成功路径
        match tokio::time::timeout(std::time::Duration::from_secs(2), old_pool.close()).await {
            Ok(()) => {}
            Err(_) => tracing::warn!("旧连接池关闭超时(2s)，已切换到新池"),
        }
        Ok(())
    }

    fn db_path() -> PathBuf {
        // --features dev-db 在开发时通过 tauri:dev 脚本注入，生产构建不含此 feature
        // src-tauri/storage/ 已在 .gitignore，不会污染 Git 历史
        if cfg!(feature = "dev-db") {
            let dev_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("storage");
            return dev_dir.join("fastclaw_dev.db");
        }
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("FastClaw");
        data_dir.join("fastclaw.db")
    }

    pub fn db_path_static() -> PathBuf {
        Self::db_path()
    }
}
