// FastClaw · 系统信息与导入导出 Tauri Commands
use crate::core::error::AppError;
use crate::state::AppState;
use crate::utils::paths;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path};

use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize)]
pub struct BrowseFilesystemResult {
    pub path: String,
    pub suggestions: Vec<BrowseFilesystemEntry>,
    pub drives: Vec<String>,
}

#[derive(Serialize)]
pub struct BrowseFilesystemEntry {
    pub path: String,
    pub name: String,
    #[serde(rename = "type")]
    pub entry_type: String,
}

/// Lists a directory for the reusable frontend filesystem browser.
#[tauri::command]
pub async fn browse_filesystem(path: Option<String>) -> Result<BrowseFilesystemResult, AppError> {
    let target = path.unwrap_or_else(|| {
        if cfg!(target_os = "windows") {
            "C:\\".into()
        } else {
            "/".into()
        }
    });
    let target_path = Path::new(&target);
    let directory = if target_path.is_dir() {
        target_path.to_path_buf()
    } else {
        target_path.parent().unwrap_or(Path::new("/")).to_path_buf()
    };

    let mut suggestions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&directory) {
        let mut items: Vec<_> = entries.filter_map(Result::ok).collect();
        items.sort_by(|left, right| {
            let left_is_dir = left.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
            let right_is_dir = right.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
            right_is_dir.cmp(&left_is_dir).then_with(|| {
                left.file_name()
                    .to_string_lossy()
                    .to_lowercase()
                    .cmp(&right.file_name().to_string_lossy().to_lowercase())
            })
        });
        for entry in items.iter().take(50) {
            let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
            suggestions.push(BrowseFilesystemEntry {
                path: entry.path().to_string_lossy().replace('\\', "/"),
                name: entry.file_name().to_string_lossy().to_string(),
                entry_type: if is_dir {
                    "directory".into()
                } else {
                    "file".into()
                },
            });
        }
    }

    let drives = if cfg!(target_os = "windows") {
        ('A'..='Z')
            .filter(|letter| Path::new(&format!("{}:\\", letter)).exists())
            .map(|letter| format!("{}:\\", letter))
            .collect()
    } else {
        vec!["/".into()]
    };

    Ok(BrowseFilesystemResult {
        path: directory.to_string_lossy().replace('\\', "/"),
        suggestions,
        drives,
    })
}

#[tauri::command]
pub async fn get_system_info(_state: State<'_, AppState>) -> Result<SystemInfo, AppError> {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();
    Ok(SystemInfo {
        os: sysinfo::System::name().unwrap_or_default(),
        os_version: sysinfo::System::os_version().unwrap_or_default(),
        total_memory_mb: sys.total_memory() / 1024 / 1024,
        used_memory_mb: sys.used_memory() / 1024 / 1024,
        cpu_count: sys.cpus().len() as u32,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[tauri::command]
pub async fn export_data(_state: State<'_, AppState>, _path: String) -> Result<(), AppError> {
    // TODO: 导出数据库 + 配置到 ZIP
    Ok(())
}

#[tauri::command]
pub async fn import_data(_state: State<'_, AppState>, _path: String) -> Result<(), AppError> {
    // TODO: 从 ZIP 导入
    Ok(())
}

#[tauri::command]
pub fn copy_file(from: String, to: String) -> Result<(), AppError> {
    std::fs::copy(&from, &to).map_err(|e| AppError::Io(e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_file(path: String) -> Result<(), AppError> {
    let p = std::path::Path::new(&path);
    if p.exists() {
        std::fs::remove_file(p).map_err(|e| AppError::Io(e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn open_directory(path: String) -> Result<(), AppError> {
    let path_buf = std::path::PathBuf::from(&path);
    if !path_buf.exists() {
        std::fs::create_dir_all(&path_buf)
            .map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, e)))?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, e)))?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_app_root_dir() -> Result<String, AppError> {
    Ok(paths::project_root().to_string_lossy().to_string())
}

#[tauri::command]
pub fn get_exe_dir() -> Result<String, AppError> {
    let exe = std::env::current_exe()
        .map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, e)))?;
    let dir = exe
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let data_dir = dir.join("data");
    if !data_dir.exists() {
        std::fs::create_dir_all(&data_dir)
            .map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, e)))?;
    }
    Ok(data_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub fn get_db_dir() -> Result<String, AppError> {
    use crate::db::Database;
    let db_path = Database::db_path_static();
    // 返回数据库文件所在的目录（FastClaw 目录）
    let dir = db_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, e)))?;
    }
    Ok(dir.to_string_lossy().to_string())
}

