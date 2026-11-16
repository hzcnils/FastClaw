// FastClaw · 模型供应商仓库层
pub mod aggregates;
pub mod catalog;
pub mod gateways;
pub mod local;
pub mod providers;
pub mod sync;
pub mod wsp_market;

pub use aggregates::*;
pub use catalog::*;
pub use gateways::{
    delete_gateway, insert_gateway, list_gateways, list_providers_by_gateway, update_gateway,
    NewGateway,
};
pub use local::*;
pub use providers::*;
pub use wsp_market::*;

#[cfg(test)]
mod typed_contract_tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::{
        get_provider_runtime_config, list_models, list_provider_summaries, list_providers,
        update_models, update_provider,
    };
    use crate::core::models::{ModelUpdatePatch, ProviderUpdatePatch};

    #[tokio::test]
    async fn provider_query_exposes_full_key_and_null_patch_clears_it() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE model_providers (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, provider_type TEXT NOT NULL,
                base_url TEXT NOT NULL, api_key_ref TEXT, is_local INTEGER NOT NULL,
                is_enabled INTEGER NOT NULL, provider_code TEXT, source_category TEXT,
                extra_config TEXT, gateway_id TEXT, sort_order INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO model_providers (id,name,provider_type,base_url,api_key_ref,is_local,is_enabled,provider_code,source_category) VALUES ('p1','Provider','openai','', 'sk-full-key',0,1,'PROVIDER','online')")
            .execute(&pool).await.unwrap();

        let providers = list_providers(&pool).await.unwrap();
        assert_eq!(providers[0].api_key.as_deref(), Some("sk-full-key"));

        let summaries = list_provider_summaries(&pool).await.unwrap();
        assert!(summaries[0].has_api_key);
        let serialized = serde_json::to_value(&summaries[0]).unwrap();
        assert!(serialized.get("api_key").is_none());
        assert!(serialized.get("api_key_ref").is_none());

        let runtime = get_provider_runtime_config(&pool, "PROVIDER")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(runtime.api_key.as_deref(), Some("sk-full-key"));

        let patch: ProviderUpdatePatch =
            serde_json::from_value(serde_json::json!({"api_key_ref": null})).unwrap();
        update_provider(&pool, "p1", &patch).await.unwrap();
        let key = sqlx::query_scalar::<_, Option<String>>(
            "SELECT api_key_ref FROM model_providers WHERE id = 'p1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(key, None);
    }

    #[tokio::test]
    async fn model_patch_binds_numbers_booleans_and_nullable_values_by_type() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE models (
                id TEXT PRIMARY KEY, model_id TEXT NOT NULL, display_name TEXT NOT NULL,
                model_type TEXT NOT NULL, is_enabled INTEGER NOT NULL, model_label TEXT NOT NULL,
                mxcs TEXT, context_window INTEGER, provider_code TEXT,
                supports_vision INTEGER NOT NULL DEFAULT 0, max_output INTEGER,
                input_price REAL, output_price REAL, temperature REAL, top_p REAL,
                sort_order INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO models (id,model_id,display_name,model_type,is_enabled,model_label,mxcs,context_window,provider_code) VALUES ('m1','gpt','GPT','chat',1,'推理','old',8192,'OPENAI')")
            .execute(&pool).await.unwrap();

        let patch: ModelUpdatePatch = serde_json::from_value(serde_json::json!({
            "context_window": 32768,
            "is_enabled": false,
            "mxcs": null
        }))
        .unwrap();
        update_models(&pool, &["m1".to_string()], &patch)
            .await
            .unwrap();

        let models = list_models(&pool, Some("OPENAI")).await.unwrap();
        assert_eq!(models[0].context_window, Some(32768));
        assert!(!models[0].is_enabled);
        assert_eq!(models[0].mxcs, None);
    }
}
