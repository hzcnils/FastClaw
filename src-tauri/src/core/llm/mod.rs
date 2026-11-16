// FastClaw · LLM 统一抽象层
// trait LLMProvider 抽象 + 多供应商实现 + 智能路由

pub mod context_budget;
pub mod openai;
pub mod provider;
pub mod router;
pub mod streaming;

use streaming::{StreamCallback, StreamContent, StreamUpdate};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::Emitter;

/// SSE 行缓冲上限，防止无换行的异常流撑爆内存
const SSE_LINE_BUFFER_MAX: usize = 1024 * 1024; // 1 MiB

/// LLM 调用响应
#[derive(Debug, Clone, Serialize)]
pub struct LlmResponse {
    pub content: String,
    pub model: String,
    pub provider: String,
    pub provider_code: String,
    pub token_input: i32,
    pub token_output: i32,
    pub cost_usd: f64,
    pub duration_ms: i64,
    /// LLM 返回的工具调用请求（OpenAI function_call / tool_calls 格式）
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    /// finish_reason: stop / tool_calls / length
    #[serde(default)]
    pub finish_reason: String,
    /// 工具调用日志 JSON（持久化用，供前端刷新后重建卡片）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_log_json: Option<String>,
    /// 流中断错误（非空时标记为部分完成，前端应保留已累积内容）
    #[serde(default)]
    pub stream_error: Option<String>,
}

/// 采样/偏好参数（来自智能体配置 agent_preferences）
/// 字段为 None 时不写入请求体，沿用供应商默认值。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SamplingParams {
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub max_tokens: Option<i64>,
    #[serde(default)]
    pub presence_penalty: Option<f64>,
    #[serde(default)]
    pub frequency_penalty: Option<f64>,
}

impl SamplingParams {
    /// 仅在字段有值时写入请求体
    pub fn apply_to(&self, body: &mut serde_json::Value) {
        if let Some(obj) = body.as_object_mut() {
            if let Some(v) = self.temperature {
                obj.insert("temperature".into(), serde_json::json!(v));
            }
            if let Some(v) = self.top_p {
                obj.insert("top_p".into(), serde_json::json!(v));
            }
            if let Some(v) = self.max_tokens {
                obj.insert("max_tokens".into(), serde_json::json!(v));
            }
            if let Some(v) = self.presence_penalty {
                obj.insert("presence_penalty".into(), serde_json::json!(v));
            }
            if let Some(v) = self.frequency_penalty {
                obj.insert("frequency_penalty".into(), serde_json::json!(v));
            }
        }
    }
}

/// 按模型系列写入思考开关（DeepSeek / Qwen3 / 其它）。
fn apply_thinking_params(body: &mut serde_json::Value, model_id: &str, enable_thinking: bool) {
    let model_id_lower = model_id.to_ascii_lowercase();
    let is_deepseek = model_id_lower.contains("deepseek");
    let is_qwen_thinking = model_id_lower.contains("qwen3")
        || model_id_lower.contains("qwq")
        || model_id_lower.contains("qwen-thinking");

    if is_deepseek {
        if enable_thinking {
            body["thinking"] = serde_json::json!({"type": "enabled"});
        } else {
            body["thinking"] = serde_json::json!({"type": "disabled"});
        }
    } else if is_qwen_thinking {
        body["enable_thinking"] = serde_json::json!(enable_thinking);
        body["chat_template_kwargs"] = serde_json::json!({"enable_thinking": enable_thinking});
    } else if enable_thinking {
        body["thinking"] = serde_json::json!({"type": "enabled", "budget_tokens": 8000});
    } else if model_id_lower.contains("glm") {
        // 智谱/GLM 默认可能开思考，短文案生成会被 reasoning 拖很久
        body["thinking"] = serde_json::json!({"type": "disabled"});
        body["enable_thinking"] = serde_json::json!(false);
    }
}

/// LLM 返回的工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String, // "function"
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String, // JSON string
}

