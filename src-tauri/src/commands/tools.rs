// FastClaw · 本地工具库 Tauri Commands
// 替代 FastPlan /api/admin/tools（HTTP 401）

use crate::core::error::AppError;
use crate::db::repo;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize)]
pub struct ToolItem {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub tool_type: String,
    pub input_schema: serde_json::Value,
    pub config: serde_json::Value,
    pub is_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_server: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct ToolListResponse {
    pub items: Vec<ToolItem>,
}

#[derive(Debug, Deserialize)]
pub struct CreateToolPayload {
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: String,
    pub tool_type: String,
    #[serde(default)]
    pub input_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub config: Option<serde_json::Value>,
    #[serde(default)]
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateToolPayload {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub config: Option<serde_json::Value>,
    #[serde(default)]
    pub is_active: Option<bool>,
}

fn parse_json_field(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or_else(|_| serde_json::json!({}))
}

fn row_to_item(row: repo::tools::ToolRow) -> ToolItem {
    ToolItem {
        id: row.id,
        name: row.name,
        display_name: row.display_name,
        description: row.description,
        tool_type: row.tool_type,
        input_schema: parse_json_field(&row.input_schema),
        config: parse_json_field(&row.config),
        is_active: row.is_active != 0,
        source_server: None,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn is_mcp_tool_id(id: &str) -> bool {
    id.starts_with("mcp:")
}

/// 列出本地工具 + 已连接 MCP 发现的工具
#[tauri::command]
pub async fn list_local_tools(
    state: State<'_, AppState>,
) -> Result<ToolListResponse, AppError> {
    let mut items: Vec<ToolItem> = repo::tools::list(&*state.db.pool().await)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?
        .into_iter()
        .map(row_to_item)
        .collect();

    // 合并运行中 MCP Server 的工具（只读展示，不可改删）
    {
        let manager = state.mcp.read().await;
        for (server_id, tool) in manager.list_all_tools() {
            let source_server = manager
                .get_server_info(&server_id)
                .map(|info| info.config.name.clone())
                .unwrap_or_else(|| server_id.clone());
            items.push(ToolItem {
                id: format!("mcp:{}:{}", server_id, tool.name),
                name: tool.name.clone(),
                display_name: tool.name.clone(),
                description: tool.description,
                tool_type: "MCP".into(),
                input_schema: tool.input_schema,
                config: serde_json::json!({ "server_id": server_id }),
                is_active: true,
                source_server: Some(source_server),
                created_at: String::new(),
                updated_at: String::new(),
            });
        }
    }

    Ok(ToolListResponse { items })
}

#[tauri::command]
pub async fn get_local_tool(
    state: State<'_, AppState>,
    id: String,
) -> Result<ToolItem, AppError> {
    if is_mcp_tool_id(&id) {
        return Err(AppError::InvalidInput(
            "MCP 工具为只读，请在 MCP 管理页操作".into(),
        ));
    }
    let row = repo::tools::get(&*state.db.pool().await, &id)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("工具不存在: {}", id)))?;
    Ok(row_to_item(row))
}

#[tauri::command]
pub async fn create_local_tool(
    state: State<'_, AppState>,
    payload: CreateToolPayload,
) -> Result<ToolItem, AppError> {
    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::InvalidInput("工具名称不能为空".into()));
    }
    let tool_type = payload.tool_type.trim().to_uppercase();
    if tool_type != "HTTP" && tool_type != "SKILL" {
        return Err(AppError::InvalidInput(
            "tool_type 仅支持 HTTP 或 SKILL".into(),
        ));
    }
    if repo::tools::get_by_name(&*state.db.pool().await, &name)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?
        .is_some()
    {
        return Err(AppError::InvalidInput(format!("工具名称已存在: {}", name)));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let display_name = payload
        .display_name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| name.clone());
    let input_schema = payload
        .input_schema
        .unwrap_or_else(|| serde_json::json!({ "type": "object", "properties": {} }));
    let config = payload.config.unwrap_or_else(|| serde_json::json!({}));
    let is_active = if payload.is_active.unwrap_or(true) {
        1
    } else {
        0
    };

    let new_tool = repo::tools::NewTool {
        id: id.clone(),
        name,
        display_name,
        description: payload.description,
        tool_type,
        input_schema: serde_json::to_string(&input_schema).unwrap_or_else(|_| "{}".into()),
        config: serde_json::to_string(&config).unwrap_or_else(|_| "{}".into()),
        is_active,
    };

    repo::tools::insert(&*state.db.pool().await, &new_tool)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;

    let row = repo::tools::get(&*state.db.pool().await, &id)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?
        .ok_or_else(|| AppError::DatabaseMsg("创建后读取失败".into()))?;
    Ok(row_to_item(row))
}

#[tauri::command]
pub async fn update_local_tool(
    state: State<'_, AppState>,
    id: String,
    payload: UpdateToolPayload,
) -> Result<ToolItem, AppError> {
    if is_mcp_tool_id(&id) {
        return Err(AppError::InvalidInput(
            "MCP 工具为只读，请在 MCP 管理页操作".into(),
        ));
    }

    let input_schema = payload
        .input_schema
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "{}".into()));
    let config = payload
        .config
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "{}".into()));
    let is_active = payload.is_active.map(|b| if b { 1 } else { 0 });

    let ok = repo::tools::update(
        &*state.db.pool().await,
        &id,
        payload.display_name.as_deref(),
        payload.description.as_deref(),
        input_schema.as_deref(),
        config.as_deref(),
        is_active,
    )
    .await
    .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;

    if !ok {
        return Err(AppError::NotFound(format!("工具不存在: {}", id)));
    }

    let row = repo::tools::get(&*state.db.pool().await, &id)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("工具不存在: {}", id)))?;
    Ok(row_to_item(row))
}

#[tauri::command]
pub async fn delete_local_tool(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), AppError> {
    if is_mcp_tool_id(&id) {
        return Err(AppError::InvalidInput(
            "MCP 工具不可从此处删除，请在 MCP 管理页操作".into(),
        ));
    }
    let ok = repo::tools::delete(&*state.db.pool().await, &id)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    if !ok {
        return Err(AppError::NotFound(format!("工具不存在: {}", id)));
    }
    Ok(())
}

#[tauri::command]
pub async fn toggle_local_tool(
    state: State<'_, AppState>,
    id: String,
    is_active: bool,
) -> Result<ToolItem, AppError> {
    if is_mcp_tool_id(&id) {
        return Err(AppError::InvalidInput(
            "MCP 工具状态由 Server 连接决定".into(),
        ));
    }
    let ok = repo::tools::set_active(&*state.db.pool().await, &id, is_active)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;
    if !ok {
        return Err(AppError::NotFound(format!("工具不存在: {}", id)));
    }
    let row = repo::tools::get(&*state.db.pool().await, &id)
        .await
        .map_err(|e| AppError::DatabaseMsg(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("工具不存在: {}", id)))?;
    Ok(row_to_item(row))
}
