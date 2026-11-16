use crate::model::{
    CleanupTarget, DownloadExecutionPlan, DownloadInstruction, DownloadTask, DownloadTaskRow,
    ReplaceSource, StartDownloadInput,
};
use futures::StreamExt;
use reqwest::{header, StatusCode};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc,
    },
};
use tauri::{AppHandle, Emitter};
use tokio::{
    io::AsyncWriteExt,
    sync::{RwLock, Semaphore},
};

pub const TASK_UPDATED_EVENT: &str = "download-task-updated";

/// 宿主授权的覆盖边界：下载、校验、解压期间不调用。
#[async_trait::async_trait]
pub trait InstallHook: Send + Sync {
    async fn before_overlay(&self, app: &AppHandle, task: &DownloadTaskRow, target: &Path)
        -> Result<Option<Box<dyn InstallGuard>>, String>;
}

#[async_trait::async_trait]
pub trait InstallGuard: Send + Sync {
    async fn finish(&mut self, success: bool) -> Result<(), String>;
}

async fn guarded_overlay<F>(mut guard: Option<Box<dyn InstallGuard>>, control: &AtomicU8, copy: F) -> Result<(), String>
where F: std::future::Future<Output = Result<(), String>> {
    if control.load(Ordering::SeqCst) != RUNNING {
        if let Some(guard) = guard.as_mut() { let _ = guard.finish(false).await; }
        return Err("TASK_CANCELLED".into());
    }
    let result = copy.await;
    let finished = if let Some(guard) = guard.as_mut() { guard.finish(result.is_ok()).await } else { Ok(()) };
    result.and(finished)
}
const RUNNING: u8 = 0;
const PAUSE: u8 = 1;
const CANCEL: u8 = 2;
const SELECT:&str="SELECT id,resource_type,resource_id,name,status,phase,progress,downloaded_bytes,total_bytes,message_key,message_params_json,error_code,error_message,payload_json,target_path,temp_path,etag,last_modified,silent,created_at,updated_at FROM package_download_tasks";

#[derive(Debug, Clone)]
pub struct DownloadManagerConfig {
    pub max_concurrency: usize,
    pub progress_emit_interval_ms: u64,
}
impl Default for DownloadManagerConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 3,
            progress_emit_interval_ms: 200,
        }
    }
}
struct Inner {
    install_hook: std::sync::RwLock<Option<Arc<dyn InstallHook>>>,
    controls: RwLock<HashMap<String, Arc<AtomicU8>>>,
    semaphore: Arc<Semaphore>,
    recovered: AtomicBool,
    config: DownloadManagerConfig,
}
#[derive(Clone)]
pub struct DownloadManager {
    inner: Arc<Inner>,
}
impl Default for DownloadManager {
    fn default() -> Self {
        Self::new(DownloadManagerConfig::default())
    }
}

#[derive(Default)]
struct Paths {
    temp: Option<PathBuf>,
    staging: Option<PathBuf>,
    target: Option<PathBuf>,
    backup: Option<PathBuf>,
}

impl DownloadManager {
    pub fn set_install_hook(&self, hook: Arc<dyn InstallHook>) {
        *self.inner.install_hook.write().unwrap() = Some(hook);
    }

