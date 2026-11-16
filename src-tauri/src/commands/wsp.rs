// FastClaw · WSP 本地微服务管理 Tauri Commands
// 前端通过 invoke() 调用这些命令管理 WSP exe 服务

use crate::core::dependency_services::{
    capture_process_facts, dependency_service_supervisor, identity_matches,
    identity_matches_process, same_executable, LifecycleState, ProcessFacts, ProcessIdentity,
    RuntimeOwnership, ServiceRuntime,
};
use crate::core::error::AppError;
use crate::core::wsp::types::*;
use crate::db::repo;
use crate::kg_coordination::{
    self, KgRuntimeRegistry, KgSharedStatus, ReleaseOutcome, SharedKgInstance,
    SharedServiceProcess, StartSlot,
};
use crate::state::AppState;
use crate::utils::paths;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use tauri::Emitter;
use tauri::Manager;
use tauri::State;
use tokio::sync::RwLock;
/// 获取所有 WSP 服务列表（含运行状态）
#[tauri::command]
pub async fn list_wsp_services(
    state: State<'_, AppState>,
) -> Result<Vec<WspServiceWithStatus>, AppError> {
    let mut services = repo::wsp::list_services_with_status(&*state.db.pool().await).await?;

    // 检查每个服务的实际运行状态，如果进程不存在则更新为stopped
    for service in &mut services {
        if let Some(status) = &service.runtime_status {
            if status == "running" || status == "starting" {
                // 检查进程是否真的在运行
                if let Some(pid) = service.pid {
                    if !is_pid_running(pid) {
                        // 进程不存在，更新状态为stopped
                        let _ = sqlx::query("UPDATE wsp_runtime_status SET status = 'stopped', pid = NULL WHERE service_id = ?")
                            .bind(&service.id)
                            .execute(&*state.db.pool().await)
                            .await;
                        service.runtime_status = Some("stopped".to_string());
                        service.pid = None;
                    }
                } else {
                    // 没有PID但状态是running，应该更新为stopped
                    let _ = sqlx::query(
                        "UPDATE wsp_runtime_status SET status = 'stopped' WHERE service_id = ?",
                    )
                    .bind(&service.id)
                    .execute(&*state.db.pool().await)
                    .await;
                    service.runtime_status = Some("stopped".to_string());
                }
            }
        }
    }

    Ok(services)
}

/// 检查PID是否在运行
fn is_pid_running(pid: i32) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;
        let mut output_cmd = Command::new("tasklist");
        output_cmd.args(&["/FI", &format!("PID eq {}", pid), "/NH"]);
        output_cmd.creation_flags(0x08000000);
        let output = output_cmd.output();
        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            return stdout.contains(&pid.to_string());
        }
        false
    }
    #[cfg(not(windows))]
    {
        use std::process::Command;
        let output = Command::new("kill")
            .args(&["-0", &pid.to_string()])
            .output();
        output.map(|o| o.status.success()).unwrap_or(false)
    }
}

/// 获取单个 WSP 服务的详细状态
#[tauri::command]
pub async fn get_wsp_status(
    state: State<'_, AppState>,
    service_id: String,
) -> Result<WspServiceWithStatus, AppError> {
    let services = repo::wsp::list_services_with_status(&*state.db.pool().await).await?;
    services
        .into_iter()
        .find(|s| s.id == service_id)
        .ok_or_else(|| AppError::NotFound(format!("WSP 服务不存在: {}", service_id)))
}

/// 启动指定 WSP 服务
#[tauri::command]
pub async fn start_wsp_service(
    state: State<'_, AppState>,
    service_id: String,
) -> Result<(), AppError> {
    state
        .wsp
        .start_service(&service_id)
        .await
        .map_err(|e| AppError::Wsp(e.to_string()))
}

/// 停止指定 WSP 服务
#[tauri::command]
pub async fn stop_wsp_service(
    state: State<'_, AppState>,
    service_id: String,
) -> Result<(), AppError> {
    state
        .wsp
        .stop_service(&service_id)
        .await
        .map_err(|e| AppError::Wsp(e.to_string()))
}

/// 重启指定 WSP 服务
#[tauri::command]
pub async fn restart_wsp_service(
    state: State<'_, AppState>,
    service_id: String,
) -> Result<(), AppError> {
    state
        .wsp
        .restart_service(&service_id)
        .await
        .map_err(|e| AppError::Wsp(e.to_string()))
}

/// 启动所有已启用的 WSP 服务
#[tauri::command]
pub async fn start_all_wsp_services(state: State<'_, AppState>) -> Result<(), AppError> {
    state
        .wsp
        .start_all()
        .await
        .map_err(|e| AppError::Wsp(e.to_string()))
}

/// 自动启动所有标记为 auto_start 的服务
#[tauri::command]
pub async fn start_all_auto(state: State<'_, AppState>) -> Result<(), AppError> {
    state
        .wsp
        .start_all_auto()
        .await
        .map_err(|e| AppError::Wsp(e.to_string()))
}

/// 停止所有运行中的 WSP 服务
#[tauri::command]
pub async fn stop_all_wsp_services(state: State<'_, AppState>) -> Result<(), AppError> {
    state
        .wsp
        .stop_all()
        .await
        .map_err(|e| AppError::Wsp(e.to_string()))
}

/// 获取 WSP 服务的事件日志
#[tauri::command]
pub async fn get_wsp_logs(
    state: State<'_, AppState>,
    service_id: String,
    limit: Option<i64>,
) -> Result<Vec<WspEventLog>, AppError> {
    let logs = repo::wsp::get_event_logs(&*state.db.pool().await, &service_id, limit.unwrap_or(50))
        .await?;
    Ok(logs)
}

/// 调用 WSP 服务的 API 端点（代理转发）
#[tauri::command]
pub async fn call_wsp_endpoint(
    state: State<'_, AppState>,
    service_id: String,
    method: String,
    path: String,
    body: Option<serde_json::Value>,
) -> Result<serde_json::Value, AppError> {
    let start = std::time::Instant::now();

    let resp = state
        .wsp
        .call_endpoint(&service_id, &method, &path, body)
        .await
        .map_err(|e| AppError::Wsp(e.to_string()))?;

    let status_code = resp.status().as_u16() as i32;
    let is_success = resp.status().is_success();
    let result: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
    let latency = start.elapsed().as_millis() as i64;

    // 记录调用日志
    let _ = repo::wsp::insert_call_log(
        &*state.db.pool().await,
        &WspCallLog {
            id: uuid::Uuid::new_v4().to_string(),
            service_id: service_id.clone(),
            endpoint_id: None,
            user_id: "default-user".to_string(),
            conversation_id: None,
            method,
            path,
            status_code: Some(status_code),
            request_size: None,
            response_size: None,
            latency_ms: Some(latency),
            is_success,
            error_message: if is_success {
                None
            } else {
                Some(result.to_string())
            },
        },
    )
    .await;

    Ok(result)
}

/// 注册新的 WSP 服务
#[tauri::command]
pub async fn register_wsp_service(
    state: State<'_, AppState>,
    input: RegisterWspInput,
) -> Result<WspService, AppError> {
    // 先修复数据库中所有 status 为 NULL 的记录
    let _ = sqlx::query("UPDATE wsp_runtime_status SET status = 'stopped' WHERE status IS NULL")
        .execute(&*state.db.pool().await)
        .await;
    let _ =
        sqlx::query("UPDATE wsp_runtime_status SET request_count = 0 WHERE request_count IS NULL")
            .execute(&*state.db.pool().await)
            .await;
    let _ = sqlx::query("UPDATE wsp_runtime_status SET error_count = 0 WHERE error_count IS NULL")
        .execute(&*state.db.pool().await)
        .await;
    let _ =
        sqlx::query("UPDATE wsp_runtime_status SET restart_count = 0 WHERE restart_count IS NULL")
            .execute(&*state.db.pool().await)
            .await;
    let _ = sqlx::query(
        "UPDATE wsp_runtime_status SET last_health_ok = 0 WHERE last_health_ok IS NULL",
    )
    .execute(&*state.db.pool().await)
    .await;

    let service = WspService {
        id: uuid::Uuid::new_v4().to_string(),
        name: input.name,
        display_name: input.display_name,
        description: input.description,
        version: input.version.unwrap_or_else(|| "1.0.0".to_string()),
        category: input.category.unwrap_or_else(|| "general".to_string()),
        category_name: input.category_name,
        sub_category_name: input.sub_category_name,
        exe_path: input.exe_path,
        work_dir: input.work_dir,
        args: input.args,
        env_vars: input.env_vars,
        protocol: input.protocol.unwrap_or_else(|| "http".to_string()),
        host: input.host.unwrap_or_default(),
        port: input.port,
        base_path: input.base_path.unwrap_or_default(),
        ws_path: input.ws_path,
        health_endpoint: input
            .health_endpoint
            .unwrap_or_else(|| "/health".to_string()),
        cpu_limit: None,
        memory_limit_mb: input.memory_limit_mb,
        gpu_required: input.gpu_required.unwrap_or(false),
        gpu_device_id: None,
        auto_start: input.auto_start.unwrap_or(false),
        start_delay_ms: 0,
        start_priority: 50,
        restart_policy: "on_failure".to_string(),
        max_restarts: 3,
        restart_cooldown_ms: 5000,
        health_check_interval_ms: 10000,
        health_check_timeout_ms: 3000,
        startup_timeout_ms: 30000,
        depends_on: input.depends_on,
        is_enabled: true,
        is_installed: true,
        sort_order: 0,
        extra_config: None,
        created_at: String::new(),
        updated_at: String::new(),
    };

    repo::wsp::insert_service(&*state.db.pool().await, &service).await?;
    Ok(service)
}

/// 更新 WSP 服务配置（轻量，仅 health 字段）
#[tauri::command]
pub async fn update_wsp_service(
    state: State<'_, AppState>,
    service_id: String,
    health_endpoint: Option<String>,
    health_check_interval_ms: Option<i32>,
) -> Result<(), AppError> {
    let pool = &*state.db.pool().await;

    if let Some(he) = health_endpoint {
        sqlx::query("UPDATE wsp_services SET health_endpoint = ? WHERE id = ?")
            .bind(&he)
            .bind(&service_id)
            .execute(pool)
            .await
            .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    }

    if let Some(hci) = health_check_interval_ms {
        sqlx::query("UPDATE wsp_services SET health_check_interval_ms = ? WHERE id = ?")
            .bind(hci)
            .bind(&service_id)
            .execute(pool)
            .await
            .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    }

    tracing::info!("WSP 服务配置已更新: {}", service_id);
    Ok(())
}

/// 编辑 WSP 服务基本信息（服务必须未运行）
#[tauri::command]
pub async fn edit_wsp_service(
    state: State<'_, AppState>,
    service_id: String,
    display_name: String,
    description: Option<String>,
    exe_path: String,
    port: i32,
    auto_start: bool,
    category: Option<String>,
    category_name: Option<String>,
    sub_category_name: Option<String>,
    host: Option<String>,
    base_path: Option<String>,
) -> Result<(), AppError> {
    let pool = &*state.db.pool().await;
    let mut service = repo::wsp::get_service(pool, &service_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("WSP 服务不存在: {}", service_id)))?;

    service.display_name = display_name;
    service.description = description;
    service.exe_path = exe_path;
    service.port = port;
    service.auto_start = auto_start;
    if let Some(h) = host {
        service.host = h;
    }
    if let Some(bp) = base_path {
        service.base_path = bp;
    }
    if let Some(cat) = category {
        service.category = cat;
    }
    service.category_name = category_name;
    service.sub_category_name = sub_category_name;

    repo::wsp::update_service(pool, &service)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;

    tracing::info!("WSP 服务已编辑: {}", service_id);
    Ok(())
}

/// 删除 WSP 服务（需先停止）
#[tauri::command]
pub async fn remove_wsp_service(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    service_id: String,
) -> Result<(), AppError> {
    // 删除前查找服务名、文件路径和模型ID，用于恢复模型市场状态、删除下载文件和清理下载中心记录
    let row: Option<(String, Option<String>, Option<i64>)> = sqlx::query_as(
        "SELECT s.name, s.exe_path, m.id FROM wsp_services s LEFT JOIN wsp_model_market m ON s.name = m.exeName WHERE s.id = ?"
    ).bind(&service_id).fetch_optional(&*state.db.pool().await).await?;
    let _ = state.wsp.stop_service(&service_id).await;
    repo::wsp::delete_service(&*state.db.pool().await, &service_id).await?;
    if let Some((ref name, exe_path, model_id)) = row {
        // 恢复模型市场状态
        sqlx::query("UPDATE wsp_model_market SET status = 1 WHERE exeName = ?")
            .bind(name)
            .execute(&*state.db.pool().await)
            .await?;
        // 删除下载的模型文件
        if let Some(ref path) = exe_path {
            let p = std::path::Path::new(path);
            if p.exists() && !p.is_dir() {
                let _ = tokio::fs::remove_file(p).await;
                log::info!("[remove_wsp] 已删除模型文件: {}", path);
            }
        }
        // 通知前端清理下载中心记录
        if let Some(mid) = model_id {
            app.emit("wsp-model-deleted", serde_json::json!({"model_id": mid}))
                .ok();
            log::info!("[remove_wsp] 已发送删除事件，model_id: {}", mid);
        }
    }
    Ok(())
}

/// 上传 WSP 服务（通过文件路径）
/// 1. 复制文件到 storage/file/wsp-{timestamp}/ 目录
/// 2. 如果是压缩包则解压
/// 3. 在数据库中创建服务记录，name 字段赋值为 wsp-{timestamp}
#[tauri::command]
pub async fn upload_wsp_service_by_path(
    state: State<'_, AppState>,
    file_path: String,
    category: String,
    port: i32,
    display_name: Option<String>,
    category_name: Option<String>,
    sub_category_name: Option<String>,
) -> Result<WspService, AppError> {
    // 1. 检查源文件是否存在
    let source_path = std::path::Path::new(&file_path);
    if !source_path.exists() {
        return Err(AppError::NotFound(format!("文件不存在: {}", file_path)));
    }

    // 2. 生成唯一文件夹名称（用UUID确保唯一）
    let unique_id = uuid::Uuid::new_v4().to_string().replace("-", "")[..8].to_string();
    let folder_name = format!(
        "wsp-{}-{}",
        chrono::Utc::now().timestamp_millis(),
        unique_id
    );

    // 3. 创建存储目录（使用应用数据目录的绝对路径）
    let base_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("FastClaw")
        .join("storage")
        .join("file");
    let folder_path = base_dir.join(&folder_name);

    tokio::fs::create_dir_all(&folder_path).await.map_err(|e| {
        AppError::Io(std::io::Error::new(
            e.kind(),
            format!("无法创建存储目录: {}", e),
        ))
    })?;

    // 4. 判断文件类型并处理
    let file_name = source_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let lower_name = file_name.to_lowercase();
    let target_file_path = folder_path.join(&file_name);

    let final_exe_path: String;

    if lower_name.ends_with(".zip") {
        // 解压 zip 文件
        tracing::info!("解压压缩包: {}", file_path);

        // 使用同步方式解压（zip crate 需要同步）
        let extract_dir = extract_zip_sync(&file_path, &folder_path)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("解压失败: {}", e)))
            .map_err(AppError::Io)?;

        // 在解压目录中查找 exe 文件
        final_exe_path = find_exe_in_dir_sync(&extract_dir)
            .map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, format!("查找 exe 失败: {}", e))
            })
            .map_err(AppError::Io)?
            .unwrap_or_else(|| {
                // 如果没找到 exe，返回解压目录本身
                extract_dir.clone()
            });

        tracing::info!("解压完成，exe 路径: {}", final_exe_path);
    } else {
        // 直接复制文件
        tracing::info!("复制文件: {} -> {}", file_path, target_file_path.display());

        tokio::fs::copy(&file_path, &target_file_path)
            .await
            .map_err(|e| {
                AppError::Io(std::io::Error::new(
                    e.kind(),
                    format!("复制文件失败: {}", e),
                ))
            })?;

        final_exe_path = target_file_path.to_string_lossy().to_string();
    }

    // 5. 创建服务记录，name 字段使用文件夹名称
    // 构建端口参数
    let args = Some(
        serde_json::to_string(&vec![format!("--port"), format!("{}", port)]).unwrap_or_default(),
    );

    let service = WspService {
        id: uuid::Uuid::new_v4().to_string(),
        name: folder_name.clone(), // 关键：name 字段使用文件夹名称
        display_name: display_name.unwrap_or_else(|| folder_name.clone()),
        description: Some(format!("上传文件: {}", file_name)),
        version: "1.0.0".to_string(),
        category,
        category_name,
        sub_category_name,
        exe_path: final_exe_path,
        work_dir: Some(folder_path.to_string_lossy().to_string()),
        args,
        env_vars: None,
        protocol: "http".to_string(),
        host: "127.0.0.1".to_string(),
        port,
        base_path: "/".to_string(),
        ws_path: None,
        health_endpoint: "/health".to_string(),
        cpu_limit: None,
        memory_limit_mb: None,
        gpu_required: false,
        gpu_device_id: None,
        auto_start: false,
        start_delay_ms: 0,
        start_priority: 50,
        restart_policy: "on_failure".to_string(),
        max_restarts: 3,
        restart_cooldown_ms: 5000,
        health_check_interval_ms: 10000,
        health_check_timeout_ms: 3000,
        startup_timeout_ms: 30000,
        depends_on: None,
        is_enabled: true,
        is_installed: true,
        sort_order: 0,
        extra_config: Some(
            serde_json::json!({
                "original_file": file_name,
                "folder_name": folder_name,
            })
            .to_string(),
        ),
        created_at: String::new(),
        updated_at: String::new(),
    };

    // 6. 保存到数据库
    repo::wsp::insert_service(&*state.db.pool().await, &service).await?;

    tracing::info!(
        "WSP 服务已上传并注册: {} (name: {}, folder: {})",
        service.display_name,
        folder_name,
        folder_name
    );

    Ok(service)
}

/// 同步解压 zip 文件
fn extract_zip_sync(archive_path: &str, extract_to: &std::path::Path) -> anyhow::Result<String> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(std::io::BufReader::new(file))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = extract_to.join(file.mangled_name());

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    std::fs::create_dir_all(p)?;
                }
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }

    Ok(extract_to.to_string_lossy().to_string())
}

/// 在目录中同步查找 exe 文件
fn find_exe_in_dir_sync(dir: &str) -> anyhow::Result<Option<String>> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "exe" || ext == "EXE" {
                        return Ok(Some(path.to_string_lossy().to_string()));
                    }
                }
            } else if path.is_dir() {
                if let Ok(Some(exe)) = find_exe_in_dir_sync(&path.to_string_lossy()) {
                    return Ok(Some(exe));
                }
            }
        }
    }
    Ok(None)
}

/// 上传 WSP 文件并注册服务（同步执行）
/// 1. 将文件保存到 storage/file/wsp-{timestamp}/ 目录
/// 2. 在数据库中创建服务记录，name 字段赋值为 wsp-{timestamp}
#[tauri::command]
pub async fn upload_wsp_service(
    state: State<'_, AppState>,
    input: UploadWspInput,
) -> Result<WspService, AppError> {
    // 1. 生成唯一文件夹名称（用UUID确保唯一）
    let unique_id = uuid::Uuid::new_v4().to_string().replace("-", "")[..8].to_string();
    let folder_name = format!(
        "wsp-{}-{}",
        chrono::Utc::now().timestamp_millis(),
        unique_id
    );

    // 2. 创建存储目录（使用应用数据目录的绝对路径）
    let base_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("FastClaw")
        .join("storage")
        .join("file");
    let folder_path = base_dir.join(&folder_name);

    tokio::fs::create_dir_all(&folder_path).await.map_err(|e| {
        AppError::Io(std::io::Error::new(
            e.kind(),
            format!("无法创建存储目录: {}", e),
        ))
    })?;

    // 3. 保存上传的文件
    let mut saved_files: Vec<String> = Vec::new();

    for file_data in &input.files {
        let file_path = folder_path.join(&file_data.name);
        tokio::fs::write(&file_path, &file_data.content)
            .await
            .map_err(|e| AppError::Io(e))?;
        saved_files.push(file_data.name.clone());
        tracing::info!("文件已保存: {}", file_path.display());
    }

    // 4. 确定 exe 路径
    let exe_path = if !input.exe_path.is_empty() {
        input.exe_path.clone()
    } else if !saved_files.is_empty() {
        let exe_file = saved_files
            .iter()
            .find(|f| f.ends_with(".exe") || f.ends_with(".EXE"));
        if let Some(exe) = exe_file {
            folder_path.join(exe).to_string_lossy().to_string()
        } else {
            folder_path
                .join(&saved_files[0])
                .to_string_lossy()
                .to_string()
        }
    } else {
        return Err(AppError::InvalidInput("没有提供可执行文件".to_string()));
    };

    // 5. 创建服务记录，name 字段使用文件夹名称
    let service = WspService {
        id: uuid::Uuid::new_v4().to_string(),
        name: folder_name.clone(), // 使用文件夹名称作为 name
        display_name: input.display_name.unwrap_or_else(|| folder_name.clone()),
        description: input.description,
        version: input.version.unwrap_or_else(|| "1.0.0".to_string()),
        category: input.category.unwrap_or_else(|| "uploaded".to_string()),
        category_name: input.category_name,
        sub_category_name: input.sub_category_name,
        exe_path,
        work_dir: Some(folder_path.to_string_lossy().to_string()),
        args: input.args,
        env_vars: input.env_vars,
        protocol: input.protocol.unwrap_or_else(|| "http".to_string()),
        host: input.host.unwrap_or_default(),
        port: input.port,
        base_path: input.base_path.unwrap_or_default(),
        ws_path: input.ws_path,
        health_endpoint: input
            .health_endpoint
            .unwrap_or_else(|| "/health".to_string()),
        cpu_limit: None,
        memory_limit_mb: input.memory_limit_mb,
        gpu_required: input.gpu_required.unwrap_or(false),
        gpu_device_id: None,
        auto_start: input.auto_start.unwrap_or(false),
        start_delay_ms: 0,
        start_priority: 50,
        restart_policy: "on_failure".to_string(),
        max_restarts: 3,
        restart_cooldown_ms: 5000,
        health_check_interval_ms: 10000,
        health_check_timeout_ms: 3000,
        startup_timeout_ms: 30000,
        depends_on: input.depends_on,
        is_enabled: true,
        is_installed: true,
        sort_order: 0,
        extra_config: Some(
            serde_json::json!({
                "uploaded_files": saved_files,
                "folder_name": folder_name,
            })
            .to_string(),
        ),
        created_at: String::new(),
        updated_at: String::new(),
    };

    // 6. 保存到数据库
    repo::wsp::insert_service(&*state.db.pool().await, &service).await?;

    tracing::info!(
        "WSP 服务已注册: {} (文件夹: {})",
        service.display_name,
        folder_name
    );
    Ok(service)
}

/// 上传文件到已有服务
#[tauri::command]
pub async fn upload_files_to_wsp(
    state: State<'_, AppState>,
    service_id: String,
    files: Vec<FileData>,
) -> Result<Vec<String>, AppError> {
    // 获取服务信息
    let service = repo::wsp::get_service(&*state.db.pool().await, &service_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("服务不存在: {}", service_id)))?;

    // 使用服务的工作目录或创建新文件夹
    let folder_name = if let Some(ref work_dir) = service.work_dir {
        let path = std::path::Path::new(work_dir);
        if path.exists() && path.is_dir() {
            path.to_string_lossy().to_string()
        } else {
            let new_folder = format!(
                "storage/file/wsp-{}-{}",
                chrono::Utc::now().timestamp_millis(),
                service_id
            );
            tokio::fs::create_dir_all(&new_folder)
                .await
                .map_err(|e| AppError::Io(e))?;
            new_folder
        }
    } else {
        let new_folder = format!(
            "storage/file/wsp-{}-{}",
            chrono::Utc::now().timestamp_millis(),
            service_id
        );
        tokio::fs::create_dir_all(&new_folder)
            .await
            .map_err(|e| AppError::Io(e))?;
        new_folder
    };

    let folder_path = std::path::Path::new(&folder_name);
    let mut saved_files: Vec<String> = Vec::new();

    for file_data in files {
        let file_path = folder_path.join(&file_data.name);
        tokio::fs::write(&file_path, &file_data.content)
            .await
            .map_err(|e| AppError::Io(e))?;
        saved_files.push(file_data.name.clone());
        tracing::info!("文件已上传: {}", file_path.display());
    }

    // 更新服务的 extra_config
    let _new_config = serde_json::json!({
        "additional_files": saved_files,
    })
    .to_string();

    // 注意：需要实现 update_service_config 方法
    tracing::info!("文件已上传到服务 {}: {:?}", service_id, saved_files);

    Ok(saved_files)
}

/// 注册 WSP 服务的前端入参
#[derive(Debug, serde::Deserialize)]
pub struct RegisterWspInput {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub category: Option<String>,
    pub category_name: Option<String>,
    pub sub_category_name: Option<String>,
    pub exe_path: String,
    pub url: Option<String>,
    pub work_dir: Option<String>,
    pub args: Option<String>,
    pub env_vars: Option<String>,
    pub protocol: Option<String>,
    pub host: Option<String>,
    pub port: i32,
    pub base_path: Option<String>,
    pub ws_path: Option<String>,
    pub health_endpoint: Option<String>,
    pub memory_limit_mb: Option<i32>,
    pub gpu_required: Option<bool>,
    pub auto_start: Option<bool>,
    pub depends_on: Option<String>,
}

/// 上传 WSP 服务的入参
#[derive(Debug, serde::Deserialize)]
pub struct UploadWspInput {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub category: Option<String>,
    pub category_name: Option<String>,
    pub sub_category_name: Option<String>,
    pub exe_path: String,
    pub args: Option<String>,
    pub env_vars: Option<String>,
    pub protocol: Option<String>,
    pub host: Option<String>,
    pub port: i32,
    pub base_path: Option<String>,
    pub ws_path: Option<String>,
    pub health_endpoint: Option<String>,
    pub memory_limit_mb: Option<i32>,
    pub gpu_required: Option<bool>,
    pub auto_start: Option<bool>,
    pub depends_on: Option<String>,
    pub files: Vec<FileData>,
}

/// 文件数据
#[derive(Debug, serde::Deserialize)]
pub struct FileData {
    pub name: String,
    pub content: Vec<u8>,
}

/// 启动知识服务（Tauri 命令）
#[tauri::command]
pub async fn start_knowledge_service(
    state: tauri::State<'_, crate::state::AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, AppError> {
    configure_preferred_ports_from_state(&state).await;
    start_kg_inner(&app_handle, &state.kg_pids).await
}

/// KG 服务启动守卫：防止 start_kg_inner 与 start_knowledge_service_async 并发启动
/// 两个入口共享此锁，保证只有一个启动流程在执行
static KG_STARTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// 单服务启动期间的取消信号。Llama-server 已验证启动耗时可接受，保持现有流程。
static QDRANT_STOP_REQUESTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static QDRANT_STARTING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static LLAMA_STOP_REQUESTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static FASTKG_STOP_REQUESTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

struct KgStartingGuard;

impl KgStartingGuard {
    fn acquire() -> Option<Self> {
        if KG_STARTING.swap(true, std::sync::atomic::Ordering::SeqCst) {
            None
        } else {
            Some(Self)
        }
    }
}

impl Drop for KgStartingGuard {
    fn drop(&mut self) {
        KG_STARTING.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

struct QdrantStartingGuard;

impl QdrantStartingGuard {
    fn acquire() -> Option<Self> {
        if QDRANT_STARTING.swap(true, std::sync::atomic::Ordering::SeqCst) {
            None
        } else {
            Some(Self)
        }
    }
}

impl Drop for QdrantStartingGuard {
    fn drop(&mut self) {
        QDRANT_STARTING.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}
static KG_LEASE_HEARTBEAT: Mutex<Option<tokio::task::JoinHandle<()>>> = Mutex::new(None);
/// 串行化完整 KG 编排与 FastKG 单服务维护，避免停止、启动和共享发布相互穿插。
static FASTKG_MAINTENANCE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// 编排启动失败时：把仍处于 Starting/Probing 的 KG 相关服务标为 Failed。
/// 否则 UI 会一直显示「启动中」且 can_start/can_stop 均为 false，按钮消失。
async fn abort_kg_orchestration(message: &str) {
    let msg = message.to_string();
    let supervisor = dependency_service_supervisor();
    for service_id in ["fastkg", "llama_server", "qdrant"] {
        if let Some(runtime) = supervisor.snapshot(service_id).await {
            if matches!(
                runtime.lifecycle,
                LifecycleState::Starting | LifecycleState::Probing
            ) {
                supervisor.mark_failed(service_id, msg.clone()).await;
            }
        }
    }
}

/// 启动锁已释放但仍卡在 Starting/Probing：回收为 Failed，恢复启动按钮。
///
/// 依赖服务页每 5s 轮询会调用这里。嵌入/FastKG 冷启动常需数十秒才写好进程身份，
/// 若立刻回收会先显示「启动失败」再变为「运行中」。宽限期需覆盖最长健康等待
///（FastKG 端口约 30s + /health 约 120s）。
const STALE_STARTING_GRACE: std::time::Duration = std::time::Duration::from_secs(180);

async fn reconcile_stale_kg_starting_states(
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
) {
    if KG_STARTING.load(std::sync::atomic::Ordering::SeqCst)
        || QDRANT_STARTING.load(std::sync::atomic::Ordering::SeqCst)
    {
        return;
    }
    let supervisor = dependency_service_supervisor();
    for service_id in ["fastkg", "llama_server", "qdrant"] {
        let Some(runtime) = supervisor.snapshot(service_id).await else {
            continue;
        };
        if !matches!(
            runtime.lifecycle,
            LifecycleState::Starting | LifecycleState::Probing
        ) {
            continue;
        }
        if runtime.lifecycle_updated_at.elapsed() < STALE_STARTING_GRACE {
            continue;
        }
        if matches!(service_id, "qdrant" | "fastkg") && runtime.process.is_some() {
            let message = format!("{} 启动流程已结束，但未完成健康验证", service_id);
            if let Err(error) =
                cleanup_tracked_dependency_start(service_id, kg_pids, message).await
            {
                tracing::warn!("[knowledge] 回收 {} 残留进程失败: {}", service_id, error);
            }
            continue;
        }
        if verified_owned_dependency_process(service_id).await.is_some() {
            continue;
        }
        tracing::warn!(
            "[knowledge] 回收陈旧启动状态: service={}, lifecycle={:?}, age={:?}",
            service_id,
            runtime.lifecycle,
            runtime.lifecycle_updated_at.elapsed()
        );
        supervisor
            .mark_failed(service_id, "启动未完成或已中断，可重新启动")
            .await;
    }
}

/// FastPlan 服务启动守卫：防止 start_fastplan_inner 并发启动
static FASTPLAN_STARTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// 启动过程中收到停止请求时，让健康检查协程以“用户取消”收敛，而不是覆盖成启动失败。
static FASTPLAN_STOP_REQUESTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

struct FastPlanStartingGuard;

impl FastPlanStartingGuard {
    fn acquire() -> Option<Self> {
        if FASTPLAN_STARTING.swap(true, std::sync::atomic::Ordering::SeqCst) {
            None
        } else {
            Some(Self)
        }
    }
}

impl Drop for FastPlanStartingGuard {
    fn drop(&mut self) {
        FASTPLAN_STARTING.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

/// 自动启动或调用方被中断后，避免 Supervisor 永久残留在 Starting。
async fn reconcile_stale_fastplan_starting_state(
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
) {
    if FASTPLAN_STARTING.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let Some(runtime) = dependency_service_supervisor().snapshot("fastplan").await else {
        return;
    };
    if runtime.lifecycle != LifecycleState::Starting {
        return;
    }
    tracing::warn!(
        "[fastplan] 回收陈旧启动状态，tracked_pid={:?}",
        runtime.process.as_ref().map(|process| process.pid)
    );
    fail_fastplan_start(kg_pids, "FastPlan 启动流程已结束，但未完成健康验证").await;
}

// 依赖服务的实际运行端口。0 表示尚未分配，此时调用方回退到配置端口。
static QDRANT_RUNTIME_PORT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);
static LLAMA_RUNTIME_PORT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);
static FASTKG_RUNTIME_PORT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);
static FASTPLAN_RUNTIME_PORT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);
static QDRANT_PREFERRED_PORT: std::sync::atomic::AtomicU16 =
    std::sync::atomic::AtomicU16::new(paths::QDRANT_DEFAULT_PORT);
static LLAMA_PREFERRED_PORT: std::sync::atomic::AtomicU16 =
    std::sync::atomic::AtomicU16::new(paths::LLAMA_SERVER_DEFAULT_PORT);
static FASTKG_PREFERRED_PORT: std::sync::atomic::AtomicU16 =
    std::sync::atomic::AtomicU16::new(paths::FASTKG_DEFAULT_PORT);
static FASTPLAN_PREFERRED_PORT: std::sync::atomic::AtomicU16 =
    std::sync::atomic::AtomicU16::new(paths::FASTPLAN_DEFAULT_PORT);
static RESERVED_DEPENDENCY_PORTS: OnceLock<Mutex<HashSet<u16>>> = OnceLock::new();

fn runtime_port_cell(service_id: &str) -> Option<&'static std::sync::atomic::AtomicU16> {
    match service_id {
        "qdrant" => Some(&QDRANT_RUNTIME_PORT),
        "llama_server" => Some(&LLAMA_RUNTIME_PORT),
        "fastkg" => Some(&FASTKG_RUNTIME_PORT),
        "fastplan" => Some(&FASTPLAN_RUNTIME_PORT),
        _ => None,
    }
}

