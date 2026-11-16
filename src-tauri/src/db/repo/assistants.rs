// FastClaw · 个人助理角色仓库层
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

// ─── 角色主表 CRUD ───

pub async fn list_roles(
    pool: &SqlitePool,
    search: Option<&str>,
    source_type: Option<&str>,
    is_active: Option<bool>,
) -> anyhow::Result<Vec<Value>> {
    let mut conditions: Vec<String> = Vec::new();
    let mut bindings: Vec<String> = Vec::new();

    if let Some(s) = search {
        if !s.is_empty() {
            conditions.push("name LIKE ?".to_string());
            bindings.push(format!("%{}%", s));
        }
    }
    if let Some(st) = source_type {
        if !st.is_empty() {
            conditions.push("source_type = ?".to_string());
            bindings.push(st.to_string());
        }
    }
    if let Some(active) = is_active {
        conditions.push("is_active = ?".to_string());
        bindings.push(if active {
            "1".to_string()
        } else {
            "0".to_string()
        });
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT id, name, description, avatar, system_prompt, default_model, temperature,
                context_turns, memory_mode, allow_publish, is_active, is_published,
                source_type, source_id, square_category, square_desc, published_at, notes,
                created_at, updated_at
         FROM roles{} ORDER BY created_at DESC",
        where_clause
    );

    let mut query = sqlx::query(&sql);
    for b in &bindings {
        query = query.bind(b);
    }
    let rows = query.fetch_all(pool).await?;
    Ok(rows.iter().map(role_row_to_json).collect())
}

pub async fn get_role(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Value>> {
    let row = sqlx::query(
        "SELECT id, name, description, avatar, system_prompt, default_model, temperature,
                context_turns, memory_mode, allow_publish, is_active, is_published,
                source_type, source_id, square_category, square_desc, published_at, notes,
                created_at, updated_at
         FROM roles WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let mut role = role_row_to_json(&row);

    let mcps = sqlx::query("SELECT mcp_id, sort_order FROM role_mcps WHERE role_id = ? ORDER BY sort_order")
        .bind(id)
        .fetch_all(pool)
        .await?
        .iter()
        .map(|r| json!({ "mcpId": r.get::<String, _>("mcp_id"), "sortOrder": r.get::<i64, _>("sort_order") }))
        .collect::<Vec<_>>();

    let skills = sqlx::query("SELECT skill_id, sort_order FROM role_skills WHERE role_id = ? ORDER BY sort_order")
        .bind(id)
        .fetch_all(pool)
        .await?
        .iter()
        .map(|r| json!({ "skillId": r.get::<String, _>("skill_id"), "sortOrder": r.get::<i64, _>("sort_order") }))
        .collect::<Vec<_>>();

    let kbs = sqlx::query("SELECT knowledge_base_id, sort_order FROM role_knowledge_bases WHERE role_id = ? ORDER BY sort_order")
        .bind(id)
        .fetch_all(pool)
        .await?
        .iter()
        .map(|r| json!({ "kbId": r.get::<String, _>("knowledge_base_id"), "sortOrder": r.get::<i64, _>("sort_order") }))
        .collect::<Vec<_>>();

    if let Some(obj) = role.as_object_mut() {
        obj.insert("mcps".to_string(), json!(mcps));
        obj.insert("skills".to_string(), json!(skills));
        obj.insert("knowledgeBases".to_string(), json!(kbs));
    }

    Ok(Some(role))
}

pub async fn insert_role(pool: &SqlitePool, id: &str, data: &Value) -> anyhow::Result<()> {
    let name = data["name"].as_str().unwrap_or("").to_string();
    let description = data["description"].as_str().map(|s| s.to_string());
    let avatar = data["avatar"].as_str().map(|s| s.to_string());
    let system_prompt = data["systemPrompt"].as_str().unwrap_or("").to_string();
    let default_model = data["defaultModel"].as_str().map(|s| s.to_string());
    let temperature = data["temperature"].as_f64();
    let context_turns = data["contextTurns"].as_i64().unwrap_or(10) as i32;
    let memory_mode = data["memoryMode"].as_str().unwrap_or("none").to_string();
    let allow_publish = if data["allowPublish"].as_bool().unwrap_or(true) {
        1i32
    } else {
        0
    };
    let notes = data["notes"].as_str().map(|s| s.to_string());

    sqlx::query(
        "INSERT INTO roles (id, name, description, avatar, system_prompt, default_model,
                            temperature, context_turns, memory_mode, allow_publish, notes,
                            created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))",
    )
    .bind(id)
    .bind(&name)
    .bind(&description)
    .bind(&avatar)
    .bind(&system_prompt)
    .bind(&default_model)
    .bind(temperature)
    .bind(context_turns)
    .bind(&memory_mode)
    .bind(allow_publish)
    .bind(&notes)
    .execute(pool)
    .await?;

    save_bindings(pool, id, data).await?;
    Ok(())
}

