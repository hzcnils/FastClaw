// FastClaw · HTTP REST API 服务器（精简版 v3）
// 知识库接口由动态启动的 FastKG.exe 提供，实际端口由依赖服务管理器分配
// 本文件仅保留非知识库路由：Shimmy 代理、模型列表、配置管理

use crate::core::config::ConfigManager;
use crate::db::Database;
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Extension, OriginalUri, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Json},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

use std::sync::{Arc, OnceLock};
use tower_http::cors::{AllowHeaders, CorsLayer};

/// 进程内复用的 HTTP 客户端（连接池 / TLS 会话复用）
fn shared_http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}
// ─── 响应包装 ───────────────────────────────────────────────

#[derive(Serialize)]
struct ApiSuccess<T: Serialize> {
    success: bool,
    data: T,
}

#[derive(Serialize)]
struct ApiError {
    success: bool,
    message: String,
}

type AppResult<T> = Result<(StatusCode, Json<ApiSuccess<T>>), (StatusCode, Json<ApiError>)>;

fn api_ok<T: Serialize>(data: T) -> AppResult<T> {
    Ok((
        StatusCode::OK,
        Json(ApiSuccess {
            success: true,
            data,
        }),
    ))
}

fn api_err(status: StatusCode, msg: &str) -> (StatusCode, Json<ApiError>) {
    (
        status,
        Json(ApiError {
            success: false,
            message: msg.to_string(),
        }),
    )
}

/// 文件上传最大限制：100 MB
const MAX_BODY_SIZE: usize = 100 * 1024 * 1024;

pub fn create_router(config_manager: Arc<ConfigManager>) -> Router<Database> {
    Router::new()
        .route("/FastPlan/*path", axum::routing::any(fastplan_proxy))
        .route("/newApi/*path", axum::routing::any(newapi_proxy))
        .route("/*path", axum::routing::any(dispatch))
        .layer(axum::middleware::from_fn(recovery_retry_after))
        .layer(DefaultBodyLimit::max(MAX_BODY_SIZE))
        // permissive() 的 Allow-Headers: * 在请求带 Authorization 时会被浏览器当成字面量 `*`，
        // 从而拒绝 X-FastKG-Service-Key。回显预检请求头才能放行自定义头。
        .layer(CorsLayer::permissive().allow_headers(AllowHeaders::mirror_request()))
        .layer(Extension(config_manager))
}

async fn recovery_retry_after(request: axum::extract::Request, next: axum::middleware::Next) -> axum::response::Response {
    let mut response = next.run(request).await;
    if response.status() == StatusCode::SERVICE_UNAVAILABLE {
        response.headers_mut().entry(axum::http::header::RETRY_AFTER)
            .or_insert(axum::http::HeaderValue::from_static("5"));
    }
    response
}

// ─── 通用路由分发 ───────────────────────────────────────────

async fn dispatch(
    method: Method,
    OriginalUri(uri): OriginalUri,
    State(state): State<Database>,
    Extension(config_manager): Extension<Arc<ConfigManager>>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Value> {
    let path = uri.path();
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if parts.len() < 3 || parts[0] != "api" || parts[1] != "v1" {
        return Err(api_err(StatusCode::NOT_FOUND, "not found"));
    }

    let resource = parts[2];

    match resource {
        "llm" if parts.get(3) == Some(&"models") => list_models(&state).await,
        "config" | "shimmy" | "proxy" => {
            let body_str = std::str::from_utf8(&body).map_err(|e| {
                api_err(
                    StatusCode::BAD_REQUEST,
                    &format!("Request body didn't contain valid UTF-8: {}", e),
                )
            })?;
            match resource {
                "config" => handle_config(&method, &state, &parts[3..], body_str).await,
                "shimmy" => handle_shimmy_proxy(&method, body_str, &config_manager).await,
                "proxy" => handle_http_proxy(&method, body_str).await,
                _ => unreachable!(),
            }
        }
        // /api/v1/health → 改写路径代理到 FastKG /health（避免前端直接跨域访问 FastKG）
        "health" => {
            proxy_to_fastkg_path(&method, "/health", &headers, &body, &config_manager).await
        }
        // 未匹配的路由 → 透明代理到 FastKG
        _ => proxy_to_fastkg(&method, &uri, &headers, &body, &config_manager).await,
    }
}

// ─── 模型列表 ───────────────────────────────────────────────

async fn list_models(state: &Database) -> AppResult<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, bool, bool)>(
        "SELECT id, model_id, display_name, model_type, is_enabled, is_default FROM models ORDER BY sort_order"
    )
    .fetch_all(&*state.pool().await)
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, &format!("查询模型失败: {}", e)))?;
    let models: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.0, "model_name": r.1, "display_name": r.2,
                "model_type": r.3, "is_active": r.4, "is_default": r.5,
                "created_at": "", "updated_at": "",
            })
        })
        .collect();
    api_ok(json!(models))
}

