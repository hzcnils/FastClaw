// FastClaw · WSP 云服务厂商 Tauri Commands
// 「对接云服务厂商」弹窗的配置读写，以及云端手写体识别调用（阿里云 / 讯飞云）

use crate::core::error::AppError;
use crate::db::repo;
use crate::db::repo::cloud_providers::WspCloudProvider;
use crate::state::AppState;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::Sha256;
use tauri::State;

/// 各厂商的静态信息
struct ProviderMeta {
    name: &'static str,
    /// 服务地址；讯飞云的地址随页面类型变化（见 OcrServiceKind::xfyun_default_endpoint）
    default_endpoint: Option<&'static str>,
    default_region: Option<&'static str>,
}

fn provider_meta(code: &str) -> Option<ProviderMeta> {
    match code {
        "aliyun" => Some(ProviderMeta {
            name: "阿里云",
            default_endpoint: Some("https://ocr-api.cn-hangzhou.aliyuncs.com"),
            default_region: Some("cn-hangzhou"),
        }),
        "xfyun" => Some(ProviderMeta {
            name: "讯飞云",
            default_endpoint: None,
            default_region: None,
        }),
        _ => None,
    }
}

/// 云 OCR 服务类型：手写体识别 / 通用文字识别（其余页面按通用文字处理）
///
/// 由页面（二级分类名）决定调用哪个接口，两套接口的域名与鉴权方式都不同：
/// - 手写体识别：讯飞云手写文字识别 WebAPI（AppId + APIKey，X-CheckSum 校验）
/// - 通用文字识别：讯飞云通用文字识别 sf8e6aca1（APIKey + APISecret，hmac-sha256 签名）
#[derive(Clone, Copy, PartialEq)]
enum OcrServiceKind {
    Handwriting,
    General,
}

impl OcrServiceKind {
    fn from_sub_category(sub_category_name: Option<&str>) -> Self {
        match sub_category_name.map(str::trim) {
            Some(sub) if !sub.is_empty() && sub.contains("手写") => Self::Handwriting,
            _ => Self::General,
        }
    }

