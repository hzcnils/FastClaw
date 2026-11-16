use crate::{core::error::AppError, state::AppState};
use serde::Serialize;
use sqlx::Row;
use tauri::State;

// 用户明确选择明文 localStorage 保存；此类型不实现 Debug，禁止记录令牌。
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FastPlanResumeData {
    pub pid: u32,
    pub process_started_at: u64,
    pub parent_token: String,
}

#[tauri::command]
pub fn provide_fastplan_resume_data(state: State<'_, AppState>, data: Option<FastPlanResumeData>) {
    state.dependency_recovery.load_fastplan_resume(data);
}

#[tauri::command]
pub async fn get_fastplan_resume_data(state: State<'_, AppState>) -> Result<Option<FastPlanResumeData>, AppError> {
    let connection = match get_fastplan_connection(state.clone()).await {
        Ok(connection) => connection,
        Err(_) => return Ok(None),
    };
    Ok(crate::core::dependency_services::dependency_service_supervisor().snapshot("fastplan").await
        .and_then(|runtime| runtime.process).map(|identity| FastPlanResumeData {
            pid: identity.pid, process_started_at: identity.process_started_at, parent_token: connection.parent_token,
        }))
}

/// Kept only for the currently owned, ready child; Debug never prints its token.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FastPlanConnectionInfo {
    pub ws_base: String,
    pub api_base: String,
    pub parent_token: String,
}

impl std::fmt::Debug for FastPlanConnectionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FastPlanConnectionInfo")
            .field("api_base", &self.api_base)
            .finish_non_exhaustive()
    }
}

#[tauri::command]
pub async fn get_fastplan_connection(
    state: State<'_, AppState>,
) -> Result<FastPlanConnectionInfo, AppError> {
    let (tracked_pid, connection) = {
        let guard = state.kg_pids.read().await;
        (guard.fastplan, guard.fastplan_connection.clone())
    };
    let Some(connection) = connection else {
        return Err(AppError::Business(
            "FastPlan 尚未就绪，请启动服务后重试".into(),
        ));
    };
    let Some(tracked_pid) = tracked_pid else {
        let mut guard = state.kg_pids.write().await;
        if guard.fastplan.is_none() && guard.fastplan_connection.as_ref() == Some(&connection) {
            guard.fastplan_connection = None;
        }
        return Err(AppError::Business(
            "FastPlan 尚未就绪，请启动服务后重试".into(),
        ));
    };

    // netstat 是同步外部进程调用：移出 Tokio async worker，且执行期间不持有 kg_pids 写锁。
    let port = reqwest::Url::parse(&connection.api_base)
        .ok()
        .and_then(|url| url.port_or_known_default())
        .unwrap_or_else(crate::utils::paths::fastplan_default_port);
    let owner = tokio::task::spawn_blocking(move || super::wsp::get_pid_listening_on_port(port))
        .await
        .map_err(|error| {
            AppError::Internal(anyhow::anyhow!("FastPlan 进程归属校验任务失败: {error}"))
        })?;
    if owner == Some(tracked_pid) {
        return Ok(connection);
    }

    // 校验期间服务可能完成重启；只清除仍与本次快照相同的旧连接。
    let mut guard = state.kg_pids.write().await;
    if guard.fastplan == Some(tracked_pid)
        && guard.fastplan_connection.as_ref() == Some(&connection)
    {
        guard.fastplan_connection = None;
    }
    Err(AppError::Business(
        "FastPlan 尚未就绪，请启动服务后重试".into(),
    ))
}

/// Resolve one exact database model ID on every send; never select a default model.
#[tauri::command]
pub async fn get_fastplan_model_config(
    state: State<'_, AppState>,
    model_id: String,
) -> Result<serde_json::Value, AppError> {
    let pool = state.db.pool().await;
    resolve_model_config(&pool, &model_id).await
}

