// FastClaw · MCP Client
// 完整的 MCP 客户端实现，支持三种传输协议
// 从 FastPlan MCP 适配：将 FastPlanError → AppError

use futures::StreamExt;
use serde_json::json;
use std::collections::HashMap;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, Mutex, RwLock};
use tracing::{debug, info};

use super::{AppError, McpServerConfig, McpTool, TransportType};

// ═══════════════════════════════════════════════════════════════
//  传输层内部状态
// ═══════════════════════════════════════════════════════════════

#[derive(Clone)]
struct HttpSseState {
    post_url: Arc<RwLock<Option<String>>>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<serde_json::Value, String>>>>>,
    base_url: String,
    listener_alive: Arc<std::sync::atomic::AtomicBool>,
    needs_reinit: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Clone)]
struct StreamableHttpState {
    endpoint_url: String,
    session_id: Arc<RwLock<Option<String>>>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<serde_json::Value, String>>>>>,
    listener_alive: Arc<std::sync::atomic::AtomicBool>,
}

enum McpTransport {
    StdioTransport {
        child: Arc<Mutex<Child>>,
        stdin: Arc<Mutex<tokio::process::ChildStdin>>,
        stdout: Arc<Mutex<BufReader<tokio::process::ChildStdout>>>,
    },
    HttpSse {
        post_client: reqwest::Client,
        state: HttpSseState,
        _task: Arc<Mutex<tokio::task::JoinHandle<()>>>,
    },
    StreamableHttp {
        client: reqwest::Client,
        state: StreamableHttpState,
        _listener: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    },
}

// ═══════════════════════════════════════════════════════════════
//  McpClient
// ═══════════════════════════════════════════════════════════════

pub struct McpClient {
    name: String,
    transport: McpTransport,
    request_id: Mutex<u64>,
    tools_cache: Mutex<Option<Vec<McpTool>>>,
}

impl McpClient {
    /// 连接到 MCP Server
    pub async fn connect(config: &McpServerConfig) -> Result<Self, AppError> {
        let transport = match &config.transport {
            TransportType::Stdio { command, args } => {
                let mut cmd = Command::new(command);
                cmd.args(args)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null());
                for (k, v) in &config.environment {
                    cmd.env(k, v);
                }
                #[cfg(windows)]
                {
                    cmd.as_std_mut().creation_flags(0x08000000);
                }
                let mut child = cmd
                    .spawn()
                    .map_err(|e| AppError::Mcp(format!("启动 MCP Server 失败: {}", e)))?;
                let stdin = child
                    .stdin
                    .take()
                    .ok_or_else(|| AppError::Mcp("获取 stdin 失败".into()))?;
                let stdout = child
                    .stdout
                    .take()
                    .ok_or_else(|| AppError::Mcp("获取 stdout 失败".into()))?;
                McpTransport::StdioTransport {
                    child: Arc::new(Mutex::new(child)),
                    stdin: Arc::new(Mutex::new(stdin)),
                    stdout: Arc::new(Mutex::new(BufReader::new(stdout))),
                }
            }
            TransportType::Sse { url } => Self::auto_detect_transport(url).await?,
            TransportType::WebSocket { .. } => {
                return Err(AppError::Mcp("WebSocket 传输暂未实现".into()));
            }
        };

        let client = Self {
            name: config.name.clone(),
            transport,
            request_id: Mutex::new(0),
            tools_cache: Mutex::new(None),
        };