fn preferred_port_cell(service_id: &str) -> Option<&'static std::sync::atomic::AtomicU16> {
    match service_id {
        "qdrant" => Some(&QDRANT_PREFERRED_PORT),
        "llama_server" => Some(&LLAMA_PREFERRED_PORT),
        "fastkg" => Some(&FASTKG_PREFERRED_PORT),
        "fastplan" => Some(&FASTPLAN_PREFERRED_PORT),
        _ => None,
    }
}

fn dependency_preferred_port(service_id: &str) -> u16 {
    preferred_port_cell(service_id)
        .map(|cell| cell.load(std::sync::atomic::Ordering::SeqCst))
        .unwrap_or_default()
}

pub fn configure_dependency_preferred_url(service_id: &str, url: &str) {
    let default_port = match service_id {
        "qdrant" => paths::qdrant_default_port(),
        "llama_server" => paths::llama_server_default_port(),
        "fastkg" => paths::fastkg_default_port(),
        "fastplan" => paths::fastplan_default_port(),
        _ => return,
    };
    let (_, port) = parse_service_url(url, "127.0.0.1", default_port);
    if let Some(cell) = preferred_port_cell(service_id) {
        cell.store(port, std::sync::atomic::Ordering::SeqCst);
    }
}

async fn configure_preferred_ports_from_state(state: &AppState) {
    let config = state.app_config.read().await;
    configure_dependency_preferred_url("qdrant", &config.services.qdrant);
    configure_dependency_preferred_url("llama_server", &config.services.llama_server);
    configure_dependency_preferred_url("fastkg", &config.services.fastkg);
    configure_dependency_preferred_url("fastplan", &config.services.fastplan);
}

pub fn dependency_runtime_port(service_id: &str) -> Option<u16> {
    runtime_port_cell(service_id)
        .map(|cell| cell.load(std::sync::atomic::Ordering::SeqCst))
        .filter(|port| *port != 0)
}

fn set_dependency_runtime_port(service_id: &str, port: Option<u16>) {
    if let Some(cell) = runtime_port_cell(service_id) {
        cell.store(port.unwrap_or(0), std::sync::atomic::Ordering::SeqCst);
    }
}

pub fn dependency_effective_url(service_id: &str, configured_url: &str) -> String {
    let (host, configured_port) = parse_service_url(
        configured_url,
        "127.0.0.1",
        match service_id {
            "qdrant" => paths::qdrant_default_port(),
            "llama_server" => paths::llama_server_default_port(),
            "fastkg" => paths::fastkg_default_port(),
            "fastplan" => paths::fastplan_default_port(),
            _ => 0,
        },
    );
    let port = dependency_runtime_port(service_id).unwrap_or(configured_port);
    format!("http://{}:{}", host, port)
}

fn parse_service_url(url: &str, default_host: &str, default_port: u16) -> (String, u16) {
    if let Ok(parsed) = reqwest::Url::parse(url) {
        let host = parsed.host_str().unwrap_or(default_host).to_string();
        return (host, parsed.port_or_known_default().unwrap_or(default_port));
    }
    (default_host.to_string(), default_port)
}

fn emit_dependency_port_state(
    app: &tauri::AppHandle,
    service_id: &str,
    phase: &str,
    preferred_port: u16,
    candidate_port: Option<u16>,
    attempt: u32,
    message: impl Into<String>,
) {
    let _ = app.emit(
        "dependency-service-port-state",
        serde_json::json!({
            "serviceId": service_id,
            "phase": phase,
            "preferredHost": "127.0.0.1",
            "preferredPort": preferred_port,
            "candidatePort": candidate_port,
            "effectivePort": dependency_runtime_port(service_id),
            "attempt": attempt,
            "message": message.into(),
        }),
    );
}

struct DependencyPortReservation {
    service_id: String,
    port: u16,
    keep_runtime: bool,
}

impl DependencyPortReservation {
    fn commit(&mut self) {
        self.keep_runtime = true;
    }
}

impl Drop for DependencyPortReservation {
    fn drop(&mut self) {
        if let Ok(mut ports) = RESERVED_DEPENDENCY_PORTS
            .get_or_init(|| Mutex::new(HashSet::new()))
            .lock()
        {
            ports.remove(&self.port);
        }
        if !self.keep_runtime {
            if let Some(cell) = runtime_port_cell(&self.service_id) {
                let _ = cell.compare_exchange(
                    self.port,
                    0,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                );
            }
        }
    }
}

/// 复用已在跑的依赖端口：标记 keep，避免 Drop 清掉 runtime port。
fn reservation_for_existing_port(service_id: &str, port: u16) -> DependencyPortReservation {
    if let Ok(mut ports) = RESERVED_DEPENDENCY_PORTS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
    {
        ports.insert(port);
    }
    set_dependency_runtime_port(service_id, Some(port));
    DependencyPortReservation {
        service_id: service_id.to_string(),
        port,
        keep_runtime: true,
    }
}

async fn reserve_available_dependency_port(
    app: &tauri::AppHandle,
    service_id: &str,
    preferred_port: u16,
) -> Result<(u16, u32, DependencyPortReservation), AppError> {
    if let Some(identity) = verified_tracked_dependency_process(service_id).await {
        return Err(AppError::Business(format!(
            "{} 仍有受管进程 PID={}，须先完成安全清理再重新分配端口",
            service_id, identity.pid
        )));
    }
    if service_id == "qdrant" && find_local_dependency_listener(service_id).is_some() {
        return Err(AppError::Business(
            "同一服务目录已有 Qdrant 进程，尚未确认可安全接管，已阻止重复打开 storage".into(),
        ));
    }
    // 与状态监控共用同一份系统监听表。不能只依赖 TcpListener::bind 探测：
    // Windows 下监听者可能绑定在通配地址/IPv6 地址，或采用与探测 socket
    // 不同的共享语义，导致页面已识别占用而分配器仍误选同一端口。
    let listeners = get_listening_port_pid_map();
    let candidates = dependency_port_candidates(preferred_port);
    for (offset, candidate) in candidates.enumerate() {
        let attempt = offset as u32 + 1;
        set_dependency_runtime_port(service_id, Some(candidate));
        dependency_service_supervisor()
            .mark_probing(service_id, preferred_port, candidate)
            .await;
        emit_dependency_port_state(
            app,
            service_id,
            "probing",
            preferred_port,
            Some(candidate),
            attempt,
            format!("正在尝试端口 {}", candidate),
        );

        let reserved = {
            let mut ports = RESERVED_DEPENDENCY_PORTS
                .get_or_init(|| Mutex::new(HashSet::new()))
                .lock()
                .map_err(|_| AppError::Business("依赖服务端口分配锁已损坏".into()))?;
            if dependency_port_is_available(candidate, &listeners, &ports) {
                ports.insert(candidate);
                true
            } else {
                false
            }
        };

        if reserved {
            return Ok((
                candidate,
                attempt,
                DependencyPortReservation {
                    service_id: service_id.to_string(),
                    port: candidate,
                    keep_runtime: false,
                },
            ));
        }
        emit_dependency_port_state(
            app,
            service_id,
            "occupied",
            preferred_port,
            Some(candidate),
            attempt,
            format!("端口 {} 已占用，正在尝试下一个端口", candidate),
        );
        // 给 WebView 留出一次渲染窗口，确保连续占用时每个候选端口都能被看到。
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    }
    set_dependency_runtime_port(service_id, None);
    Err(AppError::DependencyPortExhausted {
        first: preferred_port.max(1), last: preferred_port.max(1).saturating_add(39),
    })
}

fn dependency_port_candidates(preferred_port: u16) -> std::ops::RangeInclusive<u16> {
    let first = preferred_port.max(1);
    first..=first.saturating_add(39)
}

enum KgStartDecision {
    Reused(String),
    StartLocal,
}

fn coordination_err(e: kg_coordination::CoordinationError) -> AppError {
    AppError::Business(format!("KG 协调状态错误: {}", e))
}

pub(crate) fn start_kg_lease_heartbeat() {
    let mut heartbeat = KG_LEASE_HEARTBEAT.lock().unwrap_or_else(|e| e.into_inner());
    if heartbeat.as_ref().is_some_and(|task| !task.is_finished())
    {
        return;
    }
    *heartbeat = Some(tokio::spawn(async {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            if let Err(e) = kg_coordination::refresh_lease() {
                tracing::debug!("[knowledge] KG lease heartbeat skipped: {}", e);
            }
        }
    }));
}

async fn stop_kg_lease_heartbeat() {
    let task = KG_LEASE_HEARTBEAT.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(task) = task {
        task.abort();
        let _ = task.await;
    }
}

async fn ensure_kg_lease_for_ready_registry(
    app: &tauri::AppHandle,
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
) -> Result<Option<KgRuntimeRegistry>, AppError> {
    kg_coordination::reconcile_stale_leases().map_err(coordination_err)?;
    let Some(registry) = kg_coordination::read_registry().map_err(coordination_err)? else {
        return Ok(None);
    };
    if !shared_kg_registry_is_valid(&registry).await {
        return Ok(None);
    }
    let instance_id = registry.instance.instance_id.clone();
    let Some(registry) = kg_coordination::register_lease(&instance_id).map_err(coordination_err)?
    else {
        // 读取与登记之间 registry 可能已进入 stopping 或完成换代；不得复用旧快照。
        return Ok(None);
    };
    apply_instance_pids(kg_pids, &registry.instance).await;
    start_kg_lease_heartbeat();
    emit_kg_status(
        app,
        "all_ready",
        kg_coordination::status_from_registry(Some(&registry)),
    );
    Ok(Some(registry))
}

async fn apply_instance_pids(
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
    instance: &SharedKgInstance,
) {
    set_dependency_runtime_port(
        "fastkg",
        Some(parse_url_port(
            &instance.fastkg.url,
            paths::fastkg_default_port(),
        )),
    );
    set_dependency_runtime_port(
        "qdrant",
        Some(parse_url_port(
            &instance.qdrant.url,
            paths::qdrant_default_port(),
        )),
    );
    set_dependency_runtime_port(
        "llama_server",
        Some(parse_url_port(
            &instance.llama_server.url,
            paths::llama_server_default_port(),
        )),
    );
    let mut guard = kg_pids.write().await;
    guard.fastkg = (instance.fastkg.pid != 0).then_some(instance.fastkg.pid);
    guard.qdrant = (instance.qdrant.pid != 0).then_some(instance.qdrant.pid);
    guard.llama_server = (instance.llama_server.pid != 0).then_some(instance.llama_server.pid);
}

fn emit_kg_status(app: &tauri::AppHandle, phase: &str, status: KgSharedStatus) {
    let mut payload = serde_json::to_value(status).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("phase".to_string(), serde_json::json!(phase));
    }
    let _ = app.emit("kg-service-status", payload);
}

#[derive(Default)]
struct FastKgHealthCache {
    url: String,
    result: bool,
    checked_at: Option<std::time::Instant>,
}

static FASTKG_HEALTH_CACHE: std::sync::OnceLock<tokio::sync::Mutex<FastKgHealthCache>> =
    std::sync::OnceLock::new();

fn fastkg_health_cache() -> &'static tokio::sync::Mutex<FastKgHealthCache> {
    FASTKG_HEALTH_CACHE.get_or_init(|| tokio::sync::Mutex::new(FastKgHealthCache::default()))
}

async fn probe_fastkg_health(url: &str) -> bool {
    let health_url = format!("{}/health", url.trim_end_matches('/'));
    // 必须 no_proxy：系统 HTTP_PROXY 会把 127.0.0.1 请求打到代理，导致
    // 「进程身份有效但健康检查失败」以及知识页一直卡在启动中。
    let client = match reqwest::Client::builder()
        .no_proxy()
        // FastKG /health 会串行检查 Qdrant 与 embedding；本机实测正常响应约 4.4s。
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(_) => return false,
    };
    let started_at = std::time::Instant::now();
    match client
        .get(health_url)
        .header(
            crate::core::constants::FASTKG_SERVICE_KEY_HEADER,
            crate::core::constants::FASTKG_SERVICE_KEY,
        )
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => true,
        Ok(response) => {
            tracing::debug!(
                "[knowledge] FastKG /health 返回 {}，耗时 {:?}",
                response.status(),
                started_at.elapsed()
            );
            false
        }
        Err(error) => {
            tracing::debug!(
                "[knowledge] FastKG /health 请求失败，耗时 {:?}: {}",
                started_at.elapsed(),
                error
            );
            false
        }
    }
}

/// FastKG 的完整 /health 会串行验证 Qdrant 与 embedding，本机约需 4 秒。
/// 这里在进程内合并并发探活，并短暂缓存成功结果；失败只缓存 800ms，避免
/// 冷启动轮询被长时间的负缓存拖慢。
async fn fastkg_health_ok(url: &str) -> bool {
    let normalized_url = url.trim_end_matches('/');
    let mut cache = fastkg_health_cache().lock().await;
    if cache.url == normalized_url {
        if let Some(checked_at) = cache.checked_at {
            let ttl = if cache.result {
                std::time::Duration::from_secs(15)
            } else {
                std::time::Duration::from_millis(800)
            };
            if checked_at.elapsed() < ttl {
                return cache.result;
            }
        }
    }

    // 整个探活期间持有互斥锁，使同时到达的调用等待并复用本次结果。
    let result = probe_fastkg_health(normalized_url).await;
    cache.url = normalized_url.to_string();
    cache.result = result;
    cache.checked_at = Some(std::time::Instant::now());
    result
}

async fn dependency_health_ok(service_id: &str, port: u16) -> bool {
    if port == 0 {
        return false;
    }
    if service_id == "fastkg" {
        return fastkg_health_ok(&format!("http://127.0.0.1:{}", port)).await;
    }
    let path = match service_id {
        "qdrant" => "/readyz",
        "llama_server" => "/health",
        "fastplan" => "/api/health",
        _ => return false,
    };
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_millis(900))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap_or_default();
    let Ok(response) = client
        .get(format!("http://127.0.0.1:{}{}", port, path))
        .send()
        .await
    else {
        return false;
    };
    if !response.status().is_success() {
        return false;
    }
    if service_id == "fastplan" {
        let Ok(body) = response.json::<serde_json::Value>().await else {
            return false;
        };
        return body.get("status").and_then(|value| value.as_str()) == Some("ok");
    }
    true
}

async fn shared_kg_registry_is_valid(registry: &KgRuntimeRegistry) -> bool {
    if registry.state != "ready" || !fastkg_health_ok(&registry.instance.fastkg.url).await {
        return false;
    }
    let listeners = get_listening_port_pid_map();
    let pids = [
        registry.instance.fastkg.pid,
        registry.instance.qdrant.pid,
        registry.instance.llama_server.pid,
    ];
    let processes = capture_process_facts(&pids);
    let service_dir = std::path::Path::new(&registry.instance.service_dir);
    shared_process_is_valid(
        &registry.instance.fastkg,
        service_dir,
        "FastKG.exe",
        &listeners,
        &processes,
    ) && shared_process_is_valid(
        &registry.instance.qdrant,
        service_dir,
        "qdrant.exe",
        &listeners,
        &processes,
    ) && shared_process_is_valid(
        &registry.instance.llama_server,
        service_dir,
        "llama-server.exe",
        &listeners,
        &processes,
    )
}

async fn wait_for_shared_ready(
    app: &tauri::AppHandle,
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
) -> Result<Option<String>, AppError> {
    // 最长约 30s；陈旧 starting 应由 reclaim_orphaned_start_slot 清理，不宜空等 2 分钟
    for _ in 0..15 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let registry = kg_coordination::read_registry().map_err(coordination_err)?;
        let Some(registry) = registry else {
            return Ok(None);
        };
        if shared_kg_registry_is_valid(&registry).await {
            let _ = ensure_kg_lease_for_ready_registry(app, kg_pids).await?;
            return Ok(Some("已复用共享 KG 服务".to_string()));
        }
        if registry.state != "starting" {
            return Ok(None);
        }
    }
    Ok(None)
}

async fn prepare_kg_start(
    app: &tauri::AppHandle,
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
) -> Result<KgStartDecision, AppError> {
    kg_coordination::begin_app_session().map_err(coordination_err)?;
    kg_coordination::reconcile_stale_leases().map_err(coordination_err)?;
    let _ = kg_coordination::reclaim_orphaned_registry().map_err(coordination_err)?;

    match kg_coordination::read_registry().map_err(coordination_err)? {
        Some(registry) if registry.state == "ready" => {
            if ensure_kg_lease_for_ready_registry(app, kg_pids)
                .await?
                .is_some()
            {
                return Ok(KgStartDecision::Reused("已复用共享 KG 服务".to_string()));
            }
            // 失效 ready：先尝试删自己的；删不掉（旧 owner）则强制回收孤儿表
            if !kg_coordination::delete_owned_registry().map_err(coordination_err)? {
                let _ = kg_coordination::reclaim_orphaned_registry().map_err(coordination_err)?;
            }
        }
        Some(registry) if registry.state == "starting" && !registry.leases.is_empty() => {
            // 本进程持有的「待恢复 FastKG」半截状态：不要 wait_for_shared_ready 空等自己。
            // 依赖仍可用时应在 start_kg_inner 里走单服务恢复；走到这里说明应整套重拉。
            let pending_self_resume = registry.instance.fastkg.pid == 0
                && kg_coordination::status_from_registry(Some(&registry)).ownership
                    == kg_coordination::SharedOwnership::SelfManaged;
            if pending_self_resume {
                let _ = kg_coordination::delete_owned_registry().map_err(coordination_err)?;
            } else if let Some(msg) = wait_for_shared_ready(app, kg_pids).await? {
                return Ok(KgStartDecision::Reused(msg));
            } else {
                let _ = kg_coordination::reclaim_orphaned_registry().map_err(coordination_err)?;
            }
        }
        Some(registry) if registry.state == "stopping" => {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let _ = kg_coordination::reclaim_orphaned_registry().map_err(coordination_err)?;
        }
        _ => {}
    }

    match kg_coordination::acquire_start_slot(kg_coordination::local_starting_instance())
        .map_err(coordination_err)?
    {
        StartSlot::Acquired => Ok(KgStartDecision::StartLocal),
        StartSlot::Busy => {
            if let Some(msg) = wait_for_shared_ready(app, kg_pids).await? {
                Ok(KgStartDecision::Reused(msg))
            } else if kg_coordination::reclaim_orphaned_registry().map_err(coordination_err)? {
                match kg_coordination::acquire_start_slot(kg_coordination::local_starting_instance())
                    .map_err(coordination_err)?
                {
                    StartSlot::Acquired => Ok(KgStartDecision::StartLocal),
                    StartSlot::Busy => Err(AppError::Business(
                        "KG 服务正在由其他应用启动，请稍后重试".into(),
                    )),
                }
            } else {
                Err(AppError::Business(
                    "KG 服务正在由其他应用启动，请稍后重试".into(),
                ))
            }
        }
    }
}

async fn register_local_kg_ready(
    app: &tauri::AppHandle,
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
) -> Result<KgRuntimeRegistry, AppError> {
    let pids = kg_pids.read().await.clone();
    let claim = kg_coordination::read_registry()
        .map_err(coordination_err)?
        .ok_or_else(|| AppError::Business("KG 启动槽已丢失，拒绝发布 ready".into()))?;
    let registry = kg_coordination::register_ready_instance(
        kg_coordination::local_ready_instance(&pids),
        &claim.instance.instance_id,
        claim.instance.owner_generation,
    )
    .map_err(coordination_err)?;
    start_kg_lease_heartbeat();
    emit_kg_status(
        app,
        "all_ready",
        kg_coordination::status_from_registry(Some(&registry)),
    );
    Ok(registry)
}

/// 检查端口是否有服务在监听（通过 TCP 连接测试）
/// 超时时间设为 100ms：localhost TCP 对于未占用端口的 RST 响应通常在 1ms 以内
async fn is_port_listening(port: u16) -> bool {
    use std::net::TcpStream;
    let addr = std::net::SocketAddr::new(std::net::Ipv4Addr::new(127, 0, 0, 1).into(), port);
    match TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(100)) {
        Ok(_) => true,
        Err(_) => false,
    }
}

async fn wait_for_owned_port(port: u16, pid: u32, attempts: usize) -> bool {
    for _ in 0..attempts {
        if is_port_listening(port).await && get_pid_listening_on_port(port) == Some(pid) {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    false
}

async fn wait_for_owned_port_cancellable(
    port: u16,
    pid: u32,
    attempts: usize,
    stop_requested: &std::sync::atomic::AtomicBool,
) -> bool {
    for _ in 0..attempts {
        if stop_requested.load(std::sync::atomic::Ordering::SeqCst) {
            return false;
        }
        if is_port_listening(port).await && get_pid_listening_on_port(port) == Some(pid) {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    false
}

/// FastKG 冷启动期间同时响应用户取消；Llama-server 继续使用原有等待逻辑。
async fn wait_for_fastkg_ready_cancellable(port: u16, pid: u32) -> bool {
    if !wait_for_owned_port_cancellable(port, pid, 120, &FASTKG_STOP_REQUESTED).await {
        return false;
    }
    let url = format!("http://127.0.0.1:{}", port);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        if FASTKG_STOP_REQUESTED.load(std::sync::atomic::Ordering::SeqCst) {
            return false;
        }
        if fastkg_health_ok(&url).await {
            return true;
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return false;
        }
        tokio::time::sleep(std::cmp::min(
            remaining,
            std::time::Duration::from_millis(500),
        ))
        .await;
    }
}

fn retain_dependency_child(id: &str, mut child: std::process::Child) -> Result<(), AppError> {
    dependency_service_supervisor().owned.capture_output(id, &mut child);
    if let Err(mut child) = dependency_service_supervisor().owned.retain(id, child) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(AppError::Business(format!("{id} 仍有受管理子进程，拒绝重复创建")));
    }
    Ok(())
}

/// 启动普通服务，返回 PID
fn start_service(
    exe_path: &std::path::PathBuf,
    work_dir: &std::path::PathBuf,
) -> Result<u32, AppError> {
    start_service_with_env(exe_path, work_dir, &[])
}

fn start_service_with_env(
    exe_path: &std::path::PathBuf,
    work_dir: &std::path::PathBuf,
    env_vars: &[(&str, String)],
) -> Result<u32, AppError> {
    if !exe_path.exists() {
        let msg = format!("服务文件不存在: {:?}", exe_path);
        tracing::error!("[knowledge] {}", msg);
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            msg,
        )));
    }

    tracing::info!("[knowledge] 启动: {:?} 在 {:?}", exe_path, work_dir);

    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    #[cfg(windows)]
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let mut cmd = std::process::Command::new(exe_path);
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    cmd.current_dir(work_dir);
    for (key, value) in env_vars {
        cmd.env(key, value);
    }
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    match cmd.spawn() {
        Ok(mut child) => {
            let pid = child.id();
            match child.try_wait() {
                Ok(Some(status)) => {
                    let msg = format!("{:?} 启动后立即退出，退出码: {:?}", exe_path, status.code());
                    tracing::error!("[knowledge] {}", msg);
                    return Err(AppError::Wsp(msg));
                }
                Ok(None) => {
                    tracing::info!("[knowledge] 服务已启动 (pid: {})", pid);
                }
                Err(e) => {
                    tracing::warn!("[knowledge] 状态检查失败: {}", e);
                }
            }
            let id = if crate::core::dependency_services::same_executable(exe_path, &paths::qdrant_path()) { "qdrant" } else { "fastkg" };
            retain_dependency_child(id, child)?;
            Ok(pid)
        }
        Err(e) => {
            tracing::error!("[knowledge] 启动失败: {}", e);
            Err(AppError::Io(e))
        }
    }
}

/// 启动 llama-server（直接 spawn，不走 PowerShell，确保进程可追踪），返回 PID
fn start_llama_server(
    exe_path: &std::path::PathBuf,
    model_path: &std::path::PathBuf,
    port: u16,
    work_dir: &std::path::PathBuf,
) -> Result<u32, AppError> {
    if !exe_path.exists() {
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("llama-server 文件不存在: {:?}", exe_path),
        )));
    }
    if !model_path.exists() {
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("模型文件不存在: {:?}", model_path),
        )));
    }

    let model_str = model_path.to_str().unwrap_or("");
    let port_str = port.to_string();

    tracing::info!(
        "[knowledge] 启动 llama-server: {} -m {} --embeddings --port {} -c 4096 -b 2048 -ub 2048",
        exe_path.display(),
        model_path.display(),
        port
    );

    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    #[cfg(windows)]
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let mut cmd = std::process::Command::new(exe_path);
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    cmd.args([
        "-m",
        model_str,
        "--embeddings",
        "--port",
        &port_str,
        "-c",
        "4096",
        "-b",
        "2048",
        "-ub",
        "2048",
    ])
    .current_dir(work_dir);

    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    match cmd.spawn() {
        Ok(mut child) => {
            let pid = child.id();
            match child.try_wait() {
                Ok(Some(status)) => {
                    let msg = format!("llama-server 启动后立即退出，退出码: {:?}", status.code());
                    tracing::error!("[knowledge] {}", msg);
                    return Err(AppError::Wsp(msg));
                }
                Ok(None) => {
                    tracing::info!("[knowledge] llama-server 已启动 (pid: {})", pid);
                }
                Err(e) => {
                    tracing::warn!("[knowledge] llama-server 状态检查失败: {}", e);
                }
            }
            retain_dependency_child("llama_server", child)?;
            Ok(pid)
        }
        Err(e) => {
            tracing::error!("[knowledge] llama-server 启动失败: {}", e);
            Err(AppError::Io(e))
        }
    }
}

fn update_fastkg_runtime_config(
    config_path: &std::path::Path,
    fastkg_port: u16,
    qdrant_port: u16,
    embedding_port: u16,
    external: &crate::core::config::KnowledgeServicesConfig,
) -> Result<(), AppError> {
    use toml_edit::{value, DocumentMut};

    let content = std::fs::read_to_string(config_path)?;
    let mut document = content.parse::<DocumentMut>().map_err(|error| {
        AppError::Parse(format!(
            "FastKG 配置格式无效 {}: {}",
            config_path.display(),
            error
        ))
    })?;
    document["server"]["port"] = value(i64::from(fastkg_port));
    document["qdrant"]["backend"] = value(external.vector_backend.trim());
    let vector_url = if external.vector_url.trim().is_empty() {
        format!("http://127.0.0.1:{}", qdrant_port)
    } else {
        external.vector_url.trim().trim_end_matches('/').to_string()
    };
    document["qdrant"]["url"] = value(vector_url);
    if let Ok(api_key) = std::env::var(external.vector_api_key_env.trim()) {
        if !api_key.trim().is_empty() {
            document["qdrant"]["api_key"] = value(api_key);
        }
    }

    let default_embedding_url = format!(
        "http://127.0.0.1:{}/v1/embeddings",
        paths::LLAMA_SERVER_DEFAULT_PORT
    );
    let embedding_url = if external.embedding_url.trim().is_empty() {
        let existing_embedding = document["embedding"]["url"]
            .as_str()
            .unwrap_or(&default_embedding_url);
        let embedding_path = reqwest::Url::parse(existing_embedding)
            .ok()
            .map(|url| url.path().to_string())
            .filter(|path| !path.is_empty() && path != "/")
            .unwrap_or_else(|| "/v1/embeddings".to_string());
        format!("http://127.0.0.1:{}{}", embedding_port, embedding_path)
    } else {
        external.embedding_url.trim().to_string()
    };
    document["embedding"]["url"] = value(embedding_url);
    document["embedding"]["model"] = value(external.embedding_model.trim());
    if let Ok(api_key) = std::env::var(external.embedding_api_key_env.trim()) {
        if !api_key.trim().is_empty() {
            document["embedding"]["api_key"] = value(api_key);
        }
    }

    let reranker_url = external.reranker_url.trim();
    document["reranker"]["enabled"] = value(!reranker_url.is_empty());
    document["reranker"]["url"] = value(reranker_url);
    document["reranker"]["model"] = value(external.reranker_model.trim());
    if let Ok(api_key) = std::env::var(external.reranker_api_key_env.trim()) {
        if !api_key.trim().is_empty() {
            document["reranker"]["api_key"] = value(api_key);
        }
    }

    let parent = config_path
        .parent()
        .ok_or_else(|| AppError::Business("FastKG 配置文件没有父目录".into()))?;
    let temp_path = parent.join(format!(".config.toml.fastclaw-{}.tmp", std::process::id()));
    let backup_path = parent.join(format!(".config.toml.fastclaw-{}.bak", std::process::id()));
    std::fs::write(&temp_path, document.to_string())
        .map_err(|error| AppError::Business(format!("无法写入 FastKG 临时配置: {}", error)))?;

    std::fs::rename(config_path, &backup_path).map_err(|error| {
        let _ = std::fs::remove_file(&temp_path);
        AppError::Business(format!("无法备份 FastKG 配置: {}", error))
    })?;
    if let Err(error) = std::fs::rename(&temp_path, config_path) {
        let _ = std::fs::rename(&backup_path, config_path);
        let _ = std::fs::remove_file(&temp_path);
        return Err(AppError::Business(format!(
            "无法原子替换 FastKG 配置: {}",
            error
        )));
    }
    let _ = std::fs::remove_file(backup_path);
    Ok(())
}

// ─── KG 服务 PID 追踪辅助函数 ───

/// 通过 netstat 获取监听指定端口的 PID
pub(crate) fn get_pid_listening_on_port(port: u16) -> Option<u32> {
    get_listening_port_pid_map().remove(&port)
}

