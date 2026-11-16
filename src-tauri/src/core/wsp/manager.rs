// FastClaw · WSP 服务管理器
// 负责所有 WSP exe 的生命周期管理

use crate::core::wsp::health::HealthChecker;
use crate::core::wsp::process::WspProcess;
use crate::core::wsp::types::*;
use crate::db::repo;
use crate::utils::paths;
use encoding_rs::GBK;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
//use tokio::io::AsyncReadExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::RwLock;

/// GBK/GB2312 乱码转 UTF-8
fn decode_console_output(bytes: &[u8]) -> String {
    // 先尝试 UTF-8
    if let Ok(s) = String::from_utf8(bytes.to_vec()) {
        return s;
    }
    // UTF-8 失败，尝试 GBK
    let (decoded, _, _) = GBK.decode(bytes);
    decoded.into_owned()
}

/// 启动检测阶段最多保留的输出字节数（超出从头丢弃）
const MAX_STARTUP_OUTPUT_BYTES: usize = 64 * 1024;

/// 追加到有界缓冲，超出上限时丢弃最旧内容
fn append_capped(buf: &mut String, chunk: &str, max_bytes: usize) {
    if chunk.is_empty() {
        return;
    }
    buf.push_str(chunk);
    if buf.len() <= max_bytes {
        return;
    }
    let overflow = buf.len() - max_bytes;
    let mut cut = overflow;
    while cut < buf.len() && !buf.is_char_boundary(cut) {
        cut += 1;
    }
    buf.drain(..cut);
}

/// WSP 服务管理器
pub struct WspManager {
    /// 数据库连接池
    pool: SqlitePool,
    /// 运行中的进程映射：service_id → WspProcess
    processes: Arc<RwLock<HashMap<String, WspProcess>>>,
    /// 健康检查器
    health_checker: Arc<HealthChecker>,
    /// Shimmy 进程（用于 GGUF 模型服务）
    shimmy_process: Arc<RwLock<Option<WspProcess>>>,
}

impl WspManager {
    /// 创建 WSP 管理器
    pub fn new(pool: SqlitePool) -> Self {
        let processes = Arc::new(RwLock::new(HashMap::new()));
        let health_checker = Arc::new(HealthChecker::new(pool.clone(), processes.clone()));
        let shimmy_process = Arc::new(RwLock::new(None));
        Self {
            pool,
            processes,
            health_checker,
            shimmy_process,
        }
    }

    /// 启动指定 WSP 服务
    pub async fn start_service(&self, service_id: &str) -> anyhow::Result<()> {
        let service: WspService = repo::wsp::get_service(&self.pool, service_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("WSP 服务不存在: {}", service_id))?;

        // 打印关键信息，帮助排查启动问题
        eprintln!("[WSP] ========== 开始启动服务 ==========");
        eprintln!("[WSP] ID: {}", service.id);
        eprintln!("[WSP] 名称: {}", service.display_name);
        eprintln!("[WSP] exe_path: {}", service.exe_path);
        eprintln!("[WSP] 端口: {}", service.port);
        eprintln!("[WSP] host: {}", service.host);
        eprintln!(
            "[WSP] 文件存在: {}",
            std::path::Path::new(&service.exe_path).exists()
        );
        eprintln!("[WSP] =====================================");

        // ========== GGUF 模型服务特殊处理 ==========
        if self.is_gguf_service(&service) {
            tracing::info!("检测到 GGUF 模型服务: {}", service.display_name);
            return self.start_gguf_service(service_id, &service).await;
        }

        eprintln!("[WSP] 非 GGUF 模型，走普通进程启动流程");
        tracing::info!("启动普通 WSP 服务: {}", service.display_name);

        // 2. 检查端口
        if self.is_port_in_use(service.port).await {
            return Err(anyhow::anyhow!("启动失败: 端口 {} 已被占用", service.port));
        }
        // 检查依赖服务是否已启动
        for dep_name in service.parsed_depends_on() {
            let dep_running = self.is_service_running_by_name(&dep_name).await;
            if !dep_running {
                return Err(anyhow::anyhow!("依赖服务 {} 未启动", dep_name));
            }
        }

        // 解析 exe、args、工作目录 —— 统一处理"真实文件"和"命令字符串"两种形式
        eprintln!("[WSP] 检查 exe 文件: {}", service.exe_path);
        let (resolved_exe, resolved_args, resolved_work_dir) =
            if std::path::Path::new(&service.exe_path).exists() {
                // ── exe 文件存在：防御性 GGUF 二次检测 ──
                let is_gguf_ext = service.exe_path.to_lowercase().ends_with(".gguf");
                let is_gguf_magic = std::fs::File::open(&service.exe_path)
                    .map(|mut f| {
                        use std::io::Read;
                        let mut buf = [0u8; 4];
                        f.read_exact(&mut buf).is_ok() && &buf == b"GGUF"
                    })
                    .unwrap_or(false);
                if is_gguf_ext || is_gguf_magic {
                    tracing::info!("检测到 GGUF 文件（第二阶段检查），重定向到 GGUF 启动流程");
                    return self.start_gguf_service(service_id, &service).await;
                }
                let work_dir = std::path::Path::new(&service.exe_path)
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                (service.exe_path.clone(), service.parsed_args(), work_dir)
            } else {
                // ── exe 不存在：把 exe_path 当完整命令行拆分 ──
                eprintln!("[WSP] exe 文件不存在，解析命令行: {}", service.exe_path);
                let parts: Vec<&str> = service.exe_path.split_whitespace().collect();
                if parts.is_empty() {
                    return Err(anyhow::anyhow!("启动命令为空"));
                }
                let exe = parts[0].to_string();
                let args = match &service.args {
                    Some(a) if !a.is_empty() => serde_json::from_str(a)
                        .unwrap_or_else(|_| parts[1..].iter().map(|s| s.to_string()).collect()),
                    _ => parts[1..].iter().map(|s| s.to_string()).collect(),
                };
                let work_dir = match &service.work_dir {
                    Some(wd) if !wd.is_empty() => std::path::PathBuf::from(wd),
                    _ => std::path::Path::new(&exe)
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
                };
                (exe, args, work_dir)
            };

        // 标记为 starting
        repo::wsp::update_runtime_status(
            &self.pool,
            service_id,
            &WspRuntimeUpdate {
                status: "starting".to_string(),
                pid: None,
                cpu_percent: None,
                memory_mb: None,
                gpu_memory_mb: None,
                started_at: Some(chrono::Utc::now().to_rfc3339()),
                stopped_at: None,
                error_message: None,
            },
        )
        .await?;

        self.spawn_and_monitor(
            service_id,
            &service,
            &resolved_exe,
            &resolved_args,
            &resolved_work_dir,
        )
        .await
    }

