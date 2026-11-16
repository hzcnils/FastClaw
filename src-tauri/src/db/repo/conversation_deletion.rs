use sqlx::SqlitePool;

/// Delete a conversation and all its messages in one transaction.
/// Execution cancellation/authorization belongs to the caller; both desktop and remote
/// callers retain their existing policies. Missing targets remain idempotent successes.
pub async fn remove(pool: &SqlitePool, conversation_id: &str) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    // messages has no conversation foreign key: deleting only the parent is insufficient.
    sqlx::query("DELETE FROM messages WHERE conversation_id = ?")
        .bind(conversation_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM conversations WHERE id = ?")
        .bind(conversation_id)
        .execute(&mut *tx)
        .await?;

    // A trigger can ignore a DELETE without reporting an SQL error. Check the final
    // state, not a fixed row count: an empty/already-deleted conversation is valid.
    let remaining: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM messages WHERE conversation_id = ?) \
         OR EXISTS(SELECT 1 FROM conversations WHERE id = ?)",
    )
    .bind(conversation_id)
    .bind(conversation_id)
    .fetch_one(&mut *tx)
    .await?;
    anyhow::ensure!(!remaining, "会话删除不完整，已回滚，请重新加载历史核对");
    tx.commit().await?;
    Ok(())
}
