// FastClaw · 对话模块 Tauri Commands
// 智能对话的核心入口：发送消息、管理会话
//
// 工具调用闭环流程：
//   用户消息 → 构建 prompt (记忆+知识库+工具定义)
//   → LLM 调用 → 检查 finish_reason
//   → 如果 tool_calls: 逐个执行 MCP/Skill → 结果回注 → 再次调用 LLM
//   → 如果 stop: 返回最终回答
//   → 最多 10 轮工具调用循环

use crate::core::error::AppError;
use crate::core::intent::RouteDecision;
use crate::core::llm::{ToolDefinition, ToolFunctionDef};
use crate::db::repo;
use crate::state::AppState;
use dirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};
use tracing::{info, warn};

/// 工具调用最大轮次（防止无限循环，控制单次请求耗时）
const MAX_TOOL_ROUNDS: usize = 6;

/// 本地知识库路径条目（来自前端）
#[derive(Debug, Deserialize)]
pub struct LocalKbEntry {
    path: String,
    #[serde(default)]
    recursive: bool,
}

/// 本轮选中的 Skill 的**声明**——注意带出来的是声明本身，不是它的名字。
///
/// 为什么不能事后拿 `skill_used_name` 去查：那个名字在**展开失败**时也会被赋值
/// （见 `send_message_stream` 里的 `if skill_used_name.is_none()` 兜底），
/// 拿它去查配置会在"名字在、但 skill 根本没生效"时得出错误结论。
/// 强制注入和分级放行都必须基于**这一次实际解析到的那份声明**。
#[derive(Debug, Clone, Default)]
struct SkillDeclaration {
    name: String,
    /// 产出是数据型（天气/股价/库存…）→ 没有真实工具证据时不许作答
    live_data: bool,
    /// 必须至少成功调用一次的工具；空 = 该 skill 没声明数据源
    requires_tools: Vec<String>,
}

/// 按名称展开 Skill 模板（按钮选择和 / 命令共用）
fn expand_skill_by_name(
    app: &tauri::AppHandle,
    name: &str,
    args: Option<&str>,
    original_content: &str,
    conversation_id: &str,
    skill_used_name: &mut Option<String>,
    decl: &mut Option<SkillDeclaration>,
) -> String {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let base = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("FastClaw")
        .join("skills");
    let discovered = crate::core::skills::discover_skills(&cwd, &[base]);

    if let Some(skill) = discovered.get(name) {
        if !skill.enabled {
            let msg = format!(
                "⚠️ Skill「{name}」已禁用，请到 Skills 管理页面启用后重试。\n\n---\n{orig}",
                name = name,
                orig = original_content
            );
            let _ = app.emit(
                "chat-skill-used",
                serde_json::json!({
                    "conversation_id": conversation_id,
                    "name": name,
                    "args": args,
                    "disabled": true,
                }),
            );
            return msg;
        }

        let template = crate::core::skills::expand_skill_template(skill, args, None);
        if !template.is_empty() {
            crate::core::skills::record_skill_usage(name, true);
            tracing::info!("🔧 {} → Skill 模板已展开 ({} 字符)", name, template.len());
            *skill_used_name = Some(name.to_string());
            // 模板展开 ≠ 执行，但"用户明确选了这个 skill"是一次强意图：
            // 声明了数据源就得真的去拿数据，而不是塞段文字祈祷模型自觉。
            *decl = Some(SkillDeclaration {
                name: name.to_string(),
                live_data: skill.live_data,
                requires_tools: skill.requires_tools.clone(),
            });
            let _ = app.emit(
                "chat-skill-used",
                serde_json::json!({
                    "conversation_id": conversation_id,
                    "name": name,
                    "args": args,
                }),
            );
            return template;
        }

        let msg = format!(
            "⚠️ Skill「{name}」模板内容为空，请检查 SKILL.md。\n\n---\n{orig}",
            name = name,
            orig = original_content
        );
        let _ = app.emit(
            "chat-stream-token",
            serde_json::json!({
                "conversation_id": conversation_id,
                "token": format!("⚠️ Skill「{}」模板为空\n\n", name),
            }),
        );
        msg
    } else {
        // Skill 不存在
        let msg = format!(
            "⚠️ Skill「{name}」未找到，请确认名称正确或先创建该 Skill。\n\n可用命令：\n- /{name}  — 使用已创建的 Skill\n- skill(name: \"list\") — 查看所有可用 Skill\n\n---\n{orig}",
            name = name,
            orig = original_content
        );
        let _ = app.emit(
            "chat-stream-token",
            serde_json::json!({
                "conversation_id": conversation_id,
                "token": format!("⚠️ Skill「{}」未找到，以普通消息发送\n\n", name),
            }),
        );
        msg
    }
}

#[tauri::command]
pub async fn send_message_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    content: String,
    model_id: Option<String>,
    knowledge_base_ids: Option<Vec<String>>,
    file_ids: Option<Vec<String>>,
    local_kb_paths: Option<Vec<LocalKbEntry>>,
    attachment_file_ids: Option<Vec<String>>,
    group_id: Option<String>,
    skip_user_message: Option<bool>,
    system_prompt: Option<String>,
    kb_search_api_url: Option<String>,
    images: Option<Vec<String>>,
    image_attachments_json: Option<String>,
    web_search: Option<bool>,
    search_provider: Option<String>,
    enable_thinking: Option<bool>,
    mcp_tools: Option<bool>,
    mcp_server_ids: Option<Vec<String>>,
    ocr_context: Option<String>,
    skill_name: Option<String>,
    // enabled_skills: 「本次对话」允许使用的 Skill 名称白名单。
    //   None      = 前端没传（老调用方 / 测试），不限制，沿用 skill 自身的 enabled
    //   Some([])  = 本次对话一个 skill 都不允许（弹层里开关默认全关就是这个值）
    //   Some(list)= 只允许名单内的
    enabled_skills: Option<Vec<String>>,
) -> Result<(), AppError> {
    tracing::info!(
        "💬 send_message_stream: conv={}, kb_ids={:?}, file_ids={:?}, local_paths={:?}, files={:?}, group_id={:?}, skip_user={:?}, sp={}, search_url={:?}",
        conversation_id, knowledge_base_ids, file_ids, local_kb_paths, attachment_file_ids, group_id, skip_user_message,
        system_prompt.is_some(), kb_search_api_url
    );

    let app_clone = app.clone();
    let app_for_guard = app.clone();
    let pool = state.db.pool().await.clone();
    let llm = state.llm.clone();
    let mcp = state.mcp.clone();
    let tool_registry = state.tool_registry.clone();
    let active_streams = state.active_streams.clone();

    let kb_ids = knowledge_base_ids.unwrap_or_default();
    let mut f_ids = file_ids.unwrap_or_default();
    // 合并 attachment_file_ids（来自前端附件上传返回的文件 ID）
    if let Some(att_ids) = attachment_file_ids {
        for id in att_ids {
            if !f_ids.contains(&id) {
                f_ids.push(id);
            }
        }
    }
    let local_paths = local_kb_paths.unwrap_or_default();
    let skip_user = skip_user_message.unwrap_or(false);
    let search_url = kb_search_api_url.unwrap_or_default();
    let img_list = images.unwrap_or_default();
    let img_attachments_json = image_attachments_json;
    let do_web_search = web_search.unwrap_or(false);
    let web_provider = search_provider.filter(|s| !s.is_empty());
    let do_think = enable_thinking.unwrap_or(false);
    crate::core::session_plan::config::set_current_ui_thinking(do_think);
    let do_mcp_tools = mcp_tools.unwrap_or(true);
    // ★ mcp_server_ids 的 None 与 Some([]) 语义不同，不能用 unwrap_or_default 抹平：
    //   None      = 调用方压根没传（agent 配置助手等老调用方）→ 保持原样：不筛选，全部 MCP 工具可用
    //   Some([])  = 本次对话一个 MCP 服务都没勾 → 一个都不注入，intent 路由也不启用
    //   Some(ids) = 只注入这些
    //   前端「工具」弹层的 MCP 服务开关现在按会话隔离，空选必须表达成「不给」。
    let mcp_servers: Option<Vec<String>> = mcp_server_ids;
    let ocr_ctx = ocr_context.unwrap_or_default();
    let conv_id_for_register = conversation_id.clone();
    let conv_id_for_cleanup = conversation_id.clone();
    let active_streams_cleanup = active_streams.clone();


    // Phase 2: Skill 模板展开（按钮选择模式）
    let mut skill_used_name: Option<String> = None;
    // 本轮选中的 skill 的声明（live_data / requires-tools），供工具注入与放行门使用
    let mut skill_decl: Option<SkillDeclaration> = None;
    // ★ 用户原文。下一行 `content` 会被展开后的模板覆盖，原文就再也拿不到了，
    //   而场景探测必须同时看两者：模板很可能不含主题词（generic 模板尤其如此），
    //   只看模板会把"帮我查天气"判成免工具提问。
    let raw_user_content = content.clone();
    let content = if let Some(ref explicit_name) = skill_name {
        // 按钮选择模式：直接展开指定 skill
        let expanded = expand_skill_by_name(
            &app,
            explicit_name,
            Some(&content),
            &content,
            &conversation_id,
            &mut skill_used_name,
            &mut skill_decl,
        );
        // 无论展开是否成功，都保留用户选择的 skill 名（用于用户消息气泡历史回显）
        if skill_used_name.is_none() {
            skill_used_name = Some(explicit_name.clone());
        }
        expanded
    } else {
        content
    };

    // ★ BERT 意图路由：在 LLM 调用前判断是否需要直接调 MCP 工具（需开启 MCP 工具）
    //   路由器是从**全量**工具索引里挑的，不看 mcp_servers，所以本次对话没勾任何
    //   服务时必须整条关掉——否则会把用户没勾的服务器上的工具直接调起来。
    let route_decision = if do_mcp_tools && !matches!(mcp_servers.as_deref(), Some([])) {
        state
            .intent_router
            .as_ref()
            .map(|router| router.route(&content))
    } else {
        None
    };

    // 先获取写锁并检查并发，再 spawn，最后在「仍持有写锁」时 insert。
    // 关键点：任务自身的清理 remove（见下）同样需要写锁，因此只要 insert 与本次
    // 检查在同一把锁的临界区内、且横跨 spawn，就能保证「insert 一定先于 remove」，
    // 杜绝快速失败路径下 remove 先跑、随后 insert 写入死 handle 导致会话被永久锁死。
    let mut streams_guard = active_streams.write().await;
    // 若同一会话上已有未结束的流：拒绝新请求，且此时尚未 spawn，不会产生重复落库/重复计费
    if streams_guard.contains_key(&conv_id_for_register) {
        let _ = app_for_guard.emit(
            "chat-stream-error",
            serde_json::json!({
                "conversation_id": conv_id_for_register,
                "error": "当前会话正在回答中，请等待回答完成或点击停止后再提问",
                "concurrent_rejected": true,
            }),
        );
        return Ok(());
    }

    let task = tokio::spawn(async move {
        let result = match route_decision {
            Some(RouteDecision::DirectTool {
                server_id,
                tool_name,
                arguments,
                score,
            }) => {
                info!(
                    "🎯 直接工具调用: {}::{} (score={:.3})",
                    server_id, tool_name, score
                );
                run_direct_tool(
                    app_clone.clone(),
                    pool,
                    conversation_id,
                    content,
                    group_id,
                    skip_user,
                    server_id,
                    tool_name,
                    arguments,
                    mcp,
                )
                .await
            }
            _ => {
                run_stream(
                    app_clone.clone(),
                    pool,
                    llm,
                    mcp,
                    tool_registry,
                    conversation_id,
                    content,
                    model_id,
                    kb_ids,
                    f_ids,
                    local_paths,
                    group_id,
                    skip_user,
                    system_prompt,
                    search_url,
                    img_list,
                    img_attachments_json,
                    do_web_search,
                    web_provider,
                    do_think,
                    do_mcp_tools,
                    mcp_servers,
                    ocr_ctx,
                    enabled_skills,
                    skill_used_name,
                    raw_user_content,
                    skill_decl,
                )
                .await
            }
        };
        // 任务结束后从注册表中移除（写锁会阻塞到上面的 insert 完成之后）
        let mut guard = active_streams_cleanup.write().await;
        guard.remove(&conv_id_for_cleanup);
        drop(guard);

        // ★ 无论成功、失败还是被中止，都必须把 thinking 状态打回 idle。
        //   否则前端会一直转圈（反馈单 298）。这里是所有路径的汇合点，
        //   放在 if let Err 之前，保证错误路径同样被清理。
        let _ = app_clone.emit(
            "chat-stream-status",
            serde_json::json!({ "conversation_id": conv_id_for_cleanup, "status": "idle" }),
        );

        if let Err(e) = result {
            let _ = app_clone.emit(
                "chat-stream-error",
                serde_json::json!({ "conversation_id": conv_id_for_cleanup, "error": e.to_string() }),
            );
        }
    });

    // 仍持有写锁 → 注册 abort handle，供 stop_stream 调用
    streams_guard.insert(conv_id_for_register, task.abort_handle());
    drop(streams_guard);
    let _ = app; // avoid unused warning on outer app

    Ok(())
}

/// 中断指定对话的进行中流式回答
/// 同时将前端已接收的部分内容保存到数据库，确保切换/刷新后能回显
#[tauri::command]
pub async fn stop_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    message_id: Option<String>,
    content: Option<String>,
    model_id: Option<String>,
    group_id: Option<String>,
    think_content: Option<String>,
) -> Result<(), AppError> {
    // ★ 先释放 active_streams 并 abort，再落库。
    //   避免 DB 慢/失败时会话锁一直占着；前端 IPC 超时重试时也能尽快解锁。
    let handle = {
        let mut guard = state.active_streams.write().await;
        guard.remove(&conversation_id)
    };
    let did_abort = if let Some(h) = handle {
        h.abort();
        // ★ abort() 会直接杀掉 future，spawn 里的 idle 广播不会执行，
        //   所以这里必须补一次，否则点了停止按钮转圈还在转（反馈单 298）。
        let _ = app.emit(
            "chat-stream-status",
            serde_json::json!({ "conversation_id": conversation_id, "status": "idle" }),
        );
        let _ = app.emit(
            "chat-stream-error",
            serde_json::json!({
                "error": "已停止",
                "canceled": true,
                "conversation_id": conversation_id,
            }),
        );
        // P2-29: stop 时也发 done，前端据此完成 UI 状态
        let _ = app.emit(
            "chat-stream-done",
            serde_json::json!({ "conversation_id": conversation_id, "content": content, "stopped": true }),
        );
        true
    } else {
        false
    };

    // 规划回合走 ACP 会话任务，不在 active_streams；停止时一并取消，避免 App/桌面卡住
    if let Some(ref handler) = state.acp_handler {
        use crate::core::session_plan::handler::AcpHandler;
        let _ = handler
            .handle_request(
                "session/cancel",
                Some(serde_json::json!({ "session_id": conversation_id })),
            )
            .await;
    }

    // 流已结束 / 重复 stop：不再落库，避免与 run_stream 已写入的完整回复重复
    if !did_abort {
        return Ok(());
    }

    let pool = state.db.pool().await.clone();

    // 将前端已接收的部分回答落库，防止丢失（失败不影响已释放的锁）
    if let (Some(ref msg_id), Some(ref msg_content)) = (&message_id, &content) {
        if !msg_content.is_empty() {
            // 给 run_stream 一个收尾窗口：若它已在 abort 前写完，下面复查能看到
            tokio::time::sleep(std::time::Duration::from_millis(80)).await;

            // 同 group 已有 assistant（多为 run_stream 抢先落库）→ 跳过，防重复行
            if let Some(ref gid) = group_id {
                let existing_assistant = sqlx::query_scalar::<_, String>(
                    "SELECT id FROM messages WHERE conversation_id = ? AND role = 'assistant' AND group_id = ? ORDER BY created_at DESC LIMIT 1",
                )
                .bind(&conversation_id)
                .bind(gid)
                .fetch_optional(&pool)
                .await
                .ok()
                .flatten();
                if existing_assistant.is_some() {
                    tracing::debug!(
                        "[stop_stream] skip persist: assistant already exists for group_id={}",
                        gid
                    );
                    return Ok(());
                }
            }

            // 同 id 已存在（重试 stop / 竞态）→ UPDATE，避免主键冲突或重复语义
            let id_exists: bool =
                sqlx::query_scalar::<_, i64>("SELECT 1 FROM messages WHERE id = ? LIMIT 1")
                    .bind(msg_id)
                    .fetch_optional(&pool)
                    .await
                    .ok()
                    .flatten()
                    .is_some();

            if id_exists {
                let _ = sqlx::query(
                    "UPDATE messages SET content = ?, model_id = COALESCE(?, model_id), status = 'canceled' WHERE id = ?",
                )
                .bind(msg_content)
                .bind(model_id.as_deref())
                .bind(msg_id)
                .execute(&pool)
                .await;
            } else {
                let parent_id = if let Some(ref gid) = group_id {
                    sqlx::query_scalar::<_, String>(
                        "SELECT id FROM messages WHERE conversation_id = ? AND role = 'user' AND group_id = ? ORDER BY created_at DESC LIMIT 1",
                    )
                    .bind(&conversation_id)
                    .bind(gid)
                    .fetch_optional(&pool)
                    .await
                    .ok()
                    .flatten()
                } else {
                    None
                };

                let _ = sqlx::query(
                    "INSERT INTO messages (id, conversation_id, role, content, model_id, parent_id, group_id, status, created_at) VALUES (?, ?, 'assistant', ?, ?, ?, ?, 'canceled', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
                )
                .bind(msg_id)
                .bind(&conversation_id)
                .bind(msg_content)
                .bind(model_id.as_deref())
                .bind(parent_id.as_deref())
                .bind(group_id.as_deref())
                .execute(&pool)
                .await;
            }

            let _ = sqlx::query(
                "UPDATE conversations SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
            )
            .bind(&conversation_id)
            .execute(&pool)
            .await;

            if let Some(ref think) = think_content {
                if !think.trim().is_empty() {
                    let _ = repo::messages::set_think_content(&pool, msg_id, think).await;
                }
            }
        }
    }

    Ok(())
}