    /// 构建命令、启动子进程并监控输出，等待服务就绪后写入最终运行状态。
    async fn spawn_and_monitor(
        &self,
        service_id: &str,
        service: &WspService,
        exe: &str,
        args: &[String],
        work_dir: &std::path::Path,
    ) -> anyhow::Result<()> {
        // ── 构建命令 ──
        let mut command = Command::new(exe);
        command
            .args(args)
            .current_dir(work_dir)
            .env("PYTHONIOENCODING", "utf-8")
            .env("PYTHONUTF8", "1")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        #[cfg(windows)]
        {
            command.as_std_mut().creation_flags(0x08000000);
        }
        for (k, v) in service.parsed_env_vars() {
            command.env(k, v);
        }

        // ── 启动进程 ──
        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => {
                let err_msg = format!("进程启动失败：{}", e);
                repo::wsp::update_runtime_status(
                    &self.pool,
                    service_id,
                    &WspRuntimeUpdate {
                        status: "error".to_string(),
                        pid: None,
                        cpu_percent: None,
                        memory_mb: None,
                        gpu_memory_mb: None,
                        started_at: None,
                        stopped_at: Some(chrono::Utc::now().to_rfc3339()),
                        error_message: Some(err_msg.clone()),
                    },
                )
                .await?;
                return Err(anyhow::anyhow!("{}", err_msg));
            }
        };

        // ── 瞬间崩溃检测（spawn 返回后立即 try_wait，无需等待） ──
        match child.try_wait() {
            Ok(Some(status)) => {
                let err_msg = format!(
                    "进程启动后立即崩溃，退出码：{}",
                    status.code().unwrap_or(-1)
                );
                repo::wsp::update_runtime_status(
                    &self.pool,
                    service_id,
                    &WspRuntimeUpdate {
                        status: "error".to_string(),
                        pid: None,
                        cpu_percent: None,
                        memory_mb: None,
                        gpu_memory_mb: None,
                        started_at: None,
                        stopped_at: Some(chrono::Utc::now().to_rfc3339()),
                        error_message: Some(err_msg.clone()),
                    },
                )
                .await?;
                return Err(anyhow::anyhow!("{}", err_msg));
            }
            Err(e) => {
                let err_msg = format!("进程状态检测异常：{}", e);
                repo::wsp::update_runtime_status(
                    &self.pool,
                    service_id,
                    &WspRuntimeUpdate {
                        status: "error".to_string(),
                        pid: None,
                        cpu_percent: None,
                        memory_mb: None,
                        gpu_memory_mb: None,
                        started_at: None,
                        stopped_at: Some(chrono::Utc::now().to_rfc3339()),
                        error_message: Some(err_msg.clone()),
                    },
                )
                .await?;
                return Err(anyhow::anyhow!("{}", err_msg));
            }
            Ok(None) => {} // 进程正在运行，继续
        }

        let pid = child.id().unwrap_or(0);

