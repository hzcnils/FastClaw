// FastClaw · MCP 服务管理 Tauri Commands
//
// 提供 MCP Server 的完整生命周期管理：
// - 列表/添加/删除 MCP Server（数据库持久化）
// - 连接/断开 MCP Server
// - 工具发现（list_tools）
// - 工具调用（call_tool）

use crate::core::error::AppError;
use crate::db::repo;
use crate::state::AppState;
use tauri::State;

/// 列出所有 MCP Server（运行时 Manager 状态）
#[tauri::command]
pub async fn list_mcp_servers(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let manager = state.mcp.read().await;
    Ok(manager.list_server_summaries())
}

/// 添加 MCP Server（DB 持久化 + 运行时注册）
#[tauri::command]
pub async fn add_mcp_server(
    state: State<'_, AppState>,
    input: serde_json::Value,
) -> Result<String, AppError> {
    let id = uuid::Uuid::new_v4().to_string();

    // 1. 持久化到 DB
    repo::mcp::insert(&*state.db.pool().await, &id, input.clone())
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;

    // 2. 解析配置并注册到运行时
    if let Ok(config) = parse_mcp_config(&input) {
        let mut manager = state.mcp.write().await;
        manager.register_server(id.clone(), config);
    }

    Ok(id)
}

pub fn parse_mcp_config(
    input: &serde_json::Value,
) -> Result<crate::core::mcp::McpServerConfig, AppError> {
    let name = input["name"].as_str().unwrap_or("unnamed").to_string();
    let description = input["description"].as_str().unwrap_or("").to_string();
    let enabled = input["enabled"].as_bool().unwrap_or(true);
    let auto_start = input["auto_start"].as_bool().unwrap_or(false);
    let keywords: Vec<String> = input["keywords"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let environment: std::collections::HashMap<String, String> = input["environment"]
        .as_object()
        .map(|o| {
            o.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                .collect()
        })
        .unwrap_or_default();

    let transport_type = input["transport_type"].as_str().unwrap_or("stdio");
    let transport = match transport_type {
        "sse" | "http" => {
            let url = input["transport_config"]["url"]
                .as_str()
                .or_else(|| input["url"].as_str())
                .unwrap_or("")
                .to_string();
            crate::core::mcp::TransportType::Sse { url }
        }
        _ => {
            let command = input["transport_config"]["command"]
                .as_str()
                .or_else(|| input["command"].as_str())
                .unwrap_or("")
                .to_string();
            let args: Vec<String> = input["transport_config"]["args"]
                .as_array()
                .or_else(|| input["args"].as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            crate::core::mcp::TransportType::Stdio { command, args }
        }
    };

    let auto_index = input["auto_index"].as_bool().unwrap_or(true);

    Ok(crate::core::mcp::McpServerConfig {
        name,
        transport,
        enabled,
        environment,
        description,
        auto_start,
        auto_index,
        keywords,
    })
}

/// 删除 MCP Server
#[tauri::command]
pub async fn remove_mcp_server(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    // 先断开连接（I/O 在写锁外）
    crate::core::mcp::disconnect_server_shared(&state.mcp, &id).await;
    // 从运行时注销，否则 list_mcp_servers 仍会返回该卡片
    {
        let mut manager = state.mcp.write().await;
        manager.unregister_server(&id);
    }
    // 再从数据库删除
    repo::mcp::delete(&*state.db.pool().await, &id)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    // 刷新意图路由工具嵌入
    if let Some(ref router) = state.intent_router {
        router.refresh_tool_embeddings(&state.mcp);
    }
    Ok(())
}

/// 连接 MCP Server
#[tauri::command]
pub async fn connect_mcp_server(
    state: State<'_, AppState>,
    server_id: String,
) -> Result<(), AppError> {
    crate::core::mcp::connect_server_shared(&state.mcp, &server_id).await?;
    // 刷新 BERT 意图路由工具嵌入
    if let Some(ref router) = state.intent_router {
        router.refresh_tool_embeddings(&state.mcp);
    }
    Ok(())
}

/// 断开 MCP Server
#[tauri::command]
pub async fn disconnect_mcp_server(
    state: State<'_, AppState>,
    server_id: String,
) -> Result<(), AppError> {
    crate::core::mcp::disconnect_server_shared(&state.mcp, &server_id).await;
    // 刷新 BERT 意图路由工具嵌入
    if let Some(ref router) = state.intent_router {
        router.refresh_tool_embeddings(&state.mcp);
    }
    Ok(())
}

/// 列出指定 Server 的所有工具
#[tauri::command]
pub async fn list_mcp_tools(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let manager = state.mcp.read().await;
    let tools: Vec<serde_json::Value> = manager
        .list_all_tools()
        .into_iter()
        .map(|(server_id, tool)| {
            serde_json::json!({
                "server_id": server_id,
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.input_schema,
            })
        })
        .collect();
    Ok(tools)
}

/// 调用 MCP 工具
#[tauri::command]
pub async fn call_mcp_tool(
    state: State<'_, AppState>,
    server_id: String,
    tool_name: String,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    crate::core::mcp::call_tool_shared(&state.mcp, &server_id, &tool_name, arguments).await
}

/// 更新 MCP Server 配置
#[tauri::command]
pub async fn update_mcp_server(
    state: State<'_, AppState>,
    server_id: String,
    input: serde_json::Value,
) -> Result<(), AppError> {
    // Persist to DB
    repo::mcp::update(&*state.db.pool().await, &server_id, input.clone())
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;

    // Parse new config and update in-memory state
    let config = parse_mcp_config(&input)?;

    let was_active = {
        let manager = state.mcp.read().await;
        manager
            .get_server_status(&server_id)
            .map(|s| {
                matches!(
                    s,
                    crate::core::mcp::ServerStatus::Running
                        | crate::core::mcp::ServerStatus::Starting
                )
            })
            .unwrap_or(false)
    };

    crate::core::mcp::disconnect_server_shared(&state.mcp, &server_id).await;
    {
        let mut manager = state.mcp.write().await;
        manager.register_server(server_id.clone(), config);
    }

    if was_active {
        if let Err(e) = crate::core::mcp::connect_server_shared(&state.mcp, &server_id).await {
            tracing::warn!(
                "⚠️ MCP Server 配置已保存，但重连失败: {} — {}",
                server_id,
                e
            );
        }
    }

    // Refresh BERT tool embeddings
    if let Some(ref router) = state.intent_router {
        router.refresh_tool_embeddings(&state.mcp);
    }

    tracing::info!("✅ MCP Server 已更新: {server_id}");
    Ok(())
}

/// 刷新指定 MCP Server 的工具缓存（断开重连以重新发现工具）
#[tauri::command]
pub async fn refresh_mcp_tools(
    state: State<'_, AppState>,
    server_id: String,
) -> Result<Vec<serde_json::Value>, AppError> {
    crate::core::mcp::disconnect_server_shared(&state.mcp, &server_id).await;
    crate::core::mcp::connect_server_shared(&state.mcp, &server_id).await?;

    // Retrieve the refreshed tools
    let manager = state.mcp.read().await;
    let tools: Vec<serde_json::Value> = manager
        .list_all_tools()
        .into_iter()
        .filter(|(sid, _)| sid == &server_id)
        .map(|(server_id, tool)| {
            serde_json::json!({
                "server_id": server_id,
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.input_schema,
            })
        })
        .collect();

    // Refresh BERT tool embeddings
    if let Some(ref router) = state.intent_router {
        router.refresh_tool_embeddings(&state.mcp);
    }

    tracing::info!(
        "🔄 MCP tools refreshed for: {server_id} ({} tools)",
        tools.len()
    );
    Ok(tools)
}

/// 测试调用 MCP Server 的某个工具
#[tauri::command]
pub async fn test_mcp_tool(
    state: State<'_, AppState>,
    server_id: String,
    tool_name: String,
    args: serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    crate::core::mcp::call_tool_shared(&state.mcp, &server_id, &tool_name, args).await
}
