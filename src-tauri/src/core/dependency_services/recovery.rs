//! 可独立测试的恢复策略。单调时钟由调用方传入，进程和共享租约由执行层管理。
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub const SERVICE_IDS: [&str; 4] = ["qdrant", "llama_server", "fastkg", "fastplan"];
pub const WINDOW: Duration = Duration::from_secs(600);
pub const STABLE_PERIOD: Duration = Duration::from_secs(60);
pub const WAKE_GRACE: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesiredState {
    Running,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryPhase {
    Idle,
    Queued,
    Starting,
    Healthy,
    Backoff,
    WaitingDependency,
    WaitingSharedOwner,
    Paused,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    PortConflict,
    PortRangeExhausted,
    ProcessExited,
    HealthTimeout,
    DependencyUnavailable,
    NotInstalled,
    InvalidConfiguration,
    PermissionDenied,
    ResourceExhausted,
    OwnershipUnverified,
    SharedMaintenanceBlocked,
    Cancelled,
    Unknown,
}

impl FailureCode {
    pub fn retryable(self) -> bool {
        matches!(
            self,
            Self::PortConflict | Self::ProcessExited | Self::HealthTimeout | Self::Unknown
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoverySettings {
    pub enabled: bool,
    pub auto_start: bool,
}

impl Default for RecoverySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_start: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RecoveryIntent {
    pub desired: DesiredState,
    pub phase: RecoveryPhase,
    pub generation: u64,
    pub settings: RecoverySettings,
    pub session_paused: bool,
    pub attempts: u32,
    pub failure_code: Option<FailureCode>,
    pub last_error: Option<String>,
    pub retry_at: Option<Instant>,
    pub stable_since: Option<Instant>,
    pub ever_ready: bool,
    pub consecutive_health_failures: u32,
    restarts: VecDeque<Instant>,
}

impl Default for RecoveryIntent {
    fn default() -> Self {
        Self {
            desired: DesiredState::Stopped,
            phase: RecoveryPhase::Idle,
            generation: 0,
            settings: RecoverySettings::default(),
            session_paused: false,
            attempts: 0,
            failure_code: None,
            last_error: None,
            retry_at: None,
            stable_since: None,
            ever_ready: false,
            consecutive_health_failures: 0,
            restarts: VecDeque::new(),
        }
    }
}

impl RecoveryIntent {
    /// 重复请求合并；显式 retry 可解除会话熔断，但不修改持久化开关。
    pub fn request_start(&mut self, now: Instant, retry: bool) -> u64 {
        if self.desired == DesiredState::Running && self.phase == RecoveryPhase::Starting {
            return self.generation;
        }
        if !retry
            && self.desired == DesiredState::Running
            && matches!(
                self.phase,
                RecoveryPhase::Queued
                    | RecoveryPhase::Starting
                    | RecoveryPhase::Backoff
                    | RecoveryPhase::Healthy
                    | RecoveryPhase::WaitingDependency
                    | RecoveryPhase::WaitingSharedOwner
            )
        {
            return self.generation;
        }
        self.generation = self.generation.wrapping_add(1);
        self.desired = DesiredState::Running;
        self.session_paused = false;
        self.phase = RecoveryPhase::Queued;
        self.attempts = 0;
        self.restarts.clear();
        self.failure_code = None;
        self.last_error = None;
        self.retry_at = Some(now);
        self.stable_since = None;
        self.consecutive_health_failures = 0;
        self.generation
    }

    pub fn stop(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.desired = DesiredState::Stopped;
        self.phase = RecoveryPhase::Idle;
        self.retry_at = None;
        self.stable_since = None;
        self.consecutive_health_failures = 0;
    }

    pub fn pause(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.session_paused = true;
        self.phase = RecoveryPhase::Paused;
        self.retry_at = None;
    }

    pub fn can_attempt(&self, now: Instant) -> bool {
        self.desired == DesiredState::Running
            && !self.session_paused
            && matches!(
                self.phase,
                RecoveryPhase::Queued | RecoveryPhase::Backoff | RecoveryPhase::WaitingSharedOwner
            )
            && self.retry_at.is_some_and(|time| time <= now)
    }

    pub fn begin_attempt(&mut self, now: Instant) -> Option<u64> {
        if !self.can_attempt(now) {
            return None;
        }
        self.restarts
            .retain(|time| now.saturating_duration_since(*time) < WINDOW);
        if self.ever_ready && self.restarts.len() >= 5 {
            self.phase = RecoveryPhase::Paused;
            self.retry_at = None;
            self.last_error = Some("10 分钟内自动恢复次数已达上限，请检查服务后手动重试".into());
            return None;
        }
        if self.ever_ready {
            self.restarts.push_back(now);
        }
        self.attempts += 1;
        self.phase = RecoveryPhase::Starting;
        self.retry_at = None;
        Some(self.generation)
    }

    pub fn ready(&mut self, generation: u64, now: Instant) -> bool {
        if self.generation != generation
            || self.desired != DesiredState::Running
            || self.session_paused
        {
            return false;
        }
        self.phase = RecoveryPhase::Healthy;
        if !self.ever_ready {
            self.attempts = 0;
        }
        self.ever_ready = true;
        self.stable_since.get_or_insert(now);
        self.consecutive_health_failures = 0;
        self.failure_code = None;
        self.last_error = None;
        self.retry_at = None;
        if self
            .stable_since
            .is_some_and(|start| now.saturating_duration_since(start) >= STABLE_PERIOD)
        {
            self.attempts = 0;
            // 不清空 restarts：短暂稳定不得绕过 10 分钟窗口限额。
        }
        true
    }

    /// 关闭恢复只取消自动重试，不取消用户明确提交的首次启动。
    pub fn disable_pending_retry(&mut self) {
        if matches!(
            self.phase,
            RecoveryPhase::Backoff | RecoveryPhase::WaitingSharedOwner
        ) || (self.phase == RecoveryPhase::WaitingDependency && self.attempts > 0)
        {
            self.phase = RecoveryPhase::Paused;
            self.retry_at = None;
            self.last_error = Some("自动恢复已关闭，可手动启动或立即重试".into());
        }
    }

    pub fn fail(
        &mut self,
        generation: u64,
        now: Instant,
        code: FailureCode,
        message: String,
        global_enabled: bool,
    ) -> bool {
        if generation != self.generation || self.desired != DesiredState::Running {
            return false;
        }
        self.failure_code = Some(code);
        self.last_error = Some(message);
        self.stable_since = None;
        if code == FailureCode::SharedMaintenanceBlocked {
            self.attempts = self.attempts.saturating_sub(1);
            if self.ever_ready {
                self.restarts.pop_back();
            }
            if self.session_paused || !global_enabled || !self.settings.enabled {
                self.phase = RecoveryPhase::Paused;
                self.retry_at = None;
            } else {
                self.phase = RecoveryPhase::WaitingSharedOwner;
                self.retry_at = Some(now + Duration::from_secs(5));
            }
            return true;
        }
        if !code.retryable() {
            self.phase = RecoveryPhase::Failed;
            self.retry_at = None;
        } else if self.session_paused
            || !global_enabled
            || !self.settings.enabled
            || (!self.ever_ready && self.attempts >= 3)
            || (self.ever_ready && self.restarts.len() >= 5)
        {
            self.phase = RecoveryPhase::Paused;
            self.retry_at = None;
        } else {
            let index = if self.ever_ready {
                self.attempts.min(4)
            } else {
                self.attempts.saturating_sub(1).min(4)
            } as usize;
            self.phase = RecoveryPhase::Backoff;
            self.retry_at = Some(now + Duration::from_secs([1, 3, 10, 30, 60][index]));
        }
        true
    }
}

pub fn startup_timeout(service_id: &str, cold: bool) -> Duration {
    Duration::from_secs(match service_id {
        "llama_server" if cold => 180,
        "llama_server" | "fastkg" => 120,
        _ => 60,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cold_start_has_three_total_attempts_with_one_and_three_second_delays() {
        let mut intent = RecoveryIntent::default();
        let now = Instant::now();
        let generation = intent.request_start(now, false);
        intent.begin_attempt(now).unwrap();
        intent.fail(
            generation,
            now,
            FailureCode::HealthTimeout,
            "timeout".into(),
            true,
        );
        assert!(!intent.can_attempt(now));
        let second = now + Duration::from_secs(1);
        intent.begin_attempt(second).unwrap();
        intent.fail(
            generation,
            second,
            FailureCode::ProcessExited,
            "exit".into(),
            true,
        );
        assert_eq!(intent.retry_at, Some(second + Duration::from_secs(3)));
        let third = second + Duration::from_secs(3);
        intent.begin_attempt(third).unwrap();
        intent.fail(
            generation,
            third,
            FailureCode::HealthTimeout,
            "timeout".into(),
            true,
        );
        assert_eq!(intent.phase, RecoveryPhase::Paused);
        assert_eq!(intent.attempts, 3);
        assert!(!intent.can_attempt(third + Duration::from_secs(100)));
    }

    #[test]
    fn user_stop_invalidates_old_completion_and_delayed_retry() {
        let mut intent = RecoveryIntent::default();
        let now = Instant::now();
        let old = intent.request_start(now, false);
        intent.begin_attempt(now);
        intent.stop();
        assert!(!intent.ready(old, now));
        assert!(!intent.fail(old, now, FailureCode::Unknown, "old".into(), true));
        assert!(!intent.can_attempt(now + Duration::from_secs(3600)));
        assert_eq!(intent.desired, DesiredState::Stopped);
    }

    #[test]
    fn duplicate_start_is_coalesced_and_manual_retry_keeps_disabled_setting() {
        let mut intent = RecoveryIntent::default();
        intent.settings.enabled = false;
        let now = Instant::now();
        let generation = intent.request_start(now, false);
        assert_eq!(generation, intent.request_start(now, false));
        intent.begin_attempt(now);
        intent.fail(
            generation,
            now,
            FailureCode::ProcessExited,
            "exit".into(),
            true,
        );
        assert_eq!(intent.phase, RecoveryPhase::Paused);
        assert_ne!(intent.request_start(now, true), generation);
        assert!(!intent.settings.enabled);
        assert!(intent.can_attempt(now));
    }

    #[test]
    fn stable_period_does_not_erase_restart_window() {
        let now = Instant::now();
        let mut intent = RecoveryIntent::default();
        let generation = intent.request_start(now, false);
        intent.ready(generation, now);
        for index in 0..5 {
            let at = now + Duration::from_secs(index * 65);
            intent.phase = RecoveryPhase::Queued;
            intent.retry_at = Some(at);
            intent.begin_attempt(at).unwrap();
            intent.ready(generation, at);
            intent.ready(generation, at + STABLE_PERIOD);
            intent.stable_since = None;
        }
        intent.phase = RecoveryPhase::Queued;
        intent.retry_at = Some(now);
        assert!(intent
            .begin_attempt(now + Duration::from_secs(330))
            .is_none());
        assert_eq!(intent.phase, RecoveryPhase::Paused);
    }

    #[test]
    fn permanent_failure_does_not_loop() {
        let mut intent = RecoveryIntent::default();
        let now = Instant::now();
        let generation = intent.request_start(now, false);
        intent.begin_attempt(now);
        intent.fail(
            generation,
            now,
            FailureCode::NotInstalled,
            "missing".into(),
            true,
        );
        assert_eq!(intent.phase, RecoveryPhase::Failed);
        assert!(!intent.can_attempt(now + WINDOW));
    }

    #[test]
    fn runtime_delays_follow_one_three_ten_thirty_sixty_seconds() {
        let mut intent = RecoveryIntent::default();
        let mut now = Instant::now();
        let generation = intent.request_start(now, false);
        intent.begin_attempt(now);
        intent.ready(generation, now);
        for delay in [1, 3, 10, 30, 60] {
            intent.fail(
                generation,
                now,
                FailureCode::HealthTimeout,
                "timeout".into(),
                true,
            );
            assert_eq!(intent.retry_at, Some(now + Duration::from_secs(delay)));
            now += Duration::from_secs(delay);
            assert!(intent.begin_attempt(now).is_some());
        }
        intent.fail(
            generation,
            now,
            FailureCode::HealthTimeout,
            "timeout".into(),
            true,
        );
        assert_eq!(intent.phase, RecoveryPhase::Paused);
    }

    #[test]
    fn shared_wait_does_not_consume_attempt_budget() {
        let mut intent = RecoveryIntent::default();
        let mut now = Instant::now();
        let generation = intent.request_start(now, false);
        for _ in 0..20 {
            intent.begin_attempt(now).unwrap();
            intent.fail(
                generation,
                now,
                FailureCode::SharedMaintenanceBlocked,
                "old peer".into(),
                true,
            );
            assert_eq!(intent.attempts, 0);
            now += Duration::from_secs(5);
        }
        assert_eq!(intent.phase, RecoveryPhase::WaitingSharedOwner);
    }

    #[test]
    fn disable_recovery_cancels_backoff_but_manual_start_remains_possible() {
        let now = Instant::now();
        let mut intent = RecoveryIntent::default();
        let generation = intent.request_start(now, false);
        intent.begin_attempt(now);
        intent.fail(
            generation,
            now,
            FailureCode::ProcessExited,
            "exit".into(),
            true,
        );
        intent.settings.enabled = false;
        intent.disable_pending_retry();
        assert!(!intent.can_attempt(now + WINDOW));
        intent.request_start(now, true);
        assert!(intent.can_attempt(now));
        assert!(!intent.settings.enabled);
    }

    #[test]
    fn repeated_manual_start_during_spawn_keeps_generation() {
        let now = Instant::now();
        let mut intent = RecoveryIntent::default();
        let generation = intent.request_start(now, false);
        intent.begin_attempt(now);
        assert_eq!(intent.request_start(now, true), generation);
        assert_eq!(intent.phase, RecoveryPhase::Starting);
        intent.pause();
        assert!(!intent.ready(generation, now));
    }
}