#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub os: String,
    pub os_version: String,
    pub total_memory_mb: u64,
    pub used_memory_mb: u64,
    pub cpu_count: u32,
    pub app_version: String,
}

// ─── 通用 HTTP 代理 Command（解决打包后跨域问题） ───────────────

#[derive(Debug, Deserialize)]
pub struct ProxyFormFile {
    field: String,
    filename: String,
    #[serde(default = "default_image_png")]
    content_type: String,
    data_base64: String,
}

fn default_image_png() -> String {
    "image/png".to_string()
}

#[derive(Debug, Deserialize)]
pub struct ProxyHttpRequest {
    url: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    body: Option<Value>,
    /// multipart 文本字段（与 body 互斥）
    #[serde(default)]
    form: Option<HashMap<String, String>>,
    /// multipart 文件字段（base64 编码）
    #[serde(default)]
    form_files: Option<Vec<ProxyFormFile>>,
}

pub fn default_method() -> String {
    "GET".to_string()
}

#[derive(Debug, Serialize)]
pub struct ProxyHttpResponse {
    status: u16,
    headers: HashMap<String, String>,
    body: Value,
    /// 二进制响应（如 audio/mp3）时返回 base64，避免 text 解码损坏
    #[serde(skip_serializing_if = "Option::is_none")]
    body_base64: Option<String>,
}

#[tauri::command]
pub async fn proxy_http_request(input: ProxyHttpRequest) -> Result<ProxyHttpResponse, AppError> {
    let client = reqwest::Client::new();
    let method =
        reqwest::Method::from_bytes(input.method.as_bytes()).unwrap_or(reqwest::Method::GET);

    let mut request = client.request(method, &input.url);

    for (key, value) in &input.headers {
        request = request.header(key.as_str(), value.as_str());
    }

    if let Some(files) = input.form_files {
        let mut multipart = reqwest::multipart::Form::new();
        if let Some(fields) = input.form {
            for (k, v) in fields {
                multipart = multipart.text(k, v);
            }
        }
        for f in files {
            let bytes = BASE64.decode(&f.data_base64).map_err(|e| {
                AppError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("invalid base64 in form file '{}': {e}", f.field),
                ))
            })?;
            let part = reqwest::multipart::Part::bytes(bytes)
                .file_name(f.filename)
                .mime_str(&f.content_type)
                .map_err(|e| {
                    AppError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("invalid mime type '{}': {e}", f.content_type),
                    ))
                })?;
            multipart = multipart.part(f.field, part);
        }
        request = request.multipart(multipart);
    } else if let Some(ref json_body) = input.body {
        request = request.json(json_body);
    }

    let response = request
        .send()
        .await
        .map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

    let status = response.status().as_u16();
    let resp_headers: HashMap<String, String> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let content_type = resp_headers
        .get("content-type")
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let is_binary = content_type.starts_with("audio/")
        || content_type.starts_with("image/")
        || content_type.starts_with("application/octet-stream")
        || content_type.starts_with("application/pdf")
        || content_type.starts_with("video/");

    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

    if is_binary {
        return Ok(ProxyHttpResponse {
            status,
            headers: resp_headers,
            body: Value::Null,
            body_base64: Some(BASE64.encode(&bytes)),
        });
    }

    let text = String::from_utf8_lossy(&bytes);
    let resp_body: Value = if text.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text).unwrap_or_else(|_| Value::String(text.into_owned()))
    };

    Ok(ProxyHttpResponse {
        status,
        headers: resp_headers,
        body: resp_body,
        body_base64: None,
    })
}

