// FastClaw · 模型评分反馈仓库层
use serde::Serialize;
use sqlx::SqlitePool;

/// upsert 一条反馈（同一 user+message 覆盖）
pub async fn upsert(
    pool: &SqlitePool,
    user_id: &str,
    model_instance_id: &str,
    model_id: &str,
    provider_code: &str,
    message_id: &str,
    signal: i32,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO model_feedback (id, user_id, model_instance_id, model_id, provider_code, message_id, signal, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now')) \
         ON CONFLICT(user_id, message_id) DO UPDATE SET \
           signal = excluded.signal, model_instance_id = excluded.model_instance_id, \
           model_id = excluded.model_id, provider_code = excluded.provider_code, updated_at = datetime('now')"
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(user_id).bind(model_instance_id).bind(model_id).bind(provider_code).bind(message_id).bind(signal)
    .execute(pool).await?;
    Ok(())
}

/// 取消评价时删除该条反馈
pub async fn remove(pool: &SqlitePool, user_id: &str, message_id: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM model_feedback WHERE user_id = ? AND message_id = ?")
        .bind(user_id)
        .bind(message_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct RatingRow {
    model_id: String,
    provider_code: String,
    likes: i64,
    dislikes: i64,
}

#[derive(Serialize)]
pub struct ModelRating {
    pub model_id: String,
    pub provider_code: String,
    pub likes: i64,
    pub dislikes: i64,
    pub score: f64,
    pub tier: String,
}

/// 聚合每个模型的评分（按 model_id + provider_code，区分服务商）。
/// scope = "global" 全局；"me" 限定 user_id。
pub async fn get_ratings(
    pool: &SqlitePool,
    scope: &str,
    user_id: Option<&str>,
) -> anyhow::Result<Vec<ModelRating>> {
    let rows: Vec<RatingRow> = if scope == "me" {
        sqlx::query_as::<_, RatingRow>(
            "SELECT model_id, provider_code, \
                SUM(CASE WHEN signal = 1 THEN 1 ELSE 0 END)  AS likes, \
                SUM(CASE WHEN signal = -1 THEN 1 ELSE 0 END) AS dislikes \
             FROM model_feedback WHERE user_id = ? AND model_id != '' AND provider_code != '' \
             GROUP BY model_id, provider_code",
        )
        .bind(user_id.unwrap_or("default-user"))
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, RatingRow>(
            "SELECT model_id, provider_code, \
                SUM(CASE WHEN signal = 1 THEN 1 ELSE 0 END)  AS likes, \
                SUM(CASE WHEN signal = -1 THEN 1 ELSE 0 END) AS dislikes \
             FROM model_feedback WHERE model_id != '' AND provider_code != '' \
             GROUP BY model_id, provider_code",
        )
        .fetch_all(pool)
        .await?
    };

    const PRIOR_MEAN: f64 = 0.6;
    const PRIOR_WEIGHT: f64 = 5.0;
    const MIN_SAMPLE: i64 = 3;

    Ok(rows
        .into_iter()
        .map(|r| {
            let n = r.likes + r.dislikes;
            let score = (r.likes as f64 + PRIOR_WEIGHT * PRIOR_MEAN) / (n as f64 + PRIOR_WEIGHT);
            let tier = if n < MIN_SAMPLE {
                "new"
            } else if score >= 0.85 {
                "excellent"
            } else if score >= 0.65 {
                "good"
            } else if score >= 0.40 {
                "neutral"
            } else {
                "bad"
            };
            ModelRating {
                model_id: r.model_id,
                provider_code: r.provider_code,
                likes: r.likes,
                dislikes: r.dislikes,
                score,
                tier: tier.into(),
            }
        })
        .collect())
}
