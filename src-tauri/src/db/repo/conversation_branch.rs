use sqlx::{Row, SqlitePool};
use std::collections::HashMap;

/// One database snapshot and transaction: an invalid target or failed copy leaves no partial branch.
pub async fn create(
    pool: &SqlitePool,
    conversation_id: &str,
    message_id: &str,
    expected_content: Option<&str>,
) -> anyhow::Result<String> {
    let mut tx = pool.begin().await?;
    let conversation: Option<(String, String)> =
        sqlx::query_as("SELECT title, user_id FROM conversations WHERE id = ?")
            .bind(conversation_id)
            .fetch_optional(&mut *tx)
            .await?;
    let (title, user_id) = conversation.ok_or_else(|| anyhow::anyhow!("源会话不存在"))?;
    let point: Option<(String, String, String)> = sqlx::query_as(
        "SELECT created_at, content, status FROM messages WHERE id = ? AND conversation_id = ?",
    )
    .bind(message_id)
    .bind(conversation_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (created_at, content, _) = point.ok_or_else(|| anyhow::anyhow!("分支点消息不存在"))?;
    if expected_content.is_some_and(|expected| expected != content) {
        anyhow::bail!("分支点内容已变更，请重新操作");
    }
    // Read only identities here; large bodies are copied within SQLite, with no 500-message cutoff.
    let rows = sqlx::query(
        "SELECT id, parent_id, group_id, status FROM messages WHERE conversation_id = ? \
         AND (created_at < ? OR (created_at = ? AND id <= ?)) ORDER BY created_at ASC, id ASC",
    )
    .bind(conversation_id)
    .bind(&created_at)
    .bind(&created_at)
    .bind(message_id)
    .fetch_all(&mut *tx)
    .await?;
    if rows
        .iter()
        .any(|row| row.get::<String, _>("status") == "streaming")
    {
        anyhow::bail!("分支范围内仍有消息生成中，请完成后重试");
    }
    let new_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO conversations (id, user_id, title, mode, created_at, updated_at) VALUES (?, ?, ?, 'normal', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))")
        .bind(&new_id)
        .bind(user_id)
        .bind(format!("{} - 分支", title))
        .execute(&mut *tx)
        .await?;
    // Sorted new UUIDs retain source order when several messages share the same timestamp.
    let mut new_ids: Vec<String> = rows
        .iter()
        .map(|_| uuid::Uuid::new_v4().to_string())
        .collect();
    new_ids.sort();
    let id_map: HashMap<String, String> = rows
        .iter()
        .zip(new_ids)
        .map(|(row, id)| (row.get("id"), id))
        .collect();
    let mut groups = HashMap::new();
    for row in rows {
        let old_id: String = row.try_get("id")?;
        let parent: Option<String> = row.try_get("parent_id")?;
        let group: Option<String> = row.try_get("group_id")?;
        let new_group = group.map(|group| {
            groups
                .entry(group)
                .or_insert_with(|| uuid::Uuid::new_v4().to_string())
                .clone()
        });
        let copied = sqlx::query(
            "INSERT INTO messages (
                id, conversation_id, role, content, model_id, model_instance_id,
                parent_id, group_id, attachments, token_input, token_output,
                status, created_at, think_content, extra_data, kb_references,
                is_liked, is_favorite, translation, translation_lang,
                is_edited, edited_at, context_cleared, duration_ms, error_message
            ) SELECT ?, ?, role, content, model_id, model_instance_id,
                ?, ?, attachments, token_input, token_output,
                status, created_at, think_content, extra_data, kb_references,
                is_liked, is_favorite, translation, translation_lang,
                is_edited, edited_at, context_cleared, duration_ms, error_message
              FROM messages WHERE id = ? AND conversation_id = ?",
        )
        .bind(&id_map[&old_id])
        .bind(&new_id)
        .bind(parent.as_ref().and_then(|id| id_map.get(id)))
        .bind(new_group)
        .bind(&old_id)
        .bind(conversation_id)
        .execute(&mut *tx)
        .await?;
        if copied.rows_affected() != 1 {
            anyhow::bail!("分支消息复制不完整");
        }
    }
    tx.commit().await?;
    Ok(new_id)
}