    /// 阿里云 OCR 的 Action
    fn aliyun_action(self) -> &'static str {
        match self {
            Self::Handwriting => "RecognizeHandwriting",
            Self::General => "RecognizeGeneral",
        }
    }

    /// 讯飞云服务地址（两类接口域名不同；不读取用户配置里的 endpoint，避免残留旧值）
    fn xfyun_endpoint(self) -> &'static str {
        match self {
            Self::Handwriting => "https://webapi.xfyun.cn/v1/service/v1/ocr/handwriting",
            Self::General => "https://api.xf-yun.com/v1/private/sf8e6aca1",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SaveCloudProviderInput {
    pub provider_code: String,
    pub display_name: Option<String>,
    pub access_key_id: Option<String>,
    pub access_key_secret: Option<String>,
    pub api_key: Option<String>,
    pub endpoint: Option<String>,
    pub region: Option<String>,
    pub is_enabled: Option<bool>,
    /// 当前页面所属分类（由父容器按菜单注入）：一级短码 / 一级名 / 二级名
    pub category: Option<String>,
    pub category_name: Option<String>,
    pub sub_category_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SaveCloudProviderResult {
    pub provider: WspCloudProvider,
}

/// 去掉首尾空白，空串视为未提供
fn normalize_opt(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// 云模型在 wsp_services 中的 id：按「厂商 + 二级分类（页面）」区分，
/// 这样同一厂商可以在不同页面各自配置一个模型。
fn service_id_for(provider_code: &str, sub_category_name: &str) -> String {
    format!("cloud-{}-{}", provider_code, sub_category_name.trim())
}

fn merge_str(input: Option<String>, existing: Option<String>, default: &str) -> Option<String> {
    match input {
        Some(v) => Some(v.trim().to_string()),
        None => {
            if let Some(e) = existing {
                Some(e)
            } else if default.is_empty() {
                None
            } else {
                Some(default.to_string())
            }
        }
    }
}

/// 列出当前页面（二级分类）下的云服务厂商配置（阿里云 / 讯飞云）
/// 各页面的凭据独立、互不共用；未配置过的厂商不会返回，由前端按默认值回显
#[tauri::command]
pub async fn list_cloud_providers(
    state: State<'_, AppState>,
    sub_category_name: Option<String>,
) -> Result<Vec<WspCloudProvider>, AppError> {
    let pool = &*state.db.pool().await;
    let sub = normalize_opt(sub_category_name).unwrap_or_default();
    let mut list = repo::cloud_providers::list_by_page(pool, &sub).await?;

    for provider in list.iter_mut() {
        let service_id = service_id_for(&provider.provider_code, &sub);
        // 模型名以该页面已保存的 WSP 服务为准；尚无配置时返回空串，
        // 由前端按「厂商-页面名」生成默认名
        provider.display_name = query_service_display_name(pool, &service_id)
            .await?
            .unwrap_or_default();
    }
    Ok(list)
}

/// 读取某条 WSP 服务的显示名
async fn query_service_display_name(
    pool: &sqlx::SqlitePool,
    service_id: &str,
) -> Result<Option<String>, AppError> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT display_name FROM wsp_services WHERE id = ?")
            .bind(service_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    Ok(row.map(|(v,)| v).filter(|v| !v.trim().is_empty()))
}

/// 保存（新增或修改）云服务厂商配置，并同步到 wsp_services 以便在模型下拉中回显
#[tauri::command]
pub async fn save_cloud_provider(
    state: State<'_, AppState>,
    input: SaveCloudProviderInput,
) -> Result<SaveCloudProviderResult, AppError> {
    let pool = &*state.db.pool().await;
    let code = input.provider_code.trim().to_lowercase();
    let meta = provider_meta(&code)
        .ok_or_else(|| AppError::InvalidInput(format!("不支持的云服务厂商: {}", input.provider_code)))?;

    // 页面作用域由父容器按当前菜单注入；配置按「厂商 + 页面」独立存储
    let sub_category_name = normalize_opt(input.sub_category_name)
        .unwrap_or_else(|| "文字识别".to_string());
    let existing =
        repo::cloud_providers::get_by_code_and_page(pool, &code, &sub_category_name).await?;

    let category = normalize_opt(input.category)
        .or_else(|| existing.as_ref().map(|e| e.category.clone()))
        .unwrap_or_else(|| "hr".to_string());
    let category_name = normalize_opt(input.category_name)
        .or_else(|| existing.as_ref().map(|e| e.category_name.clone()))
        .unwrap_or_else(|| "文字识别".to_string());

    let default_display_name = format!("{}-{}", meta.name, sub_category_name);
    // 服务地址按当前页面类型（手写体 / 通用文字）取；讯飞云两类接口域名不同，不沿用配置里的旧值
    let kind = OcrServiceKind::from_sub_category(Some(&sub_category_name));
    let endpoint = if code == "xfyun" {
        kind.xfyun_endpoint().to_string()
    } else {
        meta.default_endpoint.unwrap_or_default().to_string()
    };

    let provider = WspCloudProvider {
        // id 与 wsp_services.id 一致：cloud-{厂商}-{页面}
        id: service_id_for(&code, &sub_category_name),
        provider_code: code.clone(),
        name: meta.name.to_string(),
        display_name: merge_str(
            input.display_name,
            existing.as_ref().map(|e| e.display_name.clone()),
            &default_display_name,
        )
        .filter(|v| !v.is_empty())
        .unwrap_or(default_display_name),
        access_key_id: merge_str(
            input.access_key_id,
            existing.as_ref().and_then(|e| e.access_key_id.clone()),
            "",
        ),
        access_key_secret: merge_str(
            input.access_key_secret,
            existing.as_ref().and_then(|e| e.access_key_secret.clone()),
            "",
        ),
        api_key: merge_str(
            input.api_key,
            existing.as_ref().and_then(|e| e.api_key.clone()),
            "",
        ),
        endpoint,
        region: merge_str(
            input.region,
            existing.as_ref().and_then(|e| e.region.clone()),
            meta.default_region.unwrap_or(""),
        ),
        category,
        category_name,
        sub_category_name,
        is_enabled: input
            .is_enabled
            .unwrap_or_else(|| existing.as_ref().map(|e| e.is_enabled).unwrap_or(false)),
        extra_config: existing.as_ref().and_then(|e| e.extra_config.clone()),
        created_at: existing
            .as_ref()
            .map(|e| e.created_at.clone())
            .unwrap_or_default(),
        updated_at: String::new(),
    };

    repo::cloud_providers::upsert(pool, &provider).await?;
    sync_wsp_service(pool, &provider).await?;

    let saved = repo::cloud_providers::get_by_code_and_page(pool, &code, &provider.sub_category_name)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("云服务厂商配置保存失败: {code}")))?;

    tracing::info!(
        "[cloud_providers] 已保存 {}-{}（enabled={}）",
        saved.provider_code,
        saved.sub_category_name,
        saved.is_enabled
    );
    Ok(SaveCloudProviderResult { provider: saved })
}

/// 把云服务厂商同步为一条 WSP 服务记录（模型下拉的数据源）
async fn sync_wsp_service(
    pool: &sqlx::SqlitePool,
    provider: &WspCloudProvider,
) -> Result<(), AppError> {
    let (protocol, host, port, base_path) = split_endpoint(&provider.endpoint);
    // 按「厂商 + 页面（二级分类）」生成服务记录，避免多页面互相覆盖
    let service_id = service_id_for(&provider.provider_code, &provider.sub_category_name);
    sqlx::query(
        r#"
        INSERT INTO wsp_services
            (id, name, display_name, description, exe_path, args, category, categoryName, subCategoryName,
             version, protocol, host, port, base_path, auto_start, is_enabled, is_installed, sort_order, extra_config)
        VALUES (?, ?, ?, ?, '', '', ?, ?, ?, '1.0.0', ?, ?, ?, ?, 0, ?, 1, 90, NULL)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            display_name = excluded.display_name,
            description = excluded.description,
            category = excluded.category,
            categoryName = excluded.categoryName,
            subCategoryName = excluded.subCategoryName,
            protocol = excluded.protocol,
            host = excluded.host,
            port = excluded.port,
            base_path = excluded.base_path,
            is_enabled = excluded.is_enabled,
            is_installed = 1,
            updated_at = datetime('now')
        "#,
    )
    .bind(&service_id)
    .bind(&service_id)
    .bind(&provider.display_name)
    .bind(format!(
        "{}云服务厂商（{}）",
        provider.name, provider.sub_category_name
    ))
    .bind(&provider.category)
    .bind(&provider.category_name)
    .bind(&provider.sub_category_name)
    .bind(&protocol)
    .bind(&host)
    .bind(port)
    .bind(&base_path)
    .bind(provider.is_enabled)
    .execute(pool)
    .await
    .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;

    sqlx::query(
        r#"
        INSERT INTO wsp_runtime_status (service_id, status, request_count, error_count, restart_count, last_health_ok, updated_at)
        VALUES (?, 'stopped', 0, 0, 0, 0, datetime('now'))
        ON CONFLICT(service_id) DO NOTHING
        "#,
    )
    .bind(&service_id)
    .execute(pool)
    .await
    .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;

    Ok(())
}

/// 拆分接口地址为 (协议, host 源, 端口, 基础路径)
fn split_endpoint(endpoint: &str) -> (String, String, i32, String) {
    let e = endpoint.trim();
    let (scheme, rest) = if let Some(r) = e.strip_prefix("https://") {
        ("https", r)
    } else if let Some(r) = e.strip_prefix("http://") {
        ("http", r)
    } else {
        ("https", e)
    };
    let default_port = if scheme == "https" { 443 } else { 80 };
    let (host_port, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let (host_name, port) = match host_port.rfind(':') {
        Some(i) => (
            &host_port[..i],
            host_port[i + 1..].parse::<i32>().unwrap_or(default_port),
        ),
        None => (host_port, default_port),
    };
    (
        scheme.to_string(),
        format!("{scheme}://{host_name}:{port}"),
        port,
        path.to_string(),
    )
}

/// 调用云端文字识别接口，返回识别文本
/// `sub_category_name` 为当前页面（二级分类名），决定调用手写体还是通用文字接口
///
/// 返回 `Err(String)`：内容为第三方返回的原始错误信息（不额外包装平台自己的错误文案）
#[tauri::command]
pub async fn cloud_ocr_recognize(
    state: State<'_, AppState>,
    provider_code: String,
    sub_category_name: Option<String>,
    file_name: Option<String>,
    image_base64: String,
) -> Result<String, String> {
    let pool = &*state.db.pool().await;
    let code = provider_code.trim().to_lowercase();
    let sub = sub_category_name
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    let kind = OcrServiceKind::from_sub_category(Some(&sub));
    // 凭据按「厂商 + 页面」独立存储，各页面互不共用
    let cfg = repo::cloud_providers::get_by_code_and_page(pool, &code, &sub)
        .await
        .map_err(|e| format!("读取云服务厂商配置失败: {e}"))?
        .ok_or_else(|| {
            format!("当前页面尚未配置该云服务厂商，请先在「对接云服务厂商」中保存: {code}")
        })?;
    if !cfg.is_enabled {
        return Err("该云服务厂商未启用，请在「对接云服务厂商」中启用后重试".into());
    }

    let image_b64 = image_base64.trim().to_string();
    // 讯飞云 GSLB 多 IP、跨境链路偶发不可达：收紧连接超时，传输层失败由 execute_with_retry 重试
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

    let text = match code.as_str() {
        "aliyun" => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(image_b64.as_bytes())
                .map_err(|e| format!("图片 base64 解码失败: {e}"))?;
            recognize_aliyun(&client, &cfg, &bytes, kind).await?
        }
        "xfyun" => recognize_xfyun(&client, &cfg, &image_b64, kind).await?,
        other => return Err(format!("暂不支持的云服务厂商: {other}")),
    };

    tracing::info!(
        "[cloud_ocr] {} 识别完成 file={:?}，文本长度={}",
        code,
        file_name.unwrap_or_default(),
        text.chars().count()
    );
    Ok(text)
}

/// 传输层失败的重试次数（连接失败 / 超时多为瞬时网络问题）
const OCR_MAX_ATTEMPTS: usize = 3;

/// 展开 reqwest 传输层错误的原因链：
/// `Display` 只给出「error sending request for url (…)」，真正的原因
///（DNS 解析失败 / 连接超时 / TLS 握手失败 / 代理拦截）在 source 里，
/// 这里一并带出，并附上可操作的排查提示。
fn describe_reqwest_error(err: &reqwest::Error) -> String {
    let mut parts = vec![err.to_string()];
    let mut source = std::error::Error::source(err);
    while let Some(inner) = source {
        let text = inner.to_string();
        if !text.is_empty() && !parts.iter().any(|p| p == &text) {
            parts.push(text);
        }
        source = inner.source();
    }
    let detail = parts.join(" ← ");
    let lower = detail.to_lowercase();
    let hint = if lower.contains("timed out") || lower.contains("timeout") {
        "：连接超时，请检查网络或代理是否能访问该接口地址"
    } else if lower.contains("dns") || lower.contains("lookup") || lower.contains("no such host") {
        "：域名解析失败，请检查本机 DNS / 网络"
    } else if lower.contains("certificate") || lower.contains("tls") || lower.contains("handshake") {
        "：TLS 握手或证书校验失败，请检查代理是否拦截了 HTTPS"
    } else if lower.contains("refused") || lower.contains("reset") || lower.contains("closed") {
        "：连接被拒绝或中断，请检查代理 / 防火墙设置"
    } else {
        ""
    };
    format!("{detail}{hint}")
}

/// 发送请求：仅传输层错误（连不上 / 超时）重试，接口返回的 HTTP 错误不重试
async fn execute_with_retry(
    client: &reqwest::Client,
    request: &reqwest::Request,
    label: &str,
) -> Result<reqwest::Response, String> {
    let mut last_error = String::new();
    for attempt in 1..=OCR_MAX_ATTEMPTS {
        // 每次重试都要一份新的请求（Request 只有 try_clone；本模块请求体都在内存中）
        let Some(cloned) = request.try_clone() else {
            return Err(format!("{label}请求无法重试（请求体不可复用）"));
        };
        match client.execute(cloned).await {
            Ok(resp) => return Ok(resp),
            Err(err) => {
                last_error = describe_reqwest_error(&err);
                tracing::warn!(
                    "[cloud_ocr] {} 第 {}/{} 次请求失败: {}",
                    label,
                    attempt,
                    OCR_MAX_ATTEMPTS,
                    last_error
                );
                if attempt < OCR_MAX_ATTEMPTS {
                    tokio::time::sleep(std::time::Duration::from_millis(500 * attempt as u64)).await;
                }
            }
        }
    }
    Err(format!(
        "{label}连接失败（已重试 {OCR_MAX_ATTEMPTS} 次）：{last_error}"
    ))
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let s: String = text.chars().take(max).collect();
    format!("{s}…")
}

/// 第三方返回的原始错误信息：优先取 JSON 里的 message 字段，否则原样返回响应体
fn raw_third_party_error(body: &str) -> String {
    let trimmed = body.trim();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        for key in ["message", "Message", "desc", "error", "error_msg"] {
            if let Some(msg) = value.get(key).and_then(|m| m.as_str()) {
                if !msg.is_empty() {
                    return msg.to_string();
                }
            }
        }
    }
    truncate(trimmed, 300)
}

