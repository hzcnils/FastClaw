// FastClaw · WSP 健康检查器：定时检查并记录异常，重启由服务管理层协调。
use crate::core::wsp::process::WspProcess;
use crate::core::wsp::types::*;
use crate::db::repo;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

pub struct HealthChecker {
    pool: SqlitePool,
    processes: Arc<RwLock<HashMap<String, WspProcess>>>,
    // 停止时取消包括进行中的 HTTP 探测，并等待任务退出。
    monitors: RwLock<HashMap<String, JoinHandle<()>>>,
}

impl HealthChecker {
    pub fn new(pool: SqlitePool, processes: Arc<RwLock<HashMap<String, WspProcess>>>) -> Self {
        Self {
            pool,
            processes,
            monitors: RwLock::new(HashMap::new()),
        }
    }

    pub async fn wait_until_healthy(&self, service: &WspService) -> bool {
        let url = service.health_url();
        let timeout = tokio::time::Duration::from_millis(service.startup_timeout_ms.max(1) as u64);
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(tokio::time::Duration::from_millis(
                service.health_check_timeout_ms.max(1) as u64,
            ))
            .build()
            .expect("local health client");
        tokio::time::timeout(timeout, async {
            loop {
                if client
                    .get(&url)
                    .send()
                    .await
                    .is_ok_and(|r| r.status().is_success())
                {
                    return true;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
        })
        .await
        .unwrap_or(false)
    }

    async fn replace_monitor(
        &self,
        service_id: String,
        task: impl Future<Output = ()> + Send + 'static,
    ) {
        let mut monitors = self.monitors.write().await;
        if let Some(previous) = monitors.remove(&service_id) {
            previous.abort();
            let _ = previous.await;
        }
        monitors.insert(service_id, tokio::spawn(task));
    }

    pub async fn start_monitoring(&self, service_id: String, service: WspService) {
        let pool = self.pool.clone();
        let processes = self.processes.clone();
        let interval =
            tokio::time::Duration::from_millis(service.health_check_interval_ms.max(1) as u64);
        self.replace_monitor(service_id.clone(), async move {
            let client = reqwest::Client::builder()
                .no_proxy()
                .timeout(tokio::time::Duration::from_millis(
                    service.health_check_timeout_ms.max(1) as u64,
                ))
                .build()
                .expect("local health client");
            let url = service.health_url();
            let mut consecutive_failures = 0;
            loop {
                tokio::time::sleep(interval).await;
                let healthy = client
                    .get(&url)
                    .send()
                    .await
                    .is_ok_and(|r| r.status().is_success());
                if healthy {
                    consecutive_failures = 0;
                    continue;
                }
                consecutive_failures += 1;
                tracing::warn!(
                    "WSP 健康检查失败 [{}]: 连续 {} 次",
                    service.display_name,
                    consecutive_failures
                );
                if consecutive_failures < 3 {
                    continue;
                }

                // 用受管 Child 检查退出状态，不扫描系统进程或阻塞 tokio 线程。
                // HTTP 失败不等于进程崩溃：仍存活时记录 error。
                let (pid, exit_code, exited) = {
                    let mut processes = processes.write().await;
                    match processes.get_mut(&service_id) {
                        Some(process) => {
                            let pid = Some(process.pid() as i32);
                            match process.try_wait() {
                                Ok(Some(status)) => (pid, status.code(), true),
                                _ => (pid, None, false),
                            }
                        }
                        None => (None, None, false),
                    }
                };
                let message = format!("连续 {} 次健康检查失败 ({})", consecutive_failures, url);
                let status = if exited { "crashed" } else { "error" };
                let _ = repo::wsp::update_runtime_status(
                    &pool,
                    &service_id,
                    &WspRuntimeUpdate {
                        status: status.into(),
                        pid,
                        cpu_percent: None,
                        memory_mb: None,
                        gpu_memory_mb: None,
                        started_at: None,
                        stopped_at: exited.then(|| chrono::Utc::now().to_rfc3339()),
                        error_message: Some(message.clone()),
                    },
                )
                .await;
                let _ = repo::wsp::insert_event_log(
                    &pool,
                    &WspEventLog {
                        id: uuid::Uuid::new_v4().to_string(),
                        service_id: service_id.clone(),
                        event_type: if exited {
                            "crash"
                        } else {
                            "health_check_failed"
                        }
                        .into(),
                        pid,
                        exit_code,
                        crash_reason: Some(
                            if exited {
                                "process_exited"
                            } else {
                                "health_check_failed"
                            }
                            .into(),
                        ),
                        cpu_percent: None,
                        memory_mb: None,
                        duration_ms: None,
                        detail: Some(message),
                        created_at: chrono::Utc::now().to_rfc3339(),
                    },
                )
                .await;
                break;
            }
        })
        .await;
    }

    pub async fn stop_monitoring(&self, service_id: &str) {
        let mut monitors = self.monitors.write().await;
        if let Some(task) = monitors.remove(service_id) {
            task.abort();
            let _ = task.await;
        }
    }
}

impl Drop for HealthChecker {
    fn drop(&mut self) {
        for (_, task) in self.monitors.get_mut().drain() {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    struct Dropped(Option<oneshot::Sender<()>>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            if let Some(tx) = self.0.take() {
                let _ = tx.send(());
            }
        }
    }

    fn checker() -> HealthChecker {
        HealthChecker::new(
            SqlitePool::connect_lazy("sqlite::memory:").unwrap(),
            Arc::default(),
        )
    }

    #[tokio::test]
    async fn stopping_waits_for_in_flight_monitor_to_be_cancelled() {
        let checker = checker();
        let (started_tx, started_rx) = oneshot::channel();
        let (dropped_tx, mut dropped_rx) = oneshot::channel();
        checker
            .replace_monitor("test".into(), async move {
                let _guard = Dropped(Some(dropped_tx));
                let _ = started_tx.send(());
                std::future::pending::<()>().await;
            })
            .await;
        started_rx.await.unwrap();
        checker.stop_monitoring("test").await;
        assert!(dropped_rx.try_recv().is_ok());
        assert!(checker.monitors.read().await.is_empty());
        checker.stop_monitoring("test").await;
    }

    #[tokio::test]
    async fn replacing_monitor_cancels_previous_task_before_new_task_runs() {
        let checker = checker();
        let (dropped_tx, dropped_rx) = oneshot::channel();
        let guard = Dropped(Some(dropped_tx));
        checker
            .replace_monitor("test".into(), async move {
                let _guard = guard;
                std::future::pending::<()>().await;
            })
            .await;
        let (observed_tx, observed_rx) = oneshot::channel();
        checker
            .replace_monitor("test".into(), async move {
                let mut dropped_rx = dropped_rx;
                let _ = observed_tx.send(dropped_rx.try_recv().is_ok());
            })
            .await;
        assert!(observed_rx.await.unwrap());
        checker.stop_monitoring("test").await;
    }
}