/// 一次读取完整 TCP 监听表。状态轮询使用同一份快照判断所有依赖服务，
/// 避免为每张卡片分别启动 netstat 子进程。
fn get_listening_port_pid_map() -> HashMap<u16, u32> {
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let output = {
        let mut cmd = Command::new("cmd");
        cmd.args(["/c", "netstat -ano -p TCP"]);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        match cmd.output() {
            Ok(output) => output,
            Err(_) => return HashMap::new(),
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut listeners = HashMap::new();
    for line in stdout.lines() {
        if line.contains("LISTENING") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let port = parts
                .get(1)
                .and_then(|address| address.rsplit(':').next())
                .and_then(|value| value.parse::<u16>().ok());
            let pid = parts.last().and_then(|value| value.parse::<u32>().ok());
            if let (Some(port), Some(pid)) = (port, pid) {
                listeners.entry(port).or_insert(pid);
            }
        }
    }
    listeners
}

/// 判断候选端口能否分配给依赖服务。
///
/// `listeners` 是状态监控使用的系统监听表，`reserved_ports` 用于阻止 FastClaw
/// 内部并发启动选择同一端口；最后再以通配 IPv4 绑定作为系统级兜底探测。
fn dependency_port_is_available(
    port: u16,
    listeners: &HashMap<u16, u32>,
    reserved_ports: &HashSet<u16>,
) -> bool {
    if listeners.contains_key(&port) || reserved_ports.contains(&port) {
        return false;
    }

    std::net::TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, port)).is_ok()
}

fn expected_dependency_exe(service_id: &str) -> Option<PathBuf> {
    match service_id {
        "qdrant" => Some(paths::qdrant_path()),
        "llama_server" => Some(paths::llama_server_path()),
        "fastkg" => Some(paths::fastkg_server_path()),
        "fastplan" => Some(paths::fastplan_server_path()),
        _ => None,
    }
}