/// 仅保存用户消息到数据库，不触发 LLM（用于任务规划模式）
#[tauri::command]
pub async fn save_user_message(
    state: State<'_, AppState>,
    conversation_id: String,
    content: String,
    group_id: Option<String>,
    attachments: Option<String>,
) -> Result<String, AppError> {
    let pool = state.db.pool().await.clone();
    let msg_id = uuid::Uuid::new_v4().to_string();
    let gid = group_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    repo::messages::insert(
        &pool,
        &repo::messages::NewMessage {
            id: msg_id.clone(),
            conversation_id,
            role: "user".into(),
            content,
            model_id: None,
            model_instance_id: None,
            parent_id: None,
            group_id: Some(gid.clone()),
            attachments,
            token_input: None,
            token_output: None,
        },
    )
    .await
    .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    Ok(gid)
}

/// 仅保存助手消息到 DB，不触发 LLM。
#[tauri::command]
pub async fn save_assistant_message(
    state: State<'_, AppState>,
    conversation_id: String,
    content: String,
    group_id: Option<String>,
    model_id: Option<String>,
    extra_data: Option<String>,
    status: Option<String>,
) -> Result<String, AppError> {
    let pool = state.db.pool().await.clone();
    let msg_id = uuid::Uuid::new_v4().to_string();
    let gid = group_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let status_val = status.unwrap_or_else(|| "completed".to_string());
    sqlx::query(
        "INSERT INTO messages (id, conversation_id, role, content, model_id, parent_id, group_id, status, created_at) VALUES (?, ?, 'assistant', ?, ?, NULL, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
    )
    .bind(&msg_id)
    .bind(&conversation_id)
    .bind(&content)
    .bind(model_id.as_deref())
    .bind(&gid)
    .bind(&status_val)
    .execute(&pool)
    .await
    .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;

    // 更新对话的 updated_at
    let _ = sqlx::query(
        "UPDATE conversations SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
    )
    .bind(&conversation_id)
    .execute(&pool)
    .await;

    if let Some(ref data) = extra_data {
        repo::messages::set_extra_data(&pool, &msg_id, data)
            .await
            .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    }
    Ok(msg_id)
}

/// 解析 `models.id`（实例主键）：
/// 1) 请求传入的实例 ID（前端正常路径）
/// 2) API 模型名 + provider_code（llm_response.model 反查）
async fn lookup_model_instance_id(
    pool: &sqlx::SqlitePool,
    requested_instance_id: Option<&str>,
    api_model_name: &str,
    provider_code: &str,
) -> Option<String> {
    if let Some(rid) = requested_instance_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        match sqlx::query_scalar::<_, String>("SELECT id FROM models WHERE id = ? LIMIT 1")
            .bind(rid)
            .fetch_optional(pool)
            .await
        {
            Ok(Some(id)) => return Some(id),
            Ok(None) => {}
            Err(e) => tracing::warn!("按 id 查找 model_instance_id 失败: {}", e),
        }
    }

    if api_model_name.trim().is_empty() || provider_code.trim().is_empty() {
        return None;
    }

    match sqlx::query_scalar::<_, String>(
        "SELECT id FROM models WHERE model_id = ? AND provider_code = ? LIMIT 1",
    )
    .bind(api_model_name)
    .bind(provider_code)
    .fetch_optional(pool)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!(
                "按 API 名查找 model_instance_id 失败 (model={}, provider={}): {}",
                api_model_name,
                provider_code,
                e
            );
            None
        }
    }
}

/// 用模型把用户口语化的问题改写为更适合搜索引擎的查询词。
/// 失败、超时或返回为空时回退为原始问题，保证联网搜索始终可用。
async fn generate_search_query(
    llm: &std::sync::Arc<crate::core::llm::LlmManager>,
    model_id: Option<&str>,
    content: &str,
) -> String {
    let system_prompt = "你是搜索查询词生成器。把用户的问题改写成一句最适合搜索引擎的查询词。\
        要求：提取关键实体与意图，可保留必要的英文术语；不超过 30 个字；\
        只输出查询词本身，不要解释、不要引号、不要换行。";
    match llm.chat(model_id, system_prompt, &[], content).await {
        Ok(resp) => {
            let q = resp
                .content
                .trim()
                .trim_matches(|c| c == '"' || c == '\'' || c == '“' || c == '”')
                .replace('\n', " ")
                .trim()
                .to_string();
            if q.is_empty() {
                content.to_string()
            } else {
                q
            }
        }
        Err(e) => {
            tracing::warn!("搜索查询词改写失败，回退原文: {}", e);
            content.to_string()
        }
    }
}

