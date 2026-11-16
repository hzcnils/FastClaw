//! 每次创建子进程都读取最新配置，不修改进程级环境，不记录密钥。
use crate::{core::error::AppError, state::AppState};
use std::collections::BTreeMap;
use tauri::Manager;

pub async fn snapshot(app: &tauri::AppHandle) -> Result<BTreeMap<String, String>, AppError> {
    let state = app.state::<AppState>();
    let pool = state.db.pool().await;
    let settings = sqlx::query_as::<_, (String, String)>(
        "SELECT key,value FROM settings WHERE key IN ('default_model','embedding_model','embedding_dimension')",
    ).fetch_all(&*pool).await?.into_iter().collect::<BTreeMap<_, _>>();
    let selected = settings
        .get("default_model")
        .map(|value| value.trim_matches('"'))
        .unwrap_or("");
    // 选中顺序：精确 id > model_id > is_default。
    // ① 原先 `m.id=? OR m.model_id=?` 挤在同一个 CASE 档里，model_id 重名时
    //    （本机有三行 model_id='deepseek-v4-pro'：DEEPSEEK / HUIZHI / PRIVATE）
    //    退化成按 m.id 字母序决胜，用户设的 default_model 会被 `HUIZHI-*` 抢走，
    //    落到内部网关而不是所选 provider。拆成两档后精确 id 优先。
    // ② 兜底档排除嵌入模型：default_model 未设置时曾选到嵌入模型，
    //    拿它做 agent 推理会被上游以 `reasoning_content` 为由 400 打回，
    //    整棵任务树跑不动。
    //    显式指定的 id/model_id 不受①的排除影响，用户仍可强制选择。
    let model = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT m.model_id,p.base_url,p.api_key_ref FROM models m JOIN model_providers p ON p.provider_code=m.provider_code WHERE m.is_enabled=1 AND p.is_enabled=1 AND (m.id=? OR m.model_id=? OR m.model_type <> 'embedding') ORDER BY CASE WHEN m.id=? THEN 0 WHEN m.model_id=? THEN 1 WHEN m.is_default=1 THEN 2 ELSE 3 END,m.sort_order,m.id LIMIT 1",
    ).bind(selected).bind(selected).bind(selected).bind(selected).fetch_optional(&*pool).await?;
    let mut values = BTreeMap::new();
    // 空值覆盖历史父进程环境：删除/禁用模型后不得继承旧密钥。
    for key in [
        "FASTCLAW_LLM_MODEL",
        "FASTCLAW_LLM_BASE_URL",
        "FASTCLAW_LLM_API_KEY",
        "OPENAI_API_KEY",
        "FASTCLAW_EMBEDDING_API_KEY",
    ] {
        values.insert(key.into(), String::new());
    }
    if let Some((model, base, key)) = model {
        values.insert("FASTCLAW_LLM_MODEL".into(), model);
        values.insert(
            "FASTCLAW_LLM_BASE_URL".into(),
            base.trim_end_matches('/')
                .trim_end_matches("/v1")
                .to_string(),
        );
        let key = key.unwrap_or_default();
        values.insert("FASTCLAW_LLM_API_KEY".into(), key.clone());
        values.insert("OPENAI_API_KEY".into(), key);
    }
    for (setting, env, fallback) in [
        ("embedding_model", "FASTCLAW_EMBEDDING_MODEL", "bge-m3"),
        ("embedding_dimension", "FASTCLAW_EMBEDDING_DIM", "1024"),
    ] {
        values.insert(
            env.into(),
            settings
                .get(setting)
                .map(|v| v.trim_matches('"'))
                .filter(|v| !v.is_empty() && *v != "null")
                .unwrap_or(fallback)
                .into(),
        );
    }
    let port = crate::commands::wsp::dependency_runtime_port("llama_server")
        .unwrap_or(crate::utils::paths::LLAMA_SERVER_DEFAULT_PORT);
    let embedding = format!("http://127.0.0.1:{port}/v1/embeddings");
    values.insert("FASTCLAW_EMBEDDING_URL".into(), embedding.clone());
    values.insert("FASTKG_EMBEDDING_URL".into(), embedding);
    Ok(values)
}