pub async fn update_role(pool: &SqlitePool, id: &str, data: &Value) -> anyhow::Result<()> {
    let name = data["name"].as_str().unwrap_or("").to_string();
    let description = data["description"].as_str().map(|s| s.to_string());
    let avatar = data["avatar"].as_str().map(|s| s.to_string());
    let system_prompt = data["systemPrompt"].as_str().unwrap_or("").to_string();
    let default_model = data["defaultModel"].as_str().map(|s| s.to_string());
    let temperature = data["temperature"].as_f64();
    let context_turns = data["contextTurns"].as_i64().unwrap_or(10) as i32;
    let memory_mode = data["memoryMode"].as_str().unwrap_or("none").to_string();
    let allow_publish = if data["allowPublish"].as_bool().unwrap_or(true) {
        1i32
    } else {
        0
    };
    let notes = data["notes"].as_str().map(|s| s.to_string());

    sqlx::query(
        "UPDATE roles SET name=?, description=?, avatar=?, system_prompt=?, default_model=?,
                          temperature=?, context_turns=?, memory_mode=?, allow_publish=?,
                          notes=?, updated_at=datetime('now')
         WHERE id=?",
    )
    .bind(&name)
    .bind(&description)
    .bind(&avatar)
    .bind(&system_prompt)
    .bind(&default_model)
    .bind(temperature)
    .bind(context_turns)
    .bind(&memory_mode)
    .bind(allow_publish)
    .bind(&notes)
    .bind(id)
    .execute(pool)
    .await?;

    // 重建关联表
    delete_bindings(pool, id).await?;
    save_bindings(pool, id, data).await?;
    Ok(())
}