async fn run_stream(
    app: AppHandle,
    pool: sqlx::SqlitePool,
    llm: std::sync::Arc<crate::core::llm::LlmManager>,
    mcp: crate::core::mcp::SharedMcpManager,
    tool_registry: crate::core::tools::registry::SharedToolRegistry,
    conversation_id: String,
    content: String,
    model_id: Option<String>,
    kb_ids: Vec<String>,
    file_ids: Vec<String>,
    local_paths: Vec<LocalKbEntry>,
    group_id: Option<String>,
    skip_user_message: bool,
    extra_system_prompt: Option<String>,
    search_api_url: String,
    images: Vec<String>,
    image_attachments_json: Option<String>,
    do_web_search: bool,
    web_provider: Option<String>,
    enable_thinking: bool,
    do_mcp_tools: bool,
    // 本次对话勾选的 MCP 服务（语义见 send_message_stream 的注释：None ≠ Some([])）
    mcp_servers: Option<Vec<String>>,
    ocr_context: String,
    // 本次对话的 Skill 白名单（语义见 send_message_stream 的注释）
    enabled_skills: Option<Vec<String>>,
    skill_used_name: Option<String>,
    // raw_user_content: 用户原文（未展开 skill 模板前）。场景探测要拿它和 `content`
    //   求并集——`content` 若是展开后的模板，很可能已经不含"天气"这类主题词了。
    // skill_decl: 本轮选中 skill 的声明（live_data / requires-tools）。注意是**声明对象**
    //   而不是 `skill_used_name`：那个名字在展开失败时也会被赋值。
    raw_user_content: String,
    skill_decl: Option<SkillDeclaration>,
) -> Result<(), AppError> {
    tracing::info!(
        "📚 run_stream: kb_ids={:?}, file_ids={:?}, local_paths={:?}, skip_user={}",
        kb_ids,
        file_ids,
        local_paths,
        skip_user_message
    );

    // 1. 保存用户消息
    let user_msg_id = if skip_user_message {
        String::new()
    } else {
        let umid = uuid::Uuid::new_v4().to_string();
        repo::messages::insert(
            &pool,
            &repo::messages::NewMessage {
                id: umid.clone(),
                conversation_id: conversation_id.clone(),
                role: "user".into(),
                content: content.clone(),
                model_id: None,
                model_instance_id: None,
                parent_id: None,
                group_id: group_id.clone(),
                attachments: image_attachments_json.clone(),
                token_input: None,
                token_output: None,
            },
        )
        .await?;
        // 用户气泡上的 Skill 标签也要入库，切换历史会话后才能回显
        if let Some(ref name) = skill_used_name {
            let extra = serde_json::json!({ "skill_used": { "name": name } }).to_string();
            let _ = repo::messages::set_extra_data(&pool, &umid, &extra).await;
        }
        umid
    };

    let memory_ctx = String::new();

    // 3. ★ 远端知识库检索 (RAG) — 调用 KG 服务 HTTP 接口
    let (kb_context, kb_refs) = if !kb_ids.is_empty() {
        match search_remote_knowledge(&search_api_url, &kb_ids, &file_ids, &content, 5).await {
            Ok((ctx, refs)) => {
                tracing::info!(
                    "📚 RAG 命中 {} 条切片,上下文 {} 字符",
                    refs.len(),
                    ctx.len()
                );
                (ctx, refs)
            }
            Err(e) => {
                tracing::warn!("RAG 检索失败,降级为无知识库模式: {}", e);
                (String::new(), Vec::<serde_json::Value>::new())
            }
        }
    } else {
        (String::new(), Vec::new())
    };

    // 3b. ★ 本地知识库 (从文件夹读取文件内容)
    let local_kb_context = if local_paths.is_empty() {
        String::new()
    } else {
        read_local_knowledge(&local_paths)
    };

    let full_kb_context = if !local_kb_context.is_empty() {
        format!("{}\n\n{}", kb_context, local_kb_context)
    } else {
        kb_context
    };

    // 3c. ★ 联网搜索：开启时「默认先执行」，结果注入上下文 + 推送给前端展示
    let mut web_context = String::new();
    let mut web_search_extra: Option<String> = None;
    if do_web_search {
        // 先用模型把用户口语化的问题改写成更适合搜索引擎的查询词；失败则回退原文
        let search_query = generate_search_query(&llm, model_id.as_deref(), &content).await;
        tracing::info!("🌐 联网搜索查询词: {:?}", search_query);

        // 通知前端进入「搜索中」状态（用改写后的查询词作为展示标题）
        let _ = app.emit(
            "chat-web-search-start",
            serde_json::json!({ "conversation_id": conversation_id, "query": search_query }),
        );

        let cfg = crate::services::web_search::load_cfg(&pool, web_provider).await;

        // 只调所选 provider，不做任何 provider 回退
        let result = match crate::services::web_search::web_search(&search_query, &cfg).await {
            Ok(hits) => Ok((cfg.provider.clone(), hits)),
            Err(e) => Err(e.to_string()),
        };

        match result {
            Ok((used_provider, hits)) => {
                tracing::info!(
                    "🌐 联网搜索命中 {} 条（provider={}）",
                    hits.len(),
                    used_provider
                );
                web_context = crate::services::web_search::format_web_context(&hits);
                let hits_json: Vec<serde_json::Value> = hits
                    .iter()
                    .map(|h| {
                        serde_json::json!({
                            "title": h.title,
                            "url": h.url,
                            "snippet": h.snippet,
                        })
                    })
                    .collect();
                // 仅当有命中结果时记录，供随 AI 消息持久化
                if !hits.is_empty() {
                    web_search_extra = Some(
                        serde_json::json!({
                            "web_search_query": &search_query,
                            "web_hits": &hits_json,
                        })
                        .to_string(),
                    );
                }
                let _ = app.emit(
                    "chat-web-search",
                    serde_json::json!({
                        "conversation_id": conversation_id,
                        "query": search_query,
                        "provider": used_provider,
                        "hits": hits_json,
                    }),
                );
            }
            Err(err_msg) => {
                tracing::warn!("联网搜索失败，降级为无搜索: {}", err_msg);
                let _ = app.emit(
                    "chat-web-search",
                    serde_json::json!({
                        "conversation_id": conversation_id,
                        "query": search_query,
                        "error": err_msg,
                        "hits": [],
                    }),
                );
            }
        }
    }

    // 3c. ★ 智能体偏好参数（温度/采样等）— 按会话关联的 assistant_id 读取
    let sampling = load_agent_sampling(&pool, &conversation_id).await;

    // 4. 对话历史（已过滤 context_cleared = 1 的旧消息）
    let history = repo::messages::get_recent(&pool, &conversation_id, 50).await?;

    // ★ 判断本轮是否需要工具。
    //   反馈单 302「这个根本就不用调工具，直接大模型回答就行」、
    //   297「莫名其妙调起了 glob」的直接修复：明确是自我认知/寒暄类的问题，
    //   一律不注入工具定义，减少无关工具干扰及输入 token。
    //
    // ★ 但要看**最近一轮**，不能只看当前这句：
    //     用户：今天天气怎么样   → 需要工具
    //     助手：你在哪个城市呢
    //     用户：我在西安市       → 这句本身像闲聊，只看它会判成"免工具"，
    //                             于是工具全被跳过，模型只能空口答"好的帮你查"
    //   只有当**当前句和上一轮都没有**任何工具信号时，才判定为免工具。
    let prev_user_msg = history
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_default();
    //
    // ★ 探测文本 = 展开后的模板 ∪ 用户原文。
    //   原来只看 `content`，而 `content` 在走了 skill 之后**已经是展开后的模板**；
    //   模板若不含主题词（generic 的 skill 模板很容易不含），用户原文里的"天气"
    //   就从来没被看到过 —— 于是工具全不注入，模型只能凭记忆编。
    let probe = if raw_user_content == content {
        content.clone()
    } else {
        format!("{content}\n{raw_user_content}")
    };
    let tool_free = crate::core::tools::selection::is_tool_free_question(&probe)
        && (prev_user_msg.is_empty()
            || crate::core::tools::selection::is_tool_free_question(&prev_user_msg));

    // ★ 用户在输入框「工具」弹层里勾了 skill —— 这是一次**显式**指令，
    //   比全局开关和免工具判定都更具体（与下面 skill_decl 的第 2 层防线同一个道理）。
    //   不认这个信号的话，勾了开关也看不到任何反应，因为后面还有两道启发式会把它筛掉：
    //     ① 免工具判定把闲聊/写作/知识问答一律判成「无需工具」→ 整个工具循环不跑；
    //     ② 即使跑起来，「skill」的场景标记是 Execution，问题里没有
    //        「运行/生成/脚本」这类词时会被场景裁剪掉。
    let skill_picked_by_user = enabled_skills.as_deref().map(|s| !s.is_empty()).unwrap_or(false);

    if tool_free {
        if skill_picked_by_user {
            tracing::info!("💬 判定为无需工具，但用户勾了 skill —— 照开工具链");
        } else {
            tracing::info!("💬 判定为无需工具的问题，跳过工具注入");
        }
    }
    if skill_picked_by_user {
        tracing::info!(
            "🎯 本次对话勾选的 Skill（会写进 prompt 让模型看见）: {:?}",
            enabled_skills
        );
    }

    // ★ 第 2 层防线：skill 只要声明了数据源，就无视 `mcpTools` 开关与免工具判定，
    //   强制走工具链。用户"选中这个 skill"是一次比全局开关更具体的指令。
    //   （`images.is_empty()` 不绕过——那是多模态链路的技术约束，不是偏好。）
    let skill_forced_tools: Vec<String> = skill_decl
        .as_ref()
        .map(|d| d.requires_tools.clone())
        .unwrap_or_default();
    let skill_forces_tools = !skill_forced_tools.is_empty();
    if let Some(decl) = skill_decl.as_ref() {
        if decl.live_data || skill_forces_tools {
            // live_data 目前只出声、不拦截（拦截是第 4 层的事）。
            // 先让它可见：甲方现场可以直接从日志确认声明有没有被读到。
            tracing::info!(
                "📌 Skill「{}」声明: live_data={} requires_tools={:?}",
                decl.name,
                decl.live_data,
                decl.requires_tools
            );
        }
    }
    if skill_forces_tools && (!do_mcp_tools || tool_free) {
        tracing::warn!(
            "📌 Skill「{}」声明了 requires-tools={:?}，强制注入（mcpTools={}, tool_free={}）",
            skill_decl.as_ref().map(|d| d.name.as_str()).unwrap_or("?"),
            skill_forced_tools,
            do_mcp_tools,
            tool_free,
        );
    }
    let will_use_tools = images.is_empty()
        && ((do_mcp_tools && (!tool_free || skill_picked_by_user)) || skill_forces_tools);

    // ★ 当前模型显示名 → 注入 prompt，让模型知道自己是谁
    let model_display = llm.current_model_display(model_id.as_deref()).await;

    let system_prompt = build_system_prompt_v2(
        &memory_ctx,
        &full_kb_context,
        &web_context,
        extra_system_prompt.as_deref(),
        Some(model_display.as_str()),
        will_use_tools,
        enabled_skills.as_deref().unwrap_or(&[]),
    );

    // 5. LLM 调用（支持图片多模态；开启 MCP 且无图片时走工具调用循环）
    if !images.is_empty() {
        tracing::info!("🖼️ 携带 {} 张图片发送给 LLM", images.len());
    }

    // ★ OCR 上下文注入：将 OCR 提取的文字注入到 LLM 的用户消息中（不写入 DB）
    let llm_content = if ocr_context.is_empty() {
        content.clone()
    } else {
        tracing::info!("📝 注入 OCR 上下文 ({} 字符)", ocr_context.len());
        format!("{}\n\n---\n用户问题：\n{}", ocr_context, content)
    };

    // ★「本次对话勾了哪些 MCP 服务」→ 注入哪些 MCP 工具。
    //   None      = 调用方没传，不筛选（老行为）
    //   Some([])  = 一个都没勾 → 整块 MCP 工具不注入（但工具循环照跑，本轮可能只想用 skill）
    //   Some(ids) = 只注入这些
    let include_mcp = !matches!(mcp_servers.as_deref(), Some([]));
    let mcp_filter: Option<&[String]> = match mcp_servers.as_deref() {
        Some(ids) if !ids.is_empty() => Some(ids),
        _ => None,
    };
    let (tools, catalog_block) = if will_use_tools {
        let all = collect_tool_definitions_filtered(&tool_registry, &mcp, include_mcp, mcp_filter).await;
        let all_count = all.len();

        // ★ 阶段一：场景裁剪。40 个 → 通常 8-15 个。
        //   原实现全量注入，仅 schema 就约 8-9k token/轮，且模型面对
        //   40 个工具选择时注意力被稀释，选错概率显著上升。
        //
        // ★ 阶段二：渐进式披露。模型此前调用过 load_tool 的工具，
        //   即使不在本轮场景内，也把完整定义注入进来。
        let loaded = crate::core::tools::disclosure::get_loaded(&conversation_id);

        // ★ 场景探测要带上最近一轮上下文。
        //   多轮补参数的场景下，用户这一句可能完全没有场景关键词：
        //     用户：今天天气怎么样      → 命中 Web，注入 get_weather
        //     助手：你在哪个城市呢
        //     用户：我在西安市          → 这句本身没有任何天气词！
        //   只看当前消息会导致 get_weather 消失，模型只能干答"好的帮你查一下"。
        //   把最近 2 条消息拼进来做探测（只用于选工具，不影响真正发给模型的内容）。
        let scope_hint = {
            let mut s = String::new();
            for m in history.iter().rev().take(2) {
                s.push_str(&m.content);
                s.push(' ');
            }
            s.push_str(&content);
            s
        };

        // ★ skill 声明的工具并入"强制保留"名单。
        //   scope_tools 的第三个参数就是强制保留通道（优先级 0，MCP 工具与
        //   已 load_tool 的工具本来就走它）。不并进去的话，声明了 get_weather
        //   但场景探测没命中 Web，工具照样会被裁掉——声明就白写了。
        let mut forced_names = loaded.clone();
        for t in &skill_forced_tools {
            if !forced_names.contains(t) {
                forced_names.push(t.clone());
            }
        }
        // 用户勾了 skill → 强制保留「skill」工具本体。
        // 它的场景标记是 Execution，问题里没有「运行/生成/脚本」这类词时会被
        // scope_tools 裁掉——那勾选就白勾了，模型压根看不到它。
        if skill_picked_by_user && !forced_names.iter().any(|n| n == "skill") {
            forced_names.push("skill".to_string());
        }
        // 声明了却不在注册表里（名字写错 / MCP server 没连上 / 该工具未启用）：
        // 现在只在日志里出声，P3 的放行门会把它当"缺工具"直接拦下。
        for t in &skill_forced_tools {
            if !all.iter().any(|d| d.function.name == *t) {
                tracing::warn!("📌 Skill 声明的工具「{}」不在可用工具集中，无法注入", t);
            }
        }

        let scoped =
            crate::core::tools::selection::scope_tools(all.clone(), &scope_hint, &forced_names);

        // 未注入完整定义的工具 → 生成"名称 + 一行摘要"清单放进 prompt。
        // 这是常驻成本（约 600 token），远小于全量 schema（约 8-9k token）。
        let injected: Vec<String> = scoped.iter().map(|d| d.function.name.clone()).collect();
        let catalog = crate::core::tools::disclosure::build_catalog_block(&all, &injected);

        tracing::info!(
            "🔧 工具: 全量 {} → 注入完整定义 {} 个，清单披露 {} 个（场景: {:?}，已加载: {:?}，skill 强制: {:?}）",
            all_count,
            scoped.len(),
            all_count.saturating_sub(scoped.len()),
            crate::core::tools::selection::detect_scenarios(&scope_hint),
            loaded,
            skill_forced_tools,
        );
        // ★ 把实际注入的工具名打出来，便于核对裁剪是否合理
        tracing::info!("🔧 注入清单: [{}]", injected.join(", "));
        (scoped, catalog)
    } else {
        (Vec::new(), String::new())
    };

    // 工具清单需要进 system prompt，所以在这里重建一次
    let system_prompt = if catalog_block.is_empty() {
        system_prompt
    } else {
        format!("{system_prompt}{catalog_block}")
    };

    let (llm_response, grounding_calls) = if will_use_tools && !tools.is_empty() {
        tracing::info!("🔧 MCP 工具模式: {} 个可用工具", tools.len());
        chat_with_mcp_tools_loop(
            &app,
            &llm,
            &mcp,
            &tool_registry,
            &pool,
            &conversation_id,
            &llm_content,
            model_id.as_deref(),
            &system_prompt,
            &history,
            &tools,
            sampling.as_ref(),
            enable_thinking,
            enabled_skills.as_deref(),
        )
        .await?
    } else {
        // 没走工具循环 → 台账为空，模型手上没有任何外部数据
        let resp = llm
            .chat_stream(
                &app,
                &conversation_id,
                model_id.as_deref(),
                &system_prompt,
                &history,
                &llm_content,
                &images,
                enable_thinking,
                sampling.as_ref(),
            )
            .await
            .map_err(|e| AppError::Llm(e.to_string()))?;
        (resp, Vec::new())
    };

    // 5.5 把 <think>...</think> 块从正文中剥离，分别落库
    let (clean_content, think_text) = split_think_blocks(&llm_response.content);

    // 6. 保存 AI 回复
    let ai_msg_id = uuid::Uuid::new_v4().to_string();
    let ai_parent_id = if skip_user_message {
        None
    } else {
        Some(user_msg_id)
    };
    // 解析模型实例 ID：优先请求传入的 UUID，其次用 API 名 + provider 反查
    let model_instance_id = lookup_model_instance_id(
        &pool,
        model_id.as_deref(),
        &llm_response.model,
        &llm_response.provider_code,
    )
    .await;
    repo::messages::insert(
        &pool,
        &repo::messages::NewMessage {
            id: ai_msg_id.clone(),
            conversation_id: conversation_id.clone(),
            role: "assistant".into(),
            content: clean_content.clone(),
            model_id: Some(llm_response.model.clone()),
            model_instance_id,
            parent_id: ai_parent_id,
            group_id: group_id.clone(),
            attachments: None,
            token_input: Some(llm_response.token_input),
            token_output: Some(llm_response.token_output),
        },
    )
    .await?;

    if let Some(ref t) = think_text {
        if !t.trim().is_empty() {
            let _ = repo::messages::set_think_content(&pool, &ai_msg_id, t).await;
        }
    }

    // ★ 证据台账：回答里的数字究竟能不能在工具返回里找到。
    //
    //   本阶段**只记录、只提示，不拦截任何内容**。先跑一段拿到真实分布
    //   （多少回答是压根没证据的、多少是值对不上的），再据此决定后续硬拦的开合。
    let grounding = crate::core::grounding::build(&clean_content, &grounding_calls);
    // expects_live_data = 本轮问题**本应**由工具回答，结果却裸答了。
    // 取 tool_free 的反面：场景探测判定"这问题不需要工具"时不去打扰用户——
    // 否则「帮我写首诗」也会带一句"未经检索"，用户很快会无视所有提示。
    let grounding_note = grounding.annotation(!tool_free);
    tracing::info!(
        "🔍 接地判定 [{}] 证据 {} 条 / 具体值 {} 个 / 无出处 {} 个{}",
        grounding.grounding.as_str(),
        grounding.sources.len(),
        grounding.claims.len(),
        grounding.claims.iter().filter(|c| !c.is_grounded()).count(),
        grounding_note
            .as_deref()
            .map(|_| " → 已附提示")
            .unwrap_or(""),
    );

    // 合并 extra_data：skill_used + web_search + tool_calls（一次性写入，避免覆盖）
    {
        let mut extra: serde_json::Value = serde_json::json!({});
        if let Some(ref name) = skill_used_name {
            extra["skill_used"] = serde_json::json!({ "name": name });
        }
        if let Some(ref ed) = web_search_extra {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(ed) {
                for (k, v) in parsed.as_object().into_iter().flat_map(|o| o.iter()) {
                    extra[k] = v.clone();
                }
            }
        }
        // ★ 接地台账 + 系统提示语。提示语在这里就算好一起落库，前端不必（也不该）
        //   自己再实现一遍判定逻辑——两边算法一旦漂移，界面显示的和实际判定的就会
        //   打架。同时这也保证这句提示不经过模型，模型无法让它消失。
        if let Ok(g) = serde_json::to_value(&grounding) {
            extra["grounding"] = g;
        }
        if let Some(ref note) = grounding_note {
            extra["grounding_note"] = serde_json::json!(note);
        }
        // tool_call_log 已移除（远端API变更）
        let extra_str = extra.to_string();
        if extra_str != "{}" {
            let _ = repo::messages::set_extra_data(&pool, &ai_msg_id, &extra_str).await;
        }
    }

    if !kb_refs.is_empty() {
        if let Ok(refs_json) = serde_json::to_string(&kb_refs) {
            let _ = repo::messages::set_kb_references(&pool, &ai_msg_id, &refs_json).await;
        }
    }

    // 7. token 统计
    repo::cost::insert_usage(
        &pool,
        &repo::cost::TokenUsage {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: "default-user".to_string(),
            conversation_id: Some(conversation_id.clone()),
            message_id: Some(ai_msg_id.clone()),
            provider_id: llm_response.provider_code.clone(),
            provider_code: llm_response.provider_code.clone(),
            model_id: llm_response.model.clone(),
            token_input: llm_response.token_input,
            token_output: llm_response.token_output,
            cost_usd: llm_response.cost_usd,
            call_type: "chat_stream".to_string(),
        },
    )
    .await?;


    // ★ 诊断：把最终 token 计数打进日志。
    //   反馈「有的会话不显示 token 用量」时，先看这一行：
    //   - 如果这里是 0 → 供应商没返回 usage（部分渠道在带 tools 的
    //     非流式请求里确实会省略 usage 字段），属于上游问题
    //   - 如果这里有值但界面不显示 → 是前端读取字段的问题，
    //     检查前端是读 token_input/token_output 还是 usage_detail
    tracing::info!(
        "📊 token 统计: 输入 {} / 输出 {}，成本 ${:.6}，模型 {}",
        llm_response.token_input,
        llm_response.token_output,
        llm_response.cost_usd,
        llm_response.model
    );

    // 9. 完成事件(附引用 + 流错误标记) — Tauri 前端
    let _ = app.emit(
        "chat-stream-done",
        serde_json::json!({
            "conversation_id": conversation_id,
            "message_id": ai_msg_id,
            "content": clean_content,
            "think_content": think_text,
            "model": llm_response.model,
            "token_input": llm_response.token_input,
            "token_output": llm_response.token_output,
            // ★ 成本归因元数据（反馈单 300/301）：
            //   前端应显示为「输入 N、输出 M tokens」，而不是「N + M tokens」。
            //   多轮工具调用时 token_input 是**累计值**，rounds/tool_calls 让用户
            //   能理解为什么一个问题会消耗这么多输入 token。
            "usage_detail": {
                "input_tokens": llm_response.token_input,
                "output_tokens": llm_response.token_output,
                "cost_usd": llm_response.cost_usd,
                "tool_rounds": llm_response.tool_call_log_json.as_ref()
                    .and_then(|j| serde_json::from_str::<Vec<serde_json::Value>>(j).ok())
                    .map(|v| v.iter()
                        .filter_map(|e| e.get("round").and_then(|r| r.as_u64()))
                        .max().unwrap_or(0))
                    .unwrap_or(0),
                "tool_calls": llm_response.tool_call_log_json.as_ref()
                    .and_then(|j| serde_json::from_str::<Vec<serde_json::Value>>(j).ok())
                    .map(|v| v.len()).unwrap_or(0),
            },
            "references": kb_refs,
            "stream_error": llm_response.stream_error,
            // ★ 接地结果随完成事件一起给前端，避免"刷新后才有提示"的不一致
            "grounding": grounding,
            "grounding_note": grounding_note,
            "skill_used": skill_used_name.map(|n| serde_json::json!({ "name": n })),
        }),
    );

    Ok(())
}

/// 把 `<think>...</think>` 块从文本中剥离，返回 (清理后正文, Option<合并后的 think 内容>)
/// 兼容 `<thinking>` / 大小写不敏感、跨行、多块。
fn split_think_blocks(text: &str) -> (String, Option<String>) {
    let lower = text.to_ascii_lowercase();
    let mut clean = String::with_capacity(text.len());
    let mut think_parts: Vec<String> = Vec::new();
    let mut cursor = 0usize;
    let bytes = text.as_bytes();

    let openers: [(&str, &str); 2] = [("<think>", "</think>"), ("<thinking>", "</thinking>")];

    while cursor < bytes.len() {
        // 在小写副本上查找最近的 opener
        let mut next: Option<(usize, &str, &str)> = None;
        for (open, close) in &openers {
            if let Some(pos) = lower[cursor..].find(open) {
                let abs = cursor + pos;
                if next.map_or(true, |(p, _, _)| abs < p) {
                    next = Some((abs, *open, *close));
                }
            }
        }

        match next {
            Some((open_pos, open_tag, close_tag)) => {
                clean.push_str(&text[cursor..open_pos]);
                let after_open = open_pos + open_tag.len();
                if let Some(rel) = lower[after_open..].find(close_tag) {
                    let close_pos = after_open + rel;
                    let inner = &text[after_open..close_pos];
                    let trimmed = inner.trim();
                    if !trimmed.is_empty() {
                        think_parts.push(trimmed.to_string());
                    }
                    cursor = close_pos + close_tag.len();
                } else {
                    // 未闭合，剩余全部当 think
                    let inner = &text[after_open..];
                    let trimmed = inner.trim();
                    if !trimmed.is_empty() {
                        think_parts.push(trimmed.to_string());
                    }
                    cursor = bytes.len();
                }
            }
            None => {
                clean.push_str(&text[cursor..]);
                cursor = bytes.len();
            }
        }
    }

    let think = if think_parts.is_empty() {
        None
    } else {
        Some(think_parts.join("\n\n"))
    };
    (clean.trim().to_string(), think)
}