// ─── 配置管理 ───────────────────────────────────────────────

async fn handle_config(
    method: &Method,
    state: &Database,
    rest: &[&str],
    body: &str,
) -> AppResult<Value> {
    match method {
        &Method::GET => {
            if rest.is_empty() || (rest.len() == 1 && rest[0].is_empty()) {
                return api_ok(json!({}));
            }
            if rest[0] == "groups" {
                return api_ok(json!([] as [&str; 0]));
            }
            Err(api_err(StatusCode::NOT_FOUND, "not found"))
        }
        &Method::PUT => {
            if rest.len() >= 2 {
                let group = rest[0];
                let key = rest[1];
                if body.is_empty() {
                    return Err(api_err(StatusCode::BAD_REQUEST, "missing body"));
                }
                let parsed: Value = serde_json::from_str(body).map_err(|e| {
                    api_err(StatusCode::BAD_REQUEST, &format!("解析 body 失败: {}", e))
                })?;
                let val = parsed
                    .get("schema")
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                sqlx::query(
                    "INSERT OR REPLACE INTO settings (key, value, category) VALUES (?, ?, ?)",
                )
                .bind(format!("{}.{}", group, key))
                .bind(val)
                .bind(group)
                .execute(&*state.pool().await)
                .await
                .map_err(|e| {
                    api_err(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        &format!("更新配置失败: {}", e),
                    )
                })?;
                return api_ok(json!({}));
            }
            Err(api_err(StatusCode::BAD_REQUEST, "invalid config path"))
        }
        _ => Err(api_err(
            StatusCode::METHOD_NOT_ALLOWED,
            "method not allowed",
        )),
    }
}

// ─── Shimmy 代理 ──────────────────────────────────────────────