pub async fn delete_role(pool: &SqlitePool, id: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM roles WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn activate_role(pool: &SqlitePool, id: &str) -> anyhow::Result<()> {
    // 事务内原子切换：全部置 0 → 目标置 1
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE roles SET is_active = 0, updated_at = datetime('now') WHERE is_active = 1")
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE roles SET is_active = 1, updated_at = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn deactivate_role(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query("UPDATE roles SET is_active = 0, updated_at = datetime('now') WHERE is_active = 1")
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_active_role(pool: &SqlitePool) -> anyhow::Result<Option<Value>> {
    let row = sqlx::query("SELECT id FROM roles WHERE is_active = 1 LIMIT 1")
        .fetch_optional(pool)
        .await?;

    let Some(row) = row else {
        return Ok(None);
    };
    let id: String = row.get("id");
    get_role(pool, &id).await
}

pub async fn get_role_stats(pool: &SqlitePool) -> anyhow::Result<Value> {
    let total: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM roles")
        .fetch_one(pool)
        .await
        .map(|r| r.get::<i64, _>("cnt"))
        .unwrap_or(0);

    let published: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM roles WHERE is_published = 1")
        .fetch_one(pool)
        .await
        .map(|r| r.get::<i64, _>("cnt"))
        .unwrap_or(0);

    let active_name: Option<String> =
        sqlx::query("SELECT name FROM roles WHERE is_active = 1 LIMIT 1")
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .map(|r| r.get("name"));

    Ok(json!({
        "total": total,
        "published": published,
        "activeName": active_name.unwrap_or_else(|| "无".to_string()),
    }))
}

pub async fn publish_role(
    pool: &SqlitePool,
    id: &str,
    category: &str,
    square_desc: &str,
    author_name: &str,
) -> anyhow::Result<()> {
    // 读取角色信息
    let row = sqlx::query("SELECT name, description, avatar FROM roles WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;

    let Some(row) = row else {
        return Err(anyhow::anyhow!("角色不存在"));
    };

    let name: String = row.get("name");
    let description: Option<String> = row.get("description");
    let avatar: Option<String> = row.get("avatar");

    // 写入广场（不含 system_prompt）
    let sq_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT OR REPLACE INTO square_roles (id, origin_id, name, description, avatar, category, author_name, tool_summary, published_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, '{}', datetime('now'), datetime('now'))",
    )
    .bind(&sq_id)
    .bind(id)
    .bind(&name)
    .bind(if square_desc.is_empty() { description.as_deref().unwrap_or("") } else { square_desc })
    .bind(&avatar)
    .bind(category)
    .bind(author_name)
    .execute(pool)
    .await?;

    // 更新角色发布状态
    sqlx::query(
        "UPDATE roles SET is_published=1, square_category=?, square_desc=?,
                          published_at=datetime('now'), updated_at=datetime('now')
         WHERE id=?",
    )
    .bind(category)
    .bind(square_desc)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn unpublish_role(pool: &SqlitePool, id: &str) -> anyhow::Result<()> {
    // 从广场删除
    sqlx::query("DELETE FROM square_roles WHERE origin_id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    // 更新角色状态
    sqlx::query(
        "UPDATE roles SET is_published=0, published_at=NULL, updated_at=datetime('now') WHERE id=?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clone_role(pool: &SqlitePool, src_id: &str, new_id: &str) -> anyhow::Result<Value> {
    sqlx::query(
        "INSERT INTO roles (id, name, description, avatar, system_prompt, default_model,
                            temperature, context_turns, memory_mode, allow_publish, notes,
                            source_type, created_at, updated_at)
         SELECT ?, name || '（副本）', description, avatar, system_prompt, default_model,
                temperature, context_turns, memory_mode, allow_publish, notes,
                'user', datetime('now'), datetime('now')
         FROM roles WHERE id = ?",
    )
    .bind(new_id)
    .bind(src_id)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT INTO role_mcps (role_id, mcp_id, sort_order)
         SELECT ?, mcp_id, sort_order FROM role_mcps WHERE role_id = ?",
    )
    .bind(new_id)
    .bind(src_id)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT INTO role_skills (role_id, skill_id, sort_order)
         SELECT ?, skill_id, sort_order FROM role_skills WHERE role_id = ?",
    )
    .bind(new_id)
    .bind(src_id)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT INTO role_knowledge_bases (role_id, knowledge_base_id, sort_order)
         SELECT ?, knowledge_base_id, sort_order FROM role_knowledge_bases WHERE role_id = ?",
    )
    .bind(new_id)
    .bind(src_id)
    .execute(pool)
    .await?;

    get_role(pool, new_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("克隆失败"))
}

// ─── 广场 ───

pub async fn list_square_roles(
    pool: &SqlitePool,
    search: Option<&str>,
    category: Option<&str>,
    sort_by: Option<&str>,
    page: i64,
    page_size: i64,
) -> anyhow::Result<(Vec<Value>, i64)> {
    let mut conditions: Vec<String> = Vec::new();
    let mut bindings: Vec<String> = Vec::new();

    if let Some(s) = search {
        if !s.is_empty() {
            conditions.push("(name LIKE ? OR description LIKE ?)".to_string());
            bindings.push(format!("%{}%", s));
            bindings.push(format!("%{}%", s));
        }
    }
    if let Some(cat) = category {
        if !cat.is_empty() && cat != "all" {
            conditions.push("category = ?".to_string());
            bindings.push(cat.to_string());
        }
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    let order = match sort_by.unwrap_or("newest") {
        "mostUsed" => "use_count DESC",
        "mostStarred" => "star_count DESC",
        _ => "published_at DESC",
    };

    let offset = (page - 1) * page_size;

    // 总数查询
    let count_sql = format!("SELECT COUNT(*) as cnt FROM square_roles{}", where_clause);
    let mut count_q = sqlx::query(&count_sql);
    for b in &bindings {
        count_q = count_q.bind(b);
    }
    let total: i64 = count_q
        .fetch_one(pool)
        .await
        .map(|r| r.get::<i64, _>("cnt"))
        .unwrap_or(0);

    // 分页查询
    let data_sql = format!(
        "SELECT id, origin_id, name, description, avatar, category, author_name,
                use_count, star_count, tool_summary, published_at
         FROM square_roles{} ORDER BY {} LIMIT ? OFFSET ?",
        where_clause, order
    );
    let mut data_q = sqlx::query(&data_sql);
    for b in &bindings {
        data_q = data_q.bind(b);
    }
    data_q = data_q.bind(page_size).bind(offset);
    let rows = data_q.fetch_all(pool).await?;

    let items = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String, _>("id"),
                "originId": r.get::<Option<String>, _>("origin_id"),
                "name": r.get::<String, _>("name"),
                "description": r.get::<Option<String>, _>("description"),
                "avatar": r.get::<Option<String>, _>("avatar"),
                "category": r.get::<String, _>("category"),
                "authorName": r.get::<Option<String>, _>("author_name"),
                "useCount": r.get::<i64, _>("use_count"),
                "starCount": r.get::<i64, _>("star_count"),
                "toolSummary": r.get::<Option<String>, _>("tool_summary")
                    .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                    .unwrap_or(json!({})),
                "publishedAt": r.get::<String, _>("published_at"),
            })
        })
        .collect();

    Ok((items, total))
}

