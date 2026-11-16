// FastClaw · 提示词仓库层

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct PromptRow {
    pub id: String,
    pub title: String,
    pub content: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub source: String,
    pub author: Option<String>,
    pub is_official: i32,
    pub download_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

const COLS: &str =
    "id, title, content, description, category, source, author, is_official, download_count, created_at, updated_at";

pub struct NewPrompt {
    pub id: String,
    pub title: String,
    pub content: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub source: String,
    pub author: Option<String>,
}

pub async fn list(
    pool: &SqlitePool,
    category: Option<&str>,
    search: Option<&str>,
    source: Option<&str>,
) -> anyhow::Result<Vec<PromptRow>> {
    let mut sql = format!("SELECT {} FROM prompts WHERE 1=1", COLS);
    if category.is_some() {
        sql.push_str(" AND category = ?");
    }
    if source.is_some() {
        sql.push_str(" AND source = ?");
    }
    if search.is_some() {
        sql.push_str(" AND (title LIKE ? OR description LIKE ? OR content LIKE ?)");
    }
    sql.push_str(" ORDER BY is_official DESC, download_count DESC, updated_at DESC");

    let mut q = sqlx::query_as::<_, PromptRow>(&sql);
    if let Some(c) = category {
        q = q.bind(c.to_string());
    }
    if let Some(s) = source {
        q = q.bind(s.to_string());
    }
    if let Some(s) = search {
        let like = format!("%{}%", s);
        q = q.bind(like.clone()).bind(like.clone()).bind(like);
    }
    let rows = q.fetch_all(pool).await?;
    Ok(rows)
}

/// 用户的"我的提示词"：包括自己创建（source='user' 且 author=user_id）以及收藏的
pub async fn list_my(pool: &SqlitePool, user_id: &str) -> anyhow::Result<Vec<PromptRow>> {
    let sql = format!(
        "SELECT {cols} FROM prompts p \
         WHERE EXISTS (SELECT 1 FROM prompt_downloads d WHERE d.prompt_id = p.id AND d.user_id = ?) \
            OR (p.source = 'user' AND p.author = ?) \
         ORDER BY p.updated_at DESC",
        cols = COLS
    );
    let rows = sqlx::query_as::<_, PromptRow>(&sql)
        .bind(user_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn get(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<PromptRow>> {
    let sql = format!("SELECT {} FROM prompts WHERE id = ?", COLS);
    let row = sqlx::query_as::<_, PromptRow>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn insert(pool: &SqlitePool, p: &NewPrompt) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO prompts (id, title, content, description, category, source, author) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&p.id)
    .bind(&p.title)
    .bind(&p.content)
    .bind(&p.description)
    .bind(&p.category)
    .bind(&p.source)
    .bind(&p.author)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    title: Option<&str>,
    content: Option<&str>,
    description: Option<&str>,
    category: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE prompts SET \
            title = COALESCE(?, title), \
            content = COALESCE(?, content), \
            description = COALESCE(?, description), \
            category = COALESCE(?, category), \
            updated_at = datetime('now') \
         WHERE id = ?",
    )
    .bind(title)
    .bind(content)
    .bind(description)
    .bind(category)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: &str) -> anyhow::Result<u64> {
    // 系统提示词不可删
    let res = sqlx::query("DELETE FROM prompts WHERE id = ? AND source != 'system'")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}

pub async fn record_download(
    pool: &SqlitePool,
    prompt_id: &str,
    user_id: &str,
) -> anyhow::Result<()> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT OR IGNORE INTO prompt_downloads (id, prompt_id, user_id) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(prompt_id)
        .bind(user_id)
        .execute(pool)
        .await?;

    sqlx::query("UPDATE prompts SET download_count = download_count + 1 WHERE id = ?")
        .bind(prompt_id)
        .execute(pool)
        .await?;
    Ok(())
}