    pub fn new(config: DownloadManagerConfig) -> Self {
        Self {
            inner: Arc::new(Inner {
                install_hook: std::sync::RwLock::new(None),
                controls: RwLock::new(HashMap::new()),
                semaphore: Arc::new(Semaphore::new(config.max_concurrency.max(1))),
                recovered: AtomicBool::new(false),
                config,
            }),
        }
    }
    pub async fn recover_interrupted(&self, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        if self.inner.recovered.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        sqlx::query("UPDATE package_download_tasks SET status='paused',phase='paused',message_key='download.restartPaused',updated_at=datetime('now') WHERE status IN ('pending','downloading','pausing','verifying','installing')").execute(pool).await?;
        Ok(())
    }
    pub async fn list(&self, pool: &SqlitePool) -> Result<Vec<DownloadTask>, sqlx::Error> {
        self.recover_interrupted(pool).await?;
        let rows =
            sqlx::query_as::<_, DownloadTaskRow>(&format!("{SELECT} ORDER BY created_at DESC"))
                .fetch_all(pool)
                .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
    pub async fn get(
        &self,
        pool: &SqlitePool,
        id: &str,
    ) -> Result<Option<DownloadTaskRow>, sqlx::Error> {
        sqlx::query_as::<_, DownloadTaskRow>(&format!("{SELECT} WHERE id=?"))
            .bind(id)
            .fetch_optional(pool)
            .await
    }
    pub async fn create(
        &self,
        pool: &SqlitePool,
        input: StartDownloadInput,
    ) -> Result<DownloadTaskRow, String> {
        if input.resource_type.trim().is_empty()
            || input.resource_id.trim().is_empty()
            || input.name.trim().is_empty()
        {
            return Err("resourceType, resourceId and name are required".into());
        }
        let active:Option<String>=sqlx::query_scalar("SELECT id FROM package_download_tasks WHERE resource_type=? AND resource_id=? AND status IN ('pending','downloading','pausing','paused','verifying','installing') LIMIT 1").bind(&input.resource_type).bind(&input.resource_id).fetch_optional(pool).await.map_err(|e|e.to_string())?;
        if active.is_some() {
            return Err("active task already exists".into());
        }
        let plan = parse_plan(&input.payload)?;
        validate_plan(&plan)?;
        let temp = plan.instructions.iter().find_map(|item| match item {
            DownloadInstruction::Download { temp_path, .. } => Some(temp_path.clone()),
            _ => None,
        });
        let target = plan.instructions.iter().rev().find_map(|item| match item {
            DownloadInstruction::ReplaceTarget { target_path, .. }
            | DownloadInstruction::OverlayTarget { target_path, .. }
            | DownloadInstruction::BackupTarget { target_path, .. } => Some(target_path.clone()),
            _ => None,
        });
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO package_download_tasks(id,resource_type,resource_id,name,status,phase,message_key,payload_json,target_path,temp_path,silent) VALUES(?,?,?,?,'pending','queued','download.queued',?,?,?,?)").bind(&id).bind(&input.resource_type).bind(&input.resource_id).bind(&input.name).bind(input.payload.to_string()).bind(target).bind(temp).bind(input.silent).execute(pool).await.map_err(|e|e.to_string())?;
        self.get(pool, &id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "task creation failed".into())
    }
    pub async fn spawn(&self, pool: SqlitePool, app: AppHandle, row: DownloadTaskRow) {
        let manager = self.clone();
        let id = row.id.clone();
        let control = Arc::new(AtomicU8::new(RUNNING));
        self.inner
            .controls
            .write()
            .await
            .insert(id.clone(), control.clone());
        tauri::async_runtime::spawn(async move {
            let _permit = match manager.inner.semaphore.clone().acquire_owned().await {
                Ok(value) => value,
                Err(_) => return,
            };
            if let Err(error) = manager.run(&pool, &app, &row, &control).await {
                if !matches!(error.as_str(), "TASK_PAUSED" | "TASK_CANCELLED") {
                    let _ = manager
                        .persist(
                            &pool,
                            &id,
                            "failed",
                            "error",
                            row.progress,
                            row.downloaded_bytes,
                            row.total_bytes,
                            "download.failed",
                            None,
                            Some(("DOWNLOAD_ERROR", &error)),
                        )
                        .await;
                    manager.emit(&pool, &app, &id).await;
                }
            }
            manager.inner.controls.write().await.remove(&id);
        });
    }
    async fn run(
        &self,
        pool: &SqlitePool,
        app: &AppHandle,
        row: &DownloadTaskRow,
        control: &AtomicU8,
    ) -> Result<(), String> {
        let payload: serde_json::Value =
            serde_json::from_str(&row.payload_json).map_err(|e| e.to_string())?;
        let plan = parse_plan(&payload)?;
        validate_plan(&plan)?;
        let mut paths = Paths::default();
        let mut downloaded = row.downloaded_bytes.max(0) as u64;
        let mut total = row.total_bytes.map(|v| v.max(0) as u64);
        for instruction in plan.instructions {
            match control.load(Ordering::SeqCst) {
                PAUSE => {
                    if let Some(target) = &paths.target {
                        self.rollback(&paths, target).await;
                    }
                    self.persist(
                        pool,
                        &row.id,
                        "paused",
                        "paused",
                        percent(downloaded, total),
                        downloaded as i64,
                        total.map(|v| v as i64),
                        "download.paused",
                        None,
                        None,
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                    self.emit(pool, app, &row.id).await;
                    return Err("TASK_PAUSED".into());
                }
                CANCEL => {
                    if let Some(target) = &paths.target {
                        self.rollback(&paths, target).await;
                    }
                    if let Some(path) = &paths.temp {
                        let _ = remove_path(path).await;
                    }
                    if let Some(path) = &paths.staging {
                        let _ = remove_path(path).await;
                    }
                    self.persist(
                        pool,
                        &row.id,
                        "cancelled",
                        "cancelled",
                        0,
                        0,
                        total.map(|v| v as i64),
                        "download.cancelled",
                        None,
                        None,
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                    self.emit(pool, app, &row.id).await;
                    return Err("TASK_CANCELLED".into());
                }
                _ => {}
            }
            match instruction {
                DownloadInstruction::Download {
                    url,
                    temp_path,
                    resume,
                } => {
                    paths.temp = Some(PathBuf::from(&temp_path));
                    let result = self
                        .download(pool, app, row, control, &url, Path::new(&temp_path), resume)
                        .await?;
                    downloaded = result.0;
                    total = result.1
                }
                DownloadInstruction::VerifySha256 { sha256 } => {
                    self.phase(
                        pool,
                        app,
                        row,
                        "verifying",
                        "download.verifying",
                        downloaded,
                        total,
                    )
                    .await?;
                    let path = paths.temp.clone().ok_or("download instruction missing")?;
                    let actual = sha256_file(path).await?;
                    if !actual.eq_ignore_ascii_case(&sha256) {
                        return Err(format!("SHA256 mismatch: expected {sha256}, got {actual}"));
                    }
                }
                DownloadInstruction::ExtractZip {
                    staging_path,
                    required_file_suffix,
                } => {
                    self.phase(
                        pool,
                        app,
                        row,
                        "extracting",
                        "download.extracting",
                        downloaded,
                        total,
                    )
                    .await?;
                    let archive = paths.temp.clone().ok_or("download instruction missing")?;
                    let staging = PathBuf::from(staging_path);
                    remove_path(&staging).await?;
                    let a = archive.clone();
                    let s = staging.clone();
                    tokio::task::spawn_blocking(move || extract_zip_safe(&a, &s))
                        .await
                        .map_err(|e| e.to_string())??;
                    if let Some(suffix) = required_file_suffix {
                        let root = staging.clone();
                        if !tokio::task::spawn_blocking(move || contains_suffix(&root, &suffix))
                            .await
                            .map_err(|e| e.to_string())?
                        {
                            return Err("required file suffix not found".into());
                        }
                    }
                    paths.staging = Some(staging)
                }
                DownloadInstruction::BackupTarget {
                    target_path,
                    backup_path,
                } => {
                    self.phase(
                        pool,
                        app,
                        row,
                        "installing",
                        "download.installing",
                        downloaded,
                        total,
                    )
                    .await?;
                    let target = PathBuf::from(target_path);
                    let backup = backup_path
                        .map(PathBuf::from)
                        .unwrap_or_else(|| target.with_extension("backup"));
                    remove_path(&backup).await?;
                    if target.exists() {
                        tokio::fs::rename(&target, &backup)
                            .await
                            .map_err(|e| e.to_string())?
                    }
                    paths.target = Some(target);
                    paths.backup = Some(backup)
                }
                DownloadInstruction::ReplaceTarget {
                    source,
                    target_path,
                } => {
                    self.phase(
                        pool,
                        app,
                        row,
                        "installing",
                        "download.installing",
                        downloaded,
                        total,
                    )
                    .await?;
                    let target = PathBuf::from(target_path);
                    let Some(source) = (match source {
                        ReplaceSource::Temp => paths.temp.clone(),
                        ReplaceSource::Staging => paths.staging.clone(),
                    }) else {
                        self.rollback(&paths, &target).await;
                        return Err("replace source is unavailable".into());
                    };
                    if target.exists() {
                        if let Err(error) = remove_path(&target).await {
                            self.rollback(&paths, &target).await;
                            return Err(error);
                        }
                    }
                    if let Err(error) = tokio::fs::rename(&source, &target).await {
                        self.rollback(&paths, &target).await;
                        return Err(error.to_string());
                    }
                    paths.target = Some(target)
                }
                DownloadInstruction::OverlayTarget {
                    source,
                    target_path,
                } => {
                    self.phase(
                        pool,
                        app,
                        row,
                        "installing",
                        "download.installing",
                        downloaded,
                        total,
                    )
                    .await?;
                    let target = PathBuf::from(target_path);
                    let Some(source) = (match source {
                        ReplaceSource::Temp => paths.temp.clone(),
                        ReplaceSource::Staging => paths.staging.clone(),
                    }) else {
                        return Err("overlay source is unavailable".into());
                    };
                    let source_for_copy = source.clone();
                    let target_for_copy = target.clone();
                    let hook = self.inner.install_hook.read().unwrap().clone();
                    let guard = match hook {
                        Some(hook) => hook.before_overlay(app, row, &target).await?,
                        None => None,
                    };
                    guarded_overlay(guard, control, async move { tokio::task::spawn_blocking(move || {
                        overlay_copy_dir(&source_for_copy, &target_for_copy)
                    })
                    .await
                    .map_err(|error| error.to_string()).and_then(|result| result) }).await?;
                    paths.target = Some(target)
                }
                DownloadInstruction::Cleanup { targets } => {
                    for item in targets {
                        match item {
                            CleanupTarget::Temp => {
                                if let Some(path) = &paths.temp {
                                    remove_path(path).await?
                                }
                            }
                            CleanupTarget::Staging => {
                                if let Some(path) = &paths.staging {
                                    remove_path(path).await?
                                }
                            }
                            CleanupTarget::Backup => {
                                if let Some(path) = &paths.backup {
                                    remove_path(path).await?
                                }
                            }
                        }
                    }
                }
            }
        }
        self.persist(
            pool,
            &row.id,
            "success",
            "complete",
            100,
            downloaded as i64,
            total.map(|v| v as i64),
            "download.completed",
            None,
            None,
        )
        .await
        .map_err(|e| e.to_string())?;
        self.emit(pool, app, &row.id).await;
        Ok(())
    }
    async fn download(
        &self,
        pool: &SqlitePool,
        app: &AppHandle,
        row: &DownloadTaskRow,
        control: &AtomicU8,
        url: &str,
        temp: &Path,
        resume: bool,
    ) -> Result<(u64, Option<u64>), String> {
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err("only HTTP/HTTPS URLs are supported".into());
        }
        if let Some(parent) = temp.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| e.to_string())?
        }
        let mut existing = if resume {
            tokio::fs::metadata(temp)
                .await
                .map(|m| m.len())
                .unwrap_or(0)
        } else {
            0
        };
        let client = reqwest::Client::new();
        let mut request = client.get(url);
        if existing > 0 {
            request = request.header(header::RANGE, format!("bytes={existing}-"));
            if let Some(etag) = &row.etag {
                request = request.header(header::IF_RANGE, etag)
            }
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        if existing > 0 && response.status() == StatusCode::RANGE_NOT_SATISFIABLE {
            return Ok((existing, Some(existing)));
        }
        if !(response.status().is_success() || response.status() == StatusCode::PARTIAL_CONTENT) {
            return Err(format!("HTTP {}", response.status()));
        }
        let append = existing > 0 && response.status() == StatusCode::PARTIAL_CONTENT;
        if existing > 0 && !append {
            existing = 0
        }
        let response_bytes = response.content_length().unwrap_or(0);
        let total = if response_bytes > 0 {
            Some(existing + response_bytes)
        } else {
            None
        };
        let etag = response
            .headers()
            .get(header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(ToOwned::to_owned);
        let modified = response
            .headers()
            .get(header::LAST_MODIFIED)
            .and_then(|v| v.to_str().ok())
            .map(ToOwned::to_owned);
        sqlx::query("UPDATE package_download_tasks SET etag=?,last_modified=? WHERE id=?")
            .bind(etag)
            .bind(modified)
            .bind(&row.id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        let mut options = tokio::fs::OpenOptions::new();
        options.create(true).write(true);
        if append {
            options.append(true);
        } else {
            options.truncate(true);
        }
        let mut file = options.open(temp).await.map_err(|e| e.to_string())?;
        let mut downloaded = existing;
        self.phase(
            pool,
            app,
            row,
            "downloading",
            "download.downloading",
            downloaded,
            total,
        )
        .await?;
        let mut stream = response.bytes_stream();
        let mut last = std::time::Instant::now();
        while let Some(chunk) = stream.next().await {
            match control.load(Ordering::SeqCst) {
                PAUSE => {
                    file.flush().await.map_err(|e| e.to_string())?;
                    self.persist(
                        pool,
                        &row.id,
                        "paused",
                        "paused",
                        percent(downloaded, total),
                        downloaded as i64,
                        total.map(|v| v as i64),
                        "download.paused",
                        None,
                        None,
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                    self.emit(pool, app, &row.id).await;
                    return Err("TASK_PAUSED".into());
                }
                CANCEL => {
                    drop(file);
                    let _ = remove_path(temp).await;
                    self.persist(
                        pool,
                        &row.id,
                        "cancelled",
                        "cancelled",
                        0,
                        0,
                        total.map(|v| v as i64),
                        "download.cancelled",
                        None,
                        None,
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                    self.emit(pool, app, &row.id).await;
                    return Err("TASK_CANCELLED".into());
                }
                _ => {}
            }
            let chunk = chunk.map_err(|e| e.to_string())?;
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
            downloaded += chunk.len() as u64;
            if last.elapsed()
                >= std::time::Duration::from_millis(self.inner.config.progress_emit_interval_ms)
            {
                self.phase(
                    pool,
                    app,
                    row,
                    "downloading",
                    "download.downloading",
                    downloaded,
                    total,
                )
                .await?;
                last = std::time::Instant::now()
            }
        }
        file.flush().await.map_err(|e| e.to_string())?;
        if let Some(total) = total {
            if downloaded != total {
                return Err(format!("incomplete download {downloaded}/{total}"));
            }
        }
        Ok((downloaded, total))
    }
    async fn rollback(&self, paths: &Paths, target: &Path) {
        if let Some(backup) = &paths.backup {
            if backup.exists() {
                let _ = remove_path(target).await;
                let _ = tokio::fs::rename(backup, target).await;
            }
        }
    }
    async fn phase(
        &self,
        pool: &SqlitePool,
        app: &AppHandle,
        row: &DownloadTaskRow,
        phase: &str,
        key: &str,
        downloaded: u64,
        total: Option<u64>,
    ) -> Result<(), String> {
        let status = match phase {
            "downloading" => "downloading",
            "verifying" => "verifying",
            _ => "installing",
        };
        self.persist(
            pool,
            &row.id,
            status,
            phase,
            percent(downloaded, total),
            downloaded as i64,
            total.map(|v| v as i64),
            key,
            None,
            None,
        )
        .await
        .map_err(|e| e.to_string())?;
        self.emit(pool, app, &row.id).await;
        Ok(())
    }
    async fn persist(
        &self,
        pool: &SqlitePool,
        id: &str,
        status: &str,
        phase: &str,
        progress: i64,
        downloaded: i64,
        total: Option<i64>,
        key: &str,
        params: Option<&serde_json::Value>,
        error: Option<(&str, &str)>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE package_download_tasks SET status=?,phase=?,progress=?,downloaded_bytes=?,total_bytes=?,message_key=?,message_params_json=?,error_code=?,error_message=?,updated_at=datetime('now') WHERE id=? AND (status!='cancelled' OR ?='cancelled')").bind(status).bind(phase).bind(progress).bind(downloaded).bind(total).bind(key).bind(params.map(ToString::to_string)).bind(error.map(|v|v.0)).bind(error.map(|v|v.1)).bind(id).bind(status).execute(pool).await?;
        Ok(())
    }
    async fn emit(&self, pool: &SqlitePool, app: &AppHandle, id: &str) {
        if let Ok(Some(row)) = self.get(pool, id).await {
            let _ = app.emit(TASK_UPDATED_EVENT, DownloadTask::from(row));
        }
    }
    pub async fn request_pause(
        &self,
        pool: &SqlitePool,
        app: &AppHandle,
        id: &str,
    ) -> Result<DownloadTask, String> {
        let row = self
            .get(pool, id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("task not found")?;
        if !matches!(
            row.status.as_str(),
            "pending" | "downloading" | "verifying" | "installing"
        ) {
            return Err("task cannot be paused in current state".into());
        }
        if let Some(control) = self.inner.controls.read().await.get(id) {
            control.store(PAUSE, Ordering::SeqCst)
        }
        self.persist(
            pool,
            id,
            "pausing",
            "pausing",
            row.progress,
            row.downloaded_bytes,
            row.total_bytes,
            "download.pausing",
            None,
            None,
        )
        .await
        .map_err(|e| e.to_string())?;
        self.emit(pool, app, id).await;
        Ok(self
            .get(pool, id)
            .await
            .map_err(|e| e.to_string())?
            .unwrap()
            .into())
    }
    pub async fn request_cancel(
        &self,
        pool: &SqlitePool,
        app: &AppHandle,
        id: &str,
    ) -> Result<DownloadTask, String> {
        let row = self
            .get(pool, id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("task not found")?;
        let has_running_control = if let Some(control) = self.inner.controls.read().await.get(id) {
            control.store(CANCEL, Ordering::SeqCst);
            true
        } else {
            false
        };
        if !has_running_control {
            if let Some(temp) = &row.temp_path {
                let _ = remove_path(Path::new(temp)).await;
            }
        }
        // Publish the terminal state immediately. Cleanup continues in the
        // running worker, but callers and the host must not remain stuck in a
        // downloading state while the HTTP stream waits for another chunk.
        self.persist(
            pool,
            id,
            "cancelled",
            "cancelled",
            0,
            0,
            row.total_bytes,
            "download.cancelled",
            None,
            None,
        )
        .await
        .map_err(|e| e.to_string())?;
        self.emit(pool, app, id).await;
        Ok(self
            .get(pool, id)
            .await
            .map_err(|e| e.to_string())?
            .unwrap()
            .into())
    }
    pub async fn resume(
        &self,
        pool: SqlitePool,
        app: AppHandle,
        id: &str,
    ) -> Result<DownloadTask, String> {
        let row = self
            .get(&pool, id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("task not found")?;
        if !matches!(row.status.as_str(), "paused" | "failed") {
            return Err("task cannot resume in current state".into());
        }
        sqlx::query("UPDATE package_download_tasks SET status='pending',phase='queued',message_key='download.queued',error_code=NULL,error_message=NULL,updated_at=datetime('now') WHERE id=?").bind(id).execute(&pool).await.map_err(|e|e.to_string())?;
        let row = self
            .get(&pool, id)
            .await
            .map_err(|e| e.to_string())?
            .unwrap();
        let task = row.clone().into();
        self.spawn(pool, app, row).await;
        Ok(task)
    }
    pub async fn remove(&self, pool: &SqlitePool, id: &str) -> Result<(), String> {
        let row = self
            .get(pool, id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("task not found")?;
        if !matches!(
            row.status.as_str(),
            "paused" | "success" | "failed" | "cancelled"
        ) {
            return Err("active task cannot be removed".into());
        }
        if let Some(temp) = row.temp_path {
            let _ = remove_path(Path::new(&temp)).await;
        }
        sqlx::query("DELETE FROM package_download_tasks WHERE id=?")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn parse_plan(payload: &serde_json::Value) -> Result<DownloadExecutionPlan, String> {
    serde_json::from_value(
        payload
            .get("executionPlan")
            .cloned()
            .unwrap_or_else(|| payload.clone()),
    )
    .map_err(|e| format!("invalid execution plan: {e}"))
}
fn validate_plan(plan: &DownloadExecutionPlan) -> Result<(), String> {
    let mut download = 0;
    let mut downloaded = false;
    let mut extracted = false;
    let mut cleaned = false;
    for item in &plan.instructions {
        if cleaned {
            return Err("cleanup must be last".into());
        }
        match item {
            DownloadInstruction::Download { .. } => {
                download += 1;
                if downloaded {
                    return Err("download must appear once".into());
                }
                downloaded = true
            }
            DownloadInstruction::VerifySha256 { .. } => {
                if !downloaded {
                    return Err("verify requires download".into());
                }
            }
            DownloadInstruction::ExtractZip { .. } => {
                if !downloaded {
                    return Err("extract requires download".into());
                }
                extracted = true
            }
            DownloadInstruction::BackupTarget { .. } => {
                if !downloaded {
                    return Err("backup requires download".into());
                }
            }
            DownloadInstruction::ReplaceTarget { source, .. } => {
                if !downloaded {
                    return Err("replace requires download".into());
                }
                if matches!(source, ReplaceSource::Staging) && !extracted {
                    return Err("staging replacement requires extraction".into());
                }
            }
            DownloadInstruction::OverlayTarget { source, .. } => {
                if !downloaded {
                    return Err("overlay requires download".into());
                }
                if matches!(source, ReplaceSource::Staging) && !extracted {
                    return Err("staging overlay requires extraction".into());
                }
            }
            DownloadInstruction::Cleanup { .. } => cleaned = true,
        }
    }
    if download != 1 {
        return Err("execution plan requires exactly one download instruction".into());
    }
    Ok(())
}
async fn remove_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let metadata = tokio::fs::metadata(path).await.map_err(|e| e.to_string())?;
    if metadata.is_dir() {
        tokio::fs::remove_dir_all(path).await
    } else {
        tokio::fs::remove_file(path).await
    }
    .map_err(|e| e.to_string())
}

fn overlay_copy_dir(source: &Path, target: &Path) -> Result<(), String> {
    std::fs::create_dir_all(target).map_err(|error| error.to_string())?;
    for entry in std::fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        let destination = target.join(entry.file_name());
        if file_type.is_dir() {
            overlay_copy_dir(&entry.path(), &destination)?;
        } else if file_type.is_file() {
            copy_file_with_retry(&entry.path(), &destination)?;
        } else {
            return Err(format!("unsupported overlay entry: {}", entry.path().display()));
        }
    }
    Ok(())
}

fn copy_file_with_retry(source: &Path, target: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut last_error = None;
    for attempt in 0..6 {
        match std::fs::copy(source, target) {
            Ok(_) => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                std::thread::sleep(std::time::Duration::from_millis(200 * (attempt + 1)));
            }
        }
    }
    Err(last_error
        .map(|error| error.to_string())
        .unwrap_or_else(|| "overlay copy failed".into()))
}
async fn sha256_file(path: PathBuf) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        use std::io::Read;
        let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count])
        }
        Ok::<_, String>(format!("{:x}", hasher.finalize()))
    })
    .await
    .map_err(|e| e.to_string())?
}
fn extract_zip_safe(archive: &Path, target: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(target).map_err(|e| e.to_string())?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|e| e.to_string())?;
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| format!("unsafe zip path: {}", entry.name()))?;
        let output = target.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?
            }
            let mut out = std::fs::File::create(output).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
