// FastClaw · 智能体管理表 agent_management CRUD
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

const ALL_COLUMNS: &str = "id, agent_name, agent_description, agent_icon, agent_prompt, agent_type, agent_model, agent_knowledgebase, agent_status, agent_visibility, agent_visibility_roles, agent_tags, agent_creator, agent_cover, agent_function_desc, agent_input_params, agent_output_results, agent_preferences, json_data, created_time, update_time";

fn row_to_json(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String, _>("id"),
        "agent_name": row.get::<String, _>("agent_name"),
        "agent_description": row.get::<Option<String>, _>("agent_description"),
        "agent_icon": row.get::<Option<String>, _>("agent_icon"),
        "agent_prompt": row.get::<Option<String>, _>("agent_prompt"),
        "agent_type": row.get::<Option<String>, _>("agent_type"),
        "agent_model": row.get::<Option<String>, _>("agent_model"),
        "agent_knowledgebase": row.get::<Option<String>, _>("agent_knowledgebase"),
        "agent_status": row.get::<String, _>("agent_status"),
        "agent_visibility": row.get::<String, _>("agent_visibility"),
        "agent_visibility_roles": row.get::<Option<String>, _>("agent_visibility_roles"),
        "agent_tags": row.get::<Option<String>, _>("agent_tags"),
        "agent_creator": row.get::<Option<String>, _>("agent_creator"),
        "agent_cover": row.get::<Option<String>, _>("agent_cover"),
        "agent_function_desc": row.get::<Option<String>, _>("agent_function_desc"),
        "agent_input_params": row.get::<Option<String>, _>("agent_input_params"),
        "agent_output_results": row.get::<Option<String>, _>("agent_output_results"),
        "agent_preferences": row.get::<Option<String>, _>("agent_preferences"),
        "json_data": row.get::<Option<String>, _>("json_data"),
        "created_time": row.get::<String, _>("created_time"),
        "update_time": row.get::<String, _>("update_time"),
    })
}

pub struct AgentFilters {
    pub search: Option<String>,
    pub agent_type: Option<String>,
    pub agent_status: Option<String>,
    pub agent_visibility: Option<String>,
}

