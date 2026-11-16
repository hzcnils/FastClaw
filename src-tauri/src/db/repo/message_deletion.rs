use sqlx::SqlitePool;

/// Delete one message and its unambiguous, now-unanswered group question atomically.
pub async fn remove(
    pool: &SqlitePool,
    conversation_id: &str,
    message_id: &str,
    expected_content: &str,
) -> anyhow::Result<Vec<String>> {
    let mut tx = pool.begin().await?;
    let target: Option<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT role, content, group_id FROM messages WHERE id = ? AND conversation_id = ?",
    )
    .bind(message_id)
    .bind(conversation_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (role, content, group) =
        target.ok_or_else(|| anyhow::anyhow!("消息已不存在或不属于该会话"))?;
    if content != expected_content {
        anyhow::bail!("消息内容已变更，请重新确认删除");
    }
    let streaming: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM messages WHERE conversation_id = ? AND status = 'streaming')",
    )
    .bind(conversation_id)
    .fetch_one(&mut *tx)
    .await?;
    if streaming {
        anyhow::bail!("会话仍有消息生成中，请结束后再删除");
    }
    let mut ids = vec![message_id.to_owned()];
    if role == "assistant" {
        if let Some(group) = group.filter(|group| !group.is_empty()) {
            let siblings: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM messages WHERE conversation_id = ? AND group_id = ? AND role = 'assistant' AND id != ?)",
            ).bind(conversation_id).bind(&group).bind(message_id).fetch_one(&mut *tx).await?;
            if !siblings {
                let users: Vec<String> = sqlx::query_scalar(
                    "SELECT id FROM messages WHERE conversation_id = ? AND group_id = ? AND role = 'user' LIMIT 2",
                ).bind(conversation_id).bind(&group).fetch_all(&mut *tx).await?;
                // Never fall back to a neighbouring question or guess between ambiguous legacy rows.
                if users.len() == 1 {
                    ids.extend(users);
                }
            }
        }
    }
    for id in &ids {
        let deleted = sqlx::query("DELETE FROM messages WHERE id = ? AND conversation_id = ?")
            .bind(id)
            .bind(conversation_id)
            .execute(&mut *tx)
            .await?;
        if deleted.rows_affected() != 1 {
            anyhow::bail!("消息删除不完整");
        }
    }
    tx.commit().await?;
    Ok(ids)
}