/// 远端知识库检索：调用 KG 服务 HTTP 搜索接口
async fn search_remote_knowledge(
    search_url: &str,
    kb_ids: &[String],
    file_ids: &[String],
    query: &str,
    top_k: usize,
) -> Result<(String, Vec<serde_json::Value>), AppError> {
    let url = if search_url.is_empty() {
        format!(
            "http://127.0.0.1:{}/api/v1/search",
            crate::commands::wsp::dependency_runtime_port("fastkg")
                .unwrap_or_else(crate::utils::paths::fastkg_default_port)
        )
    } else {
        search_url.to_string()
    };

    let client = reqwest::Client::new();
    let mut request_body = serde_json::json!({
        "include_trace": true,
        "query": query,
        "rerank": true,
        "top_k": top_k,
        "knowledge_base_ids": kb_ids,
    });
    // 检索接口使用 raw_file_id 字段（字符串），多文件时取第一个
    if !file_ids.is_empty() {
        request_body["raw_file_id"] = serde_json::json!(file_ids[0]);
        if file_ids.len() > 1 {
            tracing::warn!(
                "search 接口 raw_file_id 仅支持单值，已忽略其余 {} 个文件 ID",
                file_ids.len() - 1
            );
        }
    }

    tracing::info!(
        "🔍 search_remote_knowledge: url={}, kb_ids={:?}, raw_file_id={:?}, query_preview={}",
        url,
        kb_ids,
        file_ids.first(),
        truncate_str(&query, 80)
    );

    let resp = client
        .post(&url)
        .header(
            crate::core::constants::FASTKG_SERVICE_KEY_HEADER,
            crate::core::constants::FASTKG_SERVICE_KEY,
        )
        .json(&request_body)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| AppError::Llm(format!("知识库检索请求失败: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Llm(format!(
            "知识库检索返回错误 ({}): {}",
            status.as_u16(),
            truncate_str(&body, 300)
        )));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Llm(format!("知识库检索响应解析失败: {}", e)))?;

    // 解析响应: { success, data: { results: [...] } }
    let data = json
        .get("data")
        .ok_or_else(|| AppError::Llm("知识库检索响应缺少 data 字段".to_string()))?;

    let results = data
        .get("results")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::Llm("知识库检索响应缺少 results 数组".to_string()))?;

    if results.is_empty() {
        tracing::info!("[远端知识库] 未检索到相关内容");
        return Ok((String::new(), Vec::new()));
    }

    tracing::info!("[远端知识库] 检索到 {} 条结果", results.len());

    let mut ctx = String::new();
    let mut refs: Vec<serde_json::Value> = Vec::new();

    for (i, item) in results.iter().enumerate() {
        let content = item.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let score = item.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let chunk_id = item.get("chunk_id").and_then(|v| v.as_str()).unwrap_or("");
        let source = item.get("source").and_then(|v| v.as_str()).unwrap_or("");
        let document_name = item
            .get("trace")
            .and_then(|t| t.get("raw_file_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        tracing::info!(
            "[远端知识库] 片段{} | 相关度={:.2} | 来源={} | 文档={} | chunk_id={}",
            i + 1,
            score,
            source,
            document_name,
            chunk_id
        );
        tracing::debug!("[远端知识库] 片段{} 内容:\n{}", i + 1, content);

        ctx.push_str(&format!(
            "[片段{}] (文档名: {}) (相关度 {:.2})\n{}\n\n",
            i + 1,
            document_name,
            score,
            content
        ));

        refs.push(serde_json::json!({
            "chunk_id": chunk_id,
            "document_name": document_name,
            "source": source,
            "score": score,
            "preview": content.chars().take(200).collect::<String>(),
        }));
    }

    Ok((ctx, refs))
}

/// 读取本地知识库（文件夹或单文件）中的文本/文档内容，组装为上下文
///
/// 支持两类 entry：
///   - 文件夹型：递归/非递归列出可读文件（纯文本 + docx/xlsx），逐个抽取内容
///   - 单文件型：按扩展名直接抽取该文件内容（docx/xlsx 走文档解析，文本走 read）
fn read_local_knowledge(entries: &[LocalKbEntry]) -> String {
    const MAX_TOTAL_BYTES: usize = 200_000;
    const MAX_FILE_BYTES: usize = 10_000;

    let text_exts: std::collections::HashSet<&str> = [
        "txt", "md", "py", "js", "ts", "vue", "json", "yaml", "yml", "xml", "html", "css", "scss",
        "rs", "toml", "cfg", "ini", "env", "sh", "bat", "ps1", "sql", "java", "go", "c", "h",
        "cpp", "hpp", "cs", "rb", "php", "swift", "kt", "r", "lua", "conf", "log", "tsx", "jsx",
        "less", "sass", "graphql", "prisma", "proto", "cmake", "svg",
    ]
    .iter()
    .copied()
    .collect();

    let mut result = String::new();
    let mut total_bytes = 0usize;

    for entry in entries {
        let path = std::path::Path::new(&entry.path);

        if path.is_dir() {
            // ── 文件夹型 ──
            result.push_str(&format!("\n## 本地知识库文件夹: {}\n\n", entry.path));

            let mut files = match list_readable_files(path, &text_exts, entry.recursive) {
                Ok(f) => f,
                Err(_) => continue,
            };
            files.sort();

            for file_path in files {
                let file_name = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");

                let content = match read_file_content(&file_path, &text_exts) {
                    Some(c) => c,
                    None => continue,
                };

                let truncated = truncate_content(content, MAX_FILE_BYTES);

                if total_bytes + truncated.len() > MAX_TOTAL_BYTES {
                    result.push_str("...(已达总量上限，省略后续文件)\n");
                    return result;
                }

                result.push_str(&format!("=== 文件: {} ===\n{}\n", file_name, truncated));
                total_bytes += truncated.len();
            }
        } else if path.is_file() {
            // ── 单文件型 ──
            let content = match read_file_content(path, &text_exts) {
                Some(c) => c,
                None => continue,
            };
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let truncated = truncate_content(content, MAX_FILE_BYTES);

            if total_bytes + truncated.len() > MAX_TOTAL_BYTES {
                result.push_str("...(已达总量上限，省略后续文件)\n");
                return result;
            }

            result.push_str(&format!(
                "\n## 本地知识库文件: {}\n\n=== 文件: {} ===\n{}\n",
                entry.path, file_name, truncated
            ));
            total_bytes += truncated.len();
        }
    }

    result
}

/// 读取单个文件内容：docx/xlsx 走文档文本抽取，纯文本直接读；其余返回 None
fn read_file_content(
    path: &std::path::Path,
    text_exts: &std::collections::HashSet<&str>,
) -> Option<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "docx" => crate::core::doc_extract::extract_docx_text(path).ok(),
        "xlsx" => crate::core::doc_extract::extract_xlsx_text(path).ok(),
        _ => {
            if !text_exts.contains(ext.as_str()) {
                return None;
            }
            let data = std::fs::read(path).ok()?;
            // 跳过二进制文件
            if !data.is_empty() && data.iter().take(512).any(|&b| b == 0) {
                return None;
            }
            Some(String::from_utf8_lossy(&data).into_owned())
        }
    }
}

/// 按字节上限截断文件内容（截断时补提示）
fn truncate_content(content: String, max_bytes: usize) -> String {
    if content.len() > max_bytes {
        let trunc: String = content.chars().take(max_bytes).collect();
        format!("{}...(文件过长，已截断)\n", trunc)
    } else {
        format!("{}\n", content)
    }
}

fn list_readable_files(
    dir: &std::path::Path,
    exts: &std::collections::HashSet<&str>,
    recursive: bool,
) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() && recursive {
            if let Ok(sub) = list_readable_files(&path, exts, recursive) {
                files.extend(sub);
            }
        } else if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            // 纯文本 + docx/xlsx 均可读
            if exts.contains(ext.as_str()) || ext == "docx" || ext == "xlsx" {
                files.push(path);
            }
        }
    }
    Ok(files)
}

/// 普通对话模式：用户消息 → 记忆 → RAG → LLM (带工具调用循环) → 回答
async fn normal_chat(
    state: &AppState,
    conversation_id: &str,
    content: &str,
    model_id: Option<&str>,
) -> Result<MessageResponse, AppError> {
    // 1. 保存用户消息
    let user_msg_id = uuid::Uuid::new_v4().to_string();
    repo::messages::insert(
        &*state.db.pool().await,
        &repo::messages::NewMessage {
            id: user_msg_id.clone(),
            conversation_id: conversation_id.to_string(),
            role: "user".to_string(),
            content: content.to_string(),
            model_id: None,
            model_instance_id: None,
            parent_id: None,
            group_id: None,
            attachments: None,
            token_input: None,
            token_output: None,
        },
    )
    .await?;

    let memory_ctx = String::new();

    // 3. 检索挂载的知识库 (RAG) — 由 FastKG 服务提供，此处保留占位扩展点
    let kb_context = String::new();

    // 4. 构建 System Prompt（含模型身份 + 工具策略）
    let tool_free = crate::core::tools::selection::is_tool_free_question(content);
    let model_display = state.llm.current_model_display(model_id).await;
    let system_prompt = build_system_prompt_v2(
        &memory_ctx,
        &kb_context,
        "",
        None,
        Some(model_display.as_str()),
        !tool_free,
        // normal_chat 这条路径拿不到会话级 skill 白名单（未被 send_message_stream 调用）
        &[],
    );

    // 5. 获取对话历史
    let history = repo::messages::get_recent(&*state.db.pool().await, conversation_id, 20).await?;

    // 6. 收集可用工具定义（场景裁剪 + 渐进式披露）
    let (tools, catalog_block) = if tool_free {
        (Vec::new(), String::new())
    } else {
        let all = collect_tool_definitions(state).await;
        let loaded = crate::core::tools::disclosure::get_loaded(conversation_id);
        let scoped = crate::core::tools::selection::scope_tools(all.clone(), content, &loaded);
        let injected: Vec<String> = scoped.iter().map(|d| d.function.name.clone()).collect();
        let catalog = crate::core::tools::disclosure::build_catalog_block(&all, &injected);
        (scoped, catalog)
    };

    let system_prompt = if catalog_block.is_empty() {
        system_prompt
    } else {
        format!("{system_prompt}{catalog_block}")
    };

    // 6b. 智能体偏好参数（温度/采样等）
    let sampling = load_agent_sampling(&*state.db.pool().await, conversation_id).await;

    // 7. 工具调用循环
    let mut total_input = 0i32;
    let mut total_output = 0i32;
    let mut total_cost = 0.0f64;
    let mut extra_messages: Vec<serde_json::Value> = Vec::new();
    let mut tool_call_log: Vec<ToolCallRecord> = Vec::new();
    let mut final_content = String::new();
    let mut final_model = String::new();
    let mut final_provider_code = String::new();
    // ★ 跨轮护栏：重复调用去重 + 连续失败熔断
    let mut guard = ToolGuard::default();

    for round in 0..MAX_TOOL_ROUNDS {
        // 调用 LLM（带工具定义 + 历史工具调用结果）
        let llm_response = state
            .llm
            .chat_with_tools(
                model_id,
                &system_prompt,
                &history,
                content,
                &tools,
                &extra_messages,
                sampling.as_ref(),
                &[],
                false,
            )
            .await
            .map_err(|e| AppError::Llm(e.to_string()))?;

        total_input += llm_response.token_input;
        total_output += llm_response.token_output;
        total_cost += llm_response.cost_usd;
        final_model = llm_response.model.clone();
        final_provider_code = llm_response.provider_code.clone();

        // 检查是否有工具调用
        if llm_response.tool_calls.is_empty() || llm_response.finish_reason == "stop" {
            // 无工具调用或 LLM 明确停止 → 返回最终回答
            final_content = llm_response.content;
            break;
        }

        info!(
            "🔧 第 {} 轮工具调用: {} 个工具",
            round + 1,
            llm_response.tool_calls.len()
        );

        // 将 assistant 的 tool_calls 消息加入上下文
        extra_messages.push(serde_json::json!({
            "role": "assistant",
            "content": llm_response.content.clone(),
            "tool_calls": llm_response.tool_calls,
        }));

        // 逐个执行工具调用
        for tc in &llm_response.tool_calls {
            let tool_name = &tc.function.name;
            let tool_args: serde_json::Value =
                serde_json::from_str(&tc.function.arguments).unwrap_or(serde_json::json!({}));

            info!("  → 执行工具: {} (id={})", tool_name, tc.id);

            // ★ 护栏：重复调用去重 + 连续失败熔断
            //   先结束对 guard 的不可变借用，再调用需要 &mut 的 record
            let blocked_reason = guard.check(tool_name, &tool_args);
            let (tool_content, tool_success) = if let Some(reason) = blocked_reason {
                warn!("  ⛔ 护栏拦截 {}", tool_name);
                (reason, false)
            } else {
                // 执行工具 → 分发到 MCP 或 Skills
                let result =
                    execute_tool(state, tool_name, tool_args.clone(), conversation_id).await;
                let (s, ok) = match result {
                    Ok(val) => {
                        let s = if val.is_string() {
                            val.as_str().unwrap_or("").to_string()
                        } else {
                            serde_json::to_string_pretty(&val).unwrap_or_default()
                        };
                        (s, true)
                    }
                    Err(e) => {
                        warn!("  ✗ 工具 {} 执行失败: {}", tool_name, e);
                        (format!("工具执行失败: {}", e), false)
                    }
                };
                guard.record(tool_name, &tool_args, ok);
                (s, ok)
            };

            // ★ 单条工具结果硬上限
            let tool_content = crate::core::llm::context_budget::truncate_tool_result(
                &tool_content,
                crate::core::llm::context_budget::MAX_TOOL_RESULT_CHARS,
            );

            // 记录工具调用日志
            tool_call_log.push(ToolCallRecord {
                tool_call_id: tc.id.clone(),
                tool_name: tool_name.clone(),
                arguments: tool_args,
                result: tool_content.clone(),
                success: tool_success,
            });

            // 将工具结果回注到消息上下文（OpenAI tool message 格式）
            extra_messages.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": tc.id,
                "content": tool_content,
            }));
        }

        // 继续循环，让 LLM 基于工具结果生成最终回答
    }

    // ★ 轮次耗尽 → 摘掉 tools 强制收敛作答（与流式路径保持一致）
    if final_content.is_empty() && !extra_messages.is_empty() {
        info!("⏱️ 工具轮次耗尽，强制基于已有信息收敛作答");
        let mut closing = extra_messages.clone();
        closing.push(serde_json::json!({
            "role": "system",
            "content": "[系统] 工具调用次数已用尽。请**立即基于上面已经获得的信息**给出你的最终回答。\
                        不要再请求调用任何工具。如果信息不足以完全回答，就回答你能确定的部分，\
                        并明确说明哪部分没能查到、原因是什么。不要让用户重新提问。",
        }));

        match state
            .llm
            .chat_with_tools(
                model_id,
                &system_prompt,
                &history,
                content,
                &[], // 不传 tools
                &extra_messages,
                sampling.as_ref(),
                &[],
                false,
            )
            .await
        {
            Ok(resp) => {
                total_input += resp.token_input;
                total_output += resp.token_output;
                total_cost += resp.cost_usd;
                if !resp.content.trim().is_empty() {
                    final_content = resp.content;
                }
            }
            Err(e) => warn!("收敛作答失败: {}", e),
        }
    }

    if final_content.trim().is_empty() {
        final_content = "抱歉，我在查询过程中没能得到可用的结果。\n\n\
                         你可以换一个更具体的问法，或者告诉我你希望我重点看哪一部分。"
            .to_string();
    }

    // 8. 保存 AI 回复
    let ai_msg_id = uuid::Uuid::new_v4().to_string();
    let model_instance_id = lookup_model_instance_id(
        &*state.db.pool().await,
        model_id,
        &final_model,
        &final_provider_code,
    )
    .await;
    repo::messages::insert(
        &*state.db.pool().await,
        &repo::messages::NewMessage {
            id: ai_msg_id.clone(),
            conversation_id: conversation_id.to_string(),
            role: "assistant".to_string(),
            content: final_content.clone(),
            model_id: Some(final_model.clone()),
            model_instance_id,
            parent_id: Some(user_msg_id),
            group_id: None,
            attachments: None,
            token_input: None,
            token_output: None,
        },
    )
    .await?;

    // 9. 保存工具调用记录（作为 tool role 消息）
    for record in &tool_call_log {
        let tool_msg_id = uuid::Uuid::new_v4().to_string();
        repo::messages::insert(
            &*state.db.pool().await,
            &repo::messages::NewMessage {
                id: tool_msg_id,
                conversation_id: conversation_id.to_string(),
                role: "tool".to_string(),
                content: serde_json::json!({
                    "tool_call_id": record.tool_call_id,
                    "tool_name": record.tool_name,
                    "arguments": record.arguments,
                    "result": record.result,
                    "success": record.success,
                })
                .to_string(),
                model_id: None,
                model_instance_id: None,
                parent_id: Some(ai_msg_id.clone()),
                group_id: None,
                attachments: None,
                token_input: None,
                token_output: None,
            },
        )
        .await?;
    }

    // 10. 记录总 token 消耗
    repo::cost::insert_usage(
        &*state.db.pool().await,
        &repo::cost::TokenUsage {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: "default-user".to_string(),
            conversation_id: Some(conversation_id.to_string()),
            message_id: Some(ai_msg_id.clone()),
            provider_id: final_provider_code.clone(),
            provider_code: final_provider_code.clone(),
            model_id: final_model.clone(),
            token_input: total_input,
            token_output: total_output,
            cost_usd: total_cost,
            call_type: if tool_call_log.is_empty() {
                "chat".to_string()
            } else {
                "tool_use".to_string()
            },
        },
    )
    .await?;

    // 11. 异步更新记忆
    let pool = state.db.pool().await.clone();
    let content_clone = content.to_string();
    let response_clone = final_content.clone();
    tokio::spawn(async move {
        // TODO: 更新情景记忆、语义记忆
        let _ = (&pool, &content_clone, &response_clone);
    });

    Ok(MessageResponse {
        message_id: ai_msg_id,
        content: final_content,
        model: final_model,
        token_input: total_input,
        token_output: total_output,
        references: vec![],
        tool_calls_count: tool_call_log.len() as i32,
    })
}