/// RFC3986 百分号编码（阿里云签名要求）
fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// 阿里云文字识别（RPC 签名，HMAC-SHA1）
/// 手写体识别 Action=RecognizeHandwriting，通用文字识别 Action=RecognizeGeneral
async fn recognize_aliyun(
    client: &reqwest::Client,
    cfg: &WspCloudProvider,
    image: &[u8],
    kind: OcrServiceKind,
) -> Result<String, String> {
    let ak_id = cfg.access_key_id.clone().unwrap_or_default();
    let ak_secret = cfg.access_key_secret.clone().unwrap_or_default();
    if ak_id.trim().is_empty() || ak_secret.trim().is_empty() {
        return Err("请先填写阿里云 AccessKeyId 与 AccessKeySecret".into());
    }
    let endpoint = if cfg.endpoint.trim().is_empty() {
        "https://ocr-api.cn-hangzhou.aliyuncs.com".to_string()
    } else {
        cfg.endpoint.trim().trim_end_matches('/').to_string()
    };

    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let mut params: Vec<(String, String)> = vec![
        ("AccessKeyId".into(), ak_id),
        ("Action".into(), kind.aliyun_action().into()),
        ("Format".into(), "JSON".into()),
        ("SignatureMethod".into(), "HMAC-SHA1".into()),
        ("SignatureNonce".into(), uuid::Uuid::new_v4().to_string()),
        ("SignatureVersion".into(), "1.0".into()),
        ("Timestamp".into(), timestamp),
        ("Version".into(), "2021-07-07".into()),
    ];
    params.sort_by(|a, b| a.0.cmp(&b.0));

    let canonical: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
        .collect();
    let canonical = canonical.join("&");
    let string_to_sign = format!("POST&{}&{}", percent_encode("/"), percent_encode(&canonical));

    let mut mac = Hmac::<Sha1>::new_from_slice(format!("{ak_secret}&").as_bytes())
        .map_err(|e| e.to_string())?;
    mac.update(string_to_sign.as_bytes());
    let signature = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());

    params.push(("Signature".into(), signature));
    let query: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
        .collect();
    let url = format!("{}?{}", endpoint, query.join("&"));

    let request = client
        .post(&url)
        .header("Content-Type", "application/octet-stream")
        .body(image.to_vec())
        .build()
        .map_err(|e| format!("构建阿里云请求失败: {e}"))?;
    let resp = execute_with_retry(client, &request, "阿里云").await?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        // 第三方原始报错
        return Err(raw_third_party_error(&body));
    }

    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("阿里云响应解析失败: {e}；原始响应: {}", truncate(&body, 300)))?;
    if let Some(code) = value.get("Code").and_then(|c| c.as_str()) {
        if !code.is_empty() && code != "200" {
            let message = value
                .get("Message")
                .and_then(|m| m.as_str())
                .unwrap_or_default();
            // 第三方原始报错：Code: Message
            return Err(format!("{code}: {message}"));
        }
    }

    let data = value.get("Data").cloned().unwrap_or(serde_json::Value::Null);
    let mut contents = Vec::new();
    collect_aliyun_content(&data, &mut contents);
    if contents.is_empty() {
        return Err(format!(
            "阿里云响应中未找到识别文本: {}",
            truncate(&body, 300)
        ));
    }
    Ok(contents.join("\n"))
}

