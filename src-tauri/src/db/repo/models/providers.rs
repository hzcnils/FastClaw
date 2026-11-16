use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::core::models::{
    PatchField, ProviderDto, ProviderRuntimeConfigDto, ProviderSummaryDto, ProviderUpdatePatch,
};

pub struct NewProvider {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key_ref: Option<String>,
    pub is_local: bool,
    pub provider_code: Option<String>,
    pub source_category: Option<String>,
    pub gateway_id: Option<String>,
}

#[derive(sqlx::FromRow)]
pub struct ProviderRow {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key_ref: Option<String>,
    pub is_local: bool,
}

pub async fn list_providers(pool: &SqlitePool) -> anyhow::Result<Vec<ProviderDto>> {
    Ok(sqlx::query_as::<_, ProviderDto>(
        "SELECT id, name, provider_type, base_url, api_key_ref AS api_key, is_local, is_enabled, provider_code, COALESCE(source_category, 'online') AS source_category, extra_config, gateway_id FROM model_providers ORDER BY sort_order"
    ).fetch_all(pool).await?)
}

pub async fn list_provider_summaries(pool: &SqlitePool) -> anyhow::Result<Vec<ProviderSummaryDto>> {
    Ok(sqlx::query_as::<_, ProviderSummaryDto>(
        "SELECT id, name, provider_type, base_url, (COALESCE(api_key_ref, '') <> '') AS has_api_key, is_local, is_enabled, provider_code, COALESCE(source_category, 'online') AS source_category FROM model_providers ORDER BY sort_order"
    ).fetch_all(pool).await?)
}

pub async fn get_provider_runtime_config(
    pool: &SqlitePool,
    provider_code: &str,
) -> anyhow::Result<Option<ProviderRuntimeConfigDto>> {
    Ok(sqlx::query_as::<_, ProviderRuntimeConfigDto>(
        "SELECT id, COALESCE(provider_code, '') AS provider_code, base_url, api_key_ref AS api_key FROM model_providers WHERE provider_code = ?"
    ).bind(provider_code).fetch_optional(pool).await?)
}

pub async fn insert_provider(pool: &SqlitePool, p: &NewProvider) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO model_providers (id, name, provider_type, base_url, api_key_ref, is_local, provider_code, source_category, gateway_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&p.id).bind(&p.name).bind(&p.provider_type).bind(&p.base_url).bind(&p.api_key_ref).bind(p.is_local).bind(&p.provider_code).bind(&p.source_category).bind(&p.gateway_id)
        .execute(pool).await?;
    Ok(())
}

pub async fn get_provider(
    pool: &SqlitePool,
    provider_code: &str,
) -> anyhow::Result<Option<ProviderRow>> {
    Ok(sqlx::query_as::<_, ProviderRow>("SELECT id, name, provider_type, base_url, api_key_ref, is_local FROM model_providers WHERE provider_code = ?").bind(provider_code).fetch_optional(pool).await?)
}

pub async fn update_provider(
    pool: &SqlitePool,
    id: &str,
    updates: &ProviderUpdatePatch,
) -> anyhow::Result<()> {
    let has_updates = updates.name.is_some()
        || updates.provider_type.is_some()
        || updates.base_url.is_some()
        || !updates.api_key_ref.is_missing()
        || updates.is_local.is_some()
        || updates.is_enabled.is_some()
        || updates.source_category.is_some()
        || updates.extra_config.is_some();
    if !has_updates {
        return Ok(());
    }

    let mut query = QueryBuilder::<Sqlite>::new("UPDATE model_providers SET ");
    let mut fields = query.separated(", ");
    if let Some(value) = &updates.name {
        fields.push("name = ").push_bind_unseparated(value);
    }
    if let Some(value) = &updates.provider_type {
        fields.push("provider_type = ").push_bind_unseparated(value);
    }
    if let Some(value) = &updates.base_url {
        fields.push("base_url = ").push_bind_unseparated(value);
    }
    if let PatchField::Value(value) = &updates.api_key_ref {
        fields.push("api_key_ref = ").push_bind_unseparated(value);
    }
    if let Some(value) = updates.is_local {
        fields.push("is_local = ").push_bind_unseparated(value);
    }
    if let Some(value) = updates.is_enabled {
        fields.push("is_enabled = ").push_bind_unseparated(value);
    }
    if let Some(value) = &updates.source_category {
        fields
            .push("source_category = ")
            .push_bind_unseparated(value);
    }
    if let Some(value) = &updates.extra_config {
        fields.push("extra_config = ").push_bind_unseparated(value);
    }
    drop(fields);
    query.push(" WHERE id = ").push_bind(id);
    query.build().execute(pool).await?;
    Ok(())
}

pub async fn existing_model_ids(
    pool: &SqlitePool,
    provider_code: &str,
) -> anyhow::Result<Vec<String>> {
    Ok(
        sqlx::query_scalar::<_, String>("SELECT id FROM models WHERE provider_code = ?")
            .bind(provider_code)
            .fetch_all(pool)
            .await?,
    )
}

pub async fn delete_provider(pool: &SqlitePool, id: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM model_providers WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
