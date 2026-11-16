use std::collections::HashMap;

use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::core::models::{
    GatewayDto, GatewayUpdatePatch, GatewayWithProvidersDto, PatchField, ProviderDto,
};

pub async fn list_with_providers(
    pool: &SqlitePool,
) -> anyhow::Result<Vec<GatewayWithProvidersDto>> {
    let gateways = list_gateways(pool).await?;
    let providers = sqlx::query_as::<_, ProviderDto>(
        "SELECT id, name, provider_type, base_url, api_key_ref AS api_key, is_local, is_enabled, provider_code, COALESCE(source_category, 'online') AS source_category, extra_config, gateway_id FROM model_providers WHERE gateway_id IS NOT NULL ORDER BY sort_order",
    )
    .fetch_all(pool)
    .await?;
    let mut grouped = HashMap::<String, Vec<ProviderDto>>::new();
    for provider in providers {
        if let Some(gateway_id) = provider.gateway_id.clone() {
            grouped.entry(gateway_id).or_default().push(provider);
        }
    }
    Ok(gateways
        .into_iter()
        .map(|gateway| GatewayWithProvidersDto {
            providers: grouped.remove(&gateway.id).unwrap_or_default(),
            gateway,
        })
        .collect())
}

pub struct NewGateway {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

pub async fn list_gateways(pool: &SqlitePool) -> anyhow::Result<Vec<GatewayDto>> {
    Ok(sqlx::query_as::<_, GatewayDto>(
        "SELECT id, name, description, sort_order, is_enabled FROM model_gateways ORDER BY sort_order"
    ).fetch_all(pool).await?)
}

pub async fn insert_gateway(pool: &SqlitePool, g: &NewGateway) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO model_gateways (id, name, description) VALUES (?, ?, ?)")
        .bind(&g.id)
        .bind(&g.name)
        .bind(&g.description)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_gateway(
    pool: &SqlitePool,
    id: &str,
    updates: &GatewayUpdatePatch,
) -> anyhow::Result<()> {
    let has_updates =
        updates.name.is_some() || !updates.description.is_missing() || updates.is_enabled.is_some();
    if !has_updates {
        return Ok(());
    }

    let mut query = QueryBuilder::<Sqlite>::new("UPDATE model_gateways SET ");
    let mut fields = query.separated(", ");
    if let Some(value) = &updates.name {
        fields.push("name = ").push_bind_unseparated(value);
    }
    if let PatchField::Value(value) = &updates.description {
        fields.push("description = ").push_bind_unseparated(value);
    }
    if let Some(value) = updates.is_enabled {
        fields.push("is_enabled = ").push_bind_unseparated(value);
    }
    drop(fields);
    query.push(" WHERE id = ").push_bind(id);
    query.build().execute(pool).await?;
    Ok(())
}

pub async fn delete_gateway(pool: &SqlitePool, id: &str) -> anyhow::Result<()> {
    // 先清除关联 provider 的 gateway_id
    sqlx::query("UPDATE model_providers SET gateway_id = NULL WHERE gateway_id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    // 再删除网关
    sqlx::query("DELETE FROM model_gateways WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_providers_by_gateway(
    pool: &SqlitePool,
    gateway_id: &str,
) -> anyhow::Result<Vec<ProviderDto>> {
    Ok(sqlx::query_as::<_, ProviderDto>(
        "SELECT id, name, provider_type, base_url, api_key_ref AS api_key, is_local, is_enabled, provider_code, COALESCE(source_category, 'online') AS source_category, extra_config, gateway_id FROM model_providers WHERE gateway_id = ? ORDER BY sort_order"
    ).bind(gateway_id).fetch_all(pool).await?)
}

// ==================== wsp_model_market 市场模型 CRUD ====================
