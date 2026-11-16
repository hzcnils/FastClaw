use sqlx::SqlitePool;

pub struct NewLocalModel {
    pub id: String,
    pub model_name: String,
    pub model_type: String,
    pub model_url: Option<String>,
    pub model_category: Option<String>,
    pub model_desc: Option<String>,
    pub model_tags: Option<String>,
    pub model_path: Option<String>,
    pub model_version: Option<String>,
    pub model_size: Option<String>,
    pub is_enabled: bool,
}
pub async fn insert_local_model(pool: &SqlitePool, model: &NewLocalModel) -> anyhow::Result<()> {
    // 生成随机的 model_id（与 id 格式相同）
    let model_id = uuid::Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO models (
            id, provider_code, model_id, display_name, model_type, model_label,
            model_name, model_category, model_desc, model_tags, model_path, model_url,
            model_version, model_size, is_local_model, is_enabled
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&model.id)
    .bind(None::<String>) // provider_code 为 NULL 表示本地模型
    .bind(&model_id) // 使用随机生成的 model_id
    .bind(&model.model_name)
    .bind(&model.model_type)
    .bind("") // model_label
    .bind(&model.model_name)
    .bind(&model.model_category)
    .bind(&model.model_desc)
    .bind(&model.model_tags)
    .bind(&model.model_path)
    .bind(&model.model_url)
    .bind(&model.model_version)
    .bind(&model.model_size)
    .bind(0) // is_local_model = 0 (未下载)
    .bind(model.is_enabled as i32)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_model_as_downloaded(pool: &SqlitePool, model_id: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE models SET is_local_model = 1 WHERE id = ?")
        .bind(model_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_local_models(pool: &SqlitePool) -> anyhow::Result<Vec<serde_json::Value>> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, i32)>(
        "SELECT id, model_name, display_name, model_type, model_category, model_desc, model_tags, model_path, model_version, model_size, is_local_model
         FROM models
         ORDER BY created_at DESC"
    ).fetch_all(pool).await?;

    Ok(rows.iter().map(|(id, name, display_name, mtype, category, desc, tags, path, version, size, is_local)| {
        serde_json::json!({
            "id": id,
            "name": name,
            "display_name": display_name,
            "type": mtype,
            "category": category,
            "desc": desc,
            "tags": tags.as_ref().map(|t| t.split(',').collect::<Vec<_>>()).unwrap_or_default(),
            "path": path,
            "version": version,
            "size": size,
            "isDownloaded": *is_local == 1
        })
    }).collect())
}
