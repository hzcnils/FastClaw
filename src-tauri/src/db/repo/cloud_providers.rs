// FastClaw · WSP 云服务厂商数据仓库
// 对应 wsp_cloud_providers 表：阿里云 / 讯飞云的凭据与配置
// 按「厂商 + 页面（二级分类）」独立，各页面互不共用

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WspCloudProvider {
    pub id: String,
    pub provider_code: String,
    pub name: String,
    pub display_name: String,
    pub access_key_id: Option<String>,
    pub access_key_secret: Option<String>,
    pub api_key: Option<String>,
    pub endpoint: String,
    pub region: Option<String>,
    pub category: String,
    pub category_name: String,
    pub sub_category_name: String,
    pub is_enabled: bool,
    pub extra_config: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

const SELECT_COLUMNS: &str = "id, provider_code, name, display_name, access_key_id, access_key_secret, \
     api_key, endpoint, region, category, category_name, sub_category_name, is_enabled, \
     extra_config, created_at, updated_at";

/// 列出某页面（二级分类）下的云服务厂商配置
/// 各页面的凭据 / 服务地址互相独立，故必须按页面查询
pub async fn list_by_page(
    pool: &SqlitePool,
    sub_category_name: &str,
) -> anyhow::Result<Vec<WspCloudProvider>> {
    let rows = sqlx::query_as::<_, WspCloudProvider>(&format!(
        "SELECT {SELECT_COLUMNS} FROM wsp_cloud_providers \
         WHERE sub_category_name = ? ORDER BY provider_code"
    ))
    .bind(sub_category_name)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 按「厂商代码 + 页面」查询（aliyun / xfyun）
pub async fn get_by_code_and_page(
    pool: &SqlitePool,
    provider_code: &str,
    sub_category_name: &str,
) -> anyhow::Result<Option<WspCloudProvider>> {
    let row = sqlx::query_as::<_, WspCloudProvider>(&format!(
        "SELECT {SELECT_COLUMNS} FROM wsp_cloud_providers \
         WHERE provider_code = ? AND sub_category_name = ?"
    ))
    .bind(provider_code)
    .bind(sub_category_name)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 新增或更新厂商配置（按 id 主键：cloud-{厂商}-{页面}）
pub async fn upsert(pool: &SqlitePool, provider: &WspCloudProvider) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO wsp_cloud_providers
            (id, provider_code, name, display_name, access_key_id, access_key_secret, api_key,
             endpoint, region, category, category_name, sub_category_name, is_enabled, extra_config)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            display_name = excluded.display_name,
            access_key_id = excluded.access_key_id,
            access_key_secret = excluded.access_key_secret,
            api_key = excluded.api_key,
            endpoint = excluded.endpoint,
            region = excluded.region,
            category = excluded.category,
            category_name = excluded.category_name,
            sub_category_name = excluded.sub_category_name,
            is_enabled = excluded.is_enabled,
            extra_config = excluded.extra_config,
            updated_at = datetime('now')
        "#,
    )
    .bind(&provider.id)
    .bind(&provider.provider_code)
    .bind(&provider.name)
    .bind(&provider.display_name)
    .bind(&provider.access_key_id)
    .bind(&provider.access_key_secret)
    .bind(&provider.api_key)
    .bind(&provider.endpoint)
    .bind(&provider.region)
    .bind(&provider.category)
    .bind(&provider.category_name)
    .bind(&provider.sub_category_name)
    .bind(provider.is_enabled)
    .bind(&provider.extra_config)
    .execute(pool)
    .await?;
    Ok(())
}