/// 兼容 Data 为 JSON 字符串 / 对象两种返回形态，提取识别文本
fn collect_aliyun_content(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
                collect_aliyun_content(&parsed, out);
            } else if !s.trim().is_empty() {
                out.push(s.clone());
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(c) = map.get("content").and_then(|c| c.as_str()) {
                out.push(c.to_string());
                return;
            }
            for key in [
                "prism_wordsInfo",
                "wordsInfo",
                "blocks",
                "lines",
                "block",
                "line",
            ] {
                if let Some(inner) = map.get(key) {
                    collect_aliyun_content(inner, out);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                collect_aliyun_content(item, out);
            }
        }
        _ => {}
    }
}

/// 由图片 base64 首部推断图片编码（通用文字识别要求在 payload 里声明 jpg/jpeg/png/bmp）
fn image_encoding(image_base64: &str) -> &'static str {
    if image_base64.starts_with("iVBORw0KGgo") {
        "png"
    } else if image_base64.starts_with("/9j/") {
        "jpeg"
    } else if image_base64.starts_with("Qk") {
        "bmp"
    } else {
        "jpg"
    }
}

/// 讯飞云文字识别入口：按页面类型分发到手写体（WebAPI）/ 通用文字（签名接口）
async fn recognize_xfyun(
    client: &reqwest::Client,
    cfg: &WspCloudProvider,
    image_base64: &str,
    kind: OcrServiceKind,
) -> Result<String, String> {
    match kind {
        OcrServiceKind::Handwriting => recognize_xfyun_handwriting(client, cfg, image_base64).await,
        OcrServiceKind::General => recognize_xfyun_general(client, cfg, image_base64).await,
    }
}

