//! 对话话题自动命名：首条用户消息 → 标题（固定原文截断，与桌面 ChatStore 对齐）。

use sqlx::SqlitePool;
use tracing::info;

use crate::core::error::AppError;
use crate::core::llm::LlmManager;

pub fn is_placeholder_title(title: &str) -> bool {
    let t = title.trim();
    t.is_empty()
        || t == "新对话"
        || t == "智能对话"
        || t == "规划会话"
        || t.eq_ignore_ascii_case("new chat")
}

fn truncate_title(raw: &str, max_chars: usize) -> String {
    let mut t = raw
        .trim()
        .trim_matches(|c| {
            c == '"' || c == '\'' || c == '“' || c == '”' || c == '「' || c == '」'
        })
        .replace('\n', " ")
        .trim()
        .to_string();
    if t.chars().count() > max_chars {
        t = t.chars().take(max_chars).collect();
    }
    t
}

/// `prefer_conversation_model=true`：优先用传入/会话模型（App 远程路径）。
/// `false`：优先设置里的「话题命名模型」（桌面默认）。
pub async fn resolve_topic_naming_model(
    pool: &SqlitePool,
    conversation_id: &str,
    preferred_model: Option<&str>,
    prefer_conversation_model: bool,
) -> Option<String> {
    let naming_model: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = 'topic_naming_model'",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .filter(|v| !v.trim().is_empty());

    let conv_model: Option<String> =
        sqlx::query_scalar::<_, String>("SELECT model_id FROM conversations WHERE id = ?")
            .bind(conversation_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .filter(|v| !v.trim().is_empty());

    let preferred = preferred_model
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    if prefer_conversation_model {
        preferred.or(conv_model).or(naming_model)
    } else {
        naming_model.or(preferred).or(conv_model)
    }
}

/// 生成并落库标题。若当前标题已不是占位名则跳过（返回 `None`）。
/// 历史标题固定为首条用户问题原文（截断），不再用模型概括改写。
pub async fn maybe_name_conversation_topic(
    pool: &SqlitePool,
    _llm: &LlmManager,
    conversation_id: &str,
    user_message: &str,
    _preferred_model: Option<&str>,
    _prefer_conversation_model: bool,
) -> Result<Option<String>, AppError> {
    let text = user_message.trim();
    if text.is_empty() {
        return Err(AppError::InvalidInput("消息为空，无法生成话题标题".into()));
    }

    let current_title: Option<String> =
        sqlx::query_scalar::<_, String>("SELECT title FROM conversations WHERE id = ?")
            .bind(conversation_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;

    let should_name = match current_title.as_deref() {
        None => true,
        Some(t) if is_placeholder_title(t) => true,
        // 已是首条原文（或同文截断）则不再改写
        Some(t) if t.trim() == text || text.starts_with(t.trim()) => false,
        Some(_) => false,
    };
    if !should_name {
        return Ok(None);
    }

    let title = truncate_title(&text.replace('\n', " "), 80);
    info!(
        "[话题命名] conversation={} title={} (原文截断，不调用模型)",
        conversation_id, title
    );

    sqlx::query(
        "UPDATE conversations SET title = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
    )
    .bind(&title)
    .bind(conversation_id)
    .execute(pool)
    .await
    .map_err(|e| AppError::DatabaseMsg(e.to_string()))?;

    Ok(Some(title))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_titles() {
        assert!(is_placeholder_title(""));
        assert!(is_placeholder_title(" 新对话 "));
        assert!(is_placeholder_title("智能对话"));
        assert!(is_placeholder_title("规划会话"));
        assert!(!is_placeholder_title("分析销量数据"));
    }

    #[test]
    fn truncate_keeps_short() {
        assert_eq!(truncate_title("你好世界", 12), "你好世界");
        assert_eq!(truncate_title("一二三四五六七八九十一二三", 12).chars().count(), 12);
    }
}