// ─── 数据备份功能 ───────────────

#[tauri::command]
pub fn backup_directory(source_path: String, backup_type: String) -> Result<String, AppError> {
    let source = Path::new(&source_path);

    tracing::info!(
        "[backup] 开始备份: path={}, type={}",
        source_path,
        backup_type
    );

    if !source.exists() {
        tracing::error!("[backup] 目录不存在: {}", source_path);
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Source directory does not exist",
        )));
    }

    // 检查目录是否有数据（文件总大小）
    let total_size = get_dir_size(source)?;
    tracing::info!("[backup] 目录总大小: {} bytes", total_size);

    if total_size == 0 {
        tracing::warn!("[backup] 目录无数据: {}", source_path);
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "无数据",
        )));
    }

    // 生成备份文件名：backup_类型_时间戳.zip
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let backup_filename = format!("backup_{}_{}.zip", backup_type, timestamp);
    // 保存到源目录下
    let backup_path = source.join(&backup_filename);

    // 创建 ZIP 文件
    let file = File::create(&backup_path).map_err(|e| AppError::Io(e))?;
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<()> = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    // 递归添加目录内容到 ZIP
    add_dir_to_zip(&mut zip, source, source, &options)?;

    zip.finish().map_err(|e| {
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
        ))
    })?;

    Ok(backup_path.to_string_lossy().to_string())
}

// 计算目录总大小
fn get_dir_size(path: &Path) -> Result<u64, AppError> {
    let mut total_size: u64 = 0;
    let mut file_count: u64 = 0;
    let mut dir_count: u64 = 0;

    for entry in std::fs::read_dir(path).map_err(|e| AppError::Io(e))? {
        let entry = entry.map_err(|e| AppError::Io(e))?;
        let entry_path = entry.path();
        if entry_path.is_file() {
            total_size += entry.metadata().map_err(|e| AppError::Io(e))?.len();
            file_count += 1;
        } else if entry_path.is_dir() {
            dir_count += 1;
            total_size += get_dir_size(&entry_path)?;
        }
    }

    // 记录调试信息
    tracing::info!(
        "[backup] 目录 {} 统计: 文件数={}, 子目录数={}, 总大小={} bytes",
        path.display(),
        file_count,
        dir_count,
        total_size
    );

    Ok(total_size)
}

fn add_dir_to_zip(
    zip: &mut zip::ZipWriter<File>,
    base_path: &Path,
    current_path: &Path,
    options: &zip::write::FileOptions<()>,
) -> Result<(), AppError> {
    for entry in std::fs::read_dir(current_path).map_err(|e| AppError::Io(e))? {
        let entry = entry.map_err(|e| AppError::Io(e))?;
        let path = entry.path();
        let name = path.strip_prefix(base_path).unwrap_or(&path);
        let name_str = name.to_string_lossy().to_string();

        // 跳过备份文件本身（避免递归包含）
        if name_str.starts_with("backup_") && name_str.ends_with(".zip") {
            continue;
        }

        if path.is_file() {
            let mut file = File::open(&path).map_err(|e| AppError::Io(e))?;
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .map_err(|e| AppError::Io(e))?;
            zip.start_file(name_str, *options).map_err(|e| {
                AppError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;
            zip.write_all(&contents).map_err(|e| AppError::Io(e))?;
        } else if path.is_dir() {
            zip.add_directory(name_str + "/", *options).map_err(|e| {
                AppError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;
            add_dir_to_zip(zip, base_path, &path, options)?;
        }
    }
    Ok(())
}

// ─── 数据恢复功能 ───────────────

/// 选择备份文件（从指定目录）
#[tauri::command]
pub async fn select_backup_file(
    app: AppHandle,
    default_dir: Option<String>,
) -> Result<Option<String>, AppError> {
    let mut dialog = app.dialog().file().add_filter("ZIP 备份文件", &["zip"]);

    // 如果提供了默认目录，设置起始目录
    if let Some(dir) = default_dir {
        let path = std::path::PathBuf::from(dir);
        if path.exists() {
            dialog = dialog.set_directory(&path);
        }
    }

    let file_path = dialog.blocking_pick_file();

    match file_path {
        Some(path) => {
            let path_str = path
                .as_path()
                .map(|p| p.to_string_lossy().to_string())
                .ok_or_else(|| {
                    AppError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "无效的文件路径",
                    ))
                })?;

            // 验证是否是有效的备份文件（以 backup_ 开头且以 .zip 结尾）
            let file_name = Path::new(&path_str)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            if !file_name.starts_with("backup_") || !file_name.ends_with(".zip") {
                return Err(AppError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "INVALID_BACKUP_FILE",
                )));
            }

            Ok(Some(path_str))
        }
        None => Ok(None),
    }
}