/// 讯飞云「手写文字识别」WebAPI：请求头带 AppId / CurTime / Param / CheckSum
async fn recognize_xfyun_handwriting(
    client: &reqwest::Client,
    cfg: &WspCloudProvider,
    image_base64: &str,
) -> Result<String, String> {
    // 凭据参与 MD5 计算，必须先去空白：复制粘贴常带首尾空格/换行，会导致 X-CheckSum 校验失败
    let app_id = cfg.access_key_id.clone().unwrap_or_default().trim().to_string();
    let api_key = cfg.api_key.clone().unwrap_or_default().trim().to_string();
    if app_id.is_empty() || api_key.is_empty() {
        return Err("请先填写讯飞云 AppId 与 APIKey".into());
    }

    let cur_time = chrono::Utc::now().timestamp().to_string();
    let param =
        base64::engine::general_purpose::STANDARD.encode(r#"{"language":"cn|en","location":"false"}"#);
    let check_sum = {
        use md5::Digest;
        let mut hasher = md5::Md5::new();
        hasher.update(api_key.as_bytes());
        hasher.update(cur_time.as_bytes());
        hasher.update(param.as_bytes());
        hex::encode(hasher.finalize())
    };

    let request = client
        .post(OcrServiceKind::Handwriting.xfyun_endpoint())
        .header("X-Appid", app_id)
        .header("X-CurTime", cur_time)
        .header("X-Param", param)
        .header("X-CheckSum", check_sum)
        .form(&[("image", image_base64)])
        .build()
        .map_err(|e| format!("构建讯飞云请求失败: {e}"))?;
    let resp = execute_with_retry(client, &request, "讯飞云手写体识别").await?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        // 第三方原始报错
        return Err(raw_third_party_error(&body));
    }

    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("讯飞云响应解析失败: {e}；原始响应: {}", truncate(&body, 300)))?;
    let code = value.get("code").and_then(|c| c.as_str()).unwrap_or("");
    if code != "0" {
        let desc = value
            .get("desc")
            .and_then(|d| d.as_str())
            .unwrap_or_default();
        // 第三方原始报错：code: desc
        return Err(format!("{code}: {desc}"));
    }

    let mut lines = Vec::new();
    if let Some(blocks) = value.pointer("/data/block").and_then(|b| b.as_array()) {
        for block in blocks {
            let Some(block_lines) = block.get("line").and_then(|l| l.as_array()) else {
                continue;
            };
            for line in block_lines {
                let mut text = String::new();
                if let Some(words) = line.get("word").and_then(|w| w.as_array()) {
                    for word in words {
                        if let Some(content) = word.get("content").and_then(|c| c.as_str()) {
                            text.push_str(content);
                        }
                    }
                }
                if !text.is_empty() {
                    lines.push(text);
                }
            }
        }
    }
    if lines.is_empty() {
        return Err(format!(
            "讯飞云响应中未找到识别文本: {}",
            truncate(&body, 300)
        ));
    }
    Ok(lines.join("\n"))
}