fn contains_suffix(root: &Path, suffix: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(root) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        if path.is_dir() {
            contains_suffix(&path, suffix)
        } else {
            path.file_name().and_then(|v| v.to_str()).is_some_and(|v| {
                v.to_ascii_lowercase()
                    .ends_with(&suffix.to_ascii_lowercase())
            })
        }
    })
}
fn percent(downloaded: u64, total: Option<u64>) -> i64 {
    total
        .filter(|v| *v > 0)
        .map(|v| ((downloaded.saturating_mul(100) / v).min(99)) as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestInstallGuard(Arc<std::sync::Mutex<Vec<bool>>>);

    #[async_trait::async_trait]
    impl InstallGuard for TestInstallGuard {
        async fn finish(&mut self, success: bool) -> Result<(), String> {
            tokio::task::yield_now().await;
            self.0.lock().unwrap().push(success);
            Ok(())
        }
    }

    #[test]
    fn overlay_guard_finishes_before_returning() {
        tauri::async_runtime::block_on(async {
            for success in [true, false] {
                let events = Arc::new(std::sync::Mutex::new(Vec::new()));
                let result = guarded_overlay(Some(Box::new(TestInstallGuard(events.clone()))), &AtomicU8::new(RUNNING), async {
                    if success { Ok(()) } else { Err("copy failed".into()) }
                }).await;
                assert_eq!(result.is_ok(), success);
                assert_eq!(*events.lock().unwrap(), vec![success]);
            }
        });
    }

    #[test]
    fn cancelled_overlay_does_not_touch_files() {
        tauri::async_runtime::block_on(async {
            let events = Arc::new(std::sync::Mutex::new(Vec::new()));
            let copied = AtomicBool::new(false);
            let result = guarded_overlay(Some(Box::new(TestInstallGuard(events.clone()))), &AtomicU8::new(CANCEL), async {
                copied.store(true, Ordering::SeqCst);
                Ok(())
            }).await;
            assert_eq!(result.unwrap_err(), "TASK_CANCELLED");
            assert!(!copied.load(Ordering::SeqCst));
            assert_eq!(*events.lock().unwrap(), vec![false]);
        });
    }

    fn download() -> DownloadInstruction {
        DownloadInstruction::Download {
            url: "https://example.com/package.zip".into(),
            temp_path: "package.part".into(),
            resume: true,
        }
    }

    #[test]
    fn replacement_does_not_require_backup() {
        let plan = DownloadExecutionPlan {
            instructions: vec![
                download(),
                DownloadInstruction::ReplaceTarget {
                    source: ReplaceSource::Temp,
                    target_path: "package.bin".into(),
                },
            ],
        };
        assert!(validate_plan(&plan).is_ok());
    }

    #[test]
    fn staging_replacement_requires_extraction() {
        let plan = DownloadExecutionPlan {
            instructions: vec![
                download(),
                DownloadInstruction::ReplaceTarget {
                    source: ReplaceSource::Staging,
                    target_path: "package".into(),
                },
            ],
        };
        assert_eq!(
            validate_plan(&plan).unwrap_err(),
            "staging replacement requires extraction"
        );
    }

    #[test]
    fn fastkg_overlay_plan_is_supported() {
        let payload = serde_json::json!({
            "executionPlan": { "instructions": [
                { "type": "download", "url": "https://example.com/fastkg.zip", "tempPath": "fastkg.part", "resume": true },
                { "type": "extractZip", "stagingPath": "fastkg.installing", "requiredFileSuffix": ".exe" },
                {
                    "type": "overlayTarget",
                    "source": "staging",
                    "targetPath": "FastKGService",
                    "stopProcessNames": ["FastKG", "qdrant", "llama-server"]
                },
                { "type": "cleanup", "targets": ["temp", "staging"] }
            ]}
        });
        let plan = parse_plan(&payload).expect("overlay plan should deserialize");
        assert!(validate_plan(&plan).is_ok());
    }

    #[test]
    fn overlay_replaces_package_files_and_preserves_local_data() {
        let root = std::env::temp_dir().join(format!(
            "download-center-overlay-test-{}",
            uuid::Uuid::new_v4()
        ));
        let source = root.join("source");
        let target = root.join("target");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(target.join("storage")).unwrap();
        std::fs::write(source.join("FastKG.exe"), b"new").unwrap();
        std::fs::write(target.join("FastKG.exe"), b"old").unwrap();
        std::fs::write(target.join("storage").join("data.db"), b"local").unwrap();

        overlay_copy_dir(&source, &target).unwrap();

        assert_eq!(std::fs::read(target.join("FastKG.exe")).unwrap(), b"new");
        assert_eq!(
            std::fs::read(target.join("storage").join("data.db")).unwrap(),
            b"local"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cleanup_is_the_last_instruction() {
        let plan = DownloadExecutionPlan {
            instructions: vec![
                download(),
                DownloadInstruction::Cleanup {
                    targets: vec![CleanupTarget::Temp],
                },
                DownloadInstruction::VerifySha256 {
                    sha256: "00".repeat(32),
                },
            ],
        };
        assert_eq!(validate_plan(&plan).unwrap_err(), "cleanup must be last");
    }

    #[test]
    fn managers_keep_independent_configuration_and_controls() {
        let first = DownloadManager::new(DownloadManagerConfig {
            max_concurrency: 1,
            progress_emit_interval_ms: 50,
        });
        let second = DownloadManager::new(DownloadManagerConfig {
            max_concurrency: 4,
            progress_emit_interval_ms: 500,
        });
        assert!(!Arc::ptr_eq(&first.inner, &second.inner));
        assert_eq!(first.inner.semaphore.available_permits(), 1);
        assert_eq!(second.inner.semaphore.available_permits(), 4);
        assert_eq!(first.inner.config.progress_emit_interval_ms, 50);
        assert_eq!(second.inner.config.progress_emit_interval_ms, 500);
    }
}
