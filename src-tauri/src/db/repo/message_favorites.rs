// FastClaw · 消息收藏独立仓库层
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(sqlx::FromRow, Serialize)]
pub struct FavoriteItem {
    pub id: String,
    pub message_id: Option<String>,
    pub conversation_id: Option<String>,
    pub conversation_title: String,
    pub role: String,
    pub content: String,
    pub model_id: Option<String>,
    pub created_at: String,
    pub conversation_deleted: i64, // 1=原对话已删除
}

/// 收藏时 upsert：如果该消息已收藏则覆盖内容快照，否则插入新行
pub async fn upsert(
    pool: &SqlitePool,
    user_id: &str,
    message_id: &str,
    conversation_id: &str,
    conversation_title: &str,
    role: &str,
    content: &str,
    model_id: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO message_favorites (id, user_id, message_id, conversation_id, conversation_title, role, content, model_id, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now')) \
         ON CONFLICT(user_id, message_id) DO UPDATE SET \
           conversation_title = excluded.conversation_title, \
           content = excluded.content, \
           updated_at = datetime('now')"
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(user_id)
    .bind(message_id)
    .bind(conversation_id)
    .bind(conversation_title)
    .bind(role)
    .bind(content)
    .bind(model_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 取消收藏时删除
pub async fn remove(pool: &SqlitePool, user_id: &str, message_id: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM message_favorites WHERE user_id = ? AND message_id = ?")
        .bind(user_id)
        .bind(message_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 列出当前用户所有收藏（LEFT JOIN 判原对话是否存在）
pub async fn list(pool: &SqlitePool, user_id: &str) -> anyhow::Result<Vec<FavoriteItem>> {
    let rows = sqlx::query_as::<_, FavoriteItem>(
        "SELECT mf.id, mf.message_id, mf.conversation_id, mf.conversation_title, \
                mf.role, mf.content, mf.model_id, mf.created_at, \
                CASE WHEN c.id IS NULL THEN 1 ELSE 0 END AS conversation_deleted \
         FROM message_favorites mf \
         LEFT JOIN conversations c ON c.id = mf.conversation_id \
         WHERE mf.user_id = ? ORDER BY mf.created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
