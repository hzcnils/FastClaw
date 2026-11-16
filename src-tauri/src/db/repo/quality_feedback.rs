use sqlx::SqlitePool;

/// Validate the target in the same statement that inserts feedback; no precheck/write race.
pub async fn submit(
    pool: &SqlitePool,
    message_id: &str,
    conversation_id: &str,
    score: f64,
    content: &str,
    expected_content: Option<&str>,
) -> anyhow::Result<bool> {
    let content = content.trim();
    if !score.is_finite() || !(0.0..=10.0).contains(&score) || (score == 0.0 && content.is_empty())
    {
        return Ok(false);
    }
    let result = sqlx::query(
        "INSERT INTO quality_evaluations (id, message_id, conversation_id, score, content, user_id) \
         SELECT ?, id, conversation_id, ?, ?, 'default-user' FROM messages \
         WHERE id = ? AND conversation_id = ? AND (? IS NULL OR content = ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(score)
    .bind(content)
    .bind(message_id)
    .bind(conversation_id)
    .bind(expected_content)
    .bind(expected_content)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}
