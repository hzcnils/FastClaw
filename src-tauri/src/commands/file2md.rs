// FastClaw · File2Md OCR Tauri Commands

use crate::core::error::AppError;
use crate::state::AppState;
use crate::utils::paths;
use tauri::State;

/// 对 base64 图片进行 OCR 文字提取
/// 参数：image_base64 — data:image/png;base64,xxxxx 格式的图片数据
/// 返回：Markdown 格式的 OCR 文字结果
#[tauri::command]
pub async fn file2md_ocr(
    state: State<'_, AppState>,
    image_base64: String,
) -> Result<String, AppError> {
    state
        .file2md
        .ocr_image(&image_base64)
        .await
        .map_err(|e| AppError::Business(format!("OCR 识别失败: {}", e)))
}

/// 对本地图片文件进行 OCR 文字提取（路径版，避免大 base64 穿越 IPC）
/// 参数：image_path — 本地图片文件的绝对路径
/// 返回：Markdown 格式的 OCR 文字结果
#[tauri::command]
pub async fn file2md_ocr_by_path(
    state: State<'_, AppState>,
    image_path: String,
) -> Result<String, AppError> {
    state
        .file2md
        .ocr_image_path(&image_path)
        .await
        .map_err(|e| AppError::Business(format!("OCR 识别失败: {}", e)))
}

/// 清理 OCR 临时文件
#[tauri::command]
pub async fn file2md_cleanup_temp(path: String) -> Result<(), AppError> {
    if let Err(e) = tokio::fs::remove_file(&path).await {
        tracing::warn!("清理 OCR 临时文件失败 ({}): {}", path, e);
    }
    Ok(())
}

/// 检查 File2Md OCR 可执行文件是否存在
/// 前端在尝试 OCR 之前先调用此命令，以便决定是否提示用户下载
#[tauri::command]
pub async fn check_file2md_exists() -> Result<bool, AppError> {
    Ok(paths::file2md_ocr_exe_path().exists())
}
