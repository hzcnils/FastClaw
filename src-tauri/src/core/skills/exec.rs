// FastClaw · Skill 脚本执行委托 FastPlan
//
// v8: 把 skill 捆绑脚本/代码的执行委托给 FastPlan 沙箱（Pyodide WASM / Node 子进程）。
// FastClaw 本地不做沙箱、不做隔离——只传 skill_id + input，拿 SkillOutput，落文件。
// FastPlan 不可达时返回可读错误，不本地偷跑。

use crate::core::error::AppError;
use serde::Deserialize;
use std::path::{Component, Path, PathBuf};

/// FastPlan 产物单文件最大体积（防止恶意/异常大文件撑爆内存）
const MAX_SKILL_OUTPUT_BYTES: u64 = 200 * 1024 * 1024;

// ── Types matching FastPlan POST /api/skills/:id/execute response ──

#[derive(Debug, Deserialize)]
pub struct SkillOutput {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub data: Vec<DataItem>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct DataItem {
    pub name: String,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub base64_data: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

// ── Delegate ──

/// Execute a skill via FastPlan sandbox. FastClaw does NO local sandboxing.
///
/// `fastplan_base` — FastPlan base URL (same source as /api/admin proxy).
/// `skill_id` — skill slug (e.g. "xlsx").
/// `input` — arbitrary JSON passed through to FastPlan executor.
/// `session_id` / `lang` — optional context forwarded to FastPlan.
pub async fn execute_skill_via_fastplan(
    fastplan_base: &str,
    skill_id: &str,
    input: serde_json::Value,
    session_id: Option<String>,
    lang: Option<String>,
) -> Result<SkillOutput, AppError> {
    let url = format!(
        "{}/api/skills/{}/execute",
        fastplan_base.trim_end_matches('/'),
        skill_id
    );

    let body = serde_json::json!({
        "input": input,
        "context": {
            "session_id": session_id,
            "lang": lang,
        }
    });

    let resp = reqwest::Client::new()
        .post(&url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| {
            AppError::Network(format!(
                "skill 脚本执行需连接 FastPlan 沙箱——FastPlan 不可达: {e}"
            ))
        })?;

    let out: SkillOutput = resp
        .json()
        .await
        .map_err(|e| AppError::Parse(format!("FastPlan 执行响应解析失败: {e}")))?;

    Ok(out)
}

/// Download skill output files to a local directory.
/// Handles `base64_data` (decode & write) and `url` (HTTP GET) items.
/// Returns paths of all locally-written files.
pub async fn download_skill_outputs(
    fastplan_base: &str,
    out: &SkillOutput,
    dest_dir: &Path,
) -> Result<Vec<PathBuf>, AppError> {
    let mut written = Vec::new();

    std::fs::create_dir_all(dest_dir).map_err(AppError::Io)?;
    let dest_dir_canon = std::fs::canonicalize(dest_dir).map_err(AppError::Io)?;

    for item in &out.data {
        let dest = resolve_skill_output_path(&dest_dir_canon, &item.name)?;

        if let Some(ref b64) = item.base64_data {
            use base64::Engine;
            // base64 膨胀约 4/3；先拦过大载荷再解码
            let approx = (b64.len() as u64).saturating_mul(3) / 4;
            if approx > MAX_SKILL_OUTPUT_BYTES {
                return Err(AppError::Business(format!(
                    "产物过大已拒绝（base64）: {} (> {} MB)",
                    item.name,
                    MAX_SKILL_OUTPUT_BYTES / (1024 * 1024)
                )));
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| AppError::Parse(format!("base64 decode {}: {e}", item.name)))?;
            if bytes.len() as u64 > MAX_SKILL_OUTPUT_BYTES {
                return Err(AppError::Business(format!(
                    "产物过大已拒绝: {} ({} bytes)",
                    item.name,
                    bytes.len()
                )));
            }
            write_skill_output_file(&dest_dir_canon, &dest, &bytes)?;
            written.push(dest);
        } else if let Some(ref file_url) = item.url {
            let full_url = if file_url.starts_with("http") {
                file_url.clone()
            } else {
                format!(
                    "{}/{}",
                    fastplan_base.trim_end_matches('/'),
                    file_url.trim_start_matches('/')
                )
            };

            let bytes = download_with_size_limit(&full_url, &item.name).await?;
            write_skill_output_file(&dest_dir_canon, &dest, &bytes)?;
            written.push(dest);
        } else if let Some(ref content) = item.content {
            if content.len() as u64 > MAX_SKILL_OUTPUT_BYTES {
                return Err(AppError::Business(format!(
                    "产物过大已拒绝（content）: {} ({} bytes)",
                    item.name,
                    content.len()
                )));
            }
            write_skill_output_file(&dest_dir_canon, &dest, content.as_bytes())?;
            written.push(dest);
        }
    }

    Ok(written)
}

/// 拒绝 `../`、绝对路径等，确保落点在 `dest_dir` 内
fn resolve_skill_output_path(dest_dir_canon: &Path, name: &str) -> Result<PathBuf, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Business("产物文件名为空".into()));
    }
    let relative = Path::new(name);
    if relative.is_absolute() {
        return Err(AppError::Business(format!(
            "产物路径不安全（绝对路径已拒绝）: {name}"
        )));
    }
    for component in relative.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::Business(format!(
                    "产物路径不安全（已拒绝）: {name}"
                )));
            }
        }
    }
    Ok(dest_dir_canon.join(relative))
}

fn write_skill_output_file(
    dest_dir_canon: &Path,
    dest: &Path,
    bytes: &[u8],
) -> Result<(), AppError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(AppError::Io)?;
        let parent_canon = std::fs::canonicalize(parent).map_err(AppError::Io)?;
        if !parent_canon.starts_with(dest_dir_canon) {
            return Err(AppError::Business(format!(
                "产物路径越界（已拒绝）: {}",
                dest.display()
            )));
        }
    }
    std::fs::write(dest, bytes).map_err(AppError::Io)?;
    // 写完再确认最终文件仍在目标目录下（防链接/竞态）
    if let Ok(file_canon) = std::fs::canonicalize(dest) {
        if !file_canon.starts_with(dest_dir_canon) {
            let _ = std::fs::remove_file(dest);
            return Err(AppError::Business(format!(
                "产物路径越界（已拒绝）: {}",
                dest.display()
            )));
        }
    }
    Ok(())
}

async fn download_with_size_limit(url: &str, name: &str) -> Result<Vec<u8>, AppError> {
    use futures::StreamExt;

    let resp = reqwest::get(url)
        .await
        .map_err(|e| AppError::Network(format!("下载产物 {name} 失败: {e}")))?;

    if !resp.status().is_success() {
        return Err(AppError::Network(format!(
            "下载产物 {name} 失败: HTTP {}",
            resp.status()
        )));
    }

    if let Some(len) = resp.content_length() {
        if len > MAX_SKILL_OUTPUT_BYTES {
            return Err(AppError::Business(format!(
                "产物过大已拒绝: {name} (Content-Length {len} > {} MB)",
                MAX_SKILL_OUTPUT_BYTES / (1024 * 1024)
            )));
        }
    }

    let mut collected = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|e| AppError::Network(format!("读取产物 {name} 失败: {e}")))?;
        if collected.len() as u64 + chunk.len() as u64 > MAX_SKILL_OUTPUT_BYTES {
            return Err(AppError::Business(format!(
                "产物过大已拒绝: {name} (> {} MB)",
                MAX_SKILL_OUTPUT_BYTES / (1024 * 1024)
            )));
        }
        collected.extend_from_slice(&chunk);
    }
    Ok(collected)
}
