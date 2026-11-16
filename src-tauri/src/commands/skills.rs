// vendored-from: FastClaw Phase 2 @ FastClaw-migration-v6.0
//
// Agent Skill CRUD 命令（Phase 2 重写，替代旧 ES commands/skills.rs）
//
// 所有命令操作文件系统（Skill = 文件夹或单文件 .md），
// 不依赖数据库。使用统计写入 ~/.config/FastClaw/skills/.stats.json。

use crate::core::error::AppError;
use crate::core::skills::{
    atomic_write, build_skill_md, discover_skills, expand_skill_template, read_skill_stats,
    record_skill_usage, scan_dir, validate_skill_name, AgentSkill,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;

// ---------------------------------------------------------------------------
// Payload types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateSkillPayload {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub description_en: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub version: String,
    pub content_zh: String,
    #[serde(default)]
    pub content_en: String,
    /// 如果是文件夹式 skill，指定目录名（默认与 name 相同）
    #[serde(default)]
    pub folder: Option<String>,
    /// 产出是数据型（天气、股价、库存…）→ 没有真实工具证据时硬拒
    #[serde(default)]
    pub live_data: bool,
    /// 声明必须成功调用的工具名，缺失则不放行
    #[serde(default)]
    pub requires_tools: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSkillPayload {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub description_en: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub content_zh: Option<String>,
    #[serde(default)]
    pub content_en: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    /// 语义同 CreateSkillPayload：只有显式传了才覆盖，不传保持原值
    #[serde(default)]
    pub live_data: Option<bool>,
    #[serde(default)]
    pub requires_tools: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct SkillListItem {
    pub name: String,
    pub description: String,
    pub description_en: String,
    pub category: String,
    pub enabled: bool,
    pub author: String,
    pub version: String,
    pub is_folder: bool,
    pub scripts_count: usize,
    pub source_path: String,
    pub created_at: String,
    pub updated_at: String,
    /// 数据型 skill 声明（list 里带上，详情表单才能回显已有声明）
    #[serde(default)]
    pub live_data: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_tools: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_count: Option<i64>,
}

impl From<&AgentSkill> for SkillListItem {
    fn from(s: &AgentSkill) -> Self {
        let stats = read_skill_stats();
        let usage_count = stats
            .get(&s.name)
            .and_then(|v| v.get("usage_count"))
            .and_then(|v| v.as_i64());

        Self {
            name: s.name.clone(),
            description: s.description.clone(),
            description_en: s.description_en.clone(),
            category: s.category.clone(),
            enabled: s.enabled,
            author: s.author.clone(),
            version: s.version.clone(),
            is_folder: s.dir.is_some(),
            scripts_count: s.scripts.len(),
            source_path: s.source_path.display().to_string(),
            created_at: s.created_at.clone(),
            updated_at: s.updated_at.clone(),
            live_data: s.live_data,
            requires_tools: s.requires_tools.clone(),
            usage_count,
        }
    }
}

// ---------------------------------------------------------------------------
// 获取 skill 基础目录
// ---------------------------------------------------------------------------

fn skills_base_dir() -> PathBuf {
    crate::core::skills::skills_base_dir()
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

#[tauri::command]
// include_disabled: 为 true 时连同已禁用的 skill 一起返回（供「工具」弹层展示开关）
pub async fn list_local_skills(
    category: Option<String>,
    include_disabled: Option<bool>,
) -> Result<Vec<SkillListItem>, AppError> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let base = skills_base_dir();

    let discovered = discover_skills(&cwd, &[base.clone()]);

    let stats = read_skill_stats();

    let mut items: Vec<SkillListItem> = discovered
        .values()
        .filter(|s| include_disabled.unwrap_or(false) || s.enabled)
        .filter(|s| {
            if let Some(ref cat) = category {
                s.category == *cat
            } else {
                true
            }
        })
        .map(|s| {
            let mut item = SkillListItem::from(s);
            item.usage_count = stats
                .get(&s.name)
                .and_then(|v| v.get("usage_count"))
                .and_then(|v| v.as_i64());
            item
        })
        .collect();

    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(items)
}

#[tauri::command]
pub async fn get_local_skill(name: String) -> Result<AgentSkill, AppError> {
    validate_skill_name(&name).map_err(AppError::InvalidInput)?;

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let base = skills_base_dir();
    let discovered = discover_skills(&cwd, &[base]);

    discovered
        .get(&name)
        .cloned()
        .ok_or_else(|| AppError::NotFound(format!("Skill 不存在: {}", name)))
}

#[tauri::command]
pub async fn create_local_skill(payload: CreateSkillPayload) -> Result<AgentSkill, AppError> {
    validate_skill_name(&payload.name).map_err(AppError::InvalidInput)?;

    let base = skills_base_dir();
    std::fs::create_dir_all(&base).map_err(|e| AppError::Io(e))?;

    // 检查重名
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let existing = discover_skills(&cwd, &[base.clone()]);
    if existing.contains_key(&payload.name) {
        return Err(AppError::Business(format!(
            "Skill 已存在: {}",
            payload.name
        )));
    }

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    let skill = if let Some(ref folder_name) = payload.folder {
        // 文件夹式 skill
        let folder = base.join(folder_name);
        std::fs::create_dir_all(&folder).map_err(|e| AppError::Io(e))?;

        let agent = AgentSkill {
            name: payload.name.clone(),
            description: payload.description.clone(),
            description_en: payload.description_en.clone(),
            category: payload.category.clone(),
            enabled: true,
            author: payload.author.clone(),
            version: if payload.version.is_empty() {
                "1.0.0".to_string()
            } else {
                payload.version.clone()
            },
            content_zh: payload.content_zh.clone(),
            content_en: payload.content_en.clone(),
            live_data: payload.live_data,
            requires_tools: payload.requires_tools.clone(),
            dir: Some(folder.clone()),
            scripts: vec![],
            source_path: folder.join("SKILL.md"),
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        let md = build_skill_md(&agent);
        let skill_md = folder.join("SKILL.md");
        atomic_write(&skill_md, &md).map_err(|e| AppError::Io(e))?;

        agent
    } else {
        // 单文件 skill
        let file = base.join(format!("{}.md", payload.name));
        let agent = AgentSkill {
            name: payload.name.clone(),
            description: payload.description.clone(),
            description_en: payload.description_en.clone(),
            category: payload.category.clone(),
            enabled: true,
            author: payload.author.clone(),
            version: if payload.version.is_empty() {
                "1.0.0".to_string()
            } else {
                payload.version.clone()
            },
            content_zh: payload.content_zh.clone(),
            content_en: payload.content_en.clone(),
            live_data: payload.live_data,
            requires_tools: payload.requires_tools.clone(),
            dir: None,
            scripts: vec![],
            source_path: file.clone(),
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        let md = build_skill_md(&agent);
        atomic_write(&file, &md).map_err(|e| AppError::Io(e))?;

        agent
    };

    info!("✅ 创建 Skill: {}", payload.name);
    Ok(skill)
}

#[tauri::command]
pub async fn update_local_skill(
    name: String,
    payload: UpdateSkillPayload,
) -> Result<AgentSkill, AppError> {
    validate_skill_name(&name).map_err(AppError::InvalidInput)?;

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let base = skills_base_dir();
    let discovered = discover_skills(&cwd, &[base.clone()]);

    let mut skill = discovered
        .get(&name)
        .cloned()
        .ok_or_else(|| AppError::NotFound(format!("Skill 不存在: {}", name)))?;

    // 只更新提供的字段
    if let Some(v) = payload.description {
        skill.description = v;
    }
    if let Some(v) = payload.description_en {
        skill.description_en = v;
    }
    if let Some(v) = payload.category {
        skill.category = v;
    }
    if let Some(v) = payload.author {
        skill.author = v;
    }
    if let Some(v) = payload.version {
        skill.version = v;
    }
    if let Some(v) = payload.content_zh {
        skill.content_zh = v;
    }
    if let Some(v) = payload.content_en {
        skill.content_en = v;
    }
    if let Some(v) = payload.enabled {
        skill.enabled = v;
    }
    if let Some(v) = payload.live_data {
        skill.live_data = v;
    }
    if let Some(v) = payload.requires_tools {
        skill.requires_tools = v;
    }

    skill.updated_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    // 写回文件
    let md = build_skill_md(&skill);
    atomic_write(&skill.source_path, &md).map_err(|e| AppError::Io(e))?;

    info!("✅ 更新 Skill: {}", name);
    Ok(skill)
}

#[tauri::command]
pub async fn delete_local_skill(name: String) -> Result<(), AppError> {
    validate_skill_name(&name).map_err(AppError::InvalidInput)?;

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let base = skills_base_dir();
    let discovered = discover_skills(&cwd, &[base.clone()]);

    let skill = discovered
        .get(&name)
        .cloned()
        .ok_or_else(|| AppError::NotFound(format!("Skill 不存在: {}", name)))?;

    // 删除文件或文件夹
    if let Some(ref dir) = skill.dir {
        std::fs::remove_dir_all(dir).map_err(|e| AppError::Io(e))?;
    } else {
        std::fs::remove_file(&skill.source_path).map_err(|e| AppError::Io(e))?;
    }

    info!("🗑️ 删除 Skill: {}", name);
    Ok(())
}

#[tauri::command]
pub async fn toggle_local_skill(name: String, enabled: bool) -> Result<AgentSkill, AppError> {
    update_local_skill(
        name.clone(),
        UpdateSkillPayload {
            description: None,
            description_en: None,
            category: None,
            author: None,
            version: None,
            content_zh: None,
            content_en: None,
            enabled: Some(enabled),
            live_data: None,
            requires_tools: None,
        },
    )
    .await
}

#[tauri::command]
pub async fn import_skills_from_folder(dir: String) -> Result<Vec<AgentSkill>, AppError> {
    let path = PathBuf::from(&dir);
    if !path.is_dir() {
        return Err(AppError::InvalidInput(format!("目录不存在: {}", dir)));
    }

    let base = skills_base_dir();
    std::fs::create_dir_all(&base).map_err(|e| AppError::Io(e))?;

    let skills = scan_dir(&path);
    let mut imported = Vec::new();

    for skill in &skills {
        let md = build_skill_md(skill);
        let dest = base.join(format!("{}.md", skill.name));
        atomic_write(&dest, &md).map_err(|e| AppError::Io(e))?;
        imported.push(skill.clone());
        info!("📥 导入 Skill: {}", skill.name);
    }

    Ok(imported)
}

#[tauri::command]
pub async fn import_skills_from_github(url: String) -> Result<Vec<AgentSkill>, AppError> {
    let skills = crate::core::skills::fetch_git_skills(&url)
        .ok_or_else(|| AppError::Network(format!("无法从 GitHub 获取 skills: {}", url)))?;

    let base = skills_base_dir();
    std::fs::create_dir_all(&base).map_err(|e| AppError::Io(e))?;

    for skill in &skills {
        let md = build_skill_md(skill);
        let dest = base.join(format!("{}.md", skill.name));
        atomic_write(&dest, &md).map_err(|e| AppError::Io(e))?;
    }

    info!("📥 从 GitHub 导入 {} 个 Skill: {}", skills.len(), url);
    Ok(skills)
}

#[tauri::command]
pub async fn get_skill_template(name: String, lang: Option<String>) -> Result<String, AppError> {
    validate_skill_name(&name).map_err(AppError::InvalidInput)?;

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let base = skills_base_dir();
    let discovered = discover_skills(&cwd, &[base]);

    let skill = discovered
        .get(&name)
        .ok_or_else(|| AppError::NotFound(format!("Skill 不存在: {}", name)))?;

    Ok(crate::core::skills::get_skill_template(
        skill,
        lang.as_deref(),
    ))
}

#[tauri::command]
pub async fn expand_skill(
    name: String,
    args: Option<String>,
    lang: Option<String>,
) -> Result<String, AppError> {
    validate_skill_name(&name).map_err(AppError::InvalidInput)?;

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let base = skills_base_dir();
    let discovered = discover_skills(&cwd, &[base]);

    let skill = discovered
        .get(&name)
        .ok_or_else(|| AppError::NotFound(format!("Skill 不存在: {}", name)))?;

    let expanded = expand_skill_template(skill, args.as_deref(), lang.as_deref());

    // 记录使用统计
    record_skill_usage(&name, true);

    Ok(expanded)
}