pub async fn apply_square_role(
    pool: &SqlitePool,
    square_id: &str,
    new_id: &str,
) -> anyhow::Result<Value> {
    let row = sqlx::query(
        "SELECT id, name, description, avatar, category FROM square_roles WHERE id = ?",
    )
    .bind(square_id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Err(anyhow::anyhow!("广场角色不存在"));
    };

    let name: String = row.get("name");
    let description: Option<String> = row.get("description");
    let avatar: Option<String> = row.get("avatar");

    // 创建本地角色副本（system_prompt 为空，用户自行填写）
    sqlx::query(
        "INSERT INTO roles (id, name, description, avatar, system_prompt,
                            source_type, source_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, '', 'square', ?, datetime('now'), datetime('now'))",
    )
    .bind(new_id)
    .bind(&name)
    .bind(&description)
    .bind(&avatar)
    .bind(square_id)
    .execute(pool)
    .await?;

    // 更新广场使用次数
    sqlx::query("UPDATE square_roles SET use_count = use_count + 1 WHERE id = ?")
        .bind(square_id)
        .execute(pool)
        .await?;

    get_role(pool, new_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("应用失败"))
}

pub async fn star_square_role(pool: &SqlitePool, square_id: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE square_roles SET star_count = star_count + 1 WHERE id = ?")
        .bind(square_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ─── 保留原有 assistants 接口（兼容旧命令） ───

pub async fn list(pool: &SqlitePool, _filter: Option<&str>) -> anyhow::Result<Vec<Value>> {
    list_roles(pool, None, None, None).await
}

pub async fn insert(pool: &SqlitePool, id: &str, data: Value) -> anyhow::Result<()> {
    insert_role(pool, id, &data).await
}

pub async fn update(pool: &SqlitePool, id: &str, data: Value) -> anyhow::Result<()> {
    update_role(pool, id, &data).await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> anyhow::Result<()> {
    delete_role(pool, id).await
}

pub async fn toggle(pool: &SqlitePool, id: &str, enabled: bool) -> anyhow::Result<()> {
    if enabled {
        activate_role(pool, id).await
    } else {
        deactivate_role(pool).await
    }
}

// ─── 内部辅助函数 ───

fn role_row_to_json(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String, _>("id"),
        "name": row.get::<String, _>("name"),
        "description": row.get::<Option<String>, _>("description"),
        "avatar": row.get::<Option<String>, _>("avatar"),
        "systemPrompt": row.get::<String, _>("system_prompt"),
        "defaultModel": row.get::<Option<String>, _>("default_model"),
        "temperature": row.get::<Option<f64>, _>("temperature"),
        "contextTurns": row.get::<i64, _>("context_turns"),
        "memoryMode": row.get::<String, _>("memory_mode"),
        "allowPublish": row.get::<i64, _>("allow_publish") == 1,
        "isActive": row.get::<i64, _>("is_active") == 1,
        "isPublished": row.get::<i64, _>("is_published") == 1,
        "sourceType": row.get::<String, _>("source_type"),
        "sourceId": row.get::<Option<String>, _>("source_id"),
        "squareCategory": row.get::<Option<String>, _>("square_category"),
        "squareDesc": row.get::<Option<String>, _>("square_desc"),
        "publishedAt": row.get::<Option<String>, _>("published_at"),
        "notes": row.get::<Option<String>, _>("notes"),
        "createdAt": row.get::<String, _>("created_at"),
        "updatedAt": row.get::<String, _>("updated_at"),
    })
}

async fn save_bindings(pool: &SqlitePool, role_id: &str, data: &Value) -> anyhow::Result<()> {
    if let Some(mcps) = data["mcpIds"].as_array() {
        for (i, mcp_id) in mcps.iter().enumerate() {
            if let Some(mid) = mcp_id.as_str() {
                sqlx::query(
                    "INSERT OR IGNORE INTO role_mcps (role_id, mcp_id, sort_order) VALUES (?, ?, ?)",
                )
                .bind(role_id)
                .bind(mid)
                .bind(i as i64)
                .execute(pool)
                .await?;
            }
        }
    }
    if let Some(skills) = data["skillIds"].as_array() {
        for (i, skill_id) in skills.iter().enumerate() {
            if let Some(sid) = skill_id.as_str() {
                sqlx::query(
                    "INSERT OR IGNORE INTO role_skills (role_id, skill_id, sort_order) VALUES (?, ?, ?)",
                )
                .bind(role_id)
                .bind(sid)
                .bind(i as i64)
                .execute(pool)
                .await?;
            }
        }
    }
    if let Some(kbs) = data["knowledgeBaseIds"].as_array() {
        for (i, kb_id) in kbs.iter().enumerate() {
            if let Some(kid) = kb_id.as_str() {
                sqlx::query(
                    "INSERT OR IGNORE INTO role_knowledge_bases (role_id, knowledge_base_id, sort_order) VALUES (?, ?, ?)",
                )
                .bind(role_id)
                .bind(kid)
                .bind(i as i64)
                .execute(pool)
                .await?;
            }
        }
    }
    Ok(())
}

async fn delete_bindings(pool: &SqlitePool, role_id: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM role_mcps WHERE role_id = ?")
        .bind(role_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM role_skills WHERE role_id = ?")
        .bind(role_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM role_knowledge_bases WHERE role_id = ?")
        .bind(role_id)
        .execute(pool)
        .await?;
    Ok(())
}