// ═══════════════════════════════════════════════════════════════
//  工具收集 & 执行
// ═══════════════════════════════════════════════════════════════

/// 从 MCP Manager + Skills Registry 收集所有可用工具定义
/// 转换为 OpenAI tools 格式（终态：内置工具 + mcp__，移除 skill__）
async fn collect_tool_definitions(state: &AppState) -> Vec<ToolDefinition> {
    collect_tool_definitions_filtered(&state.tool_registry, &state.mcp, true, None).await
}

async fn collect_tool_definitions_filtered(
    tools: &crate::core::tools::registry::SharedToolRegistry,
    mcp: &crate::core::mcp::SharedMcpManager,
    include_mcp: bool,
    mcp_server_ids: Option<&[String]>,
) -> Vec<ToolDefinition> {
    let mut result = Vec::new();

    // ① 内置工具（无前缀，从 ToolRegistry 收集）
    {
        let registry = tools.read().await;
        result.extend(registry.list_definitions());
    }

    // ② MCP 工具（前缀 mcp__）
    if include_mcp {
        let manager = mcp.read().await;
        for (server_id, mcp_tool) in manager.list_all_tools() {
            if let Some(ids) = mcp_server_ids {
                if !ids.is_empty() && !ids.iter().any(|id| id == &server_id) {
                    continue;
                }
            }
            result.push(ToolDefinition {
                r#type: "function".to_string(),
                function: ToolFunctionDef {
                    name: format!("mcp__{}__{}", server_id, mcp_tool.name),
                    description: mcp_tool.description.clone(),
                    parameters: if mcp_tool.input_schema.is_null()
                        || mcp_tool.input_schema == serde_json::json!({})
                    {
                        None
                    } else {
                        Some(mcp_tool.input_schema.clone())
                    },
                },
            });
        }
    }

    result
}

/// MCP 工具调用循环（非 token 级流式，但向前端推送进度与最终回答）
async fn chat_with_mcp_tools_loop(
    app: &AppHandle,
    llm: &std::sync::Arc<crate::core::llm::LlmManager>,
    mcp: &crate::core::mcp::SharedMcpManager,
    tool_registry: &crate::core::tools::registry::SharedToolRegistry,
    pool: &sqlx::SqlitePool,
    conversation_id: &str,
    content: &str,
    model_id: Option<&str>,
    system_prompt: &str,
    history: &[MessageRow],
    tools: &[ToolDefinition],
    sampling: Option<&crate::core::llm::SamplingParams>,
    enable_thinking: bool,
    // 本次对话的 Skill 白名单，原样透给 execute_tool_with
    enabled_skills: Option<&[String]>,
) -> Result<(crate::core::llm::LlmResponse, Vec<crate::core::grounding::ToolCall>), AppError> {
    use crate::core::llm::streaming::StreamUpdate;
    let on_update = |update| {
        let (event, payload) = match update {
            StreamUpdate::Reset => ("chat-stream-reset", serde_json::json!({
                "conversation_id": conversation_id,
            })),
            StreamUpdate::Token(token) => ("chat-stream-token", serde_json::json!({
                "conversation_id": conversation_id, "token": token,
            })),
            StreamUpdate::Retry { attempt, max_retries, reason } => ("chat-stream-retry", serde_json::json!({
                "conversation_id": conversation_id, "attempt": attempt,
                "max_retries": max_retries, "reason": reason,
            })),
        };
        let _ = app.emit(event, payload);
    };
    let mut extra_messages: Vec<serde_json::Value> = Vec::new();
    let mut total_input = 0i32;
    let mut total_output = 0i32;
    let mut total_cost = 0.0f64;
    let mut final_content = String::new();
    let mut final_model = String::new();
    let mut final_provider_code = String::new();
    let mut final_stream_error = None;
    let mut final_finish_reason = "stop".to_string();
    let mut tool_log_entries: Vec<serde_json::Value> = Vec::new();
    // ★ 证据台账：本轮每一次工具调用（含失败）连同**返回全文**。
    //   与 tool_log_entries 的区别：那边是给前端渲染卡片的 500 字预览，这边是给
    //   接地校验用的完整证据——拿预览去判定，被截断掉的那半段里的数值会被误判成
    //   「无出处」。两者用途不同，不能合并。
    let mut grounding_calls: Vec<crate::core::grounding::ToolCall> = Vec::new();
    // ★ 跨轮护栏：重复调用去重 + 连续失败熔断
    let mut guard = ToolGuard::default();

    // 进入工具循环时立即通知前端「AI 正在思考」，避免白框
    let _ = app.emit(
        "chat-stream-status",
        serde_json::json!({
            "conversation_id": conversation_id, "status": "thinking"
        }),
    );

    for round in 0..MAX_TOOL_ROUNDS {
        // ★ 每轮心跳：让前端能显示「第 N/6 轮」进度，而不是无限转圈
        //   （反馈单 298：动作停止了但转圈依旧）
        let _ = app.emit(
            "chat-stream-status",
            serde_json::json!({
                "conversation_id": conversation_id,
                "status": "thinking",
                "round": round + 1,
                "max_rounds": MAX_TOOL_ROUNDS,
            }),
        );

        let llm_response = llm
            .chat_with_tools_stream(
                model_id,
                system_prompt,
                history,
                content,
                tools,
                &extra_messages,
                sampling,
                &[],
                enable_thinking,
                Some(&on_update),
            )
            .await
            .map_err(|e| AppError::Llm(e.to_string()))?;

        total_input += llm_response.token_input;
        total_output += llm_response.token_output;
        total_cost += llm_response.cost_usd;
        final_model = llm_response.model.clone();
        final_provider_code = llm_response.provider_code.clone();

        // 中断流可能含未完成的工具参数；保留部分正文并报告错误，不执行残缺调用。
        if llm_response.stream_error.is_some() || llm_response.tool_calls.is_empty() || llm_response.finish_reason == "stop" {
            final_stream_error = llm_response.stream_error;
            final_finish_reason = llm_response.finish_reason;
            final_content = llm_response.content;
            break;
        }

        info!(
            "🔧 第 {} 轮工具调用: {} 个工具",
            round + 1,
            llm_response.tool_calls.len()
        );

        extra_messages.push(serde_json::json!({
            "role": "assistant",
            "content": llm_response.content.clone(),
            "tool_calls": llm_response.tool_calls,
        }));

        for tc in &llm_response.tool_calls {
            let tool_name = &tc.function.name;
            let tool_args: serde_json::Value =
                serde_json::from_str(&tc.function.arguments).unwrap_or(serde_json::json!({}));

            // 结构化工具调用事件：status=running
            // ★ 增加 title（人话描述）与 round，供前端渲染可读卡片
            let kind = tool_kind(tool_name);
            let title = tool_display_title(tool_name, &tool_args);
            let _ = app.emit(
                "chat-tool-call",
                serde_json::json!({
                    "conversation_id": conversation_id,
                    "id": tc.id,
                    "name": tool_name,
                    "title": &title,
                    "kind": kind,
                    "args": tool_args,
                    "status": "running",
                    "round": round + 1,
                }),
            );

            // ★ 护栏检查：重复调用 / 连续失败熔断。
            //   命中时不真正执行工具，而是把拒绝理由作为 tool 结果回注，
            //   让模型明确知道"这条路走不通"，而不是继续盲目重试。
            //
            //   注意：先把 check 的结果收进局部变量，结束对 guard 的不可变借用，
            //   之后才能调用需要 &mut 的 record。
            let started = std::time::Instant::now();
            let blocked_reason = guard.check(tool_name, &tool_args);
            let (tool_content, exec_ok, blocked) = if let Some(reason) = blocked_reason {
                warn!(
                    "  ⛔ 护栏拦截 {}: {}",
                    tool_name,
                    reason.lines().next().unwrap_or("")
                );
                (reason, false, true)
            } else {
                let result = execute_tool_with(
                    tool_registry,
                    mcp,
                    Some(llm),
                    tool_name,
                    tool_args.clone(),
                    conversation_id,
                    enabled_skills,
                )
                .await;
                let ok = result.is_ok();
                let content = match &result {
                    Ok(val) => {
                        if val.is_string() {
                            val.as_str().unwrap_or("").to_string()
                        } else {
                            serde_json::to_string_pretty(val).unwrap_or_default()
                        }
                    }
                    Err(e) => {
                        warn!("  ✗ 工具 {} 执行失败: {}", tool_name, e);
                        format!("工具执行失败: {}", e)
                    }
                };
                guard.record(tool_name, &tool_args, ok);
                (content, ok, false)
            };
            let duration_ms = started.elapsed().as_millis() as u64;

            // ★ 单条工具结果硬上限：防止某个工具（bash/read_file 原本无上限）
            //   单点把上下文撑爆
            let tool_content = crate::core::llm::context_budget::truncate_tool_result(
                &tool_content,
                crate::core::llm::context_budget::MAX_TOOL_RESULT_CHARS,
            );

            // ★ 产出文件：让前端能渲染打开/下载/另存按钮
            let artifacts = if exec_ok {
                extract_artifacts(tool_name, &tool_args, &tool_content)
            } else {
                Vec::new()
            };

            let result_preview = if tool_content.chars().count() > 500 {
                format!("{}...", tool_content.chars().take(500).collect::<String>())
            } else {
                tool_content.clone()
            };
            let final_status = if blocked {
                "blocked"
            } else if exec_ok {
                "done"
            } else {
                "error"
            };

            // ★ 证据台账记的是模型**实际看到的那份**内容（上面刚做过长度截断）。
            //   若拿未截断的原文比对，模型没收到的那部分也会被算成"有出处"。
            //   被护栏拦下的调用同样记为失败——它确实没有产生数据。
            grounding_calls.push(if exec_ok {
                crate::core::grounding::ToolCall::ok(tool_name, tool_args.to_string(), &tool_content)
            } else {
                crate::core::grounding::ToolCall::failed(
                    tool_name,
                    tool_args.to_string(),
                    &tool_content,
                )
            });
            let _ = app.emit(
                "chat-tool-call",
                serde_json::json!({
                    "conversation_id": conversation_id,
                    "id": tc.id,
                    "name": tool_name,
                    "title": &title,
                    "kind": kind,
                    "status": final_status,
                    "result_preview": result_preview,
                    "artifacts": artifacts,
                    "duration_ms": duration_ms,
                    "round": round + 1,
                }),
            );
            let tool_msg_id = uuid::Uuid::new_v4().to_string();
            let _ = repo::messages::insert(
                pool,
                &repo::messages::NewMessage {
                    id: tool_msg_id.clone(),
                    conversation_id: conversation_id.to_string(),
                    role: "tool".into(),
                    content: serde_json::json!({
                        "tool_call_id": tc.id,
                        "tool_name": tool_name,
                        "arguments": tool_args,
                        "result": tool_content,
                        "success": exec_ok,
                    })
                    .to_string(),
                    model_id: None,
                    model_instance_id: None,
                    parent_id: None,
                    group_id: None,
                    attachments: None,
                    token_input: None,
                    token_output: None,
                },
            )
            .await;
            let extra = serde_json::json!({
                "toolName": tool_name,
                "toolId": tc.id,
                "input": tool_args,
                "isError": !exec_ok,
            })
            .to_string();
            let _ = repo::messages::set_extra_data(pool, &tool_msg_id, &extra).await;

            // 持久化：记录工具调用明细供前端刷新后重建卡片
            tool_log_entries.push(serde_json::json!({
                "id": tc.id,
                "name": tool_name,
                "title": &title,
                "kind": kind,
                "status": final_status,
                "result_preview": result_preview,
                "artifacts": artifacts,
                "duration_ms": duration_ms,
                "round": round + 1,
            }));

            extra_messages.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": tc.id,
                "content": tool_content,
            }));
        }
    }

    // ★ 轮次耗尽 ≠ 失败，而是「该收敛了」。
    //
    //   原实现直接返回「工具调用轮次已达上限，请重新提问。」——
    //   系统已经花了十几万 token、调用了 6 个工具、拿到了大量内容，
    //   然后把全部工作成果丢弃，还引导用户按同样路径再烧一遍。
    //
    //   正确做法：把 tools 摘掉再请求一次，强制模型基于已有信息作答。
    if final_content.is_empty() && final_stream_error.is_none() && !extra_messages.is_empty() {
        info!("⏱️ 工具轮次耗尽，强制基于已有信息收敛作答");
        let _ = app.emit(
            "chat-stream-status",
            serde_json::json!({
                "conversation_id": conversation_id,
                "status": "finalizing",
            }),
        );

        let mut closing = extra_messages.clone();
        closing.push(serde_json::json!({
            "role": "system",
            "content": "[系统] 工具调用次数已用尽。请**立即基于上面已经获得的信息**给出你的最终回答。\
                        不要再请求调用任何工具。如果信息不足以完全回答，就回答你能确定的部分，\
                        并明确说明哪部分没能查到、原因是什么。不要让用户重新提问。",
        }));

        match llm
            .chat_with_tools_stream(
                model_id,
                system_prompt,
                history,
                content,
                &[], // ★ 关键：不传 tools，模型无法再发起工具调用
                &closing,
                sampling,
                &[],
                enable_thinking,
                Some(&on_update),
            )
            .await
        {
            Ok(resp) => {
                total_input += resp.token_input;
                total_output += resp.token_output;
                total_cost += resp.cost_usd;
                final_stream_error = resp.stream_error;
                final_finish_reason = resp.finish_reason;
                if !resp.content.trim().is_empty() {
                    final_content = resp.content;
                }
            }
            Err(e) => {
                warn!("收敛作答失败: {}", e);
            }
        }
    }

    // 兜底：连收敛请求都失败了，也要给出有信息量的说明，而不是"请重新提问"
    if final_content.trim().is_empty() {
        final_content = "抱歉，我在查询过程中没能得到可用的结果。\n\n\
                         已尝试的操作可以在上方的工具调用记录里看到。\
                         你可以换一个更具体的问法，或者告诉我你希望我重点看哪一部分。"
            .to_string();
    }

    // ★ 终态广播：无论正常收敛还是轮次耗尽，都必须把状态打回 idle。
    //   反馈单 298「动作停止了，但转圈依旧」的根因就是这里缺失 ——
    //   进入循环时 emit 了 thinking，工具完成时只 emit 了 chat-tool-call 的 done，
    //   全局 thinking 状态从来没有人清除，前端只能一直转圈。
    let _ = app.emit(
        "chat-stream-status",
        serde_json::json!({
            "conversation_id": conversation_id,
            "status": "idle",
            "rounds_used": tool_log_entries
                .iter()
                .filter_map(|e| e.get("round").and_then(|r| r.as_u64()))
                .max()
                .unwrap_or(0),
        }),
    );

    // 增量正文已由 on_update 转发；done 只校准最终正文，不再追加全文 token。

    let log_json = if tool_log_entries.is_empty() {
        None
    } else {
        serde_json::to_string(&tool_log_entries).ok()
    };

    Ok((
        crate::core::llm::LlmResponse {
            content: final_content,
            model: final_model,
            provider: final_provider_code.clone(),
            provider_code: final_provider_code,
            token_input: total_input,
            token_output: total_output,
            cost_usd: total_cost,
            duration_ms: 0,
            tool_calls: vec![],
            finish_reason: final_finish_reason,
            tool_call_log_json: log_json,
            stream_error: final_stream_error,
        },
        grounding_calls,
    ))
}

