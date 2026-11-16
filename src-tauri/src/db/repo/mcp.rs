// FastClaw · MCP Server 仓库层
// 使用 settings 表持久化（category = 'mcp_servers'），免额外 migration
use sqlx::SqlitePool;

const CATEGORY: &str = "mcp_servers";

pub async fn list(
    pool: &SqlitePool,
    _filter: Option<&str>,
) -> anyhow::Result<Vec<(String, serde_json::Value)>> {
    let rows =
        sqlx::query_as::<_, (String, String)>("SELECT key, value FROM settings WHERE category = ?")
            .bind(CATEGORY)
            .fetch_all(pool)
            .await?;

    Ok(rows
        .into_iter()
        .filter_map(|(id, val)| {
            serde_json::from_str::<serde_json::Value>(&val)
                .ok()
                .map(|v| (id, v))
        })
        .collect())
}

pub async fn insert(pool: &SqlitePool, id: &str, data: serde_json::Value) -> anyhow::Result<()> {
    let value = serde_json::to_string(&data)?;
    sqlx::query("INSERT OR REPLACE INTO settings (key, value, category) VALUES (?, ?, ?)")
        .bind(id)
        .bind(&value)
        .bind(CATEGORY)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update(pool: &SqlitePool, id: &str, data: serde_json::Value) -> anyhow::Result<()> {
    insert(pool, id, data).await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM settings WHERE key = ? AND category = ?")
        .bind(id)
        .bind(CATEGORY)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn toggle(pool: &SqlitePool, id: &str, enabled: bool) -> anyhow::Result<()> {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT value FROM settings WHERE key = ? AND category = ? LIMIT 1",
    )
    .bind(id)
    .bind(CATEGORY)
    .fetch_optional(pool)
    .await?;

    if let Some((val,)) = row {
        if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&val) {
            json["enabled"] = serde_json::Value::Bool(enabled);
            return insert(pool, id, json).await;
        }
    }
    Ok(())
}