/// 从备份文件恢复数据
#[tauri::command]
pub async fn restore_from_backup(
    state: State<'_, AppState>,
    backup_path: String,
    target_path: String,
    restore_type: String,
    skip_pre_backup: bool,
) -> Result<String, AppError> {
    let is_db = restore_type.contains("数据库") || restore_type.contains("Database");
    // close 后无论成败都必须 recreate，否则连接池永久关闭
    let mut pool_closed = false;
    if is_db {
        state.db.close_pool().await.map_err(|e| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("关闭数据库连接失败: {}", e),
            ))
        })?;
        pool_closed = true;
    }

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let pre_restore_name = format!("pre_restore_backup_{}_{}.zip", restore_type, timestamp);
    let backup_path_owned = backup_path.clone();
    let target_path_owned = target_path.clone();
    let pre_restore_name_owned = pre_restore_name.clone();
    let target_path_for_err = target_path.clone();
    let skip_pre_backup_flag = skip_pre_backup;

    let outcome = async {
        let join = tokio::task::spawn_blocking(move || {
            restore_backup_sync(
                Path::new(&backup_path_owned),
                Path::new(&target_path_owned),
                &pre_restore_name_owned,
                skip_pre_backup_flag,
            )
        })
        .await
        .map_err(|e| {
            let backup_msg = if !skip_pre_backup {
                format!("|{}\\{}", target_path_for_err, pre_restore_name)
            } else {
                String::new()
            };
            AppError::Business(format!("RESTORE_ERROR|{}{}", e, backup_msg))
        })?;

        join.map_err(|e| {
            let backup_msg = if !skip_pre_backup {
                format!("|{}\\{}", target_path, pre_restore_name)
            } else {
                String::new()
            };
            match e {
                AppError::Business(msg) if msg.starts_with("RESTORE_ERROR|") => {
                    AppError::Business(msg)
                }
                other => AppError::Business(format!("RESTORE_ERROR|{}{}", other, backup_msg)),
            }
        })
    }
    .await;

    if pool_closed {
        if let Err(e) = state.db.recreate_pool().await {
            tracing::error!("[restore] recreate_pool 失败: {:#}", e);
            if outcome.is_ok() {
                return Err(AppError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("重建数据库连接失败: {}", e),
                )));
            }
            // 恢复已失败：仍尽量重建；重建也失败则保留原错误，并提示需重启
            tracing::error!("[restore] 恢复失败且连接池重建失败，后续 DB 可能不可用，需重启应用");
        }
    }

    outcome
}