        // ── 异步收集 stdout / stderr（启动期有界缓冲；之后只读丢弃防管道阻塞）──
        let output_buffer: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
        let collecting = Arc::new(AtomicBool::new(true));

        let stdout = child.stdout.take().expect("stdout pipe missing");
        let stderr = child.stderr.take().expect("stderr pipe missing");

        let out_buf = output_buffer.clone();
        let out_collecting = collecting.clone();
        let sid_out = service_id.to_string();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut buf = vec![0u8; 4096];
            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let s = decode_console_output(&buf[..n]);
                        println!("[{sid_out} OUT] {}", s.trim_end());
                        if out_collecting.load(Ordering::Relaxed) {
                            let mut guard = out_buf.lock().unwrap();
                            append_capped(&mut guard, &s, MAX_STARTUP_OUTPUT_BYTES);
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let err_buf = output_buffer.clone();
        let err_collecting = collecting.clone();
        let sid_err = service_id.to_string();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut buf = vec![0u8; 4096];
            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let s = decode_console_output(&buf[..n]);
                        eprintln!("[{sid_err} ERR] {}", s.trim_end());
                        if err_collecting.load(Ordering::Relaxed) {
                            let mut guard = err_buf.lock().unwrap();
                            append_capped(&mut guard, &s, MAX_STARTUP_OUTPUT_BYTES);
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // ── 保存进程句柄 ──
        {
            let mut procs = self.processes.write().await;
            procs.insert(service_id.to_string(), WspProcess::new(child));
        }

        // ── 最多等待 30s，检测首条输出 ──
        let mut final_output = String::new();
        let mut has_output = false;
        for i in 0..30u32 {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            let snap = output_buffer.lock().unwrap().clone();
            if !snap.is_empty() {
                println!("[WSP] 第 {} 秒检测到输出，再等 2s 收尾", i + 1);
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                final_output = output_buffer.lock().unwrap().clone();
                has_output = true;
                break;
            }
        }

        // 启动检测结束：停止写入共享缓冲，后续输出只读丢弃（防管道堵 + 防内存涨）
        collecting.store(false, Ordering::Relaxed);
        if let Ok(mut guard) = output_buffer.lock() {
            guard.clear();
            guard.shrink_to_fit();
        }

        // ── 进程存活二次确认 ──
        let is_alive = {
            let mut procs = self.processes.write().await;
            procs
                .get_mut(service_id)
                .map(|p| matches!(p.try_wait(), Ok(None)))
                .unwrap_or(false)
        };
        if !is_alive {
            let last = final_output.lines().last().unwrap_or("无输出").to_string();
            let err_msg = if has_output {
                format!("进程在启动期间已退出，末行输出：{}", last)
            } else {
                "进程在初始化期间静默退出".to_string()
            };
            repo::wsp::update_runtime_status(
                &self.pool,
                service_id,
                &WspRuntimeUpdate {
                    status: "error".to_string(),
                    pid: Some(pid as i32),
                    cpu_percent: None,
                    memory_mb: None,
                    gpu_memory_mb: None,
                    started_at: None,
                    stopped_at: Some(chrono::Utc::now().to_rfc3339()),
                    error_message: Some(err_msg.clone()),
                },
            )
            .await?;
            return Err(anyhow::anyhow!("{}", err_msg));
        }

        // ── 授权类错误关键词检测（只检测明确失败，其余一律视为成功） ──
        let has_auth_error = final_output.contains("授权")
            && (final_output.contains("不存在") || final_output.contains("失败"));
        let has_license_error =
            final_output.contains("授权失败") || final_output.contains("授权签名验证失败");

        if has_auth_error || has_license_error {
            let msg = if has_auth_error {
                "授权文件不存在".to_string()
            } else {
                "授权签名验证失败".to_string()
            };
            println!("❌ [WSP] {}", msg);
            repo::wsp::update_runtime_status(
                &self.pool,
                service_id,
                &WspRuntimeUpdate {
                    status: "error".to_string(),
                    pid: Some(pid as i32),
                    cpu_percent: None,
                    memory_mb: None,
                    gpu_memory_mb: None,
                    started_at: None,
                    stopped_at: Some(chrono::Utc::now().to_rfc3339()),
                    error_message: Some(msg.clone()),
                },
            )
            .await?;
            return Err(anyhow::anyhow!("{}", msg));
        }

        // ── 无输出时做非阻塞单次 health ping（仅作提示，不阻塞成功） ──
        let error_message: Option<String> = if !has_output {
            let health_url = service.health_url();
            let has_valid_url = !health_url.is_empty() && health_url != "http://127.0.0.1:0/health";
            if has_valid_url {
                let client = reqwest::Client::builder()
                    .timeout(tokio::time::Duration::from_millis(500))
                    .build()
                    .unwrap_or_default();
                let ok = client
                    .get(&health_url)
                    .send()
                    .await
                    .map(|r| r.status().is_success())
                    .unwrap_or(false);
                if ok {
                    None
                } else {
                    Some("健康端点暂未响应，服务可能仍在初始化".to_string())
                }
            } else {
                None
            }
        } else {
            None
        };

        // ── 写入 running 状态，启动持续监控 ──
        println!("✅ [WSP] 服务启动成功，PID: {}", pid);
        repo::wsp::update_runtime_status(
            &self.pool,
            service_id,
            &WspRuntimeUpdate {
                status: "running".to_string(),
                pid: Some(pid as i32),
                cpu_percent: None,
                memory_mb: None,
                gpu_memory_mb: None,
                started_at: Some(chrono::Utc::now().to_rfc3339()),
                stopped_at: None,
                error_message,
            },
        )
        .await?;
        self.health_checker
            .start_monitoring(service_id.to_string(), service.clone()).await;
        Ok(())
    }

    /// 停止指定 WSP 服务
    pub async fn stop_service(&self, service_id: &str) -> anyhow::Result<()> {
        // ========== GGUF 模型服务特殊处理 ==========
        if let Ok(Some(service)) = repo::wsp::get_service(&self.pool, service_id).await {
            if self.is_gguf_service(&service) {
                return self.stop_gguf_service(service_id).await;
            }
        }

        // 停止健康检查
        self.health_checker.stop_monitoring(service_id).await;

        // 更新状态
        repo::wsp::update_runtime_status(
            &self.pool,
            service_id,
            &WspRuntimeUpdate {
                status: "stopping".to_string(),
                pid: None,
                cpu_percent: None,
                memory_mb: None,
                gpu_memory_mb: None,
                started_at: None,
                stopped_at: None,
                error_message: None,
            },
        )
        .await?;

        // 杀掉进程
        self.kill_process(service_id).await;

        // 清理 PyInstaller _MEI 临时目录
        if let Ok(Some(service)) = repo::wsp::get_service(&self.pool, service_id).await {
            let mut dirs_to_check = vec![std::env::temp_dir()];
            // 添加 C:\Temp（某些 PyInstaller 打包的程序解压到这里）
            dirs_to_check.push(std::path::PathBuf::from("C:\\Temp"));
            if let Some(ref wd) = service.work_dir {
                dirs_to_check.push(std::path::PathBuf::from(wd));
            }
            // 添加模型文件所在目录
            if !service.exe_path.is_empty() {
                if let Some(parent) = std::path::Path::new(&service.exe_path).parent() {
                    dirs_to_check.push(parent.to_path_buf());
                }
            }
            for dir in &dirs_to_check {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name();
                        let name_str = name.to_string_lossy();
                        if name_str.starts_with("_MEI") {
                            let path = entry.path();
                            if path.is_dir() {
                                let _ = std::fs::remove_dir_all(&path);
                                log::info!("[WSP] 已清理临时目录: {:?}", path);
                            }
                        }
                    }
                }
            }
        }

