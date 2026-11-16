use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use super::ProviderRow;
use crate::core::models::{ModelDto, ModelExportDto, ModelUpdatePatch, PatchField};

pub async fn list_models(
    pool: &SqlitePool,
    provider_code: Option<&str>,
) -> anyhow::Result<Vec<ModelDto>> {
    let base_sql = "SELECT id, model_id, display_name, model_type, is_enabled, model_label, mxcs, context_window, COALESCE(provider_code, '') AS provider_code, COALESCE(supports_vision, 0) AS supports_vision FROM models";
    if let Some(provider_code) = provider_code {
        Ok(sqlx::query_as::<_, ModelDto>(&format!(
            "{base_sql} WHERE provider_code = ? ORDER BY sort_order"
        ))
        .bind(provider_code)
        .fetch_all(pool)
        .await?)
    } else {
        Ok(
            sqlx::query_as::<_, ModelDto>(&format!("{base_sql} ORDER BY sort_order"))
                .fetch_all(pool)
                .await?,
        )
    }
}

pub async fn list_models_for_export(pool: &SqlitePool) -> anyhow::Result<Vec<ModelExportDto>> {
    Ok(sqlx::query_as::<_, ModelExportDto>(
        r#"SELECT m.id, m.model_id, m.display_name, m.model_type, m.is_enabled, m.model_label,
               m.mxcs, m.context_window, m.temperature, m.top_p, m.extra_config, m.provider_code,
               COALESCE(p.base_url, '') as base_url, p.api_key_ref
           FROM models m
           LEFT JOIN model_providers p ON m.provider_code = p.provider_code
           ORDER BY m.sort_order"#,
    )
    .fetch_all(pool)
    .await?)
}

pub struct NewModel {
    pub id: String,
    pub provider_code: String,
    pub model_id: String,
    pub display_name: String,
    pub model_type: String,
    pub model_label: String,
    pub context_window: Option<i32>,
    pub max_output: Option<i32>,
    pub input_price: Option<f64>,
    pub output_price: Option<f64>,
    pub is_enabled: bool,
    pub is_local_model: bool,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub extra_config: Option<String>,
    pub mxcs: Option<String>,
}

// 本地模型市场模型结构体

pub async fn insert_model(pool: &SqlitePool, model: &NewModel) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO models (id, provider_code, model_id, display_name, model_type, model_label, context_window, max_output, input_price, output_price, is_enabled, is_local_model, temperature, top_p, extra_config, mxcs) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
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
        .bind(model.is_enabled as i32)
        .bind(model.is_local_model as i32)
        .bind(model.temperature)
        .bind(model.top_p)
        .bind(&model.extra_config)
        .bind(&model.mxcs)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn find_provider_by_name(
    pool: &SqlitePool,
    name: &str,
) -> anyhow::Result<Option<ProviderRow>> {
    let row = sqlx::query_as::<_, ProviderRow>(
        "SELECT id, name, provider_type, base_url, api_key_ref, is_local FROM model_providers WHERE name = ?"
    ).bind(name).fetch_optional(pool).await?;
    Ok(row)
}

pub async fn upsert_model(pool: &SqlitePool, model: &NewModel) -> anyhow::Result<bool> {
    let existed = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM models WHERE model_id = ? AND provider_code = ?)",
    )
    .bind(&model.model_id)
    .bind(&model.provider_code)
    .fetch_one(pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO models (
               id, provider_code, model_id, display_name, model_type, model_label,
               context_window, max_output, input_price, output_price, is_enabled,
               is_local_model, temperature, top_p, extra_config, mxcs
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(provider_code, model_id) WHERE provider_code IS NOT NULL DO UPDATE SET
               display_name=excluded.display_name, model_type=excluded.model_type,
               model_label=excluded.model_label, context_window=excluded.context_window,
               max_output=excluded.max_output, input_price=excluded.input_price,
               output_price=excluded.output_price, is_enabled=excluded.is_enabled,
               is_local_model=excluded.is_local_model, temperature=excluded.temperature,
               top_p=excluded.top_p, extra_config=excluded.extra_config, mxcs=excluded.mxcs,
               updated_at=datetime('now')"#,
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
    .execute(pool)
    .await?;
    Ok(!existed)
}

pub async fn update_models(
    pool: &SqlitePool,
    ids: &[String],
    updates: &ModelUpdatePatch,
) -> anyhow::Result<()> {
    if ids.is_empty() {
        return Ok(());
    }

    let has_updates = !updates.display_name.is_missing()
        || updates.model_type.is_some()
        || updates.context_window.is_some()
        || updates.max_output.is_some()
        || updates.input_price.is_some()
        || updates.output_price.is_some()
        || !updates.model_label.is_missing()
        || updates.is_enabled.is_some()
        || updates.temperature.is_some()
        || updates.top_p.is_some()
        || !updates.mxcs.is_missing();
    if !has_updates {
        return Ok(());
    }

    let mut query = QueryBuilder::<Sqlite>::new("UPDATE models SET ");
    let mut fields = query.separated(", ");
    if let PatchField::Value(value) = &updates.display_name {
        fields
            .push("display_name = ")
            .push_bind_unseparated(value.clone().unwrap_or_default());
    }
    if let Some(value) = &updates.model_type {
        fields.push("model_type = ").push_bind_unseparated(value);
    }
    if let Some(value) = updates.context_window {
        fields
            .push("context_window = ")
            .push_bind_unseparated(value);
    }
    if let Some(value) = updates.max_output {
        fields.push("max_output = ").push_bind_unseparated(value);
    }
    if let Some(value) = updates.input_price {
        fields.push("input_price = ").push_bind_unseparated(value);
    }
    if let Some(value) = updates.output_price {
        fields.push("output_price = ").push_bind_unseparated(value);
    }
    if let PatchField::Value(value) = &updates.model_label {
        fields
            .push("model_label = ")
            .push_bind_unseparated(value.clone().unwrap_or_default());
    }
    if let Some(value) = updates.is_enabled {
        fields.push("is_enabled = ").push_bind_unseparated(value);
    }
    if let Some(value) = updates.temperature {
        fields.push("temperature = ").push_bind_unseparated(value);
    }
    if let Some(value) = updates.top_p {
        fields.push("top_p = ").push_bind_unseparated(value);
    }
    if let PatchField::Value(value) = &updates.mxcs {
        fields.push("mxcs = ").push_bind_unseparated(value);
    }
    drop(fields);
    query.push(" WHERE id IN (");
    let mut id_values = query.separated(", ");
    for id in ids {
        id_values.push_bind(id);
    }
    drop(id_values);
    query.push(")");
    query.build().execute(pool).await?;
    Ok(())
}

pub async fn delete_models(pool: &SqlitePool, ids: &[String]) -> anyhow::Result<()> {
    if ids.is_empty() {
        return Ok(());
    }

    let placeholders: Vec<&str> = ids.iter().map(|_| "?").collect();
    let sql = format!(
        "DELETE FROM models WHERE id IN ({})",
        placeholders.join(", ")
    );

    let mut q = sqlx::query(&sql);
    for id in ids {
        q = q.bind(id);
    }
    q.execute(pool).await?;
    Ok(())
}

// ─── 模型网关 CRUD ───
