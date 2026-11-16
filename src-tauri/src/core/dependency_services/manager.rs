//! 应用级依赖服务恢复调度。只保存用户意图和重试信息，实际进程事实来自监督器。
use super::recovery::*;
use crate::{commands::wsp, core::error::AppError, state::AppState};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
use tokio::sync::Notify;

#[derive(Clone, Copy, PartialEq, Eq)]
struct ObservationStamp {
    generation: u64,
    phase: RecoveryPhase,
    attempt: u32,
}

impl ObservationStamp {
    fn of(intent: &RecoveryIntent) -> Self {
        Self {
            generation: intent.generation,
            phase: intent.phase,
            attempt: intent.attempts,
        }
    }

    fn accepts(self, intent: &RecoveryIntent) -> bool {
        self == Self::of(intent)
            && intent.desired == DesiredState::Running
            && !intent.session_paused
            && self.phase != RecoveryPhase::Starting
    }
}

struct Observation {
    sampled_at: Instant,
    stamps: BTreeMap<String, ObservationStamp>,
    services: Vec<wsp::DepServiceStatus>,
}

async fn wait_for_active_task(
    task: &mut Option<tokio::task::JoinHandle<()>>,
) -> Result<(), tokio::task::JoinError> {
    match task.as_mut() {
        Some(task) => task.await,
        None => std::future::pending().await,
    }
}

fn upstream_is_ready(intent: &RecoveryIntent, observed: Option<&(ObservationStamp, bool)>) -> bool {
    intent.phase == RecoveryPhase::Healthy
        || observed
            .is_some_and(|(stamp, running)| *running && *stamp == ObservationStamp::of(intent))
}

#[derive(Debug, Clone, Serialize)]
pub struct RecoveryStatus {
    pub id: String,
    pub desired_state: DesiredState,
    pub recovery_state: RecoveryPhase,
    pub generation: u64,
    pub recovery_attempt: u32,
    pub max_recovery_attempts: u32,
    pub retry_in_ms: Option<u64>,
    pub failure_code: Option<FailureCode>,
    pub last_error: Option<String>,
    pub enabled: bool,
    pub global_enabled: bool,
    pub service_enabled: bool,
    pub auto_start: bool,
    pub can_retry: bool,
}

pub struct DependencyRecoveryManager {
    intents: Mutex<BTreeMap<String, RecoveryIntent>>,
    pub operation: Arc<tokio::sync::Mutex<()>>,
    kg_installing: AtomicBool,
    wake: Notify,
    enabled: AtomicBool,
    shutdown: AtomicBool,
    started: AtomicBool,
    pub fastplan_resume: Mutex<Option<crate::commands::fastplan_connection::FastPlanResumeData>>,
    fastplan_resume_loaded: AtomicBool,
    last_events: Mutex<BTreeMap<String, String>>,
}

impl Default for DependencyRecoveryManager {
    fn default() -> Self {
        Self {
            intents: Mutex::new(
                SERVICE_IDS
                    .into_iter()
                    .map(|id| (id.into(), RecoveryIntent::default()))
                    .collect(),
            ),
            operation: Arc::new(tokio::sync::Mutex::new(())),
            kg_installing: AtomicBool::new(false),
            wake: Notify::new(),
            enabled: AtomicBool::new(true),
            shutdown: AtomicBool::new(false),
            started: AtomicBool::new(false),
            fastplan_resume: Mutex::new(None),
            fastplan_resume_loaded: AtomicBool::new(false),
            last_events: Mutex::new(BTreeMap::new()),
        }
    }
}

impl DependencyRecoveryManager {
    pub fn is_shutting_down(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }
    pub fn kg_installing(&self) -> bool {
        self.kg_installing.load(Ordering::SeqCst)
    }