/// 同步恢复：先安全解压到临时目录，成功后再清空 target 并迁入（防 zip-slip / 半残目录）
fn restore_backup_sync(
    backup: &Path,
    target: &Path,
    pre_restore_name: &str,
    skip_pre_backup: bool,
) -> Result<String, AppError> {
    if !backup.exists() {
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "备份文件不存在",
        )));
    }

    if !target.exists() {
        std::fs::create_dir_all(target).map_err(AppError::Io)?;
    }

    let pre_restore_backup_path = target.join(pre_restore_name);
    let target_size = get_dir_size(target).unwrap_or(0);
    let has_pre_backup = target_size > 0 && !skip_pre_backup;
    if has_pre_backup {
        let pre_backup_file = File::create(&pre_restore_backup_path).map_err(AppError::Io)?;
        let mut pre_zip = zip::ZipWriter::new(pre_backup_file);
        let options: zip::write::FileOptions<()> = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);
        add_dir_to_zip(&mut pre_zip, target, target, &options)?;
        pre_zip.finish().map_err(|e| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;
    }

    // 解到系统临时目录，失败时不影响已有 target 内容（预恢复 zip 已写入 target）
    let staging = std::env::temp_dir().join(format!(
        "fastclaw_restore_{}",
        uuid::Uuid::new_v4().simple()
    ));
    if staging.exists() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    std::fs::create_dir_all(&staging).map_err(AppError::Io)?;

    let extract_result = extract_backup_zip_safe(backup, &staging);
    if let Err(e) = extract_result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }

    // 解压成功后再清空 target（保留备份 zip），再迁入
    if let Err(e) = clear_restore_target(target) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    if let Err(e) = move_dir_contents(&staging, target) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    let _ = std::fs::remove_dir_all(&staging);

    let result_msg = if has_pre_backup {
        format!(
            "RESTORE_SUCCESS|{}",
            pre_restore_backup_path.to_string_lossy()
        )
    } else {
        "RESTORE_SUCCESS".to_string()
    };
    Ok(result_msg)
}

fn clear_restore_target(target: &Path) -> Result<(), AppError> {
    for entry in std::fs::read_dir(target).map_err(AppError::Io)? {
        let entry = entry.map_err(AppError::Io)?;
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with("pre_restore_backup_") && name.ends_with(".zip") {
            continue;
        }
        if name.starts_with("backup_") && name.ends_with(".zip") {
            continue;
        }
        if path.is_file() {
            std::fs::remove_file(&path).map_err(AppError::Io)?;
        } else if path.is_dir() {
            std::fs::remove_dir_all(&path).map_err(AppError::Io)?;
        }
    }
    Ok(())
}

/// 使用 enclosed_name + 落点前缀校验，拒绝 zip-slip
fn extract_backup_zip_safe(backup: &Path, dest: &Path) -> Result<(), AppError> {
    std::fs::create_dir_all(dest).map_err(AppError::Io)?;
    let dest_canon = std::fs::canonicalize(dest).map_err(AppError::Io)?;

    let backup_file = File::open(backup).map_err(AppError::Io)?;
    let mut archive = zip::ZipArchive::new(backup_file).map_err(|e| {
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("无法打开备份文件: {}", e),
        ))
    })?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("无法读取备份文件内容: {}", e),
            ))
        })?;

        let Some(enclosed) = file.enclosed_name() else {
            return Err(AppError::Business(format!(
                "ZIP 包含不安全路径（已拒绝）: {}",
                file.name()
            )));
        };
        if !is_relative_zip_entry_safe(&enclosed) {
            return Err(AppError::Business(format!(
                "ZIP 包含不安全路径（已拒绝）: {}",
                enclosed.display()
            )));
        }

        let outpath = dest.join(&enclosed);
        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath).map_err(AppError::Io)?;
            let dir_canon = std::fs::canonicalize(&outpath).map_err(AppError::Io)?;
            if !dir_canon.starts_with(&dest_canon) {
                let _ = std::fs::remove_dir_all(&outpath);
                return Err(AppError::Business(format!(
                    "ZIP 路径越界（已拒绝）: {}",
                    enclosed.display()
                )));
            }
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent).map_err(AppError::Io)?;
                let parent_canon = std::fs::canonicalize(parent).map_err(AppError::Io)?;
                if !parent_canon.starts_with(&dest_canon) {
                    return Err(AppError::Business(format!(
                        "ZIP 路径越界（已拒绝）: {}",
                        enclosed.display()
                    )));
                }
            }
            let mut outfile = File::create(&outpath).map_err(AppError::Io)?;
            std::io::copy(&mut file, &mut outfile).map_err(AppError::Io)?;
        }
    }
    Ok(())
}

