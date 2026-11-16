use sqlx::{Row, SqlitePool};

fn deser_maybe_int<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;
    let v: serde_json::Value = serde::Deserialize::deserialize(deserializer)?;
    match v {
        serde_json::Value::Null => Ok(0),
        serde_json::Value::Number(n) => n
            .as_i64()
            .ok_or_else(|| de::Error::custom("not a valid integer")),
        serde_json::Value::String(s) => s
            .parse::<i64>()
            .map_err(|_| de::Error::custom(format!("cannot parse '{}' as integer", s))),
        _ => Err(de::Error::custom("expected number or string")),
    }
}

fn deser_nullable_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let v: Option<String> = Option::deserialize(deserializer)?;
    Ok(v.unwrap_or_default())
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WspMarketModel {
    #[serde(deserialize_with = "deser_maybe_int", default)]
    pub id: i64,
    #[serde(deserialize_with = "deser_nullable_string", default)]
    pub exeName: String,
    #[serde(deserialize_with = "deser_nullable_string", default)]
    pub exeOriginName: String,
    #[serde(deserialize_with = "deser_nullable_string", default)]
    pub exePath: String,
    #[serde(deserialize_with = "deser_nullable_string", default)]
    pub exeRelativePath: String,
    #[serde(deserialize_with = "deser_nullable_string", default)]
    pub exeSize: String,
    #[serde(deserialize_with = "deser_nullable_string", default)]
    pub exeStoreType: String,
    #[serde(default)]
    pub lastRunTime: Option<String>,
    #[serde(deserialize_with = "deser_maybe_int", default)]
    pub categoryId: i64,
    #[serde(deserialize_with = "deser_maybe_int", default)]
    pub subCategoryId: i64,
    #[serde(default)]
    pub categoryName: Option<String>,
    #[serde(default)]
    pub subCategoryName: Option<String>,
    #[serde(deserialize_with = "deser_nullable_string", default)]
    pub skillName: String,
    #[serde(deserialize_with = "deser_nullable_string", default)]
    pub skillDescription: String,
    #[serde(deserialize_with = "deser_nullable_string", default)]
    pub skillInstruction: String,
    #[serde(deserialize_with = "deser_nullable_string", default)]
    pub skillLicense: String,
    #[serde(deserialize_with = "deser_nullable_string", default)]
    pub skillCompatibility: String,
    #[serde(deserialize_with = "deser_nullable_string", default)]
    pub skillVersion: String,
    #[serde(default)]
    pub skillUrl: Option<String>,
    #[serde(default)]
    pub skillWellKnown: Option<String>,
    #[serde(default)]
    pub skillEntryPoint: Option<String>,
    #[serde(default)]
    pub startupArgs: Option<String>,
    #[serde(default)]
    pub startupApiUrl: Option<String>,
    #[serde(default)]
    pub startupApiMethod: Option<String>,
    #[serde(default)]
    pub startupApiParams: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub remark: Option<String>,
    #[serde(default)]
    pub createUser: Option<String>,
    #[serde(default)]
    pub createDept: Option<String>,
    #[serde(default)]
    pub createTime: Option<String>,
    #[serde(default)]
    pub updateUser: Option<String>,
    #[serde(default)]
    pub updateTime: Option<String>,
    #[serde(deserialize_with = "deser_maybe_int", default)]
    pub status: i64,
    #[serde(deserialize_with = "deser_maybe_int", default)]
    pub isDeleted: i64,
    #[serde(deserialize_with = "deser_nullable_string", default)]
    pub tenantId: String,
}

