use tauri::{AppHandle, Emitter, State};

use crate::core::error::AppError;
use crate::core::models::{
    generate_unique_provider_code, normalize_api_key, ProviderDto, ProviderRuntimeConfigDto,
    ProviderSummaryDto, ProviderUpdatePatch, TestResult,
};
use crate::db::repo;
use crate::state::AppState;

#[tauri::command]
pub async fn list_providers(state: State<'_, AppState>) -> Result<Vec<ProviderDto>, AppError> {
    let rows = repo::models::list_providers(&*state.db.pool().await).await?;
    Ok(rows)
}

#[tauri::command]
pub async fn list_provider_summaries(
    state: State<'_, AppState>,
) -> Result<Vec<ProviderSummaryDto>, AppError> {
    Ok(repo::models::list_provider_summaries(&*state.db.pool().await).await?)
}

#[tauri::command]
pub async fn get_provider_runtime_config(
    state: State<'_, AppState>,
    provider_code: String,
) -> Result<ProviderRuntimeConfigDto, AppError> {
    repo::models::get_provider_runtime_config(&*state.db.pool().await, &provider_code)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Provider {provider_code} not found")))
}

#[tauri::command]
pub async fn add_provider(
    state: State<'_, AppState>,
    name: String,
    provider_type: String,
    base_url: String,
    api_key: Option<String>,
    is_local: Option<bool>,
    source_category: Option<String>,
    gateway_id: Option<String>,
) -> Result<String, AppError> {
    let id = uuid::Uuid::new_v4().to_string();

    // 如果有 API Key，直接存储到数据库
    // 注：桌面应用中直接存储足够，若需更高安全性可使用 OS keyring
    let api_key_ref = normalize_api_key(api_key);
    let pool = state.db.pool().await;
    let provider_code = generate_unique_provider_code(&pool, &name).await?;

    repo::models::insert_provider(
        &*pool,
        &repo::models::NewProvider {
            id: id.clone(),
            name,
            provider_type,
            base_url,
            api_key_ref,
            is_local: is_local.unwrap_or(false),
            provider_code: Some(provider_code),
            source_category: Some(source_category.unwrap_or_else(|| "online".to_string())),
            gateway_id,
        },
    )
    .await?;

    Ok(id)
}

#[tauri::command]
pub async fn update_provider(
    state: State<'_, AppState>,
    id: String,
    updates: ProviderUpdatePatch,
) -> Result<(), AppError> {
    repo::models::update_provider(&*state.db.pool().await, &id, &updates).await?;
    Ok(())
}

#[tauri::command]
pub async fn delete_provider(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    repo::models::delete_provider(&*state.db.pool().await, &id).await?;
    Ok(())
}