/// 讯飞云「通用文字识别」sf8e6aca1：URL 带 hmac-sha256 签名（APIKey + APISecret），body 为 JSON
/// 文档请求行：POST https://api.xf-yun.com/v1/private/sf8e6aca1
async fn recognize_xfyun_general(
    client: &reqwest::Client,
    cfg: &WspCloudProvider,
    image_base64: &str,
) -> Result<String, String> {
    // 凭据参与签名计算，必须先去空白：复制粘贴常带首尾空格/换行，会导致签名校验失败
    let app_id = cfg.access_key_id.clone().unwrap_or_default().trim().to_string();
    let api_key = cfg.api_key.clone().unwrap_or_default().trim().to_string();
    let api_secret = cfg
        .access_key_secret
        .clone()
        .unwrap_or_default()
        .trim()
        .to_string();
    if app_id.is_empty() || api_key.is_empty() || api_secret.is_empty() {
        return Err("请先填写讯飞云 AppId、APIKey 与 APISecret".into());
    }

    let host = "api.xf-yun.com";
    let path = "/v1/private/sf8e6aca1";
    // 签名串三段：host / date（RFC1123 GMT）/ request-line，headers 固定为 "host date request-line"
    let date = chrono::Utc::now()
        .format("%a, %d %b %Y %H:%M:%S GMT")
        .to_string();
    let signature_origin = format!("host: {host}\ndate: {date}\nPOST {path} HTTP/1.1");
    let signature = {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(api_secret.as_bytes()).map_err(|e| e.to_string())?;
        mac.update(signature_origin.as_bytes());
        base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
    };
    let authorization_origin = format!(
        r#"api_key="{api_key}", algorithm="hmac-sha256", headers="host date request-line", signature="{signature}""#
    );
    let authorization =
        base64::engine::general_purpose::STANDARD.encode(authorization_origin.as_bytes());
    let url = format!(
        "https://{host}{path}?authorization={}&date={}&host={}",
        percent_encode(&authorization),
        percent_encode(&date),
        percent_encode(host)
    );

    // 请求体三段：header（app_id/status）、parameter（category/result）、payload（图片 base64）；
    // parameter 与 payload 的字段名必须与接口同名（sf8e6aca1 / sf8e6aca1_data_1）
    let body = serde_json::json!({
        "header": { "app_id": app_id, "status": 3 },
        "parameter": {
            "sf8e6aca1": {
                "category": "ch_en_public_cloud",
                "result": { "encoding": "utf8", "compress": "raw", "format": "json" }
            }
        },
        "payload": {
            "sf8e6aca1_data_1": {
                "encoding": image_encoding(image_base64),
                "image": image_base64,
                "status": 3
            }
        }
    });

    let request = client
        .post(&url)
        .json(&body)
        .build()
        .map_err(|e| format!("构建讯飞云请求失败: {e}"))?;
    let resp = execute_with_retry(client, &request, "讯飞云通用文字识别").await?;
    let status = resp.status();
    let raw = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        // 第三方原始报错（如 {"message":"HMAC signature does not match"}）
        return Err(raw_third_party_error(&raw));
    }

    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("讯飞云响应解析失败: {e}；原始响应: {}", truncate(&raw, 300)))?;
    let code = value
        .pointer("/header/code")
        .and_then(|c| c.as_i64())
        .unwrap_or(0);
    let message = value
        .pointer("/header/message")
        .and_then(|m| m.as_str())
        .unwrap_or_default();
    if code != 0 {
        // 第三方原始报错：code: message
        return Err(format!("{code}: {message}"));
    }

    // payload.result.text 为 base64 后的 JSON，识别文本在 pages[].lines[].content
    let encoded = value
        .pointer("/payload/result/text")
        .and_then(|t| t.as_str())
        .unwrap_or_default();
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded.as_bytes())
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .unwrap_or(serde_json::Value::Null);

    let mut lines = Vec::new();
    if let Some(pages) = decoded.get("pages").and_then(|p| p.as_array()) {
        for page in pages {
            let Some(page_lines) = page.get("lines").and_then(|l| l.as_array()) else {
                continue;
            };
            for line in page_lines {
                if let Some(content) = line.get("content").and_then(|c| c.as_str()) {
                    if !content.trim().is_empty() {
                        lines.push(content.to_string());
                    }
                }
            }
        }
    }
    if lines.is_empty() {
        return Err(format!("讯飞云响应中未找到识别文本: {}", truncate(&raw, 300)));
    }
    Ok(lines.join("\n"))
}