/// 按工具名分类：mcp__ → "mcp"、skill/run_skill → "skill"、其余 → "builtin"
fn tool_kind(name: &str) -> &'static str {
    if name.starts_with("mcp__") {
        "mcp"
    } else if name == "skill" || name == "run_skill" {
        "skill"
    } else {
        "builtin"
    }
}

// ── 工具调用护栏（Guardrails）────────────────────────────────────────
//
// 反馈单 305「一堆 bash，对用户来说不知道是干什么」和 306「搜索都是重复的」
// 的直接成因：循环里没有任何"别再这么干了"的约束，模型可以无限重复
// 同一个失败动作，直到轮次烧光。

/// 工具调用护栏状态：跨轮次追踪重复调用与连续失败。
#[derive(Default)]
struct ToolGuard {
    /// 已执行过的调用指纹（工具名 + 归一化参数）
    seen: std::collections::HashSet<String>,
    /// 每个工具的连续失败次数
    consecutive_failures: std::collections::HashMap<String, u32>,
}

/// 同一工具连续失败多少次后拒绝继续执行
const MAX_CONSECUTIVE_FAILURES: u32 = 2;

impl ToolGuard {
    /// 生成调用指纹。参数做归一化（去空白、小写）以捕捉"换个说法再搜一遍"。
    fn fingerprint(name: &str, args: &serde_json::Value) -> String {
        let mut norm = args.to_string().to_lowercase();
        norm.retain(|c| !c.is_whitespace());
        format!("{name}::{norm}")
    }

    /// 调用前检查。返回 Some(拒绝理由) 表示应跳过本次执行。
    fn check(&self, name: &str, args: &serde_json::Value) -> Option<String> {
        if self.consecutive_failures.get(name).copied().unwrap_or(0) >= MAX_CONSECUTIVE_FAILURES {
            return Some(format!(
                "GUARD_TOO_MANY_FAILURES: 工具「{name}」已连续失败 {MAX_CONSECUTIVE_FAILURES} 次，已停止重试。\n\
                 请不要再调用它，也不要用其他工具手工模拟它的功能。\n\
                 请基于目前已有的信息回答用户，并说明这部分没能完成。"
            ));
        }
        if self.seen.contains(&Self::fingerprint(name, args)) {
            return Some(format!(
                "GUARD_DUPLICATE_CALL: 你已经用完全相同的参数调用过「{name}」了，结果就在上文。\n\
                 重复调用不会得到新信息。请直接基于已有结果作答；\n\
                 如果确实需要不同信息，请换一个实质不同的查询，而不是换个说法重搜。"
            ));
        }
        None
    }

    /// 记录一次执行结果。
    fn record(&mut self, name: &str, args: &serde_json::Value, success: bool) {
        self.seen.insert(Self::fingerprint(name, args));
        if success {
            self.consecutive_failures.remove(name);
        } else {
            *self
                .consecutive_failures
                .entry(name.to_string())
                .or_insert(0) += 1;
        }
    }
}

/// 工具名 → 面向用户的中文描述。
///
/// 反馈单 305：用户看到的是 `bash`、`glob`、`web_fetch` 这些开发者词汇，
/// 完全不知道系统在干什么。前端应优先渲染这个 title 而非裸工具名。
fn tool_display_title(name: &str, args: &serde_json::Value) -> String {
    let arg_str = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("");
    let brief = |s: &str, n: usize| -> String {
        let t: String = s.chars().take(n).collect();
        if s.chars().count() > n {
            format!("{t}…")
        } else {
            t
        }
    };

    match name {
        "web_search" => format!("联网搜索「{}」", brief(arg_str("query"), 30)),
        "get_weather" => format!("查询{}天气", brief(arg_str("location"), 20)),
        "get_exchange_rate" => format!(
            "查询汇率 {}→{}",
            brief(arg_str("from"), 10),
            brief(arg_str("to"), 16)
        ),
        "get_crypto_price" => format!("查询币价 {}", brief(arg_str("coins"), 20)),
        "get_location_info" => format!("查询地点 {}", brief(arg_str("location"), 20)),
        "web_fetch" => format!("读取网页 {}", brief(arg_str("url"), 40)),
        "read_file" => format!("读取文件 {}", brief(arg_str("file_path"), 40)),
        "write_file" => format!("写入文件 {}", brief(arg_str("file_path"), 40)),
        "edit_file" => format!("修改文件 {}", brief(arg_str("file_path"), 40)),
        "apply_patch" | "batch_edit" => "批量修改文件".to_string(),
        "glob" => format!("查找文件 {}", brief(arg_str("pattern"), 30)),
        "grep" => format!("搜索内容「{}」", brief(arg_str("pattern"), 30)),
        "bash" => format!("执行命令 {}", brief(arg_str("command"), 50)),
        "download_file" => format!("下载文件 {}", brief(arg_str("url"), 40)),
        "run_skill" => format!("运行技能「{}」", brief(arg_str("skill_id"), 20)),
        "skill" => "加载技能模板".to_string(),
        "todo_write" => "更新任务清单".to_string(),
        "ToolSearch" => "查找可用工具".to_string(),
        "computer" => "操作桌面".to_string(),
        "Sleep" => "等待".to_string(),
        "SendMessage" => "发送消息".to_string(),
        n if n.starts_with("mcp__") => {
            let tail = n.rsplit("__").next().unwrap_or(n);
            format!("调用外部工具 {tail}")
        }
        other => format!("执行 {other}"),
    }
}

/// 从工具参数与结果中提取产出文件，供前端渲染"打开/下载/另存"按钮。
///
/// 反馈单 303「文件位置信息需要优化：C:\root\fastclaw\xjtu_portal.html」
/// 与 304「要把文件在界面中变成可以打开、可以下载、可以另存」的根因是：
/// 文件路径只是被 dump 在文本里，前端根本无法识别它是个文件。
fn extract_artifacts(
    tool_name: &str,
    args: &serde_json::Value,
    result: &str,
) -> Vec<serde_json::Value> {
    let mut out = Vec::new();

    let mut push_path = |p: &str| {
        let p = p.trim();
        if p.is_empty() {
            return;
        }
        let path = std::path::Path::new(p);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(p)
            .to_string();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        // 只上报真实存在的文件，避免把模型脑补的路径当成产出
        if size == 0 && !path.exists() {
            return;
        }
        out.push(serde_json::json!({
            "path": p,
            "name": name,
            "ext": ext,
            "size": size,
            // 前端据此决定展示哪些按钮
            "can_preview": matches!(
                ext.as_str(),
                "html" | "md" | "txt" | "json" | "csv" | "log" | "png" | "jpg" | "jpeg" | "svg"
            ),
            "can_download": true,
            "can_save_as": true,
        }));
    };

    match tool_name {
        "write_file" | "edit_file" | "apply_patch" => {
            if let Some(p) = args.get("file_path").and_then(|v| v.as_str()) {
                push_path(p);
            }
        }
        "download_file" => {
            if let Some(p) = args.get("output_path").and_then(|v| v.as_str()) {
                push_path(p);
            }
        }
        "run_skill" => {
            // run_skill 的结果里会带 "Generated files: a.xlsx, b.csv"
            if let Some(idx) = result.find("Generated files:") {
                let tail = &result[idx + "Generated files:".len()..];
                let line = tail.lines().next().unwrap_or("");
                for f in line.split(',') {
                    push_path(f);
                }
            }
        }
        _ => {}
    }

    out
}

async fn execute_tool_with(
    tools: &crate::core::tools::registry::SharedToolRegistry,
    mcp: &crate::core::mcp::SharedMcpManager,
    llm: Option<&std::sync::Arc<crate::core::llm::LlmManager>>,
    tool_name: &str,
    arguments: serde_json::Value,
    // ★ 真实会话 id：渐进式工具披露需要它来隔离各会话的"已加载工具"集合
    conversation_id: &str,
    // 本次对话的 Skill 白名单。None = 不限制；Some([]) = 一个都不允许。
    // 只对内置的「skill」工具生效（run_skill 是沙箱代理，不查本地清单）。
    enabled_skills: Option<&[String]>,
) -> Result<serde_json::Value, AppError> {
    // ① MCP 前缀 mcp__
    if let Some(rest) = tool_name.strip_prefix("mcp__") {
        let parts: Vec<&str> = rest.splitn(2, "__").collect();
        if parts.len() == 2 {
            let server_id = parts[0];
            let mcp_tool_name = parts[1];
            return crate::core::mcp::call_tool_shared(mcp, server_id, mcp_tool_name, arguments)
                .await;
        }
        return Err(AppError::Mcp(format!("MCP 工具名格式错误: {}", tool_name)));
    }

    // ② 内置工具（无前缀，经 ToolRegistry 执行）
    {
        let registry = tools.read().await;
        if registry.contains(tool_name) {
            // ★ 传入真实 conversation_id（原来是空字符串，导致 load_tool
            //   无法区分会话，所有对话会共用一份"已加载工具"集合）。
            //   working_dir 仍待后续补全。
            let mut ctx = crate::core::tools::ToolContext::minimal(
                dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from(".")),
                conversation_id.to_string(),
            );
            if let Some(l) = llm {
                ctx = ctx.with_llm((*l).clone());
            }
            // ★ 「本次对话」的 Skill 白名单：只在 Some 时注入。
            //   None 保持 ToolContext 默认（不限制），老路径行为不变。
            if let Some(list) = enabled_skills {
                ctx = ctx.with_enabled_skills(list.to_vec());
            }
            let result = registry.execute(tool_name, arguments, &ctx).await?;
            return Ok(serde_json::json!({
                "content": result.content,
                "is_error": result.is_error,
                "metadata": result.metadata,
            }));
        }
    }

    // ③ 回退：MCP 精确名称匹配
    {
        let manager = mcp.read().await;
        for (server_id, tool) in manager.list_all_tools() {
            if tool.name == tool_name {
                drop(manager);
                return crate::core::mcp::call_tool_shared(mcp, &server_id, tool_name, arguments)
                    .await;
            }
        }
    }

    Err(AppError::NotFound(format!("工具不存在: {}", tool_name)))
}

/// 根据工具名称前缀分发执行
async fn execute_tool(
    state: &AppState,
    tool_name: &str,
    arguments: serde_json::Value,
    conversation_id: &str,
) -> Result<serde_json::Value, AppError> {
    execute_tool_with(
        &state.tool_registry,
        &state.mcp,
        Some(&state.llm),
        tool_name,
        arguments,
        conversation_id,
        // 非流式路径（normal_chat）暂不带会话级白名单
        None,
    )
    .await
}

// ═══════════════════════════════════════════════════════════════
//  其他对话管理命令
// ═══════════════════════════════════════════════════════════════

/// 读取会话关联智能体的采样/偏好参数
/// 无关联智能体、未配置或解析失败时返回 None（沿用供应商默认值）
async fn load_agent_sampling(
    pool: &sqlx::SqlitePool,
    conversation_id: &str,
) -> Option<crate::core::llm::SamplingParams> {
    let raw = repo::agents::get_preferences_by_conversation(pool, conversation_id)
        .await
        .ok()
        .flatten()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    match serde_json::from_str::<crate::core::llm::SamplingParams>(trimmed) {
        Ok(s) => {
            tracing::info!("🎚️ 应用智能体偏好参数: {:?}", s);
            Some(s)
        }
        Err(e) => {
            tracing::warn!("智能体偏好参数解析失败,忽略: {}", e);
            None
        }
    }
}

/// 构建 System Prompt（注入真实系统时间 + 记忆 + 知识库 + 联网）
/// 稳定前缀：人设 + 工具使用策略。
///
/// ★ 这一段必须是**逐字节恒定**的，不能包含时间、模型名、记忆等任何易变内容，
///   否则 provider 的 prompt caching 会从第一个 token 就 miss。
///   易变内容一律排在本段之后。
const CORE_PERSONA_AND_TOOL_POLICY: &str = r#"你是 FastClaw 智能助手。回答简洁、直接、说人话。

# 工具使用策略

## 首要原则：先判断「要不要用工具」，再判断「用哪个」

绝大多数问题**不需要**调用任何工具。凭你自己的知识就能回答的，直接回答。
调工具是有成本的（耗时、花钱、可能失败），不是越多越好。

### 直接回答，不要调工具的情况
- 关于你自己的问题（你是谁、你用的什么模型、你能做什么）——答案在下方「当前运行环境」里，不要去翻文件系统
- 概念解释、原理说明、写代码、翻译、润色、头脑风暴
- 常识性事实、数学计算、逻辑推理
- 闲聊、寒暄、情绪回应
- 用户已经把所需信息贴在对话里了

### 需要调工具的情况
- 需要**实时**信息（今天的天气/新闻/股价/汇率）→ web_search
- 需要读写用户**本地**的具体文件 → read_file / write_file / edit_file
- 需要**执行**命令或脚本 → bash / run_skill

## 调用预算

| 问题类型 | 期望工具调用次数 |
|---|---|
| 简单事实（天气、汇率、某个数字） | **1 次**，搜索结果的摘要通常就够了 |
| 需要交叉验证的问题 | 2-3 次 |
| 多步骤任务（读文件→改→验证） | 3-6 次 |

**你最多只有 6 轮**。超过就会被强制中断。请把预算花在刀刃上。

## 停止规则（重要）

- **拿到足够信息就立刻停止检索，直接作答。** 不要为了"更全面"反复搜索。
- **不要重复调用相同或近似的查询。** 如果一次搜索没拿到想要的，换个思路或直接基于已有信息回答，不要换个说法再搜一遍。
- **不要对同一个 URL 抓取两次。**
- 如果某个工具连续失败 2 次，**停止重试**，向用户说明情况。
- 如果工具不可用（服务未启动等），**不要用 bash 手工模拟它的功能**，直接告诉用户。

## 工具仲裁

- 找文件用 `glob`，搜内容用 `grep`——**不要**用 `bash` 跑 `find`/`grep`
- 读文件用 `read_file`——**不要**用 `bash` 跑 `cat`
- **实时结构化数据一律走专用工具，绝对不要用 `web_search`**。
  这类数字在网页上多由 JS 渲染，静态抓取拿不到，搜再多轮也是白搭：

  | 问题类型 | 用这个工具 |
  |---|---|
  | 天气、气温、下不下雨 | `get_weather` |
  | 汇率、外币换算 | `get_exchange_rate` |
  | 比特币等加密货币价格 | `get_crypto_price` |
  | 某地经纬度、时区、当地几点 | `get_location_info` |

  只有当专用工具明确返回"网络不可用"或"不支持该项"时，才考虑其他办法。