/// 工具定义（发送给 LLM 的 tools 参数）
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub r#type: String, // "function"
    pub function: ToolFunctionDef,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolFunctionDef {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// LLM 管理器：管理多供应商、执行对话调用
pub struct LlmManager {
    pool: SqlitePool,
    client: reqwest::Client,
}

impl LlmManager {
    /// 默认 max_tokens：防止 LLM 生成超长内容时服务端强行断开连接
    const DEFAULT_MAX_TOKENS: i64 = 16384;

    pub async fn new(pool: SqlitePool) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(30))
            .read_timeout(std::time::Duration::from_secs(600))
            .pool_max_idle_per_host(2)
            .build()?;
        Ok(Self { pool, client })
    }

    /// 发送对话请求
    /// model_id 为空时使用默认模型
    pub async fn chat(
        &self,
        model_id: Option<&str>,
        system_prompt: &str,
        history: &[crate::commands::chat::MessageRow],
        user_message: &str,
    ) -> anyhow::Result<LlmResponse> {
        self.chat_with_tools(model_id, system_prompt, history, user_message, &[], &[], None, &[], false)
            .await
    }

    /// 带工具定义的对话请求
    /// tools: 可用工具列表（OpenAI tools 格式）
    /// extra_messages: 额外消息（如 tool_call 结果回注）
    /// sampling: 采样/偏好参数（来自智能体配置），None 时使用供应商默认值
    pub async fn chat_with_tools(
        &self,
        model_id: Option<&str>,
        system_prompt: &str,
        history: &[crate::commands::chat::MessageRow],
        user_message: &str,
        tools: &[ToolDefinition],
        extra_messages: &[serde_json::Value],
        sampling: Option<&SamplingParams>,
        images: &[String],
        enable_thinking: bool,
    ) -> anyhow::Result<LlmResponse> {
        self.chat_with_tools_stream(
            model_id, system_prompt, history, user_message, tools, extra_messages,
            sampling, images, enable_thinking, None,
        ).await
    }

    /// 工具聊天可选实时输出；每轮及重试先 Reset，完成正文由调用方统一落库。
    pub async fn chat_with_tools_stream(
        &self,
        model_id: Option<&str>,
        system_prompt: &str,
        history: &[crate::commands::chat::MessageRow],
        user_message: &str,
        tools: &[ToolDefinition],
        extra_messages: &[serde_json::Value],
        sampling: Option<&SamplingParams>,
        images: &[String],
        enable_thinking: bool,
        on_update: Option<&StreamCallback<'_>>,
    ) -> anyhow::Result<LlmResponse> {
        let max_retries = self.load_retry_count().await;
        let mut attempt = 0u32;
        loop {
            if let Some(callback) = on_update {
                callback(StreamUpdate::Reset);
            }
            match self
                .chat_with_tools_once(
                    model_id,
                    system_prompt,
                    history,
                    user_message,
                    tools,
                    extra_messages,
                    sampling,
                    images,
                    enable_thinking,
                    on_update,
                )
                .await
            {
                Ok(resp) => {
                    if Self::should_retry_empty_stream(&resp) && attempt < max_retries {
                        attempt += 1;
                        tracing::warn!(
                            "LLM chat_with_tools 空流失败，第 {}/{} 次重试",
                            attempt,
                            max_retries
                        );
                        if let Some(callback) = on_update {
                            callback(StreamUpdate::Retry { attempt, max_retries, reason: resp.stream_error.clone().unwrap_or_default() });
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(300 * attempt as u64))
                            .await;
                        continue;
                    }
                    return Ok(resp);
                }
                Err(e) if !Self::is_non_retryable_llm_error(&e) && attempt < max_retries => {
                    attempt += 1;
                    tracing::warn!(
                        "LLM chat_with_tools 失败，第 {}/{} 次重试: {}",
                        attempt,
                        max_retries,
                        e
                    );
                    if let Some(callback) = on_update {
                        callback(StreamUpdate::Retry { attempt, max_retries, reason: e.to_string() });
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(300 * attempt as u64))
                        .await;
                    continue;
                }
                Err(e) => return Err(Self::annotate_retry_exhausted(e, attempt)),
            }
        }
    }

    async fn chat_with_tools_once(
        &self,
        model_id: Option<&str>,
        system_prompt: &str,
        history: &[crate::commands::chat::MessageRow],
        user_message: &str,
        tools: &[ToolDefinition],
        extra_messages: &[serde_json::Value],
        sampling: Option<&SamplingParams>,
        images: &[String],
        enable_thinking: bool,
        on_update: Option<&StreamCallback<'_>>,
    ) -> anyhow::Result<LlmResponse> {
        let (model, provider) = self.resolve_model(model_id).await?;

        let api_key = if let Some(ref_id) = &provider.api_key_ref {
            // 先尝试从 keyring 读取（安全的方式）
            let key_from_keyring = keyring::Entry::new("fastclaw", ref_id)
                .ok()
                .and_then(|e| e.get_password().ok());

            if let Some(key) = key_from_keyring {
                log::debug!("使用 keyring 中的 API Key");
                key
            } else if !ref_id.is_empty() {
                // keyring 失败时，直接使用 ref_id 作为 API Key
                // （兼容数据库中直接存储的明文 API Key）
                log::debug!("使用数据库中的 API Key");
                ref_id.clone()
            } else {
                log::warn!("API Key 为空");
                String::new()
            }
        } else {
            log::warn!("provider {} 未配置 API Key", provider.name);
            String::new()
        };

        // 构建消息列表
        let mut messages = vec![serde_json::json!({"role": "system", "content": system_prompt})];
        append_history_messages(&mut messages, history);
        let user_content = if images.is_empty() {
            serde_json::json!(user_message)
        } else {
            let mut parts = vec![serde_json::json!({"type": "text", "text": user_message})];
            for img_url in images {
                parts.push(serde_json::json!({
                    "type": "image_url",
                    "image_url": { "url": img_url }
                }));
            }
            serde_json::json!(parts)
        };
        messages.push(serde_json::json!({"role": "user", "content": user_content}));

        // 追加额外消息（tool_call 结果回注等）
        for extra in extra_messages {
            messages.push(extra.clone());
        }

        // 构建请求体（默认流式，提供打字机效果）
        let has_tools = !tools.is_empty();
        let mut body = serde_json::json!({
            "model": model.model_id,
            "messages": messages,
            "stream": true,
        });

        if has_tools {
            body["tools"] = serde_json::to_value(tools)?;
            body["tool_choice"] = serde_json::json!("auto");
        }

        apply_thinking_params(&mut body, &model.model_id, enable_thinking);

        // 设置默认 max_tokens：防止生成超长内容时服务端强行断开，sampling 可覆盖
        if !body
            .as_object()
            .map_or(false, |o| o.contains_key("max_tokens"))
        {
            body["max_tokens"] = serde_json::json!(Self::DEFAULT_MAX_TOKENS);
        }

        // 注入智能体偏好参数（温度/采样等，可覆盖 max_tokens）
        if let Some(s) = sampling {
            s.apply_to(&mut body);
        }

        // 发送请求
        let start = std::time::Instant::now();
        let resp = self
            .send_chat_completions(&provider, &model, &api_key, &body)
            .await?;

        let duration = start.elapsed().as_millis() as i64;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "LLM API 错误\n模型: {}\n供应商：{} ({})\nHTTP状态码:{}\n响应:\n{}",
                model.model_id,
                provider.name,
                provider.base_url,
                status.as_u16(),
                err_text
            ));
        }

        // 处理流式响应
        let mut streamed_content = StreamContent::default();
        let mut finish_reason = "stop".to_string();
        let mut tc_acc: std::collections::BTreeMap<usize, (String, String, String)> =
            std::collections::BTreeMap::new();
        let mut token_input = 0i32;
        let mut token_output = 0i32;
        let mut stream_error: Option<String> = None;

        // ── 流式响应 ──
        let resp_status = resp.status();
        let mut stream = resp.bytes_stream();
        use futures::stream::StreamExt;
        // SSE 数据可能跨 chunk 拆分，用 buffer 拼接不完整的行
        let mut line_buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    let msg = if e.is_timeout() {
                        "响应超时：模型生成时间过长，请尝试简化问题或更换模型".to_string()
                    } else if e.is_connect() {
                        "连接失败：无法连接到模型服务，请检查网络和API地址".to_string()
                    } else if e.is_body() || e.is_decode() {
                        format!("响应流中断(HTTP {})：连接被重置", resp_status)
                    } else {
                        format!("流读取错误: {e}")
                    };
                    stream_error = Some(msg);
                    break; // 退出循环，保留已累积内容
                }
            };
            let text = String::from_utf8_lossy(&chunk);
            line_buffer.push_str(&text);
            if line_buffer.len() > SSE_LINE_BUFFER_MAX {
                stream_error = Some("响应异常：SSE 行缓冲超过上限".to_string());
                break;
            }

            // 按换行切分，保留最后一个不完整的行
            while let Some(pos) = line_buffer.find('\n') {
                let line = line_buffer[..pos].trim().to_string();
                line_buffer = line_buffer[pos + 1..].to_string();
                if line.is_empty() || line == ": OPENAI PROCESSING" {
                    continue;
                }

                if let Some(json_str) = line.strip_prefix("data: ") {
                    if json_str == "[DONE]" {
                        break;
                    }

                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(json_str) {
                        // 提取流式内容
                        if let Some(delta) = data
                            .get("choices")
                            .and_then(|c| c.get(0))
                            .and_then(|c| c.get("delta"))
                        {
                            if on_update.is_some() {
                                streamed_content.delta(delta, on_update);
                            } else if let Some(text) = delta.get("content").and_then(|c| c.as_str()) {
                                // 原有批量调用方（标题生成、远程调用等）仍只接收正文。
                                streamed_content.text.push_str(text);
                            }

                            // 流式 tool_calls：按 index 分片累加，arguments 逐块拼接
                            if let Some(arr) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                                for tc in arr {
                                    let idx = tc.get("index").and_then(|v| v.as_u64()).unwrap_or(0)
                                        as usize;
                                    let e = tc_acc.entry(idx).or_default();
                                    if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                                        if !id.is_empty() {
                                            e.0 = id.to_string();
                                        }
                                    }
                                    if let Some(f) = tc.get("function") {
                                        if let Some(n) = f.get("name").and_then(|v| v.as_str()) {
                                            if !n.is_empty() {
                                                e.1 = n.to_string();
                                            }
                                        }
                                        if let Some(a) = f.get("arguments").and_then(|v| v.as_str())
                                        {
                                            e.2.push_str(a);
                                        }
                                    }
                                }
                            }
                        }

                        // 获取 finish_reason
                        if let Some(reason) = data
                            .get("choices")
                            .and_then(|c| c.get(0))
                            .and_then(|c| c.get("finish_reason"))
                            .and_then(|r| r.as_str())
                        {
                            if reason != "null" {
                                finish_reason = reason.to_string();
                            }
                        }

                        // 获取 token 统计（通常在最后一个消息）
                        if let Some(usage) = data.get("usage") {
                            token_input = usage
                                .get("prompt_tokens")
                                .and_then(|t| t.as_i64())
                                .unwrap_or(0) as i32;
                            token_output = usage
                                .get("completion_tokens")
                                .and_then(|t| t.as_i64())
                                .unwrap_or(0) as i32;
                        }
                    }
                }
            }
        }

        streamed_content.finish(on_update);
        let content = streamed_content.text;

        // 累加器落成 Vec<ToolCall>（过滤没 name 的空片；补齐缺失 id）
        let tool_calls: Vec<ToolCall> = tc_acc
            .into_values()
            .filter(|(_, name, _)| !name.is_empty())
            .enumerate()
            .map(|(i, (id, name, arguments))| ToolCall {
                id: if id.is_empty() {
                    format!("call_{i}_{name}")
                } else {
                    id
                },
                r#type: "function".to_string(),
                function: ToolCallFunction {
                    name,
                    arguments: if arguments.trim().is_empty() {
                        "{}".to_string()
                    } else {
                        arguments
                    },
                },
            })
            .collect();

        let data = serde_json::json!({
            "choices": [{"message": {"content": content, "tool_calls": tool_calls}, "finish_reason": finish_reason}],
            "usage": {"prompt_tokens": token_input, "completion_tokens": token_output}
        });

        // 解析响应
        let choice = &data["choices"][0];
        let message = &choice["message"];

        let content = message["content"].as_str().unwrap_or("").to_string();

        let finish_reason = choice["finish_reason"]
            .as_str()
            .unwrap_or("stop")
            .to_string();

        // tool_calls 已在上文从流式/非流式分支正确构建，直接用不重解析

        let token_input = data["usage"]["prompt_tokens"].as_i64().unwrap_or(0) as i32;
        let token_output = data["usage"]["completion_tokens"].as_i64().unwrap_or(0) as i32;
        let cost_usd = self.calculate_cost(&model, token_input as f64, token_output as f64);

        Ok(LlmResponse {
            content,
            model: model.model_id.clone(),
            provider: provider.name.clone(),
            provider_code: provider.id.clone(),
            token_input,
            token_output,
            cost_usd,
            duration_ms: duration,
            tool_calls,
            finish_reason,
            tool_call_log_json: None,
            stream_error,
        })
    }

    /// 流式对话请求 — 逐 token 通过 Tauri 事件推送到前端
    /// images: base64 data URL 列表（如 "data:image/png;base64,..."），OpenAI/Ollama 多模态格式
    pub async fn chat_stream(
        &self,
        app_handle: &tauri::AppHandle,
        conversation_id: &str,
        model_id: Option<&str>,
        system_prompt: &str,
        history: &[crate::commands::chat::MessageRow],
        user_message: &str,
        images: &[String],
        enable_thinking: bool,
        sampling: Option<&SamplingParams>,
    ) -> anyhow::Result<LlmResponse> {
        let max_retries = self.load_retry_count().await;
        let mut attempt = 0u32;
        loop {
            let _ = app_handle.emit("chat-stream-reset", serde_json::json!({
                "conversation_id": conversation_id,
            }));
            match self
                .chat_stream_once(
                    app_handle,
                    conversation_id,
                    model_id,
                    system_prompt,
                    history,
                    user_message,
                    images,
                    enable_thinking,
                    sampling,
                )
                .await
            {
                Ok(resp) => {
                    if Self::should_retry_empty_stream(&resp) && attempt < max_retries {
                        attempt += 1;
                        tracing::warn!(
                            "LLM chat_stream 空流失败，第 {}/{} 次重试",
                            attempt,
                            max_retries
                        );
                        let _ = app_handle.emit(
                            "chat-stream-retry",
                            serde_json::json!({
                                "conversation_id": conversation_id,
                                "attempt": attempt,
                                "max_retries": max_retries,
                                "reason": resp.stream_error,
                            }),
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(300 * attempt as u64))
                            .await;
                        continue;
                    }
                    return Ok(resp);
                }
                Err(e) if !Self::is_non_retryable_llm_error(&e) && attempt < max_retries => {
                    attempt += 1;
                    tracing::warn!(
                        "LLM chat_stream 失败，第 {}/{} 次重试: {}",
                        attempt,
                        max_retries,
                        e
                    );
                    let _ = app_handle.emit(
                        "chat-stream-retry",
                        serde_json::json!({
                            "conversation_id": conversation_id,
                            "attempt": attempt,
                            "max_retries": max_retries,
                            "reason": e.to_string(),
                        }),
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(300 * attempt as u64))
                        .await;
                    continue;
                }
                Err(e) => return Err(Self::annotate_retry_exhausted(e, attempt)),
            }
        }
    }

    async fn chat_stream_once(
        &self,
        app_handle: &tauri::AppHandle,
        conversation_id: &str,
        model_id: Option<&str>,
        system_prompt: &str,
        history: &[crate::commands::chat::MessageRow],
        user_message: &str,
        images: &[String],
        enable_thinking: bool,
        sampling: Option<&SamplingParams>,
    ) -> anyhow::Result<LlmResponse> {
        let (model, provider) = self.resolve_model(model_id).await?;

        let api_key = if let Some(ref_id) = &provider.api_key_ref {
            let key_from_keyring = keyring::Entry::new("fastclaw", ref_id)
                .ok()
                .and_then(|e| e.get_password().ok());
            if let Some(key) = key_from_keyring {
                key
            } else if !ref_id.is_empty() {
                ref_id.clone()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // 构建消息列表
        let mut messages = vec![serde_json::json!({"role": "system", "content": system_prompt})];
        append_history_messages(&mut messages, history);

        // 构建用户消息：有图片时使用多模态数组格式（OpenAI/Ollama 兼容）
        let user_content = if images.is_empty() {
            serde_json::json!(user_message)
        } else {
            let mut parts = vec![serde_json::json!({"type": "text", "text": user_message})];
            for img_url in images {
                parts.push(serde_json::json!({
                    "type": "image_url",
                    "image_url": { "url": img_url }
                }));
            }
            serde_json::json!(parts)
        };
        messages.push(serde_json::json!({"role": "user", "content": user_content}));

        let mut body = serde_json::json!({
            "model": model.model_id,
            "messages": messages,
            "stream": true,
        });

        apply_thinking_params(&mut body, &model.model_id, enable_thinking);

        // 设置默认 max_tokens：防止生成超长内容时服务端强行断开，sampling 可覆盖
        if !body
            .as_object()
            .map_or(false, |o| o.contains_key("max_tokens"))
        {
            body["max_tokens"] = serde_json::json!(Self::DEFAULT_MAX_TOKENS);
        }

        // 注入智能体偏好参数（温度/采样等，可覆盖 max_tokens）
        if let Some(s) = sampling {
            s.apply_to(&mut body);
        }

        let start = std::time::Instant::now();
        let resp = self
            .send_chat_completions(&provider, &model, &api_key, &body)
            .await?;

        let duration = start.elapsed().as_millis() as i64;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "LLM API 错误\n模型: {}\n供应商：{} ({})\nHTTP状态码:{}\n响应:\n{}",
                model.model_id,
                provider.name,
                provider.base_url,
                status.as_u16(),
                err_text
            ));
        }

        // 逐 chunk 读取并 emit 事件
        let mut content = String::new();
        let mut finish_reason = "stop".to_string();
        let mut token_input = 0i32;
        let mut token_output = 0i32;
        // 跟踪 DeepSeek reasoning_content 阶段，用于合成 <think>...</think> 包裹
        let mut in_reasoning = false;
        let mut stream_error: Option<String> = None;

        let resp_status = resp.status();
        let mut stream = resp.bytes_stream();
        use futures::stream::StreamExt;
        // SSE 数据可能跨 chunk 拆分，用 buffer 拼接不完整的行
        let mut line_buffer = String::new();
        let mut last_chunk_time = std::time::Instant::now();

        while let Some(chunk) = stream.next().await {
            // 检查 chunk 间间隔是否过长（超过60秒无数据则超时）
            let elapsed = last_chunk_time.elapsed().as_secs();
            if elapsed > 60 {
                stream_error = Some("响应超时：模型超过60秒未返回数据".to_string());
                break;
            }
            last_chunk_time = std::time::Instant::now();
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    let msg = if e.is_timeout() {
                        "响应超时：模型生成时间过长，请尝试简化问题或更换模型".to_string()
                    } else if e.is_connect() {
                        "连接失败：无法连接到模型服务，请检查网络和API地址".to_string()
                    } else if e.is_body() || e.is_decode() {
                        format!("响应流中断(HTTP {})：连接被重置", resp_status)
                    } else {
                        format!("流读取错误: {e}")
                    };
                    stream_error = Some(msg);
                    break; // 退出循环，保留已累积内容
                }
            };
            let text = String::from_utf8_lossy(&chunk);
            line_buffer.push_str(&text);
            if line_buffer.len() > SSE_LINE_BUFFER_MAX {
                stream_error = Some("响应异常：SSE 行缓冲超过上限".to_string());
                break;
            }

            // 按换行切分，保留最后一个不完整的行
            while let Some(pos) = line_buffer.find('\n') {
                let line = line_buffer[..pos].trim().to_string();
                line_buffer = line_buffer[pos + 1..].to_string();

                if line.is_empty() || line == ": OPENAI PROCESSING" {
                    continue;
                }

                if let Some(json_str) = line.strip_prefix("data: ") {
                    if json_str.trim() == "[DONE]" {
                        continue;
                    }

                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(json_str) {
                        if let Some(delta) = data
                            .get("choices")
                            .and_then(|c| c.get(0))
                            .and_then(|c| c.get("delta"))
                        {
                            // DeepSeek reasoning_content 字段（深度思考内容）
                            // 用合成的 <think>...</think> 标签包裹，复用现有的思考块解析管道
                            let reasoning = delta
                                .get("reasoning_content")
                                .and_then(|c| c.as_str())
                                .filter(|s| !s.is_empty());

                            if let Some(rtok) = reasoning {
                                if !in_reasoning {
                                    in_reasoning = true;
                                    content.push_str("<think>");
                                    let _ = app_handle.emit(
                                        "chat-stream-token",
                                        serde_json::json!({ "conversation_id": conversation_id, "token": "<think>" }),
                                    );
                                }
                                content.push_str(rtok);
                                let _ = app_handle.emit(
                                    "chat-stream-token",
                                    serde_json::json!({ "conversation_id": conversation_id, "token": rtok }),
                                );
                            }

                            // 普通 content 字段（实际回答）
                            if let Some(token) = delta.get("content").and_then(|c| c.as_str()) {
                                if !token.is_empty() {
                                    // 如果刚从 reasoning 阶段切换过来，先关闭 <think>
                                    if in_reasoning {
                                        in_reasoning = false;
                                        content.push_str("</think>");
                                        let _ = app_handle.emit(
                                            "chat-stream-token",
                                            serde_json::json!({ "conversation_id": conversation_id, "token": "</think>" }),
                                        );
                                    }
                                    content.push_str(token);
                                    let _ = app_handle.emit(
                                        "chat-stream-token",
                                        serde_json::json!({ "conversation_id": conversation_id, "token": token }),
                                    );
                                }
                            }
                        }

                        if let Some(reason) = data
                            .get("choices")
                            .and_then(|c| c.get(0))
                            .and_then(|c| c.get("finish_reason"))
                            .and_then(|r| r.as_str())
                        {
                            if reason != "null" {
                                finish_reason = reason.to_string();
                            }
                        }

                        if let Some(usage) = data.get("usage") {
                            token_input = usage
                                .get("prompt_tokens")
                                .and_then(|t| t.as_i64())
                                .unwrap_or(0) as i32;
                            token_output = usage
                                .get("completion_tokens")
                                .and_then(|t| t.as_i64())
                                .unwrap_or(0) as i32;
                        }
                    }
                }
            }
        }

        // 流结束时若仍处于 reasoning 阶段（极少数情况：只有思考内容、无正文）
        // 必须同时 emit，否则前端累积 token 与落库 content 不一致
        if in_reasoning {
            content.push_str("</think>");
            let _ = app_handle.emit(
                "chat-stream-token",
                serde_json::json!({ "conversation_id": conversation_id, "token": "</think>" }),
            );
        }

        let cost_usd = self.calculate_cost(&model, token_input as f64, token_output as f64);

        Ok(LlmResponse {
            content,
            model: model.model_id.clone(),
            provider: provider.name.clone(),
            provider_code: provider.id.clone(),
            token_input,
            token_output,
            cost_usd,
            duration_ms: duration,
            tool_calls: vec![],
            finish_reason,
            tool_call_log_json: None,
            stream_error,
        })
    }

    /// 获取当前模型的显示名称
    pub async fn current_model_display(&self, model_id: Option<&str>) -> String {
        match self.resolve_model(model_id).await {
            Ok((_, provider)) => format!("{} / {}", provider.name, model_id.unwrap_or("default")),
            Err(_) => model_id.unwrap_or("default").to_string(),
        }
    }

    /// 读取基础参数「重试次数」，默认 5；非法值回退为 5
    async fn load_retry_count(&self) -> u32 {
        let val: Option<String> =
            sqlx::query_scalar("SELECT value FROM settings WHERE key = 'retry_count'")
                .fetch_optional(&self.pool)
                .await
                .ok()
                .flatten();
        match val.as_deref().map(str::trim) {
            Some(s) if !s.is_empty() => s.parse::<u32>().unwrap_or(5),
            _ => 5,
        }
    }

    /// 仅当流中断且尚未产出任何内容时才适合整请求重试（避免重复推送 token）
    fn should_retry_empty_stream(resp: &LlmResponse) -> bool {
        resp.stream_error.is_some() && resp.content.trim().is_empty()
    }

    /// 认证/客户端错误通常重试无效
    fn is_non_retryable_llm_error(err: &anyhow::Error) -> bool {
        let s = err.to_string();
        for code in ["401", "403", "400", "404", "422"] {
            if s.contains(&format!("HTTP状态码:{code}"))
                || s.contains(&format!("HTTP状态码: {code}"))
            {
                return true;
            }
        }
        let lower = s.to_ascii_lowercase();
        lower.contains("builder error")
            || lower.contains("relative url")
            || lower.contains("invalid header")
            || s.contains("请求地址无效")
            || s.contains("API Key 含有非法字符")
            || s.contains("找不到已启用的模型")
            || s.contains("API 地址为空")
    }

    fn annotate_retry_exhausted(err: anyhow::Error, _retries_done: u32) -> anyhow::Error {
        // 重试仍失败时只返回原始错误，不附加「已按配置重试 N 次后仍失败」给用户
        err
    }

    async fn send_chat_completions(
        &self,
        provider: &ProviderInfo,
        model: &ModelInfo,
        api_key: &str,
        body: &serde_json::Value,
    ) -> anyhow::Result<reqwest::Response> {
        let url = chat_completions_url(&provider.base_url).map_err(|e| {
            anyhow::anyhow!(
                "模型 {}（{}）请求地址无效: {e}",
                model.model_id,
                provider.name
            )
        })?;
        let key = sanitize_secret(api_key);
        let mut builder = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(body);
        if !key.is_empty() {
            let auth = format!("Bearer {key}");
            match reqwest::header::HeaderValue::from_str(&auth) {
                Ok(h) => builder = builder.header("Authorization", h),
                Err(_) => {
                    anyhow::bail!(
                        "模型 {}（{}）的 API Key 含有非法字符，无法发送请求",
                        model.model_id,
                        provider.name
                    );
                }
            }
        }
        builder.send().await.map_err(|e| {
            anyhow::anyhow!(
                "LLM 请求错误\n模型: {}\n供应商：{} ({})\n请求地址:{}\n错误详情:\n{}",
                model.model_id,
                provider.name,
                provider.base_url,
                url,
                error_chain(&e)
            )
        })
    }

    /// 解析模型和供应商信息。
    /// `model_id` 可为：
    /// - `models.id`（前端传入的实例主键 UUID，首选）
    /// - `models.model_id`（API 模型名，兼容 messages.model_id / 旧路径）
    async fn resolve_model(
        &self,
        model_id: Option<&str>,
    ) -> anyhow::Result<(ModelInfo, ProviderInfo)> {
        const SELECT: &str = r#"SELECT m.model_id, m.input_price, m.output_price,
                   p.provider_code as provider_code, p.name as provider_name,
                   p.base_url, p.api_key_ref, p.provider_type
                   FROM models m JOIN model_providers p ON m.provider_code = p.provider_code"#;

        if let Some(mid) = model_id.map(str::trim).filter(|s| !s.is_empty()) {
            // 先精确匹配 models.id（桌面选中的是 UUID）。
            // 同 API 名可能有多条（DeepSeek / 私域网），再按「有 API 地址、非内置网」排序。
            let row = sqlx::query_as::<_, ModelProviderRow>(&format!(
                "{SELECT}
                   WHERE m.is_enabled = 1 AND (m.id = ? OR m.model_id = ? OR m.display_name = ?)
                   ORDER BY CASE WHEN m.id = ? THEN 0 ELSE 1 END,
                            CASE WHEN TRIM(COALESCE(p.base_url, '')) != '' THEN 0 ELSE 1 END,
                            CASE WHEN UPPER(COALESCE(p.provider_code, '')) IN ('PRIVATE', 'HUIZHI', 'LOCAL') THEN 1 ELSE 0 END,
                            m.sort_order ASC, m.id ASC
                   LIMIT 1"
            ))
            .bind(mid)
            .bind(mid)
            .bind(mid)
            .bind(mid)
            .fetch_optional(&self.pool)
            .await?;
            if let Some(row) = row {
                if row.base_url.trim().is_empty() {
                    let alt = sqlx::query_as::<_, ModelProviderRow>(&format!(
                        "{SELECT}
                           WHERE m.is_enabled = 1 AND m.model_id = ?
                             AND TRIM(COALESCE(p.base_url, '')) != ''
                           ORDER BY CASE WHEN UPPER(COALESCE(p.provider_code, '')) IN ('PRIVATE', 'HUIZHI', 'LOCAL') THEN 1 ELSE 0 END,
                                    m.sort_order ASC
                           LIMIT 1"
                    ))
                    .bind(&row.model_id)
                    .fetch_optional(&self.pool)
                    .await?;
                    if let Some(alt) = alt {
                        return Ok(alt.into());
                    }
                }
                return Ok(row.into());
            }
            anyhow::bail!("找不到已启用的模型: {mid}（请在 FastClaw 中选择已配置供应商的模型）");
        }

        let row = sqlx::query_as::<_, ModelProviderRow>(&format!(
            "{SELECT}
                   WHERE m.is_default = 1 AND m.is_enabled = 1
                   ORDER BY m.sort_order ASC, m.id ASC
                   LIMIT 1"
        ))
        .fetch_optional(&self.pool)
        .await?;
        if let Some(row) = row {
            return Ok(row.into());
        }
        let row = sqlx::query_as::<_, ModelProviderRow>(&format!(
            "{SELECT}
                   WHERE m.is_enabled = 1 AND TRIM(COALESCE(p.base_url, '')) != ''
                   ORDER BY m.sort_order ASC, m.id ASC
                   LIMIT 1"
        ))
        .fetch_optional(&self.pool)
        .await?;
        if let Some(row) = row {
            return Ok(row.into());
        }
        anyhow::bail!("没有可用的默认模型，请在 FastClaw 中启用一个已配置 API 地址的模型");
    }

    /// 计算费用（每百万 token 的价格）
    fn calculate_cost(&self, model: &ModelInfo, input_tokens: f64, output_tokens: f64) -> f64 {
        let input_cost = model.input_price.unwrap_or(0.0) * input_tokens / 1_000_000.0;
        let output_cost = model.output_price.unwrap_or(0.0) * output_tokens / 1_000_000.0;
        input_cost + output_cost
    }
}

