use serde::Serialize;
pub mod recovery;
pub mod manager;
pub mod environment;
pub mod owned_process;
pub mod install;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use sysinfo::{Pid, ProcessesToUpdate, System};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    Stopped,
    PortOccupied,
    Probing,
    Starting,
    Running,
    Unhealthy,
    Stopping,
    Failed,
}

impl LifecycleState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::PortOccupied => "port_occupied",
            Self::Probing => "probing",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Unhealthy => "unhealthy",
            Self::Stopping => "stopping",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOwnership {
    SelfManaged,
    SharedManaged,
    ExternalUnmanaged,
    None,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub process_started_at: u64,
    pub canonical_exe_path: PathBuf,
    pub effective_port: u16,
}

#[derive(Debug, Clone)]
pub struct ServiceRuntime {
    pub lifecycle: LifecycleState,
    pub ownership: RuntimeOwnership,
    pub preferred_port: u16,
    pub effective_port: Option<u16>,
    pub process: Option<ProcessIdentity>,
    pub last_error: Option<String>,
    /// 最近一次生命周期变更时间；用于避免状态轮询过早回收 Starting/Probing。
    pub lifecycle_updated_at: Instant,
}

impl Default for ServiceRuntime {
    fn default() -> Self {
        Self {
            lifecycle: LifecycleState::Stopped,
            ownership: RuntimeOwnership::None,
            preferred_port: 0,
            effective_port: None,
            process: None,
            last_error: None,
            lifecycle_updated_at: Instant::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProcessFacts {
    pub process_started_at: u64,
    pub canonical_exe_path: PathBuf,
}

#[derive(Default)]
pub struct DependencyServiceSupervisor {
    services: RwLock<HashMap<String, ServiceRuntime>>,
    pub owned: owned_process::OwnedProcesses,
}

static GLOBAL_SUPERVISOR: OnceLock<Arc<DependencyServiceSupervisor>> = OnceLock::new();

pub fn dependency_service_supervisor() -> Arc<DependencyServiceSupervisor> {
    GLOBAL_SUPERVISOR
        .get_or_init(|| Arc::new(DependencyServiceSupervisor::default()))
        .clone()
}

impl DependencyServiceSupervisor {
    pub async fn mark_probing(&self, service_id: &str, preferred_port: u16, port: u16) {
        let mut services = self.services.write().await;
        let runtime = services.entry(service_id.to_string()).or_default();
        runtime.lifecycle = LifecycleState::Probing;
        runtime.ownership = RuntimeOwnership::None;
        runtime.preferred_port = preferred_port;
        runtime.effective_port = Some(port);
        runtime.process = None;
        runtime.last_error = None;
        runtime.lifecycle_updated_at = Instant::now();
    }

    pub async fn mark_starting(&self, service_id: &str, preferred_port: u16, port: u16) {
        let mut services = self.services.write().await;
        let runtime = services.entry(service_id.to_string()).or_default();
        runtime.lifecycle = LifecycleState::Starting;
        runtime.ownership = RuntimeOwnership::SelfManaged;
        runtime.preferred_port = preferred_port;
        runtime.effective_port = Some(port);
        runtime.process = None;
        runtime.last_error = None;
        runtime.lifecycle_updated_at = Instant::now();
    }

    /// 在健康检查完成前记录刚刚创建的进程身份。
    ///
    /// 这使启动中的服务也能被安全停止；调用方仍需在停止前重新校验
    /// PID、启动时间和可执行文件路径，避免误伤 PID 复用后的其他进程。
    pub async fn record_starting_process(
        &self,
        service_id: &str,
        preferred_port: u16,
        port: u16,
        pid: u32,
        expected_exe: &Path,
    ) -> Result<ProcessIdentity, String> {
        let facts = capture_process_facts(&[pid])
            .remove(&pid)
            .ok_or_else(|| format!("无法读取 PID={} 的进程身份", pid))?;
        if !same_executable(&facts.canonical_exe_path, expected_exe) {
            return Err(format!(
                "PID={} 的可执行文件不匹配：实际={}，期望={}",
                pid,
                facts.canonical_exe_path.display(),
                expected_exe.display()
            ));
        }
        let identity = ProcessIdentity {
            pid,
            process_started_at: facts.process_started_at,
            canonical_exe_path: facts.canonical_exe_path,
            effective_port: port,
        };
        let mut services = self.services.write().await;
        services.insert(
            service_id.to_string(),
            ServiceRuntime {
                lifecycle: LifecycleState::Starting,
                ownership: RuntimeOwnership::SelfManaged,
                preferred_port,
                effective_port: Some(port),
                process: Some(identity.clone()),
                last_error: None,
                lifecycle_updated_at: Instant::now(),
            },
        );
        Ok(identity)
    }

    pub async fn record_owned_process(
        &self,
        service_id: &str,
        preferred_port: u16,
        port: u16,
        pid: u32,
        expected_exe: &Path,
    ) -> Result<ProcessIdentity, String> {
        let facts = capture_process_facts(&[pid])
            .remove(&pid)
            .ok_or_else(|| format!("无法读取 PID={} 的进程身份", pid))?;
        if !same_executable(&facts.canonical_exe_path, expected_exe) {
            return Err(format!(
                "PID={} 的可执行文件不匹配：实际={}，期望={}",
                pid,
                facts.canonical_exe_path.display(),
                expected_exe.display()
            ));
        }
        let identity = ProcessIdentity {
            pid,
            process_started_at: facts.process_started_at,
            canonical_exe_path: facts.canonical_exe_path,
            effective_port: port,
        };
        let mut services = self.services.write().await;
        services.insert(
            service_id.to_string(),
            ServiceRuntime {
                lifecycle: LifecycleState::Running,
                ownership: RuntimeOwnership::SelfManaged,
                preferred_port,
                effective_port: Some(port),
                process: Some(identity.clone()),
                last_error: None,
                lifecycle_updated_at: Instant::now(),
            },
        );
        Ok(identity)
    }

    pub async fn mark_failed(&self, service_id: &str, message: impl Into<String>) {
        let mut services = self.services.write().await;
        let runtime = services.entry(service_id.to_string()).or_default();
        runtime.lifecycle = LifecycleState::Failed;
        // 启动失败不等于子进程已经退出。保留身份，直到调用方完成
        // 身份验证和清理后显式 clear，避免残留进程失去安全停止依据。
        runtime.last_error = Some(message.into());
        runtime.lifecycle_updated_at = Instant::now();
    }

    /// 保留已验证的进程身份，将服务标记为异常，以便 UI 继续提供停止操作。
    pub async fn mark_unhealthy(&self, service_id: &str, message: impl Into<String>) {
        let mut services = self.services.write().await;
        let runtime = services.entry(service_id.to_string()).or_default();
        runtime.lifecycle = LifecycleState::Unhealthy;
        runtime.last_error = Some(message.into());
        runtime.lifecycle_updated_at = Instant::now();
    }

    pub async fn mark_stopping(&self, service_id: &str) {
        if let Some(runtime) = self.services.write().await.get_mut(service_id) {
            runtime.lifecycle = LifecycleState::Stopping;
            runtime.lifecycle_updated_at = Instant::now();
        }
    }

    pub async fn clear(&self, service_id: &str) {
        self.services.write().await.remove(service_id);
        self.owned.forget(service_id);
    }

    pub async fn snapshot(&self, service_id: &str) -> Option<ServiceRuntime> {
        self.services.read().await.get(service_id).cloned()
    }

    pub async fn snapshots(&self) -> HashMap<String, ServiceRuntime> {
        self.services.read().await.clone()
    }
}

pub fn capture_process_facts(pids: &[u32]) -> HashMap<u32, ProcessFacts> {
    if pids.is_empty() {
        return HashMap::new();
    }
    let mut system = System::new();
    let requested = pids.iter().copied().map(Pid::from_u32).collect::<Vec<_>>();
    system.refresh_processes(ProcessesToUpdate::Some(&requested), true);
    let mut result = HashMap::new();
    for pid in pids {
        let Some(process) = system.process(Pid::from_u32(*pid)) else {
            continue;
        };
        let Some(exe) = process.exe() else {
            continue;
        };
        result.insert(
            *pid,
            ProcessFacts {
                process_started_at: process.start_time(),
                canonical_exe_path: canonical_or_owned(exe),
            },
        );
    }
    result
}

pub fn identity_matches(
    identity: &ProcessIdentity,
    facts: Option<&ProcessFacts>,
    listener_pid: Option<u32>,
    expected_exe: &Path,
) -> bool {
    let Some(facts) = facts else {
        return false;
    };
    listener_pid == Some(identity.pid)
        && identity.effective_port != 0
        && identity.process_started_at == facts.process_started_at
        && same_executable(&identity.canonical_exe_path, &facts.canonical_exe_path)
        && same_executable(&facts.canonical_exe_path, expected_exe)
}

/// 校验受管理进程身份，但不要求它已经开始监听预期端口。
/// 仅用于 Starting/Stopping/Unhealthy 阶段的安全清理。
pub fn identity_matches_process(
    identity: &ProcessIdentity,
    facts: Option<&ProcessFacts>,
    expected_exe: &Path,
) -> bool {
    let Some(facts) = facts else {
        return false;
    };
    identity.process_started_at == facts.process_started_at
        && same_executable(&identity.canonical_exe_path, &facts.canonical_exe_path)
        && same_executable(&facts.canonical_exe_path, expected_exe)
}

pub fn same_executable(left: &Path, right: &Path) -> bool {
    normalize_path(&canonical_or_owned(left)) == normalize_path(&canonical_or_owned(right))
}

fn canonical_or_owned(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn failure_preserves_process_identity_until_explicit_cleanup() {
        let supervisor = DependencyServiceSupervisor::default();
        let exe = std::env::current_exe().unwrap();
        supervisor.services.write().await.insert(
            "qdrant".into(),
            ServiceRuntime {
                lifecycle: LifecycleState::Starting,
                ownership: RuntimeOwnership::SelfManaged,
                preferred_port: 23060,
                effective_port: Some(23061),
                process: Some(ProcessIdentity {
                    pid: 42,
                    process_started_at: 100,
                    canonical_exe_path: exe.clone(),
                    effective_port: 23061,
                }),
                ..ServiceRuntime::default()
            },
        );
        supervisor.mark_failed("qdrant", "健康检查超时").await;
        let runtime = supervisor.snapshot("qdrant").await.unwrap();
        assert_eq!(runtime.lifecycle, LifecycleState::Failed);
        assert_eq!(runtime.ownership, RuntimeOwnership::SelfManaged);
        let process = runtime.process.unwrap();
        assert_eq!(process.pid, 42);
        assert_eq!(process.process_started_at, 100);
        assert_eq!(process.canonical_exe_path, exe);
        assert_eq!(runtime.effective_port, Some(23061));
        assert_eq!(runtime.last_error.as_deref(), Some("健康检查超时"));

        supervisor.clear("qdrant").await;
        supervisor.mark_failed("qdrant", "进程已清理").await;
        let cleaned = supervisor.snapshot("qdrant").await.unwrap();
        assert!(cleaned.process.is_none());
        assert_eq!(cleaned.ownership, RuntimeOwnership::None);
    }

    #[test]
    fn process_identity_requires_same_pid_start_time_path_and_listener() {
        let exe = std::env::current_exe().expect("current exe");
        let identity = ProcessIdentity {
            pid: 42,
            process_started_at: 100,
            canonical_exe_path: exe.clone(),
            effective_port: 6333,
        };
        let facts = ProcessFacts {
            process_started_at: 100,
            canonical_exe_path: exe.clone(),
        };
        assert!(identity_matches(&identity, Some(&facts), Some(42), &exe));
        assert!(!identity_matches(&identity, Some(&facts), Some(43), &exe));
        let reused = ProcessFacts {
            process_started_at: 101,
            canonical_exe_path: exe.clone(),
        };
        assert!(!identity_matches(&identity, Some(&reused), Some(42), &exe));
    }

    #[test]
    fn pending_process_identity_does_not_require_a_listener() {
        let exe = std::env::current_exe().expect("current exe");
        let identity = ProcessIdentity {
            pid: 42,
            process_started_at: 100,
            canonical_exe_path: exe.clone(),
            effective_port: 23180,
        };
        let facts = ProcessFacts {
            process_started_at: 100,
            canonical_exe_path: exe.clone(),
        };
        assert!(identity_matches_process(&identity, Some(&facts), &exe));
        let reused = ProcessFacts {
            process_started_at: 101,
            canonical_exe_path: exe.clone(),
        };
        assert!(!identity_matches_process(&identity, Some(&reused), &exe));
    }
}
