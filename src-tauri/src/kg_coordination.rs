use crate::utils::paths;
use chrono::{DateTime, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::OnceLock;
use uuid::Uuid;

pub const PROTOCOL_VERSION: u32 = 1;
pub const TARGET_PROTOCOL_VERSION: u32 = 2;
pub const APP_ID: &str = "fastclaw";
pub const RECOVERY_CAPABILITY: &str = "owner_recovery_v1";
const LEASE_STALE_SECS: i64 = 20;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SharedOwnership {
    SelfManaged,
    SharedManaged,
    ExternalUnmanaged,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedServiceProcess {
    pub url: String,
    pub pid: u32,
    pub exe: String,
    /// v1 兼容扩展；目标 v2 中为必填。用于防止 Windows PID 复用误认进程。
    #[serde(default)]
    pub process_started_at: Option<DateTime<Utc>>,
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedKgInstance {
    pub instance_id: String,
    pub starter_app: String,
    /// v1 兼容扩展；旧共享方会忽略这些字段。
    #[serde(default)]
    pub owner_app_pid: Option<u32>,
    #[serde(default)]
    pub owner_app_started_at: Option<DateTime<Utc>>,
    #[serde(default = "default_owner_generation")]
    pub owner_generation: u64,
    pub service_dir: String,
    #[serde(default)]
    pub package_version: String,
    pub fastkg: SharedServiceProcess,
    pub qdrant: SharedServiceProcess,
    pub llama_server: SharedServiceProcess,
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KgConsumerLease {
    pub lease_id: String,
    pub app: String,
    pub app_pid: u32,
    pub process_started_at: DateTime<Utc>,
    pub heartbeat_at: DateTime<Utc>,
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KgRuntimeRegistry {
    pub protocol_version: u32,
    pub state: String,
    pub instance: SharedKgInstance,
    #[serde(default)]
    pub leases: Vec<KgConsumerLease>,
    pub updated_at: DateTime<Utc>,
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KgSharedStatus {
    pub ownership: SharedOwnership,
    pub starter_app: Option<String>,
    pub lease_count: usize,
    pub consumers: Vec<String>,
    pub lease_registered: bool,
    pub shared_instance: bool,
    pub pending_restart: bool,
    pub state: String,
    pub instance_id: Option<String>,
    pub service_dir: Option<String>,
}

#[derive(Debug)]
pub enum StartSlot {
    Acquired,
    Busy,
}

#[derive(Debug, Clone)]
pub struct FastKgMaintenanceClaim {
    pub instance_id: String,
    pub owner_generation: u64,
    pub recovery_generation: Option<u64>,
}

#[derive(Debug)]
pub enum ReleaseOutcome {
    NoRegistry,
    Released {
        remaining: usize,
    },
    OwnershipTransferred {
        instance: SharedKgInstance,
        new_owner: String,
        remaining: usize,
    },
    ReadyToStop {
        instance: SharedKgInstance,
    },
    NeedsAdoption {
        snapshot: SharedKgInstance,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum CoordinationError {
    #[error("coordination I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid coordination registry: {0}")]
    InvalidRegistry(#[from] serde_json::Error),
    #[error("unsupported coordination protocol: {0}")]
    UnsupportedProtocol(u32),
    #[error("KG coordination ownership changed while publishing the ready instance")]
    OwnershipChanged,
    #[error("KG 服务当前不能执行单服务维护：{0}")]
    MaintenanceDenied(String),
}

struct RegistryLock {
    file: File,
}

impl Drop for RegistryLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

fn lock_registry() -> Result<RegistryLock, CoordinationError> {
    fs::create_dir_all(paths::kg_coordination_dir())?;
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(paths::kg_runtime_lock_path())?;
    file.lock_exclusive()?;
    Ok(RegistryLock { file })
}

fn read_registry_unlocked() -> Result<Option<KgRuntimeRegistry>, CoordinationError> {
    let path = paths::kg_runtime_registry_path();
    if !path.exists() {
        return Ok(None);
    }
    let mut text = String::new();
    File::open(path)?.read_to_string(&mut text)?;
    if text.trim().is_empty() {
        return Ok(None);
    }
    let registry: KgRuntimeRegistry = serde_json::from_str(&text)?;
    if !matches!(
        registry.protocol_version,
        PROTOCOL_VERSION | TARGET_PROTOCOL_VERSION
    ) {
        return Err(CoordinationError::UnsupportedProtocol(
            registry.protocol_version,
        ));
    }
    Ok(Some(registry))
}

fn write_registry_unlocked(registry: &KgRuntimeRegistry) -> Result<(), CoordinationError> {
    fs::create_dir_all(paths::kg_coordination_dir())?;
    let target = paths::kg_runtime_registry_path();
    let tmp = target.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(registry)?;
    {
        let mut file = File::create(&tmp)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
    }
    fs::rename(tmp, target)?;
    Ok(())
}

pub fn read_registry() -> Result<Option<KgRuntimeRegistry>, CoordinationError> {
    let _lock = lock_registry()?;
    read_registry_unlocked()
}

pub fn update_registry<F>(f: F) -> Result<Option<KgRuntimeRegistry>, CoordinationError>
where
    F: FnOnce(Option<KgRuntimeRegistry>) -> Option<KgRuntimeRegistry>,
{
    let _lock = lock_registry()?;
    let current = read_registry_unlocked()?;
    let next = f(current);
    if let Some(mut registry) = next {
        registry.updated_at = Utc::now();
        write_registry_unlocked(&registry)?;
        Ok(Some(registry))
    } else {
        let path = paths::kg_runtime_registry_path();
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(None)
    }
}

pub fn discover_shared_instance() -> Result<Option<SharedKgInstance>, CoordinationError> {
    Ok(read_registry()?.and_then(|r| {
        if r.state == "ready" {
            Some(r.instance)
        } else {
            None
        }
    }))
}

pub fn acquire_start_slot(instance: SharedKgInstance) -> Result<StartSlot, CoordinationError> {
    let _lock = lock_registry()?;
    match read_registry_unlocked()? {
        Some(mut registry) if registry.state == "ready" || registry.state == "starting" => {
            normalize_leases_in_place(&mut registry);
            write_registry_unlocked(&registry)?;
            Ok(StartSlot::Busy)
        }
        Some(registry) if registry.state == "stopping" => Ok(StartSlot::Busy),
        _ => {
            let registry = KgRuntimeRegistry {
                protocol_version: PROTOCOL_VERSION,
                state: "starting".to_string(),
                instance,
                leases: vec![new_lease()],
                updated_at: Utc::now(),
                extra: serde_json::Map::new(),
            };
            write_registry_unlocked(&registry)?;
            Ok(StartSlot::Acquired)
        }
    }
}

pub fn register_ready_instance(
    mut instance: SharedKgInstance,
    expected_instance_id: &str,
    expected_owner_generation: u64,
) -> Result<KgRuntimeRegistry, CoordinationError> {
    let _lock = lock_registry()?;
    let Some(mut current) = read_registry_unlocked()? else {
        return Err(CoordinationError::OwnershipChanged);
    };
    if !ready_publish_claim_matches(
        &current,
        &instance,
        expected_instance_id,
        expected_owner_generation,
    ) {
        return Err(CoordinationError::OwnershipChanged);
    }

    // 最终 PID/端点来自 proposed，但实例令牌必须沿用获得启动槽时的值。
    // 否则旧启动方可能用一个新 UUID 覆盖已经变化的注册表。
    instance.instance_id = current.instance.instance_id.clone();
    instance.owner_generation = current.instance.owner_generation;
    remove_stale_leases_in_place(&mut current);
    normalize_leases_in_place(&mut current);
    current.instance = instance;
    current.state = "ready".to_string();
    current.protocol_version = PROTOCOL_VERSION;
    current.updated_at = Utc::now();
    upsert_own_lease(&mut current);
    let registry = current;
    write_registry_unlocked(&registry)?;
    Ok(registry)
}

/// 将当前 FastClaw 所有的共享实例切换到不可复用的维护状态。
///
/// `initial_instance` 仅在注册表不存在时使用：手动启动 FastKG 可以据此创建
/// `starting` 注册表；手动停止传入 `None`，避免为了停止一个未共享的进程而创建注册表。
pub fn begin_fastkg_maintenance(
    initial_instance: Option<SharedKgInstance>,
) -> Result<Option<FastKgMaintenanceClaim>, CoordinationError> {
    let _lock = lock_registry()?;
    let mut registry = match read_registry_unlocked()? {
        Some(registry) => registry,
        None => {
            let Some(instance) = initial_instance else {
                return Ok(None);
            };
            if normalize_app(&instance.starter_app).as_deref() != Some(APP_ID)
                || instance.owner_app_pid != Some(std::process::id())
                || instance.owner_app_started_at != Some(own_process_started_at())
            {
                return Err(CoordinationError::MaintenanceDenied(
                    "新实例的 owner 身份与当前 FastClaw 不一致".to_string(),
                ));
            }
            let claim = FastKgMaintenanceClaim {
                instance_id: instance.instance_id.clone(),
                owner_generation: instance.owner_generation,
                recovery_generation: None,
            };
            let registry = KgRuntimeRegistry {
                protocol_version: PROTOCOL_VERSION,
                state: "starting".to_string(),
                instance,
                leases: vec![new_lease()],
                updated_at: Utc::now(),
                extra: serde_json::Map::new(),
            };
            write_registry_unlocked(&registry)?;
            return Ok(Some(claim));
        }
    };

    if !current_fastclaw_is_owner(&registry) {
        return Err(CoordinationError::MaintenanceDenied(format!(
            "共享实例由 {} 管理",
            registry.instance.starter_app
        )));
    }
    if !matches!(registry.state.as_str(), "ready" | "starting") {
        return Err(CoordinationError::MaintenanceDenied(format!(
            "共享实例当前状态为 {}",
            registry.state
        )));
    }

    retain_live_lease_identities(&mut registry);
    normalize_leases_in_place(&mut registry);
    let other_consumers = registry
        .leases
        .iter()
        .filter_map(|lease| {
            let own = normalize_app(&lease.app).as_deref() == Some(APP_ID)
                && lease.app_pid == std::process::id()
                && lease.process_started_at == own_process_started_at();
            (!own).then(|| lease.app.clone())
        })
        .collect::<Vec<_>>();
    if !other_consumers.is_empty() && !negotiated_recovery_is_active(&registry) {
        return Err(CoordinationError::MaintenanceDenied(format!(
            "仍有其他消费者使用：{}",
            other_consumers.join(", ")
        )));
    }

    upsert_own_lease(&mut registry);
    registry.state = "starting".to_string();
    registry.updated_at = Utc::now();
    let claim = FastKgMaintenanceClaim {
        instance_id: registry.instance.instance_id.clone(),
        owner_generation: registry.instance.owner_generation,
        recovery_generation: recovery_generation(&registry),
    };
    write_registry_unlocked(&registry)?;
    Ok(Some(claim))
}

/// FastKG 已停止或启动失败：清除旧进程指纹，同时保持 `starting`，防止共享方复用。
pub fn mark_fastkg_stopped(claim: &FastKgMaintenanceClaim) -> Result<bool, CoordinationError> {
    let _lock = lock_registry()?;
    let Some(mut registry) = read_registry_unlocked()? else {
        return Ok(false);
    };
    if !fastkg_maintenance_claim_matches(&registry, claim) {
        return Ok(false);
    }
    apply_fastkg_stopped_in_place(&mut registry);
    registry.updated_at = Utc::now();
    write_registry_unlocked(&registry)?;
    Ok(true)
}

/// 停止动作失败且原进程仍然有效时，恢复共享实例的 ready 状态。
pub fn restore_fastkg_ready(claim: &FastKgMaintenanceClaim) -> Result<bool, CoordinationError> {
    let _lock = lock_registry()?;
    let Some(mut registry) = read_registry_unlocked()? else {
        return Ok(false);
    };
    if !fastkg_maintenance_claim_matches(&registry, claim)
        || registry.instance.fastkg.pid == 0
        || registry.instance.fastkg.process_started_at.is_none()
    {
        return Ok(false);
    }
    registry.state = "ready".to_string();
    registry.updated_at = Utc::now();
    write_registry_unlocked(&registry)?;
    Ok(true)
}

/// 使用维护令牌只更新 FastKG 进程指纹，保留 Qdrant/embedding 的共享信息。
pub fn publish_fastkg_ready(
    claim: &FastKgMaintenanceClaim,
    fastkg: SharedServiceProcess,
) -> Result<KgRuntimeRegistry, CoordinationError> {
    if fastkg.pid == 0 || fastkg.process_started_at.is_none() {
        return Err(CoordinationError::MaintenanceDenied(
            "FastKG 进程指纹不完整".to_string(),
        ));
    }
    let _lock = lock_registry()?;
    let Some(mut registry) = read_registry_unlocked()? else {
        return Err(CoordinationError::OwnershipChanged);
    };
    if !fastkg_maintenance_claim_matches(&registry, claim) {
        return Err(CoordinationError::OwnershipChanged);
    }
    apply_fastkg_ready_in_place(&mut registry, fastkg);
    registry.updated_at = Utc::now();
    upsert_own_lease(&mut registry);
    write_registry_unlocked(&registry)?;
    Ok(registry)
}

pub(crate) fn current_fastclaw_is_owner(registry: &KgRuntimeRegistry) -> bool {
    normalize_app(&registry.instance.starter_app).as_deref() == Some(APP_ID)
        && registry.instance.owner_app_pid == Some(std::process::id())
        && registry.instance.owner_app_started_at == Some(own_process_started_at())
}

fn consumer_supports_recovery(lease: &KgConsumerLease) -> bool {
    lease.extra.get("capabilities").and_then(|v| v.as_array())
        .is_some_and(|values| values.iter().any(|value| value.as_str() == Some(RECOVERY_CAPABILITY)))
}

fn negotiated_recovery_is_active(registry: &KgRuntimeRegistry) -> bool {
    registry.extra.get("recovery").and_then(|v| v.get("phase")).and_then(|v| v.as_str()) == Some("recovering")
        && registry.leases.iter().all(consumer_supports_recovery)
}

/// 在原有 owner CAS 之外维护独立恢复代次；只读旧方租约，绝不替旧方声明能力。
pub fn begin_owner_recovery() -> Result<Option<FastKgMaintenanceClaim>, CoordinationError> {
    let _lock = lock_registry()?;
    let Some(mut registry) = read_registry_unlocked()? else { return Ok(None); };
    if !current_fastclaw_is_owner(&registry) { return Err(CoordinationError::OwnershipChanged); }
    retain_live_lease_identities(&mut registry);
    if registry.leases.iter().any(|lease| {
        let own = normalize_app(&lease.app).as_deref() == Some(APP_ID)
            && lease.app_pid == std::process::id() && lease.process_started_at == own_process_started_at();
        !own && (!consumer_supports_recovery(lease) || normalize_app(&lease.app).is_none())
    }) {
        return Err(CoordinationError::MaintenanceDenied("存在不支持恢复协议的在线消费者，请升级共享方或等待其退出".into()));
    }
    upsert_own_lease(&mut registry);
    let generation = if negotiated_recovery_is_active(&registry) {
        recovery_generation(&registry).unwrap_or(1)
    } else { recovery_generation(&registry).unwrap_or(0).saturating_add(1) };
    registry.extra.insert("recovery".into(), serde_json::json!({
        "generation": generation, "phase": "recovering", "owner_generation": registry.instance.owner_generation,
        "started_at": Utc::now(), "message": "KG 所有者正在恢复服务，请暂停新业务请求并保持租约心跳"
    }));
    registry.state = "starting".into();
    registry.updated_at = Utc::now();
    write_registry_unlocked(&registry)?;
    Ok(Some(FastKgMaintenanceClaim {
        instance_id: registry.instance.instance_id,
        owner_generation: registry.instance.owner_generation,
        recovery_generation: Some(generation),
    }))
}

fn recovery_generation(registry: &KgRuntimeRegistry) -> Option<u64> {
    registry.extra.get("recovery").and_then(|v| v.get("generation")).and_then(|v| v.as_u64())
}

/// 失败也必须通过同一 CAS 通知共享方；不得覆盖后继 owner 或后继恢复任务。
pub fn publish_recovery_failure(claim: &FastKgMaintenanceClaim, message: &str) -> Result<bool, CoordinationError> {
    let _lock = lock_registry()?;
    let Some(mut registry) = read_registry_unlocked()? else { return Ok(false); };
    if !fastkg_maintenance_claim_matches(&registry, claim) { return Ok(false); }
    if let Some(recovery) = registry.extra.get_mut("recovery").and_then(|v| v.as_object_mut()) {
        recovery.insert("phase".into(), serde_json::json!("failed"));
        recovery.insert("message".into(), serde_json::json!(message));
        recovery.insert("completed_at".into(), serde_json::json!(Utc::now()));
    }
    registry.updated_at = Utc::now();
    write_registry_unlocked(&registry)?;
    Ok(true)
}

/// 接管上次 FastClaw 异常退出后的实例；在同一注册表锁内校验 owner 和服务指纹。
pub fn adopt_own_orphan_for_recovery() -> Result<bool, CoordinationError> {
    let _lock = lock_registry()?;
    let Some(mut registry) = read_registry_unlocked()? else { return Ok(false); };
    if current_fastclaw_is_owner(&registry) || normalize_app(&registry.instance.starter_app).as_deref() != Some(APP_ID) {
        return Ok(false);
    }
    let (Some(owner_pid), Some(owner_started)) = (registry.instance.owner_app_pid, registry.instance.owner_app_started_at) else { return Ok(false); };
    let owner_facts = crate::core::dependency_services::capture_process_facts(&[owner_pid]);
    if owner_facts.get(&owner_pid).is_some_and(|facts| process_time_matches(facts.process_started_at, owner_started))
        || (owner_facts.get(&owner_pid).is_none() && pid_exists(owner_pid)) { return Ok(false); }
    let pairs = [(&registry.instance.qdrant, paths::qdrant_path()),
        (&registry.instance.llama_server, paths::llama_server_path()),
        (&registry.instance.fastkg, paths::fastkg_server_path())];
    let facts = crate::core::dependency_services::capture_process_facts(&pairs.iter().map(|(p, _)| p.pid).collect::<Vec<_>>());
    for (process, expected) in pairs {
        if process.pid == 0 { continue; }
        let Some(started) = process.process_started_at else { return Ok(false); };
        if let Some(actual) = facts.get(&process.pid) {
            if !process_time_matches(actual.process_started_at, started)
                || !crate::core::dependency_services::same_executable(&actual.canonical_exe_path, &expected)
                || !crate::core::dependency_services::same_executable(&actual.canonical_exe_path, std::path::Path::new(&process.exe))
            { return Ok(false); }
        } else if pid_exists(process.pid) { return Ok(false); }
    }
    retain_live_lease_identities(&mut registry);
    registry.instance.owner_app_pid = Some(std::process::id());
    registry.instance.owner_app_started_at = Some(own_process_started_at());
    registry.instance.owner_generation = registry.instance.owner_generation.saturating_add(1);
    upsert_own_lease(&mut registry);
    registry.updated_at = Utc::now();
    write_registry_unlocked(&registry)?;
    Ok(true)
}

pub fn publish_recovered_dependencies(claim: &FastKgMaintenanceClaim, qdrant: SharedServiceProcess, llama: SharedServiceProcess) -> Result<(), CoordinationError> {
    let _lock = lock_registry()?;
    let Some(mut registry) = read_registry_unlocked()? else { return Err(CoordinationError::OwnershipChanged); };
    if !fastkg_maintenance_claim_matches(&registry, claim) { return Err(CoordinationError::OwnershipChanged); }
    registry.instance.qdrant = qdrant;
    registry.instance.llama_server = llama;
    registry.updated_at = Utc::now();
    write_registry_unlocked(&registry)
}

/// 在同一恢复代次内逐个发布上游身份，避免冷启动中断后丢失已启动的进程。
pub fn publish_recovered_upstream(claim: &FastKgMaintenanceClaim, service: &str, process: SharedServiceProcess) -> Result<(), CoordinationError> {
    let _lock = lock_registry()?;
    let Some(mut registry) = read_registry_unlocked()? else { return Err(CoordinationError::OwnershipChanged); };
    if !fastkg_maintenance_claim_matches(&registry, claim) { return Err(CoordinationError::OwnershipChanged); }
    match service {
        "qdrant" => registry.instance.qdrant = process,
        "llama_server" => registry.instance.llama_server = process,
        _ => return Err(CoordinationError::MaintenanceDenied("不是 KG 上游服务".into())),
    }
    registry.updated_at = Utc::now();
    write_registry_unlocked(&registry)
}

fn fastkg_maintenance_claim_matches(
    registry: &KgRuntimeRegistry,
    claim: &FastKgMaintenanceClaim,
) -> bool {
    registry.state == "starting"
        && current_fastclaw_is_owner(registry)
        && registry.instance.instance_id == claim.instance_id
        && registry.instance.owner_generation == claim.owner_generation
        && recovery_generation(registry) == claim.recovery_generation
}

fn apply_fastkg_stopped_in_place(registry: &mut KgRuntimeRegistry) {
    registry.instance.fastkg.pid = 0;
    registry.instance.fastkg.process_started_at = None;
    registry.state = "starting".to_string();
}

fn apply_fastkg_ready_in_place(registry: &mut KgRuntimeRegistry, fastkg: SharedServiceProcess) {
    registry.instance.fastkg = fastkg;
    registry.state = "ready".to_string();
    if let Some(recovery) = registry.extra.get_mut("recovery").and_then(|v| v.as_object_mut()) {
        recovery.insert("phase".into(), serde_json::json!("ready"));
        recovery.insert("completed_at".into(), serde_json::json!(Utc::now()));
        recovery.insert("message".into(), serde_json::json!("恢复完成，请重新读取服务端点"));
    }
}

fn retain_live_lease_identities(registry: &mut KgRuntimeRegistry) {
    let pids = registry
        .leases
        .iter()
        .map(|lease| lease.app_pid)
        .collect::<Vec<_>>();
    let processes = crate::core::dependency_services::capture_process_facts(&pids);
    registry.leases.retain(|lease| {
        match processes.get(&lease.app_pid) {
            Some(facts) => process_time_matches(facts.process_started_at, lease.process_started_at),
            // 无法读取完整进程指纹时采取保守策略：PID 仍存在就保留租约，
            // 防止一次系统采集失败导致误停其他应用正在使用的共享服务。
            None => pid_exists(lease.app_pid),
        }
    });
}

fn ready_publish_claim_matches(
    current: &KgRuntimeRegistry,
    proposed: &SharedKgInstance,
    expected_instance_id: &str,
    expected_owner_generation: u64,
) -> bool {
    matches!(current.state.as_str(), "starting" | "ready")
        && current.instance.instance_id == expected_instance_id
        && current.instance.owner_generation == expected_owner_generation
        && current.instance.starter_app == APP_ID
        && proposed.starter_app == APP_ID
        && current.instance.owner_app_pid == Some(std::process::id())
        && proposed.owner_app_pid == current.instance.owner_app_pid
        && current.instance.owner_app_started_at == Some(own_process_started_at())
        && proposed.owner_app_started_at == current.instance.owner_app_started_at
        && current.instance.owner_generation == proposed.owner_generation
}

pub fn register_lease(instance_id: &str) -> Result<Option<KgRuntimeRegistry>, CoordinationError> {
    let _lock = lock_registry()?;
    let Some(mut registry) = read_registry_unlocked()? else {
        return Ok(None);
    };
    if registry.state != "ready" || registry.instance.instance_id != instance_id {
        return Ok(None);
    }
    remove_stale_leases_in_place(&mut registry);
    normalize_leases_in_place(&mut registry);
    upsert_own_lease(&mut registry);
    registry.updated_at = Utc::now();
    write_registry_unlocked(&registry)?;
    Ok(Some(registry))
}

pub fn refresh_lease() -> Result<(), CoordinationError> {
    let _ = update_registry(|current| {
        let mut registry = current?;
        remove_stale_leases_in_place(&mut registry);
        normalize_leases_in_place(&mut registry);
        let now = Utc::now();
        for lease in &mut registry.leases {
            if normalize_app(&lease.app).as_deref() == Some(APP_ID)
                && lease.app_pid == std::process::id()
            {
                lease.heartbeat_at = now;
            }
        }
        Some(registry)
    })?;
    Ok(())
}

pub fn release_lease() -> Result<ReleaseOutcome, CoordinationError> {
    let _lock = lock_registry()?;
    let Some(mut registry) = read_registry_unlocked()? else {
        return Ok(ReleaseOutcome::NoRegistry);
    };
    let lease_pids = registry
        .leases
        .iter()
        .map(|lease| lease.app_pid)
        .collect::<Vec<_>>();
    let processes = crate::core::dependency_services::capture_process_facts(&lease_pids);
    let outcome = release_registry_in_place(
        &mut registry,
        &processes,
        std::process::id(),
        own_process_started_at(),
    );
    registry.updated_at = Utc::now();
    write_registry_unlocked(&registry)?;
    Ok(outcome)
}

fn release_registry_in_place(
    registry: &mut KgRuntimeRegistry,
    processes: &std::collections::HashMap<u32, crate::core::dependency_services::ProcessFacts>,
    own_pid: u32,
    own_started_at: DateTime<Utc>,
) -> ReleaseOutcome {
    registry.leases.retain(|lease| {
        processes
            .get(&lease.app_pid)
            .map(|facts| process_time_matches(facts.process_started_at, lease.process_started_at))
            .unwrap_or_else(|| pid_exists(lease.app_pid))
    });
    normalize_leases_in_place(registry);

    let current_is_owner = normalize_app(&registry.instance.starter_app).as_deref() == Some(APP_ID)
        && registry.instance.owner_app_pid == Some(own_pid)
        && registry.instance.owner_app_started_at == Some(own_started_at);
    let own_lease_matches = registry.leases.iter().any(|lease| {
        normalize_app(&lease.app).as_deref() == Some(APP_ID)
            && lease.app_pid == own_pid
            && lease.process_started_at == own_started_at
    });

    if current_is_owner {
        registry.leases.retain(|lease| {
            !(normalize_app(&lease.app).as_deref() == Some(APP_ID)
                && lease.app_pid == own_pid
                && lease.process_started_at == own_started_at)
        });
        if !registry.leases.is_empty() {
            return ReleaseOutcome::Released { remaining: registry.leases.len() };
        }
        registry.state = "stopping".to_string();
        return ReleaseOutcome::ReadyToStop {
            instance: registry.instance.clone(),
        };
    }

    let has_other_consumer = registry
        .leases
        .iter()
        .any(|lease| !(normalize_app(&lease.app).as_deref() == Some(APP_ID)
            && lease.app_pid == own_pid && lease.process_started_at == own_started_at));
    if has_other_consumer || !own_lease_matches {
        registry.leases.retain(|lease| {
            !(normalize_app(&lease.app).as_deref() == Some(APP_ID)
                && lease.app_pid == own_pid
                && lease.process_started_at == own_started_at)
        });
        return ReleaseOutcome::Released {
            remaining: registry.leases.len(),
        };
    }

    // 非 owner 的最后一个 FastClaw 消费者必须保留租约，直到锁外完成服务指纹验证。
    ReleaseOutcome::NeedsAdoption {
        snapshot: registry.instance.clone(),
    }
}

pub fn adopt_for_shutdown(
    snapshot: &SharedKgInstance,
) -> Result<Option<SharedKgInstance>, CoordinationError> {
    let _lock = lock_registry()?;
    let Some(mut current) = read_registry_unlocked()? else {
        return Ok(None);
    };
    let lease_pids = current
        .leases
        .iter()
        .map(|lease| lease.app_pid)
        .collect::<Vec<_>>();
    let processes = crate::core::dependency_services::capture_process_facts(&lease_pids);
    current.leases.retain(|lease| {
        processes
            .get(&lease.app_pid)
            .map(|facts| process_time_matches(facts.process_started_at, lease.process_started_at))
            .unwrap_or(false)
    });
    normalize_leases_in_place(&mut current);

    if !adoption_claim_matches(
        &current,
        snapshot,
        std::process::id(),
        own_process_started_at(),
        registered_owner_is_gone(snapshot),
    ) {
        return Ok(None);
    }

    current.instance.starter_app = APP_ID.to_string();
    current.instance.owner_app_pid = Some(std::process::id());
    current.instance.owner_app_started_at = Some(own_process_started_at());
    current.instance.owner_generation = current.instance.owner_generation.saturating_add(1);
    current.state = "stopping".to_string();
    current.leases.clear();
    current.updated_at = Utc::now();
    let adopted = current.instance.clone();
    write_registry_unlocked(&current)?;
    Ok(Some(adopted))
}

fn adoption_claim_matches(
    current: &KgRuntimeRegistry,
    snapshot: &SharedKgInstance,
    own_pid: u32,
    own_started_at: DateTime<Utc>,
    original_owner_is_gone: bool,
) -> bool {
    current.state == "ready"
        && current.instance.instance_id == snapshot.instance_id
        && current.instance.owner_generation == snapshot.owner_generation
        && current.instance.starter_app == snapshot.starter_app
        && current.instance.owner_app_pid == snapshot.owner_app_pid
        && current.instance.owner_app_started_at == snapshot.owner_app_started_at
        && original_owner_is_gone
        && current.leases.len() == 1
        && current.leases.iter().any(|lease| {
            normalize_app(&lease.app).as_deref() == Some(APP_ID)
                && lease.app_pid == own_pid
                && lease.process_started_at == own_started_at
        })
}

/// 接管验证或 CAS 失败时只释放当前 FastClaw 租约，绝不终止共享进程。
pub fn release_own_lease_only() -> Result<usize, CoordinationError> {
    let _lock = lock_registry()?;
    let Some(mut registry) = read_registry_unlocked()? else {
        return Ok(0);
    };
    registry.leases.retain(|lease| {
        !(normalize_app(&lease.app).as_deref() == Some(APP_ID)
            && lease.app_pid == std::process::id()
            && lease.process_started_at == own_process_started_at())
    });
    registry.updated_at = Utc::now();
    let remaining = registry.leases.len();
    write_registry_unlocked(&registry)?;
    Ok(remaining)
}

pub fn registered_owner_is_gone(instance: &SharedKgInstance) -> bool {
    instance
        .owner_app_pid
        .filter(|pid| *pid != 0)
        .map(|pid| !pid_exists(pid))
        .unwrap_or(false)
}

pub fn reconcile_stale_leases() -> Result<Vec<KgConsumerLease>, CoordinationError> {
    let mut removed = Vec::new();
    let _ = update_registry(|current| {
        let mut registry = current?;
        removed = remove_stale_leases_in_place(&mut registry);
        normalize_leases_in_place(&mut registry);
        Some(registry)
    })?;
    Ok(removed)
}

pub fn begin_app_session() -> Result<(), CoordinationError> {
    let _ = update_registry(|current| {
        let mut registry = current?;
        remove_stale_leases_in_place(&mut registry);
        normalize_leases_in_place(&mut registry);
        registry
            .leases
            .retain(|lease| normalize_app(&lease.app).as_deref() != Some(APP_ID));
        Some(registry)
    })?;
    Ok(())
}

pub fn delete_registry_if_matches(
    instance_id: &str,
    owner_generation: u64,
) -> Result<bool, CoordinationError> {
    let _lock = lock_registry()?;
    let Some(current) = read_registry_unlocked()? else {
        return Ok(false);
    };
    if !registry_can_be_deleted(
        &current,
        instance_id,
        owner_generation,
        std::process::id(),
        own_process_started_at(),
    ) {
        return Ok(false);
    }
    let path = paths::kg_runtime_registry_path();
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(true)
}

fn registry_can_be_deleted(
    current: &KgRuntimeRegistry,
    instance_id: &str,
    owner_generation: u64,
    own_pid: u32,
    own_started_at: DateTime<Utc>,
) -> bool {
    current.state == "stopping"
        && normalize_app(&current.instance.starter_app).as_deref() == Some(APP_ID)
        && current.instance.owner_app_pid == Some(own_pid)
        && current.instance.owner_app_started_at == Some(own_started_at)
        && current.instance.instance_id == instance_id
        && current.instance.owner_generation == owner_generation
        && current.leases.is_empty()
}

pub fn delete_owned_registry() -> Result<bool, CoordinationError> {
    let _lock = lock_registry()?;
    let Some(current) = read_registry_unlocked()? else {
        return Ok(false);
    };
    let current_is_owner = normalize_app(&current.instance.starter_app).as_deref() == Some(APP_ID)
        && current.instance.owner_app_pid == Some(std::process::id())
        && current.instance.owner_app_started_at == Some(own_process_started_at());
    if !current_is_owner {
        return Ok(false);
    }
    let path = paths::kg_runtime_registry_path();
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(true)
}

/// 清理孤儿协调注册表（`starting` / 失效 `ready`）。
///
/// 典型场景：
/// 1. 上次 FastClaw 启动中途崩溃，残留 `state=starting`
/// 2. 残留 `state=ready` 但 owner 进程已死、服务 PID 为 0/已失效
/// 这两种都会导致 `acquire_start_slot` 一直 Busy，知识页无法启动。
pub fn reclaim_orphaned_registry() -> Result<bool, CoordinationError> {
    let _lock = lock_registry()?;
    let Some(current) = read_registry_unlocked()? else {
        return Ok(false);
    };

    let own_pid = std::process::id();
    let own_started = own_process_started_at();
    let is_our_live_claim = current.instance.owner_app_pid == Some(own_pid)
        && current.instance.owner_app_started_at == Some(own_started);
    if is_our_live_claim {
        return Ok(false);
    }

    let owner_pid = current.instance.owner_app_pid;
    let owner_alive = owner_pid.map(pid_exists).unwrap_or(false);
    // 其他仍存活的应用持有槽位时不抢
    if owner_alive {
        return Ok(false);
    }

    let service_pids_dead = [
        current.instance.fastkg.pid,
        current.instance.qdrant.pid,
        current.instance.llama_server.pid,
    ]
    .iter()
    .all(|&pid| pid == 0 || !pid_exists(pid));

    // 保留仍存活的服务身份和消费者证据，只允许删除完全空置的孤儿槽位。
    // 活进程走完整身份校验和 CAS 接管，不能先删凭据再按路径猜测归属。
    if !service_pids_dead || current.leases.iter().any(|lease| pid_exists(lease.app_pid)) {
        return Ok(false);
    }

    let path = paths::kg_runtime_registry_path();
    if path.exists() {
        fs::remove_file(&path)?;
    }
    tracing::warn!(
        "[kg_coordination] 已清理陈旧 {} 注册表 (owner_pid={:?}, starter={}, service_pids_dead={})",
        current.state,
        owner_pid,
        current.instance.starter_app,
        service_pids_dead
    );
    Ok(true)
}

/// 兼容旧名
pub fn reclaim_orphaned_start_slot() -> Result<bool, CoordinationError> {
    reclaim_orphaned_registry()
}

pub fn status_from_registry(registry: Option<&KgRuntimeRegistry>) -> KgSharedStatus {
    let Some(registry) = registry else {
        return KgSharedStatus::none();
    };
    let ownership = if normalize_app(&registry.instance.starter_app).as_deref() == Some(APP_ID)
        && registry.instance.owner_app_pid == Some(std::process::id())
        && registry.instance.owner_app_started_at == Some(own_process_started_at())
    {
        SharedOwnership::SelfManaged
    } else {
        SharedOwnership::SharedManaged
    };
    let shared_instance = ownership != SharedOwnership::SelfManaged;
    KgSharedStatus {
        ownership,
        starter_app: Some(registry.instance.starter_app.clone()),
        lease_count: registry.leases.len(),
        consumers: registry
            .leases
            .iter()
            .map(|lease| lease.app.clone())
            .collect(),
        lease_registered: own_lease_registered(registry),
        shared_instance,
        pending_restart: false,
        state: registry.state.clone(),
        instance_id: Some(registry.instance.instance_id.clone()),
        service_dir: Some(registry.instance.service_dir.clone()),
    }
}

pub fn external_unmanaged_status() -> KgSharedStatus {
    KgSharedStatus {
        ownership: SharedOwnership::ExternalUnmanaged,
        starter_app: None,
        lease_count: 0,
        consumers: Vec::new(),
        lease_registered: false,
        shared_instance: true,
        pending_restart: false,
        state: "ready".to_string(),
        instance_id: None,
        service_dir: None,
    }
}

impl KgSharedStatus {
    pub fn none() -> Self {
        Self {
            ownership: SharedOwnership::None,
            starter_app: None,
            lease_count: 0,
            consumers: Vec::new(),
            lease_registered: false,
            shared_instance: false,
            pending_restart: false,
            state: "none".to_string(),
            instance_id: None,
            service_dir: None,
        }
    }
}

pub fn local_starting_instance() -> SharedKgInstance {
    let fastkg_port = crate::commands::wsp::dependency_runtime_port("fastkg")
        .unwrap_or_else(paths::fastkg_default_port);
    let qdrant_port = crate::commands::wsp::dependency_runtime_port("qdrant")
        .unwrap_or_else(paths::qdrant_default_port);
    let llama_port = crate::commands::wsp::dependency_runtime_port("llama_server")
        .unwrap_or_else(paths::llama_server_default_port);
    SharedKgInstance {
        instance_id: Uuid::new_v4().to_string(),
        starter_app: APP_ID.to_string(),
        owner_app_pid: Some(std::process::id()),
        owner_app_started_at: Some(own_process_started_at()),
        owner_generation: 1,
        service_dir: path_string(paths::fastkg_service_dir()),
        package_version: String::new(),
        fastkg: SharedServiceProcess {
            url: format!("http://127.0.0.1:{}", fastkg_port),
            pid: 0,
            exe: path_string(paths::fastkg_server_path()),
            process_started_at: None,
            extra: serde_json::Map::new(),
        },
        qdrant: SharedServiceProcess {
            url: format!("http://127.0.0.1:{}", qdrant_port),
            pid: 0,
            exe: path_string(paths::qdrant_path()),
            process_started_at: None,
            extra: serde_json::Map::new(),
        },
        llama_server: SharedServiceProcess {
            url: format!("http://127.0.0.1:{}", llama_port),
            pid: 0,
            exe: path_string(paths::llama_server_path()),
            process_started_at: None,
            extra: serde_json::Map::new(),
        },
        extra: serde_json::Map::new(),
    }
}

pub fn local_ready_instance(pids: &crate::state::KgServicePids) -> SharedKgInstance {
    let mut instance = local_starting_instance();
    instance.fastkg.pid = pids.fastkg.unwrap_or(0);
    instance.qdrant.pid = pids.qdrant.unwrap_or(0);
    instance.llama_server.pid = pids.llama_server.unwrap_or(0);
    instance.fastkg.process_started_at = process_started_at(instance.fastkg.pid);
    instance.qdrant.process_started_at = process_started_at(instance.qdrant.pid);
    instance.llama_server.process_started_at = process_started_at(instance.llama_server.pid);
    instance
}

fn default_owner_generation() -> u64 {
    1
}

fn process_started_at(pid: u32) -> Option<DateTime<Utc>> {
    if pid == 0 {
        return None;
    }
    let facts = crate::core::dependency_services::capture_process_facts(&[pid]);
    let started_at = facts.get(&pid)?.process_started_at;
    DateTime::<Utc>::from_timestamp(started_at as i64, 0)
}

fn process_time_matches(actual: u64, expected: DateTime<Utc>) -> bool {
    expected.timestamp() >= 0 && actual == expected.timestamp() as u64
}

fn new_lease() -> KgConsumerLease {
    let now = Utc::now();
    KgConsumerLease {
        lease_id: Uuid::new_v4().to_string(),
        app: APP_ID.to_string(),
        app_pid: std::process::id(),
        process_started_at: own_process_started_at(),
        heartbeat_at: now,
        extra: serde_json::Map::from_iter([("capabilities".into(), serde_json::json!([RECOVERY_CAPABILITY]))]),
    }
}

fn upsert_own_lease(registry: &mut KgRuntimeRegistry) {
    registry
        .leases
        .retain(|lease| normalize_app(&lease.app).as_deref() != Some(APP_ID));
    registry.leases.push(new_lease());
}

fn remove_stale_leases_in_place(registry: &mut KgRuntimeRegistry) -> Vec<KgConsumerLease> {
    let now = Utc::now();
    let mut removed = Vec::new();
    registry.leases.retain(|lease| {
        // 未知消费者也保留，不能把“无法识别”解释为“无人使用”。
        let app = normalize_app(&lease.app);
        let stale_by_time =
            now.signed_duration_since(lease.heartbeat_at).num_seconds() > LEASE_STALE_SECS;
        let stale = stale_by_time && !pid_exists(lease.app_pid);
        if stale {
            removed.push(lease.clone());
        }
        if !stale {
            // Keep the serialized protocol canonical even if the peer wrote mixed case.
            // The clone is handled by normalize_leases_in_place immediately after.
            let _ = app;
        }
        !stale
    });
    removed
}

fn normalize_leases_in_place(registry: &mut KgRuntimeRegistry) {
    let mut fastclaw: Option<KgConsumerLease> = None;
    let mut unknown = Vec::new();
    for mut lease in registry.leases.drain(..) {
        let Some(app) = normalize_app(&lease.app) else {
            unknown.push(lease);
            continue;
        };
        lease.app = app;
        let keep_new = match &fastclaw {
            Some(existing) => {
                let current = lease.app_pid == std::process::id()
                    && lease.process_started_at == own_process_started_at();
                let existing_current = existing.app_pid == std::process::id()
                    && existing.process_started_at == own_process_started_at();
                current || (!existing_current && lease.heartbeat_at > existing.heartbeat_at)
            }
            None => true,
        };
        if keep_new {
            fastclaw = Some(lease);
        }
    }
    registry.leases = fastclaw.into_iter().chain(unknown).collect();
}

fn normalize_app(app: &str) -> Option<String> {
    let app = app.trim().to_ascii_lowercase();
    (app == APP_ID).then_some(app)
}

fn own_process_started_at() -> DateTime<Utc> {
    static STARTED_AT: OnceLock<DateTime<Utc>> = OnceLock::new();
    *STARTED_AT.get_or_init(|| {
        process_started_at(std::process::id())
            .expect("无法读取 FastClaw 当前进程的操作系统启动时间")
    })
}

fn own_lease_registered(registry: &KgRuntimeRegistry) -> bool {
    registry.leases.iter().any(|lease| {
        normalize_app(&lease.app).as_deref() == Some(APP_ID)
            && lease.app_pid == std::process::id()
            && lease.process_started_at == own_process_started_at()
    })
}

fn pid_exists(pid: u32) -> bool {
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    system.process(sysinfo::Pid::from_u32(pid)).is_some()
}

fn path_string(path: PathBuf) -> String {
    path.canonicalize()
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::dependency_services::ProcessFacts;
    use std::collections::HashMap;

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(seconds, 0).expect("valid timestamp")
    }

    fn lease(app: &str, pid: u32, started_at: DateTime<Utc>) -> KgConsumerLease {
        KgConsumerLease {
            lease_id: format!("{}-{}", app, pid),
            app: app.to_string(),
            app_pid: pid,
            process_started_at: started_at,
            heartbeat_at: Utc::now(),
            extra: serde_json::Map::new(),
        }
    }

    fn facts(pid: u32, started_at: DateTime<Utc>) -> (u32, ProcessFacts) {
        (
            pid,
            ProcessFacts {
                process_started_at: started_at.timestamp() as u64,
                canonical_exe_path: PathBuf::from(format!(r"C:\Apps\{}.exe", pid)),
            },
        )
    }

    fn registry_owned_by(app: &str, pid: u32, started_at: DateTime<Utc>) -> KgRuntimeRegistry {
        let mut instance = local_starting_instance();
        instance.instance_id = "instance-a".to_string();
        instance.starter_app = app.to_string();
        instance.owner_app_pid = Some(pid);
        instance.owner_app_started_at = Some(started_at);
        instance.owner_generation = 7;
        KgRuntimeRegistry {
            protocol_version: TARGET_PROTOCOL_VERSION,
            state: "ready".to_string(),
            instance,
            leases: Vec::new(),
            updated_at: Utc::now(),
            extra: serde_json::Map::new(),
        }
    }

    #[test]
    fn none_status_is_stable() {
        let status = KgSharedStatus::none();
        assert_eq!(status.ownership, SharedOwnership::None);
        assert_eq!(status.lease_count, 0);
    }

    #[test]
    fn local_instance_uses_fastclaw_identity() {
        let instance = local_starting_instance();
        assert_eq!(instance.starter_app, APP_ID);
        assert_eq!(instance.fastkg.pid, 0);
    }

    #[test]
    fn own_identity_uses_operating_system_process_start_time() {
        assert_eq!(
            own_process_started_at(),
            process_started_at(std::process::id()).expect("current process start time")
        );
    }

    #[test]
    fn ready_publish_rejects_changed_owner_generation() {
        let proposed = local_starting_instance();
        let mut registry = KgRuntimeRegistry {
            protocol_version: PROTOCOL_VERSION,
            state: "starting".to_string(),
            instance: proposed.clone(),
            leases: vec![],
            updated_at: Utc::now(),
            extra: serde_json::Map::new(),
        };
        assert!(ready_publish_claim_matches(
            &registry,
            &proposed,
            &registry.instance.instance_id,
            registry.instance.owner_generation,
        ));
        let expected_instance_id = registry.instance.instance_id.clone();
        let expected_generation = registry.instance.owner_generation;
        registry.instance.owner_generation += 1;
        assert!(!ready_publish_claim_matches(
            &registry,
            &proposed,
            &expected_instance_id,
            expected_generation,
        ));
    }

    #[test]
    fn ready_publish_rejects_different_instance_owner() {
        let proposed = local_starting_instance();
        let mut registry = KgRuntimeRegistry {
            protocol_version: PROTOCOL_VERSION,
            state: "starting".to_string(),
            instance: proposed.clone(),
            leases: vec![],
            updated_at: Utc::now(),
            extra: serde_json::Map::new(),
        };
        let expected_instance_id = registry.instance.instance_id.clone();
        let expected_generation = registry.instance.owner_generation;
        registry.instance.owner_app_pid = Some(std::process::id().saturating_add(1));
        assert!(!ready_publish_claim_matches(
            &registry,
            &proposed,
            &expected_instance_id,
            expected_generation,
        ));
    }

    #[test]
    fn compatibility_fields_survive_read_write() {
        let mut instance = local_starting_instance();
        instance.extra.insert(
            "peer_extension".to_string(),
            serde_json::json!({"enabled": true}),
        );
        instance.fastkg.extra.insert(
            "protocol_fingerprint".to_string(),
            serde_json::json!("fastkg-v3"),
        );
        let json = serde_json::to_string(&instance).expect("serialize instance");
        let decoded: SharedKgInstance = serde_json::from_str(&json).expect("deserialize instance");
        assert_eq!(
            decoded.extra.get("peer_extension"),
            Some(&serde_json::json!({"enabled": true}))
        );
        assert_eq!(
            decoded.fastkg.extra.get("protocol_fingerprint"),
            Some(&serde_json::json!("fastkg-v3"))
        );
    }

    #[test]
    fn fastclaw_owner_releases_without_handing_off_to_another_app() {
        let own_pid = 1001;
        let own_started = at(100);
        let peer_pid = 2001;
        let peer_started = at(200);
        let mut registry = registry_owned_by(APP_ID, own_pid, own_started);
        registry.leases = vec![
            lease(APP_ID, own_pid, own_started),
            lease("other-app", peer_pid, peer_started),
        ];
        let processes = HashMap::from([facts(own_pid, own_started), facts(peer_pid, peer_started)]);

        let outcome = release_registry_in_place(&mut registry, &processes, own_pid, own_started);

        assert!(matches!(outcome, ReleaseOutcome::Released { remaining: 1 }));
        assert_eq!(registry.instance.starter_app, APP_ID);
        assert_eq!(registry.instance.owner_app_pid, Some(own_pid));
        assert_eq!(registry.leases[0].app, "other-app");
        assert_eq!(registry.state, "ready");
    }

    #[test]
    fn peer_transferred_fastclaw_owner_becomes_ready_to_stop_as_last_consumer() {
        let own_pid = 1002;
        let own_started = at(101);
        let mut registry = registry_owned_by(APP_ID, own_pid, own_started);
        registry.leases = vec![lease(APP_ID, own_pid, own_started)];
        let processes = HashMap::from([facts(own_pid, own_started)]);

        let outcome = release_registry_in_place(&mut registry, &processes, own_pid, own_started);

        assert!(matches!(outcome, ReleaseOutcome::ReadyToStop { .. }));
        assert_eq!(registry.state, "stopping");
        assert!(registry.leases.is_empty());
    }

    #[test]
    fn last_fastclaw_consumer_keeps_lease_while_dead_owner_is_validated() {
        let own_pid = 1003;
        let own_started = at(102);
        let mut registry = registry_owned_by("other-app", 2003, at(202));
        registry.leases = vec![lease(APP_ID, own_pid, own_started)];
        let processes = HashMap::from([facts(own_pid, own_started)]);

        let outcome = release_registry_in_place(&mut registry, &processes, own_pid, own_started);

        assert!(matches!(outcome, ReleaseOutcome::NeedsAdoption { .. }));
        assert_eq!(registry.leases.len(), 1);
        assert_eq!(registry.leases[0].app, APP_ID);
    }

    #[test]
    fn adoption_rejects_reused_owner_pid_and_changed_generation() {
        let own_pid = 1004;
        let own_started = at(103);
        let mut registry = registry_owned_by("other-app", 2004, at(203));
        registry.leases = vec![lease(APP_ID, own_pid, own_started)];
        let snapshot = registry.instance.clone();

        assert!(!adoption_claim_matches(
            &registry,
            &snapshot,
            own_pid,
            own_started,
            false
        ));

        registry.instance.owner_generation += 1;
        assert!(!adoption_claim_matches(
            &registry,
            &snapshot,
            own_pid,
            own_started,
            true
        ));
    }

    #[test]
    fn adoption_accepts_dead_owner_snapshot_with_only_matching_fastclaw_lease() {
        let own_pid = 1006;
        let own_started = at(105);
        let mut registry = registry_owned_by("other-app", 2006, at(205));
        registry.leases = vec![lease(APP_ID, own_pid, own_started)];
        let snapshot = registry.instance.clone();

        assert!(adoption_claim_matches(
            &registry,
            &snapshot,
            own_pid,
            own_started,
            true
        ));
    }

    #[test]
    fn owner_does_not_transfer_to_reused_peer_pid() {
        let own_pid = 1007;
        let own_started = at(106);
        let peer_pid = 2007;
        let peer_started = at(206);
        let mut registry = registry_owned_by(APP_ID, own_pid, own_started);
        registry.leases = vec![
            lease(APP_ID, own_pid, own_started),
            lease("other-app", peer_pid, peer_started),
        ];
        let processes = HashMap::from([
            facts(own_pid, own_started),
            facts(peer_pid, at(peer_started.timestamp() + 1)),
        ]);

        let outcome = release_registry_in_place(&mut registry, &processes, own_pid, own_started);

        assert!(matches!(outcome, ReleaseOutcome::ReadyToStop { .. }));
        assert_eq!(registry.instance.starter_app, APP_ID);
        assert!(registry.leases.is_empty());
    }

    #[test]
    fn invalid_duplicate_peer_lease_does_not_hide_valid_consumer() {
        let own_pid = 1008;
        let own_started = at(107);
        let valid_peer_pid = 2008;
        let valid_peer_started = at(207);
        let mut invalid_peer = lease("other-app", 2009, at(208));
        invalid_peer.heartbeat_at = Utc::now() + chrono::Duration::seconds(1);
        let mut registry = registry_owned_by(APP_ID, own_pid, own_started);
        registry.leases = vec![
            lease(APP_ID, own_pid, own_started),
            lease("other-app", valid_peer_pid, valid_peer_started),
            invalid_peer,
        ];
        let processes = HashMap::from([
            facts(own_pid, own_started),
            facts(valid_peer_pid, valid_peer_started),
        ]);

        let outcome = release_registry_in_place(&mut registry, &processes, own_pid, own_started);

        assert!(matches!(outcome, ReleaseOutcome::Released { remaining: 1 }));
        assert_eq!(registry.instance.starter_app, APP_ID);
        assert_eq!(registry.instance.owner_app_pid, Some(own_pid));
        assert!(registry.leases.iter().any(|lease| lease.app_pid == valid_peer_pid));
    }

    #[test]
    fn registry_delete_requires_current_fastclaw_stopping_owner() {
        let own_pid = 1005;
        let own_started = at(104);
        let mut registry = registry_owned_by(APP_ID, own_pid, own_started);
        assert!(!registry_can_be_deleted(
            &registry,
            "instance-a",
            7,
            own_pid,
            own_started
        ));

        registry.state = "stopping".to_string();
        assert!(registry_can_be_deleted(
            &registry,
            "instance-a",
            7,
            own_pid,
            own_started
        ));
        registry.instance.starter_app = "other-app".to_string();
        assert!(!registry_can_be_deleted(
            &registry,
            "instance-a",
            7,
            own_pid,
            own_started
        ));
    }

    #[test]
    fn fastkg_maintenance_clears_stale_pid_and_publishes_new_identity() {
        let mut registry = registry_owned_by(APP_ID, std::process::id(), own_process_started_at());
        registry.state = "starting".to_string();
        registry.instance.fastkg.pid = 3001;
        registry.instance.fastkg.process_started_at = Some(at(301));
        let claim = FastKgMaintenanceClaim {
            instance_id: registry.instance.instance_id.clone(),
            owner_generation: registry.instance.owner_generation,
            recovery_generation: None,
        };

        assert!(fastkg_maintenance_claim_matches(&registry, &claim));
        apply_fastkg_stopped_in_place(&mut registry);
        assert_eq!(registry.state, "starting");
        assert_eq!(registry.instance.fastkg.pid, 0);
        assert!(registry.instance.fastkg.process_started_at.is_none());

        let new_process = SharedServiceProcess {
            url: "http://127.0.0.1:3002".to_string(),
            pid: 3002,
            exe: r"C:\Apps\FastKG.exe".to_string(),
            process_started_at: Some(at(302)),
            extra: serde_json::Map::new(),
        };
        apply_fastkg_ready_in_place(&mut registry, new_process);
        assert_eq!(registry.state, "ready");
        assert_eq!(registry.instance.fastkg.pid, 3002);
        assert_eq!(registry.instance.fastkg.url, "http://127.0.0.1:3002");
        assert_eq!(registry.instance.fastkg.process_started_at, Some(at(302)));
    }

    #[test]
    fn fastkg_maintenance_claim_rejects_generation_change() {
        let mut registry = registry_owned_by(APP_ID, std::process::id(), own_process_started_at());
        registry.state = "starting".to_string();
        let claim = FastKgMaintenanceClaim {
            instance_id: registry.instance.instance_id.clone(),
            owner_generation: registry.instance.owner_generation,
            recovery_generation: None,
        };
        registry.instance.owner_generation += 1;

        assert!(!fastkg_maintenance_claim_matches(&registry, &claim));
    }

    #[test]
    fn recovery_requires_capability_from_every_consumer() {
        let mut registry = registry_owned_by(APP_ID, std::process::id(), own_process_started_at());
        registry.extra.insert("recovery".into(), serde_json::json!({"phase":"recovering", "generation":1}));
        registry.leases = vec![new_lease(), lease("other-app", 2001, at(200))];
        assert!(!negotiated_recovery_is_active(&registry));
        registry.leases[1].extra.insert("capabilities".into(), serde_json::json!([RECOVERY_CAPABILITY]));
        assert!(negotiated_recovery_is_active(&registry));
        registry.extra.get_mut("recovery").unwrap()["phase"] = serde_json::json!("failed");
        assert!(!negotiated_recovery_is_active(&registry));
    }

    #[test]
    fn recovery_claim_rejects_late_completion() {
        let mut registry = registry_owned_by(APP_ID, std::process::id(), own_process_started_at());
        registry.state = "starting".into();
        registry.extra.insert("recovery".into(), serde_json::json!({"generation":2}));
        let claim = FastKgMaintenanceClaim { instance_id: registry.instance.instance_id.clone(), owner_generation: registry.instance.owner_generation, recovery_generation: Some(1) };
        assert!(!fastkg_maintenance_claim_matches(&registry, &claim));
    }

    #[test]
    fn unknown_live_consumer_prevents_owner_shutdown() {
        let mut registry = registry_owned_by(APP_ID, 1001, at(100));
        registry.leases = vec![lease(APP_ID, 1001, at(100)), lease("unknown-app", 2001, at(200))];
        let processes = HashMap::from([facts(1001, at(100)), facts(2001, at(200))]);
        assert!(matches!(release_registry_in_place(&mut registry, &processes, 1001, at(100)), ReleaseOutcome::Released { remaining: 1 }));
        assert_eq!(registry.leases[0].app, "unknown-app");
    }

    #[test]
    fn consumer_release_during_recovery_does_not_publish_ready() {
        let mut registry = registry_owned_by("other-app", 2001, at(200));
        registry.state = "starting".into();
        registry.leases = vec![lease(APP_ID, 1001, at(100)), lease("other-app", 2001, at(200))];
        let processes = HashMap::from([facts(1001, at(100)), facts(2001, at(200))]);
        assert!(matches!(release_registry_in_place(&mut registry, &processes, 1001, at(100)), ReleaseOutcome::Released { .. }));
        assert_eq!(registry.state, "starting");
    }
}