pub(crate) async fn resolve_model_config(
    pool: &sqlx::SqlitePool,
    model_id: &str,
) -> Result<serde_json::Value, AppError> {
    let row = sqlx::query("SELECT m.model_id, p.provider_type, p.base_url, p.api_key_ref, p.is_local, p.extra_config FROM models m JOIN model_providers p ON p.provider_code = m.provider_code WHERE m.id = ? AND m.is_enabled = 1 AND p.is_enabled = 1")
        .bind(model_id).fetch_optional(pool).await?
        .ok_or_else(|| AppError::Business("所选模型不存在或供应商已停用，请重新选择模型".into()))?;
    let model: String = row.try_get("model_id")?;
    let base_url: String = row.try_get("base_url")?;
    let provider: String = row.try_get("provider_type")?;
    let url = reqwest::Url::parse(&base_url)
        .map_err(|_| AppError::Business("模型端点 URL 无效".into()))?;
    if model.trim().is_empty()
        || !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(AppError::Business(
            "请配置模型名称及不含内嵌凭据的 HTTP/HTTPS 端点".into(),
        ));
    }
    let stored: Option<String> = row.try_get("api_key_ref")?;
    let key = stored
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|value| {
            keyring::Entry::new("fastclaw", value)
                .ok()
                .and_then(|entry| entry.get_password().ok())
                .unwrap_or_else(|| value.to_owned())
        })
        .unwrap_or_default();
    let extra: Option<String> = row.try_get("extra_config")?;
    let extra: serde_json::Value = extra
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default();
    let local: bool = row.try_get("is_local")?;
    let auth_type = extra
        .get("auth_type")
        .and_then(|value| value.as_str())
        .unwrap_or(if local && key.is_empty() {
            "none"
        } else {
            "bearer"
        });
    if !matches!(auth_type, "none" | "bearer") || (auth_type == "bearer" && key.trim().is_empty()) {
        return Err(AppError::Business(
            "所选模型缺少有效密钥；无鉴权端点请配置 auth_type=none".into(),
        ));
    }
    Ok(
        serde_json::json!({ "provider": if provider == "openai" { "openai_compatible" } else { &provider },
        "model": model, "base_url": base_url, "auth_type": auth_type,
        "api_key": if auth_type == "none" { "" } else { &key } }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn database() -> sqlx::SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE model_providers (provider_code TEXT, provider_type TEXT, base_url TEXT, api_key_ref TEXT, is_local INTEGER, is_enabled INTEGER, extra_config TEXT)").execute(&pool).await.unwrap();
        sqlx::query(
            "CREATE TABLE models (id TEXT, model_id TEXT, provider_code TEXT, is_enabled INTEGER)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO model_providers VALUES ('a','openai','http://localhost:9001/v1','test-key-a',0,1,NULL),('b','openai','http://localhost:9002/v1','test-key-b',0,1,NULL)").execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO models VALUES ('db-a','same-model','a',1),('db-b','same-model','b',1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn exact_selection_and_fresh_credentials() {
        let pool = database().await;
        let a = resolve_model_config(&pool, "db-a").await.unwrap();
        let b = resolve_model_config(&pool, "db-b").await.unwrap();
        assert_eq!(a["api_key"], "test-key-a");
        assert_eq!(b["api_key"], "test-key-b");
        assert_eq!(a["base_url"], "http://localhost:9001/v1");
        assert_eq!(b["provider"], "openai_compatible");
        assert!(resolve_model_config(&pool, "same-model").await.is_err());
        sqlx::query("UPDATE model_providers SET api_key_ref='updated-key' WHERE provider_code='a'")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            resolve_model_config(&pool, "db-a").await.unwrap()["api_key"],
            "updated-key"
        );
    }

    #[tokio::test]
    async fn missing_key_disabled_provider_and_no_auth() {
        let pool = database().await;
        sqlx::query("UPDATE model_providers SET api_key_ref=NULL WHERE provider_code='a'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(resolve_model_config(&pool, "db-a").await.is_err());
        sqlx::query("UPDATE model_providers SET extra_config='{\"auth_type\":\"none\"}' WHERE provider_code='a'").execute(&pool).await.unwrap();
        let config = resolve_model_config(&pool, "db-a").await.unwrap();
        assert_eq!(config["auth_type"], "none");
        assert_eq!(config["api_key"], "");
        sqlx::query("UPDATE model_providers SET is_enabled=0 WHERE provider_code='a'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(resolve_model_config(&pool, "db-a").await.is_err());
    }

    #[tokio::test]
    async fn invalid_endpoint_and_debug_do_not_expose_credentials() {
        let pool = database().await;
        sqlx::query("UPDATE model_providers SET base_url='http://user:password@localhost/v1'")
            .execute(&pool)
            .await
            .unwrap();
        let error = resolve_model_config(&pool, "db-a")
            .await
            .unwrap_err()
            .to_string();
        assert!(!error.contains("password"));
        let connection = FastPlanConnectionInfo {
            ws_base: "ws://localhost".into(),
            api_base: "http://localhost".into(),
            parent_token: "private-token".into(),
        };
        assert!(!format!("{connection:?}").contains("private-token"));
    }
}