        client.initialize().await?;
        Ok(client)
    }

    // ─── 自动检测传输协议 ───────────────────────────────────────

    async fn auto_detect_transport(url: &str) -> Result<McpTransport, AppError> {
        info!("🔌 自动检测 MCP 传输协议: {}", url);
        // 先尝试 Streamable HTTP
        match Self::try_streamable_http(url).await {
            Ok(t) => {
                info!("✅ 使用 Streamable HTTP");
                return Ok(t);
            }
            Err(e) => {
                info!("ℹ️ Streamable HTTP 不可用 ({}), 尝试 Legacy SSE...", e);
            }
        }
        // 回退到 Legacy SSE
        match Self::try_legacy_sse(url).await {
            Ok(t) => {
                info!("✅ 使用 Legacy SSE");
                Ok(t)
            }
            Err(e) => Err(AppError::Mcp(format!(
                "Streamable HTTP 和 Legacy SSE 均失败: {}",
                e
            ))),
        }
    }

    // ─── Streamable HTTP (2025-03-26) ───────────────────────────

    async fn try_streamable_http(url: &str) -> Result<McpTransport, AppError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| AppError::Mcp(e.to_string()))?;

        let init_req = json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": { "tools": {}, "resources": {} },
                "clientInfo": { "name": "FastClaw", "version": env!("CARGO_PKG_VERSION") }
            }
        });

        let resp = client
            .post(url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .json(&init_req)
            .send()
            .await
            .map_err(|e| AppError::Mcp(format!("POST 失败: {}", e)))?;

        if resp.status().is_client_error() {
            return Err(AppError::Mcp(format!(
                "HTTP {} - 非 Streamable HTTP",
                resp.status()
            )));
        }
        if !resp.status().is_success() {
            return Err(AppError::Mcp(format!("HTTP {}", resp.status())));
        }

        let session_id = resp
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let init_result = if ct.contains("text/event-stream") {
            Self::read_sse_to_json(resp).await?
        } else {
            resp.json::<serde_json::Value>()
                .await
                .map_err(|e| AppError::Mcp(format!("解析响应失败: {}", e)))?
        };

        if init_result.get("error").is_some() {
            return Err(AppError::Mcp(format!(
                "初始化错误: {:?}",
                init_result.get("error")
            )));
        }
        if init_result.get("result").is_none() {
            return Err(AppError::Mcp("响应缺少 result 字段".into()));
        }

        let state = StreamableHttpState {
            endpoint_url: url.to_string(),
            session_id: Arc::new(RwLock::new(session_id)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            listener_alive: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };

        // 发送 initialized 通知
        let notif =
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized", "params": {} });
        let mut nr = client.post(url).header("Content-Type", "application/json");
        {
            let sid = state.session_id.read().await;
            if let Some(ref s) = *sid {
                nr = nr.header("Mcp-Session-Id", s.as_str());
            }
        }
        let _ = nr.json(&notif).send().await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        Ok(McpTransport::StreamableHttp {
            client,
            state,
            _listener: Arc::new(Mutex::new(None)),
        })
    }

    // ─── Legacy SSE (2024-11-05) ────────────────────────────────

    async fn try_legacy_sse(url: &str) -> Result<McpTransport, AppError> {
        let sse_client = reqwest::Client::builder()
            .pool_idle_timeout(None)
            .build()
            .map_err(|e| AppError::Mcp(e.to_string()))?;
        let post_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| AppError::Mcp(e.to_string()))?;

        let base_url = if let Some(idx) = url.rfind("/sse") {
            url[..idx].to_string()
        } else {
            url.rfind('/')
                .filter(|&i| i > 8)
                .map_or(url.to_string(), |i| url[..i].to_string())
        };

        let resp = sse_client
            .get(url)
            .header("Accept", "text/event-stream")
            .send()
            .await
            .map_err(|e| AppError::Mcp(format!("SSE GET 失败: {}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::Mcp(format!("SSE HTTP {}", resp.status())));
        }

        let state = HttpSseState {
            post_url: Arc::new(RwLock::new(None)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            base_url: base_url.clone(),
            listener_alive: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            needs_reinit: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };

        let (ready_tx, ready_rx) = oneshot::channel::<()>();
        let ready_tx = Arc::new(Mutex::new(Some(ready_tx)));
        let state_c = state.clone();

        let task = tokio::spawn(async move {
            Self::sse_listener(resp, &state_c, &ready_tx).await;
        });

        // 等待 endpoint 事件
        match tokio::time::timeout(Duration::from_secs(30), ready_rx).await {
            Ok(Ok(_)) => info!("✅ SSE endpoint 已发现"),
            _ => return Err(AppError::Mcp("等待 SSE endpoint 超时".into())),
        }

        Ok(McpTransport::HttpSse {
            post_client,
            state,
            _task: Arc::new(Mutex::new(task)),
        })
    }

    // ─── SSE 事件监听 ───────────────────────────────────────────

    async fn sse_listener(
        resp: reqwest::Response,
        state: &HttpSseState,
        ready_tx: &Arc<Mutex<Option<oneshot::Sender<()>>>>,
    ) {
        let mut stream = resp.bytes_stream();
        let mut buf = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(_) => break,
            };
            buf.push_str(&String::from_utf8_lossy(&chunk));
            buf = buf.replace("\r\n", "\n");

            while let Some(pos) = buf.find("\n\n") {
                let block = buf[..pos].to_string();
                buf = buf[pos + 2..].to_string();
                if block.trim().is_empty() {
                    continue;
                }

                let mut evt_type = "message".to_string();
                let mut data = String::new();
                for line in block.lines() {
                    if let Some(r) = line.strip_prefix("event:") {
                        evt_type = r.trim().to_string();
                    } else if let Some(r) = line.strip_prefix("data:") {
                        if !data.is_empty() {
                            data.push('\n');
                        }
                        data.push_str(r.trim());
                    }
                }

                match evt_type.as_str() {
                    "endpoint" => {
                        let ep = data.trim();
                        let full = if ep.starts_with("http") {
                            ep.to_string()
                        } else if ep.starts_with('/') {
                            let se = state.base_url.find("://").map(|i| i + 3).unwrap_or(0);
                            let he = state.base_url[se..]
                                .find('/')
                                .map(|i| i + se)
                                .unwrap_or(state.base_url.len());
                            format!("{}{}", &state.base_url[..he], ep)
                        } else {
                            format!("{}/{}", state.base_url, ep)
                        };
                        *state.post_url.write().await = Some(full);
                        if let Some(tx) = ready_tx.lock().await.take() {
                            let _ = tx.send(());
                        }
                    }
                    "message" => {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
                            if let Some(id) = v.get("id").and_then(|i| i.as_u64()) {
                                if let Some(tx) = state.pending.lock().await.remove(&id) {
                                    let r = if v.get("error").is_some() {
                                        Err(format!("MCP 错误: {}", v.get("error").unwrap()))
                                    } else {
                                        Ok(v.get("result").cloned().unwrap_or(json!(null)))
                                    };
                                    let _ = tx.send(r);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // ─── SSE 响应解析辅助 ───────────────────────────────────────

    async fn read_sse_to_json(resp: reqwest::Response) -> Result<serde_json::Value, AppError> {
        let mut stream = resp.bytes_stream();
        let mut buf = String::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AppError::Mcp(format!("SSE 流错误: {}", e)))?;
            buf.push_str(&String::from_utf8_lossy(&chunk));
            buf = buf.replace("\r\n", "\n");
            while let Some(pos) = buf.find("\n\n") {
                let block = buf[..pos].to_string();
                buf = buf[pos + 2..].to_string();
                let mut data = String::new();
                for line in block.lines() {
                    if let Some(r) = line.strip_prefix("data:") {
                        if !data.is_empty() {
                            data.push('\n');
                        }
                        data.push_str(r.trim());
                    }
                }
                if !data.is_empty() {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
                        if v.get("id").is_some() {
                            return Ok(v);
                        }
                    }
                }
            }
        }
        Err(AppError::Mcp("SSE 流结束但未收到 JSON-RPC 响应".into()))
    }

    async fn read_sse_for_id(
        resp: reqwest::Response,
        target_id: u64,
    ) -> Result<serde_json::Value, AppError> {
        let mut stream = resp.bytes_stream();
        let mut buf = String::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(60);

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(AppError::Mcp("SSE 响应超时(60s)".into()));
            }
            match tokio::time::timeout(remaining, stream.next()).await {
                Ok(Some(Ok(chunk))) => {
                    buf.push_str(&String::from_utf8_lossy(&chunk));
                    buf = buf.replace("\r\n", "\n");
                    while let Some(pos) = buf.find("\n\n") {
                        let block = buf[..pos].to_string();
                        buf = buf[pos + 2..].to_string();
                        let mut data = String::new();
                        for line in block.lines() {
                            if let Some(r) = line.strip_prefix("data:") {
                                if !data.is_empty() {
                                    data.push('\n');
                                }
                                data.push_str(r.trim());
                            }
                        }
                        if data.is_empty() {
                            continue;
                        }
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
                            if v.get("id").and_then(|i| i.as_u64()) == Some(target_id) {
                                if let Some(err) = v.get("error") {
                                    return Err(AppError::Mcp(format!("MCP 错误: {}", err)));
                                }
                                return Ok(v.get("result").cloned().unwrap_or(json!(null)));
                            }
                        }
                    }
                }
                Ok(Some(Err(e))) => return Err(AppError::Mcp(format!("SSE 错误: {}", e))),
                Ok(None) => return Err(AppError::Mcp("SSE 流意外结束".into())),
                Err(_) => return Err(AppError::Mcp("SSE 响应超时".into())),
            }
        }
    }

    // ─── 初始化 ──────────────────────────────────────────────────

    async fn initialize(&self) -> Result<(), AppError> {
        if matches!(&self.transport, McpTransport::StreamableHttp { .. }) {
            return Ok(()); // Streamable HTTP 在握手时已初始化
        }
        info!("🚀 初始化 MCP 连接...");
        let _result = self
            .send_request(
                "initialize",
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {}, "resources": {} },
                    "clientInfo": { "name": "FastClaw", "version": env!("CARGO_PKG_VERSION") }
                }),
            )
            .await?;
        info!("✅ MCP 初始化完成");
        self.send_notification("notifications/initialized", json!({}))
            .await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        Ok(())
    }

    // ─── 核心通信：发送请求 ──────────────────────────────────────

    async fn send_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let id = {
            let mut c = self.request_id.lock().await;
            *c += 1;
            *c
        };

        let request = if params.is_null() {
            json!({ "jsonrpc": "2.0", "id": id, "method": method })
        } else {
            json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
        };

        debug!("📤 MCP 请求 id={} method={}", id, method);

        match &self.transport {
            McpTransport::StdioTransport { stdin, .. } => {
                let mut w = stdin.lock().await;
                let s =
                    serde_json::to_string(&request).map_err(|e| AppError::Mcp(e.to_string()))?;
                w.write_all(s.as_bytes())
                    .await
                    .map_err(|e| AppError::Mcp(e.to_string()))?;
                w.write_all(b"\n")
                    .await
                    .map_err(|e| AppError::Mcp(e.to_string()))?;
                w.flush().await.map_err(|e| AppError::Mcp(e.to_string()))?;
                let resp = self.read_stdio().await?;
                if let Some(err) = resp.get("error") {
                    return Err(AppError::Mcp(format!("MCP 错误: {}", err)));
                }
                Ok(resp.get("result").cloned().unwrap_or(json!(null)))
            }

            McpTransport::StreamableHttp { client, state, .. } => {
                let mut req = client
                    .post(&state.endpoint_url)
                    .header("Content-Type", "application/json")
                    .header("Accept", "application/json, text/event-stream");
                {
                    let sid = state.session_id.read().await;
                    if let Some(ref s) = *sid {
                        req = req.header("Mcp-Session-Id", s.as_str());
                    }
                }

                let resp = req
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| AppError::Mcp(format!("POST 失败: {}", e)))?;
                if let Some(new_sid) = resp
                    .headers()
                    .get("mcp-session-id")
                    .and_then(|v| v.to_str().ok())
                {
                    *state.session_id.write().await = Some(new_sid.to_string());
                }
                if !resp.status().is_success() {
                    let s = resp.status();
                    let b = resp.text().await.unwrap_or_default();
                    return Err(AppError::Mcp(format!("HTTP {} - {}", s, b)));
                }
                let ct = resp
                    .headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                if ct.contains("text/event-stream") {
                    Self::read_sse_for_id(resp, id).await
                } else {
                    let v: serde_json::Value = resp
                        .json()
                        .await
                        .map_err(|e| AppError::Mcp(e.to_string()))?;
                    if let Some(err) = v.get("error") {
                        return Err(AppError::Mcp(format!("MCP 错误: {}", err)));
                    }
                    Ok(v.get("result").cloned().unwrap_or(json!(null)))
                }
            }

            McpTransport::HttpSse {
                post_client, state, ..
            } => {
                let post_url = state
                    .post_url
                    .read()
                    .await
                    .clone()
                    .ok_or_else(|| AppError::Mcp("SSE endpoint 未就绪".into()))?;

                let (tx, rx) = oneshot::channel();
                state.pending.lock().await.insert(id, tx);

                let resp = post_client
                    .post(&post_url)
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| AppError::Mcp(format!("POST 失败: {}", e)))?;

                if !resp.status().is_success() {
                    state.pending.lock().await.remove(&id);
                    return Err(AppError::Mcp(format!("HTTP {}", resp.status())));
                }

                match tokio::time::timeout(Duration::from_secs(60), rx).await {
                    Ok(Ok(Ok(v))) => Ok(v),
                    Ok(Ok(Err(e))) => Err(AppError::Mcp(e)),
                    Ok(Err(_)) => Err(AppError::Mcp("响应通道关闭".into())),
                    Err(_) => {
                        state.pending.lock().await.remove(&id);
                        Err(AppError::Mcp("请求超时(60s)".into()))
                    }
                }
            }
        }
    }

    async fn send_notification(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<(), AppError> {
        let notif = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        match &self.transport {
            McpTransport::StdioTransport { stdin, .. } => {
                let mut w = stdin.lock().await;
                let s = serde_json::to_string(&notif).map_err(|e| AppError::Mcp(e.to_string()))?;
                w.write_all(s.as_bytes())
                    .await
                    .map_err(|e| AppError::Mcp(e.to_string()))?;
                w.write_all(b"\n")
                    .await
                    .map_err(|e| AppError::Mcp(e.to_string()))?;
                w.flush().await.map_err(|e| AppError::Mcp(e.to_string()))?;
                Ok(())
            }
            McpTransport::HttpSse {
                post_client, state, ..
            } => {
                if let Some(url) = state.post_url.read().await.as_ref() {
                    let _ = post_client
                        .post(url)
                        .header("Content-Type", "application/json")
                        .json(&notif)
                        .send()
                        .await;
                }
                Ok(())
            }
            McpTransport::StreamableHttp { client, state, .. } => {
                let mut r = client
                    .post(&state.endpoint_url)
                    .header("Content-Type", "application/json");
                {
                    let sid = state.session_id.read().await;
                    if let Some(ref s) = *sid {
                        r = r.header("Mcp-Session-Id", s.as_str());
                    }
                }
                let _ = r.json(&notif).send().await;
                Ok(())
            }
        }
    }

    async fn read_stdio(&self) -> Result<serde_json::Value, AppError> {
        if let McpTransport::StdioTransport { stdout, .. } = &self.transport {
            let mut r = stdout.lock().await;
            let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
            loop {
                let rem = deadline.saturating_duration_since(tokio::time::Instant::now());
                if rem.is_zero() {
                    return Err(AppError::Mcp("Stdio 响应超时(30s)".into()));
                }
                let mut line = String::new();
                match tokio::time::timeout(rem, r.read_line(&mut line)).await {
                    Ok(Ok(0)) => return Err(AppError::Mcp("MCP Server 进程已退出".into())),
                    Ok(Ok(_)) => {
                        let t = line.trim();
                        if t.is_empty() {
                            continue;
                        }
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(t) {
                            if v.get("jsonrpc").is_some()
                                || v.get("id").is_some()
                                || v.get("result").is_some()
                            {
                                return Ok(v);
                            }
                        }
                        continue;
                    }
                    Ok(Err(e)) => return Err(AppError::Mcp(format!("读取 stdout 失败: {}", e))),
                    Err(_) => return Err(AppError::Mcp("Stdio 响应超时".into())),
                }
            }
        }
        Err(AppError::Mcp("非 Stdio 传输".into()))
    }

    // ─── 公共 API ────────────────────────────────────────────────

    /// 列出所有工具（带参数格式兼容重试）
    pub async fn list_tools(&self) -> Result<Vec<McpTool>, AppError> {
        {
            let c = self.tools_cache.lock().await;
            if let Some(t) = &*c {
                return Ok(t.clone());
            }
        }

        let variants: Vec<(&str, serde_json::Value)> = vec![
            ("no params", json!(null)),
            ("empty object", json!({})),
            ("cursor null", json!({"cursor": null})),
        ];
        let mut last_err = None;
        for (label, params) in &variants {
            match self.send_request("tools/list", params.clone()).await {
                Ok(resp) => {
                    let tools: Vec<McpTool> = resp
                        .get("tools")
                        .and_then(|t| serde_json::from_value(t.clone()).ok())
                        .unwrap_or_default();
                    info!("✅ tools/list ({}) 发现 {} 个工具", label, tools.len());
                    *self.tools_cache.lock().await = Some(tools.clone());
                    return Ok(tools);
                }
                Err(e) => {
                    let es = e.to_string();
                    if es.contains("-32602") || es.contains("Invalid") || es.contains("invalid") {
                        last_err = Some(e);
                        continue;
                    }
                    return Err(e);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| AppError::Mcp("tools/list 所有参数格式均失败".into())))
    }

    /// 调用工具
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        self.send_request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        )
        .await
    }

    /// 断开连接
    pub async fn disconnect(&self) {
        match &self.transport {
            McpTransport::StdioTransport { child, .. } => {
                let _ = child.lock().await.kill().await;
            }
            McpTransport::HttpSse { _task, state, .. } => {
                state
                    .listener_alive
                    .store(false, std::sync::atomic::Ordering::SeqCst);
                _task.lock().await.abort();
            }
            McpTransport::StreamableHttp {
                _listener, state, ..
            } => {
                state
                    .listener_alive
                    .store(false, std::sync::atomic::Ordering::SeqCst);
                if let Some(h) = _listener.lock().await.as_ref() {
                    h.abort();
                }
            }
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