#[tauri::command]
pub async fn test_connection(
    app: AppHandle,
    state: State<'_, AppState>,
    provider_code: String,
    test_api_url: Option<String>,
    test_api_key: Option<String>,
) -> Result<TestResult, AppError> {
    let provider = repo::models::get_provider(&*state.db.pool().await, &provider_code)
        .await?
        .ok_or_else(|| AppError::NotFound("供应商不存在".to_string()))?;

    // 优先使用前端传入的完整测试地址，否则从数据库读取并拼接 /models
    let request_url =
        test_api_url.unwrap_or_else(|| format!("{}/models", provider.base_url.clone()));

    let api_key = test_api_key.unwrap_or_else(|| {
        provider
            .api_key_ref
            .as_ref()
            .and_then(|ref_id| {
                keyring::Entry::new("fastclaw", ref_id)
                    .ok()
                    .and_then(|e| e.get_password().ok())
            })
            .unwrap_or_default()
    });

    // ===== 阶段1：正在组装请求参数 =====
    emit_log(&app, "info", "[系统] 正在组装请求参数...");

    // ===== 阶段2：校验URL并展示请求信息 =====
    emit_log(&app, "info", "[请求] 请求方式: GET");
    emit_log(&app, "info", &format!("[请求] 请求地址: {}", request_url));

    // URL 格式校验
    if request_url.trim().is_empty() {
        emit_log(&app, "error", "[错误] 接口地址不能为空");
        return Ok(TestResult {
            success: false,
            latency_ms: 0,
            message: "接口地址不能为空".to_string(),
        });
    }
    let lower_url = request_url.to_lowercase();
    if !lower_url.starts_with("http://") && !lower_url.starts_with("https://") {
        emit_log(
            &app,
            "error",
            &format!(
                "[错误] 接口地址格式不合法: {} (需要以 http:// 或 https:// 开头)",
                request_url
            ),
        );
        return Ok(TestResult {
            success: false,
            latency_ms: 0,
            message: format!("接口地址格式不合法: {}", request_url),
        });
    }

    // 脱敏展示 API Key
    if api_key.is_empty() {
        emit_log(&app, "warning", "[警告] API 密钥为空，部分服务可能拒绝请求");
    } else {
        let masked = if api_key.len() > 8 {
            format!("{}...{}", &api_key[..4], &api_key[api_key.len() - 4..])
        } else {
            "***".to_string()
        };
        emit_log(
            &app,
            "info",
            &format!("[请求] Authorization: Bearer {}", masked),
        );
    }

    log::info!("[test_connection] provider_code   = {}", provider_code);
    log::info!("[test_connection] provider_name = {}", provider.name);
    log::info!("[test_connection] request_url   = {}", request_url);

    // ===== 阶段3：发送请求（含重试） =====
    emit_log(&app, "info", "[系统] 正在发送请求...");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let start = std::time::Instant::now();

    const MAX_RETRIES: u32 = 2;
    let mut last_error_msg: Option<String> = None;

    let resp = {
        let mut attempt: u32 = 0;
        loop {
            let result = client
                .get(&request_url)
                .header("Authorization", format!("Bearer {}", api_key))
                .send()
                .await;

            match result {
                Ok(response) => break Ok(response),
                Err(e) if attempt < MAX_RETRIES && is_retryable(&e) => {
                    attempt += 1;
                    let delay_ms = 500 * attempt;
                    let err_msg = classify_network_error(&e);
                    emit_log(
                        &app,
                        "warning",
                        &format!(
                            "[重试] 第 {}/{} 次失败，{}ms 后重试... ({})",
                            attempt, MAX_RETRIES, delay_ms, err_msg
                        ),
                    );
                    last_error_msg = Some(err_msg);
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms as u64)).await;
                }
                Err(e) => {
                    last_error_msg = Some(classify_network_error(&e));
                    break Err(());
                }
            }
        }
    };

    let latency = start.elapsed().as_millis() as i64;

    match resp {
        Ok(r) => {
            let status_code = r.status().as_u16();
            emit_log(&app, "info", &format!("[响应] 状态码: {}", status_code));
            emit_log(&app, "info", &format!("[响应] 耗时: {}ms", latency));

            if r.status().is_success() {
                log::info!("[test_connection] ✅ 成功, latency={}ms", latency);
                emit_log(&app, "success", "[成功] 供应商接口连通性测试通过！");
                Ok(TestResult {
                    success: true,
                    latency_ms: latency,
                    message: "连接成功".to_string(),
                })
            } else if status_code == 401 || status_code == 403 {
                let _body = r.text().await.unwrap_or_default();
                log::error!("[test_connection] ❌ 认证失败 HTTP {}", status_code);
                emit_log(
                    &app,
                    "error",
                    &format!(
                        "[失败] 认证失败 (HTTP {})，请检查 API 密钥是否正确",
                        status_code
                    ),
                );
                Ok(TestResult {
                    success: false,
                    latency_ms: latency,
                    message: format!("认证失败 (HTTP {})，请检查 API 密钥", status_code),
                })
            } else {
                let status = r.status();
                let body = r
                    .text()
                    .await
                    .unwrap_or_else(|_| "<读取body失败>".to_string());
                log::error!("[test_connection] ❌ HTTP {} | body = {}", status, body);
                emit_log(&app, "error", &format!("[失败] HTTP {} | {}", status, body));
                Ok(TestResult {
                    success: false,
                    latency_ms: latency,
                    message: format!("HTTP {} | {}", status, body),
                })
            }
        }
        Err(()) => {
            let err_msg = last_error_msg.unwrap_or_else(|| "未知网络错误".to_string());
            log::error!("[test_connection] ❌ 请求失败: {}", err_msg);
            emit_log(&app, "error", &format!("[失败] {}", err_msg));
            Ok(TestResult {
                success: false,
                latency_ms: latency,
                message: err_msg,
            })
        }
    }
}

// ===== 连接测试辅助函数 =====

/// 推送一条连接测试日志事件到前端
fn emit_log(app: &AppHandle, level: &str, message: &str) {
    let _ = app.emit(
        "conn-test-log",
        serde_json::json!({
            "level": level,
            "message": message,
            "timestamp_ms": chrono::Utc::now().timestamp_millis(),
        }),
    );
}

/// 将 reqwest 网络错误分类为中文友好提示
fn classify_network_error(err: &reqwest::Error) -> String {
    let msg = err.to_string().to_lowercase();
    if err.is_timeout() {
        "连接超时 (10s)，请检查网络或目标服务是否可访问".to_string()
    } else if msg.contains("dns") || msg.contains("resolve") || msg.contains("no such host") {
        format!("无法解析该域名，请检查接口地址是否正确")
    } else if err.is_connect() || msg.contains("connection refused") {
        format!("连接被拒绝，目标服务可能未启动")
    } else if msg.contains("ssl") || msg.contains("tls") || msg.contains("certificate") {
        format!("SSL/TLS 证书验证失败，请检查接口地址")
    } else {
        format!("网络请求失败: {}", err)
    }
}

/// 判断网络错误是否应触发重试
fn is_retryable(err: &reqwest::Error) -> bool {
    if err.is_timeout() || err.is_connect() {
        return true;
    }
    let msg = err.to_string().to_lowercase();
    msg.contains("dns")
        || msg.contains("resolve")
        || msg.contains("connection reset")
        || msg.contains("timed out")
}
