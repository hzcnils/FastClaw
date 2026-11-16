use sqlx::SqlitePool;

/// Database-scoped claim, shared by Tabs/processes using the same database.
pub struct TranslationTicket {
    message_id: String,
    conversation_id: String,
    revision: i64,
    request_id: String,
}

/// Registration is atomic. A newer registered request supersedes an older one,
/// even if the older LLM call finishes later. No transaction spans the LLM call.
pub async fn begin(
    pool: &SqlitePool,
    message_id: &str,
    conversation_id: &str,
    content: &str,
) -> anyhow::Result<Option<TranslationTicket>> {
    let request_id = uuid::Uuid::new_v4().to_string();
    let revision = sqlx::query_scalar::<_, i64>(
        "UPDATE messages SET translation_request_id = ? \
         WHERE id = ? AND conversation_id = ? AND content = ? \
         RETURNING content_revision",
    )
    .bind(&request_id)
    .bind(message_id)
    .bind(conversation_id)
    .bind(content)
    .fetch_optional(pool)
    .await?;
    Ok(revision.map(|revision| TranslationTicket {
        message_id: message_id.to_owned(),
        conversation_id: conversation_id.to_owned(),
        revision,
        request_id,
    }))
}

/// A single conditional UPDATE validates the claim and persists the result.
pub async fn finish(
    pool: &SqlitePool,
    ticket: &TranslationTicket,
    translation: &str,
    language: &str,
) -> anyhow::Result<bool> {
    let result = sqlx::query(
        "UPDATE messages SET translation = ?, translation_lang = ?, translation_request_id = NULL \
         WHERE id = ? AND conversation_id = ? AND content_revision = ? AND translation_request_id = ?",
    )
    .bind(translation)
    .bind(language)
    .bind(&ticket.message_id)
    .bind(&ticket.conversation_id)
    .bind(ticket.revision)
    .bind(&ticket.request_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Late failures must not clear a newer request or revive a superseded request.
pub async fn release(pool: &SqlitePool, ticket: &TranslationTicket) -> anyhow::Result<()> {
    sqlx::query("UPDATE messages SET translation_request_id = NULL WHERE id = ? AND translation_request_id = ?")
        .bind(&ticket.message_id)
        .bind(&ticket.request_id)
        .execute(pool)
        .await?;
    Ok(())
}