    pub fn resume_generation(&self, app: &tauri::AppHandle, id: &str, generation: u64) {
        let mut intents = self.intents.lock().unwrap();
        if !self.shutdown.load(Ordering::SeqCst) {
            if let Some(intent) = intents
                .get_mut(id)
                .filter(|intent| intent.generation == generation)
            {
                intent.request_start(Instant::now(), true);
            }
        }
        drop(intents);
        self.publish(app);
        self.wake.notify_one();
    }
    pub fn begin_kg_install(&self, app: &tauri::AppHandle) -> Result<Vec<(String, u64)>, AppError> {
        if self.is_shutting_down() {
            return Err(AppError::Business("应用正在退出，已取消 KG 安装".into()));
        }
        if self.kg_installing.swap(true, Ordering::SeqCst) {
            return Err(AppError::Business("KG 已在安装中".into()));
        }
        for id in ["qdrant", "llama_server", "fastkg"] {
            self.request_stop(app, id)?;
        }
        Ok(self
            .snapshots()
            .into_iter()
            .filter(|s| ["qdrant", "llama_server", "fastkg"].contains(&s.id.as_str()))
            .map(|s| (s.id, s.generation))
            .collect())
    }

    pub fn finish_kg_install(
        &self,
        app: &tauri::AppHandle,
        generations: &[(String, u64)],
        success: bool,
    ) {
        let mut intents = self.intents.lock().unwrap();
        for (id, generation) in generations {
            let intent = intents.get_mut(id).unwrap();
            if intent.generation != *generation || self.shutdown.load(Ordering::SeqCst) {
                continue;
            }
            if success {
                intent.request_start(Instant::now(), true);
            } else {
                intent.desired = DesiredState::Running;
                intent.fail(
                    *generation,
                    Instant::now(),
                    FailureCode::InvalidConfiguration,
                    "安装未完成，自动恢复已暂停；请重新安装后重试".into(),
                    false,
                );
            }
        }
        drop(intents);
        self.kg_installing.store(false, Ordering::SeqCst);
        self.publish(app);
        self.wake.notify_one();
    }

    pub fn snapshots(&self) -> Vec<RecoveryStatus> {
        let now = Instant::now();
        self.intents
            .lock()
            .unwrap()
            .iter()
            .map(|(id, intent)| RecoveryStatus {
                id: id.clone(),
                desired_state: intent.desired,
                recovery_state: intent.phase,
                generation: intent.generation,
                recovery_attempt: intent.attempts,
                max_recovery_attempts: if intent.ever_ready { 5 } else { 3 },
                retry_in_ms: intent
                    .retry_at
                    .map(|at| at.saturating_duration_since(now).as_millis() as u64),
                failure_code: intent.failure_code,
                last_error: intent.last_error.clone(),
                enabled: self.enabled.load(Ordering::SeqCst) && intent.settings.enabled,
                global_enabled: self.enabled.load(Ordering::SeqCst),
                service_enabled: intent.settings.enabled,
                auto_start: intent.settings.auto_start,
                can_retry: matches!(
                    intent.phase,
                    RecoveryPhase::Failed
                        | RecoveryPhase::Paused
                        | RecoveryPhase::Backoff
                        | RecoveryPhase::WaitingSharedOwner
                ),
            })
            .collect()
    }

