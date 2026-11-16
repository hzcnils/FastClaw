// FastClaw · 对话仓库层

use crate::commands::chat::ConversationSummary;
use sqlx::SqlitePool;

pub struct NewConversation {
    pub id: String,
    pub user_id: String,
    pub title: String,
    pub model_id: Option<String>,
    pub assistant_id: Option<String>,
    pub mode: String,
}

pub async fn insert(pool: &SqlitePool, c: &NewConversation) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO conversations (id, user_id, title, model_id, assistant_id, mode, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))"
    ).bind(&c.id).bind(&c.user_id).bind(&c.title).bind(&c.model_id).bind(&c.assistant_id).bind(&c.mode)
    .execute(pool).await?;
    Ok(())
}

pub async fn list_by_user(
    pool: &SqlitePool,
    user_id: &str,
) -> anyhow::Result<Vec<ConversationSummary>> {
    let rows = sqlx::query_as::<_, ConversationSummary>(
        "SELECT id, title, model_id, assistant_id, mode, is_pinned, is_archived, updated_at, extra_config FROM conversations WHERE user_id = ? AND is_archived = 0 ORDER BY is_pinned DESC, updated_at DESC"
    ).bind(user_id).fetch_all(pool).await?;
    Ok(rows)
}

/// 远程控制列出本机未归档对话（不按 user_id 过滤，与桌面侧栏同一张表）。
pub async fn list_active(
    pool: &SqlitePool,
    limit: i64,
) -> anyhow::Result<Vec<ConversationSummary>> {
    let rows = sqlx::query_as::<_, ConversationSummary>(
        "SELECT id, title, model_id, assistant_id, mode, is_pinned, is_archived, updated_at, extra_config \
         FROM conversations WHERE is_archived = 0 \
         ORDER BY is_pinned DESC, updated_at DESC LIMIT ?"
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_by_id(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<ConversationSummary>> {
    let row = sqlx::query_as::<_, ConversationSummary>(
        "SELECT id, title, model_id, assistant_id, mode, is_pinned, is_archived, updated_at, extra_config \
         FROM conversations WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    title: Option<String>,
    is_pinned: Option<bool>,
    is_archived: Option<bool>,
) -> anyhow::Result<()> {
    if let Some(t) = title {
        sqlx::query("UPDATE conversations SET title = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?").bind(&t).bind(id).execute(pool).await?;
    }
    if let Some(p) = is_pinned {
        sqlx::query("UPDATE conversations SET is_pinned = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?").bind(p).bind(id).execute(pool).await?;
    }
    if let Some(a) = is_archived {
        sqlx::query("UPDATE conversations SET is_archived = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?").bind(a).bind(id).execute(pool).await?;
    }
    Ok(())
}

pub async fn update_mode(pool: &SqlitePool, id: &str, mode: &str) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE conversations SET mode = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
    )
    .bind(mode)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn message_count(pool: &SqlitePool, id: &str) -> anyhow::Result<i64> {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE conversation_id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(n)
}

pub async fn update_model_id(pool: &SqlitePool, id: &str, model_id: &str) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE conversations SET model_id = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
    )
    .bind(model_id)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: &str) -> anyhow::Result<()> {
    super::conversation_deletion::remove(pool, id).await
}

pub async fn update_extra_config(
    pool: &SqlitePool,
    id: &str,
    extra_config: &str,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE conversations SET extra_config = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?")
        .bind(extra_config)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