fn is_relative_zip_entry_safe(relative: &Path) -> bool {
    if relative.as_os_str().is_empty() || relative.is_absolute() {
        return false;
    }
    for component in relative.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

fn move_dir_contents(src: &Path, dst: &Path) -> Result<(), AppError> {
    for entry in std::fs::read_dir(src).map_err(AppError::Io)? {
        let entry = entry.map_err(AppError::Io)?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if to.exists() {
            if to.is_dir() {
                std::fs::remove_dir_all(&to).map_err(AppError::Io)?;
            } else {
                std::fs::remove_file(&to).map_err(AppError::Io)?;
            }
        }
        match std::fs::rename(&from, &to) {
            Ok(()) => {}
            Err(_) => {
                if from.is_dir() {
                    copy_dir_recursive_sys(&from, &to)?;
                    std::fs::remove_dir_all(&from).map_err(AppError::Io)?;
                } else {
                    std::fs::copy(&from, &to).map_err(AppError::Io)?;
                    std::fs::remove_file(&from).map_err(AppError::Io)?;
                }
            }
        }
    }
    Ok(())
}

fn copy_dir_recursive_sys(src: &Path, dst: &Path) -> Result<(), AppError> {
    std::fs::create_dir_all(dst).map_err(AppError::Io)?;
    for entry in std::fs::read_dir(src).map_err(AppError::Io)? {
        let entry = entry.map_err(AppError::Io)?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive_sys(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(AppError::Io)?;
        }
    }
    Ok(())
}

// ─── 备份文件信息 ───────────────

#[derive(Debug, Serialize)]
pub struct BackupFileInfo {
    pub file_name: String,
    pub file_size: u64,
    pub file_size_formatted: String,
    pub created_time: String,
    pub backup_type: String,
}

#[tauri::command]
pub fn get_backup_file_info(backup_path: String) -> Result<BackupFileInfo, AppError> {
    let path = Path::new(&backup_path);

    if !path.exists() {
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "备份文件不存在",
        )));
    }

    let metadata = std::fs::metadata(path).map_err(|e| AppError::Io(e))?;
    let file_size = metadata.len();

    // 格式化文件大小
    let file_size_formatted = format_file_size(file_size);

    // 获取文件名
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // 解析备份类型（支持中英文）
    let backup_type = if file_name.contains("程序目录数据")
        || file_name.contains("Program Directory Data")
    {
        "程序目录数据".to_string()
    } else if file_name.contains("数据库数据") || file_name.contains("Database Data") {
        "数据库数据".to_string()
    } else {
        "未知类型".to_string()
    };

    // 获取创建时间
    let created_time = metadata
        .created()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| {
            let datetime = chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                .unwrap_or_else(|| chrono::DateTime::UNIX_EPOCH);
            datetime.format("%Y-%m-%d %H:%M:%S").to_string()
        })
        .unwrap_or_else(|| "未知时间".to_string());

    Ok(BackupFileInfo {
        file_name,
        file_size,
        file_size_formatted,
        created_time,
        backup_type,
    })
}

fn format_file_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = size as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_index])
}

// ─── 磁盘空间检查 ───────────────

#[derive(Debug, Serialize)]
pub struct DiskSpaceInfo {
    pub available_space: u64,
    pub available_space_formatted: String,
    pub total_space: u64,
    pub total_space_formatted: String,
    pub is_sufficient: bool,
}

#[tauri::command]
pub fn check_disk_space(path: String, required_space: u64) -> Result<DiskSpaceInfo, AppError> {
    let path = Path::new(&path);
    let (available_space, total_space) = query_disk_space(path)?;
    let is_sufficient = available_space >= required_space;

    Ok(DiskSpaceInfo {
        available_space,
        available_space_formatted: format_file_size(available_space),
        total_space,
        total_space_formatted: format_file_size(total_space),
        is_sufficient,
    })
}

