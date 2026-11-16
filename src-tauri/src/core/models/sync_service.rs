use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::Value;
use sqlx::{Acquire, SqlitePool};

use crate::core::error::AppError;
use crate::db::repo::models::{self, sync as sync_repo, NewModel, NewProvider};

use super::infer_model_info;

const PROVIDER_NAME_OVERRIDES: &[(&str, &str)] = &[
    ("CHATGLM", "ZHIPU"),
    ("RERANKER", "LOCAL"),
    ("OLLAMA", "LOCAL"),
    ("LLAMACPP", "LOCAL"),
    ("NEWAPI", "HUIZHI"),
];

const PROVIDER_ALIASES: &[(&str, &str)] = &[
    ("google", "GEMINI"),
    ("multiprovider", "HUIZHI"),
    ("multiprovider2", "HUIZHI"),
    ("multiprovider3", "HUIZHI"),
    ("nova", "HUIZHI"),
];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SyncResult {
    pub providers_created: usize,
    pub models_inserted: usize,
    pub models_updated: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IndependentSyncResult {
    pub providers_created: usize,
    pub models_inserted: usize,
    pub models_updated: usize,
    pub models_disabled: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SyncPreview {
    pub reused_providers: Vec<ProviderReuse>,
    pub new_providers: Vec<ProviderNew>,
    pub models_insert: usize,
    pub models_update: usize,
    pub base_url_conflicts: Vec<UrlConflict>,
    pub merge_hints: Vec<MergeHint>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProviderReuse {
    pub json_name: String,
    pub db_name: String,
    pub db_display_name: String,
    pub match_level: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProviderNew {
    pub name: String,
    pub model_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UrlConflict {
    pub provider_name: String,
    pub provider_display_name: Option<String>,
    pub urls: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MergeHint {
    pub db_name: String,
    pub db_display_name: String,
    pub json_names: Vec<String>,
}

pub(crate) fn remap_provider_name(name: &str) -> &str {
    PROVIDER_NAME_OVERRIDES
        .iter()
        .find(|(from, _)| from.eq_ignore_ascii_case(name))
        .map(|(_, to)| *to)
        .unwrap_or(name)
}

fn normalize_provider_name(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .filter(|character| !matches!(character, '_' | '-' | ' '))
        .collect()
}

pub(crate) fn tokenize_and_normalize(name: &str) -> Vec<String> {
    name.split(['_', '-', ' '])
        .map(normalize_provider_name)
        .filter(|token| token.len() >= 4)
        .collect()
}

pub async fn preview_catalog_sync(
    pool: &SqlitePool,
    items: &[Value],
) -> Result<SyncPreview, AppError> {
    let providers = models::list_all_providers_raw(pool).await?;
    let mut grouped = HashMap::<String, (Vec<String>, Vec<String>)>::new();
    for item in items {
        let provider_name = remap_provider_name(
            item.get("provider")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
        )
        .to_string();
        let entry = grouped.entry(provider_name).or_default();
        entry.0.push(
            item.get("model")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
        );
        if let Some(url) = item
            .get("baseUrl")
            .and_then(Value::as_str)
            .filter(|url| !url.is_empty())
        {
            entry.1.push(url.to_string());
        }
    }
    let mut preview = SyncPreview {
        reused_providers: vec![],
        new_providers: vec![],
        models_insert: 0,
        models_update: 0,
        base_url_conflicts: vec![],
        merge_hints: vec![],
    };
    let mut aliases = HashMap::<String, (Vec<String>, String)>::new();
    for (json_name, (model_ids, urls)) in grouped {
        let mut unique_urls = vec![];
        let mut seen_urls = HashSet::new();
        for url in urls {
            if seen_urls.insert(url.clone()) {
                unique_urls.push(url);
            }
        }
        if let Some((code, _, level, display_name)) = match_provider(&json_name, &providers) {
            preview.models_update += model_ids.len();
            preview.reused_providers.push(ProviderReuse {
                json_name: json_name.clone(),
                db_name: code.clone(),
                db_display_name: display_name.clone(),
                match_level: level,
            });
            aliases
                .entry(code)
                .or_insert_with(|| (vec![], display_name.clone()))
                .0
                .push(json_name.clone());
            if unique_urls.len() > 1 {
                preview.base_url_conflicts.push(UrlConflict {
                    provider_name: json_name,
                    provider_display_name: Some(display_name),
                    urls: unique_urls,
                });
            }
        } else {
            preview.models_insert += model_ids.len();
            preview.new_providers.push(ProviderNew {
                name: json_name.clone(),
                model_count: model_ids.len(),
            });
            if unique_urls.len() > 1 {
                preview.base_url_conflicts.push(UrlConflict {
                    provider_name: json_name,
                    provider_display_name: None,
                    urls: unique_urls,
                });
            }
        }
    }
    preview.merge_hints = aliases
        .into_iter()
        .filter(|(_, (names, _))| names.len() > 1)
        .map(|(db_name, (json_names, db_display_name))| MergeHint {
            db_name,
            db_display_name,
            json_names,
        })
        .collect();
    Ok(preview)
}

pub(crate) fn match_provider(
    json_provider: &str,
    providers: &[(String, String, String)],
) -> Option<(String, String, String, String)> {
    let normalized = normalize_provider_name(json_provider);
    for (code, id, name) in providers {
        if normalize_provider_name(code) == normalized {
            return Some((code.clone(), id.clone(), "L1:名称匹配".into(), name.clone()));
        }
    }
    for (alias, target) in PROVIDER_ALIASES {
        if *alias == normalized {
            return providers
                .iter()
                .find(|(code, _, _)| code == target)
                .map(|(code, id, name)| {
                    (code.clone(), id.clone(), "L2:别名映射".into(), name.clone())
                });
        }
    }
    let json_tokens = tokenize_and_normalize(json_provider);
    for (code, id, name) in providers {
        let code_normalized = normalize_provider_name(code);
        if code_normalized.len() < 4 {
            continue;
        }
        let name_tokens = tokenize_and_normalize(name);
        if json_tokens.iter().any(|token| {
            token == &code_normalized || name_tokens.iter().any(|other| other == token)
        }) || name_tokens.iter().any(|token| token == &normalized)
        {
            return Some((
                code.clone(),
                id.clone(),
                "L2.5:分词匹配".into(),
                name.clone(),
            ));
        }
    }
    for (code, id, name) in providers {
        let code_normalized = normalize_provider_name(code);
        if code_normalized.len().min(normalized.len()) >= 4
            && (normalized.contains(&code_normalized) || code_normalized.contains(&normalized))
        {
            return Some((
                code.clone(),
                id.clone(),
                "L2.5:包含匹配".into(),
                name.clone(),
            ));
        }
    }
    None
}

fn provider_rows(rows: Vec<sync_repo::ProviderMatchRow>) -> Vec<(String, String, String)> {
    rows.into_iter()
        .map(|row| (row.provider_code, row.id, row.name))
        .collect()
}

fn source_category(item: &Value) -> &'static str {
    match item.get("source").and_then(Value::as_str) {
        Some("LOCAL") => "local",
        Some("HUIZHI") => "huizhi",
        _ => "online",
    }
}

fn extra_config(item: &Value) -> Option<String> {
    serde_json::to_string(&serde_json::json!({
        "endpoint": item.get("endpoint").and_then(Value::as_str),
        "secretKey": item.get("secretKey").and_then(Value::as_str),
        "geminiLocation": item.get("geminiLocation").and_then(Value::as_str),
        "geminiProject": item.get("geminiProject").and_then(Value::as_str),
        "azureDeploymentName": item.get("azureDeploymentName").and_then(Value::as_str),
        "imageSize": item.get("imageSize").and_then(Value::as_str),
        "imageQuality": item.get("imageQuality").and_then(Value::as_str),
        "imageStyle": item.get("imageStyle").and_then(Value::as_str),
        "dimension": item.get("dimension").and_then(Value::as_str),
        "responseLimit": item.get("responseLimit").and_then(Value::as_i64),
        "source": item.get("source").and_then(Value::as_str),
    }))
    .ok()
}

fn api_key(item: &Value) -> Option<String> {
    item.get("apiKey")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub async fn execute_catalog_sync(
    pool: &SqlitePool,
    items: &[Value],
) -> Result<SyncResult, AppError> {
    let mut transaction = pool.begin().await?;
    let connection = transaction.acquire().await?;
    let mut providers = provider_rows(sync_repo::list_provider_matches(connection).await?);
    let mut cache = HashMap::<String, (String, String)>::new();
    let mut result = SyncResult {
        providers_created: 0,
        models_inserted: 0,
        models_updated: 0,
    };

    for item in items {
        let provider_name = remap_provider_name(
            item.get("provider")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
        )
        .to_string();
        let model_id = item
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let provider_code = if let Some((_, code)) = cache.get(&provider_name) {
            code.clone()
        } else if let Some((code, id, _, _)) = match_provider(&provider_name, &providers) {
            cache.insert(provider_name.clone(), (id, code.clone()));
            code
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            let code = provider_name.clone();
            sync_repo::insert_provider(
                connection,
                &NewProvider {
                    id: id.clone(),
                    name: provider_name.clone(),
                    provider_type: "openai".into(),
                    base_url: item
                        .get("baseUrl")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .into(),
                    api_key_ref: api_key(item),
                    is_local: false,
                    provider_code: Some(code.clone()),
                    source_category: Some(source_category(item).into()),
                    gateway_id: None,
                },
            )
            .await?;
            result.providers_created += 1;
            providers.push((code.clone(), id.clone(), provider_name.clone()));
            cache.insert(provider_name.clone(), (id, code.clone()));
            code
        };
        let display_name = item.get("name").and_then(Value::as_str).unwrap_or(model_id);
        let (model_type, model_label) = infer_model_info(
            model_id,
            Some(item.get("type").and_then(Value::as_str).unwrap_or("chat")),
        );
        let inserted = sync_repo::upsert_model(
            connection,
            &NewModel {
                id: item
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{}-{}", provider_code, model_id)),
                provider_code,
                model_id: model_id.into(),
                display_name: display_name.into(),
                model_type,
                model_label,
                context_window: None,
                max_output: item
                    .get("responseLimit")
                    .and_then(Value::as_i64)
                    .map(|value| value as i32),
                input_price: None,
                output_price: None,
                is_enabled: true,
                is_local_model: item.get("source").and_then(Value::as_str) == Some("LOCAL"),
                temperature: item.get("temperature").and_then(Value::as_f64),
                top_p: item.get("topP").and_then(Value::as_f64),
                extra_config: extra_config(item),
                mxcs: item.get("mxcs").and_then(Value::as_str).map(str::to_string),
            },
        )
        .await?;
        if inserted {
            result.models_inserted += 1;
        } else {
            result.models_updated += 1;
        }
    }

    let history_entry = serde_json::json!({
        "timestamp": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        "providers_created": result.providers_created,
        "models_inserted": result.models_inserted,
        "models_updated": result.models_updated,
    });
    let mut history = sync_repo::read_sync_history(connection)
        .await?
        .and_then(|value| serde_json::from_str::<Vec<Value>>(&value).ok())
        .unwrap_or_default();
    history.push(history_entry);
    let history_json =
        serde_json::to_string(&history).map_err(|error| AppError::Parse(error.to_string()))?;
    sync_repo::write_sync_history(connection, &history_json).await?;
    transaction.commit().await?;
    Ok(result)
}

pub async fn import_catalog_models(
    pool: &SqlitePool,
    items: &[Value],
) -> Result<SyncResult, AppError> {
    let mut transaction = pool.begin().await?;
    let connection = transaction.acquire().await?;
    let mut providers = provider_rows(sync_repo::list_provider_matches(connection).await?);
    let mut cache = HashMap::<String, (String, String)>::new();
    let mut result = SyncResult {
        providers_created: 0,
        models_inserted: 0,
        models_updated: 0,
    };

    for item in items {
        let raw_provider = item
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        let provider_name = if raw_provider.is_empty() {
            String::new()
        } else {
            remap_provider_name(raw_provider).to_string()
        };
        let provider_code = if provider_name.is_empty() {
            None
        } else if let Some((_, code)) = cache.get(&provider_name) {
            Some(code.clone())
        } else if let Some((code, id, _, _)) = match_provider(&provider_name, &providers) {
            cache.insert(provider_name.clone(), (id, code.clone()));
            Some(code)
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            let code = provider_name.clone();
            sync_repo::insert_provider(
                connection,
                &NewProvider {
                    id: id.clone(),
                    name: provider_name.clone(),
                    provider_type: "openai".into(),
                    base_url: item
                        .get("baseUrl")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .into(),
                    api_key_ref: api_key(item),
                    is_local: false,
                    provider_code: Some(code.clone()),
                    source_category: Some(source_category(item).into()),
                    gateway_id: None,
                },
            )
            .await?;
            result.providers_created += 1;
            providers.push((code.clone(), id.clone(), provider_name.clone()));
            cache.insert(provider_name.clone(), (id, code.clone()));
            Some(code)
        };
        let model_id = item
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let (model_type, model_label) = infer_model_info(
            model_id,
            Some(item.get("type").and_then(Value::as_str).unwrap_or("chat")),
        );
        let inserted = sync_repo::upsert_import_model(
            connection,
            &sync_repo::ImportModelWrite {
                id: item
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                provider_code,
                model_id: model_id.into(),
                display_name: item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(model_id)
                    .into(),
                model_type,
                model_label,
                max_output: item
                    .get("responseLimit")
                    .and_then(Value::as_i64)
                    .map(|value| value as i32),
                is_local_model: provider_name.is_empty()
                    || item.get("source").and_then(Value::as_str) == Some("LOCAL"),
                temperature: item.get("temperature").and_then(Value::as_f64),
                top_p: item.get("topP").and_then(Value::as_f64),
                extra_config: extra_config(item),
                mxcs: item.get("mxcs").and_then(Value::as_str).map(str::to_string),
            },
        )
        .await?;
        if inserted {
            result.models_inserted += 1;
        } else {
            result.models_updated += 1;
        }
    }

    transaction.commit().await?;
    Ok(result)
}

pub async fn synchronize_independent_models(
    pool: &SqlitePool,
    items: &[Value],
) -> Result<IndependentSyncResult, AppError> {
    if items.is_empty() {
        return Ok(IndependentSyncResult {
            providers_created: 0,
            models_inserted: 0,
            models_updated: 0,
            models_disabled: 0,
        });
    }
    let mut groups = HashMap::<(String, String), Vec<&Value>>::new();
    for item in items {
        let base_url = item
            .get("baseUrl")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim_end_matches('/')
            .to_string();
        let key = item
            .get("apiKey")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        groups.entry((base_url, key)).or_default().push(item);
    }

    let mut transaction = pool.begin().await?;
    let connection = transaction.acquire().await?;
    let existing = sync_repo::list_enabled_independent_models(connection).await?;
    let mut seen = HashSet::new();
    let mut result = IndependentSyncResult {
        providers_created: 0,
        models_inserted: 0,
        models_updated: 0,
        models_disabled: 0,
    };
    for ((base_url, api_key), group) in groups {
        let provider_code = ensure_independent_provider(
            connection,
            &base_url,
            &api_key,
            &mut result.providers_created,
        )
        .await?;
        for item in group {
            let Some(model_id) = item.get("id").and_then(Value::as_str) else {
                continue;
            };
            let (model_type, model_label) = infer_model_info(
                model_id,
                Some(item.get("type").and_then(Value::as_str).unwrap_or("chat")),
            );
            let inserted = sync_repo::upsert_model(
                connection,
                &NewModel {
                    id: format!("{}-{}", provider_code, model_id),
                    provider_code: provider_code.clone(),
                    model_id: model_id.into(),
                    display_name: item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or(model_id)
                        .into(),
                    model_type,
                    model_label,
                    context_window: item
                        .get("context_window")
                        .or_else(|| item.get("context_length"))
                        .and_then(Value::as_i64)
                        .map(|value| value as i32),
                    max_output: item
                        .get("max_output")
                        .or_else(|| item.get("responseLimit"))
                        .and_then(Value::as_i64)
                        .map(|value| value as i32),
                    input_price: item.get("input_price").and_then(Value::as_f64),
                    output_price: item.get("output_price").and_then(Value::as_f64),
                    is_enabled: true,
                    is_local_model: false,
                    temperature: None,
                    top_p: None,
                    extra_config: None,
                    mxcs: None,
                },
            )
            .await?;
            if inserted {
                result.models_inserted += 1;
            } else {
                result.models_updated += 1;
            }
            seen.insert((model_id.to_string(), provider_code.clone()));
        }
    }
    let removed: HashSet<_> = existing.difference(&seen).cloned().collect();
    result.models_disabled = sync_repo::disable_independent_models(connection, &removed).await?;
    transaction.commit().await?;
    Ok(result)
}

async fn ensure_independent_provider(
    connection: &mut sqlx::SqliteConnection,
    base_url: &str,
    api_key: &str,
    created: &mut usize,
) -> Result<String, AppError> {
    if let Some((code, existing_key)) =
        sync_repo::find_independent_provider(connection, base_url).await?
    {
        if existing_key.as_deref() == Some(api_key)
            || (existing_key.is_none() && api_key.is_empty())
        {
            return Ok(code);
        }
    }
    let domain = base_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("unknown")
        .split(':')
        .next()
        .unwrap_or("unknown")
        .split('.')
        .find(|part| !part.is_empty() && *part != "api" && *part != "www")
        .unwrap_or("unknown")
        .to_uppercase();
    let mut code = format!("IND_{}", domain);
    if sync_repo::provider_code_exists(connection, &code).await? {
        code = format!(
            "{}_{}",
            code,
            uuid::Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("x")
        );
    }
    sync_repo::insert_provider(
        connection,
        &NewProvider {
            id: uuid::Uuid::new_v4().to_string(),
            name: if domain == "UNKNOWN" {
                "独立模型".into()
            } else {
                format!("{} (独立)", domain)
            },
            provider_type: "openai".into(),
            base_url: base_url.into(),
            api_key_ref: (!api_key.is_empty()).then(|| api_key.to_string()),
            is_local: false,
            provider_code: Some(code.clone()),
            source_category: Some("independent".into()),
            gateway_id: None,
        },
    )
    .await?;
    *created += 1;
    Ok(code)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sqlx::sqlite::SqlitePoolOptions;

    use super::{
        execute_catalog_sync, extra_config, import_catalog_models, match_provider,
        remap_provider_name, tokenize_and_normalize,
    };

    async fn test_pool() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(
            r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE model_providers (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, provider_type TEXT NOT NULL,
                base_url TEXT NOT NULL DEFAULT '', api_key_ref TEXT, is_local INTEGER NOT NULL DEFAULT 0,
                is_enabled INTEGER NOT NULL DEFAULT 1, provider_code TEXT UNIQUE,
                source_category TEXT, gateway_id TEXT
            );
            CREATE TABLE models (
                id TEXT PRIMARY KEY, provider_code TEXT, model_id TEXT NOT NULL,
                display_name TEXT NOT NULL, model_type TEXT NOT NULL, model_label TEXT NOT NULL DEFAULT '',
                context_window INTEGER, max_output INTEGER, input_price REAL, output_price REAL,
                is_enabled INTEGER NOT NULL DEFAULT 1, is_local_model INTEGER NOT NULL DEFAULT 0,
                temperature REAL, top_p REAL, extra_config TEXT, mxcs TEXT,
                created_at TEXT DEFAULT (datetime('now')), updated_at TEXT DEFAULT (datetime('now'))
            );
            CREATE UNIQUE INDEX idx_models_provider_model_unique
                ON models(provider_code, model_id) WHERE provider_code IS NOT NULL;
            CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT, category TEXT, updated_at TEXT);
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[test]
    fn provider_matching_contract_is_preserved() {
        let providers = vec![
            ("GEMINI".into(), "gemini".into(), "Google Gemini".into()),
            ("HUIZHI".into(), "huizhi".into(), "汇智网".into()),
        ];
        assert_eq!(remap_provider_name("NEWAPI"), "HUIZHI");
        assert_eq!(
            tokenize_and_normalize("ERNIE_DEEPSEEK"),
            ["ernie", "deepseek"]
        );
        assert_eq!(match_provider("google", &providers).unwrap().0, "GEMINI");
    }

    #[test]
    fn imported_model_extra_config_does_not_duplicate_provider_api_key() {
        let serialized = extra_config(&json!({
            "apiKey": "provider-key",
            "secretKey": "model-secret",
            "source": "OPENAI"
        }))
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert!(parsed.get("apiKey").is_none());
        assert_eq!(parsed["secretKey"], "model-secret");
    }

    #[tokio::test]
    async fn credential_migration_only_removes_safe_duplicates() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(
            r#"
            CREATE TABLE model_providers (provider_code TEXT PRIMARY KEY, api_key_ref TEXT);
            CREATE TABLE models (id TEXT PRIMARY KEY, provider_code TEXT, extra_config TEXT);
            INSERT INTO model_providers VALUES ('OPENAI', 'provider-key');
            INSERT INTO model_providers VALUES ('LOCAL', '');
            INSERT INTO models VALUES ('same', 'OPENAI', '{"apiKey":"provider-key","source":"OPENAI"}');
            INSERT INTO models VALUES ('conflict', 'OPENAI', '{"apiKey":"different","source":"OPENAI"}');
            INSERT INTO models VALUES ('local', 'LOCAL', '{"apiKey":"qwen3:32b","source":"LOCAL"}');
            INSERT INTO models VALUES ('null-key', 'OPENAI', '{"apiKey":null,"source":"OPENAI"}');
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!(
            "../../../migrations/20260827000002_provider_credential_source.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();

        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT id, extra_config FROM models ORDER BY id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        let configs: std::collections::HashMap<_, _> = rows.into_iter().collect();
        assert!(!configs["same"].contains("apiKey"));
        assert!(!configs["null-key"].contains("apiKey"));
        assert!(configs["conflict"].contains("different"));
        assert!(configs["local"].contains("qwen3:32b"));
    }

    #[tokio::test]
    async fn catalog_sync_rolls_back_provider_models_and_history_together() {
        let pool = test_pool().await;
        sqlx::raw_sql(
            "CREATE TRIGGER reject_failed_model BEFORE INSERT ON models WHEN NEW.model_id = 'fail' BEGIN SELECT RAISE(ABORT, 'forced failure'); END;",
        )
        .execute(&pool)
        .await
        .unwrap();
        let items = vec![
            json!({"provider":"ROLLBACK","model":"ok","name":"OK"}),
            json!({"provider":"ROLLBACK","model":"fail","name":"Fail"}),
        ];

        assert!(execute_catalog_sync(&pool, &items).await.is_err());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM model_providers")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM models")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM settings")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn repeated_import_is_idempotent_by_provider_and_model_key() {
        let pool = test_pool().await;
        let items = vec![json!({
            "id":"first-id", "provider":"OPENAI", "model":"gpt-test", "name":"GPT Test"
        })];
        let first = import_catalog_models(&pool, &items).await.unwrap();
        let second = import_catalog_models(&pool, &items).await.unwrap();

        assert_eq!(first.models_inserted, 1);
        assert_eq!(second.models_updated, 1);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM models")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn duplicate_migration_repoints_instance_references_before_delete() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(
            r#"
            CREATE TABLE models (
                id TEXT PRIMARY KEY, provider_code TEXT, model_id TEXT,
                created_at TEXT, updated_at TEXT
            );
            CREATE TABLE messages (id TEXT PRIMARY KEY, model_instance_id TEXT);
            CREATE TABLE model_feedback (id TEXT PRIMARY KEY, model_instance_id TEXT);
            INSERT INTO models VALUES ('old', 'OPENAI', 'gpt-test', '2026-01-01', '2026-01-01');
            INSERT INTO models VALUES ('new', 'OPENAI', 'gpt-test', '2026-01-02', '2026-01-02');
            INSERT INTO messages VALUES ('message', 'old');
            INSERT INTO model_feedback VALUES ('feedback', 'old');
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!(
            "../../../migrations/20260827000001_models_provider_model_unique.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT id FROM models")
                .fetch_one(&pool)
                .await
                .unwrap(),
            "new"
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT model_instance_id FROM messages")
                .fetch_one(&pool)
                .await
                .unwrap(),
            "new"
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT model_instance_id FROM model_feedback")
                .fetch_one(&pool)
                .await
                .unwrap(),
            "new"
        );
    }
}
