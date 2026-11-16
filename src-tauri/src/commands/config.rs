// FastClaw · 应用配置 Tauri Commands
// 前端调用这些命令读写 config.json

use crate::core::config::AppConfig;
use crate::core::error::AppError;
use crate::state::AppState;
use tauri::State;

/// 获取完整应用配置
#[tauri::command]
pub async fn get_app_config(state: State<'_, AppState>) -> Result<AppConfig, AppError> {
    Ok(state.app_config.read().await.clone())
}

/// 按点号路径更新单个配置字段，即时保存到 config.json
/// dot_path 示例: "services.fastplan.url"
/// value 为 JSON 字符串
#[tauri::command]
pub async fn update_app_config_field(
    state: State<'_, AppState>,
    dot_path: String,
    value: String,
) -> Result<(), AppError> {
    let v: serde_json::Value =
        serde_json::from_str(&value).map_err(|e| AppError::Business(e.to_string()))?;
    state
        .app_config
        .update_field(&dot_path, v)
        .await
        .map_err(|e| AppError::Business(e.to_string()))?;
    Ok(())
}

/// 重置指定服务的 URL 为出厂默认值
#[tauri::command]
pub async fn reset_service_url(
    state: State<'_, AppState>,
    service_name: String,
) -> Result<(), AppError> {
    let defaults = AppConfig::default();
    let default_url = match service_name.as_str() {
        "fastplan" => &defaults.services.fastplan,
        "fastkg" => &defaults.services.fastkg,
        "shimmy" => &defaults.services.shimmy,
        "pinchtab" => &defaults.services.pinchtab,
        "workflow" => &defaults.services.workflow,
        "qdrant" => &defaults.services.qdrant,
        "llama_server" => &defaults.services.llama_server,
        "asr" => &defaults.services.asr,
        _ => return Err(AppError::Business(format!("未知服务: {}", service_name))),
    };

    let dot_path = format!("services.{}", service_name);
    let value = serde_json::Value::String(default_url.clone());
    state
        .app_config
        .update_field(&dot_path, value)
        .await
        .map_err(|e| AppError::Business(e.to_string()))?;
    Ok(())
}

/// 重置指定远程服务器 URL 为出厂默认值
#[tauri::command]
pub async fn reset_remote_server_url(
    state: State<'_, AppState>,
    server_key: String,
) -> Result<(), AppError> {
    let defaults = AppConfig::default();

    // 独立模型数据开关 — 布尔类型，需特殊处理
    if server_key == "independent_model_data" {
        let value = serde_json::Value::Bool(false);
        state
            .app_config
            .update_field("remote_servers.independent_model_data", value)
            .await
            .map_err(|e| AppError::Business(e.to_string()))?;
        return Ok(());
    }

    let default_url = match server_key.as_str() {
        "download_api_base_url" => &defaults.remote_servers.download_api_base_url,
        "download_api_base_url_private" => &defaults.remote_servers.download_api_base_url_private,
        "model_sync_url" => &defaults.remote_servers.model_sync_url,
        "fastci_base_url" => &defaults.remote_servers.fastci_base_url,
        "fastci_model_download_url" => &defaults.remote_servers.fastci_model_download_url,
        "independent_model_api_url" => &defaults.remote_servers.independent_model_api_url,
        _ => {
            return Err(AppError::Business(format!(
                "未知远程服务器配置: {}",
                server_key
            )))
        }
    };

    let dot_path = format!("remote_servers.{}", server_key);
    let value = serde_json::Value::String(default_url.clone());
    state
        .app_config
        .update_field(&dot_path, value)
        .await
        .map_err(|e| AppError::Business(e.to_string()))?;
    Ok(())
}

/// 全量更新配置
#[tauri::command]
pub async fn update_app_config(
    state: State<'_, AppState>,
    config: AppConfig,
) -> Result<(), AppError> {
    state
        .app_config
        .update_full(config)
        .await
        .map_err(|e| AppError::Business(e.to_string()))?;
    Ok(())
}
