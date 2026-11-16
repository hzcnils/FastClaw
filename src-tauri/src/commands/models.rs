// FastClaw · 模型与供应商管理 Tauri Commands

pub mod catalog;
pub mod gateways;
pub mod import_export;
pub mod providers;
pub mod ratings;
pub mod sync;

use crate::core::error::AppError;
use crate::db::repo;
use crate::state::AppState;
use serde::Deserialize;
use sqlx::Row;
use tauri::State;

#[tauri::command]
pub async fn list_models_tree(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let rows = repo::models::list_models_tree(&*state.db.pool().await).await?;
    Ok(rows)
}

#[tauri::command]
pub async fn list_all_providers_with_models(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, AppError> {
    log::info!("list_all_providers_with_models 命令被调用");
    let rows = repo::models::list_all_providers_with_models(&*state.db.pool().await).await?;
    log::info!("list_all_providers_with_models 返回 {} 条记录", rows.len());
    Ok(rows)
}

#[derive(Debug, Deserialize)]
pub struct UploadLocalModelRequest {
    pub name: String,
    #[serde(rename = "modelType")]
    pub model_type: String,
    pub url: Option<String>,
    pub category: Option<String>,
    pub desc: Option<String>,
    pub tags: Vec<String>,
    #[serde(rename = "localPath")]
    pub local_path: Option<String>,
}

#[tauri::command]
pub async fn upload_local_model(
    state: State<'_, AppState>,
    request: UploadLocalModelRequest,
) -> Result<String, AppError> {
    let id = uuid::Uuid::new_v4().to_string();

    // 计算模型大小（如果路径存在）
    let model_size = if let Some(ref path) = request.local_path {
        tokio::fs::metadata(path).await.ok().map(|m| {
            let size = m.len();
            if size > 1024 * 1024 * 1024 {
                format!("{:.2}GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
            } else if size > 1024 * 1024 {
                format!("{:.2}MB", size as f64 / (1024.0 * 1024.0))
            } else {
                format!("{:.2}KB", size as f64 / 1024.0)
            }
        })
    } else {
        None
    };

    repo::models::insert_local_model(
        &*state.db.pool().await,
        &repo::models::NewLocalModel {
            id: id.clone(),
            model_name: request.name,
            model_type: request.model_type,
            model_url: request.url,
            model_category: request.category,
            model_desc: request.desc,
            model_tags: Some(request.tags.join(",")),
            model_path: request.local_path,
            model_version: Some("v1.0.0".to_string()),
            model_size,
            is_enabled: true,
        },
    )
    .await?;

    Ok(id)
}

#[tauri::command]
pub async fn list_local_models(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let models = repo::models::list_wsp_market_models(&*state.db.pool().await).await?;
    Ok(models)
}

#[tauri::command]
pub async fn mark_model_downloaded(
    state: State<'_, AppState>,
    model_id: String,
) -> Result<(), AppError> {
    repo::models::mark_model_as_downloaded(&*state.db.pool().await, &model_id).await?;
    Ok(())
}

// ─── 数据加载 ───

pub(super) async fn load_mock_items(
    model_download_url: &str,
) -> Result<Vec<serde_json::Value>, AppError> {
    let url = model_download_url;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Network(format!("创建请求客户端失败: {}", e)))?;
    let resp = client.get(url).send().await
        .map_err(|e| {
            if e.is_timeout() {
                AppError::Network("模型同步超时：无法连接到同步服务器，请检查网络后再试。".into())
            } else if e.is_connect() {
                AppError::Network("模型同步失败：无法连接到同步服务器，请检查地址后再试。".into())
            } else {
                AppError::Network(format!("请求模型同步接口失败: {}", e))
            }
        })?;
    if !resp.status().is_success() {
        return Err(AppError::Network(format!(
            "模型同步接口返回错误状态: {} ({})",
            resp.status(),
            url
        )));
    }
    resp.json::<Vec<serde_json::Value>>()
        .await
        .map_err(|e| AppError::Parse(format!("解析模型同步接口响应失败: {}", e)))
}

// ─── 同步执行（写入 DB） ───

// ─── 导入模型（前端上传 JSON 数据） ───

// ==================== wsp_model_market 命令 ====================

#[tauri::command]
pub async fn sync_wsp_market(
    state: State<'_, AppState>,
    models: Vec<repo::models::WspMarketModel>,
) -> Result<String, AppError> {
    repo::models::sync_wsp_market_models(&*state.db.pool().await, &models).await?;
    Ok(format!("已同步 {} 个模型", models.len()))
}

#[tauri::command]
pub async fn list_wsp_market(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let models = repo::models::list_wsp_market_models(&*state.db.pool().await).await?;
    Ok(models)
}

#[tauri::command]
pub async fn import_wsp_models(
    state: State<'_, AppState>,
    models: Vec<repo::models::WspMarketModel>,
) -> Result<String, AppError> {
    let inserted = repo::models::sync_wsp_market_models(&*state.db.pool().await, &models).await?;
    Ok(format!(
        "已导入 {} 个新模型，更新了 {} 个已有模型",
        inserted,
        models.len() - inserted
    ))
}

#[tauri::command]
pub async fn prepare_wsp_download(
    state: State<'_, AppState>,
    model_id: String,
    model_name: Option<String>,
) -> Result<tauri_plugin_download_center::model::StartDownloadInput, AppError> {
    let id: i64 = model_id
        .parse()
        .map_err(|_| AppError::InvalidInput("无效的模型 ID".into()))?;
    let pool = &*state.db.pool().await;
    let row = sqlx::query("SELECT * FROM wsp_model_market WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("模型不存在".into()))?;
    let get_s =
        |column: &str| -> String { row.get::<Option<String>, _>(column).unwrap_or_default() };
    let exe_name = get_s("exeName");
    let origin_name = get_s("exeOriginName");
    let file_name = if origin_name.is_empty() {
        format!("{exe_name}.exe")
    } else {
        origin_name
    };
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.to_path_buf()))
        .unwrap_or_else(|| ".".into());
    let target = exe_dir.join("data").join("models").join(file_name);
    let base = state
        .app_config
        .read()
        .await
        .remote_servers
        .fastci_base_url
        .clone();
    Ok(tauri_plugin_download_center::model::StartDownloadInput {
        resource_type: "wsp-model".into(),
        resource_id: model_id.clone(),
        name: model_name
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| exe_name.clone()),
        payload: serde_json::json!({
            "targetPath": target.to_string_lossy(), "modelId": model_id,
            "executionPlan": { "instructions": [
                { "type":"download", "url":format!("{base}/api/wsp/{model_id}/download"), "tempPath":format!("{}.part",target.to_string_lossy()), "resume":true },
                { "type":"backupTarget", "targetPath":target.to_string_lossy() },
                { "type":"replaceTarget", "source":"temp", "targetPath":target.to_string_lossy() },
                { "type":"cleanup", "targets":["backup"] }
            ]}
        }),
        silent: false,
    })
}