async fn record_owned_dependency_process(
    service_id: &str,
    preferred_port: u16,
    port: u16,
    pid: u32,
    exe: &std::path::Path,
) -> Result<ProcessIdentity, AppError> {
    ensure_dependency_registry_ownership(service_id)?;
    let supervisor = dependency_service_supervisor();
    let mut last_error = String::new();
    for attempt in 1..=20 {
        match supervisor
            .record_owned_process(service_id, preferred_port, port, pid, exe)
            .await
        {
            Ok(identity) => {
                supervisor.owned.save(service_id, &identity)?;
                return Ok(identity);
            }
            Err(message) => {
                last_error = message;
                tracing::debug!(
                    "[dependency] {} 进程身份暂未就绪 (attempt {}/20, PID={}): {}",
                    service_id,
                    attempt,
                    pid,
                    last_error
                );
            }
        }
        if attempt < 20 {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }
    let message = format!(
        "{} 启动后的进程身份校验失败（已重试 20 次，PID={}）：{}",
        service_id, pid, last_error
    );
    tracing::error!("[dependency] {}", message);
    Err(AppError::Business(message))
}

async fn record_starting_dependency_process(
    service_id: &str,
    preferred_port: u16,
    port: u16,
    pid: u32,
    exe: &std::path::Path,
) -> Result<ProcessIdentity, AppError> {
    let supervisor = dependency_service_supervisor();
    let mut last_error = String::new();
    for attempt in 1..=20 {
        match supervisor
            .record_starting_process(service_id, preferred_port, port, pid, exe)
            .await
        {
            Ok(identity) => {
                supervisor.owned.save(service_id, &identity)?;
                return Ok(identity);
            }
            Err(message) => {
                last_error = message;
                tracing::debug!(
                    "[dependency] {} 启动中进程身份暂未就绪 (attempt {}/20, PID={}): {}",
                    service_id,
                    attempt,
                    pid,
                    last_error
                );
            }
        }
        if attempt < 20 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }
    Err(AppError::Business(format!(
        "{} 启动中进程身份登记失败（PID={}）：{}",
        service_id, pid, last_error
    )))
}

fn shared_service_process_from_identity(
    identity: &ProcessIdentity,
    url: String,
) -> Result<SharedServiceProcess, AppError> {
    let process_started_at =
        chrono::DateTime::<chrono::Utc>::from_timestamp(identity.process_started_at as i64, 0)
            .ok_or_else(|| AppError::Business("受管理进程的启动时间无效".to_string()))?;
    Ok(SharedServiceProcess {
        url,
        pid: identity.pid,
        exe: identity.canonical_exe_path.to_string_lossy().to_string(),
        process_started_at: Some(process_started_at),
        extra: serde_json::Map::new(),
    })
}

async fn restore_owned_dependency_runtime(service_id: &str, identity: &ProcessIdentity) {
    let supervisor = dependency_service_supervisor();
    if let Err(message) = supervisor
        .record_owned_process(
            service_id,
            dependency_preferred_port(service_id),
            identity.effective_port,
            identity.pid,
            &identity.canonical_exe_path,
        )
        .await
    {
        supervisor
            .mark_failed(
                service_id,
                format!("停止失败后无法恢复进程监督状态：{}", message),
            )
            .await;
    }
}

fn runtime_identity_is_valid(
    runtime: Option<&ServiceRuntime>,
    expected_exe: &std::path::Path,
    listeners: &HashMap<u16, u32>,
    processes: &HashMap<u32, ProcessFacts>,
) -> bool {
    let Some(runtime) = runtime else {
        return false;
    };
    if runtime.ownership != RuntimeOwnership::SelfManaged
        || runtime.lifecycle != LifecycleState::Running
    {
        return false;
    }
    let Some(identity) = runtime.process.as_ref() else {
        return false;
    };
    identity_matches(
        identity,
        processes.get(&identity.pid),
        listeners.get(&identity.effective_port).copied(),
        expected_exe,
    )
}

fn runtime_process_identity_is_valid(
    runtime: Option<&ServiceRuntime>,
    expected_exe: &std::path::Path,
    processes: &HashMap<u32, ProcessFacts>,
) -> bool {
    let Some(runtime) = runtime else {
        return false;
    };
    if runtime.ownership != RuntimeOwnership::SelfManaged {
        return false;
    }
    let Some(identity) = runtime.process.as_ref() else {
        return false;
    };
    identity_matches_process(identity, processes.get(&identity.pid), expected_exe)
}

async fn verified_tracked_dependency_process(
    service_id: &str,
) -> Option<crate::core::dependency_services::ProcessIdentity> {
    ensure_dependency_registry_ownership(service_id).ok()?;
    let expected_exe = expected_dependency_exe(service_id)?;
    let runtime = dependency_service_supervisor().snapshot(service_id).await?;
    let identity = runtime.process.clone()?;
    let processes = capture_process_facts(&[identity.pid]);
    runtime_process_identity_is_valid(Some(&runtime), &expected_exe, &processes).then_some(identity)
}

async fn verified_owned_dependency_process(
    service_id: &str,
) -> Option<crate::core::dependency_services::ProcessIdentity> {
    ensure_dependency_registry_ownership(service_id).ok()?;
    let expected_exe = expected_dependency_exe(service_id)?;
    let supervisor = dependency_service_supervisor();
    let runtime = supervisor.snapshot(service_id).await?;
    let identity = runtime.process.clone()?;
    let listeners = get_listening_port_pid_map();
    let processes = capture_process_facts(&[identity.pid]);
    if runtime_identity_is_valid(Some(&runtime), &expected_exe, &listeners, &processes) {
        Some(identity)
    } else {
        // 查询失败不代表进程已退出。保留原始身份供显式清理使用，
        // 尤其是 Starting/Unhealthy/Failed 阶段尚未监听端口的子进程。
        None
    }
}

/// 获取指定 PID 进程的可执行文件路径
fn get_process_exe_path(pid: u32) -> Option<String> {
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let ps_script = format!("(Get-Process -Id {}).Path", pid);
    let output = {
        let mut cmd = Command::new("powershell");
        cmd.args(&["-NoProfile", "-Command", &ps_script]);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        cmd.output().ok()?
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let path = stdout.trim();
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

fn normalized_path_starts_with(path: &std::path::Path, dir: &std::path::Path) -> bool {
    let path = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .trim()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase();
    let dir = dir
        .canonicalize()
        .unwrap_or_else(|_| dir.to_path_buf())
        .to_string_lossy()
        .trim()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase();
    !dir.is_empty()
        && (path == dir
            || path
                .strip_prefix(&dir)
                .map(|suffix| suffix.starts_with('\\'))
                .unwrap_or(false))
}

/// 判断占用端口的进程是否仍是当前启动事务记录的 KG 进程。
/// 未追踪进程即使位于服务目录，也不能自动取得终止权限。
fn is_our_kg_process(
    pid: u32,
    kg_pids: &crate::state::KgServicePids,
    _kg_dir: &std::path::Path,
) -> bool {
    let expected = if kg_pids.qdrant == Some(pid) {
        paths::qdrant_path()
    } else if kg_pids.fastkg == Some(pid) {
        paths::fastkg_server_path()
    } else if kg_pids.llama_server == Some(pid) {
        paths::llama_server_path()
    } else {
        return false;
    };
    get_process_exe_path(pid)
        .map(|actual| same_executable(std::path::Path::new(&actual), &expected))
        .unwrap_or(false)
}

/// 精确杀死指定 PID 的进程树（同步版本，返回是否成功）
fn kill_pid_sync(pid: u32) -> bool {
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    tracing::info!("[knowledge] 终止进程 PID={}", pid);
    let mut cmd = Command::new("taskkill");
    cmd.args(&["/F", "/T", "/PID", &pid.to_string()]);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    match cmd.output() {
        Ok(output) => {
            let success = output.status.success();
            if !success {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                tracing::warn!(
                    "[knowledge] taskkill PID={} 失败 (exit={:?}): stdout={}, stderr={}",
                    pid,
                    output.status.code(),
                    stdout.trim(),
                    stderr.trim()
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
            success
        }
        Err(e) => {
            tracing::error!("[knowledge] 无法执行 taskkill PID={}: {}", pid, e);
            false
        }
    }
}

async fn clear_fastplan_runtime_tracking(kg_pids: &Arc<RwLock<crate::state::KgServicePids>>) {
    let mut guard = kg_pids.write().await;
    guard.fastplan = None;
    guard.fastplan_connection = None;
    drop(guard);
    crate::state::FASTPLAN_PID.store(0, std::sync::atomic::Ordering::SeqCst);
    set_dependency_runtime_port("fastplan", None);
}

async fn clear_dependency_process_tracking(
    service_id: &str,
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
) {
    let mut guard = kg_pids.write().await;
    match service_id {
        "qdrant" => guard.qdrant = None,
        "fastkg" => guard.fastkg = None,
        _ => {}
    }
    drop(guard);
    set_dependency_runtime_port(service_id, None);
}

/// 清理启动失败或被取消的受管进程；校验失败时绝不按裸 PID 终止。
async fn cleanup_tracked_dependency_start(
    service_id: &str,
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
    message: impl Into<String>,
) -> Result<(), AppError> {
    let message = message.into();
    let supervisor = dependency_service_supervisor();
    ensure_dependency_registry_ownership(service_id)?;
    let stopped_child = supervisor.owned.stop_child(service_id).map_err(|error| {
        tracing::error!("[{service_id}] 停止受管理子进程失败，保留跟踪记录: {error}");
        error
    })?;
    if let Some(identity) = verified_tracked_dependency_process(service_id).await.filter(|_| !stopped_child) {
        if !kill_pid_sync(identity.pid) {
            supervisor
                .mark_unhealthy(
                    service_id,
                    format!("{}；残留进程 PID={} 未能终止", message, identity.pid),
                )
                .await;
            return Err(AppError::Business(format!(
                "{}，且受管理进程 PID={} 未能终止",
                message, identity.pid
            )));
        }
    }
    clear_dependency_process_tracking(service_id, kg_pids).await;
    supervisor.clear(service_id).await;
    Ok(())
}

/// 收敛 FastPlan 启动失败。只终止已经记录且再次通过进程身份校验的子进程。
async fn fail_fastplan_start(
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
    message: impl Into<String>,
) {
    let message = message.into();
    let supervisor = dependency_service_supervisor();
    let tracked = verified_tracked_dependency_process("fastplan").await;
    if let Some(identity) = tracked {
        if !kill_pid_sync(identity.pid) {
            supervisor
                .mark_unhealthy(
                    "fastplan",
                    format!("{}；残留进程 PID={} 未能终止", message, identity.pid),
                )
                .await;
            return;
        }
    }

    clear_fastplan_runtime_tracking(kg_pids).await;
    supervisor.mark_failed("fastplan", message).await;
}

/// 按进程名终止，但仅杀掉 exe 位于 `service_dir` 下的实例（避免误杀其它安装目录的同名服务）
async fn kill_process_by_name_in_dir(
    proc_name: &str,
    service_dir: &std::path::Path,
) -> Result<(), AppError> {
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    // Get-Process -Name 精确匹配（不含 .exe）；禁止 '*name*' 通配
    let name = proc_name.trim_end_matches(".exe").trim_end_matches(".EXE");
    let ps_script = format!(
        "Get-Process -Name '{}' -ErrorAction SilentlyContinue | ForEach-Object {{ Write-Output \"$($_.Id)|$($_.Path)\" }}",
        name.replace('\'', "''")
    );

    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-Command", &ps_script]);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);

    let output = cmd.output().map_err(|e| AppError::Io(e))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.splitn(2, '|').collect();
        if parts.len() != 2 {
            continue;
        }
        let pid: u32 = match parts[0].trim().parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let exe_path = parts[1].trim();
        if exe_path.is_empty() {
            continue;
        }
        if normalized_path_starts_with(std::path::Path::new(exe_path), service_dir) {
            tracing::info!(
                "[wsp] 按名称清理 {} PID={}（路径在服务目录内）: {}",
                name,
                pid,
                exe_path
            );
            kill_pid_sync(pid);
        } else {
            tracing::warn!(
                "[wsp] 跳过同名进程 {} PID={}，exe 不在服务目录: {}",
                name,
                pid,
                exe_path
            );
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    Ok(())
}

// ─── KG 服务内部实现函数 ───

/// 启动 KG 服务（内部实现）
pub async fn start_kg_inner(
    app_handle: &tauri::AppHandle,
    _kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
) -> Result<String, AppError> {
    app_handle.state::<AppState>().dependency_recovery.request(app_handle, "fastkg", true, false, false)
}

async fn start_kg_legacy_once(
    app_handle: &tauri::AppHandle,
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
) -> Result<String, AppError> {
    let _maintenance_guard = FASTKG_MAINTENANCE_LOCK.lock().await;
    // ─── 启动守卫：防止与 start_knowledge_service_async 并发启动 ───
    let Some(_kg_starting_guard) = KgStartingGuard::acquire() else {
        tracing::info!("[knowledge] KG 服务正在启动中（由另一个流程执行），跳过");
        return Ok("KG 服务正在启动中".to_string());
    };
    let Some(_qdrant_starting_guard) = QdrantStartingGuard::acquire() else {
        return Err(AppError::Business("Qdrant 正在由另一个流程启动".into()));
    };
    QDRANT_STOP_REQUESTED.store(false, std::sync::atomic::Ordering::SeqCst);
    FASTKG_STOP_REQUESTED.store(false, std::sync::atomic::Ordering::SeqCst);

    // 手动停止 FastKG 后，共享注册表会保留 Qdrant/embedding 并进入 starting。
    // 该状态仍由当前 FastClaw 持有，不能按普通启动槽等待，否则会等待自己。
    // 仅当向量库+嵌入仍可用时才「只恢复 FastKG」；依赖也停了则清半截注册表，改走完整编排。
    let pending_fastkg_resume = match kg_coordination::read_registry().map_err(coordination_err) {
        Ok(registry) => registry.map(|registry| {
            registry.state == "starting"
                && registry.instance.fastkg.pid == 0
                && kg_coordination::status_from_registry(Some(&registry)).ownership
                    == kg_coordination::SharedOwnership::SelfManaged
        }),
        Err(error) => return Err(error),
    }
    .unwrap_or(false);
    if pending_fastkg_resume {
        let qdrant_ok = dependency_service_is_usable("qdrant").await;
        let llama_ok = dependency_service_is_usable("llama_server").await;
        if qdrant_ok && llama_ok {
            return start_single_fastkg_if_not_running(app_handle, kg_pids).await;
        }
        tracing::warn!(
            "[knowledge] FastKG 待恢复但依赖不可用 (qdrant={}, llama={})，清除半截注册表并改走完整编排",
            qdrant_ok,
            llama_ok
        );
        kg_coordination::delete_owned_registry().map_err(coordination_err)?;
        dependency_service_supervisor().clear("fastkg").await;
        let _ = app_handle.emit(
            "kg-service-status",
            serde_json::json!({
                "phase": "starting_full_stack",
                "message": "依赖服务不可用，正在重新启动向量库、嵌入与知识库服务",
            }),
        );
        // 继续走下方完整编排，不要在这里 return Err
    }

    match prepare_kg_start(app_handle, kg_pids).await {
        Ok(KgStartDecision::Reused(msg)) => return Ok(msg),
        Ok(KgStartDecision::StartLocal) => {}
        Err(e) => return Err(e),
    }

    // 先接管仍在运行的本机依赖，避免二次启动同目录进程（Qdrant 会因存储锁立即退出 101）
    let adopted_qdrant = adopt_local_dependency_process("qdrant").await;
    let adopted_llama = adopt_local_dependency_process("llama_server").await;
    if let Some(identity) = verified_owned_dependency_process("qdrant").await {
        kg_pids.write().await.qdrant = Some(identity.pid);
    }
    if let Some(identity) = verified_owned_dependency_process("llama_server").await {
        kg_pids.write().await.llama_server = Some(identity.pid);
    }

    let qdrant_path = paths::qdrant_path();
    let fastkg_path = paths::fastkg_server_path();
    let llama_path = paths::llama_server_path();
    let bge_model_path = paths::bge_m3_model_path();
    let ebinding_dir = paths::ebinding_start_path();

    let (qdrant_port, qdrant_attempt, mut qdrant_reservation) =
        if let Some(port) = adopted_qdrant {
            (port, 1u32, reservation_for_existing_port("qdrant", port))
        } else {
            match reserve_available_dependency_port(
                app_handle,
                "qdrant",
                dependency_preferred_port("qdrant"),
            )
            .await
            {
                Ok(value) => value,
                Err(error) => return Err(error),
            }
        };
    let (llama_port, llama_attempt, mut llama_reservation) =
        if let Some(port) = adopted_llama {
            (port, 1u32, reservation_for_existing_port("llama_server", port))
        } else {
            match reserve_available_dependency_port(
                app_handle,
                "llama_server",
                dependency_preferred_port("llama_server"),
            )
            .await
            {
                Ok(value) => value,
                Err(error) => {
                    set_dependency_runtime_port("qdrant", None);
                    return Err(error);
                }
            }
        };
    let (fastkg_port, fastkg_attempt, mut fastkg_reservation) =
        match reserve_available_dependency_port(
            app_handle,
            "fastkg",
            dependency_preferred_port("fastkg"),
        )
        .await
        {
            Ok(value) => value,
            Err(error) => {
                set_dependency_runtime_port("qdrant", None);
                set_dependency_runtime_port("llama_server", None);
                return Err(error);
            }
        };

    tracing::info!("[knowledge] 启动知识服务...");

    // ─── 按顺序检查并启动每个服务 ───

    // 1. qdrant
    {
        if adopted_qdrant.is_some() {
            tracing::info!(
                "[knowledge] qdrant 已在端口 {} 运行，跳过启动",
                qdrant_port
            );
            emit_dependency_port_state(
                app_handle,
                "qdrant",
                "ready",
                dependency_preferred_port("qdrant"),
                Some(qdrant_port),
                qdrant_attempt,
                format!("Qdrant 已在端口 {} 运行（复用）", qdrant_port),
            );
            qdrant_reservation.commit();
        } else {
        // 二次检查端口（启动守卫期间状态可能已变化）
        let port_listening = is_port_listening(qdrant_port).await;
        let need_start = if port_listening {
            match get_pid_listening_on_port(qdrant_port) {
                Some(pid) => {
                    let expected = paths::qdrant_path();
                    let is_ours = get_process_exe_path(pid)
                        .map(|actual| same_executable(std::path::Path::new(&actual), &expected))
                        .unwrap_or(false);
                    if is_ours {
                        // 本机 qdrant 已在目标端口：接管，不要杀了重拉（会触发存储锁 101）
                        if let Err(error) = record_owned_dependency_process(
                            "qdrant",
                            dependency_preferred_port("qdrant"),
                            qdrant_port,
                            pid,
                            &expected,
                        )
                        .await
                        {
                            tracing::warn!("[knowledge] 接管端口上的 qdrant 失败: {}", error);
                            true
                        } else {
                            kg_pids.write().await.qdrant = Some(pid);
                            set_dependency_runtime_port("qdrant", Some(qdrant_port));
                            tracing::info!(
                                "[knowledge] qdrant 端口已被本机进程占用 (PID={})，复用",
                                pid
                            );
                            false
                        }
                    } else {
                        let msg = format!(
                            "端口 {} 已被外部进程占用 (PID {})，跳过 qdrant 启动",
                            qdrant_port, pid
                        );
                        tracing::warn!("[knowledge] {}", msg);
                        let _ = app_handle.emit(
                            "kg-service-status",
                            serde_json::json!({
                                "phase": "port_conflict",
                                "service": "qdrant",
                                "port": qdrant_port,
                                "pid": pid,
                                "message": msg,
                            }),
                        );
                        false
                    }
                }
                None => true, // 端口检查出错了，尝试启动
            }
        } else {
            true // 端口空闲，启动
        };

        if !need_start && kg_pids.read().await.qdrant.is_some() {
            emit_dependency_port_state(
                app_handle,
                "qdrant",
                "ready",
                dependency_preferred_port("qdrant"),
                Some(qdrant_port),
                qdrant_attempt,
                format!("Qdrant 已在端口 {} 运行（复用）", qdrant_port),
            );
            qdrant_reservation.commit();
        } else if need_start {
            tracing::info!("[knowledge] 启动 qdrant...");
            emit_dependency_port_state(
                app_handle,
                "qdrant",
                "starting",
                dependency_preferred_port("qdrant"),
                Some(qdrant_port),
                qdrant_attempt,
                format!("正在端口 {} 启动 Qdrant", qdrant_port),
            );
            dependency_service_supervisor()
                .mark_starting("qdrant", dependency_preferred_port("qdrant"), qdrant_port)
                .await;
            let qdrant_work_dir = qdrant_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            match start_service_with_env(
                &qdrant_path,
                &qdrant_work_dir,
                &[("QDRANT__SERVICE__HTTP_PORT", qdrant_port.to_string())],
            ) {
                Ok(pid) => {
                    kg_pids.write().await.qdrant = Some(pid);
                    if let Err(error) = record_starting_dependency_process(
                        "qdrant",
                        dependency_preferred_port("qdrant"),
                        qdrant_port,
                        pid,
                        &qdrant_path,
                    )
                    .await
                    {
                        let _ = kill_pid_sync(pid);
                        clear_dependency_process_tracking("qdrant", kg_pids).await;
                        if QDRANT_STOP_REQUESTED.load(std::sync::atomic::Ordering::SeqCst) {
                            dependency_service_supervisor().clear("qdrant").await;
                            abort_kg_orchestration("Qdrant 启动已取消").await;
                            return Ok("Qdrant 启动已取消".into());
                        }
                        abort_kg_orchestration(&error.to_string()).await;
                        return Err(error);
                    }
                    if !wait_for_owned_port_cancellable(
                        qdrant_port,
                        pid,
                        30,
                        &QDRANT_STOP_REQUESTED,
                    )
                    .await
                    {
                        let cancelled = QDRANT_STOP_REQUESTED
                            .load(std::sync::atomic::Ordering::SeqCst);
                        let msg = format!("Qdrant 未在候选端口 {} 就绪", qdrant_port);
                        if let Err(error) =
                            cleanup_tracked_dependency_start("qdrant", kg_pids, &msg).await
                        {
                            abort_kg_orchestration(&error.to_string()).await;
                            return Err(error);
                        }
                        abort_kg_orchestration(&msg).await;
                        if cancelled {
                            return Ok("Qdrant 启动已取消".into());
                        }
                        return Err(AppError::Business(msg));
                    }
                    if let Err(error) = record_owned_dependency_process(
                        "qdrant",
                        dependency_preferred_port("qdrant"),
                        qdrant_port,
                        pid,
                        &qdrant_path,
                    )
                    .await
                    {
                        let _ = kill_pid_sync(pid);
                        kg_pids.write().await.qdrant = None;
                        set_dependency_runtime_port("qdrant", None);
                        abort_kg_orchestration(&error.to_string()).await;
                        return Err(error);
                    }
                    emit_dependency_port_state(
                        app_handle,
                        "qdrant",
                        "ready",
                        dependency_preferred_port("qdrant"),
                        Some(qdrant_port),
                        qdrant_attempt,
                        format!("Qdrant 已在端口 {} 就绪", qdrant_port),
                    );
                    qdrant_reservation.commit();
                }
                Err(e) => {
                    tracing::error!("[knowledge] qdrant 启动失败: {}", e);
                    set_dependency_runtime_port("qdrant", None);
                    let msg = format!("Qdrant 启动失败: {}", e);
                    abort_kg_orchestration(&msg).await;
                    return Err(AppError::Business(msg));
                }
            }
        }
        } // end else !adopted_qdrant
    }

    // 2. llama-server。FastKG 启动前必须先拿到嵌入服务的实际端口。
    {
        if adopted_llama.is_some() {
            tracing::info!(
                "[knowledge] 嵌入向量服务已在端口 {} 运行，跳过启动",
                llama_port
            );
            emit_dependency_port_state(
                app_handle,
                "llama_server",
                "ready",
                dependency_preferred_port("llama_server"),
                Some(llama_port),
                llama_attempt,
                format!("嵌入向量服务已在端口 {} 运行（复用）", llama_port),
            );
            llama_reservation.commit();
        } else {
        emit_dependency_port_state(
            app_handle,
            "llama_server",
            "starting",
            dependency_preferred_port("llama_server"),
            Some(llama_port),
            llama_attempt,
            format!("正在端口 {} 启动嵌入向量服务", llama_port),
        );
        // 目标端口上已是本机 llama：复用，禁止再起一份
        let reuse_llama = if is_port_listening(llama_port).await {
            if let Some(pid) = get_pid_listening_on_port(llama_port) {
                let expected = paths::llama_server_path();
                let is_ours = get_process_exe_path(pid)
                    .map(|actual| same_executable(std::path::Path::new(&actual), &expected))
                    .unwrap_or(false);
                if is_ours
                    && dependency_health_ok("llama_server", llama_port).await
                    && record_owned_dependency_process(
                        "llama_server",
                        dependency_preferred_port("llama_server"),
                        llama_port,
                        pid,
                        &expected,
                    )
                    .await
                    .is_ok()
                {
                    kg_pids.write().await.llama_server = Some(pid);
                    set_dependency_runtime_port("llama_server", Some(llama_port));
                    tracing::info!(
                        "[knowledge] llama-server 端口已被本机进程占用 (PID={})，复用",
                        pid
                    );
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        if reuse_llama {
            emit_dependency_port_state(
                app_handle,
                "llama_server",
                "ready",
                dependency_preferred_port("llama_server"),
                Some(llama_port),
                llama_attempt,
                format!("嵌入向量服务已在端口 {} 运行（复用）", llama_port),
            );
            llama_reservation.commit();
        } else {
        dependency_service_supervisor()
            .mark_starting(
                "llama_server",
                dependency_preferred_port("llama_server"),
                llama_port,
            )
            .await;
        let llama_work_dir = ebinding_dir.join("llamacpp");
        let pid = match start_llama_server(
            &llama_path,
            &bge_model_path,
            llama_port,
            &llama_work_dir,
        ) {
            Ok(pid) => pid,
            Err(error) => {
                let msg = format!("嵌入向量服务启动失败: {}", error);
                abort_kg_orchestration(&msg).await;
                return Err(AppError::Business(msg));
            }
        };
        kg_pids.write().await.llama_server = Some(pid);
        if !wait_for_owned_port(llama_port, pid, 30).await {
            let _ = kill_pid_sync(pid);
            kg_pids.write().await.llama_server = None;
            set_dependency_runtime_port("llama_server", None);
            let msg = format!("嵌入向量服务未在候选端口 {} 就绪", llama_port);
            abort_kg_orchestration(&msg).await;
            return Err(AppError::Business(msg));
        }
        if let Err(error) = record_owned_dependency_process(
            "llama_server",
            dependency_preferred_port("llama_server"),
            llama_port,
            pid,
            &llama_path,
        )
        .await
        {
            let _ = kill_pid_sync(pid);
            kg_pids.write().await.llama_server = None;
            set_dependency_runtime_port("llama_server", None);
            abort_kg_orchestration(&error.to_string()).await;
            return Err(error);
        }
        emit_dependency_port_state(
            app_handle,
            "llama_server",
            "ready",
            dependency_preferred_port("llama_server"),
            Some(llama_port),
            llama_attempt,
            format!("嵌入向量服务已在端口 {} 就绪", llama_port),
        );
        llama_reservation.commit();
        }
        } // end else !adopted_llama
    }

    emit_dependency_port_state(
        app_handle,
        "fastkg",
        "configuring",
        dependency_preferred_port("fastkg"),
        Some(fastkg_port),
        fastkg_attempt,
        "正在同步 FastKG 服务及依赖端点",
    );
    let fastkg_config_path = fastkg_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("config.toml");
    let knowledge_config = app_handle
        .state::<AppState>()
        .app_config
        .read()
        .await
        .knowledge
        .clone();
    if let Err(error) = update_fastkg_runtime_config(
        &fastkg_config_path,
        fastkg_port,
        qdrant_port,
        llama_port,
        &knowledge_config,
    )
    {
        abort_kg_orchestration(&error.to_string()).await;
        return Err(error);
    }

    // 3. FastKG
    {
        // 二次检查端口（qdrant 启动后状态可能已变化）
        let port_listening = is_port_listening(fastkg_port).await;
        let need_start = if port_listening {
            match get_pid_listening_on_port(fastkg_port) {
                Some(pid) => {
                    let expected = paths::fastkg_server_path();
                    let is_ours = get_process_exe_path(pid)
                        .map(|actual| same_executable(std::path::Path::new(&actual), &expected))
                        .unwrap_or(false);
                    if is_ours {
                        if let Err(error) = record_owned_dependency_process(
                            "fastkg",
                            dependency_preferred_port("fastkg"),
                            fastkg_port,
                            pid,
                            &expected,
                        )
                        .await
                        {
                            tracing::warn!("[knowledge] 接管端口上的 FastKG 失败: {}", error);
                            true
                        } else {
                            kg_pids.write().await.fastkg = Some(pid);
                            set_dependency_runtime_port("fastkg", Some(fastkg_port));
                            tracing::info!(
                                "[knowledge] FastKG 端口已被本机进程占用 (PID={})，复用",
                                pid
                            );
                            false
                        }
                    } else {
                        let msg = format!(
                            "端口 {} 已被外部进程占用 (PID {})，跳过 FastKG 启动",
                            fastkg_port, pid
                        );
                        tracing::warn!("[knowledge] {}", msg);
                        let _ = app_handle.emit(
                            "kg-service-status",
                            serde_json::json!({
                                "phase": "port_conflict",
                                "service": "fastkg",
                                "port": fastkg_port,
                                "pid": pid,
                                "message": msg,
                            }),
                        );
                        false
                    }
                }
                None => true,
            }
        } else {
            true
        };

        if need_start {
            if FASTKG_STOP_REQUESTED.load(std::sync::atomic::Ordering::SeqCst) {
                abort_kg_orchestration("FastKG 启动已取消").await;
                return Ok("FastKG 启动已取消".into());
            }
            tracing::info!("[knowledge] 启动 FastKG...");
            emit_dependency_port_state(
                app_handle,
                "fastkg",
                "starting",
                dependency_preferred_port("fastkg"),
                Some(fastkg_port),
                fastkg_attempt,
                format!("正在端口 {} 启动 FastKG", fastkg_port),
            );
            dependency_service_supervisor()
                .mark_starting("fastkg", dependency_preferred_port("fastkg"), fastkg_port)
                .await;
            let fastkg_work_dir = fastkg_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            match start_service(&fastkg_path, &fastkg_work_dir) {
                Ok(pid) => {
                    kg_pids.write().await.fastkg = Some(pid);
                    if let Err(error) = record_starting_dependency_process(
                        "fastkg",
                        dependency_preferred_port("fastkg"),
                        fastkg_port,
                        pid,
                        &fastkg_path,
                    )
                    .await
                    {
                        let _ = kill_pid_sync(pid);
                        clear_dependency_process_tracking("fastkg", kg_pids).await;
                        abort_kg_orchestration(&error.to_string()).await;
                        return Err(error);
                    }
                    if !wait_for_fastkg_ready_cancellable(fastkg_port, pid).await {
                        let cancelled =
                            FASTKG_STOP_REQUESTED.load(std::sync::atomic::Ordering::SeqCst);
                        let msg = format!("FastKG 未在候选端口 {} 就绪", fastkg_port);
                        if let Err(error) =
                            cleanup_tracked_dependency_start("fastkg", kg_pids, &msg).await
                        {
                            abort_kg_orchestration(&error.to_string()).await;
                            return Err(error);
                        }
                        abort_kg_orchestration(&msg).await;
                        if cancelled {
                            return Ok("FastKG 启动已取消".into());
                        }
                        return Err(AppError::Business(msg));
                    }
                }
                Err(e) => {
                    tracing::error!("[knowledge] FastKG 启动失败: {}", e);
                    set_dependency_runtime_port("fastkg", None);
                    let msg = format!("FastKG 启动失败: {}", e);
                    abort_kg_orchestration(&msg).await;
                    return Err(AppError::Business(msg));
                }
            }
        }
    }

    // 3. llama-server（编排后段兜底：前面已启动/复用则跳过）
    {
        if kg_pids.read().await.llama_server.is_some()
            || verified_owned_dependency_process("llama_server")
                .await
                .is_some()
        {
            tracing::info!("[knowledge] llama-server 已就绪，跳过后段重复启动");
        } else {
            let port_listening = is_port_listening(llama_port).await;
            let need_start = if port_listening {
                match get_pid_listening_on_port(llama_port) {
                    Some(pid) => {
                        let expected = paths::llama_server_path();
                        let is_ours = get_process_exe_path(pid)
                            .map(|actual| {
                                same_executable(std::path::Path::new(&actual), &expected)
                            })
                            .unwrap_or(false);
                        if is_ours {
                            let _ = record_owned_dependency_process(
                                "llama_server",
                                dependency_preferred_port("llama_server"),
                                llama_port,
                                pid,
                                &expected,
                            )
                            .await;
                            kg_pids.write().await.llama_server = Some(pid);
                            false
                        } else {
                            tracing::warn!(
                                "[knowledge] 端口 {} 已被外部进程占用 (PID {})，跳过 llama-server 启动",
                                llama_port,
                                pid
                            );
                            false
                        }
                    }
                    None => true,
                }
            } else {
                true
            };

            if need_start {
                tracing::info!("[knowledge] 启动 llama-server...");
                let llama_work_dir = ebinding_dir.join("llamacpp");
                match start_llama_server(&llama_path, &bge_model_path, llama_port, &llama_work_dir)
                {
                    Ok(pid) => {
                        kg_pids.write().await.llama_server = Some(pid);
                        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                    }
                    Err(e) => {
                        tracing::error!("[knowledge] llama-server 启动失败: {}", e);
                    }
                }
            }
        }
    }

    tracing::info!("[knowledge] 所有服务启动完成");

    // 前面的 wait_for_fastkg_ready 已完成带总时限的就绪等待；此处只做最终确认。
    let fastkg_url = format!("http://127.0.0.1:{}", fastkg_port);
    let fastkg_ok = {
        let mut ok = fastkg_health_ok(&fastkg_url).await;
        if ok {
            tracing::info!("[knowledge] FastKG /health 最终确认通过");
        }
        // 降级：端口监听即视为可用（兼容旧版 FastKG 无 /health 端点）
        if !ok {
            ok = is_port_listening(fastkg_port).await;
            if ok {
                tracing::warn!("[knowledge] FastKG /health 轮询超时，降级为端口检测通过");
            }
        }
        ok
    };

    if fastkg_ok {
        let fastkg_pid = match kg_pids.read().await.fastkg {
            Some(pid) => pid,
            None => {
                let msg = "FastKG 已就绪但缺少受管理 PID".to_string();
                abort_kg_orchestration(&msg).await;
                return Err(AppError::Business(msg));
            }
        };
        if let Err(error) = record_owned_dependency_process(
            "fastkg",
            dependency_preferred_port("fastkg"),
            fastkg_port,
            fastkg_pid,
            &fastkg_path,
        )
        .await
        {
            let _ = kill_pid_sync(fastkg_pid);
            kg_pids.write().await.fastkg = None;
            set_dependency_runtime_port("fastkg", None);
            abort_kg_orchestration(&error.to_string()).await;
            return Err(error);
        }
        tracing::info!("[knowledge] FastKG 服务就绪");
        emit_dependency_port_state(
            app_handle,
            "fastkg",
            "ready",
            dependency_preferred_port("fastkg"),
            Some(fastkg_port),
            fastkg_attempt,
            format!("FastKG 已在端口 {} 就绪", fastkg_port),
        );
        fastkg_reservation.commit();
        if let Err(error) = register_local_kg_ready(app_handle, kg_pids).await {
            tracing::error!(
                "[knowledge] 发布共享 KG ready 失败，回滚本次进程: {}",
                error
            );
            let supervisor = dependency_service_supervisor();
            for service_id in ["fastkg", "llama_server", "qdrant"] {
                if let Some(identity) = verified_owned_dependency_process(service_id).await {
                    let _ = kill_pid_sync(identity.pid);
                }
                supervisor.clear(service_id).await;
                set_dependency_runtime_port(service_id, None);
            }
            let mut guard = kg_pids.write().await;
            guard.fastkg = None;
            guard.llama_server = None;
            guard.qdrant = None;
            return Err(error);
        }
    } else {
        tracing::error!("[knowledge] FastKG 服务启动失败：/health 和端口检测均未通过");
        let _ = kg_coordination::delete_owned_registry();
        if let Some(pid) = kg_pids.write().await.fastkg.take() {
            let _ = kill_pid_sync(pid);
        }
        emit_dependency_port_state(
            app_handle,
            "fastkg",
            "failed",
            dependency_preferred_port("fastkg"),
            Some(fastkg_port),
            fastkg_attempt,
            format!("FastKG 未在端口 {} 就绪", fastkg_port),
        );
        let _ = app_handle.emit(
            "kg-service-status",
            serde_json::json!({
                "phase": "error",
                "message": "知识库服务启动失败，请检查服务文件是否已安装"
            }),
        );
        set_dependency_runtime_port("fastkg", None);
        let msg = format!("FastKG 未在候选端口 {} 就绪", fastkg_port);
        abort_kg_orchestration(&msg).await;
        return Err(AppError::Business(msg));
    }

    Ok("知识服务启动完成".to_string())
}

fn registered_process_is_valid_for_shutdown(
    process: &SharedServiceProcess,
    service_dir: &std::path::Path,
    expected_exe_name: &str,
    listeners: Option<&HashMap<u16, u32>>,
    processes: &HashMap<u32, ProcessFacts>,
) -> bool {
    if process.pid == 0 {
        return false;
    }
    let Some(registered_started_at) = process.process_started_at else {
        return false;
    };
    let Some(facts) = processes.get(&process.pid) else {
        return false;
    };
    if registered_started_at.timestamp() < 0
        || registered_started_at.timestamp() as u64 != facts.process_started_at
        || !same_executable(
            &facts.canonical_exe_path,
            std::path::Path::new(&process.exe),
        )
        || !normalized_path_starts_with(&facts.canonical_exe_path, service_dir)
        || facts
            .canonical_exe_path
            .file_name()
            .map(|name| {
                !name
                    .to_string_lossy()
                    .eq_ignore_ascii_case(expected_exe_name)
            })
            .unwrap_or(true)
    {
        return false;
    }
    listeners
        .map(|listeners| {
            let port = parse_url_port(&process.url, 0);
            port != 0 && listeners.get(&port).copied() == Some(process.pid)
        })
        .unwrap_or(true)
}

fn registered_instance_is_safe_to_adopt(instance: &SharedKgInstance) -> bool {
    if !kg_coordination::registered_owner_is_gone(instance) {
        return false;
    }
    let service_dir = std::path::Path::new(&instance.service_dir);
    if !service_dir.is_dir() || service_dir.parent().is_none() {
        return false;
    }
    let services = [
        (&instance.fastkg, "FastKG.exe"),
        (&instance.llama_server, "llama-server.exe"),
        (&instance.qdrant, "qdrant.exe"),
    ];
    let pids = services
        .iter()
        .map(|(process, _)| process.pid)
        .collect::<Vec<_>>();
    let processes = capture_process_facts(&pids);
    let listeners = get_listening_port_pid_map();
    services.iter().all(|(process, expected_exe_name)| {
        registered_process_is_valid_for_shutdown(
            process,
            service_dir,
            expected_exe_name,
            Some(&listeners),
            &processes,
        )
    })
}

async fn wait_for_registered_process_identity_to_disappear(
    process: &SharedServiceProcess,
    attempts: usize,
) -> bool {
    let Some(registered_started_at) = process.process_started_at else {
        return false;
    };
    for _ in 0..attempts {
        let facts = capture_process_facts(&[process.pid]);
        let same_identity = facts
            .get(&process.pid)
            .map(|facts| {
                registered_started_at.timestamp() >= 0
                    && registered_started_at.timestamp() as u64 == facts.process_started_at
                    && same_executable(
                        &facts.canonical_exe_path,
                        std::path::Path::new(&process.exe),
                    )
            })
            .unwrap_or(false);
        if !same_identity {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
    false
}

async fn stop_registered_instance(instance: &SharedKgInstance) -> Result<(), AppError> {
    let service_dir = std::path::Path::new(&instance.service_dir);
    for (process, display_name, expected_exe_name) in [
        (&instance.fastkg, "FastKG", "FastKG.exe"),
        (&instance.llama_server, "llama-server", "llama-server.exe"),
        (&instance.qdrant, "Qdrant", "qdrant.exe"),
    ] {
        let processes = capture_process_facts(&[process.pid]);
        if !processes.contains_key(&process.pid) {
            log::info!(
                "[knowledge] 注册的 {} 进程已退出 (PID={})",
                display_name,
                process.pid
            );
            continue;
        }
        if !registered_process_is_valid_for_shutdown(
            process,
            service_dir,
            expected_exe_name,
            None,
            &processes,
        ) {
            return Err(AppError::Business(format!(
                "注册的 {} 进程身份已变化，拒绝终止 PID={}，保留 stopping 注册表",
                display_name, process.pid
            )));
        }
        if !kill_pid_sync(process.pid) {
            return Err(AppError::Business(format!(
                "无法停止注册的 {} 进程 PID={}，保留 stopping 注册表",
                display_name, process.pid
            )));
        }
        if !wait_for_registered_process_identity_to_disappear(process, 30).await {
            return Err(AppError::Business(format!(
                "等待 {} 进程 PID={} 退出超时，保留 stopping 注册表",
                display_name, process.pid
            )));
        }
        log::info!(
            "[knowledge] 已按注册进程指纹停止 {} (PID={})",
            display_name,
            process.pid
        );
    }
    Ok(())
}

async fn clear_local_kg_tracking(kg_pids: &Arc<RwLock<crate::state::KgServicePids>>) {
    let mut guard = kg_pids.write().await;
    guard.qdrant = None;
    guard.fastkg = None;
    guard.llama_server = None;
    drop(guard);
    let supervisor = dependency_service_supervisor();
    for service_id in ["fastkg", "llama_server", "qdrant"] {
        supervisor.clear(service_id).await;
        set_dependency_runtime_port(service_id, None);
    }
}

/// 停止 KG 服务：正常退出主动移交；失联 owner 仅在完整验证和 CAS 接管后终止。
pub async fn stop_kg_inner(
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
) -> Result<String, AppError> {
    let _maintenance_guard = FASTKG_MAINTENANCE_LOCK.lock().await;
    stop_kg_lease_heartbeat().await;
    let mut registered_instance: Option<SharedKgInstance> = None;
    let release = match kg_coordination::release_lease() {
        Ok(release) => release,
        Err(error) => {
            // 释放失败时仍持有租约，恢复心跳，不能让共享方误判本应用失联。
            start_kg_lease_heartbeat();
            return Err(coordination_err(error));
        }
    };
    match release {
        ReleaseOutcome::Released { remaining } => {
            clear_local_kg_tracking(kg_pids).await;
            return Ok(format!(
                "KG 服务仍被其他应用使用，剩余消费者: {}",
                remaining
            ));
        }
        ReleaseOutcome::OwnershipTransferred {
            instance,
            new_owner,
            remaining,
        } => {
            clear_local_kg_tracking(kg_pids).await;
            return Ok(format!(
                "KG 服务所有权已移交给 {}（实例 {}，剩余消费者 {}）",
                new_owner, instance.instance_id, remaining
            ));
        }
        ReleaseOutcome::ReadyToStop { instance } => {
            let mut guard = kg_pids.write().await;
            guard.qdrant = (instance.qdrant.pid != 0).then_some(instance.qdrant.pid);
            guard.fastkg = (instance.fastkg.pid != 0).then_some(instance.fastkg.pid);
            guard.llama_server =
                (instance.llama_server.pid != 0).then_some(instance.llama_server.pid);
            registered_instance = Some(instance);
        }
        ReleaseOutcome::NeedsAdoption { snapshot } => {
            if !registered_instance_is_safe_to_adopt(&snapshot) {
                kg_coordination::release_own_lease_only().map_err(coordination_err)?;
                clear_local_kg_tracking(kg_pids).await;
                return Ok(format!(
                    "KG 共享实例 {} 的 owner 或服务身份无法安全验证；已仅释放 FastClaw 租约，未终止任何进程",
                    snapshot.instance_id
                ));
            }
            match kg_coordination::adopt_for_shutdown(&snapshot).map_err(coordination_err)? {
                Some(instance) => {
                    let mut guard = kg_pids.write().await;
                    guard.qdrant = Some(instance.qdrant.pid);
                    guard.fastkg = Some(instance.fastkg.pid);
                    guard.llama_server = Some(instance.llama_server.pid);
                    registered_instance = Some(instance);
                }
                None => {
                    kg_coordination::release_own_lease_only().map_err(coordination_err)?;
                    clear_local_kg_tracking(kg_pids).await;
                    return Ok(
                        "KG 共享实例在接管期间发生变化；已放弃接管且未终止任何进程".to_string()
                    );
                }
            }
        }
        ReleaseOutcome::NoRegistry => {
            let owns_any_kg_process = verified_owned_dependency_process("fastkg").await.is_some()
                || verified_owned_dependency_process("llama_server")
                    .await
                    .is_some()
                || verified_owned_dependency_process("qdrant").await.is_some();
            if !owns_any_kg_process
                && fastkg_health_ok(&format!(
                    "http://127.0.0.1:{}",
                    dependency_runtime_port("fastkg")
                        .unwrap_or_else(|| dependency_preferred_port("fastkg"))
                ))
                .await
            {
                let mut guard = kg_pids.write().await;
                guard.qdrant = None;
                guard.fastkg = None;
                guard.llama_server = None;
                return Ok("KG 服务由未注册的外部进程管理，已跳过停止".to_string());
            }
        }
    }

    let pids = {
        let mut guard = kg_pids.write().await;
        let pids = guard.clone();
        guard.qdrant = None;
        guard.fastkg = None;
        guard.llama_server = None;
        pids
    };

    // 空转检测：PIDs 已清空且端口均无监听 → 无需重复清理
    if pids.qdrant.is_none() && pids.fastkg.is_none() && pids.llama_server.is_none()
        && !["qdrant", "llama_server", "fastkg"].iter()
            .any(|id| dependency_service_supervisor().owned.running(id)) {
        let any_listening = is_port_listening(
            dependency_runtime_port("qdrant")
                .unwrap_or_else(|| dependency_preferred_port("qdrant")),
        )
        .await
            || is_port_listening(
                dependency_runtime_port("fastkg")
                    .unwrap_or_else(|| dependency_preferred_port("fastkg")),
            )
            .await
            || is_port_listening(
                dependency_runtime_port("llama_server")
                    .unwrap_or_else(|| dependency_preferred_port("llama_server")),
            )
            .await;
        if !any_listening {
            if let Some(instance) = registered_instance.as_ref() {
                if !kg_coordination::delete_registry_if_matches(&instance.instance_id, instance.owner_generation)
                    .map_err(coordination_err)? {
                    return Err(AppError::Business("空置 KG 槽位所有权发生变化，拒绝清理".into()));
                }
            }
            clear_local_kg_tracking(kg_pids).await;
            log::info!("[knowledge] KG 服务未运行，跳过清理");
            return Ok("KG 服务未运行".to_string());
        }
    }

    log::info!("[knowledge] 停止知识服务...");
    log::info!(
        "[knowledge] 已记录的 PID: qdrant={:?}, fastkg={:?}, llama={:?}",
        pids.qdrant,
        pids.fastkg,
        pids.llama_server
    );

    if let Some(instance) = registered_instance.as_ref() {
        stop_registered_instance(instance).await?;
        let deleted = kg_coordination::delete_registry_if_matches(
            &instance.instance_id,
            instance.owner_generation,
        )
        .map_err(coordination_err)?;
        if !deleted {
            return Err(AppError::Business(
                "KG 服务已停止，但注册表所有权发生变化，已拒绝删除注册表".into(),
            ));
        }
    } else {
        let supervisor = dependency_service_supervisor();
        for (service_id, display_name) in [
            ("fastkg", "FastKG"),
            ("llama_server", "llama-server"),
            ("qdrant", "Qdrant"),
        ] {
            if supervisor.owned.stop_child(service_id)? {
                supervisor.clear(service_id).await;
                continue;
            }
            if let Some(identity) = verified_tracked_dependency_process(service_id).await {
                supervisor.mark_stopping(service_id).await;
                if !kill_pid_sync(identity.pid) {
                    return Err(AppError::Business(format!(
                        "无法停止受管理的 {} 进程 PID={}，已中止 KG 清理",
                        display_name, identity.pid
                    )));
                }
            }
            supervisor.clear(service_id).await;
        }
        let _ = kg_coordination::delete_owned_registry();
    }
    clear_local_kg_tracking(kg_pids).await;

    // 短暂等待确保进程完全退出
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    log::info!("[knowledge] 知识服务已停止");
    Ok("知识服务已停止".to_string())
}

// ─── FastPlan 服务内部实现函数 ───

/// 启动 FastPlan 服务（内部实现，参照 KG 模式）
pub async fn start_fastplan_inner(
    app_handle: &tauri::AppHandle,
    _kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
) -> Result<String, AppError> {
    app_handle.state::<AppState>().dependency_recovery.request(app_handle, "fastplan", false, true, true)
}

async fn start_fastplan_once(
    app_handle: &tauri::AppHandle,
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
) -> Result<String, AppError> {
    // ─── 启动守卫 ───
    let Some(_starting_guard) = FastPlanStartingGuard::acquire() else {
        tracing::info!("[fastplan] 服务正在启动中（由另一个流程执行），跳过");
        return Ok("FastPlan 正在启动中".to_string());
    };
    FASTPLAN_STOP_REQUESTED.store(false, std::sync::atomic::Ordering::SeqCst);

    let fp_path = paths::fastplan_server_path();
    if let Some(identity) = verified_owned_dependency_process("fastplan").await {
        let has_connection = kg_pids.read().await.fastplan_connection.is_some();
        if has_connection {
            kg_pids.write().await.fastplan = Some(identity.pid);
            set_dependency_runtime_port("fastplan", Some(identity.effective_port));
            return Ok(format!("FastPlan 已在运行 (PID={})", identity.pid));
        }
        tracing::info!(
            "[fastplan] 已有受管进程 PID={} 但缺少连接令牌，将重启以恢复鉴权",
            identity.pid
        );
    }

    // 在分配新端口之前清理失效的追踪状态/残留进程；停止逻辑会清空运行端口。
    let _ = stop_fastplan_inner(kg_pids).await;
    FASTPLAN_STOP_REQUESTED.store(false, std::sync::atomic::Ordering::SeqCst);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let preferred_port = dependency_preferred_port("fastplan");
    let (fp_port, fp_attempt, mut reservation) =
        match reserve_available_dependency_port(app_handle, "fastplan", preferred_port).await {
            Ok(value) => value,
            Err(error) => return Err(error),
        };

    tracing::info!("[fastplan] 启动 FastPlan 服务...");

    // 快速检查是否已在运行
    if is_port_listening(fp_port).await {
        // 检查是否是我们的进程
        if let Some(pid) = get_pid_listening_on_port(fp_port) {
            let pids_guard = kg_pids.read().await;
            let is_ours =
                pids_guard.fastplan == Some(pid) && pids_guard.fastplan_connection.is_some();
            drop(pids_guard);
            if is_ours {
                tracing::info!("[fastplan] FastPlan 已在运行 (PID={})", pid);
                let _ = app_handle.emit(
                    "kg-service-status",
                    serde_json::json!({"phase": "all_ready"}),
                );
                return Ok(format!("FastPlan 已在运行 (PID={})", pid));
            } else {
                let msg = format!(
                    "端口 {} 已被外部进程占用 (PID {})，无法启动 FastPlan",
                    fp_port, pid
                );
                tracing::warn!("[fastplan] {}", msg);
                kg_pids.write().await.fastplan_connection = None;
                let _ = app_handle.emit(
                    "kg-service-status",
                    serde_json::json!({
                        "phase": "port_conflict",
                        "service": "fastplan",
                        "port": fp_port,
                        "pid": pid,
                        "message": msg,
                    }),
                );
                return Err(AppError::Business(msg));
            }
        }
    }

    kg_pids.write().await.fastplan_connection = None;
    // 检查可执行文件是否存在
    if !fp_path.exists() {
        let msg = format!("FastPlan 可执行文件不存在: {:?}", fp_path);
        tracing::error!("[fastplan] {}", msg);
        return Err(AppError::Business(msg));
    }

    // 每次启动前，用 FastClaw 程序所在目录的配置覆盖工作目录中的配置。
    let (fp_data_dir, fp_config) = async {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| AppError::Business(format!("无法获取应用数据目录: {}", e)))?;
        let fp_data_dir = app_data_dir.join("fastplan");
        let fp_config = fp_data_dir.join("fastplan.toml");
        let app_exe = std::env::current_exe()
            .map_err(|e| AppError::Business(format!("无法获取程序路径: {}", e)))?;
        let app_dir = app_exe
            .parent()
            .ok_or_else(|| AppError::Business("无法获取程序所在目录".to_string()))?;
        let source_config = app_dir.join("fastplan.toml");

        tokio::fs::create_dir_all(&fp_data_dir)
            .await
            .map_err(|e| AppError::Business(format!("无法创建 FastPlan 数据目录: {}", e)))?;
        match tokio::fs::copy(&source_config, &fp_config).await {
            Ok(_) => log::info!(
                "[fastplan] 已更新配置: {} -> {}",
                source_config.display(),
                fp_config.display()
            ),
            Err(e) => log::warn!(
                "[fastplan] 无法同步配置 {} -> {}: {}，继续启动",
                source_config.display(),
                fp_config.display(),
                e
            ),
        }
        Ok::<_, AppError>((fp_data_dir, fp_config))
    }
    .await
    .map_err(|e| {
        log::error!("[fastplan] 启动前配置同步失败: {}", e);
        e
    })?;

    // 启动 FastPlan 进程
    tracing::info!("[fastplan] 启动进程: {:?}", fp_path);

    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    #[cfg(windows)]
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let mut cmd = std::process::Command::new(&fp_path);
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    cmd.envs(crate::core::dependency_services::environment::snapshot(app_handle).await?);
    let mut random = [0u8; 32];
    getrandom::getrandom(&mut random)
        .map_err(|_| AppError::Business("无法生成 FastPlan 连接令牌".into()))?;
    let parent_token = hex::encode(random);
    cmd.arg("--parent-managed")
        .env("FASTPLAN_PARENT_TOKEN", &parent_token)
        .env("FASTPLAN_PORT", fp_port.to_string());
    emit_dependency_port_state(
        app_handle,
        "fastplan",
        "starting",
        preferred_port,
        Some(fp_port),
        fp_attempt,
        format!("正在端口 {} 启动 FastPlan", fp_port),
    );
    dependency_service_supervisor()
        .mark_starting("fastplan", preferred_port, fp_port)
        .await;
    // Auto-start can run before the WebView has navigated to devUrl.
    let dev_origin = app_handle
        .config()
        .build
        .dev_url
        .as_ref()
        .map(|url| url.origin().ascii_serialization());
    let window_origin = app_handle
        .get_webview_window("main")
        .and_then(|window| window.url().ok())
        .map(|url| url.origin().ascii_serialization());
    let parent_origins = super::fastplan_origins::parent_origins(
        cfg!(dev),
        dev_origin.as_deref(),
        window_origin.as_deref(),
    );
    log::info!("[fastplan] Origin 白名单: {}", parent_origins);
    cmd.env("FASTPLAN_PARENT_ORIGINS", parent_origins);
    cmd.env("FASTPLAN_CONFIG", fp_config.to_string_lossy().to_string())
        .current_dir(&fp_data_dir);

    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    match cmd.spawn() {
        Ok(mut child) => {
            let pid = child.id();
            match child.try_wait() {
                Ok(Some(status)) => {
                    let msg = format!("FastPlan 启动后立即退出，退出码: {:?}", status.code());
                    tracing::error!("[fastplan] {}", msg);
                    set_dependency_runtime_port("fastplan", None);
                    dependency_service_supervisor()
                        .mark_failed("fastplan", msg.clone())
                        .await;
                    return Err(AppError::Wsp(msg));
                }
                Ok(None) => {
                    tracing::info!("[fastplan] 进程已启动 (pid: {})", pid);
                }
                Err(e) => {
                    tracing::warn!("[fastplan] 状态检查失败: {}", e);
                }
            }

            if FASTPLAN_STOP_REQUESTED.load(std::sync::atomic::Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                clear_fastplan_runtime_tracking(kg_pids).await;
                dependency_service_supervisor().clear("fastplan").await;
                return Ok("FastPlan 启动已取消".into());
            }

            if let Err(error) = record_starting_dependency_process(
                "fastplan",
                preferred_port,
                fp_port,
                pid,
                &fp_path,
            )
            .await
            {
                let _ = child.kill();
                let _ = child.wait();
                clear_fastplan_runtime_tracking(kg_pids).await;
                dependency_service_supervisor()
                    .mark_failed("fastplan", error.to_string())
                    .await;
                return Err(error);
            }

            // 存储 PID
            retain_dependency_child("fastplan", child)?;
            kg_pids.write().await.fastplan = Some(pid);
            // 同步更新全局 FASTPLAN_PID（向后兼容）
            crate::state::FASTPLAN_PID.store(pid, std::sync::atomic::Ordering::SeqCst);

            // 轮询 FastPlan 健康检查（最多 30 次，每次请求超时 1s）
            // 响应格式: {"status":"ok","version":"...","components":{...}}
            let fp_health_url = format!("http://127.0.0.1:{}/api/health", fp_port);
            let health_client = reqwest::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(1))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap_or_default();
            let fp_ok = {
                let mut ok = false;
                for i in 0..60 {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    if FASTPLAN_STOP_REQUESTED.load(std::sync::atomic::Ordering::SeqCst)
                        || kg_pids.read().await.fastplan != Some(pid)
                        || !dependency_service_supervisor().owned.running("fastplan")
                    {
                        break;
                    }
                    match health_client.get(&fp_health_url).send().await {
                        Ok(resp) if resp.status().is_success() => {
                            // 解析响应体，验证 status 字段为 "ok"
                            match resp.json::<serde_json::Value>().await {
                                Ok(body)
                                    if body.get("status").and_then(|v| v.as_str())
                                        == Some("ok") =>
                                {
                                    let version = body
                                        .get("version")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown");
                                    tracing::info!(
                                        "[fastplan] FastPlan /api/health 就绪 (version={}, 第{}次检查)",
                                        version,
                                        i + 1
                                    );
                                    ok = true;
                                    break;
                                }
                                Ok(body) => {
                                    let st = body
                                        .get("status")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("missing");
                                    tracing::debug!(
                                        "[fastplan] FastPlan /api/health status={} (第{}次检查)",
                                        st,
                                        i + 1
                                    );
                                }
                                Err(e) => {
                                    tracing::debug!(
                                        "[fastplan] FastPlan /api/health JSON 解析失败 (第{}次): {}",
                                        i + 1,
                                        e
                                    );
                                }
                            }
                        }
                        Ok(resp) => {
                            tracing::debug!(
                                "[fastplan] FastPlan /api/health 返回 {} (第{}次检查)",
                                resp.status(),
                                i + 1
                            );
                        }
                        Err(e) => {
                            tracing::debug!(
                                "[fastplan] FastPlan /api/health 检查失败 (第{}次): {}",
                                i + 1,
                                e
                            );
                        }
                    }
                }
                ok
            };

            let published = if fp_ok && get_pid_listening_on_port(fp_port) == Some(pid) {
                let mut guard = kg_pids.write().await;
                if guard.fastplan == Some(pid) {
                    guard.fastplan_connection = Some(
                        crate::commands::fastplan_connection::FastPlanConnectionInfo {
                            ws_base: format!("ws://127.0.0.1:{}", fp_port),
                            api_base: format!("http://127.0.0.1:{}", fp_port),
                            parent_token,
                        },
                    );
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if published {
                if let Err(error) = record_owned_dependency_process(
                    "fastplan",
                    preferred_port,
                    fp_port,
                    pid,
                    &fp_path,
                )
                .await
                {
                    fail_fastplan_start(kg_pids, error.to_string()).await;
                    return Err(error);
                }
                tracing::info!("[fastplan] FastPlan 服务就绪");
                emit_dependency_port_state(
                    app_handle,
                    "fastplan",
                    "ready",
                    preferred_port,
                    Some(fp_port),
                    fp_attempt,
                    format!("FastPlan 已在端口 {} 就绪", fp_port),
                );
                reservation.commit();
                let _ = app_handle.emit(
                    "kg-service-status",
                    serde_json::json!({"phase": "all_ready"}),
                );
            } else {
                if FASTPLAN_STOP_REQUESTED.load(std::sync::atomic::Ordering::SeqCst) {
                    clear_fastplan_runtime_tracking(kg_pids).await;
                    dependency_service_supervisor().clear("fastplan").await;
                    tracing::info!("[fastplan] 启动过程已由用户取消");
                    return Ok("FastPlan 启动已取消".into());
                }
                let message = "FastPlan 启动失败：健康检查或端口归属检查未通过";
                fail_fastplan_start(kg_pids, message).await;
                let _ = app_handle.emit(
                    "kg-service-status",
                    serde_json::json!({
                        "phase": "error",
                        "message": "FastPlan 服务启动失败，请检查服务文件是否已安装"
                    }),
                );
                return Err(AppError::Business(message.into()));
            }

            Ok(format!("FastPlan 已启动 (PID={})", pid))
        }
        Err(e) => {
            tracing::error!("[fastplan] 启动失败: {}", e);
            set_dependency_runtime_port("fastplan", None);
            dependency_service_supervisor()
                .mark_failed("fastplan", format!("FastPlan 进程创建失败: {}", e))
                .await;
            Err(AppError::Io(e))
        }
    }
}

/// 停止 FastPlan 服务（内部实现，参照 KG 模式）
pub async fn stop_fastplan_inner(
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
) -> Result<String, AppError> {
    let supervisor = dependency_service_supervisor();
    FASTPLAN_STOP_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);
    let runtime = supervisor.snapshot("fastplan").await;
    let identity = match runtime.as_ref().map(|runtime| runtime.lifecycle) {
        Some(
            LifecycleState::Starting
            | LifecycleState::Stopping
            | LifecycleState::Unhealthy
            | LifecycleState::Running,
        ) => verified_tracked_dependency_process("fastplan").await,
        _ => None,
    };
    let Some(identity) = identity else {
        clear_fastplan_runtime_tracking(kg_pids).await;
        supervisor.clear("fastplan").await;
        if FASTPLAN_STARTING.load(std::sync::atomic::Ordering::SeqCst) {
            return Ok("已请求取消 FastPlan 启动".into());
        }
        return Err(AppError::Business(
            "FastPlan 不是由当前 FastClaw 管理的进程，已拒绝停止；端口占用者不会受到影响".into(),
        ));
    };

    supervisor.mark_stopping("fastplan").await;
    if !kill_pid_sync(identity.pid) {
        supervisor
            .mark_unhealthy(
                "fastplan",
                format!("无法停止受管理的 FastPlan 进程 PID={}", identity.pid),
            )
            .await;
        return Err(AppError::Business(format!(
            "无法停止受管理的 FastPlan 进程 PID={}，运行记录已保留",
            identity.pid
        )));
    }

    clear_fastplan_runtime_tracking(kg_pids).await;
    supervisor.clear("fastplan").await;
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    log::info!("[fastplan] FastPlan 服务已停止 (PID={})", identity.pid);
    Ok(format!("FastPlan 已停止 (PID={})", identity.pid))
}

// ─── Tauri 命令包装器 ───

/// 停止知识服务（Tauri 命令）
#[tauri::command]
pub async fn stop_knowledge_service(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<String, AppError> {
    for id in ["fastkg", "qdrant", "llama_server"] { state.dependency_recovery.request_stop(&app, id)?; }
    let _operation = state.dependency_recovery.operation.lock().await;
    stop_kg_inner(&state.kg_pids).await
}

/// 更新覆盖文件前强制停止本目录 FastKG 相关进程，避免 Windows 文件占用
#[tauri::command]
pub async fn stop_kg_for_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<(), AppError> {
    let service_dir = paths::fastkg_service_dir();
    tracing::info!(
        "[knowledge] 更新前强制停止 FastKG: {}",
        service_dir.display()
    );
    if let Some(registry) = kg_coordination::read_registry().map_err(coordination_err)? {
        if !kg_coordination::current_fastclaw_is_owner(&registry)
            || registry.leases.iter().any(|lease| lease.app_pid != std::process::id())
        { return Err(AppError::Business("共享 KG 仍被其他消费者使用，请退出共享方后更新".into())); }
    }
    for id in ["fastkg", "qdrant", "llama_server"] { state.dependency_recovery.request_stop(&app, id)?; }
    let _operation = state.dependency_recovery.operation.lock().await;
    stop_kg_inner(&state.kg_pids).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(1200)).await;
    let _ = app.emit(
        "kg-service-status",
        serde_json::json!({ "phase": "stopped" }),
    );
    Ok(())
}

// ─── FastKG 下载 API 响应结构体 ───────────────

#[derive(Debug, serde::Deserialize)]
struct ApiResponse<T> {
    #[allow(dead_code)]
    code: i32,
    data: T,
}

#[derive(Debug, serde::Deserialize)]
struct AppInfo {
    versions: Option<Vec<VersionBrief>>,
}

#[derive(Debug, serde::Deserialize)]
struct VersionBrief {
    version: String,
    is_latest: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
struct VersionDetail {
    #[allow(dead_code)]
    version: String,
    artifacts: Option<PlatformArtifacts>,
}

#[derive(Debug, serde::Deserialize)]
struct PlatformArtifacts {
    #[serde(rename = "windows-x86_64")]
    windows: Option<Artifact>,
}

#[derive(Debug, serde::Deserialize)]
struct Artifact {
    package: Option<PackageInfo>,
}

#[derive(Debug, serde::Deserialize)]
struct PackageInfo {
    url: String,
    #[allow(dead_code)]
    size: Option<u64>,
    sha256: Option<String>,
}

// ──────────────────────────────────────────

/// KG 服务文件检测结果
#[derive(Debug, serde::Serialize)]
pub struct KgServiceStatus {
    pub all_exist: bool,
    pub fastkg_exist: bool,
    pub qdrant_exist: bool,
    pub llama_exist: bool,
    pub model_exist: bool,
    pub missing_files: Vec<String>,
}

/// KG 服务文件完整性检测的唯一实现，供页面状态与启动前检查共同复用。
fn inspect_kg_service_files() -> KgServiceStatus {
    let fastkg = paths::fastkg_server_path();
    let qdrant = paths::qdrant_path();
    let llama = paths::llama_server_path();
    let model = paths::bge_m3_model_path();

    let fastkg_ok = fastkg.exists();
    let qdrant_ok = qdrant.exists();
    let llama_ok = llama.exists();
    let model_ok = model.exists();

    let mut missing = Vec::new();
    if !fastkg_ok {
        missing.push("FastKG.exe".to_string());
    }
    if !qdrant_ok {
        missing.push("qdrant.exe".to_string());
    }
    if !llama_ok {
        missing.push("llama-server.exe".to_string());
    }
    if !model_ok {
        missing.push("bge-m3-Q8_0.gguf".to_string());
    }

    tracing::debug!(
        "[knowledge] 文件检测 - fastkg: {}, qdrant: {}, llama: {}, model: {}",
        fastkg_ok,
        qdrant_ok,
        llama_ok,
        model_ok
    );

    KgServiceStatus {
        all_exist: fastkg_ok && qdrant_ok && llama_ok && model_ok,
        fastkg_exist: fastkg_ok,
        qdrant_exist: qdrant_ok,
        llama_exist: llama_ok,
        model_exist: model_ok,
        missing_files: missing,
    }
}

/// 检查 KG 服务所需的关键文件是否存在
#[tauri::command]
pub fn check_kg_service_files() -> Result<KgServiceStatus, AppError> {
    Ok(inspect_kg_service_files())
}

/// 启动知识服务（异步版本，emit 事件到前端）
#[tauri::command]
pub async fn start_knowledge_service_async(app: tauri::AppHandle) -> Result<String, AppError> {
    app.state::<AppState>().dependency_recovery.request(&app, "fastkg", true, false, false)
}
/// KG 服务运行状态检测结果
#[derive(Debug, serde::Serialize)]
pub struct KgServiceRunning {
    pub running: bool,
}

/// KG 服务完整就绪检测结果。
#[derive(Debug, serde::Serialize)]
pub struct KgServiceReady {
    pub ready: bool,
}

/// 检查 KG 服务是否在本次实际端口上监听
#[tauri::command]
pub async fn check_kg_service_running() -> Result<KgServiceRunning, AppError> {
    let running = dependency_service_is_running("fastkg").await;
    tracing::info!("[knowledge] KG服务运行状态: {}", running);
    Ok(KgServiceRunning { running })
}

/// 完整验证 FastKG 及其依赖是否就绪。该命令可能触发较慢的 /health，
/// 与只判断 PID/监听端口的 check_kg_service_running 明确分离。
#[tauri::command]
pub async fn check_kg_service_ready() -> Result<KgServiceReady, AppError> {
    let ready = dependency_service_is_usable("fastkg").await;
    tracing::info!("[knowledge] KG服务就绪状态: {}", ready);
    Ok(KgServiceReady { ready })
}

// ─── 互联网状态 & 取消下载 ───────────────

/// 获取持久化的互联网连接状态
#[tauri::command]
pub async fn get_kg_shared_status() -> Result<KgSharedStatus, AppError> {
    kg_coordination::reconcile_stale_leases().map_err(coordination_err)?;
    let registry = kg_coordination::read_registry().map_err(coordination_err)?;
    Ok(kg_coordination::status_from_registry(registry.as_ref()))
}

/// 启动、异常恢复或租约补登记时使用的深度共享运行时校验。
/// 普通 UI 状态同步必须使用 get_kg_shared_status，避免隐式触发 /health。
#[tauri::command]
pub async fn validate_kg_shared_runtime() -> Result<KgSharedStatus, AppError> {
    kg_coordination::reconcile_stale_leases().map_err(coordination_err)?;
    let registry = kg_coordination::read_registry().map_err(coordination_err)?;
    if let Some(registry) = registry
        .as_ref()
        .filter(|registry| registry.state == "ready")
    {
        if shared_kg_registry_is_valid(registry).await {
            return Ok(kg_coordination::status_from_registry(Some(registry)));
        }
        return Ok(KgSharedStatus::none());
    }
    if fastkg_health_ok(&format!(
        "http://127.0.0.1:{}",
        dependency_runtime_port("fastkg").unwrap_or_else(|| dependency_preferred_port("fastkg"))
    ))
    .await
    {
        return Ok(kg_coordination::external_unmanaged_status());
    }
    Ok(KgSharedStatus::none())
}

#[tauri::command]
pub async fn ensure_kg_shared_lease(
    state: tauri::State<'_, crate::state::AppState>,
    app_handle: tauri::AppHandle,
) -> Result<KgSharedStatus, AppError> {
    state.dependency_recovery.request(&app_handle, "fastkg", true, false, false)?;
    validate_kg_shared_runtime().await
}

#[tauri::command]
pub async fn get_internet_status(state: State<'_, AppState>) -> Result<bool, AppError> {
    let val = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = 'internet_connected'",
    )
    .fetch_optional(&*state.db.pool().await)
    .await
    .ok()
    .flatten();
    Ok(val.as_deref() == Some("true"))
}

/// 取消正在进行的下载
// ─── 在线下载 FastKG 服务 ───────────────

/// 检查磁盘可用空间（字节）
fn available_disk_space(path: &std::path::Path) -> u64 {
    // 使用 sysinfo 获取磁盘信息

    let disks = sysinfo::Disks::new_with_refreshed_list();
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    for disk in disks.list() {
        if canonical.starts_with(disk.mount_point()) {
            return disk.available_space();
        }
    }
    // 回退：假设有足够空间
    u64::MAX
}

/// 验证 ZIP 条目路径是否在目标目录内（防止路径穿越）
fn is_safe_path(base: &std::path::Path, entry_path: &std::path::Path) -> bool {
    // 规范化后检查是否以 base 开头
    if let Ok(canonical) = base.join(entry_path).canonicalize() {
        return canonical.starts_with(base);
    }
    // 如果文件还不存在（解压前），手动检查路径组件
    // 拒绝包含 ".." 或绝对路径的条目
    if entry_path.is_absolute() {
        return false;
    }
    for component in entry_path.components() {
        if let std::path::Component::ParentDir = component {
            return false;
        }
    }
    true
}

/// 将更新包文件覆盖进现有服务目录，不替换目录本身。
/// 包内没有的文件（storage、logs、snapshots 等）会保留。
/// Windows 上进程退出后文件锁可能延迟释放，覆盖文件时带重试。
fn overlay_copy_dir(src: &std::path::Path, dst: &std::path::Path) -> Result<(), AppError> {
    std::fs::create_dir_all(dst).map_err(AppError::Io)?;
    overlay_copy_contents(src, dst).map_err(|e| {
        AppError::Io(std::io::Error::new(
            e.kind(),
            format!("覆盖安装到 {} 失败: {}", dst.display(), e),
        ))
    })
}

fn overlay_copy_contents(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest = dst.join(entry.file_name());
        if ty.is_dir() {
            std::fs::create_dir_all(&dest)?;
            overlay_copy_contents(&entry.path(), &dest)?;
        } else {
            copy_file_replace(&entry.path(), &dest)?;
        }
    }
    Ok(())
}

fn copy_file_replace(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<u64> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    const ATTEMPTS: u32 = 6;
    let mut last_err: Option<std::io::Error> = None;
    for attempt in 0..ATTEMPTS {
        match std::fs::copy(src, dst) {
            Ok(n) => {
                let mut old = dst.as_os_str().to_os_string();
                old.push(".old");
                let _ = std::fs::remove_file(std::path::PathBuf::from(old));
                return Ok(n);
            }
            Err(e) => {
                last_err = Some(e);
                let mut old = dst.as_os_str().to_os_string();
                old.push(".old");
                let old = std::path::PathBuf::from(old);
                let _ = std::fs::remove_file(&old);
                let _ = std::fs::rename(dst, &old);
                std::thread::sleep(std::time::Duration::from_millis(200 * (attempt as u64 + 1)));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::Other, "overlay copy retries exhausted")
    }))
}

/// 解压 zip 到临时目录（带路径穿越防护）
fn extract_zip_safe(archive_path: &str, extract_to: &std::path::Path) -> Result<(), AppError> {
    let file = std::fs::File::open(archive_path).map_err(|e| AppError::Io(e))?;
    let mut archive = zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(|e| {
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("打开 ZIP 文件失败: {}", e),
        ))
    })?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("读取 ZIP 条目失败: {}", e),
            ))
        })?;
        let entry_path = file.mangled_name();
        let outpath = extract_to.join(&entry_path);

        // 路径穿越防护
        if !is_safe_path(extract_to, &entry_path) {
            return Err(AppError::Business(format!(
                "ZIP 包含不安全的路径: {}",
                entry_path.display()
            )));
        }

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath).map_err(|e| AppError::Io(e))?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    std::fs::create_dir_all(p).map_err(|e| AppError::Io(e))?;
                }
            }
            let mut outfile = std::fs::File::create(&outpath).map_err(|e| AppError::Io(e))?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| AppError::Io(e))?;
        }
    }
    Ok(())
}