async fn handle_shimmy_proxy(
    method: &Method,
    body: &str,
    config_manager: &ConfigManager,
) -> AppResult<Value> {
    let shimmy_base_url = &config_manager.read().await.services.shimmy;

    let client = shared_http_client();

    match method {
        &Method::POST => {
            if body.is_empty() {
                return Err(api_err(StatusCode::BAD_REQUEST, "missing body"));
            }
            let body_json: Value = serde_json::from_str(body)
                .map_err(|e| api_err(StatusCode::BAD_REQUEST, &format!("解析 body 失败: {}", e)))?;

            let endpoints = [
                "/v1/chat/completions",
                "/api/generate",
                "/healthe",
                "/v1/models",
            ];

            for endpoint in endpoints.iter() {
                let full_url = format!("{}{}", shimmy_base_url, endpoint);
                tracing::info!("[Shimmy Proxy] 尝试连接: {}", full_url);

                let response = client
                    .post(&full_url)
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer dummy")
                    .json(&body_json)
                    .send()
                    .await;

                match response {
                    Ok(response) => {
                        tracing::info!(
                            "[Shimmy Proxy] 端点 {} 返回状态码: {}",
                            endpoint,
                            response.status()
                        );
                        if response.status().is_success() {
                            let data: Value = response.json().await.map_err(|e| {
                                api_err(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    &format!("解析响应失败: {}", e),
                                )
                            })?;
                            tracing::info!("[Shimmy Proxy] 请求成功");
                            return api_ok(data);
                        } else {
                            let error_text = response.text().await.unwrap_or_default();
                            tracing::warn!(
                                "[Shimmy Proxy] 端点 {} 返回错误: {}",
                                endpoint,
                                error_text
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!("[Shimmy Proxy] 无法连接到端点 {}: {}", endpoint, e);
                        continue;
                    }
                }
            }

            tracing::error!("[Shimmy Proxy] 所有端点都不可用");
            return Err(api_err(
                StatusCode::BAD_GATEWAY,
                "无法连接到 Shimmy 服务或所有端点都不可用",
            ));
        }
        &Method::GET => {
            let response = client
                .get(format!("{}/v1/models", shimmy_base_url))
                .header("Authorization", "Bearer dummy")
                .send()
                .await
                .map_err(|e| {
                    api_err(StatusCode::BAD_GATEWAY, &format!("连接 Shimmy 失败: {}", e))
                })?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                return Err(api_err(status, &format!("Shimmy 服务错误: {}", error_text)));
            }

            let data: Value = response.json().await.map_err(|e| {
                api_err(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("解析响应失败: {}", e),
                )
            })?;

            api_ok(data)
        }
        _ => Err(api_err(
            StatusCode::METHOD_NOT_ALLOWED,
            "method not allowed",
        )),
    }
}

// ─── 通用 HTTP 代理（解决跨域问题） ─────────────────────────────

#[derive(Deserialize)]
struct ProxyRequest {
    url: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    body: Option<Value>,
}

fn default_method() -> String {
    "GET".to_string()
}

#[derive(Serialize)]
struct ProxyResponse {
    status: u16,
    headers: HashMap<String, String>,
    body: Value,
}

async fn handle_http_proxy(method: &Method, body: &str) -> AppResult<Value> {
    if *method != Method::POST {
        return Err(api_err(StatusCode::METHOD_NOT_ALLOWED, "仅支持 POST"));
    }

    if body.is_empty() {
        return Err(api_err(StatusCode::BAD_REQUEST, "missing body"));
    }
    let req: ProxyRequest = serde_json::from_str(body)
        .map_err(|e| api_err(StatusCode::BAD_REQUEST, &format!("解析请求失败: {}", e)))?;

    let client = shared_http_client();
    let target_method =
        reqwest::Method::from_bytes(req.method.as_bytes()).unwrap_or(reqwest::Method::GET);

    let mut request = client.request(target_method, &req.url);

    for (key, value) in &req.headers {
        request = request.header(key.as_str(), value.as_str());
    }

    if let Some(ref json_body) = req.body {
        request = request.json(json_body);
    }

    let response = request
        .send()
        .await
        .map_err(|e| api_err(StatusCode::BAD_GATEWAY, &format!("代理请求失败: {}", e)))?;

    let status = response.status().as_u16();
    let resp_headers: HashMap<String, String> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let resp_body: Value = response.json().await.unwrap_or(Value::Null);

    api_ok(json!(ProxyResponse {
        status,
        headers: resp_headers,
        body: resp_body,
    }))
}

// ─── FastPlan 透明代理（/FastPlan/* → FastPlan 端口） ─────────────────

async fn fastplan_proxy(
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: axum::http::HeaderMap,
    Extension(config_manager): Extension<Arc<ConfigManager>>,
    body: Bytes,
) -> axum::response::Response {
    let configured = config_manager.read().await.services.fastplan.clone();
    let fastplan_url = crate::commands::wsp::dependency_effective_url("fastplan", &configured);
    let path_and_query = uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or(uri.path());
    let stripped = path_and_query
        .strip_prefix("/FastPlan")
        .unwrap_or(path_and_query);
    let target_url = format!("{}{}", fastplan_url, stripped);

    if stripped.split('?').next() == Some("/api/plan") {
        return (StatusCode::GONE, "旧规划入口已停用，请使用已鉴权的会话连接").into_response();
    }

    let body_bytes = body;

    let client = reqwest::ClientBuilder::new()
        .no_proxy()
        .http1_only()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap_or_default();
    let reqwest_method =
        reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET);

    let mut req = client.request(reqwest_method, &target_url);
    for (key, value) in &headers {
        let k = key.as_str();
        if k != "host" && k != "content-length" {
            if let Ok(v) = value.to_str() {
                req = req.header(k, v);
            }
        }
    }
    if !body_bytes.is_empty() {
        req = req.body(body_bytes);
    }

    match req.send().await {
        Ok(resp) => {
            let status = axum::http::StatusCode::from_u16(resp.status().as_u16())
                .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
            let resp_headers = resp.headers().clone();
            let body_bytes = resp.bytes().await.unwrap_or_default();
            let mut builder = axum::response::Response::builder().status(status);
            for (key, value) in &resp_headers {
                if key.as_str() != "transfer-encoding" {
                    builder = builder.header(key.as_str(), value);
                }
            }
            builder
                .body(axum::body::Body::from(body_bytes))
                .unwrap_or_else(|_| {
                    axum::response::Response::builder()
                        .status(500)
                        .body(axum::body::Body::empty())
                        .unwrap()
                })
        }
        Err(e) => {
            tracing::warn!("[FastPlan Proxy] 请求失败 ({}): {}", target_url, e);
            axum::response::Response::builder()
                .status(502)
                .body(axum::body::Body::from(format!(
                    "FastPlan 服务不可用: {}",
                    e
                )))
                .unwrap()
        }
    }
}