/// 查询 `path` 所在卷的可用/总空间（字节）
fn query_disk_space(path: &Path) -> Result<(u64, u64), AppError> {
    // 路径不存在时仍尽量落到父目录/根，便于下载前检查目标盘
    let probe = if path.exists() {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    } else {
        path.ancestors()
            .find(|p| p.exists())
            .map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()))
            .unwrap_or_else(|| path.to_path_buf())
    };

    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut best: Option<(usize, u64, u64)> = None; // (mount_len, available, total)
    for disk in disks.list() {
        let mount = disk.mount_point();
        if probe.starts_with(mount) {
            let mount_len = mount.as_os_str().len();
            match best {
                Some((len, _, _)) if len >= mount_len => {}
                _ => best = Some((mount_len, disk.available_space(), disk.total_space())),
            }
        }
    }

    best.map(|(_, available, total)| (available, total))
        .ok_or_else(|| {
            AppError::Business(format!(
                "无法获取磁盘空间信息: {}",
                path.display()
            ))
        })
}

// ─── 重启应用 ───────────────

#[tauri::command]
pub async fn restart_app(app: AppHandle) -> Result<(), AppError> {
    let recovery = app.state::<crate::state::AppState>().dependency_recovery.clone();
    recovery.shutdown();
    let _recovery_operation = recovery.operation.lock().await;
    // 重启前清理子进程（FastPlan + KG + MCP + WSP），防止孤儿进程泄漏
    let kg_pids = app.state::<crate::state::AppState>().kg_pids.clone();
    let wsp = app.state::<crate::state::AppState>().wsp.clone();
    let mcp = app.state::<crate::state::AppState>().mcp.clone();
    let _ = crate::commands::wsp::stop_fastplan_inner(&kg_pids).await;
    let _ = crate::commands::wsp::stop_kg_inner(&kg_pids).await;
    crate::core::mcp::disconnect_all_shared(&mcp).await;
    let _ = wsp.shutdown().await;
    crate::state::SHUTDOWN_DONE.store(true, std::sync::atomic::Ordering::SeqCst);
    app.restart();
    Ok(())
}

// ─── 网络连通性检测 ───────────────

#[tauri::command]
pub async fn check_internet_connectivity(url: Option<String>) -> Result<bool, AppError> {
    let check_url = url.unwrap_or_else(|| "https://www.baidu.com/favicon.ico".to_string());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| AppError::Network(format!("创建 HTTP 客户端失败: {}", e)))?;

    // 先尝试 HEAD 请求
    match client.head(&check_url).send().await {
        Ok(resp) => return Ok(resp.status().is_success()),
        Err(_) => {}
    }

    // HEAD 失败则回退到 GET
    match client.get(&check_url).send().await {
        Ok(resp) => Ok(resp.status().is_success()),
        Err(e) => {
            tracing::warn!("[connectivity] 网络检测失败: {}", e);
            Ok(false)
        }
    }
}

// ─── 重置数据 ───────────────

#[tauri::command]
pub async fn reset_data(
    state: State<'_, AppState>,
    reset_type: String,
    keep_files: Vec<String>,
) -> Result<String, AppError> {
    let app_data_dir = std::env::current_exe()
        .map_err(AppError::Io)?
        .parent()
        .map(|p| p.join("data"))
        .unwrap_or_else(|| std::path::PathBuf::from("./data"));

    let db_dir = crate::db::Database::db_path_static()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    let reset_app = reset_type == "appData" || reset_type == "all";
    let reset_db = reset_type == "dbData" || reset_type == "all";

    let mut pool_closed = false;
    if reset_db {
        state.db.close_pool().await.map_err(|e| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("关闭数据库连接失败: {}", e),
            ))
        })?;
        pool_closed = true;
    }

    let ad = app_data_dir.clone();
    let dd = db_dir.clone();
    let kf = keep_files.clone();
    let clear_result = tokio::task::spawn_blocking(move || {
        if reset_app && ad.exists() {
            if let Ok(entries) = std::fs::read_dir(&ad) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if kf.contains(&name.to_string()) {
                        continue;
                    }
                    if path.is_file() {
                        let _ = std::fs::remove_file(&path);
                    } else if path.is_dir() {
                        let _ = std::fs::remove_dir_all(&path);
                    }
                }
            }
        }
        if reset_db {
            if let Ok(entries) = std::fs::read_dir(&dd) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if kf.contains(&name.to_string()) {
                        continue;
                    }
                    if path.is_file() {
                        let _ = std::fs::remove_file(&path);
                    } else if path.is_dir() {
                        let _ = std::fs::remove_dir_all(&path);
                    }
                }
            }
        }
        Ok::<(), AppError>(())
    })
    .await
    .map_err(|e| {
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
        ))
    });

    let clear_result = match clear_result {
        Ok(inner) => inner,
        Err(e) => Err(e),
    };

    if pool_closed {
        if let Err(e) = state.db.recreate_pool().await {
            tracing::error!("[reset_data] recreate_pool 失败: {:#}", e);
            if clear_result.is_ok() {
                return Err(AppError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("重建数据库连接失败: {}", e),
                )));
            }
            tracing::error!(
                "[reset_data] 清理失败且连接池重建失败，后续 DB 可能不可用，需重启应用"
            );
            return clear_result.map(|_| "数据重置成功".to_string());
        }
    }

    clear_result.map(|_| "数据重置成功".to_string())
}