fn chat_completions_url(base_url: &str) -> anyhow::Result<String> {
    let mut u: String = base_url.chars().filter(|c| !c.is_control()).collect();
    u = u.trim().trim_matches(|c| c == '"' || c == '\'').to_string();
    u = u.trim_end_matches('/').to_string();
    if u.is_empty() {
        anyhow::bail!("供应商 API 地址为空");
    }
    if !u.contains("://") {
        u = format!("https://{u}");
    }
    if !(u.starts_with("http://") || u.starts_with("https://")) {
        anyhow::bail!("供应商 API 地址无效: {u}");
    }
    if u.ends_with("/chat/completions") {
        Ok(u)
    } else {
        Ok(format!("{u}/chat/completions"))
    }
}

/// 历史里的 tool/function 行不能原样发给 OpenAI 兼容接口：
/// 落库时只有 JSON 正文、没有顶层 `tool_call_id`，也没有配对的 `assistant.tool_calls`，
/// DeepSeek / 小米会 400（missing field `tool_call_id`）。
/// 当轮 `extra_messages` 仍按完整格式追加，不走这里。
fn is_replayable_history_role(role: &str) -> bool {
    matches!(role, "user" | "assistant" | "system")
}

fn append_history_messages(
    messages: &mut Vec<serde_json::Value>,
    history: &[crate::commands::chat::MessageRow],
) {
    for msg in history {
        if !is_replayable_history_role(&msg.role) {
            continue;
        }
        messages.push(serde_json::json!({
            "role": msg.role,
            "content": msg.content,
        }));
    }
}