        // 更新为 stopped
        repo::wsp::update_runtime_status(
            &self.pool,
            service_id,
            &WspRuntimeUpdate {
                status: "stopped".to_string(),
                pid: None,
                cpu_percent: None,
                memory_mb: None,
                gpu_memory_mb: None,
                started_at: None,
                stopped_at: Some(chrono::Utc::now().to_rfc3339()),
                error_message: None,
            },
        )
        .await?;

        // 记录停止事件
        repo::wsp::insert_event_log(
            &self.pool,
            &WspEventLog {
                id: uuid::Uuid::new_v4().to_string(),
                service_id: service_id.to_string(),
                event_type: "stop".to_string(),
                pid: None,
                exit_code: None,
                crash_reason: None,
                cpu_percent: None,
                memory_mb: None,
                duration_ms: None,
                detail: None,
                created_at: chrono::Utc::now().to_rfc3339(),
            },
        )
        .await?;

        tracing::info!("WSP 服务已停止: {}", service_id);
        Ok(())
    }

    /// 停止 GGUF 模型服务
    async fn stop_gguf_service(&self, service_id: &str) -> anyhow::Result<()> {
        // 从进程映射中移除
        {
            let mut procs = self.processes.write().await;
            procs.remove(service_id);
        }

        // 更新状态为 stopped
        repo::wsp::update_runtime_status(
            &self.pool,
            service_id,
            &WspRuntimeUpdate {
                status: "stopped".to_string(),
                pid: None,
                cpu_percent: None,
                memory_mb: None,
                gpu_memory_mb: None,
                started_at: None,
                stopped_at: Some(chrono::Utc::now().to_rfc3339()),
                error_message: None,
            },
        )
        .await?;

        // 记录停止事件
        repo::wsp::insert_event_log(
            &self.pool,
            &WspEventLog {
                id: uuid::Uuid::new_v4().to_string(),
                service_id: service_id.to_string(),
                event_type: "stop".to_string(),
                pid: None,
                exit_code: None,
                crash_reason: None,
                cpu_percent: None,
                memory_mb: None,
                duration_ms: None,
                detail: None,
                created_at: chrono::Utc::now().to_rfc3339(),
            },
        )
        .await?;

        // 检查是否还有其他运行中的 GGUF 服务
        let remaining_gguf_count = self.get_running_gguf_count().await;

        if remaining_gguf_count == 0 {
            // 没有其他 GGUF 服务了，停止 Shimmy
            tracing::info!("没有其他 GGUF 服务运行，停止 Shimmy");
            self.stop_shimmy().await;
        } else {
            // 还有其他 GGUF 服务在运行，不停止 Shimmy
            tracing::info!(
                "还有 {} 个 GGUF 服务在运行，保留 Shimmy",
                remaining_gguf_count
            );
        }

        tracing::info!("GGUF 服务已停止: {}", service_id);
        Ok(())
    }

    /// 重启指定 WSP 服务
    pub async fn restart_service(&self, service_id: &str) -> anyhow::Result<()> {
        self.stop_service(service_id).await?;

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        self.start_service(service_id).await?;
        Ok(())
    }

    /// 启动所有已启用的服务
    pub async fn start_all(&self) -> anyhow::Result<()> {
        let services = repo::wsp::get_enabled_services(&self.pool).await?;
        tracing::info!("启动所有 WSP 服务 (共 {} 个)...", services.len());

        for service in services {
            // 检查是否已经在运行
            if self.is_service_running(&service.id).await {
                continue;
            }

            // 按 start_delay_ms 延迟启动
            if service.start_delay_ms > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(
                    service.start_delay_ms as u64,
                ))
                .await;
            }

            if let Err(e) = self.start_service(&service.id).await {
                tracing::error!("WSP 启动失败 [{}]: {}", service.display_name, e);
            }
        }
        Ok(())
    }

    /// 自动启动所有标记为 auto_start 的服务
    pub async fn start_all_auto(&self) -> anyhow::Result<()> {
        let services = repo::wsp::get_auto_start_services(&self.pool).await?;
        tracing::info!("自动启动 WSP 服务 (共 {} 个)...", services.len());

        for service in services {
            // 检查是否已经在运行
            if self.is_service_running(&service.id).await {
                continue;
            }

            // 按 start_delay_ms 延迟启动
            if service.start_delay_ms > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(
                    service.start_delay_ms as u64,
                ))
                .await;
            }

            if let Err(e) = self.start_service(&service.id).await {
                tracing::error!("WSP 自动启动失败 [{}]: {}", service.display_name, e);
            }
        }
        Ok(())
    }

    /// 停止所有运行中的服务
    pub async fn stop_all(&self) -> anyhow::Result<()> {
        // 1. 从内存中的进程映射获取
        let ids: Vec<String> = {
            let procs = self.processes.read().await;
            procs.keys().cloned().collect()
        };

        for id in ids {
            if let Err(e) = self.stop_service(&id).await {
                tracing::error!("WSP 停止失败 [{}]: {}", id, e);
            }
        }

        // 2. 检查数据库中是否还有处于 'running' 或 'starting' 状态但不在内存中的服务
        // 这通常发生在程序异常退出后重启的情况
        let services = repo::wsp::list_services_with_status(&self.pool).await?;
        for svc in services {
            // 使用 matches! 宏来优雅地匹配 Option 的内容
            if matches!(
                svc.runtime_status.as_deref(),
                Some("running") | Some("starting")
            ) {
                // 如果内存中没有，说明是孤立状态，更新数据库状态为 stopped
                let is_in_memory = {
                    let procs = self.processes.read().await;
                    procs.contains_key(&svc.id)
                };

                if !is_in_memory {
                    tracing::warn!("清理孤立的 WSP 服务状态: {}", svc.id);
                    repo::wsp::update_runtime_status(
                        &self.pool,
                        &svc.id,
                        &WspRuntimeUpdate {
                            status: "stopped".to_string(),
                            pid: None,
                            cpu_percent: None,
                            memory_mb: None,
                            gpu_memory_mb: None,
                            started_at: None,
                            stopped_at: Some(chrono::Utc::now().to_rfc3339()),
                            error_message: None,
                        },
                    )
                    .await?;
                }
            }
        }

        Ok(())
    }

    /// 完全关闭：停止所有 WSP 服务、Shimmy 等子进程
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        if let Err(e) = self.stop_all().await {
            tracing::error!("停止 WSP 服务失败: {}", e);
        }
        self.stop_shimmy().await;
        Ok(())
    }

    /// 检查服务是否正在运行
    async fn is_service_running(&self, service_id: &str) -> bool {
        let procs = self.processes.read().await;
        procs.contains_key(service_id)
    }

    /// 调用 WSP 服务的 API 端点
    pub async fn call_endpoint(
        &self,
        service_id: &str,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> anyhow::Result<reqwest::Response> {
        let service = repo::wsp::get_service(&self.pool, service_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("WSP 服务不存在"))?;

        let url = format!("{}{}", service.base_url(), path);
        let client = reqwest::Client::new();

        let req = match method.to_uppercase().as_str() {
            "GET" => client.get(&url),
            "POST" => client.post(&url),
            "PUT" => client.put(&url),
            "DELETE" => client.delete(&url),
            _ => return Err(anyhow::anyhow!("不支持的 HTTP 方法: {}", method)),
        };

        let req = if let Some(b) = body {
            req.json(&b)
        } else {
            req
        };

        let resp = req.send().await?;
        Ok(resp)
    }

    // ─── 内部辅助方法 ───

    /// 杀掉进程并从映射中移除
    async fn kill_process(&self, service_id: &str) {
        // let mut procs = self.processes.write().await;
        // if let Some(mut process) = procs.remove(service_id) {
        //     let _ = process.kill().await;
        // }
        let mut procs = self.processes.write().await;

        if let Some(process) = procs.remove(service_id) {
            // 1. 使用 pid() 方法获取进程ID (根据报错提示修改)
            // 注意：pid() 通常直接返回 u32，而不是 Option<u32>
            let pid = process.pid();

            // 2. 使用 Windows taskkill 强制终止进程树
            let mut cmd = Command::new("taskkill");
            cmd.args(&["/F", "/T", "/PID", &pid.to_string()]);
            #[cfg(target_os = "windows")]
            cmd.creation_flags(0x08000000);
            let _ = cmd.output();
        }
    }

    /// 检查服务是否正在运行（按名称查）
    async fn is_service_running_by_name(&self, name: &str) -> bool {
        if let Ok(Some(svc)) = repo::wsp::get_service_by_name(&self.pool, name).await {
            let procs = self.processes.read().await;
            procs.contains_key(&svc.id)
        } else {
            false
        }
    }

    /// 检查端口是否被占用
    async fn is_port_in_use(&self, port: i32) -> bool {
        tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .is_ok()
    }

    // ========== GGUF 模型服务特殊处理方法 ==========

    /// 判断是否是 GGUF 模型服务
    fn is_gguf_service(&self, service: &WspService) -> bool {
        // 检查 exe_path 是否以 .gguf 结尾
        if service.exe_path.to_lowercase().ends_with(".gguf") {
            return true;
        }

        // 检查 args 中是否包含 .gguf 文件路径
        if let Some(args_str) = &service.args {
            if args_str.to_lowercase().contains(".gguf") {
                return true;
            }
        }

        // 检查 extra_config 中是否包含 GGUF 相关配置
        if let Some(extra) = &service.extra_config {
            if extra.to_lowercase().contains(".gguf") || extra.to_lowercase().contains("gguf") {
                return true;
            }
        }

        // 检查文件魔数：GGUF 文件以 "GGUF" 开头（0x47 0x47 0x55 0x46）
        if let Ok(mut f) = std::fs::File::open(&service.exe_path) {
            use std::io::Read;
            let mut buf = [0u8; 4];
            if f.read_exact(&mut buf).is_ok() && &buf == b"GGUF" {
                return true;
            }
        }

        false
    }

    /// 从服务配置中提取 GGUF 模型文件路径
    fn extract_gguf_path(&self, service: &WspService) -> Option<String> {
        // 首先检查 exe_path
        if service.exe_path.to_lowercase().ends_with(".gguf") {
            return Some(service.exe_path.clone());
        }

        // 检查 args 中是否包含 .gguf 文件路径
        if let Some(args_str) = &service.args {
            let args: Vec<String> = match serde_json::from_str(args_str) {
                Ok(v) => v,
                Err(_) => return None,
            };

            for arg in args {
                if arg.to_lowercase().ends_with(".gguf") {
                    return Some(arg);
                }
            }
        }

        None
    }

    /// 启动 GGUF 模型服务
    async fn start_gguf_service(
        &self,
        service_id: &str,
        service: &WspService,
    ) -> anyhow::Result<()> {
        tracing::info!(
            "========== 启动 GGUF 服务: {} ==========",
            service.display_name
        );

        // 获取 GGUF 模型文件路径
        let model_path = self.extract_gguf_path(service);
        tracing::info!("GGUF 模型路径: {:?}", model_path);

        // 检查 Shimmy 是否已经启动
        let shimmy_running = self.is_shimmy_running().await;
        tracing::info!("Shimmy 当前运行状态: {}", shimmy_running);

        if !shimmy_running {
            // Shimmy 未启动，需要先启动 Shimmy
            tracing::info!("Shimmy 未启动，正在启动...");
            match self.start_shimmy(model_path.as_deref()).await {
                Ok(_) => tracing::info!("Shimmy 启动成功"),
                Err(e) => {
                    tracing::error!("Shimmy 启动失败: {}", e);
                    return Err(anyhow::anyhow!("Shimmy 启动失败: {}", e));
                }
            }
        } else {
            tracing::info!("Shimmy 已启动，直接设置服务状态");
        }

        // 获取 Shimmy 进程的实际 PID
        let pid = {
            let shimmy = self.shimmy_process.read().await;
            shimmy.as_ref().map(|p| p.pid() as i32)
        };

        tracing::info!("Shimmy 进程 PID: {:?}", pid);

        // 更新服务状态为 running
        repo::wsp::update_runtime_status(
            &self.pool,
            service_id,
            &WspRuntimeUpdate {
                status: "running".to_string(),
                pid,
                cpu_percent: None,
                memory_mb: None,
                gpu_memory_mb: None,
                started_at: Some(chrono::Utc::now().to_rfc3339()),
                stopped_at: None,
                error_message: None,
            },
        )
        .await?;

        // 保存到进程映射（使用服务ID）
        {
            let mut procs = self.processes.write().await;
            // 创建一个虚拟进程对象（GGUF 服务不直接运行，通过 Shimmy 提供服务）
            procs.insert(service_id.to_string(), WspProcess::new_virtual());
        }

        tracing::info!("GGUF 服务已启动: {}", service.display_name);
        Ok(())
    }

    /// 检查 Shimmy 是否正在运行
    async fn is_shimmy_running(&self) -> bool {
        let shimmy = self.shimmy_process.read().await;
        shimmy.is_some()
    }

    /// 启动 Shimmy 进程（自动扫描 GGUF 模型）
    async fn start_shimmy(&self, model_path: Option<&str>) -> anyhow::Result<()> {
        const SHIMMY_PORT: i32 = 11435;

        let shimmy_exe = paths::shimmy_server_path();
        let shimmy_path_str = shimmy_exe.to_string_lossy().to_string();

        // 检查 Shimmy 可执行文件是否存在
        if !shimmy_exe.exists() {
            let err_msg = format!("Shimmy 可执行文件不存在: {}", shimmy_path_str);
            tracing::error!("{}", err_msg);
            return Err(anyhow::anyhow!("{}", err_msg));
        }

        let shimmy_path = shimmy_path_str.as_str();

        // 检查端口是否被占用
        if self.is_port_in_use(SHIMMY_PORT).await {
            return Err(anyhow::anyhow!("Shimmy 端口 {} 已被占用，未终止占用进程，请释放端口后重试", SHIMMY_PORT));
        }

        // 获取 Shimmy 所在目录
        let shimmy_dir = match std::path::Path::new(shimmy_path).parent() {
            Some(dir) => dir,
            None => return Err(anyhow::anyhow!("无法获取 Shimmy 目录路径")),
        };

        tracing::info!("Shimmy 目录: {:?}", shimmy_dir);

        // 自动扫描 GGUF 模型目录
        let mut model_dirs = std::collections::HashSet::new();

        // 1. 扫描 shimmy 目录下的所有子目录（可能包含 GGUF 模型）
        if let Ok(entries) = std::fs::read_dir(shimmy_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // 检查目录是否包含 .gguf 文件
                    if let Ok(sub_entries) = std::fs::read_dir(&path) {
                        for sub_entry in sub_entries.flatten() {
                            let sub_path = sub_entry.path();
                            if sub_path
                                .extension()
                                .map_or(false, |ext| ext.eq_ignore_ascii_case("gguf"))
                            {
                                model_dirs.insert(path.to_string_lossy().to_string());
                                tracing::info!("[Shimmy] 发现 GGUF 模型目录: {:?}", path);
                                break;
                            }
                        }
                    }
                }
            }
        }

        // 2. 如果调用者指定了模型路径，也添加
        if let Some(ref custom_path) = model_path {
            let custom_dir = std::path::Path::new(custom_path);
            if let Some(parent) = custom_dir.parent() {
                let dir_str = parent.to_string_lossy().to_string();
                if std::path::Path::new(&dir_str).exists() {
                    model_dirs.insert(dir_str);
                }
            } else if custom_dir.exists() {
                model_dirs.insert(custom_path.to_string());
            }
        }

        // 构建启动命令: Shimmy.exe serve
        let mut command = tokio::process::Command::new(shimmy_path);
        command.arg("serve").current_dir(shimmy_dir);
        #[cfg(target_os = "windows")]
        command.creation_flags(0x08000000);

        // 添加所有模型目录（合并为一个参数）
        let dirs_str: Vec<String> = model_dirs.iter().cloned().collect();
        if !dirs_str.is_empty() {
            command
                .arg("--model-dirs")
                .arg(dirs_str.join(if cfg!(target_os = "windows") {
                    ";"
                } else {
                    ":"
                }));
        }

        let cmd_str = if !dirs_str.is_empty() {
            format!(
                "{} serve --model-dirs {}",
                shimmy_path,
                dirs_str.join(if cfg!(target_os = "windows") {
                    ";"
                } else {
                    ":"
                })
            )
        } else {
            format!("{} serve", shimmy_path)
        };
        tracing::info!("启动命令: {}", cmd_str);

        #[cfg(windows)]
        {
            command.as_std_mut().creation_flags(0x08000000);
        }
        let mut child = match command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => return Err(anyhow::anyhow!("启动 Shimmy 失败: {}", e)),
        };

        tracing::info!("Shimmy 进程已创建，PID: {:?}", child.id());

        // 异步读取进程输出
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        tokio::spawn(async move {
            if let Some(stdout) = stdout {
                let mut reader = tokio::io::BufReader::new(stdout);
                let mut line = String::new();
                while let Ok(n) = reader.read_line(&mut line).await {
                    if n == 0 {
                        break;
                    }
                    tracing::info!("[Shimmy stdout] {}", line.trim());
                    line.clear();
                }
            }
        });

        tokio::spawn(async move {
            if let Some(stderr) = stderr {
                let mut reader = tokio::io::BufReader::new(stderr);
                let mut line = String::new();
                while let Ok(n) = reader.read_line(&mut line).await {
                    if n == 0 {
                        break;
                    }
                    tracing::error!("[Shimmy stderr] {}", line.trim());
                    line.clear();
                }
            }
        });

        // 保存 Shimmy 进程
        {
            let mut shimmy = self.shimmy_process.write().await;
            *shimmy = Some(WspProcess::new(child));
        }

        // 等待 Shimmy 启动
        tokio::time::sleep(tokio::time::Duration::from_millis(3000)).await;

        // 检查端口是否被占用（判断 Shimmy 是否成功启动）
        if self.is_port_in_use(SHIMMY_PORT).await {
            tracing::info!("Shimmy 启动成功，监听端口 {}", SHIMMY_PORT);
        } else {
            tracing::error!("Shimmy 进程启动后端口未被占用，可能已退出");
            return Err(anyhow::anyhow!("Shimmy 进程启动后立即退出"));
        }

        Ok(())
    }

    /// 根据端口关闭进程（Windows）：仅终止 LISTENING 监听方，避免误杀 ESTABLISHED 客户端
    async fn kill_process_by_port(&self, port: i32) {
        let port_suffix = format!(":{}", port);
        let mut cmd = Command::new("cmd");
        cmd.args(&["/c", &format!("netstat -ano | findstr :{}", port)]);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);
        let output = cmd.output().await;

        if let Ok(output) = output {
            let result = String::from_utf8_lossy(&output.stdout);
            let mut killed = std::collections::HashSet::new();
            for line in result.lines() {
                // 只处理监听行；ESTABLISHED 等是连接方，杀了会误伤浏览器/其它客户端
                if !line.to_ascii_uppercase().contains("LISTENING") {
                    continue;
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                // TCP  0.0.0.0:11435  0.0.0.0:0  LISTENING  1234
                if parts.len() < 5 {
                    continue;
                }
                let local_addr = parts[1];
                // 精确匹配本地端口，避免 findstr :3000 误匹配 :30001
                if !local_addr.ends_with(&port_suffix) {
                    continue;
                }
                if let Some(pid_str) = parts.last() {
                    if let Ok(pid) = pid_str.parse::<u32>() {
                        if !killed.insert(pid) {
                            continue;
                        }
                        tracing::info!("关闭监听端口 {} 的进程 PID: {}", port, pid);
                        let mut cmd = Command::new("taskkill");
                        cmd.args(&["/F", "/T", "/PID", &pid.to_string()]);
                        #[cfg(target_os = "windows")]
                        cmd.creation_flags(0x08000000);
                        let _ = cmd.output().await;
                    }
                }
            }
        }
    }

    /// 获取运行中的 GGUF 服务数量
    async fn get_running_gguf_count(&self) -> usize {
        let procs = self.processes.read().await;
        let mut count = 0;

        for id in procs.keys() {
            if let Ok(Some(service)) = repo::wsp::get_service(&self.pool, id).await {
                if self.is_gguf_service(&service) {
                    count += 1;
                }
            }
        }

        count
    }

    /// 停止 Shimmy 进程
    async fn stop_shimmy(&self) {
        let mut shimmy = self.shimmy_process.write().await;
        if let Some(process) = shimmy.take() {
            let pid = process.pid();
            tracing::info!("停止 Shimmy 进程 PID: {}", pid);
            let mut cmd = Command::new("taskkill");
            cmd.args(&["/F", "/T", "/PID", &pid.to_string()]);
            #[cfg(target_os = "windows")]
            cmd.creation_flags(0x08000000);
            let _ = cmd.output().await;
            tracing::info!("Shimmy 已停止");
        }
    }
}