    fn publish(&self, app: &tauri::AppHandle) {
        let snapshots = self.snapshots();
        let mut changed = Vec::new();
        {
            let mut previous = self.last_events.lock().unwrap();
            for status in &snapshots {
                let key = format!(
                    "{}:{:?}:{}:{:?}",
                    status.generation,
                    status.recovery_state,
                    status.recovery_attempt,
                    status.failure_code
                );
                if previous.get(&status.id) != Some(&key) {
                    previous.insert(status.id.clone(), key);
                    changed.push(status.clone());
                }
            }
        }
        let _ = app.emit("dependency-recovery-state", snapshots);
        if !changed.is_empty() {
            let app = app.clone();
            // 重试/暂停来自同步 IPC，调用线程不一定进入 Tokio runtime。
            // 使用 Tauri 全局运行时，避免 tokio::spawn 在主线程 panic 导致应用退出。
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>();
                let pool = state.db.pool().await;
                for status in changed {
                    let phase = serde_json::to_value(status.recovery_state).unwrap_or_default();
                    if let Err(error) = sqlx::query("INSERT INTO dependency_recovery_events(service_id,generation,phase,attempt,detail) VALUES (?,?,?,?,?)")
                        .bind(status.id).bind(status.generation as i64).bind(phase.as_str().unwrap_or("unknown"))
                        .bind(status.recovery_attempt).bind(status.last_error).execute(&*pool).await {
                        tracing::warn!("保存依赖服务恢复事件失败: {error}");
                    }
                }
                let _ = sqlx::query("DELETE FROM dependency_recovery_events WHERE id NOT IN (SELECT id FROM dependency_recovery_events ORDER BY id DESC LIMIT 1000)").execute(&*pool).await;
            });
        }
    }

    pub fn is_current(&self, id: &str, generation: u64) -> bool {
        !self.shutdown.load(Ordering::SeqCst)
            && self.intents.lock().unwrap().get(id).is_some_and(|intent| {
                intent.generation == generation
                    && intent.desired == DesiredState::Running
                    && !intent.session_paused
            })
    }

    pub fn load_fastplan_resume(
        &self,
        data: Option<crate::commands::fastplan_connection::FastPlanResumeData>,
    ) {
        *self.fastplan_resume.lock().unwrap() = data;
        self.fastplan_resume_loaded.store(true, Ordering::SeqCst);
        self.wake.notify_one();
    }

    pub fn upstream_changed(&self) {
        let mut intents = self.intents.lock().unwrap();
        let kg = intents.get_mut("fastkg").unwrap();
        if kg.desired == DesiredState::Running
            && !kg.session_paused
            && kg.phase == RecoveryPhase::Healthy
        {
            kg.phase = RecoveryPhase::WaitingDependency;
            kg.retry_at = None;
            kg.last_error = Some("上游服务恢复后将同步端点并重启知识库服务".into());
        }
    }

    pub fn request(
        &self,
        app: &tauri::AppHandle,
        id: &str,
        include_dependencies: bool,
        retry: bool,
        explicit: bool,
    ) -> Result<String, AppError> {
        if self.kg_installing.load(Ordering::SeqCst)
            && matches!(id, "qdrant" | "llama_server" | "fastkg")
        {
            return Err(AppError::Business(
                "KG 正在覆盖安装文件，请等待安装完成".into(),
            ));
        }
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(AppError::Business("应用正在退出".into()));
        }
        let mut intents = self.intents.lock().unwrap();
        if !intents.contains_key(id) {
            return Err(AppError::InvalidInput(format!("未知依赖服务: {}", id)));
        }
        let targets: Vec<&str> = if id == "fastkg" && include_dependencies {
            vec!["qdrant", "llama_server", "fastkg"]
        } else {
            vec![id]
        };
        // 会话内用户停止过的服务，业务请求不能重新拉起。
        if !explicit
            && targets.iter().any(|target| {
                let intent = &intents[*target];
                (intent.desired == DesiredState::Stopped && intent.generation > 0)
                    || intent.session_paused
            })
        {
            return Err(AppError::Business(
                "依赖服务已由用户停止或暂停，请在依赖服务页面手动启动".into(),
            ));
        }
        for target in targets {
            let intent = intents.get_mut(target).unwrap();
            if !explicit && intent.desired == DesiredState::Running {
                if matches!(intent.phase, RecoveryPhase::Failed | RecoveryPhase::Paused) {
                    return Err(AppError::Business(
                        intent
                            .last_error
                            .clone()
                            .unwrap_or_else(|| "自动恢复已暂停，请手动重试".into()),
                    ));
                }
                continue;
            }
            intent.request_start(Instant::now(), retry);
        }
        drop(intents);
        self.publish(app);
        self.wake.notify_one();
        Ok("服务启动请求已提交，正在等待健康验证".into())
    }

    pub fn request_stop(&self, app: &tauri::AppHandle, id: &str) -> Result<(), AppError> {
        let mut intents = self.intents.lock().unwrap();
        let intent = intents
            .get_mut(id)
            .ok_or_else(|| AppError::InvalidInput(format!("未知依赖服务: {}", id)))?;
        intent.stop();
        wsp::signal_dependency_stop(id);
        if matches!(id, "qdrant" | "llama_server") {
            intents.get_mut("fastkg").unwrap().stop();
            wsp::signal_dependency_stop("fastkg");
        }
        drop(intents);
        self.publish(app);
        self.wake.notify_one();
        Ok(())
    }

    pub fn pause(&self, app: &tauri::AppHandle, id: &str) -> Result<(), AppError> {
        let mut intents = self.intents.lock().unwrap();
        intents
            .get_mut(id)
            .ok_or_else(|| AppError::InvalidInput(format!("未知依赖服务: {}", id)))?
            .pause();
        wsp::signal_dependency_stop(id);
        drop(intents);
        self.publish(app);
        self.wake.notify_one();
        Ok(())
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        for (id, intent) in self.intents.lock().unwrap().iter_mut() {
            intent.stop();
            wsp::signal_dependency_stop(id);
        }
        self.wake.notify_one();
    }

    pub async fn start(self: &Arc<Self>, app: tauri::AppHandle) {
        if self.started.swap(true, Ordering::SeqCst) {
            return;
        }
        match app.path().app_data_dir() {
            Ok(directory) => {
                if let Err(error) = super::dependency_service_supervisor()
                    .owned
                    .configure(directory.join("dependency-runtime"))
                {
                    tracing::error!("无法初始化依赖服务进程凭证目录: {error}");
                    return;
                }
            }
            Err(error) => {
                tracing::error!("无法定位依赖服务进程凭证目录: {error}");
                return;
            }
        }
        let state = app.state::<AppState>();
        let pool = state.db.pool().await;
        let entries = sqlx::query_as::<_, (String, String)>("SELECT key, value FROM settings")
            .fetch_all(&*pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        self.enabled.store(
            entries
                .get("dependency_recovery_enabled")
                .map(|v| v != "false")
                .unwrap_or(true),
            Ordering::SeqCst,
        );
        {
            let mut intents = self.intents.lock().unwrap();
            for id in SERVICE_IDS {
                let legacy = match id {
                    "fastplan" => "fastplan_service_auto_start",
                    _ => "kg_service_downloaded",
                };
                let auto_start = entries
                    .get(&format!("dependency_{}_auto_start", id))
                    .or_else(|| entries.get(legacy))
                    .map(|value| value == "true")
                    .unwrap_or(id == "fastplan");
                let intent = intents.get_mut(id).unwrap();
                intent.settings.auto_start = auto_start;
                intent.settings.enabled = entries
                    .get(&format!("dependency_{}_recovery_enabled", id))
                    .map(|v| v != "false")
                    .unwrap_or(true);
                if auto_start && intent.generation == 0 {
                    intent.request_start(Instant::now(), false);
                }
            }
        }
        drop(pool);
        drop(state);
        let manager = self.clone();
        tokio::spawn(async move {
            manager.run(app).await;
        });
    }

    async fn observe(
        self: Arc<Self>,
        app: tauri::AppHandle,
        sender: tokio::sync::mpsc::Sender<Observation>,
    ) {
        loop {
            if self.is_shutting_down() {
                break;
            }
            let sampled_at = Instant::now();
            let (stamps, idle) = {
                let intents = self.intents.lock().unwrap();
                let stamps = intents
                    .iter()
                    .map(|(id, intent)| (id.clone(), ObservationStamp::of(intent)))
                    .collect();
                let idle = !intents.values().any(|intent| {
                    matches!(
                        intent.phase,
                        RecoveryPhase::Starting | RecoveryPhase::Queued
                    )
                });
                (stamps, idle)
            };
            // 孤儿接管/共享租约的写操作只在空闲时尝试，不抢占已入队的启动。
            // 启动执行器自身仍会完成全部身份、共享所有权和健康校验。
            if idle {
                if let Ok(_operation) = self.operation.try_lock() {
                    let (still_idle, kg_wanted) = {
                        let intents = self.intents.lock().unwrap();
                        (
                            !intents.values().any(|intent| {
                                matches!(
                                    intent.phase,
                                    RecoveryPhase::Starting | RecoveryPhase::Queued
                                )
                            }),
                            intents["fastkg"].desired == DesiredState::Running
                                && !intents["fastkg"].session_paused,
                        )
                    };
                    if still_idle && kg_wanted {
                        if let Err(error) = wsp::reconcile_dependency_shared_runtime(&app).await {
                            tracing::debug!("[dependency-recovery] 共享状态暂不可用: {error}");
                        }
                    }
                    if still_idle {
                        if let Err(error) = wsp::reconcile_dependency_processes(&app).await {
                            tracing::debug!("依赖服务进程状态暂不可用: {error}");
                        }
                    }
                }
            }
            match wsp::get_dependency_services_status(app.state::<AppState>()).await {
                Ok(services) => {
                    if sender
                        .send(Observation {
                            sampled_at,
                            stamps,
                            services,
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(error) => tracing::warn!("[dependency-recovery] 状态采集失败: {error}"),
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    async fn run(self: Arc<Self>, app: tauri::AppHandle) {
        let (sender, mut observations) = tokio::sync::mpsc::channel(1);
        let observer = tokio::spawn(self.clone().observe(app.clone(), sender));
        let mut observation: Option<Observation> = None;
        let mut observed_upstreams = BTreeMap::new();
        let mut last_tick = Instant::now();
        let mut grace_until = last_tick;
        let mut last_health = last_tick;
        let mut active_task: Option<tokio::task::JoinHandle<()>> = None;
        let mut completed_task: Option<Result<(), tokio::task::JoinError>> = None;
        let mut observer_open = true;
        let mut active_identity: Option<(&'static str, u64)> = None;
        loop {
            if self.shutdown.load(Ordering::SeqCst) {
                observer.abort();
                let _ = observer.await;
                if let Some(task) = active_task.take() {
                    if completed_task.is_none() {
                        let _ = task.await;
                    }
                }
                break;
            }
            if completed_task.is_some()
                || active_task.as_ref().is_some_and(|task| task.is_finished())
            {
                if let Some(task) = active_task.take() {
                    let result = match completed_task.take() {
                        Some(result) => result,
                        None => task.await,
                    };
                    if let Err(error) = result {
                        tracing::error!("恢复任务异常退出: {error}");
                        if let Some((id, generation)) = active_identity {
                            wsp::signal_dependency_stop(id);
                            let message =
                                "恢复任务异常中断，已暂停；请检查恢复记录后重试".to_string();
                            self.intents.lock().unwrap().get_mut(id).unwrap().fail(
                                generation,
                                Instant::now(),
                                FailureCode::OwnershipUnverified,
                                message.clone(),
                                false,
                            );
                            let _ = app.emit(
                                "dependency-recovery-failed",
                                serde_json::json!({"serviceId":id,"message":message}),
                            );
                        }
                    }
                    active_identity = None;
                }
            }
            let now = Instant::now();
            if now.saturating_duration_since(last_tick) > Duration::from_secs(30) {
                grace_until = now + WAKE_GRACE;
            }
            last_tick = now;
            let mut selected = None;
            let mut failures = Vec::new();
            {
                let mut intents = self.intents.lock().unwrap();
                if let Some(sample) = observation.take() {
                    let health_due = sample.sampled_at.saturating_duration_since(last_health)
                        >= Duration::from_secs(10);
                    if health_due {
                        last_health = sample.sampled_at;
                    }
                    for status in sample.services {
                        let Some(intent) = intents.get_mut(&status.id) else {
                            continue;
                        };
                        // 自动启动关闭不代表共享上游未运行；保留原有的共享就绪判断。
                        // 用户停止/重试改变 stamp 后，这份历史事实立即失效。
                        if matches!(status.id.as_str(), "qdrant" | "llama_server") {
                            if let Some(stamp) = sample.stamps.get(&status.id) {
                                observed_upstreams
                                    .insert(status.id.clone(), (*stamp, status.running));
                            }
                        }
                        if !sample
                            .stamps
                            .get(&status.id)
                            .is_some_and(|stamp| stamp.accepts(intent))
                        {
                            continue;
                        }
                        if status.id == "fastplan"
                            && !self.fastplan_resume_loaded.load(Ordering::SeqCst)
                        {
                            continue;
                        }
                        if status.running {
                            intent.ready(intent.generation, now);
                        } else if !status.installed {
                            if intent.phase != RecoveryPhase::Failed {
                                intent.fail(
                                    intent.generation,
                                    now,
                                    FailureCode::NotInstalled,
                                    "服务文件未安装，请下载或离线安装".into(),
                                    false,
                                );
                                failures.push((
                                    status.id,
                                    intent.last_error.clone().unwrap_or_default(),
                                ));
                            }
                        } else if intent.phase == RecoveryPhase::Healthy
                            && sample.sampled_at >= grace_until
                            && health_due
                        {
                            intent.consecutive_health_failures += 1;
                            if intent.consecutive_health_failures >= 3 {
                                intent.fail(
                                    intent.generation,
                                    now,
                                    FailureCode::HealthTimeout,
                                    status.status_message.clone(),
                                    self.enabled.load(Ordering::SeqCst),
                                );
                                if matches!(
                                    intent.phase,
                                    RecoveryPhase::Failed | RecoveryPhase::Paused
                                ) {
                                    failures.push((status.id, status.status_message));
                                }
                            }
                        }
                    }
                }
                // 无需等待观察者 HTTP 返回即可启动；前置服务就绪由执行器发布，
                // 真正启动 FastKG 前仍再次验证上游进程身份和健康状态。
                for id in SERVICE_IDS {
                    let dependencies_ready = id != "fastkg"
                        || ["qdrant", "llama_server"].iter().all(|dependency| {
                            upstream_is_ready(
                                &intents[*dependency],
                                observed_upstreams.get(*dependency),
                            )
                        });
                    let intent = intents.get_mut(id).unwrap();
                    if intent.desired != DesiredState::Running
                        || intent.session_paused
                        || matches!(
                            intent.phase,
                            RecoveryPhase::Starting | RecoveryPhase::Healthy
                        )
                    {
                        continue;
                    }
                    if id == "fastplan" && !self.fastplan_resume_loaded.load(Ordering::SeqCst) {
                        continue;
                    }
                    if !dependencies_ready {
                        if matches!(intent.phase, RecoveryPhase::Queued | RecoveryPhase::Backoff) {
                            intent.phase = RecoveryPhase::WaitingDependency;
                            intent.retry_at = None;
                        }
                        continue;
                    }
                    if intent.phase == RecoveryPhase::WaitingDependency {
                        intent.phase = RecoveryPhase::Queued;
                        intent.retry_at = Some(now);
                    }
                    if active_task.is_none() && now >= grace_until && intent.can_attempt(now) {
                        let due = intent.retry_at.unwrap_or(now);
                        if selected
                            .as_ref()
                            .is_none_or(|(_, _, _, selected_due)| due < *selected_due)
                        {
                            selected = Some((id, intent.generation, !intent.ever_ready, due));
                        }
                    }
                }
                if let Some((id, _, _, _)) = selected {
                    if intents.get_mut(id).unwrap().begin_attempt(now).is_none() {
                        let intent = &intents[id];
                        if matches!(intent.phase, RecoveryPhase::Failed | RecoveryPhase::Paused) {
                            failures.push((
                                id.to_string(),
                                intent.last_error.clone().unwrap_or_else(|| {
                                    "已达到自动恢复次数上限，请检查后手动重试".into()
                                }),
                            ));
                        }
                        selected = None;
                    }
                }
            }
            self.publish(&app);
            for (id, message) in failures {
                let _ = app.emit(
                    "dependency-recovery-failed",
                    serde_json::json!({"serviceId": id, "message": message}),
                );
            }
            if let Some((id, generation, cold, due)) = selected {
                tracing::info!(
                    "[dependency-startup] {id} 调度启动 generation={generation}, queue_wait_ms={}",
                    now.saturating_duration_since(due).as_millis()
                );
                active_identity = Some((id, generation));
                let manager = self.clone();
                let task_app = app.clone();
                active_task = Some(tokio::spawn(async move {
                    let dispatched_at = Instant::now();
                    let app = task_app;
                    let _operation = manager.operation.lock().await;
                    let active = {
                        let intents = manager.intents.lock().unwrap();
                        let intent = &intents[id];
                        !manager.shutdown.load(Ordering::SeqCst)
                            && intent.generation == generation
                            && intent.desired == DesiredState::Running
                            && !intent.session_paused
                    };
                    if !active {
                        manager.wake.notify_one();
                        return;
                    }
                    tracing::info!(
                        "[dependency-startup] {id} 开始执行 lock_wait_ms={}",
                        dispatched_at.elapsed().as_millis()
                    );
                    let started_at = Instant::now();
                    let result = wsp::recover_dependency_once(
                        &app,
                        id,
                        generation,
                        startup_timeout(id, cold),
                    )
                    .await;
                    tracing::info!(
                        "[dependency-startup] {id} 本次启动结束 success={}, elapsed_ms={}",
                        result.is_ok(),
                        started_at.elapsed().as_millis()
                    );
                    let mut terminal = None;
                    {
                        let mut intents = manager.intents.lock().unwrap();
                        let intent = intents.get_mut(id).unwrap();
                        match result {
                            Ok(()) => {
                                intent.ready(generation, Instant::now());
                            }
                            Err((code, message)) => {
                                if intent.fail(
                                    generation,
                                    Instant::now(),
                                    code,
                                    message.clone(),
                                    manager.enabled.load(Ordering::SeqCst),
                                ) && matches!(
                                    intent.phase,
                                    RecoveryPhase::Failed | RecoveryPhase::Paused
                                ) {
                                    terminal = Some(message);
                                }
                            }
                        }
                    }
                    manager.publish(&app);
                    if let Some(message) = terminal {
                        let _ = app.emit(
                            "dependency-recovery-failed",
                            serde_json::json!({"serviceId": id, "message": message}),
                        );
                        if id == "fastkg" {
                            let _ = app.emit(
                                "kg-service-status",
                                serde_json::json!({"phase":"error", "message":message}),
                            );
                        }
                    }
                    manager.wake.notify_one();
                }));
            }
            let sleep = if active_task.is_some() {
                Duration::from_secs(5)
            } else {
                self.intents
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|(id, _)| {
                        id.as_str() != "fastplan"
                            || self.fastplan_resume_loaded.load(Ordering::SeqCst)
                    })
                    .filter_map(|(_, intent)| intent.retry_at)
                    .map(|at| at.saturating_duration_since(Instant::now()))
                    .min()
                    .unwrap_or(Duration::from_secs(5))
                    .clamp(Duration::from_millis(100), Duration::from_secs(5))
            };
            tokio::select! {
                _ = self.wake.notified() => {},
                sample = observations.recv(), if observer_open => {
                    observer_open = sample.is_some();
                    observation = sample;
                    if !observer_open { tracing::error!("依赖服务健康观察任务已退出"); }
                },
                // 直接等待 JoinHandle 完成，避免 notify 先于任务结束时再多等一个轮询周期。
                result = wait_for_active_task(&mut active_task) => { completed_task = Some(result); },
                _ = tokio::time::sleep(sleep) => {},
            }
        }
    }
}

#[cfg(test)]
mod dependency_startup_tests {
    use super::*;

    #[test]
    fn verified_shared_upstream_can_be_used_without_auto_start_but_not_after_stop() {
        let mut intent = RecoveryIntent::default();
        intent.settings.auto_start = false;
        let observed = (ObservationStamp::of(&intent), true);
        assert!(upstream_is_ready(&intent, Some(&observed)));
        assert!(!upstream_is_ready(&intent, None));
        intent.stop();
        assert!(!upstream_is_ready(&intent, Some(&observed)));
        let now = Instant::now();
        intent.request_start(now, true);
        intent.ready(intent.generation, now);
        assert!(upstream_is_ready(&intent, None));
    }

    #[test]
    fn stale_observations_cannot_override_stop_retry_or_start_completion() {
        let now = Instant::now();
        let mut intent = RecoveryIntent::default();
        intent.request_start(now, false);
        let queued = ObservationStamp::of(&intent);
        assert!(queued.accepts(&intent));
        intent.begin_attempt(now);
        assert!(!queued.accepts(&intent));
        let starting = ObservationStamp::of(&intent);
        assert!(!starting.accepts(&intent));
        intent.ready(intent.generation, now);
        assert!(!starting.accepts(&intent));
        let healthy = ObservationStamp::of(&intent);
        assert!(healthy.accepts(&intent));
        intent.stop();
        assert!(!healthy.accepts(&intent));
        intent.request_start(now, true);
        assert!(!healthy.accepts(&intent));
    }

    #[test]
    fn same_generation_retry_and_pause_reject_old_health_samples() {
        let now = Instant::now();
        let mut intent = RecoveryIntent::default();
        intent.request_start(now, false);
        intent.ready(intent.generation, now);
        let previous = ObservationStamp::of(&intent);
        intent.attempts += 1;
        assert!(!previous.accepts(&intent));
        let current = ObservationStamp::of(&intent);
        intent.session_paused = true;
        assert!(!current.accepts(&intent));
    }

    #[tokio::test]
    async fn completed_start_wakes_scheduler_without_a_health_response_or_poll_tick() {
        let (_sender, mut receiver) = tokio::sync::mpsc::channel::<Observation>(1);
        let mut active = Some(tokio::spawn(async {}));
        let completed = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::select! {
                result = wait_for_active_task(&mut active) => result,
                _ = receiver.recv() => panic!("observation was not sent"),
                _ = tokio::time::sleep(Duration::from_secs(5)) => panic!("waited for poll tick"),
            }
        })
        .await
        .expect("startup completion must wake the scheduler");
        assert!(completed.is_ok());
        // 与调度器一致：已获取结果的句柄只能丢弃，不能再次 await。
        active.take();
        assert!(
            tokio::time::timeout(Duration::from_millis(10), wait_for_active_task(&mut active))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn panicked_start_returns_an_error_instead_of_stalling_the_queue() {
        let mut active = Some(tokio::spawn(async { panic!("simulated startup panic") }));
        assert!(wait_for_active_task(&mut active)
            .await
            .unwrap_err()
            .is_panic());
        active.take();
    }
}

#[tauri::command]
pub fn get_dependency_recovery_status(state: tauri::State<'_, AppState>) -> Vec<RecoveryStatus> {
    state.dependency_recovery.snapshots()
}

#[tauri::command]
pub fn get_dependency_diagnostics(service_id: String) -> Vec<String> {
    super::dependency_service_supervisor()
        .owned
        .diagnostics(&service_id)
}

#[derive(Serialize, sqlx::FromRow)]
pub struct RecoveryEvent {
    service_id: String,
    phase: String,
    attempt: i64,
    detail: Option<String>,
    created_at: String,
}

#[tauri::command]
pub async fn get_dependency_recovery_history(
    state: tauri::State<'_, AppState>,
    service_id: String,
) -> Result<Vec<RecoveryEvent>, AppError> {
    let pool = state.db.pool().await;
    Ok(sqlx::query_as::<_, RecoveryEvent>("SELECT service_id,phase,attempt,detail,created_at FROM dependency_recovery_events WHERE service_id=? ORDER BY id DESC LIMIT 50")
        .bind(service_id).fetch_all(&*pool).await?)
}

#[tauri::command]
pub fn retry_dependency_service(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    service_id: String,
) -> Result<String, AppError> {
    state
        .dependency_recovery
        .request(&app, &service_id, service_id == "fastkg", true, true)
}

#[tauri::command]
pub fn pause_dependency_recovery(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    service_id: String,
) -> Result<(), AppError> {
    state.dependency_recovery.pause(&app, &service_id)
}

#[tauri::command]
pub async fn set_dependency_recovery_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    service_id: Option<String>,
    enabled: Option<bool>,
    auto_start: Option<bool>,
) -> Result<(), AppError> {
    if service_id
        .as_ref()
        .is_some_and(|id| !SERVICE_IDS.contains(&id.as_str()))
    {
        return Err(AppError::InvalidInput("未知依赖服务".into()));
    }
    if service_id.is_none() && auto_start.is_some() {
        return Err(AppError::InvalidInput("自动启动配置需要指定服务".into()));
    }
    let pool = state.db.pool().await;
    let mut transaction = pool.begin().await?;
    for (key, value) in [
        (
            service_id
                .as_ref()
                .map(|id| format!("dependency_{}_recovery_enabled", id))
                .unwrap_or_else(|| "dependency_recovery_enabled".into()),
            enabled,
        ),
        (
            format!(
                "dependency_{}_auto_start",
                service_id.as_deref().unwrap_or("")
            ),
            auto_start,
        ),
    ] {
        if let Some(value) = value {
            sqlx::query("INSERT INTO settings (key,value,category,updated_at) VALUES (?,?,'general',datetime('now')) ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at=excluded.updated_at")
                .bind(key).bind(value.to_string()).execute(&mut *transaction).await?;
        }
    }
    transaction.commit().await?;
    let manager = &state.dependency_recovery;
    if let Some(id) = service_id {
        let mut intents = manager.intents.lock().unwrap();
        let intent = intents.get_mut(&id).unwrap();
        if let Some(enabled) = enabled {
            intent.settings.enabled = enabled;
            if !enabled {
                intent.disable_pending_retry();
            }
        }
        if let Some(auto_start) = auto_start {
            intent.settings.auto_start = auto_start;
        }
    } else if let Some(enabled) = enabled {
        manager.enabled.store(enabled, Ordering::SeqCst);
        if !enabled {
            for intent in manager.intents.lock().unwrap().values_mut() {
                intent.disable_pending_retry();
            }
        }
    }
    manager.publish(&app);
    manager.wake.notify_one();
    Ok(())
}