/// 增量同步：插入或更新记录（只按 id 判断，skillName 可以重复）
pub async fn sync_wsp_market_models(
    pool: &SqlitePool,
    models: &[WspMarketModel],
) -> anyhow::Result<usize> {
    // 先清空表，再全量插入最新数据
    sqlx::query("DELETE FROM wsp_model_market")
        .execute(pool)
        .await?;
    // log::info!("[sync_wsp_market_models] 已清空 wsp_model_market 表");

    let mut inserted = 0;
    for m in models {
        // log::info!("[sync_wsp_market_models] 插入模型 id={}, skillName={}", m.id, m.skillName);
        match sqlx::query(
            "INSERT INTO wsp_model_market (id, exeName, exeOriginName, exePath, exeRelativePath, exeSize, exeStoreType, lastRunTime, categoryId, subCategoryId, categoryName, subCategoryName, skillName, skillDescription, skillInstruction, skillLicense, skillCompatibility, skillVersion, skillUrl, skillWellKnown, skillEntryPoint, startupArgs, startupApiUrl, startupApiMethod, startupApiParams, tag, remark, createUser, createDept, createTime, updateUser, updateTime, status, isDeleted, tenantId) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"
        )
        .bind(m.id).bind(&m.exeName).bind(&m.exeOriginName).bind(&m.exePath).bind(&m.exeRelativePath)
        .bind(&m.exeSize).bind(&m.exeStoreType).bind(&m.lastRunTime)
        .bind(m.categoryId).bind(m.subCategoryId).bind(&m.categoryName).bind(&m.subCategoryName)
        .bind(&m.skillName).bind(&m.skillDescription).bind(&m.skillInstruction)
        .bind(&m.skillLicense).bind(&m.skillCompatibility).bind(&m.skillVersion)
        .bind(&m.skillUrl).bind(&m.skillWellKnown).bind(&m.skillEntryPoint)
        .bind(&m.startupArgs).bind(&m.startupApiUrl).bind(&m.startupApiMethod).bind(&m.startupApiParams)
        .bind(&m.tag).bind(&m.remark)
        .bind(&m.createUser).bind(&m.createDept).bind(&m.createTime)
        .bind(&m.updateUser).bind(&m.updateTime)
        .bind(m.status).bind(m.isDeleted).bind(&m.tenantId)
        .execute(pool).await {
            Ok(_) => {
                inserted += 1;
            }
            Err(e) => {
                // log::error!("[sync_wsp_market_models] 插入模型 id={} 失败: {}", m.id, e);
            }
        }
    }
    log::info!(
        "[sync_wsp_market_models] 同步完成: 清空旧数据后插入 {} 个模型",
        inserted
    );
    Ok(inserted)
}

/// 查询所有已启用的市场模型
pub async fn list_wsp_market_models(pool: &SqlitePool) -> anyhow::Result<Vec<serde_json::Value>> {
    let rows = sqlx::query(
        "SELECT m.*, CASE WHEN s.id IS NOT NULL THEN 1 ELSE 0 END AS is_downloaded FROM wsp_model_market m LEFT JOIN wsp_services s ON m.id = s.id"
    ).fetch_all(pool).await?;

    Ok(rows
        .iter()
        .map(|row| {
            let get_str =
                |col: &str| -> String { row.get::<Option<String>, _>(col).unwrap_or_default() };
            let get_opt = |col: &str| -> Option<String> { row.get::<Option<String>, _>(col) };
            let get_int = |col: &str| -> i64 { row.get::<Option<i64>, _>(col).unwrap_or(0) };
            let get_int_str = |col: &str| -> String { get_int(col).to_string() };
            let get_bool = |col: &str| -> bool { row.get::<Option<i64>, _>(col).unwrap_or(0) == 1 };
            // 分类树未写入时 categoryName/subCategoryName 常为 NULL、skillName 为空；
            // 用 exeName 兜底，便于前端按菜单标题匹配未下载模型。
            let exe_name = get_str("exeName");
            let skill_name = {
                let s = get_str("skillName");
                if s.is_empty() {
                    exe_name.clone()
                } else {
                    s
                }
            };
            let sub_category_name = get_opt("subCategoryName").or_else(|| {
                if exe_name.is_empty() {
                    None
                } else {
                    Some(exe_name.clone())
                }
            });
            serde_json::json!({
                "id": get_int_str("id"),
                "isDownloaded": get_bool("is_downloaded"),
                "exeName": exe_name,
                "exeOriginName": get_str("exeOriginName"),
                "exePath": get_str("exePath"),
                "exeRelativePath": get_str("exeRelativePath"),
                "exeSize": get_str("exeSize"),
                "exeStoreType": get_str("exeStoreType"),
                "lastRunTime": get_opt("lastRunTime"),
                "categoryId": get_int_str("categoryId"),
                "subCategoryId": get_int_str("subCategoryId"),
                "categoryName": get_opt("categoryName"),
                "subCategoryName": sub_category_name,
                "skillName": skill_name,
                "skillDescription": get_str("skillDescription"),
                "skillInstruction": get_str("skillInstruction"),
                "skillLicense": get_str("skillLicense"),
                "skillCompatibility": get_str("skillCompatibility"),
                "skillVersion": get_str("skillVersion"),
                "skillUrl": get_opt("skillUrl"),
                "skillWellKnown": get_opt("skillWellKnown"),
                "skillEntryPoint": get_opt("skillEntryPoint"),
                "startupArgs": get_opt("startupArgs"),
                "startupApiUrl": get_opt("startupApiUrl"),
                "startupApiMethod": get_opt("startupApiMethod"),
                "startupApiParams": get_opt("startupApiParams"),
                "tag": get_opt("tag"),
                "remark": get_opt("remark"),
                "createUser": get_opt("createUser"),
                "createDept": get_opt("createDept"),
                "createTime": get_opt("createTime"),
                "updateUser": get_opt("updateUser"),
                "updateTime": get_opt("updateTime"),
                "status": get_int("status"),
                "isDeleted": get_int("isDeleted"),
                "tenantId": get_str("tenantId")
            })
        })
        .collect())
}