- **用户补上缺失参数后，立刻在本轮把工具调掉。**
  比如你上一轮问"你在哪个城市"，用户这轮答"我在西安市"，
  那这一轮就要直接调 `get_weather(location="西安")` 并给出结果，
  **不要**只回复"好的，帮你查一下"然后什么都不做 —— 那样用户还得再催一次，
  而且你会把这次调用拖到下一个不相关的问题里，两件事混在一起答。
- 每一轮结束时，用户当前这个问题必须已经有答案，或者你明确说明了为什么答不了。
- `web_search` 的摘要够用时，**不要**再调 `web_fetch` 抓全文
- `web_search` 连续 2 次没拿到有用信息，就**停止搜索**，直接告诉用户没查到并给出替代建议，
  不要换关键词反复重试
- `bash` 是最后手段，只在没有专用工具时使用
- 需要的工具不在已加载列表里时，看 system prompt 末尾的「可按需加载的工具」清单，
  用 `load_tool` 加载后**下一轮**再调用；不要用 bash 硬凑
- `load_tool` 一次可传多个工具名，尽量一次传齐，避免多轮往返

## 执行纪律

- 调工具前，先在心里过一遍：这个调用能带来什么我现在没有的信息？答不上来就别调。
- 涉及本地文件时，不要臆造路径；不确定就先确认。
- 工具返回后，基于**真实结果**回答，不要脑补工具没说的内容。
- 工具失败时，如实告诉用户失败了、为什么失败，而不是假装成功或悄悄换方案。
"#;

/// 构建系统提示词。
///
/// ★ 段落顺序经过刻意设计，服务于 prompt caching：
///   [稳定前缀：人设 + 工具策略]  ← 逐字节恒定，可被缓存
///   [半稳定：运行环境 / 模型身份] ← 同一会话内不变
///   [易变：时间 / 记忆 / 知识库 / 搜索结果] ← 每轮都可能变，放最后
///
///   原实现把实时时间戳放在**第一段**，导致每一次请求都从第 1 个 token 起
///   cache miss，完全拿不到缓存折扣。
#[allow(dead_code)]
fn build_system_prompt(
    memory_ctx: &str,
    kb_context: &str,
    web_context: &str,
    extra: Option<&str>,
) -> String {
    build_system_prompt_v2(memory_ctx, kb_context, web_context, extra, None, true, &[])
}