// ─── FastKG 透明代理（兜底：未识别路由 → 转发到 FastKG 端口） ──────

/// 代理到 FastKG 的固定路径（用于需要路径改写的场景，如 /health）
async fn proxy_to_fastkg_path(
    method: &Method,
    path: &str,
    headers: &HeaderMap,
    body: &Bytes,
    config_manager: &ConfigManager,
) -> AppResult<Value> {
    let configured = config_manager.read().await.services.fastkg.clone();
    let fastkg_url = crate::commands::wsp::dependency_effective_url("fastkg", &configured);
    let target_url = format!("{}{}", fastkg_url, path);
    fastkg_request(method, &target_url, headers, body, false).await
}

async fn proxy_to_fastkg(
    method: &Method,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    body: &Bytes,
    config_manager: &ConfigManager,
) -> AppResult<Value> {
    let configured = config_manager.read().await.services.fastkg.clone();
    let fastkg_url = crate::commands::wsp::dependency_effective_url("fastkg", &configured);
    let path_and_query = uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or(uri.path());
    let target_url = format!("{}{}", fastkg_url, path_and_query);
    fastkg_request(method, &target_url, headers, body, true).await
}

/// 统一的 FastKG HTTP 请求实现
/// - unwrap_data: true 时对 {success:true, data:...} 结构拆包（避免双重包装）
/// - GET 最多重试一次（间隔 2s）；写请求不自动重放，恢复期间返回 503。
async fn fastkg_request(
    method: &Method,
    target_url: &str,
    headers: &HeaderMap,
    body: &Bytes,
    unwrap_data: bool,
) -> AppResult<Value> {
    // 写操作绝不重放；恢复交给应用级管理器，调用方收到 503 后自行决定是否重试。
    let max_retries: u32 = if *method == Method::GET { 1 } else { 0 };
    const RETRY_DELAY_MS: u64 = 2000;

    match crate::kg_coordination::read_registry() {
        Ok(Some(registry)) if registry.state != "ready" => {
            return Err(api_err(StatusCode::SERVICE_UNAVAILABLE, "知识库服务尚未就绪或正在恢复，请稍后重试"));
        }
        Err(_) => return Err(api_err(StatusCode::SERVICE_UNAVAILABLE, "知识库共享状态暂不可用，请稍后重试")),
        _ => {}
    }

    let client = reqwest::ClientBuilder::new()
        .no_proxy()
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .http1_only()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default();
    let reqwest_method =
        reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET);

    // 带重试的请求执行：连接拒绝/超时时自动重试，适应 KG 服务冷启动
    let mut last_err: Option<String> = None;
    for attempt in 0..=max_retries {
        if attempt > 0 {
            tracing::info!(
                "[FastKG Proxy] 重试 {}/{} ({}): 等待 {}ms...",
                attempt,
                max_retries,
                target_url,
                RETRY_DELAY_MS
            );
            tokio::time::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS)).await;
        }

        // 每次重试都重新构建 request（避免 body 不可 clone 的问题）
        let retry_request =
            build_fastkg_request(&client, &reqwest_method, target_url, headers, body);
        let retry_built = match retry_request {
            Ok(req) => req,
            Err(e) => return Err(e),
        };

        match client.execute(retry_built).await {
            Ok(response) => {
                let http_status = response.status();
                let resp_body = response.text().await.unwrap_or_default();

                if !http_status.is_success() {
                    tracing::warn!(
                        "[FastKG Proxy] {} 返回 {}: {}",
                        target_url,
                        http_status.as_u16(),
                        resp_body.chars().take(200).collect::<String>()
                    );
                    let axum_status = StatusCode::from_u16(http_status.as_u16())
                        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                    let msg = if resp_body.is_empty() {
                        format!("FastKG 返回 {}", http_status.as_u16())
                    } else {
                        resp_body.chars().take(200).collect::<String>()
                    };
                    // 503 (Service Unavailable) → KG 可能还在初始化，重试
                    if http_status.as_u16() == 503 && attempt < max_retries {
                        last_err = Some(msg);
                        continue;
                    }
                    return Err(api_err(axum_status, &msg));
                }

                match serde_json::from_str::<Value>(&resp_body) {
                    Ok(Value::Object(ref obj))
                        if unwrap_data
                            && obj.get("success").and_then(|v| v.as_bool()) == Some(true) =>
                    {
                        let inner = obj.get("data").cloned().unwrap_or(Value::Null);
                        return api_ok(inner);
                    }
                    Ok(json_val) => return api_ok(json_val),
                    Err(_) => return api_ok(Value::String(resp_body)),
                }
            }
            Err(e) => {
                let err_msg = e.to_string();
                tracing::warn!(
                    "[FastKG Proxy] 请求失败 ({}): {} (attempt {}/{})",
                    target_url,
                    err_msg,
                    attempt + 1,
                    max_retries + 1
                );

                // 判断是否为可重试的错误（连接超时、连接拒绝、DNS 解析失败）
                let retryable = err_msg.contains("timeout")
                    || err_msg.contains("refused")
                    || err_msg.contains("connect")
                    || err_msg.contains("dns");

                if retryable && attempt < max_retries {
                    last_err = Some(err_msg);
                    continue;
                }

                // 重试耗尽或不可重试错误
                if last_err.is_some() {
                    return Err(api_err(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "知识库服务正在启动中，请稍后重试",
                    ));
                }
                return Err(api_err(
                    StatusCode::BAD_GATEWAY,
                    &format!("FastKG 服务不可用: {}", e),
                ));
            }
        }
    }

    // 重试耗尽（不应到达这里，但编译器需要）
    Err(api_err(
        StatusCode::SERVICE_UNAVAILABLE,
        "知识库服务正在启动中，请稍后重试",
    ))
}

