// FastClaw · 本地工具库（library_tools）

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ToolRow {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub tool_type: String,
    pub input_schema: String,
    pub config: String,
    pub is_active: i32,
    pub created_at: String,
    pub updated_at: String,
}

pub struct NewTool {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub tool_type: String,
    pub input_schema: String,
    pub config: String,
    pub is_active: i32,
}

const COLS: &str = "id, name, display_name, description, tool_type, input_schema, config, is_active, created_at, updated_at";

pub async fn list(pool: &SqlitePool) -> anyhow::Result<Vec<ToolRow>> {
    let sql = format!(
        "SELECT {} FROM library_tools ORDER BY updated_at DESC",
        COLS
    );
    let rows = sqlx::query_as::<_, ToolRow>(&sql).fetch_all(pool).await?;
    Ok(rows)
}

pub async fn get(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<ToolRow>> {
    let sql = format!("SELECT {} FROM library_tools WHERE id = ?", COLS);
    let row = sqlx::query_as::<_, ToolRow>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn get_by_name(pool: &SqlitePool, name: &str) -> anyhow::Result<Option<ToolRow>> {
    let sql = format!("SELECT {} FROM library_tools WHERE name = ?", COLS);
    let row = sqlx::query_as::<_, ToolRow>(&sql)
        .bind(name)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn insert(pool: &SqlitePool, t: &NewTool) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO library_tools \
         (id, name, display_name, description, tool_type, input_schema, config, is_active) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&t.id)
    .bind(&t.name)
    .bind(&t.display_name)
    .bind(&t.description)
    .bind(&t.tool_type)
    .bind(&t.input_schema)
    .bind(&t.config)
    .bind(t.is_active)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    display_name: Option<&str>,
    description: Option<&str>,
    input_schema: Option<&str>,
    config: Option<&str>,
    is_active: Option<i32>,
) -> anyhow::Result<bool> {
    let existing = match get(pool, id).await? {
        Some(r) => r,
        None => return Ok(false),
    };

    let display_name = display_name.unwrap_or(&existing.display_name);
    let description = description.unwrap_or(&existing.description);
    let input_schema = input_schema.unwrap_or(&existing.input_schema);
    let config = config.unwrap_or(&existing.config);
    let is_active = is_active.unwrap_or(existing.is_active);

    let result = sqlx::query(
        "UPDATE library_tools SET display_name = ?, description = ?, input_schema = ?, \
         config = ?, is_active = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(display_name)
    .bind(description)
    .bind(input_schema)
    .bind(config)
    .bind(is_active)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn set_active(pool: &SqlitePool, id: &str, is_active: bool) -> anyhow::Result<bool> {
    update(pool, id, None, None, None, None, Some(if is_active { 1 } else { 0 })).await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> anyhow::Result<bool> {
    let result = sqlx::query("DELETE FROM library_tools WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