struct AppPackageInfo {
    download_url: String,
    sha256: Option<String>,
    version: String,
    size: Option<u64>,
}

/// Resolve FastClaw-specific package metadata into the reusable download plugin contract.
#[tauri::command]
pub async fn prepare_app_download(
    state: State<'_, AppState>,
    api_base_url: Option<String>,
    app_name: Option<String>,
    silent: Option<bool>,
    target_path: Option<String>,
    install: Option<bool>,
) -> Result<tauri_plugin_download_center::model::StartDownloadInput, AppError> {
    let base = match api_base_url {
        Some(url) => url,
        None => state
            .app_config
            .read()
            .await
            .remote_servers
            .download_api_base_url
            .clone(),
    };
    let app_name = app_name.unwrap_or_else(|| "FastKG".to_string());
    let service_dir = if let Some(path) = target_path.as_ref() {
        std::path::PathBuf::from(path)
    } else if app_name == "File2Md" {
        paths::file2md_service_dir()
    } else {
        paths::fastkg_service_dir()
    };
    let disk_check_path = service_dir.parent().unwrap_or(&service_dir);
    let available = available_disk_space(disk_check_path);
    if available < 5 * 1024 * 1024 * 1024 {
        return Err(AppError::Business(
            "磁盘空间不足，至少需要 5 GB 可用空间".into(),
        ));
    }

    // FastKG 下载期间保持运行；文件覆盖前由宿主 InstallHook 校验租约并等待安全停止。
    // 其它应用整目录替换仍需在下载前停掉目标进程。
    if app_name != "FastKG" {
        let _ = kill_process_by_name_in_dir(&app_name, &service_dir).await;
    }
    // KG 下载期间保持可用，覆盖前由宿主 InstallHook 完成租约检查与安全停止。
    let info = fetch_app_package_info(&base, &app_name).await?;
    let temp_path = format!("{}.part", service_dir.to_string_lossy());
    let staging_path = format!("{}.installing", service_dir.to_string_lossy());
    let mut instructions = vec![
        serde_json::json!({"type":"download","url":info.download_url.clone(),"tempPath":temp_path,"resume":true}),
    ];
    if let Some(sha256) = &info.sha256 {
        instructions.push(serde_json::json!({"type":"verifySha256","sha256":sha256}));
    }
    let should_install = install.unwrap_or(true);
    if should_install {
        if app_name == "FastKG" {
            // 只覆盖更新包内文件，保留 FastKGService 目录及 storage/logs 等本地数据
            instructions.extend([
                serde_json::json!({"type":"extractZip","stagingPath":staging_path,"requiredFileSuffix":".exe"}),
                serde_json::json!({
                    "type":"overlayTarget",
                    "source":"staging",
                    "targetPath":service_dir.to_string_lossy()
                }),
                serde_json::json!({"type":"cleanup","targets":["temp","staging"]}),
            ]);
        } else {
            instructions.extend([
                serde_json::json!({"type":"extractZip","stagingPath":staging_path,"requiredFileSuffix":".exe"}),
                serde_json::json!({"type":"backupTarget","targetPath":service_dir.to_string_lossy()}),
                serde_json::json!({"type":"replaceTarget","source":"staging","targetPath":service_dir.to_string_lossy()}),
                serde_json::json!({"type":"cleanup","targets":["temp","backup"]}),
            ]);
        }
    } else {
        instructions.extend([
            serde_json::json!({"type":"backupTarget","targetPath":service_dir.to_string_lossy()}),
            serde_json::json!({"type":"replaceTarget","source":"temp","targetPath":service_dir.to_string_lossy()}),
            serde_json::json!({"type":"cleanup","targets":["backup"]}),
        ]);
    }
    Ok(tauri_plugin_download_center::model::StartDownloadInput {
        resource_type: "fastclaw-app".into(),
        resource_id: app_name.to_ascii_lowercase(),
        name: app_name.clone(),
        payload: serde_json::json!({
            "url": info.download_url.clone(),
            "targetPath": service_dir.to_string_lossy(),
            "sha256": info.sha256.clone(),
            "installMode": if should_install { "zip" } else { "file" },
            "requiredFileSuffix": if should_install { Some(".exe") } else { None },
            "appName": app_name,
            "version": info.version,
            "executionPlan": { "instructions": instructions }
        }),
        silent: silent.unwrap_or(false),
    })
}

#[tauri::command]
pub async fn complete_app_download(
    state: State<'_, AppState>,
    app_name: String,
    version: Option<String>,
) -> Result<(), AppError> {
    // 下载中心的 success 只表示执行计划已完成。FastKG 在写入“已下载”标记前
    // 再按启动流程使用的同一组路径校验一次，避免历史标记与磁盘状态再次脱节。
    if app_name == "FastKG" {
        let files = inspect_kg_service_files();
        if !files.all_exist {
            return Err(AppError::Business(format!(
                "FastKG 安装包文件不完整：{}",
                files.missing_files.join("、")
            )));
        }

        // 文件重新落盘后，清除先前因文件缺失产生的无进程失败记录。
        // 只清 Failed 且没有进程身份的记录，不覆盖任何仍有效的运行实例。
        let supervisor = dependency_service_supervisor();
        for service_id in ["qdrant", "llama_server", "fastkg"] {
            let stale_failure = supervisor
                .snapshot(service_id)
                .await
                .is_some_and(|runtime| {
                    runtime.lifecycle == LifecycleState::Failed && runtime.process.is_none()
                });
            if stale_failure {
                supervisor.clear(service_id).await;
            }
        }
    }

    let pool = state.db.pool().await;
    let setting_key = if app_name == "File2Md" {
        "file2md_service_downloaded"
    } else {
        "kg_service_downloaded"
    };
    sqlx::query("INSERT OR REPLACE INTO settings (key, value, category, updated_at) VALUES (?, 'true', 'general', datetime('now'))")
        .bind(setting_key).execute(&*pool).await?;

    // 仅在安装包校验并覆盖成功后写入 FastKG 版本（由下载中心完成回调触发）
    if app_name == "FastKG" {
        persist_fastkg_version_after_install(&*pool, version.as_deref()).await;
    }
    Ok(())
}

#[derive(Debug, serde::Serialize)]
pub struct FastKgVersionCheck {
    pub local_version: String,
    pub remote_version: String,
    pub has_update: bool,
    pub remote_available: bool,
}

/// 对比本地 FastKG 版本与远端最新版本（语义化版本）
#[tauri::command]
pub async fn check_fastkg_version(
    state: State<'_, AppState>,
    api_base_url: Option<String>,
) -> Result<FastKgVersionCheck, AppError> {
    let pool = state.db.pool().await;
    let mut local_version = read_setting_value(&*pool, "fastkg_version")
        .await
        .unwrap_or_default();

    // 已安装但尚未写入 settings 时，尝试从本地安装包回填（不改变下载失败不写版本的约束）
    if local_version.trim().is_empty() {
        if let Some(from_pkg) = read_fastkg_package_version(&paths::fastkg_service_dir()) {
            persist_fastkg_version(&*pool, &from_pkg).await;
            local_version = from_pkg;
        }
    }

    let base = match api_base_url {
        Some(url) => url,
        None => state
            .app_config
            .read()
            .await
            .remote_servers
            .download_api_base_url
            .clone(),
    };
    match fetch_app_package_info(&base, "FastKG").await {
        Ok(info) => {
            let remote_version = info.version;
            let has_update =
                compare_semver(&remote_version, &local_version) == std::cmp::Ordering::Greater;
            Ok(FastKgVersionCheck {
                local_version,
                remote_version,
                has_update,
                remote_available: true,
            })
        }
        Err(e) => {
            tracing::info!("[knowledge] 无法获取远端 FastKG 版本（可能离线）: {}", e);
            Ok(FastKgVersionCheck {
                local_version,
                remote_version: String::new(),
                has_update: false,
                remote_available: false,
            })
        }
    }
}

const FASTKG_VERSION_SETTING: &str = "fastkg_version";

async fn read_setting_value(pool: &sqlx::SqlitePool, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

/// 从已安装的 FastKG 目录读取包内版本号（VERSION / version.json，JSON 或纯文本）
fn read_fastkg_package_version(service_dir: &std::path::Path) -> Option<String> {
    const CANDIDATES: &[&str] = &["VERSION", "version.json", "VERSION.json", "version.txt"];
    for name in CANDIDATES {
        let path = service_dir.join(name);
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let trimmed = content.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
            for key in ["app_version", "version"] {
                if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
                    let s = s.trim();
                    if !s.is_empty() {
                        return Some(s.to_string());
                    }
                }
            }
        } else if let Some(line) = trimmed.lines().next() {
            let line = line.trim().trim_start_matches(['v', 'V']);
            if !line.is_empty() {
                return Some(line.to_string());
            }
        }
    }
    None
}

async fn persist_fastkg_version(pool: &sqlx::SqlitePool, version: &str) {
    let version = version.trim();
    if version.is_empty() {
        return;
    }
    if let Err(e) = sqlx::query(
        "INSERT OR REPLACE INTO settings (key, value, category, description, updated_at) VALUES (?, ?, 'general', '当前本地已安装的 FastKG 版本号', datetime('now'))",
    )
    .bind(FASTKG_VERSION_SETTING)
    .bind(version)
    .execute(pool)
    .await
    {
        tracing::warn!("[knowledge] 写入 fastkg_version 失败: {}", e);
    } else {
        tracing::info!("[knowledge] 已记录 FastKG 版本: {}", version);
    }
}

/// 在线安装/更新成功后写入版本：优先包内 VERSION，若下载元数据更新则用之
async fn persist_fastkg_version_after_install(pool: &sqlx::SqlitePool, fallback: Option<&str>) {
    let from_pkg = read_fastkg_package_version(&paths::fastkg_service_dir());
    let from_meta = fallback
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let version = match (from_pkg, from_meta) {
        (Some(pkg), Some(meta))
            if compare_semver(&meta, &pkg) == std::cmp::Ordering::Greater =>
        {
            Some(meta)
        }
        (Some(pkg), _) => Some(pkg),
        (None, Some(meta)) => Some(meta),
        (None, None) => None,
    };
    match version {
        Some(v) => persist_fastkg_version(pool, &v).await,
        None => tracing::warn!("[knowledge] 安装成功但未读到 FastKG 包内版本号，不更新 fastkg_version"),
    }
}

/// 离线上传版本：首次上传固定 0.0.1；再次上传视为最新（包内版本更新则用之，否则在本地版本上 +1）
fn resolve_uploaded_fastkg_version(previous_local: &str, package_version: Option<&str>) -> String {
    let prev = previous_local.trim();
    if prev.is_empty() {
        return "0.0.1".to_string();
    }
    if let Some(pkg) = package_version.map(str::trim).filter(|s| !s.is_empty()) {
        if compare_semver(pkg, prev) == std::cmp::Ordering::Greater {
            return pkg.to_string();
        }
    }
    bump_semver_patch(prev)
}

fn bump_semver_patch(version: &str) -> String {
    let mut parts: Vec<u64> = version
        .trim()
        .trim_start_matches(['v', 'V'])
        .split(|c: char| c == '.' || c == '-' || c == '+')
        .filter_map(|part| {
            let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                None
            } else {
                digits.parse().ok()
            }
        })
        .collect();
    if parts.is_empty() {
        return "0.0.1".to_string();
    }
    while parts.len() < 3 {
        parts.push(0);
    }
    if let Some(last) = parts.last_mut() {
        *last += 1;
    }
    parts
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

async fn persist_uploaded_fastkg_version(pool: &sqlx::SqlitePool, service_dir: &std::path::Path) {
    let previous = read_setting_value(pool, FASTKG_VERSION_SETTING)
        .await
        .unwrap_or_default();
    let package = read_fastkg_package_version(service_dir);
    let version = resolve_uploaded_fastkg_version(&previous, package.as_deref());
    persist_fastkg_version(pool, &version).await;
    if let Err(e) = std::fs::write(service_dir.join("VERSION"), format!("{version}\n")) {
        tracing::warn!("[knowledge] 写入 FastKG VERSION 文件失败: {}", e);
    }
}