// ─── 清除缓存 ───────────────

#[tauri::command]
pub async fn clear_cache(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    cache_type: String,
) -> Result<String, AppError> {
    let mut results: Vec<String> = vec![];

    // 1. WebView 浏览器缓存
    if cache_type == "browser" || cache_type == "all" {
        if let Some(w) = app_handle.get_webview_window("main") {
            let _ = w.clear_all_browsing_data().map_err(|_| ()).ok();
            results.push("浏览器缓存已清除".into());
        }
    }

    // 2. 数据库 WAL 收缩
    if cache_type == "db" || cache_type == "all" {
        let pool = state.db.pool().await;
        let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&*pool)
            .await;
        results.push("数据库缓存已清理".into());
    }

    // 3. kg_storage 临时文件清理
    if cache_type == "kg" || cache_type == "all" {
        let kg_storage = std::path::PathBuf::from(std::env::var("APPDATA").unwrap_or_default())
            .join("FastClaw")
            .join("kg_storage");
        if kg_storage.exists() {
            for entry in std::fs::read_dir(&kg_storage).map_err(|e| AppError::Io(e))? {
                let entry = entry.map_err(|e| AppError::Io(e))?;
                let path = entry.path();
                // 只清理临时/缓存文件，保留索引
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.contains("temp") || name.contains("cache") || name.contains("tmp") {
                    if path.is_file() {
                        let _ = std::fs::remove_file(&path);
                    } else if path.is_dir() {
                        let _ = std::fs::remove_dir_all(&path);
                    }
                }
            }
            results.push("知识库临时文件已清理".into());
        }
    }

    // 4. 程序目录 data/ 中非备份的临时文件
    if cache_type == "app" || cache_type == "all" {
        let app_data = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.join("data")));
        if let Some(ad) = app_data {
            if ad.exists() {
                for entry in std::fs::read_dir(&ad).map_err(|e| AppError::Io(e))? {
                    let entry = entry.map_err(|e| AppError::Io(e))?;
                    let path = entry.path();
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if name.starts_with("backup_") {
                        continue;
                    }
                    if name.contains("temp") || name.contains("cache") || name.contains("tmp") {
                        if path.is_file() {
                            let _ = std::fs::remove_file(&path);
                        } else if path.is_dir() {
                            let _ = std::fs::remove_dir_all(&path);
                        }
                    }
                }
                results.push("程序临时文件已清理".into());
            }
        }
    }

    if results.is_empty() {
        Ok("无需清理的缓存".to_string())
    } else {
        Ok(results.join("\n"))
    }
}

/// 获取 HTTP 服务器实际绑定的端口（5179 被占用时可能漂移到 5180-5189）
#[tauri::command]
pub fn get_http_port(state: State<'_, AppState>) -> u16 {
    state.http_port.load(std::sync::atomic::Ordering::SeqCst)
}
