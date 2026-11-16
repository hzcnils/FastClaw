use std::collections::HashSet;

use sqlx::{Row, SqliteConnection};

use super::{NewModel, NewProvider};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProviderMatchRow {
    pub provider_code: String,
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ImportModelWrite {
    pub id: String,
    pub provider_code: Option<String>,
    pub model_id: String,
    pub display_name: String,
    pub model_type: String,
    pub model_label: String,
    pub max_output: Option<i32>,
    pub is_local_model: bool,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub extra_config: Option<String>,
    pub mxcs: Option<String>,
}

pub async fn list_provider_matches(
    connection: &mut SqliteConnection,
) -> anyhow::Result<Vec<ProviderMatchRow>> {
    Ok(sqlx::query_as::<_, ProviderMatchRow>(
        "SELECT provider_code, id, name FROM model_providers WHERE provider_code IS NOT NULL",
    )
    .fetch_all(&mut *connection)
    .await?)
}

pub async fn insert_provider(
    connection: &mut SqliteConnection,
    provider: &NewProvider,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO model_providers (id, name, provider_type, base_url, api_key_ref, is_local, provider_code, source_category, gateway_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&provider.id)
    .bind(&provider.name)
    .bind(&provider.provider_type)
    .bind(&provider.base_url)
    .bind(&provider.api_key_ref)
    .bind(provider.is_local)
    .bind(&provider.provider_code)
    .bind(&provider.source_category)
    .bind(&provider.gateway_id)
    .execute(&mut *connection)
    .await?;
    Ok(())
}

pub async fn upsert_model(
    connection: &mut SqliteConnection,
    model: &NewModel,
) -> anyhow::Result<bool> {
    let existed = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM models WHERE provider_code = ? AND model_id = ?)",
    )
    .bind(&model.provider_code)
    .bind(&model.model_id)
    .fetch_one(&mut *connection)
    .await?;

    sqlx::query(
        r#"INSERT INTO models (
               id, provider_code, model_id, display_name, model_type, model_label,
               context_window, max_output, input_price, output_price, is_enabled,
               is_local_model, temperature, top_p, extra_config, mxcs
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(provider_code, model_id) WHERE provider_code IS NOT NULL DO UPDATE SET
               display_name = excluded.display_name,
               model_type = excluded.model_type,
               model_label = excluded.model_label,
               context_window = excluded.context_window,
               max_output = excluded.max_output,
               input_price = excluded.input_price,
               output_price = excluded.output_price,
               is_enabled = excluded.is_enabled,
               is_local_model = excluded.is_local_model,
               temperature = excluded.temperature,
               top_p = excluded.top_p,
               extra_config = excluded.extra_config,
               mxcs = excluded.mxcs,
               updated_at = datetime('now')"#,
    )
    .bind(&model.id)
    .bind(&model.provider_code)
    .bind(&model.model_id)
    .bind(&model.display_name)
    .bind(&model.model_type)
    .bind(&model.model_label)
    .bind(model.context_window)
    .bind(model.max_output)
    .bind(model.input_price)
    .bind(model.output_price)
    .bind(model.is_enabled)
    .bind(model.is_local_model)
    .bind(model.temperature)
    .bind(model.top_p)
    .bind(&model.extra_config)
    .bind(&model.mxcs)
    .execute(&mut *connection)
    .await?;

    Ok(!existed)
}

pub async fn upsert_import_model(
    connection: &mut SqliteConnection,
    model: &ImportModelWrite,
) -> anyhow::Result<bool> {
    let by_id = sqlx::query_scalar::<_, String>("SELECT id FROM models WHERE id = ?")
        .bind(&model.id)
        .fetch_optional(&mut *connection)
        .await?;
    let existing_id = if by_id.is_some() || model.provider_code.is_none() {
        by_id
    } else {
        sqlx::query_scalar::<_, String>(
            "SELECT id FROM models WHERE provider_code = ? AND model_id = ?",
        )
        .bind(&model.provider_code)
        .bind(&model.model_id)
        .fetch_optional(&mut *connection)
        .await?
    };

    if let Some(existing_id) = existing_id {
        sqlx::query(
            "UPDATE models SET display_name=?, model_type=?, model_label=?, max_output=?, temperature=?, top_p=?, extra_config=?, mxcs=?, updated_at=datetime('now') WHERE id=?",
        )
        .bind(&model.display_name)
        .bind(&model.model_type)
        .bind(&model.model_label)
        .bind(model.max_output)
        .bind(model.temperature)
        .bind(model.top_p)
        .bind(&model.extra_config)
        .bind(&model.mxcs)
        .bind(existing_id)
        .execute(&mut *connection)
        .await?;
        return Ok(false);
    }

    sqlx::query(
        "INSERT INTO models (id, provider_code, model_id, display_name, model_type, model_label, max_output, is_enabled, is_local_model, temperature, top_p, extra_config, mxcs) VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?)",
    )
    .bind(&model.id)
    .bind(&model.provider_code)
    .bind(&model.model_id)
    .bind(&model.display_name)
    .bind(&model.model_type)
    .bind(&model.model_label)
    .bind(model.max_output)
    .bind(model.is_local_model)
    .bind(model.temperature)
    .bind(model.top_p)
    .bind(&model.extra_config)
    .bind(&model.mxcs)
    .execute(&mut *connection)
    .await?;
    Ok(true)
}

pub async fn read_sync_history(
    connection: &mut SqliteConnection,
) -> anyhow::Result<Option<String>> {
    Ok(
        sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'sync_history'")
            .fetch_optional(&mut *connection)
            .await?,
    )
}

pub async fn write_sync_history(
    connection: &mut SqliteConnection,
    history: &str,
) -> anyhow::Result<()> {
    sqlx::query("INSERT OR REPLACE INTO settings (key, value, category, updated_at) VALUES ('sync_history', ?, 'sync', datetime('now'))")
        .bind(history)
        .execute(&mut *connection)
        .await?;
    Ok(())
}

pub async fn list_enabled_independent_models(
    connection: &mut SqliteConnection,
) -> anyhow::Result<HashSet<(String, String)>> {
    let rows = sqlx::query(
        "SELECT m.model_id, m.provider_code FROM models m INNER JOIN model_providers p ON m.provider_code = p.provider_code WHERE p.source_category = 'independent' AND m.is_enabled = 1",
    )
    .fetch_all(&mut *connection)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.get("model_id"), row.get("provider_code")))
        .collect())
}

pub async fn find_independent_provider(
    connection: &mut SqliteConnection,
    base_url: &str,
) -> anyhow::Result<Option<(String, Option<String>)>> {
    Ok(sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT provider_code, api_key_ref FROM model_providers WHERE base_url = ? AND source_category = 'independent' LIMIT 1",
    )
    .bind(base_url)
    .fetch_optional(&mut *connection)
    .await?)
}

pub async fn provider_code_exists(
    connection: &mut SqliteConnection,
    provider_code: &str,
) -> anyhow::Result<bool> {
    Ok(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM model_providers WHERE provider_code = ?)",
    )
    .bind(provider_code)
    .fetch_one(&mut *connection)
    .await?)
}

pub async fn disable_independent_models(
    connection: &mut SqliteConnection,
    keys: &HashSet<(String, String)>,
) -> anyhow::Result<usize> {
    for (model_id, provider_code) in keys {
        sqlx::query("UPDATE models SET is_enabled = 0, updated_at = datetime('now') WHERE model_id = ? AND provider_code = ?")
            .bind(model_id)
            .bind(provider_code)
            .execute(&mut *connection)
            .await?;
    }
    Ok(keys.len())
}