/// 构建到 FastKG 的 HTTP 请求（提取为独立函数，供重试循环使用）
fn build_fastkg_request(
    client: &reqwest::Client,
    method: &reqwest::Method,
    target_url: &str,
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<reqwest::Request, (StatusCode, Json<ApiError>)> {
    let mut request = client
        .request(method.clone(), target_url)
        .header("Accept", "application/json")
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .header(
            crate::core::constants::FASTKG_SERVICE_KEY_HEADER,
            crate::core::constants::FASTKG_SERVICE_KEY,
        );

    if !body.is_empty() {
        let content_type = headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/json");
        request = request
            .header("Content-Type", content_type)
            .body(body.clone());
    }

    request.build().map_err(|e| {
        api_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("构建请求失败: {}", e),
        )
    })
}

// ── NewApi 反向代理 ─────────────────────────────────────────

async fn newapi_proxy(
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: axum::http::HeaderMap,
    Extension(config_manager): Extension<Arc<ConfigManager>>,
    body: Bytes,
) -> axum::response::Response {
    let model_sync_url = config_manager
        .read()
        .await
        .remote_servers
        .model_sync_url
        .clone();
    let path_and_query = uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or(uri.path());
    let stripped = path_and_query
        .strip_prefix("/newApi")
        .unwrap_or(path_and_query);
    let target_url = format!("{}{}", model_sync_url, stripped);

    let client = reqwest::ClientBuilder::new()
        .no_proxy()
        .build()
        .unwrap_or_default();
    let reqwest_method =
        reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET);

    let mut req = client.request(reqwest_method, &target_url);
    for (key, value) in &headers {
        let k = key.as_str();
        if k != "host" && k != "content-length" {
            if let Ok(v) = value.to_str() {
                req = req.header(k, v);
            }
        }
    }
    if !body.is_empty() {
        req = req.body(body);
    }

    match req.send().await {
        Ok(resp) => {
            let status = axum::http::StatusCode::from_u16(resp.status().as_u16())
                .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
            let resp_headers = resp.headers().clone();
            let body_bytes = resp.bytes().await.unwrap_or_default();
            let mut builder = axum::response::Response::builder().status(status);
            for (key, value) in &resp_headers {
                if key.as_str() != "transfer-encoding" {
                    builder = builder.header(key.as_str(), value);
                }
            }
            builder
                .body(axum::body::Body::from(body_bytes))
                .unwrap_or_else(|_| {
                    axum::response::Response::builder()
                        .status(500)
                        .body(axum::body::Body::empty())
                        .unwrap()
                })
        }
        Err(e) => {
            tracing::warn!("[NewApi Proxy] 请求失败 ({}): {}", target_url, e);
            axum::response::Response::builder()
                .status(502)
                .body(axum::body::Body::from(format!("NewApi 服务不可用: {}", e)))
                .unwrap()
        }
    }
}