fn sanitize_secret(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '\u{feff}' && !c.is_control())
        .collect::<String>()
        .trim()
        .to_string()
}

fn error_chain(err: &dyn std::error::Error) -> String {
    let mut parts = vec![err.to_string()];
    let mut cur = err.source();
    while let Some(e) = cur {
        let t = e.to_string();
        if !parts.iter().any(|p| p == &t) {
            parts.push(t);
        }
        cur = e.source();
    }
    parts.join(" → ")
}

// ─── 内部类型 ───

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub model_id: String,
    pub input_price: Option<f64>,
    pub output_price: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key_ref: Option<String>,
    pub provider_type: String,
}

#[derive(Debug, sqlx::FromRow)]
struct ModelProviderRow {
    model_id: String,
    input_price: Option<f64>,
    output_price: Option<f64>,
    provider_code: String,
    provider_name: String,
    base_url: String,
    api_key_ref: Option<String>,
    provider_type: String,
}

impl From<ModelProviderRow> for (ModelInfo, ProviderInfo) {
    fn from(r: ModelProviderRow) -> Self {
        (
            ModelInfo {
                model_id: r.model_id,
                input_price: r.input_price,
                output_price: r.output_price,
            },
            ProviderInfo {
                id: r.provider_code,
                name: r.provider_name,
                base_url: r.base_url,
                api_key_ref: r.api_key_ref,
                provider_type: r.provider_type,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::chat::MessageRow;

    fn history_row(role: &str, content: &str) -> MessageRow {
        MessageRow {
            id: "id".into(),
            conversation_id: "c".into(),
            role: role.into(),
            content: content.into(),
            model_id: None,
            model_instance_id: None,
            parent_id: None,
            token_input: None,
            token_output: None,
            status: "done".into(),
            created_at: String::new(),
            group_id: None,
            is_liked: None,
            is_favorite: None,
            translation: None,
            translation_lang: None,
            think_content: None,
            is_edited: 0,
            edited_at: None,
            context_cleared: 0,
            attachments: None,
            extra_data: None,
            kb_references: None,
        }
    }

    #[test]
    fn replayable_roles_keep_user_assistant_system() {
        assert!(is_replayable_history_role("user"));
        assert!(is_replayable_history_role("assistant"));
        assert!(is_replayable_history_role("system"));
        assert!(!is_replayable_history_role("tool"));
        assert!(!is_replayable_history_role("function"));
    }

    #[test]
    fn append_history_skips_tool_rows_without_tool_call_id() {
        let history = vec![
            history_row("user", "查一下天气"),
            history_row(
                "tool",
                r#"{"tool_call_id":"call_1","tool_name":"get_weather","result":"晴"}"#,
            ),
            history_row("assistant", "今天晴"),
        ];
        let mut messages = vec![serde_json::json!({"role":"system","content":"sys"})];
        append_history_messages(&mut messages, &history);

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[2]["role"], "assistant");
        assert!(messages.iter().all(|m| m.get("tool_call_id").is_none()));
        assert!(messages.iter().all(|m| m["role"] != "tool"));
    }
}