/// 完整版：可传入当前模型显示名，并控制是否包含工具策略段。
///
/// - `model_display`：当前实际使用的模型名。修复「模型不知道自己是谁」——
///   此前这个信息只用于路由，从未进入 prompt，用户问"你底层模型是"时
///   模型只能猜，或者去调 glob 翻配置文件。
/// - `with_tools`：本轮是否会注入工具。纯问答场景下不带工具策略，
///   省掉约 600 token，也避免暗示模型"你应该调点什么"。
/// 本次对话勾选的 Skill → 让模型「看得见」的 prompt 区块。
///
/// MCP 工具靠完整 schema（名字 + 描述 + 参数）自己就能被模型看见；skill 只有一个
/// 通用入口（`skill` 工具 + `name` 参数），不把这层清单写进 prompt，勾选就等于白勾——
/// 模型没有任何理由主动去调 `skill(name: "list")`。
fn build_enabled_skills_block(enabled_skills: &[String]) -> String {
    if enabled_skills.is_empty() {
        return String::new();
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let known =
        crate::core::skills::discover_skills(&cwd, &[crate::core::skills::skills_base_dir()]);

    let mut lines: Vec<String> = Vec::with_capacity(enabled_skills.len());
    for name in enabled_skills {
        match known.get(name.as_str()) {
            Some(s) => {
                let desc = if s.description.trim().is_empty() {
                    s.category.as_str()
                } else {
                    s.description.as_str()
                };
                lines.push(format!("- {}：{}", s.name, desc.trim()));
            }
            // 名字对不上（skill 被删了 / 目录变了）也要说出来，别让勾选静默失效
            None => {
                tracing::warn!("🎯 本次对话勾选的 Skill「{}」在本地找不到，prompt 里会标出来", name);
                lines.push(format!("- {}（本地未找到该 Skill）", name));
            }
        }
    }

    format!(
        "# 本次对话已启用的 Skill\n\
         用户在输入框的「工具」弹层里勾选了下面这些 Skill。本轮任务如果与它们的用途\
         相关，就调用 `skill` 工具（`name` 填下面的名字）取出完整说明再照做：\n{}\n\n",
        lines.join("\n")
    )
}

fn build_system_prompt_v2(
    memory_ctx: &str,
    kb_context: &str,
    web_context: &str,
    extra: Option<&str>,
    model_display: Option<&str>,
    with_tools: bool,
    enabled_skills: &[String],
) -> String {
    let mut prompt = String::with_capacity(4096);

    // ═══ 1. 稳定前缀（可缓存）═══
    if with_tools {
        prompt.push_str(CORE_PERSONA_AND_TOOL_POLICY);
    } else {
        prompt.push_str("你是 FastClaw 智能助手。回答简洁、直接、说人话。\n");
    }
    prompt.push('\n');

    // ═══ 2. 半稳定：运行环境 / 模型身份 ═══
    prompt.push_str("# 当前运行环境\n");
    prompt.push_str("- 产品：FastClaw 桌面智能助手\n");
    if let Some(m) = model_display {
        if !m.trim().is_empty() {
            prompt.push_str(&format!(
                "- 你当前实际使用的底层模型：**{}**\n\
                 - 用户问你「你是什么模型 / 底层模型是什么」时，直接回答上面这个名字，不要调用任何工具去查。\n",
                m.trim()
            ));
        }
    }
    prompt.push('\n');

    // ═══ 3. 易变内容（放最后，避免污染缓存前缀）═══
    prompt.push_str(&crate::core::time_inject::build_time_constraint_block());

    // ★ 只在本轮真会注入工具时才宣告 skill 清单：图片多模态链路会强制
    //   `with_tools = false`，那种情况下宣告了也调不到，只会让模型去撞墙。
    if with_tools {
        prompt.push_str(&build_enabled_skills_block(enabled_skills));
    }

    if let Some(extra_text) = extra {
        let trimmed = extra_text.trim();
        if !trimmed.is_empty() {
            prompt.push_str("## 当前任务/提示词\n");
            prompt.push_str(trimmed);
            prompt.push_str("\n\n");
        }
    }

    if !memory_ctx.is_empty() {
        prompt.push_str(&format!("## 关于用户的记忆\n{}\n\n", memory_ctx));
    }
    if !kb_context.is_empty() {
        prompt.push_str(&format!(
            "## 相关知识库内容\n{}\n请基于以上内容回答。\n\n",
            kb_context
        ));
    }
    if !web_context.is_empty() {
        prompt.push_str(&format!(
            "## 联网搜索结果\n以下是与用户问题相关的实时网页搜索结果，请优先基于这些信息作答，\
             并在引用具体信息时用 [编号] 标注对应来源（如 [1][2]）：\n\n{}\n",
            web_context
        ));
    }
    prompt
}

// ── 直接工具调用（BERT 意图路由命中）─────────────────────────────────

/// 不经过 LLM，直接调用 MCP 工具并将结果流式返回
#[allow(clippy::too_many_arguments)]
async fn run_direct_tool(
    app: AppHandle,
    pool: sqlx::SqlitePool,
    conversation_id: String,
    content: String,
    group_id: Option<String>,
    skip_user_message: bool,
    server_id: String,
    tool_name: String,
    arguments: Value,
    mcp: crate::core::mcp::SharedMcpManager,
) -> Result<(), AppError> {
    // 1. 保存用户消息
    let user_msg_id = if skip_user_message {
        String::new()
    } else {
        let umid = uuid::Uuid::new_v4().to_string();
        repo::messages::insert(
            &pool,
            &repo::messages::NewMessage {
                id: umid.clone(),
                conversation_id: conversation_id.clone(),
                role: "user".into(),
                content: content.clone(),
                model_id: None,
                model_instance_id: None,
                parent_id: None,
                group_id: group_id.clone(),
                attachments: None,
                token_input: None,
                token_output: None,
            },
        )
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
        umid
    };

    let ai_msg_id = uuid::Uuid::new_v4().to_string();

    // 2. 通知前端：开始工具调用
    let tool_start_msg = format!("🔧 正在调用工具 **{}** ...\n", tool_name);
    let _ = app.emit(
        "chat-stream-token",
        serde_json::json!({
            "conversation_id": conversation_id,
            "message_id": ai_msg_id,
            "token": tool_start_msg,
            "model": "bert-router",
            "done": false,
            "tool_call": {
                "server_id": server_id,
                "tool_name": tool_name,
                "arguments": arguments,
            }
        }),
    );

    // 3. 调用 MCP 工具（网络 I/O 在写锁外，避免阻塞其它会话）
    let tool_result =
        crate::core::mcp::call_tool_shared(&mcp, &server_id, &tool_name, arguments.clone()).await;

    // 4. 格式化工具返回结果
    let result_content = match &tool_result {
        Ok(content) => {
            // MCP 工具返回 JSON Value，尝试提取可读文本
            if let Some(text_array) = content
                .get("content")
                .and_then(|v| v.as_array())
                .or_else(|| content.get("text").and_then(|v| v.as_array()))
            {
                text_array
                    .iter()
                    .filter_map(|t| t.get("text").and_then(|v| v.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            } else if let Some(s) = content.as_str() {
                s.to_string()
            } else {
                serde_json::to_string_pretty(content)
                    .unwrap_or_else(|_| "工具返回格式无法解析".to_string())
            }
        }
        Err(e) => {
            format!("工具调用失败: {}", e)
        }
    };

    // 5. 流式发出结果 token
    // ★ 原来用 `i == result_content.len() - 1` 判断最后一个字符：
    //   i 是 **char 下标**，len() 是 **字节长度**，中文内容下永远不相等，
    //   导致 done 标志从不为 true。改用 char 总数比较。
    let total_chars = result_content.chars().count();
    for (i, ch) in result_content.chars().enumerate() {
        let done = i + 1 == total_chars;
        let token = if ch == '\n' {
            "\n\n".to_string()
        } else {
            ch.to_string()
        };
        let _ = app.emit(
            "chat-stream-token",
            serde_json::json!({
                "conversation_id": conversation_id,
                "message_id": ai_msg_id,
                "token": token,
                "model": format!("mcp::{server_id}::{tool_name}"),
                "done": done,
            }),
        );
        if i % 100 == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    }

    // 6. 保存 AI 回复到 DB
    let _ = repo::messages::insert(
        &pool,
        &repo::messages::NewMessage {
            id: ai_msg_id.clone(),
            conversation_id: conversation_id.clone(),
            role: "assistant".into(),
            content: result_content.clone(),
            model_id: Some(format!("mcp::{server_id}::{tool_name}")),
            model_instance_id: None,
            parent_id: if user_msg_id.is_empty() {
                None
            } else {
                Some(user_msg_id)
            },
            group_id: group_id.clone(),
            attachments: None,
            token_input: None,
            token_output: None,
        },
    )
    .await;

    // 7. 发出完成事件
    let conv_id_for_done = conversation_id.clone();
    let _ = app.emit(
        "chat-stream-done",
        serde_json::json!({
            "conversation_id": conv_id_for_done,
            "message_id": ai_msg_id,
            "content": result_content,
            "model": format!("mcp::{server_id}::{tool_name}"),
            "role": "assistant",
            "tool_called": true,
            "tool_name": tool_name,
            "tool_server": server_id,
        }),
    );

    Ok(())
}

fn truncate_str(s: &str, max_len: usize) -> &str {
    match s.char_indices().nth(max_len) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

#[tauri::command]
pub async fn get_conversations(
    state: State<'_, AppState>,
    user_id: Option<String>,
) -> Result<Vec<ConversationSummary>, AppError> {
    let uid = user_id.unwrap_or_else(|| "default-user".to_string());
    let rows = repo::conversations::list_by_user(&*state.db.pool().await, &uid).await?;
    Ok(rows)
}

#[tauri::command]
pub async fn get_user_message_count(state: State<'_, AppState>) -> Result<i64, AppError> {
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM messages WHERE role = 'user'")
        .fetch_one(&*state.db.pool().await)
        .await?;
    Ok(count)
}

#[tauri::command]
pub async fn get_messages(
    state: State<'_, AppState>,
    conversation_id: String,
    limit: Option<i64>,
) -> Result<Vec<MessageRow>, AppError> {
    let msgs = repo::messages::get_latest_by_conversation(
        &*state.db.pool().await,
        &conversation_id,
        limit.unwrap_or(500),
    )
    .await?;
    Ok(msgs)
}

/// 游标分页：取早于 (before_created_at, before_id) 的更早一页，返回时间正序（旧 → 新）。
/// 前端滚动到顶时逐页 prepend，替代原先 `limit` 硬编码的全量截断。
#[tauri::command]
pub async fn get_messages_page(
    state: State<'_, AppState>,
    conversation_id: String,
    before_created_at: String,
    before_id: String,
    limit: Option<i64>,
) -> Result<Vec<MessageRow>, AppError> {
    let msgs = repo::messages::get_page_before(
        &*state.db.pool().await,
        &conversation_id,
        &before_created_at,
        &before_id,
        limit.unwrap_or(50),
    )
    .await?;
    Ok(msgs)
}

#[tauri::command]
pub async fn create_conversation(
    state: State<'_, AppState>,
    title: Option<String>,
    model_id: Option<String>,
    assistant_id: Option<String>,
    mode: Option<String>,
) -> Result<String, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let title_text = title.unwrap_or_else(|| "新对话".to_string());
    let mode_text = mode.unwrap_or_else(|| "normal".to_string());
    let is_planning = mode_text.trim().eq_ignore_ascii_case("planning");
    repo::conversations::insert(
        &*state.db.pool().await,
        &repo::conversations::NewConversation {
            id: id.clone(),
            user_id: "default-user".to_string(),
            title: title_text.clone(),
            model_id,
            assistant_id,
            mode: mode_text,
        },
    )
    .await?;
    if is_planning {
        let pool = state.db.pool().await;
        let _ = crate::core::session_plan::fastplan::persist_conversation_task_plan(&pool, &id, true).await;
    }
    Ok(id)
}

#[tauri::command]
pub async fn update_conversation(
    state: State<'_, AppState>,
    id: String,
    title: Option<String>,
    is_pinned: Option<bool>,
    is_archived: Option<bool>,
) -> Result<(), AppError> {
    repo::conversations::update(
        &*state.db.pool().await,
        &id,
        title.clone(),
        is_pinned,
        is_archived,
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn update_conversation_extra_config(
    state: State<'_, AppState>,
    id: String,
    extra_config: String,
) -> Result<(), AppError> {
    sqlx::query("UPDATE conversations SET extra_config = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?")
        .bind(&extra_config)
        .bind(&id)
        .execute(&*state.db.pool().await)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn delete_conversation(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    repo::conversations::delete(&*state.db.pool().await, &id).await?;
    Ok(())
}

#[tauri::command]
pub async fn delete_message(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    crate::db::repo::messages::delete(&*state.db.pool().await, &id)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    Ok(())
}

/// Ordinary message deletion; legacy single-message callers retain their original command.
#[tauri::command]
pub async fn delete_message_turn(
    state: State<'_, AppState>,
    conversation_id: String,
    message_id: String,
    expected_content: String,
) -> Result<Vec<String>, AppError> {
    crate::db::repo::message_deletion::remove(
        &*state.db.pool().await, &conversation_id, &message_id, &expected_content,
    ).await.map_err(|error| AppError::DatabaseMsg(error.to_string()))
}

/// 对话分支：从指定消息处创建分支（新对话 + 复制之前的所有消息）
#[tauri::command]
pub async fn branch_conversation(
    state: State<'_, AppState>,
    conversation_id: String,
    branch_point_message_id: String,
    expected_content: Option<String>,
) -> Result<String, AppError> {
    crate::db::repo::conversation_branch::create(
        &*state.db.pool().await,
        &conversation_id,
        &branch_point_message_id,
        expected_content.as_deref(),
    )
    .await
    .map_err(|error| AppError::DatabaseMsg(error.to_string()))
}

// ═══════════════════════════════════════════════════════════════
//  数据类型
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message_id: String,
    pub content: String,
    pub model: String,
    pub token_input: i32,
    pub token_output: i32,
    pub references: Vec<KbReference>,
    pub tool_calls_count: i32,
}

#[derive(Debug, Serialize)]
pub struct KbReference {
    pub chunk_id: String,
    pub document_name: String,
    pub section: Option<String>,
    pub page: Option<i32>,
    pub preview: String,
    pub score: f64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub model_id: Option<String>,
    pub assistant_id: Option<String>,
    pub mode: String,
    pub is_pinned: bool,
    pub is_archived: bool,
    pub updated_at: String,
    pub extra_config: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct MessageRow {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub model_id: Option<String>,
    #[serde(default)]
    pub model_instance_id: Option<String>,
    #[serde(default)]
    pub parent_id: Option<String>,
    pub token_input: Option<i32>,
    pub token_output: Option<i32>,
    pub status: String,
    pub created_at: String,
    pub group_id: Option<String>,
    #[serde(default)]
    pub is_liked: Option<i32>,
    #[serde(default)]
    pub is_favorite: Option<i32>,
    #[serde(default)]
    pub translation: Option<String>,
    #[serde(default)]
    pub translation_lang: Option<String>,
    #[serde(default)]
    pub think_content: Option<String>,
    #[serde(default)]
    pub is_edited: i32,
    #[serde(default)]
    pub edited_at: Option<String>,
    #[serde(default)]
    pub context_cleared: i32,
    /// 图片/截图附件 JSON（`[{id, name, type, preview_url, status}]`）
    #[serde(default)]
    pub attachments: Option<String>,
    #[serde(default)]
    pub extra_data: Option<String>,
    #[serde(default)]
    pub kb_references: Option<String>,
}

#[tauri::command]
pub async fn like_message(
    state: State<'_, AppState>,
    message_id: String,
    is_liked: i32,
) -> Result<(), AppError> {
    let pool = state.db.pool().await;
    sqlx::query("UPDATE messages SET is_liked = ? WHERE id = ?")
        .bind(is_liked)
        .bind(&message_id)
        .execute(&*pool)
        .await?;

    // 沉淀到模型评分：解析 model_instance_id、归属用户、稳定的 model_id + provider_code
    if let Ok(Some((model_instance_id, user_id, stable_model_id, provider_code))) =
        sqlx::query_as::<_, (Option<String>, String, Option<String>, Option<String>)>(
            "SELECT m.model_instance_id, c.user_id, mod.model_id, mod.provider_code \
         FROM messages m \
         JOIN conversations c ON c.id = m.conversation_id \
         LEFT JOIN models mod ON mod.id = m.model_instance_id \
         WHERE m.id = ?",
        )
        .bind(&message_id)
        .fetch_optional(&*pool)
        .await
    {
        if let (Some(mid), Some(smid), Some(pc)) =
            (model_instance_id, stable_model_id, provider_code)
        {
            if is_liked == 1 || is_liked == -1 {
                let _ = crate::db::repo::model_feedback::upsert(
                    &pool,
                    &user_id,
                    &mid,
                    &smid,
                    &pc,
                    &message_id,
                    is_liked,
                )
                .await;
            } else {
                let _ = crate::db::repo::model_feedback::remove(&pool, &user_id, &message_id).await;
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn favorite_message(
    state: State<'_, AppState>,
    message_id: String,
    is_favorite: bool,
) -> Result<(), AppError> {
    let pool = state.db.pool().await;
    let val: i32 = if is_favorite { 1 } else { 0 };
    // 先检查消息是否存在
    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM messages WHERE id = ?")
        .bind(&message_id)
        .fetch_one(&*pool)
        .await?;
    if exists == 0 {
        return Err(AppError::NotFound(format!("消息不存在: {}", message_id)));
    }
    // 始终更新 messages 表的标记位（保证对话内星标回显）
    sqlx::query("UPDATE messages SET is_favorite = ? WHERE id = ?")
        .bind(val)
        .bind(&message_id)
        .execute(&*pool)
        .await?;

    const DEFAULT_USER: &str = "default-user";
    if is_favorite {
        // 收藏时：从 messages + conversations 抓取快照，写入独立表
        match sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
            "SELECT m.content, m.role, m.conversation_id, COALESCE(c.title, '未命名对话'), m.model_id \
             FROM messages m \
             JOIN conversations c ON c.id = m.conversation_id \
             WHERE m.id = ?"
        ).bind(&message_id).fetch_optional(&*pool).await {
            Ok(Some((content, role, conv_id, conv_title, model_id))) => {
                if let Err(e) = crate::db::repo::message_favorites::upsert(
                    &pool, DEFAULT_USER, &message_id, &conv_id, &conv_title, &role, &content, model_id.as_deref()
                ).await {
                    log::error!("message_favorites::upsert failed: {e}");
                }
            }
            Ok(None) => log::warn!("favorite_message: message {message_id} not found"),
            Err(e) => log::error!("favorite_message: query message failed: {e}"),
        }
    } else {
        // 取消收藏时：从独立表删除
        if let Err(e) =
            crate::db::repo::message_favorites::remove(&pool, DEFAULT_USER, &message_id).await
        {
            log::error!("message_favorites::remove failed: {e}");
        }
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FavoriteMessageItem {
    pub id: String,
    pub message_id: Option<String>,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub conversation_id: Option<String>,
    pub conversation_title: String,
    pub conversation_deleted: bool,
    pub model_id: Option<String>,
}

#[tauri::command]
pub async fn get_favorite_messages(
    state: State<'_, AppState>,
) -> Result<Vec<FavoriteMessageItem>, AppError> {
    let pool = state.db.pool().await;
    let items = crate::db::repo::message_favorites::list(&pool, "default-user").await?;
    Ok(items
        .into_iter()
        .map(|f| {
            let deleted = f.conversation_deleted != 0;
            FavoriteMessageItem {
                id: f.id,
                message_id: f.message_id,
                role: f.role,
                content: f.content,
                created_at: f.created_at,
                conversation_id: f.conversation_id,
                conversation_title: f.conversation_title,
                conversation_deleted: deleted,
                model_id: f.model_id,
            }
        })
        .collect())
}

#[tauri::command]
pub async fn clear_conversation_messages(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM messages WHERE conversation_id = ?")
        .bind(&conversation_id)
        .execute(&*state.db.pool().await)
        .await?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct QualityEvaluationRow {
    pub id: String,
    pub message_id: String,
    pub conversation_id: String,
    pub score: f64,
    pub content: String,
    pub user_id: String,
    pub created_at: String,
}

#[tauri::command]
pub async fn submit_quality_evaluation(
    state: State<'_, AppState>,
    message_id: String,
    conversation_id: String,
    score: f64,
    content: String,
    expected_content: Option<String>,
) -> Result<(), AppError> {
    let saved = repo::quality_feedback::submit(
        &*state.db.pool().await,
        &message_id,
        &conversation_id,
        score,
        &content,
        expected_content.as_deref(),
    )
    .await
    .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    if !saved {
        return Err(AppError::InvalidInput(
            "反馈无效或消息已变化，请刷新后重试".into(),
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn get_quality_evaluations(
    state: State<'_, AppState>,
    message_id: String,
) -> Result<Vec<QualityEvaluationRow>, AppError> {
    let rows = sqlx::query_as::<_, QualityEvaluationRow>(
        "SELECT id, message_id, conversation_id, score, content, user_id, created_at FROM quality_evaluations WHERE message_id = ? ORDER BY created_at DESC"
    ).bind(&message_id).fetch_all(&*state.db.pool().await).await?;
    Ok(rows)
}

/// 工具调用记录（内部使用）
#[derive(Debug, Clone)]
struct ToolCallRecord {
    tool_call_id: String,
    tool_name: String,
    arguments: serde_json::Value,
    result: String,
    success: bool,
}

// ═══════════════════════════════════════════════════════════════
//  ChatGLM 迁移新增：编辑 / 翻译 / 上下文管理 / 最近模型
// ═══════════════════════════════════════════════════════════════

#[tauri::command]
pub async fn update_message_content(
    state: State<'_, AppState>,
    id: String,
    content: String,
) -> Result<(), AppError> {
    repo::messages::update_content(&*state.db.pool().await, &id, &content)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    Ok(())
}

/// 把消息正文翻译为目标语言（一次性请求，不走流式），写回 messages 表
#[tauri::command]
pub async fn translate_message(
    state: State<'_, AppState>,
    message_id: String,
    target_lang: String,
    model_id: Option<String>,
    expected_content: Option<String>,
    expected_conversation_id: Option<String>,
) -> Result<String, AppError> {
    let pool = state.db.pool().await.clone();
    let llm = state.llm.clone();

    let msg = repo::messages::get_by_id(&pool, &message_id)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("消息不存在: {}", message_id)))?;

    if msg.content.trim().is_empty() {
        return Err(AppError::InvalidInput("消息为空，无需翻译".into()));
    }
    // Optional for older callers. New UI sends its captured source, not the current Tab.
    if expected_content.as_deref().is_some_and(|content| content != msg.content)
        || expected_conversation_id.as_deref().is_some_and(|id| id != msg.conversation_id)
    {
        return Err(AppError::InvalidInput("消息已变化，请基于最新内容重新翻译".into()));
    }

    // 优先级：default_translation_model 设置 → 消息自身 model_id → 对话模型 → 当前页面选中模型 → 系统默认模型
    let translation_model_setting: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = 'default_translation_model'",
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::DatabaseMsg(e.to_string()))?
    .filter(|v| !v.trim().is_empty());

    let conversation_model_id: Option<String> =
        sqlx::query_scalar::<_, String>("SELECT model_id FROM conversations WHERE id = ?")
            .bind(&msg.conversation_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;

    // model_instance_id = models 主键；model_id = API 名。
    // resolve_model 已同时支持两者；此处仍优先实例 ID，保证多供应商同名模型时选对行。
    let msg_model_id = msg
        .model_instance_id
        .clone()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| msg.model_id.clone().filter(|v| !v.trim().is_empty()));

    let resolved_model = translation_model_setting
        .as_deref()
        .or(msg_model_id.as_deref())
        .or(conversation_model_id.as_deref())
        .or(model_id.as_deref().filter(|v| !v.trim().is_empty()));

    let lang_label = lang_label(&target_lang);
    let system_prompt = format!(
        "你是一名专业翻译。请把用户提供的文本翻译为{lang}，要求自然流畅、忠实原意。\n\
         直接输出译文，不要附加解释、不要使用代码块包裹。",
        lang = lang_label
    );

    let ticket = repo::message_translation::begin(&pool, &message_id, &msg.conversation_id, &msg.content)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?
        .ok_or_else(|| AppError::InvalidInput("消息已变化，请基于最新内容重新翻译".into()))?;

    let result = async {
        let resp = llm
            .chat(resolved_model, &system_prompt, &[], &msg.content)
            .await
            .map_err(|e| AppError::Llm(format!(
                "翻译失败 [模型: {}]: {}", resolved_model.unwrap_or("默认模型"), e
            )))?;
        let translation = resp.content.trim().to_string();
        let saved = repo::message_translation::finish(&pool, &ticket, &translation, &target_lang)
            .await.map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
        if !saved {
            return Err(AppError::InvalidInput("原文已修改或有更新的翻译请求，本次译文未保存，请刷新后重试".into()));
        }
        Ok(translation)
    }.await;
    // Best-effort cleanup also covers LLM/database failures; identity-scoped release
    // cannot erase a newer claim. A process crash leaves only an overwriteable nonce.
    if let Err(error) = repo::message_translation::release(&pool, &ticket).await {
        log::warn!("release translation request failed: {error}");
    }
    result
}

fn lang_label(code: &str) -> &'static str {
    match code {
        "zh" | "zh-CN" | "zh-Hans" => "简体中文",
        "zh-TW" | "zh-Hant" => "繁体中文",
        "en" => "英文",
        "ja" => "日文",
        "ko" => "韩文",
        "fr" => "法文",
        "es" => "西班牙文",
        "de" => "德文",
        "ru" => "俄文",
        "ar" => "阿拉伯文",
        "pt" => "葡萄牙文",
        _ => "目标语言",
    }
}

/// 清除指定对话的"上下文"：保留消息显示，但下次 LLM 调用不再带入。
#[tauri::command]
pub async fn clear_conversation_context(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<(), AppError> {
    repo::messages::mark_conversation_context_cleared(&*state.db.pool().await, &conversation_id)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    // ★ 同步清空渐进式披露的"已加载工具"记录，
    //   否则清了上下文但工具还挂在那里，白白占 token
    crate::core::tools::disclosure::clear_session(&conversation_id);
    Ok(())
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RecentModelRow {
    pub model_id: String,
    pub display_name: String,
    pub last_used_at: String,
}

/// 最近使用过的模型，按最后调用时间倒序
#[tauri::command]
pub async fn get_recent_models(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<RecentModelRow>, AppError> {
    let lim = limit.unwrap_or(8);
    let rows = sqlx::query_as::<_, RecentModelRow>(
        "SELECT m.id AS model_id, m.display_name AS display_name, MAX(t.created_at) AS last_used_at \
         FROM token_usage_logs t \
         JOIN models m ON m.model_id = t.model_id \
         WHERE t.user_id = 'default-user' \
         GROUP BY m.id, m.display_name \
         ORDER BY last_used_at DESC \
         LIMIT ?"
    )
    .bind(lim)
    .fetch_all(&*state.db.pool().await)
    .await
    .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    Ok(rows)
}

/// 使用「话题命名模型」根据用户首条消息生成对话标题。
///
/// 模型选择优先级：
///   1. 设置中的 `topic_naming_model`（设置页「话题命名模型」）
///   2. 该对话当前使用的 `model_id`
///   3. 交给 LLM 层使用默认模型（model_id = None）
///
/// 生成失败时回退为截断后的原始消息，保证主流程不被阻塞。
#[tauri::command]
pub async fn generate_topic_title(
    state: State<'_, AppState>,
    conversation_id: String,
    user_message: String,
) -> Result<String, AppError> {
    let pool = state.db.pool().await.clone();
    let llm = state.llm.clone();

    let title = match crate::core::topic_title::maybe_name_conversation_topic(
        &pool,
        llm.as_ref(),
        &conversation_id,
        &user_message,
        None,
        false, // 桌面：优先话题命名模型设置
    )
    .await?
    {
        Some(t) => t,
        // 已是正式短标题：原样返回，便于前端幂等
        None => {
            let existing: String = sqlx::query_scalar(
                "SELECT COALESCE(title, '') FROM conversations WHERE id = ?",
            )
            .bind(&conversation_id)
            .fetch_one(&pool)
            .await
            .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
            existing
        }
    };

    Ok(title)
}

/// 在系统默认浏览器中打开外部链接（用于联网搜索来源点击跳转等）。
///
/// 说明：Tauri Webview 里的 `window.open` 无法可靠唤起系统浏览器，故走系统命令。
/// - Windows 用 `rundll32 url.dll,FileProtocolHandler`：能正确处理 URL 中的 `&`，且不弹黑窗；
/// - macOS 用 `open`，Linux 用 `xdg-open`。
/// 仅放行 http/https，避免触发本地协议或命令注入。
#[tauri::command]
pub fn open_external(url: String) -> Result<(), AppError> {
    let u = url.trim();
    if !(u.starts_with("http://") || u.starts_with("https://") || u.starts_with("mailto:")) {
        return Err(AppError::InvalidInput(
            "仅支持 http/https/mailto 链接".into(),
        ));
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", u])
            .spawn()
            .map_err(|e| AppError::Business(format!("打开链接失败: {}", e)))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(u)
            .spawn()
            .map_err(|e| AppError::Business(format!("打开链接失败: {}", e)))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(u)
            .spawn()
            .map_err(|e| AppError::Business(format!("打开链接失败: {}", e)))?;
    }

    Ok(())
}