#[tauri::command]
pub async fn complete_wsp_download(
    state: State<'_, AppState>,
    model_id: String,
    target_path: String,
) -> Result<(), AppError> {
    let id: i64 = model_id
        .parse()
        .map_err(|_| AppError::InvalidInput("无效的模型 ID".into()))?;
    let pool = &*state.db.pool().await;
    let row = sqlx::query("SELECT * FROM wsp_model_market WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("模型不存在".into()))?;
    let get_s =
        |column: &str| -> String { row.get::<Option<String>, _>(column).unwrap_or_default() };
    let exe_name = get_s("exeName");
    let api_url = get_s("startupApiUrl");
    let (host, port, base_path) = if api_url.is_empty() {
        (String::new(), 0, String::new())
    } else {
        let without_protocol = api_url
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        let (host_port, path) = without_protocol
            .find('/')
            .map(|at| (&without_protocol[..at], &without_protocol[at..]))
            .unwrap_or((without_protocol, "/"));
        let (host_name, port) = host_port
            .rfind(':')
            .map(|at| {
                (
                    &host_port[..at],
                    host_port[at + 1..].parse::<i32>().unwrap_or(80),
                )
            })
            .unwrap_or((host_port, 80));
        let scheme = if api_url.starts_with("https") {
            "https"
        } else {
            "http"
        };
        (
            format!("{scheme}://{host_name}:{port}"),
            port,
            path.to_string(),
        )
    };
    sqlx::query("DELETE FROM wsp_services WHERE name = ?")
        .bind(&exe_name)
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO wsp_services (id,name,display_name,description,exe_path,args,category,categoryName,subCategoryName,version,protocol,host,port,base_path,auto_start,is_enabled,is_installed) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
        .bind(&model_id).bind(&exe_name).bind(&exe_name).bind(get_s("skillDescription")).bind(target_path).bind("").bind(get_s("tag"))
        .bind(row.try_get::<Option<String>, _>("categoryName").ok().flatten()).bind(row.try_get::<Option<String>, _>("subCategoryName").ok().flatten())
        .bind(get_s("skillVersion")).bind("http").bind(host).bind(port).bind(base_path).bind(false).bind(true).bind(true).execute(pool).await?;
    sqlx::query("UPDATE wsp_model_market SET status = 0 WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 取消进行中的 WSP 模型下载：标记该 model_id，下载循环下一轮检查到即中断并清理部分文件
/// 从下载中心删除已下载模型：清除 wsp_services 记录，恢复 model market 状态
#[tauri::command]
pub async fn delete_downloaded_model(
    state: State<'_, AppState>,
    model_id: String,
) -> Result<(), AppError> {
    let model_id_i64: i64 = model_id
        .parse()
        .map_err(|_| AppError::InvalidInput("无效的模型ID".into()))?;
    let pool = &*state.db.pool().await;
    // 查找模型 exeName 和下载文件路径
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT name, exe_path FROM wsp_services WHERE name = (SELECT exeName FROM wsp_model_market WHERE id = ? LIMIT 1)"
    ).bind(model_id_i64).fetch_optional(pool).await?;
    if let Some((ref name, exe_path)) = row {
        sqlx::query("DELETE FROM wsp_services WHERE name = ?")
            .bind(name)
            .execute(pool)
            .await?;
        // 删除下载的模型文件
        if let Some(ref path) = exe_path {
            let p = std::path::Path::new(path);
            if p.exists() && !p.is_dir() {
                tokio::fs::remove_file(p).await.ok();
                log::info!("[delete_downloaded] 已删除模型文件: {}", path);
            }
        }
    }
    // 恢复模型市场状态为可下载
    sqlx::query("UPDATE wsp_model_market SET status = 1 WHERE id = ?")
        .bind(model_id_i64)
        .execute(pool)
        .await?;
    log::info!(
        "[delete_downloaded] 模型 id={} 已删除，状态恢复为可下载",
        model_id
    );
    Ok(())
}

// ─── 独立模型数据同步 ───────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::core::models::{
        infer_model_info, match_provider, remap_provider_name, tokenize_and_normalize,
    };

    fn providers() -> Vec<(String, String, String)> {
        vec![
            ("OPENAI".into(), "provider-openai".into(), "OpenAI".into()),
            (
                "GEMINI".into(),
                "provider-gemini".into(),
                "Google Gemini".into(),
            ),
            (
                "DEEPSEEK".into(),
                "provider-deepseek".into(),
                "DeepSeek".into(),
            ),
            ("HUIZHI".into(), "provider-huizhi".into(), "汇智网".into()),
            ("LOCAL".into(), "provider-local".into(), "本电脑".into()),
        ]
    }

    #[test]
    fn infers_model_type_and_capability_labels() {
        assert_eq!(
            infer_model_info("gpt-4o", None),
            ("chat".into(), "视觉,工具,推理".into())
        );
        assert_eq!(
            infer_model_info("text-embedding-3-small", Some("chat")),
            ("embedding".into(), "嵌入".into())
        );
        assert_eq!(
            infer_model_info("bge-reranker-v2", None),
            ("rerank".into(), "重排".into())
        );
    }

    #[test]
    fn preserves_fixed_model_label_overrides() {
        assert_eq!(
            infer_model_info("glm-5", None),
            ("chat".into(), "推理,工具,联网".into())
        );
        assert_eq!(
            infer_model_info("qwen3-235b-a22b", None),
            ("chat".into(), "视觉,推理,工具".into())
        );
    }

    #[test]
    fn remaps_known_provider_names_case_insensitively() {
        assert_eq!(remap_provider_name("chatglm"), "ZHIPU");
        assert_eq!(remap_provider_name("OLLAMA"), "LOCAL");
        assert_eq!(remap_provider_name("newapi"), "HUIZHI");
        assert_eq!(remap_provider_name("custom"), "custom");
    }

    #[test]
    fn tokenizes_provider_names_and_filters_short_tokens() {
        assert_eq!(
            tokenize_and_normalize("ERNIE_DEEP-SEEK AI"),
            vec!["ernie", "deep", "seek"]
        );
    }

    #[test]
    fn matches_provider_by_exact_normalized_name() {
        let matched = match_provider("deep-seek", &providers()).expect("provider should match");
        assert_eq!(matched.0, "DEEPSEEK");
        assert_eq!(matched.2, "L1:名称匹配");
    }

    #[test]
    fn matches_provider_by_alias() {
        let matched = match_provider("google", &providers()).expect("alias should match");
        assert_eq!(matched.0, "GEMINI");
        assert_eq!(matched.2, "L2:别名映射");
    }

    #[test]
    fn matches_provider_by_token_or_containment() {
        let token_match =
            match_provider("ERNIE_DEEPSEEK", &providers()).expect("token should match");
        assert_eq!(token_match.0, "DEEPSEEK");
        assert_eq!(token_match.2, "L2.5:分词匹配");

        let contains_match =
            match_provider("OPENAICompatible", &providers()).expect("substring should match");
        assert_eq!(contains_match.0, "OPENAI");
        assert_eq!(contains_match.2, "L2.5:包含匹配");
    }

    #[test]
    fn returns_none_for_unknown_provider() {
        assert!(match_provider("brand-new-provider", &providers()).is_none());
    }
}
