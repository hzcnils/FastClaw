// FastClaw · 消息仓库层

use crate::commands::chat::MessageRow;
use sqlx::SqlitePool;

pub struct NewMessage {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub model_id: Option<String>,
    pub model_instance_id: Option<String>,
    pub parent_id: Option<String>,
    pub group_id: Option<String>,
    /// 图片/截图附件 JSON（`[{id, name, type, preview_url, status}]`）
    pub attachments: Option<String>,
    pub token_input: Option<i32>,
    pub token_output: Option<i32>,
}

const SELECT_COLS: &str =
    "id, conversation_id, role, content, model_id, model_instance_id, parent_id, token_input, token_output, status, created_at, \
     group_id, is_liked, is_favorite, translation, translation_lang, think_content, is_edited, edited_at, context_cleared, attachments, \
     extra_data, kb_references";

pub async fn insert(pool: &SqlitePool, m: &NewMessage) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO messages (id, conversation_id, role, content, model_id, model_instance_id, parent_id, group_id, attachments, token_input, token_output, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))"
    ).bind(&m.id).bind(&m.conversation_id).bind(&m.role).bind(&m.content).bind(&m.model_id).bind(&m.model_instance_id).bind(&m.parent_id).bind(&m.group_id).bind(&m.attachments).bind(&m.token_input).bind(&m.token_output)
    .execute(pool).await?;
    // 更新对话的 updated_at
    sqlx::query(
        "UPDATE conversations SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
    )
    .bind(&m.conversation_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_by_conversation(
    pool: &SqlitePool,
    conv_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<MessageRow>> {
    let sql = format!(
        "SELECT {} FROM messages WHERE conversation_id = ? ORDER BY created_at ASC LIMIT ?",
        SELECT_COLS
    );
    let rows = sqlx::query_as::<_, MessageRow>(&sql)
        .bind(conv_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// 取最近 N 条，返回时间正序（旧 → 新），供远程历史与桌面会话对齐。
/// `id` 作为同一秒内消息的稳定 tiebreaker，与 get_page_before 保持排序一致，
/// 确保首屏与「加载更早」的 keyset 边界不重不漏。
pub async fn get_latest_by_conversation(
    pool: &SqlitePool,
    conv_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<MessageRow>> {
    let sql = format!(
        "SELECT {} FROM messages WHERE conversation_id = ? ORDER BY created_at DESC, id DESC LIMIT ?",
        SELECT_COLS
    );
    let mut rows = sqlx::query_as::<_, MessageRow>(&sql)
        .bind(conv_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
    rows.reverse();
    Ok(rows)
}

/// 游标分页：取 (created_at, id) 早于给定游标的更早一页，返回时间正序（旧 → 新）。
/// 与 get_latest_by_conversation 使用相同的 `ORDER BY created_at DESC, id DESC`，
/// 保证首屏与加载更多在同一秒内消息顺序一致、keyset 边界不重不漏。
pub async fn get_page_before(
    pool: &SqlitePool,
    conv_id: &str,
    before_created_at: &str,
    before_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<MessageRow>> {
    let sql = format!(
        "SELECT {} FROM messages WHERE conversation_id = ? AND (created_at < ? OR (created_at = ? AND id < ?)) ORDER BY created_at DESC, id DESC LIMIT ?",
        SELECT_COLS
    );
    let mut rows = sqlx::query_as::<_, MessageRow>(&sql)
        .bind(conv_id)
        .bind(before_created_at)
        .bind(before_created_at)
        .bind(before_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
    rows.reverse();
    Ok(rows)
}

/// 用于构建 LLM 上下文：跳过 context_cleared，以及 tool/function 行。
/// tool 行没有顶层 `tool_call_id`，回放进 DeepSeek / 小米会 400。
pub async fn get_recent(
    pool: &SqlitePool,
    conv_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<MessageRow>> {
    let sql = format!(
        "SELECT {} FROM messages WHERE conversation_id = ? AND context_cleared = 0 \
         AND role IN ('user', 'assistant') ORDER BY created_at ASC LIMIT ?",
        SELECT_COLS
    );
    let rows = sqlx::query_as::<_, MessageRow>(&sql)
        .bind(conv_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn delete(pool: &SqlitePool, id: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM messages WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 编辑消息正文（同时标记 is_edited / edited_at）
pub async fn update_content(pool: &SqlitePool, id: &str, content: &str) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE messages SET content = ?, is_edited = 1, edited_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?"
    )
    .bind(content).bind(id).execute(pool).await?;
    Ok(())
}

/// 在已写入的助手消息上追加 think 内容
pub async fn set_think_content(pool: &SqlitePool, id: &str, think: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE messages SET think_content = ? WHERE id = ?")
        .bind(think)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 写入消息的 extra_data（联网搜索结果等附加 JSON）
pub async fn set_extra_data(pool: &SqlitePool, id: &str, json: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE messages SET extra_data = ? WHERE id = ?")
        .bind(json)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 写入知识库引用切片（溯源数据 JSON）
pub async fn set_kb_references(pool: &SqlitePool, id: &str, json: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE messages SET kb_references = ? WHERE id = ?")
        .bind(json)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 把整段对话标记为上下文已清除（保留消息但不参与下次 LLM 上下文）
pub async fn mark_conversation_context_cleared(
    pool: &SqlitePool,
    conv_id: &str,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE messages SET context_cleared = 1 WHERE conversation_id = ?")
        .bind(conv_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 编辑 / 重新生成：按时间序删除指定消息及之后（from），或仅删除其后（after）。
pub async fn truncate_conversation(
    pool: &SqlitePool,
    conv_id: &str,
    from_id: Option<&str>,
    after_id: Option<&str>,
) -> anyhow::Result<u64> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM messages WHERE conversation_id = ? ORDER BY created_at ASC, id ASC",
    )
    .bind(conv_id)
    .fetch_all(pool)
    .await?;
    let ids: Vec<String> = rows.into_iter().map(|(id,)| id).collect();
    let start = if let Some(fid) = from_id.map(str::trim).filter(|s| !s.is_empty()) {
        ids.iter().position(|id| id == fid)
    } else if let Some(aid) = after_id.map(str::trim).filter(|s| !s.is_empty()) {
        ids.iter().position(|id| id == aid).map(|i| i + 1)
    } else {
        None
    };
    let Some(start) = start else {
        return Ok(0);
    };
    let mut deleted = 0u64;
    for id in ids.into_iter().skip(start) {
        let r = sqlx::query("DELETE FROM messages WHERE id = ?")
            .bind(&id)
            .execute(pool)
            .await?;
        deleted += r.rows_affected();
    }
    sqlx::query(
        "UPDATE conversations SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?",
    )
    .bind(conv_id)
    .execute(pool)
    .await?;
    Ok(deleted)
}

/// 取单条消息（用于编辑、翻译时拿原文）
pub async fn get_by_id(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<MessageRow>> {
    let sql = format!("SELECT {} FROM messages WHERE id = ?", SELECT_COLS);
    let row = sqlx::query_as::<_, MessageRow>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}
