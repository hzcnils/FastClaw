use sqlx::SqlitePool;

pub async fn list_models_tree(pool: &SqlitePool) -> anyhow::Result<Vec<serde_json::Value>> {
    let providers = sqlx::query_as::<_, (String, String, String, Option<String>)>(
        "SELECT provider_code, id, name, source_category FROM model_providers WHERE is_enabled = 1 ORDER BY sort_order"
    ).fetch_all(pool).await?;

    let mut result = Vec::new();
    for (pcode, pid, pname, scategory) in providers {
        let models: Vec<(String, String, String, String, Option<String>, bool)> = sqlx::query_as::<_, (String, String, String, String, Option<String>, bool)>(
            "SELECT id, model_id, display_name, model_label, mxcs, COALESCE(supports_vision,0) FROM models WHERE provider_code = ? AND is_enabled = 1 ORDER BY sort_order"
        ).bind(&pcode).fetch_all(pool).await?;

        result.push(serde_json::json!({
            "platformId": pid,
            "platformName": pname,
            "providerCode": pcode,
            "sourceCategory": scategory.as_deref().unwrap_or("online"),
            "modelList": models.iter().map(|(mid, raw_id, mname, mlabel, mxcs, sv)| serde_json::json!({
                "modelId": mid,
                "rawModelId": raw_id,
                "modelName": mname,
                "modelLabel": mlabel,
                "mxcs": mxcs,
                "supportsVision": sv,
            })).collect::<Vec<_>>(),
        }));
    }

    // 获取本地模型（provider_code 为 NULL 的模型）
    let local_models: Vec<(String, String, String, String, Option<String>, bool)> = sqlx::query_as::<_, (String, String, String, String, Option<String>, bool)>(
        "SELECT id, model_id, display_name, model_label, mxcs, COALESCE(supports_vision,0) FROM models WHERE provider_code IS NULL AND is_enabled = 1 ORDER BY sort_order"
    ).fetch_all(pool).await?;

    if !local_models.is_empty() {
        result.push(serde_json::json!({
            "platformId": "local",
            "platformName": "本地模型",
            "modelList": local_models.iter().map(|(mid, raw_id, mname, mlabel, mxcs, sv)| serde_json::json!({
                "modelId": mid,
                "rawModelId": raw_id,
                "modelName": mname,
                "modelLabel": mlabel,
                "mxcs": mxcs,
                "supportsVision": sv,
            })).collect::<Vec<_>>(),
        }));
    }

    Ok(result)
}

// 获取所有供应商的 (provider_code, id, name) 元组，用于同步匹配
pub async fn list_all_providers_raw(
    pool: &SqlitePool,
) -> anyhow::Result<Vec<(String, String, String)>> {
    let rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT provider_code, id, name FROM model_providers ORDER BY sort_order",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// 获取所有供应商（包括没有模型的），用于默认模型设置
pub async fn list_all_providers_with_models(
    pool: &SqlitePool,
) -> anyhow::Result<Vec<serde_json::Value>> {
    log::info!("开始查询所有供应商...");
    let providers = sqlx::query_as::<_, (String, String, String, bool)>(
        "SELECT provider_code, id, name, is_enabled FROM model_providers ORDER BY sort_order",
    )
    .fetch_all(pool)
    .await?;
    log::info!("查询到 {} 个供应商", providers.len());

    let mut result = Vec::new();
    for (pcode, pid, pname, is_enabled) in providers {
        // 获取该供应商下的所有启用模型
        let models: Vec<(String, String, String)> = sqlx::query_as::<_, (String, String, String)>(
            "SELECT id, display_name, model_label FROM models WHERE provider_code = ? AND is_enabled = 1 ORDER BY sort_order"
        ).bind(&pcode).fetch_all(pool).await?;

        result.push(serde_json::json!({
            "platformId": pid,
            "platformName": pname,
            "providerCode": pcode,
            "isEnabled": is_enabled,
            "modelList": models.iter().map(|(mid, mname, mlabel)| serde_json::json!({
                "modelId": mid,
                "modelName": mname,
                "modelLabel": mlabel,
            })).collect::<Vec<_>>(),
        }));
    }

    // 获取本地模型（provider_code 为 NULL 的模型）
    let local_models: Vec<(String, String, String)> = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, display_name, model_label FROM models WHERE provider_code IS NULL AND is_enabled = 1 ORDER BY sort_order"
    ).fetch_all(pool).await?;

    if !local_models.is_empty() {
        result.push(serde_json::json!({
            "platformId": "local",
            "platformName": "本地模型",
            "isEnabled": true,
            "modelList": local_models.iter().map(|(mid, mname, mlabel)| serde_json::json!({
                "modelId": mid,
                "modelName": mname,
                "modelLabel": mlabel,
            })).collect::<Vec<_>>(),
        }));
    }

    Ok(result)
}