fn build_where_clause(filters: &AgentFilters) -> (String, Vec<String>) {
    let mut conditions = Vec::new();
    let mut bindings = Vec::new();

    if let Some(ref search) = filters.search {
        if !search.is_empty() {
            conditions.push("agent_name LIKE ?".to_string());
            bindings.push(format!("%{}%", search));
        }
    }
    if let Some(ref agent_type) = filters.agent_type {
        if !agent_type.is_empty() {
            conditions.push("agent_type = ?".to_string());
            bindings.push(agent_type.clone());
        }
    }
    if let Some(ref agent_status) = filters.agent_status {
        if !agent_status.is_empty() {
            conditions.push("agent_status = ?".to_string());
            bindings.push(agent_status.clone());
        }
    }
    if let Some(ref agent_visibility) = filters.agent_visibility {
        if !agent_visibility.is_empty() {
            conditions.push("agent_visibility = ?".to_string());
            bindings.push(agent_visibility.clone());
        }
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    (where_clause, bindings)
}

pub async fn get_agents(
    pool: &SqlitePool,
    page: Option<u32>,
    page_size: Option<u32>,
    filters: Option<&AgentFilters>,
) -> anyhow::Result<Vec<Value>> {
    let default_filters = AgentFilters {
        search: None,
        agent_type: None,
        agent_status: None,
        agent_visibility: None,
    };
    let f = filters.unwrap_or(&default_filters);
    let (where_clause, bindings) = build_where_clause(f);

    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(20).max(1).min(100);
    let offset = (page - 1) * page_size;

    let sql = format!(
        "SELECT {} FROM agent_management{} ORDER BY created_time DESC LIMIT ? OFFSET ?",
        ALL_COLUMNS, where_clause
    );

    let mut query = sqlx::query(&sql);
    for b in &bindings {
        query = query.bind(b);
    }
    query = query.bind(page_size as i64).bind(offset as i64);

    let rows = query.fetch_all(pool).await?;

    let mut results = Vec::new();
    for row in &rows {
        results.push(row_to_json(row));
    }
    Ok(results)
}

pub async fn get_agent_count(
    pool: &SqlitePool,
    filters: Option<&AgentFilters>,
) -> anyhow::Result<i64> {
    let default_filters = AgentFilters {
        search: None,
        agent_type: None,
        agent_status: None,
        agent_visibility: None,
    };
    let f = filters.unwrap_or(&default_filters);
    let (where_clause, bindings) = build_where_clause(f);

    let sql = format!(
        "SELECT COUNT(*) as cnt FROM agent_management{}",
        where_clause
    );

    let mut query = sqlx::query_scalar::<_, i64>(&sql);
    for b in &bindings {
        query = query.bind(b);
    }

    let count = query.fetch_one(pool).await?;
    Ok(count)
}

pub async fn get_agent_by_id(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Value>> {
    let sql = format!("SELECT {} FROM agent_management WHERE id = ?", ALL_COLUMNS);
    let row = sqlx::query(&sql).bind(id).fetch_optional(pool).await?;

    if let Some(ref row) = row {
        Ok(Some(row_to_json(row)))
    } else {
        Ok(None)
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_agent(
    pool: &SqlitePool,
    id: &str,
    agent_name: &str,
    agent_description: Option<&str>,
    agent_icon: Option<&str>,
    agent_prompt: Option<&str>,
    agent_type: Option<&str>,
    agent_model: Option<&str>,
    agent_knowledgebase: Option<&str>,
    agent_status: Option<&str>,
    agent_visibility: Option<&str>,
    agent_visibility_roles: Option<&str>,
    agent_tags: Option<&str>,
    agent_creator: Option<&str>,
    agent_cover: Option<&str>,
    agent_function_desc: Option<&str>,
    agent_input_params: Option<&str>,
    agent_output_results: Option<&str>,
    agent_preferences: Option<&str>,
    json_data: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO agent_management (id, agent_name, agent_description, agent_icon, agent_prompt, agent_type, agent_model, agent_knowledgebase, agent_status, agent_visibility, agent_visibility_roles, agent_tags, agent_creator, agent_cover, agent_function_desc, agent_input_params, agent_output_results, agent_preferences, json_data, created_time, update_time) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))"
    )
    .bind(id)
    .bind(agent_name)
    .bind(agent_description)
    .bind(agent_icon)
    .bind(agent_prompt)
    .bind(agent_type)
    .bind(agent_model)
    .bind(agent_knowledgebase)
    .bind(agent_status.unwrap_or("enabled"))
    .bind(agent_visibility.unwrap_or("all"))
    .bind(agent_visibility_roles)
    .bind(agent_tags)
    .bind(agent_creator)
    .bind(agent_cover)
    .bind(agent_function_desc)
    .bind(agent_input_params)
    .bind(agent_output_results)
    .bind(agent_preferences)
    .bind(json_data)
    .execute(pool)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
pub async fn update_agent(
    pool: &SqlitePool,
    id: &str,
    agent_name: &str,
    agent_description: Option<&str>,
    agent_icon: Option<&str>,
    agent_prompt: Option<&str>,
    agent_type: Option<&str>,
    agent_model: Option<&str>,
    agent_knowledgebase: Option<&str>,
    agent_status: Option<&str>,
    agent_visibility: Option<&str>,
    agent_visibility_roles: Option<&str>,
    agent_tags: Option<&str>,
    agent_creator: Option<&str>,
    agent_cover: Option<&str>,
    agent_function_desc: Option<&str>,
    agent_input_params: Option<&str>,
    agent_output_results: Option<&str>,
    agent_preferences: Option<&str>,
    json_data: Option<&str>,
) -> anyhow::Result<()> {
    let base_sql = "UPDATE agent_management SET agent_name = ?, agent_description = COALESCE(?, agent_description), agent_icon = COALESCE(?, agent_icon), agent_prompt = COALESCE(?, agent_prompt), agent_type = COALESCE(?, agent_type), agent_model = COALESCE(?, agent_model), agent_knowledgebase = COALESCE(?, agent_knowledgebase), agent_status = COALESCE(?, agent_status), agent_visibility = COALESCE(?, agent_visibility), agent_visibility_roles = COALESCE(?, agent_visibility_roles), agent_tags = COALESCE(?, agent_tags), agent_creator = COALESCE(?, agent_creator), agent_cover = COALESCE(?, agent_cover), agent_function_desc = COALESCE(?, agent_function_desc), agent_input_params = COALESCE(?, agent_input_params), agent_output_results = COALESCE(?, agent_output_results), agent_preferences = COALESCE(?, agent_preferences)";
    let update_time = ", update_time = datetime('now')";
    let where_clause = " WHERE id = ?";

    let sql = if json_data.is_some() {
        format!("{}{}, json_data = ?{}", base_sql, update_time, where_clause)
    } else {
        format!("{}{}{}", base_sql, update_time, where_clause)
    };

    let mut query = sqlx::query(&sql)
        .bind(agent_name)
        .bind(agent_description)
        .bind(agent_icon)
        .bind(agent_prompt)
        .bind(agent_type)
        .bind(agent_model)
        .bind(agent_knowledgebase)
        .bind(agent_status)
        .bind(agent_visibility)
        .bind(agent_visibility_roles)
        .bind(agent_tags)
        .bind(agent_creator)
        .bind(agent_cover)
        .bind(agent_function_desc)
        .bind(agent_input_params)
        .bind(agent_output_results)
        .bind(agent_preferences);

    if let Some(data) = json_data {
        query = query.bind(data);
    }

    query = query.bind(id);
    query.execute(pool).await?;
    Ok(())
}

/// 根据会话的 assistant_id 读取该智能体的偏好参数（JSON 字符串）
/// 返回 None 表示无关联智能体或未配置偏好参数
pub async fn get_preferences_by_conversation(
    pool: &SqlitePool,
    conversation_id: &str,
) -> anyhow::Result<Option<String>> {
    let prefs = sqlx::query_scalar::<_, Option<String>>(
        "SELECT a.agent_preferences FROM conversations c \
         JOIN agent_management a ON a.id = c.assistant_id \
         WHERE c.id = ?",
    )
    .bind(conversation_id)
    .fetch_optional(pool)
    .await?
    .flatten();
    Ok(prefs)
}

pub async fn delete_agent(pool: &SqlitePool, id: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM agent_management WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn check_agent_name_exists(
    pool: &SqlitePool,
    name: &str,
    exclude_id: Option<&str>,
) -> anyhow::Result<bool> {
    if let Some(exclude_id) = exclude_id {
        let row = sqlx::query(
            "SELECT COUNT(*) as cnt FROM agent_management WHERE agent_name = ? AND id != ?",
        )
        .bind(name)
        .bind(exclude_id)
        .fetch_one(pool)
        .await?;
        Ok(row.get::<i64, _>("cnt") > 0)
    } else {
        let row = sqlx::query("SELECT COUNT(*) as cnt FROM agent_management WHERE agent_name = ?")
            .bind(name)
            .fetch_one(pool)
            .await?;
        Ok(row.get::<i64, _>("cnt") > 0)
    }
}