/// 语义化版本比较（x.y.z，忽略 v 前缀与预发布后缀）
fn compare_semver(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> Vec<u64> {
        v.trim()
            .trim_start_matches(['v', 'V'])
            .split(|c: char| c == '.' || c == '-' || c == '+')
            .filter_map(|part| {
                let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
                if digits.is_empty() {
                    None
                } else {
                    digits.parse().ok()
                }
            })
            .collect()
    };
    let a_parts = parse(a);
    let b_parts = parse(b);
    let max_len = a_parts.len().max(b_parts.len());
    for i in 0..max_len {
        let a_num = a_parts.get(i).copied().unwrap_or(0);
        let b_num = b_parts.get(i).copied().unwrap_or(0);
        match a_num.cmp(&b_num) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

#[cfg(test)]
mod fastkg_version_tests {
    use super::compare_semver;

    #[test]
    fn remote_newer_than_local() {
        assert_eq!(compare_semver("1.2.0", "1.1.9"), std::cmp::Ordering::Greater);
        assert_eq!(compare_semver("2.0.0", "1.9.9"), std::cmp::Ordering::Greater);
        assert_eq!(compare_semver("1.0.0", ""), std::cmp::Ordering::Greater);
        assert_eq!(compare_semver("1.0.1", "1.0.0"), std::cmp::Ordering::Greater);
    }

    #[test]
    fn equal_or_older_hides_update() {
        assert_eq!(compare_semver("1.2.0", "1.2.0"), std::cmp::Ordering::Equal);
        assert_eq!(compare_semver("v1.2.0", "1.2.0"), std::cmp::Ordering::Equal);
        assert_eq!(compare_semver("1.1.0", "1.2.0"), std::cmp::Ordering::Less);
        assert_eq!(compare_semver("1.2.0-beta", "1.2.0"), std::cmp::Ordering::Equal);
    }

    #[test]
    fn first_upload_is_0_0_1() {
        assert_eq!(
            super::resolve_uploaded_fastkg_version("", None),
            "0.0.1"
        );
        assert_eq!(
            super::resolve_uploaded_fastkg_version("", Some("1.4.0")),
            "0.0.1"
        );
    }

    #[test]
    fn subsequent_upload_becomes_latest() {
        assert_eq!(
            super::resolve_uploaded_fastkg_version("0.0.1", Some("1.2.0")),
            "1.2.0"
        );
        assert_eq!(
            super::resolve_uploaded_fastkg_version("0.0.1", None),
            "0.0.2"
        );
        assert_eq!(
            super::resolve_uploaded_fastkg_version("1.2.0", Some("1.2.0")),
            "1.2.1"
        );
        assert_eq!(
            super::resolve_uploaded_fastkg_version("1.2.0", Some("1.1.9")),
            "1.2.1"
        );
    }
}

#[cfg(test)]
mod fastkg_overlay_tests {
    use super::overlay_copy_dir;

    #[test]
    fn overlay_replaces_package_files_and_keeps_local_data() {
        let root = std::env::temp_dir().join(format!(
            "fastkg-overlay-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let pkg = root.join("pkg");
        let dest = root.join("FastKGService");
        std::fs::create_dir_all(pkg.join("ebinding_start")).unwrap();
        std::fs::write(pkg.join("FastKG.exe"), b"new-exe").unwrap();
        std::fs::write(pkg.join("config.toml"), b"new-cfg").unwrap();
        std::fs::write(pkg.join("ebinding_start/run.bat"), b"new-bat").unwrap();

        std::fs::create_dir_all(dest.join("storage/db")).unwrap();
        std::fs::create_dir_all(dest.join("logs")).unwrap();
        std::fs::write(dest.join("FastKG.exe"), b"old-exe").unwrap();
        std::fs::write(dest.join("config.toml"), b"old-cfg").unwrap();
        std::fs::write(dest.join("storage/db/knowledge.db"), b"user-db").unwrap();
        std::fs::write(dest.join("logs/fastkg.log"), b"keep-log").unwrap();

        overlay_copy_dir(&pkg, &dest).unwrap();

        assert_eq!(std::fs::read(dest.join("FastKG.exe")).unwrap(), b"new-exe");
        assert_eq!(std::fs::read(dest.join("config.toml")).unwrap(), b"new-cfg");
        assert_eq!(
            std::fs::read(dest.join("ebinding_start/run.bat")).unwrap(),
            b"new-bat"
        );
        assert_eq!(
            std::fs::read(dest.join("storage/db/knowledge.db")).unwrap(),
            b"user-db"
        );
        assert_eq!(
            std::fs::read(dest.join("logs/fastkg.log")).unwrap(),
            b"keep-log"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}

async fn fetch_app_package_info(base: &str, app_name: &str) -> Result<AppPackageInfo, AppError> {
    let client = reqwest::Client::new();

    let app_info_url = format!("{}/api/v1/apps/{}", base, app_name);
    let app_resp: ApiResponse<AppInfo> = client
        .get(&app_info_url)
        .send()
        .await
        .map_err(|e| AppError::Network(format!("获取应用信息失败: {}", e)))?
        .json()
        .await
        .map_err(|e| AppError::Parse(format!("解析应用信息失败: {}", e)))?;

    let versions = app_resp.data.versions.unwrap_or_default();
    if versions.is_empty() {
        return Err(AppError::Business("未找到任何可用版本".to_string()));
    }

    let target_version = versions
        .iter()
        .find(|v| v.is_latest == Some(true))
        .or_else(|| versions.last())
        .ok_or_else(|| AppError::Business("无法确定最新版本".to_string()))?;

    let version_url = format!(
        "{}/api/v1/apps/{}/versions/{}",
        base, app_name, target_version.version
    );
    let version_resp: ApiResponse<VersionDetail> = client
        .get(&version_url)
        .send()
        .await
        .map_err(|e| AppError::Network(format!("获取版本详情失败: {}", e)))?
        .json()
        .await
        .map_err(|e| AppError::Parse(format!("解析版本详情失败: {}", e)))?;

    let pkg_info = version_resp
        .data
        .artifacts
        .as_ref()
        .and_then(|a| a.windows.as_ref())
        .and_then(|a| a.package.as_ref())
        .ok_or_else(|| AppError::Business("未找到 windows-x86_64 平台的下载包".to_string()))?;

    Ok(AppPackageInfo {
        download_url: pkg_info.url.clone(),
        sha256: pkg_info.sha256.clone(),
        version: target_version.version.clone(),
        size: pkg_info.size,
    })
}

// ─── 离线上传 FastKG 服务 ───────────────

#[tauri::command]
pub async fn upload_fastkg_service(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    file_path: String,
) -> Result<String, AppError> {
    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err(AppError::NotFound(format!("文件不存在: {}", file_path)));
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "zip" {
        return Err(AppError::Validation("请上传 .zip 格式的压缩包".to_string()));
    }

    let service_dir = paths::fastkg_service_dir();
    let tmp_service_dir = service_dir.with_extension("tmp");

    // 磁盘空间检查
    let available = available_disk_space(&service_dir);
    let min_space: u64 = 5 * 1024 * 1024 * 1024;
    if available < min_space {
        return Err(AppError::Business(format!(
            "磁盘空间不足，可用 {:.1} GB，需至少 5 GB",
            available as f64 / 1_073_741_824.0
        )));
    }

    let _ = app.emit(
        "kg-upload-progress",
        serde_json::json!({
            "phase": "extracting",
            "progress": 10,
            "message": "正在解压服务包..."
        }),
    );

    // 清理旧临时目录
    if tmp_service_dir.exists() {
        let _ = std::fs::remove_dir_all(&tmp_service_dir);
    }
    std::fs::create_dir_all(&tmp_service_dir).map_err(|e| {
        AppError::Io(std::io::Error::new(
            e.kind(),
            format!("创建临时目录失败: {}", e),
        ))
    })?;

    let file_path_clone = file_path.clone();
    let tmp_dir_clone = tmp_service_dir.clone();
    tokio::task::spawn_blocking(move || extract_zip_safe(&file_path_clone, &tmp_dir_clone))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("解压线程异常: {}", e)))??;

    let _ = app.emit(
        "kg-upload-progress",
        serde_json::json!({
            "phase": "verifying",
            "progress": 80,
            "message": "正在验证服务文件..."
        }),
    );

    let tmp_has_exe = find_exe_in_dir_sync(&tmp_service_dir.to_string_lossy())
        .unwrap_or(None)
        .is_some();

    if !tmp_has_exe {
        let _ = std::fs::remove_dir_all(&tmp_service_dir);
        return Err(AppError::Business(
            "解压后未找到可执行文件，请确认 ZIP 包内容正确".to_string(),
        ));
    }

    // 离线安装与在线覆盖共用同一可等待的安全维护边界。
    use tauri_plugin_download_center::manager::InstallGuard;
    let mut install_guard = crate::core::dependency_services::install::prepare(&app).await
        .map_err(AppError::Business)?;

    // 只覆盖更新包内文件，保留现有 FastKGService 目录及本地数据
    let install_result = overlay_copy_dir(&tmp_service_dir, &service_dir);
    install_guard.finish(install_result.is_ok()).await.map_err(AppError::Business)?;
    install_result?;
    drop(install_guard);
    let _ = std::fs::remove_dir_all(&tmp_service_dir);

    // 持久化（仅校验/解压成功后才写版本）
    let pool = state.db.pool().await;
    let _ = sqlx::query(
        "INSERT OR REPLACE INTO settings (key, value, category, updated_at) VALUES ('kg_service_downloaded', 'true', 'general', datetime('now'))"
    )
    .execute(&*pool)
    .await;
    persist_uploaded_fastkg_version(&*pool, &service_dir).await;

    let _ = app.emit(
        "kg-upload-progress",
        serde_json::json!({
            "phase": "complete",
            "progress": 100,
            "message": "服务包已就绪，正在启动..."
        }),
    );

    tracing::info!("[knowledge] FastKG 服务上传安装完成");
    Ok("安装完成".to_string())
}

// ──────────────────────────────────────────────
// 依赖服务管理（WSP 管理页面 → 依赖服务 Tab）
// 这些命令仅由前端手动操作触发，不介入 main.rs 的自启动流程
// ──────────────────────────────────────────────

/// 依赖服务状态（返回给前端）
#[derive(Debug, serde::Serialize)]
pub struct DepServiceStatus {
    model_sync_status: Option<&'static str>,
    pub(crate) id: String,
    name: String,
    port: u16,
    preferred_port: u16,
    using_fallback: bool,
    host: String,
    pub(crate) running: bool,
    pid: Option<u32>,
    lifecycle_state: String,
    effective_port: Option<u16>,
    port_occupied: bool,
    occupied_by_pid: Option<u32>,
    occupied_by_name: Option<String>,
    can_start: bool,
    can_stop: bool,
    pub(crate) status_message: String,
    description: String,
    pub(crate) installed: bool,
    configurable: bool,
    ownership: kg_coordination::SharedOwnership,
    starter_app: Option<String>,
    consumers: Vec<String>,
    lease_count: usize,
    lease_registered: bool,
    shared_instance: bool,
    pending_restart: bool,
}

struct DependencyAssessment {
    lifecycle: LifecycleState,
    ownership: kg_coordination::SharedOwnership,
    display_port: u16,
    effective_port: Option<u16>,
    running: bool,
    managed_pid: Option<u32>,
    port_occupied: bool,
    occupied_by_pid: Option<u32>,
    occupied_by_name: Option<String>,
    can_start: bool,
    can_stop: bool,
    status_message: String,
}

fn process_display_name(pid: u32, processes: &HashMap<u32, ProcessFacts>) -> Option<String> {
    processes
        .get(&pid)
        .and_then(|facts| facts.canonical_exe_path.file_name())
        .map(|name| name.to_string_lossy().to_string())
}

fn shared_process_is_valid(
    process: &SharedServiceProcess,
    service_dir: &std::path::Path,
    expected_exe_name: &str,
    listeners: &HashMap<u16, u32>,
    processes: &HashMap<u32, ProcessFacts>,
) -> bool {
    if process.pid == 0 {
        return false;
    }
    let port = parse_url_port(&process.url, 0);
    if port == 0 || listeners.get(&port).copied() != Some(process.pid) {
        return false;
    }
    let Some(facts) = processes.get(&process.pid) else {
        return false;
    };
    let registered_exe = std::path::Path::new(&process.exe);
    if !same_executable(&facts.canonical_exe_path, registered_exe)
        || !normalized_path_starts_with(&facts.canonical_exe_path, service_dir)
        || facts
            .canonical_exe_path
            .file_name()
            .map(|name| {
                !name
                    .to_string_lossy()
                    .eq_ignore_ascii_case(expected_exe_name)
            })
            .unwrap_or(true)
    {
        return false;
    }
    process
        .process_started_at
        .map(|started_at| {
            started_at.timestamp() >= 0 && started_at.timestamp() as u64 == facts.process_started_at
        })
        .unwrap_or(true)
}

fn assess_dependency_service(
    service_name: &str,
    preferred_port: u16,
    configured_effective_port: u16,
    expected_exe: &std::path::Path,
    runtime: Option<&ServiceRuntime>,
    self_healthy: bool,
    shared: Option<(&SharedServiceProcess, &SharedKgInstance)>,
    shared_healthy: bool,
    listeners: &HashMap<u16, u32>,
    processes: &HashMap<u32, ProcessFacts>,
) -> DependencyAssessment {
    if runtime_identity_is_valid(runtime, expected_exe, listeners, processes) {
        let identity = runtime.and_then(|value| value.process.as_ref()).unwrap();
        if !self_healthy {
            return DependencyAssessment {
                lifecycle: LifecycleState::Unhealthy,
                ownership: kg_coordination::SharedOwnership::SelfManaged,
                display_port: identity.effective_port,
                effective_port: Some(identity.effective_port),
                running: false,
                managed_pid: Some(identity.pid),
                port_occupied: false,
                occupied_by_pid: None,
                occupied_by_name: None,
                // 允许「停止」后重拉；若前端只露出停止按钮也足够恢复
                can_start: false,
                can_stop: true,
                status_message: format!("{} 的进程身份有效，但服务健康检查未通过", service_name),
            };
        }
        return DependencyAssessment {
            lifecycle: LifecycleState::Running,
            ownership: kg_coordination::SharedOwnership::SelfManaged,
            display_port: identity.effective_port,
            effective_port: Some(identity.effective_port),
            running: true,
            managed_pid: Some(identity.pid),
            port_occupied: false,
            occupied_by_pid: None,
            occupied_by_name: None,
            can_start: false,
            can_stop: true,
            status_message: format!("{} 已由 FastClaw 管理并通过进程身份校验", service_name),
        };
    }

    if let Some(runtime) = runtime.filter(|runtime| {
        matches!(
            runtime.lifecycle,
            LifecycleState::Running | LifecycleState::Unhealthy | LifecycleState::Failed
        ) && runtime_process_identity_is_valid(Some(runtime), expected_exe, processes)
            && runtime
                .process
                .as_ref()
                .is_some_and(|identity| !listeners.contains_key(&identity.effective_port))
    }) {
        let identity = runtime.process.as_ref().unwrap();
        return DependencyAssessment {
            lifecycle: LifecycleState::Unhealthy,
            ownership: kg_coordination::SharedOwnership::SelfManaged,
            display_port: identity.effective_port,
            effective_port: Some(identity.effective_port),
            running: false,
            managed_pid: Some(identity.pid),
            port_occupied: false,
            occupied_by_pid: None,
            occupied_by_name: None,
            can_start: false,
            can_stop: true,
            status_message: runtime.last_error.clone().unwrap_or_else(|| {
                format!(
                    "{} 的进程仍存在，但健康检查或监听端口验证未通过",
                    service_name
                )
            }),
        };
    }

    if let Some(runtime) = runtime.filter(|runtime| {
        matches!(
            runtime.lifecycle,
            LifecycleState::Probing | LifecycleState::Starting | LifecycleState::Stopping
        )
    }) {
        let port = runtime.effective_port.unwrap_or(configured_effective_port);
        let tracked_process_valid =
            runtime_process_identity_is_valid(Some(runtime), expected_exe, processes);
        return DependencyAssessment {
            lifecycle: runtime.lifecycle,
            ownership: if tracked_process_valid {
                kg_coordination::SharedOwnership::SelfManaged
            } else {
                kg_coordination::SharedOwnership::None
            },
            display_port: port,
            effective_port: Some(port),
            running: false,
            managed_pid: runtime
                .process
                .as_ref()
                .filter(|_| tracked_process_valid)
                .map(|process| process.pid),
            port_occupied: false,
            occupied_by_pid: None,
            occupied_by_name: None,
            can_start: false,
            can_stop: runtime.lifecycle == LifecycleState::Starting && tracked_process_valid,
            status_message: match runtime.lifecycle {
                LifecycleState::Probing => format!("正在为 {} 检查候选端口", service_name),
                LifecycleState::Starting => format!("{} 正在启动", service_name),
                LifecycleState::Stopping => format!("{} 正在停止", service_name),
                _ => String::new(),
            },
        };
    }

    if shared_healthy {
        if let Some((process, instance)) = shared {
            let expected_name = expected_exe
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            if shared_process_is_valid(
                process,
                std::path::Path::new(&instance.service_dir),
                &expected_name,
                listeners,
                processes,
            ) {
                let port = parse_url_port(&process.url, preferred_port);
                return DependencyAssessment {
                    lifecycle: LifecycleState::Running,
                    ownership: kg_coordination::SharedOwnership::SharedManaged,
                    display_port: port,
                    effective_port: Some(port),
                    running: true,
                    managed_pid: Some(process.pid),
                    port_occupied: false,
                    occupied_by_pid: None,
                    occupied_by_name: None,
                    can_start: false,
                    can_stop: false,
                    status_message: format!(
                        "{} 来自共享 KG 实例，当前应用仅持有使用租约",
                        service_name
                    ),
                };
            }
        }
    }

    let probe_port = runtime
        .and_then(|value| value.effective_port)
        .unwrap_or(preferred_port);
    if let Some(pid) = listeners.get(&probe_port).copied() {
        let name = process_display_name(pid, processes);
        return DependencyAssessment {
            lifecycle: LifecycleState::PortOccupied,
            ownership: kg_coordination::SharedOwnership::ExternalUnmanaged,
            display_port: probe_port,
            effective_port: None,
            running: false,
            managed_pid: None,
            port_occupied: true,
            occupied_by_pid: Some(pid),
            occupied_by_name: name.clone(),
            can_start: true,
            can_stop: false,
            status_message: match name {
                Some(name) => format!(
                    "端口 {} 已被外部程序 {}（PID {}）占用，启动时将自动选择其他端口",
                    probe_port, name, pid
                ),
                None => format!(
                    "端口 {} 已被外部程序（PID {}）占用，启动时将自动选择其他端口",
                    probe_port, pid
                ),
            },
        };
    }

    if let Some(runtime) = runtime.filter(|runtime| {
        matches!(
            runtime.lifecycle,
            LifecycleState::Running | LifecycleState::Unhealthy | LifecycleState::Failed
        )
    }) {
        let message = runtime.last_error.clone().unwrap_or_else(|| {
            if runtime.lifecycle == LifecycleState::Running {
                format!(
                    "{} 的运行记录已失效（PID、启动时间、程序路径或监听端口不一致），未终止任何进程",
                    service_name
                )
            } else {
                format!("{} 当前不可用", service_name)
            }
        });
        return DependencyAssessment {
            lifecycle: if runtime.lifecycle == LifecycleState::Running {
                LifecycleState::Failed
            } else {
                runtime.lifecycle
            },
            ownership: kg_coordination::SharedOwnership::None,
            display_port: probe_port,
            effective_port: None,
            running: false,
            managed_pid: None,
            port_occupied: false,
            occupied_by_pid: None,
            occupied_by_name: None,
            can_start: true,
            can_stop: false,
            status_message: message,
        };
    }

    DependencyAssessment {
        lifecycle: LifecycleState::Stopped,
        ownership: kg_coordination::SharedOwnership::None,
        display_port: preferred_port,
        effective_port: None,
        running: false,
        managed_pid: None,
        port_occupied: false,
        occupied_by_pid: None,
        occupied_by_name: None,
        can_start: true,
        can_stop: false,
        status_message: format!("{} 未启动，首选端口 {} 可用", service_name, preferred_port),
    }
}

async fn dependency_service_is_running(service_id: &str) -> bool {
    if verified_owned_dependency_process(service_id)
        .await
        .is_some()
    {
        return true;
    }
    let Ok(Some(registry)) = kg_coordination::read_registry() else {
        return false;
    };
    if registry.state != "ready" {
        return false;
    }
    let (process, expected_name) = match service_id {
        "qdrant" => (&registry.instance.qdrant, "qdrant.exe"),
        "llama_server" => (&registry.instance.llama_server, "llama-server.exe"),
        "fastkg" => (&registry.instance.fastkg, "FastKG.exe"),
        _ => return false,
    };
    let listeners = get_listening_port_pid_map();
    let processes = capture_process_facts(&[process.pid]);
    shared_process_is_valid(
        process,
        std::path::Path::new(&registry.instance.service_dir),
        expected_name,
        &listeners,
        &processes,
    )
}

async fn dependency_service_is_usable(service_id: &str) -> bool {
    if let Some(identity) = verified_owned_dependency_process(service_id).await {
        return dependency_health_ok(service_id, identity.effective_port).await;
    }
    // 监督器身份丢失时：若本机 FastKG 目录下的同名进程仍在监听且健康，也视为可用
    if adopt_local_dependency_process(service_id).await.is_some() {
        return true;
    }
    let Ok(Some(registry)) = kg_coordination::read_registry() else {
        return false;
    };
    if registry.state != "ready" || !fastkg_health_ok(&registry.instance.fastkg.url).await {
        return false;
    }
    let (process, expected_name) = match service_id {
        "qdrant" => (&registry.instance.qdrant, "qdrant.exe"),
        "llama_server" => (&registry.instance.llama_server, "llama-server.exe"),
        "fastkg" => (&registry.instance.fastkg, "FastKG.exe"),
        _ => return false,
    };
    let listeners = get_listening_port_pid_map();
    let processes = capture_process_facts(&[process.pid]);
    shared_process_is_valid(
        process,
        std::path::Path::new(&registry.instance.service_dir),
        expected_name,
        &listeners,
        &processes,
    )
}

/// 查找本机期望 exe 正在监听的端口（不依赖监督器 / kg_pids）。
fn find_local_dependency_listener(service_id: &str) -> Option<(u16, u32)> {
    let expected_exe = expected_dependency_exe(service_id)?;
    let preferred = dependency_preferred_port(service_id);
    let mut candidates: Vec<u16> = Vec::new();
    candidates.push(preferred);
    if let Some(runtime) = dependency_runtime_port(service_id) {
        if runtime != preferred {
            candidates.push(runtime);
        }
    }
    let listeners = get_listening_port_pid_map();
    for (&port, &pid) in &listeners {
        if candidates.contains(&port) {
            continue;
        }
        let Some(exe) = get_process_exe_path(pid) else {
            continue;
        };
        if same_executable(std::path::Path::new(&exe), &expected_exe) {
            candidates.push(port);
        }
    }
    for port in candidates {
        let Some(pid) = listeners
            .get(&port)
            .copied()
            .or_else(|| get_pid_listening_on_port(port))
        else {
            continue;
        };
        let Some(exe) = get_process_exe_path(pid) else {
            continue;
        };
        if same_executable(std::path::Path::new(&exe), &expected_exe) {
            return Some((port, pid));
        }
    }
    None
}

// 所有权移交后，旧监督器快照不能继续授权停止或重新登记为自身进程。
fn ensure_dependency_registry_ownership(service_id: &str) -> Result<(), AppError> {
    if matches!(service_id, "qdrant" | "llama_server" | "fastkg") {
        if let Some(registry) = kg_coordination::read_registry().map_err(coordination_err)? {
            if !kg_coordination::current_fastclaw_is_owner(&registry) {
                return Err(AppError::Business(format!(
                    "KG 实例由 {} 管理，当前应用不能接管或重启该实例",
                    registry.instance.starter_app
                )));
            }
        }
    }
    Ok(())
}

/// 只按已有进程身份或本应用拥有的共享指纹接管，不以相同 exe 路径证明所有权。
async fn adopt_local_dependency_process(service_id: &str) -> Option<u16> {
    ensure_dependency_registry_ownership(service_id).ok()?;
    if let Some(identity) = verified_owned_dependency_process(service_id).await {
        if dependency_health_ok(service_id, identity.effective_port).await {
            set_dependency_runtime_port(service_id, Some(identity.effective_port));
            return Some(identity.effective_port);
        }
    }
    let (port, pid) = find_local_dependency_listener(service_id)?;
    let tracked = verified_tracked_dependency_process(service_id).await
        .is_some_and(|identity| identity.pid == pid && identity.effective_port == port);
    if !tracked {
        let registry = kg_coordination::read_registry().ok()??;
        if !kg_coordination::current_fastclaw_is_owner(&registry) { return None; }
        let process = match service_id {
            "qdrant" => &registry.instance.qdrant,
            "llama_server" => &registry.instance.llama_server,
            "fastkg" => &registry.instance.fastkg,
            _ => return None,
        };
        let facts = capture_process_facts(&[pid]);
        let expected = expected_dependency_exe(service_id)?;
        if process.pid != pid || process.process_started_at.is_none()
            || !shared_process_is_valid(process, std::path::Path::new(&registry.instance.service_dir),
                expected.file_name()?.to_str()?, &HashMap::from([(port, pid)]), &facts)
        { return None; }
    }
    let healthy = dependency_health_ok(service_id, port).await;
    if !healthy {
        // 本机同目录进程已在监听时绝不能再起第二份（Qdrant 同 storage 会立刻 exit 101）。
        // 健康检查短暂失败也优先接管，由后续 /health 或业务请求再判定。
        tracing::warn!(
            "[knowledge] 发现本机 {} 进程 PID={} 端口={}，健康检查暂未通过，仍接管以避免重复启动",
            service_id,
            pid,
            port
        );
    }
    let expected_exe = expected_dependency_exe(service_id)?;
    let preferred = dependency_preferred_port(service_id);
    match record_owned_dependency_process(service_id, preferred, port, pid, &expected_exe).await {
        Ok(_) => {
            set_dependency_runtime_port(service_id, Some(port));
            if !healthy {
                dependency_service_supervisor().mark_unhealthy(service_id, "进程身份有效，但健康检查未通过").await;
                return None;
            }
            tracing::info!(
                "[knowledge] 复用已运行的 {} (PID={}, 端口={})",
                service_id,
                pid,
                port
            );
            Some(port)
        }
        Err(error) => {
            tracing::warn!(
                "[knowledge] 无法接管已运行的 {} PID={}：{}",
                service_id,
                pid,
                error
            );
            None
        }
    }
}

/// 检查 KG 服务文件是否全部存在（内部辅助函数）
pub(crate) fn check_kg_files_installed() -> bool {
    inspect_kg_service_files().all_exist
}

/// 只有已校验的自身/共享监听进程才需要 HTTP 探活。
fn dependency_health_probe_port(
    runtime: Option<&crate::core::dependency_services::ServiceRuntime>,
    expected_exe: &std::path::Path,
    shared: Option<(&SharedServiceProcess, &SharedKgInstance)>,
    listeners: &HashMap<u16, u32>,
    processes: &HashMap<u32, ProcessFacts>,
) -> Option<u16> {
    if let Some(identity) = runtime.and_then(|runtime| runtime.process.as_ref()) {
        if crate::core::dependency_services::identity_matches(
            identity, processes.get(&identity.pid), listeners.get(&identity.effective_port).copied(), expected_exe,
        ) {
            return Some(identity.effective_port);
        }
    }
    let (process, instance) = shared?;
    if shared_process_is_valid(process, std::path::Path::new(&instance.service_dir),
        expected_exe.file_name()?.to_str()?, listeners, processes) {
        Some(parse_url_port(&process.url, 0))
    } else { None }
}

async fn probe_dependency_if_present(service_id: &str, port: Option<u16>) -> bool {
    match port {
        Some(port) => dependency_health_ok(service_id, port).await,
        None => false,
    }
}

/// 只对经过身份与监听校验的进程探活，不向空端口或外部占用者发送 HTTP。
#[tauri::command]
pub async fn get_dependency_services_status(
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<Vec<DepServiceStatus>, AppError> {
    let kg_installed = check_kg_files_installed();

    // 读取 config.json 中的服务 URL 配置
    let config = state.app_config.read().await;
    let svc = &config.services;

    let (qdrant_host, qdrant_port) =
        parse_service_url(&svc.qdrant, "127.0.0.1", paths::qdrant_default_port());
    let (fastkg_host, fastkg_port) =
        parse_service_url(&svc.fastkg, "127.0.0.1", paths::fastkg_default_port());
    let (llama_host, llama_port) = parse_service_url(
        &svc.llama_server,
        "127.0.0.1",
        paths::llama_server_default_port(),
    );
    let (fp_host, fp_port) =
        parse_service_url(&svc.fastplan, "127.0.0.1", paths::fastplan_default_port());
    drop(config);
    let qdrant_effective_port = dependency_runtime_port("qdrant").unwrap_or(qdrant_port);
    let fastkg_effective_port = dependency_runtime_port("fastkg").unwrap_or(fastkg_port);
    let llama_effective_port = dependency_runtime_port("llama_server").unwrap_or(llama_port);
    let fp_effective_port = dependency_runtime_port("fastplan").unwrap_or(fp_port);
    let kg_registry = kg_coordination::read_registry().ok().flatten();
    let kg_shared_status = kg_registry
        .as_ref()
        .map(|registry| kg_coordination::status_from_registry(Some(registry)))
        .unwrap_or_else(KgSharedStatus::none);
    let runtimes = state.dependency_services.snapshots().await;
    let listeners = get_listening_port_pid_map();
    let mut relevant_pids: Vec<u32> = listeners.values().copied().collect();
    relevant_pids.extend(
        runtimes
            .values()
            .filter_map(|runtime| runtime.process.as_ref().map(|process| process.pid)),
    );
    if let Some(registry) = kg_registry.as_ref() {
        relevant_pids.extend([
            registry.instance.qdrant.pid,
            registry.instance.llama_server.pid,
            registry.instance.fastkg.pid,
        ]);
    }
    relevant_pids.sort_unstable();
    relevant_pids.dedup();
    let processes = capture_process_facts(&relevant_pids);
    let ready_instance = kg_registry.as_ref().filter(|registry| registry.state == "ready").map(|registry| &registry.instance);
    let probe_port = |id: &str, expected: &std::path::Path, shared| {
        dependency_health_probe_port(runtimes.get(id), expected, shared, &listeners, &processes)
    };
    let shared_fastkg_port = dependency_health_probe_port(None, &paths::fastkg_server_path(),
        ready_instance.map(|instance| (&instance.fastkg, instance)), &listeners, &processes);
    let (qdrant_healthy, llama_healthy, fastkg_healthy, fastplan_healthy, shared_healthy) = tokio::join!(
        probe_dependency_if_present("qdrant", probe_port("qdrant", &paths::qdrant_path(), ready_instance.map(|i| (&i.qdrant, i)))),
        probe_dependency_if_present("llama_server", probe_port("llama_server", &paths::llama_server_path(), ready_instance.map(|i| (&i.llama_server, i)))),
        probe_dependency_if_present("fastkg", probe_port("fastkg", &paths::fastkg_server_path(), ready_instance.map(|i| (&i.fastkg, i)))),
        probe_dependency_if_present("fastplan", probe_port("fastplan", &paths::fastplan_server_path(), None)),
        probe_dependency_if_present("fastkg", shared_fastkg_port),
    );

    let mut qdrant_assessment = assess_dependency_service(
        "向量库服务",
        qdrant_port,
        qdrant_effective_port,
        &paths::qdrant_path(),
        runtimes.get("qdrant"),
        qdrant_healthy,
        kg_registry
            .as_ref()
            .map(|registry| (&registry.instance.qdrant, &registry.instance)),
        shared_healthy,
        &listeners,
        &processes,
    );
    let mut llama_assessment = assess_dependency_service(
        "嵌入向量服务",
        llama_port,
        llama_effective_port,
        &paths::llama_server_path(),
        runtimes.get("llama_server"),
        llama_healthy,
        kg_registry
            .as_ref()
            .map(|registry| (&registry.instance.llama_server, &registry.instance)),
        shared_healthy,
        &listeners,
        &processes,
    );
    let mut fastkg_assessment = assess_dependency_service(
        "知识库服务",
        fastkg_port,
        fastkg_effective_port,
        &paths::fastkg_server_path(),
        runtimes.get("fastkg"),
        fastkg_healthy,
        kg_registry
            .as_ref()
            .map(|registry| (&registry.instance.fastkg, &registry.instance)),
        shared_healthy,
        &listeners,
        &processes,
    );
    if !kg_installed {
        for assessment in [
            &mut qdrant_assessment,
            &mut llama_assessment,
            &mut fastkg_assessment,
        ] {
            assessment.can_start = false;
            if !assessment.running {
                assessment.status_message = "KG 服务包尚未下载完整".to_string();
            }
        }
    }
    let fastplan_assessment = assess_dependency_service(
        "FastPlan服务",
        fp_port,
        fp_effective_port,
        &paths::fastplan_server_path(),
        runtimes.get("fastplan"),
        fastplan_healthy,
        None,
        false,
        &listeners,
        &processes,
    );
    let kg_has_other_consumers = kg_shared_status
        .consumers
        .iter()
        .any(|app| !app.eq_ignore_ascii_case(kg_coordination::APP_ID));
    if kg_has_other_consumers {
        qdrant_assessment.can_stop = false;
        llama_assessment.can_stop = false;
        fastkg_assessment.can_stop = false;
    }

    Ok(vec![
        DepServiceStatus {
            model_sync_status: None,
            id: "qdrant".into(),
            name: "向量库服务".into(),
            port: qdrant_assessment.display_port,
            preferred_port: qdrant_port,
            using_fallback: qdrant_assessment
                .effective_port
                .map(|port| port != qdrant_port)
                .unwrap_or(false),
            host: qdrant_host,
            running: qdrant_assessment.running,
            pid: qdrant_assessment.managed_pid,
            lifecycle_state: qdrant_assessment.lifecycle.as_str().into(),
            effective_port: qdrant_assessment.effective_port,
            port_occupied: qdrant_assessment.port_occupied,
            occupied_by_pid: qdrant_assessment.occupied_by_pid,
            occupied_by_name: qdrant_assessment.occupied_by_name,
            can_start: qdrant_assessment.can_start,
            can_stop: qdrant_assessment.can_stop,
            status_message: qdrant_assessment.status_message,
            description: "向量数据库 (Qdrant)，存储文档嵌入向量用于相似度搜索".into(),
            installed: kg_installed,
            configurable: true,
            ownership: qdrant_assessment.ownership,
            starter_app: qdrant_assessment
                .running
                .then(|| kg_shared_status.starter_app.clone())
                .flatten(),
            consumers: kg_shared_status.consumers.clone(),
            lease_count: kg_shared_status.lease_count,
            lease_registered: kg_shared_status.lease_registered,
            shared_instance: kg_shared_status.shared_instance,
            pending_restart: kg_shared_status.pending_restart,
        },
        DepServiceStatus {
            model_sync_status: None,
            id: "llama_server".into(),
            name: "嵌入向量服务".into(),
            port: llama_assessment.display_port,
            preferred_port: llama_port,
            using_fallback: llama_assessment
                .effective_port
                .map(|port| port != llama_port)
                .unwrap_or(false),
            host: llama_host,
            running: llama_assessment.running,
            pid: llama_assessment.managed_pid,
            lifecycle_state: llama_assessment.lifecycle.as_str().into(),
            effective_port: llama_assessment.effective_port,
            port_occupied: llama_assessment.port_occupied,
            occupied_by_pid: llama_assessment.occupied_by_pid,
            occupied_by_name: llama_assessment.occupied_by_name,
            can_start: llama_assessment.can_start,
            can_stop: llama_assessment.can_stop,
            status_message: llama_assessment.status_message,
            description: "文本嵌入模型服务 (BGE-M3)，将文档转换为向量".into(),
            installed: kg_installed,
            configurable: true,
            ownership: llama_assessment.ownership,
            starter_app: llama_assessment
                .running
                .then(|| kg_shared_status.starter_app.clone())
                .flatten(),
            consumers: kg_shared_status.consumers.clone(),
            lease_count: kg_shared_status.lease_count,
            lease_registered: kg_shared_status.lease_registered,
            shared_instance: kg_shared_status.shared_instance,
            pending_restart: kg_shared_status.pending_restart,
        },
        DepServiceStatus {
            model_sync_status: None,
            id: "fastkg".into(),
            name: "知识库服务".into(),
            port: fastkg_assessment.display_port,
            preferred_port: fastkg_port,
            using_fallback: fastkg_assessment
                .effective_port
                .map(|port| port != fastkg_port)
                .unwrap_or(false),
            host: fastkg_host,
            running: fastkg_assessment.running,
            pid: fastkg_assessment.managed_pid,
            lifecycle_state: fastkg_assessment.lifecycle.as_str().into(),
            effective_port: fastkg_assessment.effective_port,
            port_occupied: fastkg_assessment.port_occupied,
            occupied_by_pid: fastkg_assessment.occupied_by_pid,
            occupied_by_name: fastkg_assessment.occupied_by_name,
            can_start: fastkg_assessment.can_start,
            can_stop: fastkg_assessment.can_stop,
            status_message: fastkg_assessment.status_message,
            description: "知识库核心服务 (FastKG)，提供知识检索与管理功能".into(),
            installed: kg_installed,
            configurable: true,
            ownership: fastkg_assessment.ownership,
            starter_app: fastkg_assessment
                .running
                .then(|| kg_shared_status.starter_app.clone())
                .flatten(),
            consumers: kg_shared_status.consumers.clone(),
            lease_count: kg_shared_status.lease_count,
            lease_registered: kg_shared_status.lease_registered,
            shared_instance: kg_shared_status.shared_instance,
            pending_restart: kg_shared_status.pending_restart,
        },
        DepServiceStatus {
            model_sync_status: None,
            id: "fastplan".into(),
            name: "FastPlan服务".into(),
            port: fastplan_assessment.display_port,
            preferred_port: fp_port,
            using_fallback: fastplan_assessment
                .effective_port
                .map(|port| port != fp_port)
                .unwrap_or(false),
            host: fp_host,
            running: fastplan_assessment.running,
            pid: fastplan_assessment.managed_pid,
            lifecycle_state: fastplan_assessment.lifecycle.as_str().into(),
            effective_port: fastplan_assessment.effective_port,
            port_occupied: fastplan_assessment.port_occupied,
            occupied_by_pid: fastplan_assessment.occupied_by_pid,
            occupied_by_name: fastplan_assessment.occupied_by_name,
            can_start: fastplan_assessment.can_start,
            can_stop: fastplan_assessment.can_stop,
            status_message: fastplan_assessment.status_message,
            description: "AI任务规划与执行引擎，提供任务拆解和DAG编排能力".into(),
            installed: paths::fastplan_server_path().is_file(),
            configurable: true,
            ownership: fastplan_assessment.ownership,
            starter_app: None,
            consumers: Vec::new(),
            lease_count: 0,
            lease_registered: false,
            shared_instance: false,
            pending_restart: false,
        },
    ])
}

/// 启动单个依赖服务（含前置依赖检查）
/// 手动启动仅启动目标服务本身，不重启已运行的前置服务
#[tauri::command]
pub async fn start_dependency_service(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, crate::state::AppState>,
    service_id: String,
) -> Result<String, AppError> {
    state.dependency_recovery.request(&app_handle, &service_id, service_id == "fastkg", false, true)
}

async fn start_dependency_once(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, crate::state::AppState>,
    service_id: String,
) -> Result<String, AppError> {
    configure_preferred_ports_from_state(&state).await;
    match service_id.as_str() {
        "qdrant" | "llama_server" | "fastkg" => {
            if !check_kg_files_installed() {
                return Err(AppError::Business(
                    "KG 服务文件未安装，请先在知识库页面下载安装".into(),
                ));
            }
        }
        _ => {}
    }

    let result = match service_id.as_str() {
        "qdrant" => start_single_qdrant_if_not_running(&app_handle, &state.kg_pids).await,
        "llama_server" => start_single_llama_if_not_running(&app_handle, &state.kg_pids).await,
        "fastkg" => {
            let _maintenance_guard = FASTKG_MAINTENANCE_LOCK.lock().await;
            // 前置依赖必须通过进程身份/共享注册表校验，不能只因端口有监听就放行。
            if !dependency_service_is_usable("qdrant").await {
                return Err(AppError::Business(
                    "FastKG 依赖 Qdrant，请先启动向量库服务".into(),
                ));
            }
            if !dependency_service_is_usable("llama_server").await {
                return Err(AppError::Business(
                    "FastKG 依赖嵌入向量服务，请先启动嵌入向量服务".into(),
                ));
            }
            // 与 start_kg_inner 共用守卫，避免 5s 状态轮询把正在启动的 FastKG 回收为 Failed
            let Some(_kg_starting_guard) = KgStartingGuard::acquire() else {
                return Err(AppError::Business("知识库服务正在启动中，请稍候".into()));
            };
            let result = start_single_fastkg_if_not_running(&app_handle, &state.kg_pids).await;
            result
        }
        "fastplan" => start_fastplan_once(&app_handle, &state.kg_pids).await,
        _ => Err(AppError::Business(format!(
            "未知的服务类型: {}",
            service_id
        ))),
    };
    if let Err(error) = &result {
        let supervisor = dependency_service_supervisor();
        let preserve_recoverable_process = supervisor
            .snapshot(&service_id)
            .await
            .is_some_and(|runtime| {
                runtime.lifecycle == LifecycleState::Unhealthy && runtime.process.is_some()
            });
        if !preserve_recoverable_process {
            supervisor
                .mark_failed(&service_id, error.to_string())
                .await;
        }
    }
    result
}

/// 从 URL 字符串中解析端口号
pub(crate) fn signal_dependency_stop(service_id: &str) {
    let flag = match service_id {
        "qdrant" => &QDRANT_STOP_REQUESTED,
        "llama_server" => &LLAMA_STOP_REQUESTED,
        "fastkg" => &FASTKG_STOP_REQUESTED,
        "fastplan" => &FASTPLAN_STOP_REQUESTED,
        _ => return,
    };
    flag.store(true, std::sync::atomic::Ordering::SeqCst);
}

pub(crate) async fn reconcile_dependency_processes(app: &tauri::AppHandle) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    reconcile_stale_kg_starting_states(&state.kg_pids).await;
    reconcile_stale_fastplan_starting_state(&state.kg_pids).await;
    Ok(())
}

pub(crate) async fn reconcile_dependency_shared_runtime(app: &tauri::AppHandle) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    kg_coordination::adopt_own_orphan_for_recovery().map_err(coordination_err)?;
    if let Some(registry) = kg_coordination::read_registry().map_err(coordination_err)? {
        let status = kg_coordination::status_from_registry(Some(&registry));
        if !status.lease_registered && registry.state == "ready" {
            let _ = ensure_kg_lease_for_ready_registry(app, &state.kg_pids).await?;
        } else if status.lease_registered {
            start_kg_lease_heartbeat();
        }
    }
    Ok(())
}

fn dependency_stop_requested(service_id: &str) -> bool {
    let flag = match service_id {
        "qdrant" => &QDRANT_STOP_REQUESTED,
        "llama_server" => &LLAMA_STOP_REQUESTED,
        "fastkg" => &FASTKG_STOP_REQUESTED,
        "fastplan" => &FASTPLAN_STOP_REQUESTED,
        _ => return false,
    };
    flag.load(std::sync::atomic::Ordering::SeqCst)
}

pub(crate) async fn recover_dependency_once(
    app: &tauri::AppHandle,
    service_id: &str,
    generation: u64,
    timeout: std::time::Duration,
) -> Result<(), (crate::core::dependency_services::recovery::FailureCode, String)> {
    use crate::core::dependency_services::recovery::FailureCode;
    let state = app.state::<AppState>();
    configure_preferred_ports_from_state(&state).await;
    let expected = expected_dependency_exe(service_id)
        .ok_or((FailureCode::InvalidConfiguration, "未知服务".into()))?;
    if !state.dependency_recovery.is_current(service_id, generation) {
        return Err((FailureCode::Cancelled, "启动请求已取消".into()));
    }
    if matches!(service_id, "qdrant" | "llama_server" | "fastkg") {
        kg_coordination::adopt_own_orphan_for_recovery().map_err(|e| (FailureCode::OwnershipUnverified, e.to_string()))?;
        let registry = kg_coordination::read_registry().map_err(|e| (FailureCode::OwnershipUnverified, e.to_string()))?;
        if let Some(registry) = registry.as_ref() {
            if !kg_coordination::current_fastclaw_is_owner(&registry) {
                if shared_kg_registry_is_valid(&registry).await {
                    ensure_kg_lease_for_ready_registry(app, &state.kg_pids).await
                        .map_err(|e| (FailureCode::OwnershipUnverified, e.to_string()))?;
                    return Ok(());
                }
                return Err((FailureCode::SharedMaintenanceBlocked, "共享 KG 正由其他应用管理，等待所有者恢复".into()));
            }
        }
        // 调度不再依赖先完成整组状态查询；共享复用必须先于本地安装检查。
        if !expected.is_file() { return Err((FailureCode::NotInstalled, format!("服务文件不存在: {}", expected.display()))); }
        if registry.is_some() {
            // 共享注册表身份是旧版本遗留进程的有效凭据；不能仅凭路径接管。
            let _ = adopt_local_dependency_process(service_id).await;
        } else {
            kg_coordination::begin_fastkg_maintenance(Some(kg_coordination::local_starting_instance()))
                .map_err(|e| (FailureCode::SharedMaintenanceBlocked, e.to_string()))?;
        }
        start_kg_lease_heartbeat();
    }
    if !expected.is_file() { return Err((FailureCode::NotInstalled, format!("服务文件不存在: {}", expected.display()))); }
    let supervisor = dependency_service_supervisor();
    if supervisor.snapshot(service_id).await.and_then(|runtime| runtime.process).is_none() {
        ensure_dependency_registry_ownership(service_id).map_err(|e| (FailureCode::OwnershipUnverified, e.to_string()))?;
        if let Some(identity) = supervisor.owned.orphan(service_id, &expected)
            .map_err(|e| (FailureCode::OwnershipUnverified, e))? {
            supervisor.record_starting_process(service_id, dependency_preferred_port(service_id), identity.effective_port, identity.pid, &expected)
                .await.map_err(|e| (FailureCode::OwnershipUnverified, e))?;
            set_dependency_runtime_port(service_id, Some(identity.effective_port));
            let mut pids = state.kg_pids.write().await;
            match service_id {
                "qdrant" => pids.qdrant = Some(identity.pid),
                "llama_server" => pids.llama_server = Some(identity.pid),
                "fastkg" => pids.fastkg = Some(identity.pid),
                "fastplan" => pids.fastplan = Some(identity.pid),
                _ => {}
            }
            drop(pids);
            if service_id == "fastplan" {
                let saved = state.dependency_recovery.fastplan_resume.lock().unwrap().clone();
                let saved = saved.filter(|saved| saved.pid == identity.pid && saved.process_started_at == identity.process_started_at);
                if let Some(saved) = saved {
                    let client = reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none())
                        .timeout(std::time::Duration::from_secs(2)).build().map_err(|e| (FailureCode::Unknown, e.to_string()))?;
                    let response = client.get(format!("http://127.0.0.1:{}/api/sessions", identity.effective_port))
                        .bearer_auth(&saved.parent_token).send().await;
                    if response.is_ok_and(|response| response.status().is_success())
                        && get_pid_listening_on_port(identity.effective_port) == Some(identity.pid) {
                        state.kg_pids.write().await.fastplan_connection = Some(crate::commands::fastplan_connection::FastPlanConnectionInfo {
                            api_base: format!("http://127.0.0.1:{}", identity.effective_port),
                            ws_base: format!("ws://127.0.0.1:{}", identity.effective_port), parent_token: saved.parent_token,
                        });
                        crate::state::FASTPLAN_PID.store(identity.pid, std::sync::atomic::Ordering::SeqCst);
                    }
                }
            }
            let can_reuse = service_id != "fastplan" || state.kg_pids.read().await.fastplan_connection.is_some();
            if can_reuse && dependency_health_ok(service_id, identity.effective_port).await {
                record_owned_dependency_process(service_id, dependency_preferred_port(service_id), identity.effective_port, identity.pid, &expected)
                    .await.map_err(|e| (FailureCode::OwnershipUnverified, e.to_string()))?;
            } else {
                supervisor.mark_unhealthy(service_id, "自身孤儿服务未通过健康或鉴权校验，等待安全重启").await;
            }
        }
    }
    if let Some(identity) = verified_tracked_dependency_process(service_id).await {
        let mut pids = state.kg_pids.write().await;
        match service_id {
            "qdrant" => pids.qdrant = Some(identity.pid),
            "llama_server" => pids.llama_server = Some(identity.pid),
            "fastkg" => pids.fastkg = Some(identity.pid),
            _ => {}
        }
    }
    let connection_ready = service_id != "fastplan" || state.kg_pids.read().await.fastplan_connection.is_some();
    if connection_ready && dependency_service_is_usable(service_id).await {
        if service_id == "fastkg" {
            // 健康孤儿可以复用，但注册表也必须恢复 ready，不能只修复本地状态。
            start_single_fastkg_if_not_running(app, &state.kg_pids).await
                .map_err(|e| (FailureCode::SharedMaintenanceBlocked, e.to_string()))?;
        }
        return Ok(());
    }
    let recovery_claim = if matches!(service_id, "qdrant" | "llama_server" | "fastkg") {
        kg_coordination::begin_owner_recovery()
            .map_err(|e| (FailureCode::SharedMaintenanceBlocked, e.to_string()))?
    } else { None };
    let outcome = async {
    // 上游恢复必须先停止自身 FastKG，避免它继续使用旧端口。
    if matches!(service_id, "qdrant" | "llama_server") && verified_tracked_dependency_process("fastkg").await.is_some() {
        stop_single_fastkg(&state.kg_pids).await.map_err(|e| (FailureCode::SharedMaintenanceBlocked, e.to_string()))?;
        state.dependency_recovery.upstream_changed();
    }
    if verified_tracked_dependency_process(service_id).await.is_some() {
        stop_dependency_once(app.state::<AppState>(), service_id.into()).await
            .map_err(|e| (FailureCode::OwnershipUnverified, e.to_string()))?;
    }
    if service_id == "fastkg" {
        if let (Some(qdrant), Some(llama)) = (verified_owned_dependency_process("qdrant").await, verified_owned_dependency_process("llama_server").await) {
            let qdrant = shared_service_process_from_identity(&qdrant, format!("http://127.0.0.1:{}", qdrant.effective_port))
                .map_err(|e| (FailureCode::OwnershipUnverified, e.to_string()))?;
            let llama = shared_service_process_from_identity(&llama, format!("http://127.0.0.1:{}", llama.effective_port))
                .map_err(|e| (FailureCode::OwnershipUnverified, e.to_string()))?;
            if let Some(claim) = &recovery_claim {
                kg_coordination::publish_recovered_dependencies(claim, qdrant, llama)
                    .map_err(|e| (FailureCode::OwnershipUnverified, e.to_string()))?;
            }
        }
    }
    if !state.dependency_recovery.is_current(service_id, generation) {
        if let Some(claim) = &recovery_claim { let _ = kg_coordination::publish_recovery_failure(claim, "恢复已被用户取消"); }
        return Err((FailureCode::Cancelled, "启动请求已取消".into()));
    }
    let startup = tokio::time::timeout(timeout, start_dependency_once(app.clone(), app.state::<AppState>(), service_id.into()));
    tokio::pin!(startup);
    let result = loop {
        tokio::select! {
            result = &mut startup => break result,
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                if !state.dependency_recovery.is_current(service_id, generation) { signal_dependency_stop(service_id); }
            }
        }
    };
    let mut outcome = match result {
        Ok(Ok(_)) if dependency_service_is_usable(service_id).await => Ok(()),
        Ok(Ok(_)) => Err((if state.dependency_recovery.is_current(service_id, generation) { FailureCode::HealthTimeout } else { FailureCode::Cancelled }, "启动已取消或未完成健康验证".into())),
        Ok(Err(error)) => {
            let code = match &error {
                AppError::DependencyPortExhausted { .. } => FailureCode::PortRangeExhausted,
                AppError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => FailureCode::NotInstalled,
                AppError::Io(io) if io.kind() == std::io::ErrorKind::PermissionDenied => FailureCode::PermissionDenied,
                AppError::Io(io) if io.kind() == std::io::ErrorKind::AddrInUse => FailureCode::PortConflict,
                AppError::Parse(_) | AppError::Validation(_) | AppError::InvalidInput(_) => FailureCode::InvalidConfiguration,
                _ => FailureCode::Unknown,
            };
            Err((code, error.to_string()))
        }
        Err(_) => {
            signal_dependency_stop(service_id);
            cleanup_tracked_dependency_start(service_id, &state.kg_pids, "启动健康检查超时").await
                .map_err(|e| (FailureCode::OwnershipUnverified, e.to_string()))?;
            dependency_service_supervisor().mark_failed(service_id, "启动健康检查超时").await;
            Err((FailureCode::HealthTimeout, "启动健康检查超时".into()))
        }
    };
    if !state.dependency_recovery.is_current(service_id, generation) {
        signal_dependency_stop(service_id);
        cleanup_tracked_dependency_start(service_id, &state.kg_pids, "启动已取消").await
            .map_err(|e| (FailureCode::OwnershipUnverified, e.to_string()))?;
        outcome = Err((FailureCode::Cancelled, "恢复已由用户取消".into()));
    }
    outcome
    }.await;
    let outcome = if outcome.is_ok() && matches!(service_id, "qdrant" | "llama_server") {
        if let (Some(claim), Some(identity)) = (&recovery_claim, verified_owned_dependency_process(service_id).await) {
            shared_service_process_from_identity(&identity, format!("http://127.0.0.1:{}", identity.effective_port))
                .map_err(|e| (FailureCode::OwnershipUnverified, e.to_string()))
                .and_then(|process| kg_coordination::publish_recovered_upstream(claim, service_id, process)
                    .map_err(|e| (FailureCode::OwnershipUnverified, e.to_string())))
        } else { outcome }
    } else { outcome };
    if let (Some(claim), Err((_, message))) = (&recovery_claim, &outcome) {
        let _ = kg_coordination::publish_recovery_failure(claim, message);
    }
    outcome
}

fn parse_url_port(url: &str, default_port: u16) -> u16 {
    if url.is_empty() {
        return default_port;
    }
    // 提取 host:port 部分
    let after_scheme = url
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let host_port = after_scheme.split('/').next().unwrap_or(after_scheme);
    if let Some(colon_pos) = host_port.rfind(':') {
        host_port[colon_pos + 1..]
            .parse::<u16>()
            .unwrap_or(default_port)
    } else {
        default_port
    }
}

/// 仅启动 Qdrant（如果端口空闲）
async fn start_single_qdrant_if_not_running(
    app_handle: &tauri::AppHandle,
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
) -> Result<String, AppError> {
    let Some(_starting_guard) = QdrantStartingGuard::acquire() else {
        return Err(AppError::Business("Qdrant 正在启动中，请稍候".into()));
    };
    QDRANT_STOP_REQUESTED.store(false, std::sync::atomic::Ordering::SeqCst);
    if let Some(identity) = verified_owned_dependency_process("qdrant").await {
        kg_pids.write().await.qdrant = Some(identity.pid);
        set_dependency_runtime_port("qdrant", Some(identity.effective_port));
        return Ok("Qdrant 已在运行".into());
    }
    kg_pids.write().await.qdrant = None;
    let preferred = dependency_preferred_port("qdrant");
    let (port, attempt, mut reservation) =
        reserve_available_dependency_port(app_handle, "qdrant", preferred).await?;
    let qdrant_path = paths::qdrant_path();
    let work_dir = qdrant_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    emit_dependency_port_state(
        app_handle,
        "qdrant",
        "starting",
        preferred,
        Some(port),
        attempt,
        format!("正在端口 {} 启动 Qdrant", port),
    );
    dependency_service_supervisor()
        .mark_starting("qdrant", preferred, port)
        .await;
    match start_service_with_env(
        &qdrant_path,
        &work_dir,
        &[("QDRANT__SERVICE__HTTP_PORT", port.to_string())],
    ) {
        Ok(pid) => {
            kg_pids.write().await.qdrant = Some(pid);
            if let Err(error) =
                record_starting_dependency_process("qdrant", preferred, port, pid, &qdrant_path)
                    .await
            {
                let _ = kill_pid_sync(pid);
                clear_dependency_process_tracking("qdrant", kg_pids).await;
                if QDRANT_STOP_REQUESTED.load(std::sync::atomic::Ordering::SeqCst) {
                    dependency_service_supervisor().clear("qdrant").await;
                    return Ok("Qdrant 启动已取消".into());
                }
                dependency_service_supervisor()
                    .mark_failed("qdrant", error.to_string())
                    .await;
                return Err(error);
            }
            if !wait_for_owned_port_cancellable(port, pid, 120, &QDRANT_STOP_REQUESTED).await
                || !wait_for_dependency_health("qdrant", port, 120).await
            {
                let cancelled =
                    QDRANT_STOP_REQUESTED.load(std::sync::atomic::Ordering::SeqCst);
                cleanup_tracked_dependency_start(
                    "qdrant",
                    kg_pids,
                    format!("Qdrant 未在端口 {} 就绪", port),
                )
                .await?;
                if cancelled {
                    return Ok("Qdrant 启动已取消".into());
                }
                return Err(AppError::Business(format!("Qdrant 未在端口 {} 就绪", port)));
            }
            if let Err(error) =
                record_owned_dependency_process("qdrant", preferred, port, pid, &qdrant_path).await
            {
                let _ = kill_pid_sync(pid);
                kg_pids.write().await.qdrant = None;
                set_dependency_runtime_port("qdrant", None);
                return Err(error);
            }
            emit_dependency_port_state(
                app_handle,
                "qdrant",
                "ready",
                preferred,
                Some(port),
                attempt,
                format!("Qdrant 已在端口 {} 就绪", port),
            );
            reservation.commit();
            Ok(format!("Qdrant 已启动 (PID={}, 端口={})", pid, port))
        }
        Err(e) => Err(e),
    }
}

/// 等待依赖服务健康检查通过（llama 冷启动加载模型可能需数十秒）
async fn wait_for_dependency_health(service_id: &str, port: u16, attempts: usize) -> bool {
    for attempt in 1..=attempts {
        if dependency_stop_requested(service_id) { return false; }
        if dependency_health_ok(service_id, port).await {
            return true;
        }
        tracing::debug!(
            "[knowledge] {} /health 尚未通过 (attempt {}/{}, port={})",
            service_id,
            attempt,
            attempts,
            port
        );
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    false
}

/// 仅启动 Llama-server（如果端口空闲）
async fn start_single_llama_if_not_running(
    app_handle: &tauri::AppHandle,
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
) -> Result<String, AppError> {
    LLAMA_STOP_REQUESTED.store(false, std::sync::atomic::Ordering::SeqCst);
    if let Some(identity) = verified_owned_dependency_process("llama_server").await {
        if dependency_health_ok("llama_server", identity.effective_port).await {
            kg_pids.write().await.llama_server = Some(identity.pid);
            set_dependency_runtime_port("llama_server", Some(identity.effective_port));
            return Ok("嵌入向量服务已在运行".into());
        }
        tracing::warn!(
            "[knowledge] 嵌入向量服务 PID={} 身份有效但健康检查失败，重启服务",
            identity.pid
        );
        let _ = kill_pid_sync(identity.pid);
        dependency_service_supervisor().clear("llama_server").await;
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    }
    kg_pids.write().await.llama_server = None;
    let preferred = dependency_preferred_port("llama_server");
    let (port, attempt, mut reservation) =
        reserve_available_dependency_port(app_handle, "llama_server", preferred).await?;
    let llama_path = paths::llama_server_path();
    let model_path = paths::bge_m3_model_path();
    let work_dir = llama_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    emit_dependency_port_state(
        app_handle,
        "llama_server",
        "starting",
        preferred,
        Some(port),
        attempt,
        format!("正在端口 {} 启动嵌入向量服务", port),
    );
    dependency_service_supervisor()
        .mark_starting("llama_server", preferred, port)
        .await;
    match start_llama_server(&llama_path, &model_path, port, &work_dir) {
        Ok(pid) => {
            kg_pids.write().await.llama_server = Some(pid);
            if let Err(error) = record_starting_dependency_process("llama_server", preferred, port, pid, &llama_path).await {
                let _ = dependency_service_supervisor().owned.stop_child("llama_server");
                clear_dependency_process_tracking("llama_server", kg_pids).await;
                return Err(error);
            }
            // 端口监听 ≠ 模型就绪；冷启动/更新后加载 GGUF 常需更长时间
            if !wait_for_owned_port_cancellable(port, pid, 180, &LLAMA_STOP_REQUESTED).await
                || !wait_for_dependency_health("llama_server", port, 360).await
            {
                let _ = kill_pid_sync(pid);
                kg_pids.write().await.llama_server = None;
                set_dependency_runtime_port("llama_server", None);
                return Err(AppError::Business(format!(
                    "嵌入向量服务未在端口 {} 就绪",
                    port
                )));
            }
            if let Err(error) =
                record_owned_dependency_process("llama_server", preferred, port, pid, &llama_path)
                    .await
            {
                let _ = kill_pid_sync(pid);
                kg_pids.write().await.llama_server = None;
                set_dependency_runtime_port("llama_server", None);
                return Err(error);
            }
            emit_dependency_port_state(
                app_handle,
                "llama_server",
                "ready",
                preferred,
                Some(port),
                attempt,
                format!("嵌入向量服务已在端口 {} 就绪", port),
            );
            reservation.commit();
            Ok(format!("嵌入向量服务已启动 (PID={}, 端口={})", pid, port))
        }
        Err(e) => Err(e),
    }
}

/// 仅启动 FastKG（如果端口空闲）
async fn start_single_fastkg_if_not_running(
    app_handle: &tauri::AppHandle,
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
) -> Result<String, AppError> {
    FASTKG_STOP_REQUESTED.store(false, std::sync::atomic::Ordering::SeqCst);
    let existing = verified_owned_dependency_process("fastkg").await;
    let mut existing_healthy = false;
    if let Some(identity) = existing.as_ref() {
        kg_pids.write().await.fastkg = Some(identity.pid);
        set_dependency_runtime_port("fastkg", Some(identity.effective_port));
        existing_healthy = dependency_health_ok("fastkg", identity.effective_port).await;
        if existing_healthy {
            let registry_matches = kg_coordination::read_registry()
                .map_err(coordination_err)?
                .map(|registry| {
                    registry.state == "ready"
                        && registry.instance.fastkg.pid == identity.pid
                        && registry
                            .instance
                            .fastkg
                            .process_started_at
                            .map(|started_at| {
                                started_at.timestamp() >= 0
                                    && started_at.timestamp() as u64 == identity.process_started_at
                            })
                            .unwrap_or(false)
                        && same_executable(
                            std::path::Path::new(&registry.instance.fastkg.exe),
                            &identity.canonical_exe_path,
                        )
                })
                .unwrap_or(false);
            if registry_matches {
                return Ok("FastKG 已在运行".into());
            }
        }
    }

    let initial_instance = {
        let pids = kg_pids.read().await.clone();
        kg_coordination::local_ready_instance(&pids)
    };
    let claim = kg_coordination::begin_fastkg_maintenance(Some(initial_instance))
        .map_err(coordination_err)?
        .ok_or_else(|| AppError::Business("无法取得 FastKG 单服务维护令牌".to_string()))?;

    if let Some(identity) = existing {
        if existing_healthy {
            let process = match shared_service_process_from_identity(
                &identity,
                format!("http://127.0.0.1:{}", identity.effective_port),
            ) {
                Ok(process) => process,
                Err(error) => {
                    let _ = kg_coordination::mark_fastkg_stopped(&claim);
                    return Err(error);
                }
            };
            let registry =
                kg_coordination::publish_fastkg_ready(&claim, process).map_err(coordination_err)?;
            start_kg_lease_heartbeat();
            emit_kg_status(
                app_handle,
                "all_ready",
                kg_coordination::status_from_registry(Some(&registry)),
            );
            return Ok("FastKG 已在运行，共享注册表已修复".into());
        }

        tracing::warn!(
            "[knowledge] FastKG PID={} 身份有效但健康检查失败，进入受控重启",
            identity.pid
        );
        dependency_service_supervisor()
            .mark_stopping("fastkg")
            .await;
        if !kill_pid_sync(identity.pid) {
            let _ = kg_coordination::restore_fastkg_ready(&claim);
            restore_owned_dependency_runtime("fastkg", &identity).await;
            return Err(AppError::Business(format!(
                "无法停止失去健康状态的 FastKG PID={}，已恢复共享状态",
                identity.pid
            )));
        }
        dependency_service_supervisor().clear("fastkg").await;
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    }

    if !kg_coordination::mark_fastkg_stopped(&claim).map_err(coordination_err)? {
        return Err(AppError::Business(
            "FastKG 维护期间共享所有权发生变化，已取消启动".to_string(),
        ));
    }
    kg_pids.write().await.fastkg = None;
    let preferred = dependency_preferred_port("fastkg");
    let (port, attempt, mut reservation) =
        match reserve_available_dependency_port(app_handle, "fastkg", preferred).await {
            Ok(value) => value,
            Err(error) => {
                let _ = kg_coordination::mark_fastkg_stopped(&claim);
                return Err(error);
            }
        };
    let fastkg_path = paths::fastkg_server_path();
    let work_dir = fastkg_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    let qdrant_port =
        dependency_runtime_port("qdrant").unwrap_or_else(|| dependency_preferred_port("qdrant"));
    let llama_port = dependency_runtime_port("llama_server")
        .unwrap_or_else(|| dependency_preferred_port("llama_server"));
    emit_dependency_port_state(
        app_handle,
        "fastkg",
        "configuring",
        preferred,
        Some(port),
        attempt,
        "正在同步 FastKG 服务及依赖端点",
    );
    let knowledge_config = app_handle
        .state::<AppState>()
        .app_config
        .read()
        .await
        .knowledge
        .clone();
    if let Err(error) = update_fastkg_runtime_config(
        &work_dir.join("config.toml"),
        port,
        qdrant_port,
        llama_port,
        &knowledge_config,
    )
    {
        let _ = kg_coordination::mark_fastkg_stopped(&claim);
        return Err(error);
    }
    emit_dependency_port_state(
        app_handle,
        "fastkg",
        "starting",
        preferred,
        Some(port),
        attempt,
        format!("正在端口 {} 启动 FastKG", port),
    );
    dependency_service_supervisor()
        .mark_starting("fastkg", preferred, port)
        .await;
    let environment = crate::core::dependency_services::environment::snapshot(app_handle).await?;
    let env_vars = environment.iter().map(|(key, value)| (key.as_str(), value.clone())).collect::<Vec<_>>();
    match start_service_with_env(&fastkg_path, &work_dir, &env_vars) {
        Ok(pid) => {
            kg_pids.write().await.fastkg = Some(pid);
            if let Err(error) =
                record_starting_dependency_process("fastkg", preferred, port, pid, &fastkg_path)
                    .await
            {
                let _ = kill_pid_sync(pid);
                clear_dependency_process_tracking("fastkg", kg_pids).await;
                dependency_service_supervisor()
                    .mark_failed("fastkg", error.to_string())
                    .await;
                let _ = kg_coordination::mark_fastkg_stopped(&claim);
                return Err(error);
            }
            if !wait_for_fastkg_ready_cancellable(port, pid).await {
                let cancelled =
                    FASTKG_STOP_REQUESTED.load(std::sync::atomic::Ordering::SeqCst);
                let message = format!("FastKG 未在端口 {} 就绪", port);
                let cleanup =
                    cleanup_tracked_dependency_start("fastkg", kg_pids, &message).await;
                let _ = kg_coordination::mark_fastkg_stopped(&claim);
                cleanup?;
                if cancelled {
                    return Ok("FastKG 启动已取消".into());
                }
                return Err(AppError::Business(format!("FastKG 未在端口 {} 就绪", port)));
            }
            let identity =
                match record_owned_dependency_process("fastkg", preferred, port, pid, &fastkg_path)
                    .await
                {
                    Ok(identity) => identity,
                    Err(error) => {
                        let _ = kill_pid_sync(pid);
                        kg_pids.write().await.fastkg = None;
                        set_dependency_runtime_port("fastkg", None);
                        dependency_service_supervisor().clear("fastkg").await;
                        let _ = kg_coordination::mark_fastkg_stopped(&claim);
                        return Err(error);
                    }
                };
            let process = match shared_service_process_from_identity(
                &identity,
                format!("http://127.0.0.1:{}", port),
            ) {
                Ok(process) => process,
                Err(error) => {
                    let _ = kill_pid_sync(pid);
                    kg_pids.write().await.fastkg = None;
                    set_dependency_runtime_port("fastkg", None);
                    dependency_service_supervisor().clear("fastkg").await;
                    let _ = kg_coordination::mark_fastkg_stopped(&claim);
                    return Err(error);
                }
            };
            let registry = match kg_coordination::publish_fastkg_ready(&claim, process) {
                Ok(registry) => registry,
                Err(error) => {
                    let _ = kill_pid_sync(pid);
                    kg_pids.write().await.fastkg = None;
                    set_dependency_runtime_port("fastkg", None);
                    dependency_service_supervisor().clear("fastkg").await;
                    let _ = kg_coordination::mark_fastkg_stopped(&claim);
                    return Err(coordination_err(error));
                }
            };
            emit_dependency_port_state(
                app_handle,
                "fastkg",
                "ready",
                preferred,
                Some(port),
                attempt,
                format!("FastKG 已在端口 {} 就绪", port),
            );
            reservation.commit();
            start_kg_lease_heartbeat();
            emit_kg_status(
                app_handle,
                "all_ready",
                kg_coordination::status_from_registry(Some(&registry)),
            );
            Ok(format!("FastKG 已启动 (PID={}, 端口={})", pid, port))
        }
        Err(e) => {
            let _ = kg_coordination::mark_fastkg_stopped(&claim);
            Err(e)
        }
    }
}

/// 批量停止先撤销所有运行意图，再持有操作锁清理；任一组失败不阻断其他组。
#[tauri::command]
pub async fn stop_all_dependency_services(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<String, AppError> {
    for id in crate::core::dependency_services::recovery::SERVICE_IDS {
        state.dependency_recovery.request_stop(&app_handle, id)?;
    }
    let _operation = state.dependency_recovery.operation.lock().await;
    let results = [
        ("KG 服务组", stop_kg_inner(&state.kg_pids).await),
        ("FastPlan", stop_fastplan_inner(&state.kg_pids).await),
    ];
    let mut messages = Vec::new();
    let mut failures = Vec::new();
    for (name, result) in results {
        match result {
            Ok(message) => messages.push(format!("{}：{}", name, message)),
            Err(error) => failures.push(format!("{}：{}", name, error)),
        }
    }
    if failures.is_empty() {
        Ok(messages.join("；"))
    } else {
        Err(AppError::Business(format!("已取消全部自动恢复任务，部分服务停止失败：{}。其他结果：{}", failures.join("；"), messages.join("；"))))
    }
}

/// 停止单个依赖服务（含消费者保护）
#[tauri::command]
pub async fn stop_dependency_service(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, crate::state::AppState>,
    service_id: String,
) -> Result<String, AppError> {
    state.dependency_recovery.request_stop(&app_handle, &service_id)?;
    let _operation = state.dependency_recovery.operation.lock().await;
    if matches!(service_id.as_str(), "qdrant" | "llama_server" | "fastkg") {
        if let Some(registry) = kg_coordination::read_registry().map_err(coordination_err)? {
            if !kg_coordination::current_fastclaw_is_owner(&registry) {
                for id in ["fastkg", "qdrant", "llama_server"] { state.dependency_recovery.request_stop(&app_handle, id)?; }
                kg_coordination::release_own_lease_only().map_err(coordination_err)?;
                stop_kg_lease_heartbeat().await;
                clear_local_kg_tracking(&state.kg_pids).await;
                return Ok("已退出 KG 共享使用，未终止共享服务".into());
            }
        }
    }
    if matches!(service_id.as_str(), "qdrant" | "llama_server")
        && verified_tracked_dependency_process("fastkg").await.is_some()
    { stop_single_fastkg(&state.kg_pids).await?; }
    if dependency_service_supervisor().snapshot(&service_id).await.and_then(|s| s.process).is_none() {
        return Ok("当前应用的服务已停止或启动已取消，未终止外部进程".into());
    }
    stop_dependency_once(app_handle.state::<AppState>(), service_id).await
}

async fn stop_dependency_once(
    state: tauri::State<'_, crate::state::AppState>,
    service_id: String,
) -> Result<String, AppError> {
    match service_id.as_str() {
        "qdrant" => {
            // 检查 FastKG 是否正在运行（它依赖 Qdrant）
            if dependency_service_is_usable("fastkg").await {
                return Err(AppError::Business(
                    "FastKG 正在运行，请先停止知识库服务再停止向量库服务".into(),
                ));
            }
            let port = dependency_runtime_port("qdrant")
                .unwrap_or_else(|| dependency_preferred_port("qdrant"));
            QDRANT_STOP_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);
            let result = stop_single_by_port(port, "qdrant", &state.kg_pids).await;
            if result.is_ok() {
                set_dependency_runtime_port("qdrant", None);
            }
            result
        }
        "llama_server" => {
            if dependency_service_is_usable("fastkg").await {
                return Err(AppError::Business(
                    "FastKG 正在运行，请先停止知识库服务再停止嵌入向量服务".into(),
                ));
            }
            let port = dependency_runtime_port("llama_server")
                .unwrap_or_else(|| dependency_preferred_port("llama_server"));
            let result = stop_single_by_port(port, "llama_server", &state.kg_pids).await;
            if result.is_ok() {
                set_dependency_runtime_port("llama_server", None);
            }
            result
        }
        "fastkg" => {
            let was_starting = dependency_service_supervisor()
                .snapshot("fastkg")
                .await
                .is_some_and(|runtime| runtime.lifecycle == LifecycleState::Starting);
            FASTKG_STOP_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);
            let _maintenance_guard = FASTKG_MAINTENANCE_LOCK.lock().await;
            if was_starting
                && dependency_service_supervisor()
                    .snapshot("fastkg")
                    .await
                    .is_none()
            {
                Ok("FastKG 启动已取消".into())
            } else {
                stop_single_fastkg(&state.kg_pids).await
            }
        }
        "fastplan" => stop_fastplan_inner(&state.kg_pids).await,
        _ => Err(AppError::Business(format!(
            "未知的服务类型: {}",
            service_id
        ))),
    }
}

/// 停止 FastKG 单服务，并将共享实例同步切换到 `starting`。
/// Qdrant 和 embedding 保持运行，以便后续手动恢复 FastKG。
async fn stop_single_fastkg(
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
) -> Result<String, AppError> {
    let runtime = dependency_service_supervisor().snapshot("fastkg").await;
    let identity = if runtime.as_ref().is_some_and(|runtime| {
        matches!(
            runtime.lifecycle,
            LifecycleState::Starting | LifecycleState::Unhealthy | LifecycleState::Stopping | LifecycleState::Failed
        )
    }) {
        verified_tracked_dependency_process("fastkg").await
    } else {
        verified_owned_dependency_process("fastkg").await
    };
    let Some(identity) = identity else {
        kg_pids.write().await.fastkg = None;
        return Err(AppError::Business(
            "FastKG 不是由当前 FastClaw 管理的进程，已拒绝停止".to_string(),
        ));
    };

    let claim = kg_coordination::begin_fastkg_maintenance(None).map_err(coordination_err)?;
    let registered_process = shared_service_process_from_identity(
        &identity,
        format!("http://127.0.0.1:{}", identity.effective_port),
    )?;
    let supervisor = dependency_service_supervisor();
    supervisor.mark_stopping("fastkg").await;
    if !kill_pid_sync(identity.pid) {
        if let Some(claim) = claim.as_ref() {
            let _ = kg_coordination::restore_fastkg_ready(claim);
        }
        restore_owned_dependency_runtime("fastkg", &identity).await;
        return Err(AppError::Business(format!(
            "无法停止受管理的 FastKG 进程 PID={}，共享状态已恢复",
            identity.pid
        )));
    }
    if !wait_for_registered_process_identity_to_disappear(&registered_process, 30).await {
        if let Some(claim) = claim.as_ref() {
            let _ = kg_coordination::restore_fastkg_ready(claim);
        }
        restore_owned_dependency_runtime("fastkg", &identity).await;
        return Err(AppError::Business(format!(
            "等待 FastKG 进程 PID={} 退出超时，已保留共享记录",
            identity.pid
        )));
    }

    if let Some(claim) = claim.as_ref() {
        if !kg_coordination::mark_fastkg_stopped(claim).map_err(coordination_err)? {
            supervisor.clear("fastkg").await;
            kg_pids.write().await.fastkg = None;
            set_dependency_runtime_port("fastkg", None);
            return Err(AppError::Business(
                "FastKG 已停止，但共享注册表在操作期间发生变化，已拒绝覆盖新状态".to_string(),
            ));
        }
    }

    supervisor.clear("fastkg").await;
    kg_pids.write().await.fastkg = None;
    set_dependency_runtime_port("fastkg", None);
    Ok(format!(
        "FastKG 已停止 (PID={})；Qdrant 和嵌入服务保持运行",
        identity.pid
    ))
}

/// 只停止由当前 FastClaw 监督器确认所有权的单个服务。
/// 端口仅用于核对监听归属，绝不作为终止授权来源。
async fn stop_single_by_port(
    _port: u16,
    name: &str,
    kg_pids: &Arc<RwLock<crate::state::KgServicePids>>,
) -> Result<String, AppError> {
    let supervisor = dependency_service_supervisor();
    let runtime = supervisor.snapshot(name).await;
    let starting = runtime
        .as_ref()
        .is_some_and(|runtime| runtime.lifecycle == LifecycleState::Starting);
    let identity = if runtime.as_ref().is_some_and(|runtime| {
            matches!(
                runtime.lifecycle,
                LifecycleState::Starting | LifecycleState::Unhealthy | LifecycleState::Stopping | LifecycleState::Failed
            )
        })
    {
        verified_tracked_dependency_process(name).await
    } else {
        verified_owned_dependency_process(name).await
    };
    let Some(identity) = identity else {
        let mut guard = kg_pids.write().await;
        match name {
            "qdrant" => guard.qdrant = None,
            "fastkg" => guard.fastkg = None,
            "llama_server" => guard.llama_server = None,
            _ => {}
        }
        drop(guard);
        if name == "qdrant" && starting {
            supervisor.clear(name).await;
            set_dependency_runtime_port(name, None);
            return Ok("Qdrant 启动已取消".into());
        }
        return Err(AppError::Business(format!(
            "{} 不是由当前 FastClaw 管理的进程，已拒绝停止；端口占用者不会受到影响",
            name
        )));
    };

    supervisor.mark_stopping(name).await;
    if !kill_pid_sync(identity.pid) {
        supervisor
            .mark_unhealthy(
                name,
                format!("无法停止受管理的 {} 进程 PID={}", name, identity.pid),
            )
            .await;
        return Err(AppError::Business(format!(
            "无法停止受管理的 {} 进程 PID={}，运行记录已保留",
            name, identity.pid
        )));
    }
    supervisor.clear(name).await;

    // 清除 PID 记录
    let mut guard = kg_pids.write().await;
    match name {
        "qdrant" => guard.qdrant = None,
        "fastkg" => guard.fastkg = None,
        "llama_server" => guard.llama_server = None,
        _ => {}
    }

    Ok(format!("{} 已停止 (PID={})", name, identity.pid))
}

/// 更新依赖服务的 IP/端口配置，若正在运行则自动重启
#[tauri::command]
pub async fn update_dependency_service_config(
    state: tauri::State<'_, crate::state::AppState>,
    app_handle: tauri::AppHandle,
    service_id: String,
    host: String,
    port: u16,
) -> Result<String, AppError> {
    // 构建新的 URL
    let new_url = format!("http://{}:{}", host.trim(), port);

    // 映射 service_id → config.json dot-path
    let dot_path = match service_id.as_str() {
        "qdrant" => "services.qdrant",
        "fastkg" => "services.fastkg",
        "llama_server" => "services.llama_server",
        "fastplan" => "services.fastplan",
        _ => {
            return Err(AppError::Business(format!(
                "未知的服务类型: {}",
                service_id
            )))
        }
    };

    // 检查是否需要重启（服务正在运行且端口或 host 变了）
    let config = state.app_config.read().await;
    let old_url = match service_id.as_str() {
        "qdrant" => &config.services.qdrant,
        "fastkg" => &config.services.fastkg,
        "llama_server" => &config.services.llama_server,
        "fastplan" => &config.services.fastplan,
        _ => "",
    };
    let needs_restart = *old_url != new_url;
    let old_port = parse_url_port(
        old_url,
        match service_id.as_str() {
            "qdrant" => paths::qdrant_default_port(),
            "fastkg" => paths::fastkg_default_port(),
            "llama_server" => paths::llama_server_default_port(),
            "fastplan" => paths::fastplan_default_port(),
            _ => 0,
        },
    );
    drop(config);

    // 必须在切换首选端口之前判断旧实例，否则运行端口尚未登记时会误查新端口。
    if state.dependency_recovery.kg_installing() && matches!(service_id.as_str(), "qdrant" | "llama_server" | "fastkg") {
        return Err(AppError::Business("KG 正在安装，请完成后修改配置".into()));
    }
    if needs_restart && matches!(service_id.as_str(), "qdrant" | "llama_server" | "fastkg") {
        if let Some(registry) = kg_coordination::read_registry().map_err(coordination_err)? {
            if !kg_coordination::current_fastclaw_is_owner(&registry)
                || registry.leases.iter().any(|lease| lease.app_pid != std::process::id()) {
                return Err(AppError::Business("共享服务仍有其他消费者，不能通过配置修改重启健康实例".into()));
            }
        }
    }
    let affected = if matches!(service_id.as_str(), "qdrant" | "llama_server" | "fastkg") {
        vec!["qdrant", "llama_server", "fastkg"]
    } else { vec![service_id.as_str()] };
    let restore = state.dependency_recovery.snapshots().into_iter()
        .filter(|status| needs_restart && affected.contains(&status.id.as_str()) && status.desired_state == crate::core::dependency_services::recovery::DesiredState::Running)
        .map(|status| status.id).collect::<Vec<_>>();
    for id in &restore { state.dependency_recovery.pause(&app_handle, id)?; }
    let restore_generations = state.dependency_recovery.snapshots().into_iter()
        .filter(|status| restore.contains(&status.id)).map(|status| (status.id, status.generation)).collect::<Vec<_>>();
    let _operation = state.dependency_recovery.operation.lock().await;
    let was_running = if needs_restart {
        let effective_old_port = dependency_runtime_port(&service_id).unwrap_or(old_port);
        match service_id.as_str() {
            "qdrant" | "fastkg" | "llama_server" | "fastplan" => {
                let _ = effective_old_port;
                verified_owned_dependency_process(&service_id)
                    .await
                    .is_some()
            }
            _ => false,
        }
    } else {
        false
    };
    if was_running && dependency_runtime_port(&service_id).is_none() {
        set_dependency_runtime_port(&service_id, Some(old_port));
    }

    // 更新 config.json
    state
        .app_config
        .update_field(dot_path, serde_json::Value::String(new_url.clone()))
        .await
        .map_err(|e| AppError::Business(format!("配置更新失败: {}", e)))?;
    configure_dependency_preferred_url(&service_id, &new_url);

    if needs_restart {
        if was_running {
            // 停止服务
            match service_id.as_str() {
                "qdrant" | "fastkg" | "llama_server" => {
                    stop_kg_inner(&state.kg_pids).await?;
                }
                "fastplan" => {
                    stop_fastplan_inner(&state.kg_pids).await?;
                }
                _ => {}
            }

        }
    }
    for (id, generation) in restore_generations {
        state.dependency_recovery.resume_generation(&app_handle, &id, generation);
    }

    Ok(format!("{} 配置已更新", service_id))
}

#[cfg(test)]
mod dependency_port_tests {
    use super::{
        assess_dependency_service, dependency_port_is_available, get_listening_port_pid_map,
        parse_service_url, registered_process_is_valid_for_shutdown, update_fastkg_runtime_config,
    };
    use crate::core::dependency_services::{
        LifecycleState, ProcessFacts, ProcessIdentity, RuntimeOwnership, ServiceRuntime,
    };
    use crate::kg_coordination::SharedOwnership;
    use std::collections::HashMap;

    #[test]
    fn dependency_health_probe_accepts_verified_shared_port_without_local_installation() {
        let exe = std::path::PathBuf::from(r"C:\SharedKG\qdrant.exe");
        let process = serde_json::json!({
            "pid": 9010, "exe": exe, "url": "http://127.0.0.1:23061",
            "process_started_at": "1970-01-01T00:00:20Z"
        });
        let instance: crate::kg_coordination::SharedKgInstance = serde_json::from_value(serde_json::json!({
            "instance_id": "test", "starter_app": "peer", "service_dir": r"C:\SharedKG",
            "fastkg": process.clone(), "qdrant": process.clone(), "llama_server": process
        })).unwrap();
        let listeners = HashMap::from([(23061, 9010)]);
        let processes = HashMap::from([(9010, ProcessFacts {
            process_started_at: 20, canonical_exe_path: exe,
        })]);
        let local_exe = std::path::Path::new(r"C:\NotInstalled\qdrant.exe");
        assert_eq!(super::dependency_health_probe_port(None, local_exe,
            Some((&instance.qdrant, &instance)), &listeners, &processes), Some(23061));
        assert_eq!(super::dependency_health_probe_port(None, local_exe,
            Some((&instance.qdrant, &instance)), &HashMap::from([(23061, 9011)]), &processes), None);
    }

    #[test]
    fn dependency_port_search_is_bounded_and_never_wraps() {
        assert_eq!(super::dependency_port_candidates(23060).count(), 40);
        assert_eq!(super::dependency_port_candidates(23060).last(), Some(23099));
        assert_eq!(super::dependency_port_candidates(65530).collect::<Vec<_>>(),
            vec![65530, 65531, 65532, 65533, 65534, 65535]);
        assert_eq!(super::dependency_port_candidates(0).next(), Some(1));
    }

    #[test]
    fn parses_configured_service_url_with_default_fallback() {
        assert_eq!(
            parse_service_url("http://localhost:6337/path", "127.0.0.1", 6333),
            ("localhost".to_string(), 6337)
        );
        assert_eq!(
            parse_service_url("not-a-url", "127.0.0.1", 6333),
            ("127.0.0.1".to_string(), 6333)
        );
    }

    #[test]
    fn rewrites_fastkg_port_and_dependency_urls_without_losing_other_config() {
        let directory = tempfile::tempdir().expect("create temp directory");
        let config_path = directory.path().join("config.toml");
        std::fs::write(
            &config_path,
            r#"[server]
port = 3000
workers = 4

[qdrant]
url = "http://127.0.0.1:6333"

[embedding]
url = "http://127.0.0.1:8089/v1/embeddings"
model = "bge-m3"
"#,
        )
        .expect("write original config");

        update_fastkg_runtime_config(
            &config_path,
            3002,
            6334,
            8091,
            &crate::core::config::KnowledgeServicesConfig::default(),
        )
            .expect("rewrite FastKG runtime config");

        let updated = std::fs::read_to_string(config_path).expect("read updated config");
        let document = updated
            .parse::<toml_edit::DocumentMut>()
            .expect("parse updated config");
        assert_eq!(document["server"]["port"].as_integer(), Some(3002));
        assert_eq!(document["server"]["workers"].as_integer(), Some(4));
        assert_eq!(
            document["qdrant"]["url"].as_str(),
            Some("http://127.0.0.1:6334")
        );
        assert_eq!(
            document["embedding"]["url"].as_str(),
            Some("http://127.0.0.1:8091/v1/embeddings")
        );
        assert_eq!(document["embedding"]["model"].as_str(), Some("bge-m3"));
    }

    #[test]
    fn applies_external_vector_embedding_and_reranker_endpoints() {
        let directory = tempfile::tempdir().expect("create temp directory");
        let config_path = directory.path().join("config.toml");
        std::fs::write(
            &config_path,
            "[server]\nport=3000\n[qdrant]\nurl=''\n[embedding]\nurl=''\n[reranker]\nenabled=false\n",
        )
        .expect("write original config");
        let external = crate::core::config::KnowledgeServicesConfig {
            vector_backend: "milvus".into(),
            vector_url: "http://vector.example:19530/".into(),
            embedding_url: "https://embedding.example/v1/embeddings".into(),
            reranker_url: "https://reranker.example/v1/rerank".into(),
            ..Default::default()
        };

        update_fastkg_runtime_config(&config_path, 3002, 6334, 8091, &external)
            .expect("rewrite external FastKG runtime config");

        let updated = std::fs::read_to_string(config_path).expect("read updated config");
        let document = updated
            .parse::<toml_edit::DocumentMut>()
            .expect("parse updated config");
        assert_eq!(document["qdrant"]["backend"].as_str(), Some("milvus"));
        assert_eq!(
            document["qdrant"]["url"].as_str(),
            Some("http://vector.example:19530")
        );
        assert_eq!(
            document["embedding"]["url"].as_str(),
            Some("https://embedding.example/v1/embeddings")
        );
        assert_eq!(document["reranker"]["enabled"].as_bool(), Some(true));
        assert_eq!(
            document["reranker"]["url"].as_str(),
            Some("https://reranker.example/v1/rerank")
        );
    }

    #[test]
    fn external_listener_is_a_startable_conflict_not_a_running_service() {
        let expected_exe = std::path::PathBuf::from(r"C:\FastClaw\qdrant.exe");
        let listeners = HashMap::from([(6333, 9001)]);
        let processes = HashMap::from([(
            9001,
            ProcessFacts {
                process_started_at: 10,
                canonical_exe_path: std::path::PathBuf::from(r"C:\Other\server.exe"),
            },
        )]);
        assert_eq!(super::dependency_health_probe_port(None, &expected_exe, None, &listeners, &processes), None);
        assert_eq!(super::dependency_health_probe_port(None, &expected_exe, None, &HashMap::new(), &HashMap::new()), None);
        let assessment = assess_dependency_service(
            "向量库服务",
            6333,
            6333,
            &expected_exe,
            None,
            false,
            None,
            false,
            &listeners,
            &processes,
        );
        assert_eq!(assessment.lifecycle, LifecycleState::PortOccupied);
        assert_eq!(assessment.ownership, SharedOwnership::ExternalUnmanaged);
        assert!(!assessment.running);
        assert!(assessment.can_start);
        assert!(!assessment.can_stop);
        assert_eq!(assessment.occupied_by_pid, Some(9001));
    }

    #[cfg(windows)]
    #[test]
    fn real_local_listener_is_reported_as_external_and_never_stoppable() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind test port");
        let port = listener.local_addr().expect("listener address").port();
        let listeners = get_listening_port_pid_map();
        let pid = listeners.get(&port).copied().expect("listener PID");
        let processes = crate::core::dependency_services::capture_process_facts(&[pid]);
        let assessment = assess_dependency_service(
            "向量库服务",
            port,
            port,
            &std::path::PathBuf::from(r"C:\FastClaw\qdrant.exe"),
            None,
            false,
            None,
            false,
            &listeners,
            &processes,
        );
        assert_eq!(assessment.lifecycle, LifecycleState::PortOccupied);
        assert_eq!(assessment.occupied_by_pid, Some(std::process::id()));
        assert!(!assessment.running);
        assert!(assessment.can_start);
        assert!(!assessment.can_stop);
        drop(listener);
    }

    #[cfg(windows)]
    #[test]
    fn real_local_listener_is_rejected_by_dependency_port_allocator() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind test port");
        let port = listener.local_addr().expect("listener address").port();
        let listeners = get_listening_port_pid_map();

        assert_eq!(listeners.get(&port), Some(&std::process::id()));
        assert!(!dependency_port_is_available(
            port,
            &listeners,
            &std::collections::HashSet::new()
        ));

        drop(listener);
    }

    #[test]
    fn dependency_port_allocator_rejects_snapshot_and_internal_reservations() {
        let listeners = HashMap::from([(6333, 9001)]);
        assert!(!dependency_port_is_available(
            6333,
            &listeners,
            &std::collections::HashSet::new()
        ));

        let reserved = std::collections::HashSet::from([6334]);
        assert!(!dependency_port_is_available(
            6334,
            &HashMap::new(),
            &reserved
        ));
    }

    #[test]
    fn adoption_shutdown_requires_exact_service_fingerprint_and_listener() {
        let directory = tempfile::tempdir().expect("create service directory");
        let exe = directory.path().join("FastKG.exe");
        std::fs::write(&exe, b"test executable").expect("create executable fixture");
        let process = crate::kg_coordination::SharedServiceProcess {
            url: "http://127.0.0.1:3000".to_string(),
            pid: 9100,
            exe: exe.to_string_lossy().to_string(),
            process_started_at: chrono::DateTime::from_timestamp(500, 0),
            extra: serde_json::Map::new(),
        };
        let listeners = HashMap::from([(3000, 9100)]);
        let matching_facts = HashMap::from([(
            9100,
            ProcessFacts {
                process_started_at: 500,
                canonical_exe_path: exe.clone(),
            },
        )]);

        assert!(registered_process_is_valid_for_shutdown(
            &process,
            directory.path(),
            "FastKG.exe",
            Some(&listeners),
            &matching_facts
        ));

        let reused_pid = HashMap::from([(
            9100,
            ProcessFacts {
                process_started_at: 501,
                canonical_exe_path: exe.clone(),
            },
        )]);
        assert!(!registered_process_is_valid_for_shutdown(
            &process,
            directory.path(),
            "FastKG.exe",
            Some(&listeners),
            &reused_pid
        ));
        assert!(!registered_process_is_valid_for_shutdown(
            &process,
            directory.path(),
            "FastKG.exe",
            Some(&HashMap::from([(3000, 9101)])),
            &matching_facts
        ));
    }

    #[test]
    fn adoption_shutdown_rejects_legacy_process_without_start_time() {
        let directory = tempfile::tempdir().expect("create service directory");
        let exe = directory.path().join("qdrant.exe");
        std::fs::write(&exe, b"test executable").expect("create executable fixture");
        let process = crate::kg_coordination::SharedServiceProcess {
            url: "http://127.0.0.1:6333".to_string(),
            pid: 9200,
            exe: exe.to_string_lossy().to_string(),
            process_started_at: None,
            extra: serde_json::Map::new(),
        };
        let facts = HashMap::from([(
            9200,
            ProcessFacts {
                process_started_at: 600,
                canonical_exe_path: exe,
            },
        )]);

        assert!(!registered_process_is_valid_for_shutdown(
            &process,
            directory.path(),
            "qdrant.exe",
            Some(&HashMap::from([(6333, 9200)])),
            &facts
        ));
    }

    #[test]
    fn self_managed_runtime_requires_the_recorded_listener_identity() {
        let expected_exe = std::path::PathBuf::from(r"C:\FastClaw\qdrant.exe");
        let runtime = ServiceRuntime {
            lifecycle: LifecycleState::Running,
            ownership: RuntimeOwnership::SelfManaged,
            preferred_port: 6333,
            effective_port: Some(6334),
            process: Some(ProcessIdentity {
                pid: 9002,
                process_started_at: 20,
                canonical_exe_path: expected_exe.clone(),
                effective_port: 6334,
            }),
            last_error: None,
            lifecycle_updated_at: std::time::Instant::now(),
        };
        let processes = HashMap::from([(
            9002,
            ProcessFacts {
                process_started_at: 20,
                canonical_exe_path: expected_exe.clone(),
            },
        )]);
        assert_eq!(super::dependency_health_probe_port(Some(&runtime), &expected_exe, None,
            &HashMap::from([(6334, 9002)]), &processes), Some(6334));
        assert_eq!(super::dependency_health_probe_port(Some(&runtime), &expected_exe, None,
            &HashMap::from([(6334, 9003)]), &processes), None);
        assert_eq!(super::dependency_health_probe_port(Some(&runtime), &expected_exe, None,
            &HashMap::new(), &processes), None);
        let reused_pid = HashMap::from([(9002, ProcessFacts { process_started_at: 21, canonical_exe_path: expected_exe.clone() })]);
        assert_eq!(super::dependency_health_probe_port(Some(&runtime), &expected_exe, None,
            &HashMap::from([(6334, 9002)]), &reused_pid), None);
        let valid = assess_dependency_service(
            "向量库服务",
            6333,
            6334,
            &expected_exe,
            Some(&runtime),
            true,
            None,
            false,
            &HashMap::from([(6334, 9002)]),
            &processes,
        );
        assert!(valid.running);
        assert!(valid.can_stop);
        assert_eq!(valid.effective_port, Some(6334));

        let unhealthy = assess_dependency_service(
            "向量库服务",
            6333,
            6334,
            &expected_exe,
            Some(&runtime),
            false,
            None,
            false,
            &HashMap::from([(6334, 9002)]),
            &processes,
        );
        assert_eq!(unhealthy.lifecycle, LifecycleState::Unhealthy);
        assert!(!unhealthy.running);
        assert!(unhealthy.can_stop);
        assert!(!unhealthy.can_start);

        let failed = ServiceRuntime { lifecycle: LifecycleState::Failed, ..runtime.clone() };
        let failed_without_listener = assess_dependency_service(
            "向量库服务", 6333, 6334, &expected_exe, Some(&failed), false,
            None, false, &HashMap::new(), &processes,
        );
        assert_eq!(failed_without_listener.lifecycle, LifecycleState::Unhealthy);
        assert!(failed_without_listener.can_stop);
        assert!(!failed_without_listener.can_start);
        assert_eq!(failed_without_listener.managed_pid, Some(9002));

        let replaced = assess_dependency_service(
            "向量库服务",
            6333,
            6334,
            &expected_exe,
            Some(&runtime),
            true,
            None,
            false,
            &HashMap::from([(6334, 9003)]),
            &processes,
        );
        assert_eq!(replaced.lifecycle, LifecycleState::PortOccupied);
        assert!(!replaced.running);
        assert!(!replaced.can_stop);
        assert_eq!(replaced.occupied_by_pid, Some(9003));
    }

    #[test]
    fn starting_service_exposes_verified_pid_and_stop_action_before_listening() {
        let expected_exe = std::env::current_exe().expect("current exe");
        let runtime = ServiceRuntime {
            lifecycle: LifecycleState::Starting,
            ownership: RuntimeOwnership::SelfManaged,
            preferred_port: 23180,
            effective_port: Some(23180),
            process: Some(ProcessIdentity {
                pid: 9300,
                process_started_at: 700,
                canonical_exe_path: expected_exe.clone(),
                effective_port: 23180,
            }),
            last_error: None,
            lifecycle_updated_at: std::time::Instant::now(),
        };
        let processes = HashMap::from([(
            9300,
            ProcessFacts {
                process_started_at: 700,
                canonical_exe_path: expected_exe.clone(),
            },
        )]);

        let assessment = assess_dependency_service(
            "FastPlan服务",
            23180,
            23180,
            &expected_exe,
            Some(&runtime),
            false,
            None,
            false,
            &HashMap::new(),
            &processes,
        );

        assert_eq!(assessment.lifecycle, LifecycleState::Starting);
        assert_eq!(assessment.managed_pid, Some(9300));
        assert!(!assessment.can_start);
        assert!(assessment.can_stop);
    }

    #[test]
    fn starting_service_rejects_reused_pid_for_stop_action() {
        let expected_exe = std::env::current_exe().expect("current exe");
        let runtime = ServiceRuntime {
            lifecycle: LifecycleState::Starting,
            ownership: RuntimeOwnership::SelfManaged,
            preferred_port: 23180,
            effective_port: Some(23180),
            process: Some(ProcessIdentity {
                pid: 9300,
                process_started_at: 700,
                canonical_exe_path: expected_exe.clone(),
                effective_port: 23180,
            }),
            last_error: None,
            lifecycle_updated_at: std::time::Instant::now(),
        };
        let processes = HashMap::from([(
            9300,
            ProcessFacts {
                process_started_at: 701,
                canonical_exe_path: expected_exe.clone(),
            },
        )]);

        let assessment = assess_dependency_service(
            "FastPlan服务",
            23180,
            23180,
            &expected_exe,
            Some(&runtime),
            false,
            None,
            false,
            &HashMap::new(),
            &processes,
        );

        assert_eq!(assessment.lifecycle, LifecycleState::Starting);
        assert_eq!(assessment.managed_pid, None);
        assert!(!assessment.can_stop);
    }
}
