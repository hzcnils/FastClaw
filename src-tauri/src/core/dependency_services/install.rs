use crate::{commands::wsp, kg_coordination, state::AppState, utils::paths};
use tauri::{AppHandle, Manager};
use tauri_plugin_download_center::{
    manager::{InstallGuard, InstallHook},
    model::DownloadTaskRow,
};

pub struct KgInstallHook;

#[async_trait::async_trait]
impl InstallHook for KgInstallHook {
    async fn before_overlay(
        &self,
        app: &AppHandle,
        task: &DownloadTaskRow,
        target: &std::path::Path,
    ) -> Result<Option<Box<dyn InstallGuard>>, String> {
        if task.resource_type != "fastclaw-app" || task.resource_id != "fastkg" {
            return Ok(None);
        }
        if !super::same_executable(target, &paths::fastkg_service_dir()) {
            return Err("KG 安装目标与当前服务目录不一致".into());
        }
        Ok(Some(Box::new(prepare(app).await?)))
    }
}

pub struct KgInstallGuard {
    app: AppHandle,
    generations: Vec<(String, u64)>,
    _operation: tokio::sync::OwnedMutexGuard<()>,
    finished: bool,
}

fn ensure_target_idle() -> Result<(), String> {
    let expected = [
        paths::qdrant_path(),
        paths::llama_server_path(),
        paths::fastkg_server_path(),
    ];
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    for (pid, process) in system.processes() {
        if process.exe().is_some_and(|exe| {
            expected
                .iter()
                .any(|path| super::same_executable(exe, path))
        }) {
            return Err(format!(
                "KG 安装目录仍有进程使用（PID={}），已拒绝覆盖；请确认所属应用并安全停止后重试",
                pid
            ));
        }
    }
    Ok(())
}

pub async fn prepare(app: &AppHandle) -> Result<KgInstallGuard, String> {
    let state = app.state::<AppState>();
    // 先检查，拒绝时不改变正在运行的服务意图。
    if let Some(registry) = kg_coordination::read_registry().map_err(|e| e.to_string())? {
        if !kg_coordination::current_fastclaw_is_owner(&registry)
            || registry
                .leases
                .iter()
                .any(|lease| lease.app_pid != std::process::id())
        {
            return Err("KG 仍被其他消费者使用，请退出共享方后重试安装".into());
        }
    }
    let generations = state
        .dependency_recovery
        .begin_kg_install(app)
        .map_err(|e| e.to_string())?;
    let operation = state
        .dependency_recovery
        .operation
        .clone()
        .lock_owned()
        .await;
    let guard = KgInstallGuard {
        app: app.clone(),
        generations,
        _operation: operation,
        finished: false,
    };
    if state.dependency_recovery.is_shutting_down() {
        return Err("应用正在退出，已取消 KG 文件覆盖".into());
    }
    wsp::stop_kg_inner(&state.kg_pids)
        .await
        .map_err(|e| e.to_string())?;
    // 安全停止可以选择保留无法确认归属的外部进程，安装不能把“已跳过”视为可覆盖。
    ensure_target_idle()?;
    // 占有 starting 槽直至覆盖完成，防止共享方在覆盖窗口创建新进程。
    kg_coordination::begin_fastkg_maintenance(Some(kg_coordination::local_starting_instance()))
        .map_err(|e| e.to_string())?;
    wsp::start_kg_lease_heartbeat();
    Ok(guard)
}

#[async_trait::async_trait]
impl InstallGuard for KgInstallGuard {
    async fn finish(&mut self, success: bool) -> Result<(), String> {
        if self.finished {
            return Ok(());
        }
        let success = success
            && wsp::check_kg_service_files()
                .map(|files| files.all_exist)
                .unwrap_or(false);
        self.app
            .state::<AppState>()
            .dependency_recovery
            .finish_kg_install(&self.app, &self.generations, success);
        self.finished = true;
        if success {
            Ok(())
        } else {
            Err("KG 安装未完成或文件不完整，已暂停自动恢复，请重新安装".into())
        }
    }
}

impl Drop for KgInstallGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.app
                .state::<AppState>()
                .dependency_recovery
                .finish_kg_install(&self.app, &self.generations, false);
        }
    }
}
