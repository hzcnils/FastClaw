// FastClaw · 费用追踪仓库层
use sqlx::SqlitePool;

pub struct TokenUsage {
    pub id: String,
    pub user_id: String,
    pub conversation_id: Option<String>,
    pub message_id: Option<String>,
    pub provider_id: String,
    pub provider_code: String,
    pub model_id: String,
    pub token_input: i32,
    pub token_output: i32,
    pub cost_usd: f64,
    pub call_type: String,
}

pub async fn insert_usage(pool: &SqlitePool, u: &TokenUsage) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO token_usage_logs (id, user_id, conversation_id, message_id, provider_id, provider_code, model_id, token_input, token_output, cost_usd, call_type) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&u.id).bind(&u.user_id).bind(&u.conversation_id).bind(&u.message_id).bind(&u.provider_id).bind(&u.provider_code).bind(&u.model_id).bind(u.token_input).bind(u.token_output).bind(u.cost_usd).bind(&u.call_type)
        .execute(pool).await?;
    Ok(())
}

pub async fn get_stats(
    pool: &SqlitePool,
    user_id: &str,
    period: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let date_filter = match period.unwrap_or("today") {
        "today" => "date(created_at) = date('now')",
        "month" => "strftime('%Y-%m', created_at) = strftime('%Y-%m', 'now')",
        _ => "1=1",
    };
    let sql = format!("SELECT COALESCE(SUM(token_input),0), COALESCE(SUM(token_output),0), COALESCE(SUM(cost_usd),0), COUNT(*) FROM token_usage_logs WHERE user_id = ? AND {}", date_filter);
    let row = sqlx::query_as::<_, (i64, i64, f64, i64)>(&sql)
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    Ok(
        serde_json::json!({"token_input": row.0, "token_output": row.1, "cost_usd": row.2, "call_count": row.3}),
    )
}

pub async fn list_budgets(
    pool: &SqlitePool,
    user_id: &str,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let rows = sqlx::query_as::<_, (String, String, f64, String, bool)>(
        "SELECT id, budget_type, limit_usd, action_on_limit, is_enabled FROM cost_budgets WHERE user_id = ?"
    ).bind(user_id).fetch_all(pool).await?;
    Ok(rows.iter().map(|(id, bt, l, a, e)| serde_json::json!({"id": id, "budget_type": bt, "limit_usd": l, "action": a, "is_enabled": e})).collect())
}

pub async fn upsert_budget(
    pool: &SqlitePool,
    user_id: &str,
    budget_type: &str,
    limit_usd: f64,
    action: &str,
) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO cost_budgets (id, user_id, budget_type, limit_usd, action_on_limit) VALUES (?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET limit_usd = excluded.limit_usd, action_on_limit = excluded.action_on_limit, updated_at = datetime('now')")
        .bind(uuid::Uuid::new_v4().to_string()).bind(user_id).bind(budget_type).bind(limit_usd).bind(action)
        .execute(pool).await?;
    Ok(())
}
