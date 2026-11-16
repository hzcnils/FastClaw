// FastClaw · Agent Skill 核心
//
// Skill = 文件夹式 Agent Skill（对齐 Anthropic Agent Skills 语义），向下兼容单文件：
//   skills/
//     <slug>/
//       SKILL.md         # frontmatter(双语) + 指令正文；主入口
//       scripts/*.py     # 可选：捆绑脚本
//       <resources...>   # 可选：捆绑资源
//     <slug>.md          # 兼容：纯提示词 skill（无捆绑代码）
//
// 搜索优先级（同名先命中为准）：
//   1. cwd 向上 `.claurst/skills/`
//   2. cwd 向上 `.agents/skills/`
//   3. 全局 `<config_dir>/skills`
//   4. 兼容 `~/.claurst/skills`
//   5. 额外路径（配置）
//   6. git 缓存

pub mod exec;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const SKILL_LANG_EN_MARKER: &str = "<!-- LANG:EN -->";

/// Skill 基础目录：`<config_dir>/FastClaw/skills`。
///
/// 列表（list_local_skills）、工具发现（SkillTool）、以及「本次对话勾了哪些 skill」
/// 的 prompt 注入，三处必须看同一个目录。各写一份迟早会漂，这里是唯一定义。
pub fn skills_base_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("FastClaw")
        .join("skills")
}

// ---------------------------------------------------------------------------
// AgentSkill 模型
// ---------------------------------------------------------------------------

/// 文件夹式 Agent Skill，向下兼容单文件 .md
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentSkill {
    /// Skill 名称（slug，来自 frontmatter name 或文件名 stem）
    pub name: String,
    /// 中文描述（来自 frontmatter description）
    pub description: String,
    /// 英文描述（来自 frontmatter description_en，空时回退 description）
    #[serde(default)]
    pub description_en: String,
    /// 分类标签
    #[serde(default)]
    pub category: String,
    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 作者
    #[serde(default)]
    pub author: String,
    /// 版本
    #[serde(default = "default_version")]
    pub version: String,
    /// 中文正文（<!-- LANG:EN --> 之前的内容）
    pub content_zh: String,
    /// 英文正文（<!-- LANG:EN --> 之后的内容，空时回退 content_zh）
    #[serde(default)]
    pub content_en: String,
    /// 文件夹式 skill 的目录路径（单文件 skill 为 None）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<PathBuf>,
    /// 捆绑脚本列表（scripts/*.py 等，相对 skill 目录）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scripts: Vec<PathBuf>,
    /// SKILL.md 或 .md 文件的绝对路径
    pub source_path: PathBuf,
    /// 创建时间
    #[serde(default)]
    pub created_at: String,
    /// 更新时间
    #[serde(default)]
    pub updated_at: String,
    /// 本 skill 产出的是**实时数据**（天气/汇率/股价…），拿不到数据就不该作答。
    ///
    /// 来自 frontmatter `live-data: true`。**不声明即 false**——保守缺省：
    /// 没声明的 skill 只走"软提示"，不会因为没取到数据而被硬拦。
    #[serde(default)]
    pub live_data: bool,
    /// 本 skill 依赖的工具名（frontmatter `requires-tools: [get_weather]`）。
    ///
    /// 声明之后有两层作用，缺一不可：
    /// 1. 选中该 skill 时**绕过一切开关**把这些工具强制注入（用户明确选了它，
    ///    就不该因为某个全局开关没开而拿不到数据）；
    /// 2. 至少成功调用一次，否则不放行（见 `core::grounding`）。
    ///
    /// 刻意**不叫** `allowed-tools`：FastPlan 那边同名键指的是"允许执行的步骤类型
    /// （python/shell/http）"，语义完全不同，共用键名会让两边的人互相误解。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_tools: Vec<String>,
}

/// frontmatter 的解析结果。
///
/// 用结构体而不是元组：字段已经多到按位置读必然出错，且三个分支各构造一次元组，
/// 漏一个字段编译器不会提醒。结构体有 `Default`，新增字段时旧分支自动安全。
#[derive(Default)]
struct Frontmatter {
    name: Option<String>,
    description: Option<String>,
    description_en: Option<String>,
    category: Option<String>,
    author: Option<String>,
    version: Option<String>,
    enabled: Option<bool>,
    live_data: bool,
    requires_tools: Vec<String>,
    content_zh: String,
    content_en: String,
}

fn default_true() -> bool {
    true
}
fn default_version() -> String {
    "1.0.0".to_string()
}

impl AgentSkill {
    /// 根据语言偏好返回正文内容
    pub fn content_for_lang(&self, lang: Option<&str>) -> &str {
        if lang == Some("en") && !self.content_en.is_empty() {
            &self.content_en
        } else {
            &self.content_zh
        }
    }

    /// 描述回退：description_en 为空时用 description
    pub fn description_for_lang(&self, lang: Option<&str>) -> &str {
        if lang == Some("en") && !self.description_en.is_empty() {
            &self.description_en
        } else {
            &self.description
        }
    }
}

// ---------------------------------------------------------------------------
// Frontmatter 解析
// ---------------------------------------------------------------------------

/// 解析 skill markdown 文件。
/// 返回 None 当文件为空时。
pub fn parse_skill_file(content: &str, path: &Path) -> Option<AgentSkill> {
    let content = content.trim();
    if content.is_empty() {
        return None;
    }

    // 畸形/缺失的 frontmatter —— 全文当作正文
    let parse_plain = |text: &str| Frontmatter {
        content_zh: text.to_string(),
        ..Default::default()
    };

    let fm = if let Some(after_open) = content.strip_prefix("---") {
        if let Some(close_pos) = after_open.find("\n---") {
            let frontmatter = &after_open[..close_pos];
            let rest = after_open[close_pos + 4..].trim_start_matches(['\r', '\n']);

            let mut fm = Frontmatter::default();

            for line in frontmatter.lines() {
                let line = line.trim();
                if let Some(v) = line.strip_prefix("name:") {
                    fm.name = Some(unquote(v));
                } else if let Some(v) = line.strip_prefix("description:") {
                    fm.description = Some(unquote(v));
                } else if let Some(v) = line.strip_prefix("description_en:") {
                    fm.description_en = Some(unquote(v));
                } else if let Some(v) = line.strip_prefix("category:") {
                    fm.category = Some(unquote(v));
                } else if let Some(v) = line.strip_prefix("author:") {
                    fm.author = Some(unquote(v));
                } else if let Some(v) = line.strip_prefix("version:") {
                    fm.version = Some(unquote(v));
                } else if let Some(v) = line.strip_prefix("enabled:") {
                    fm.enabled = Some(v.trim().eq_ignore_ascii_case("true"));
                } else if let Some(v) = line
                    .strip_prefix("live-data:")
                    .or_else(|| line.strip_prefix("live_data:"))
                {
                    // 缺省 false：没写就是"没声明"，走软提示而非硬拦。
                    // 认 yes/on/1：手写 frontmatter 的人不会去查我们只认字面量 true，
                    // 读歪的代价是他以为配好了、其实没配——正是本次事故的同款沉默失败。
                    fm.live_data = parse_bool_lenient(v);
                } else if let Some(v) = line
                    .strip_prefix("requires-tools:")
                    .or_else(|| line.strip_prefix("requires_tools:"))
                {
                    fm.requires_tools = parse_inline_list(v);
                }
            }

            // 双语拆分
            let (content_zh, content_en) = split_bilingual(rest);
            fm.content_zh = content_zh;
            fm.content_en = content_en;
            fm
        } else {
            parse_plain(content)
        }
    } else {
        parse_plain(content)
    };

    let name = fm.name.unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed")
            .to_string()
    });

    if fm.content_zh.is_empty() && name.is_empty() {
        return None;
    }

    Some(AgentSkill {
        name,
        description: fm.description.unwrap_or_else(|| "Custom skill".to_string()),
        description_en: fm.description_en.unwrap_or_default(),
        category: fm.category.unwrap_or_default(),
        enabled: fm.enabled.unwrap_or(true),
        author: fm.author.unwrap_or_default(),
        version: fm.version.unwrap_or_else(|| "1.0.0".to_string()),
        content_zh: fm.content_zh,
        content_en: fm.content_en,
        dir: None,
        scripts: vec![],
        source_path: path.to_path_buf(),
        created_at: String::new(),
        updated_at: String::new(),
        live_data: fm.live_data,
        requires_tools: fm.requires_tools,
    })
}

/// 解析 frontmatter 里的行内列表：`[get_weather]`、`["get_weather", "web_search"]`、
/// 以及不带方括号的 `get_weather, web_search`。
///
/// 写法放宽是有意的：手写 frontmatter 的人不会去查我们支持哪种写法，
/// 多认几种比让人以为"写了但没生效"要好（那正是本次事故的同款失败模式）。
fn parse_inline_list(v: &str) -> Vec<String> {
    let t = v.trim();
    let t = t.strip_prefix('[').unwrap_or(t);
    let t = t.strip_suffix(']').unwrap_or(t);
    t.split(',')
        .map(unquote)
        .filter(|s| !s.is_empty())
        .collect()
}

/// 按 `<!-- LANG:EN -->` 拆分中文/英文正文
fn split_bilingual(body: &str) -> (String, String) {
    if let Some(marker_pos) = body.find(SKILL_LANG_EN_MARKER) {
        let zh = body[..marker_pos].trim().to_string();
        let en = body[marker_pos + SKILL_LANG_EN_MARKER.len()..]
            .trim()
            .to_string();
        (zh, en)
    } else {
        (body.to_string(), String::new())
    }
}

fn unquote(s: &str) -> String {
    s.trim().trim_matches('"').trim_matches('\'').to_string()
}

/// 宽松布尔：`true`/`yes`/`on`/`1`（大小写不敏感）都算真，其余一律假。
///
/// 只用于**本次新增的** `live-data:`。`enabled:` 仍走原来的严格字面量比较——
/// 那边放宽会把运维明确关掉的 skill 悄悄打开，是另一个方向的危险。
fn parse_bool_lenient(s: &str) -> bool {
    let t = unquote(s);
    t.eq_ignore_ascii_case("true")
        || t.eq_ignore_ascii_case("yes")
        || t.eq_ignore_ascii_case("on")
        || t == "1"
}

// ---------------------------------------------------------------------------
// 文件夹式 Skill 解析
// ---------------------------------------------------------------------------

/// 解析文件夹式 skill：读取 `<dir>/SKILL.md`，收集 `scripts/` 下的文件。
pub fn parse_folder_skill(dir: &Path) -> Option<AgentSkill> {
    let skill_md = dir.join("SKILL.md");
    if !skill_md.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&skill_md).ok()?;
    let mut skill = parse_skill_file(&content, &skill_md)?;
    skill.dir = Some(dir.to_path_buf());

    // 收集 scripts/
    let scripts_dir = dir.join("scripts");
    if scripts_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&scripts_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    skill.scripts.push(p);
                }
            }
        }
    }

    // 更新时间从文件系统读取
    if let Ok(meta) = std::fs::metadata(&skill_md) {
        if let Ok(mtime) = meta.modified() {
            use std::time::UNIX_EPOCH;
            let ts = mtime
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            skill.updated_at = chrono::DateTime::from_timestamp(ts as i64, 0)
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string())
                .unwrap_or_default();
        }
    }

    Some(skill)
}

// ---------------------------------------------------------------------------
// 目录扫描（扩展文件夹支持）
// ---------------------------------------------------------------------------

/// 扫描目录中的 skill 文件（单文件 .md + 文件夹 SKILL.md）。
pub fn scan_dir(dir: &Path) -> Vec<AgentSkill> {
    let mut skills = Vec::new();
    if !dir.is_dir() {
        return skills;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(err) => {
            tracing::debug!(dir = %dir.display(), error = %err, "skill_discovery: read_dir failed");
            return skills;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            // 文件夹式 skill（不按 enabled 过滤，交给调用方决定）
            if let Some(skill) = parse_folder_skill(&path) {
                skills.push(skill);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            // 单文件 skill（不按 enabled 过滤，交给调用方决定）
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    if let Some(skill) = parse_skill_file(&content, &path) {
                        skills.push(skill);
                    }
                }
                Err(err) => {
                    tracing::debug!(path = %path.display(), error = %err, "skill_discovery: read failed");
                }
            }
        }
    }

    skills
}

// ---------------------------------------------------------------------------
// 顶层发现
// ---------------------------------------------------------------------------

/// 发现所有 skill。同名先命中为准。
pub fn discover_skills(cwd: &Path, extra_paths: &[PathBuf]) -> HashMap<String, AgentSkill> {
    let mut all: HashMap<String, AgentSkill> = HashMap::new();
    let mut warn_duplicates: Vec<String> = Vec::new();

    let mut add = |skills: Vec<AgentSkill>| {
        for skill in skills {
            if let Some(existing) = all.get(&skill.name) {
                warn_duplicates.push(format!(
                    "Duplicate skill '{}' found at {} (keeping {})",
                    skill.name,
                    skill.source_path.display(),
                    existing.source_path.display()
                ));
            } else {
                all.insert(skill.name.clone(), skill);
            }
        }
    };

    // 1. 项目 skills：cwd 向上
    {
        let mut d: &Path = cwd;
        loop {
            add(scan_dir(&d.join(".claurst").join("skills")));
            add(scan_dir(&d.join(".agents").join("skills")));
            match d.parent() {
                Some(parent) if parent != d => d = parent,
                _ => break,
            }
        }
    }

    // 2. 全局 skills：FastClaw config 目录
    if let Some(config_dir) = dirs::config_dir() {
        add(scan_dir(&config_dir.join("FastClaw").join("skills")));
        // 兼容旧目录
        add(scan_dir(&config_dir.join("claurst").join("skills")));
    }

    // 3. 兼容 ~/.claurst/skills
    if let Some(home) = dirs::home_dir() {
        add(scan_dir(&home.join(".claurst").join("skills")));
    }

    // 4. 额外路径
    for path in extra_paths {
        let p = if path.is_absolute() {
            path.clone()
        } else {
            cwd.join(path)
        };
        add(scan_dir(&p));
    }

    for w in &warn_duplicates {
        tracing::warn!("{}", w);
    }

    all
}

// ---------------------------------------------------------------------------
// Git URL 技能获取
// ---------------------------------------------------------------------------

/// Clone 或复用缓存的 git repo，返回其中发现的 skills。
pub fn fetch_git_skills(url: &str) -> Option<Vec<AgentSkill>> {
    let cache_dir = dirs::cache_dir()?.join("FastClaw").join("skills");

    let repo_name = url.split('/').next_back()?.trim_end_matches(".git");

    if repo_name.is_empty() {
        tracing::warn!(url, "skill_discovery: cannot derive repo name from git URL");
        return None;
    }

    let repo_dir = cache_dir.join(repo_name);

    if !repo_dir.exists() {
        tracing::info!(url, dest = %repo_dir.display(), "skill_discovery: cloning skills repo");

        if let Err(err) = std::fs::create_dir_all(&cache_dir) {
            tracing::warn!(dir = %cache_dir.display(), error = %err, "skill_discovery: could not create cache dir");
            return None;
        }

        let repo_dir_str = repo_dir.to_str()?;
        let status = {
            let mut cmd = std::process::Command::new("git");
            cmd.args(["clone", "--depth=1", url, repo_dir_str]);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0800_0000);
            }
            cmd.status()
        };

        match status {
            Ok(s) if s.success() => {
                tracing::info!(url, "skill_discovery: clone succeeded");
            }
            Ok(s) => {
                tracing::warn!(url, exit_code = ?s.code(), "skill_discovery: git clone failed");
                return None;
            }
            Err(err) => {
                tracing::warn!(url, error = %err, "skill_discovery: could not spawn git");
                return None;
            }
        }
    }

    let mut skills = scan_dir(&repo_dir);
    skills.extend(scan_dir(&repo_dir.join("skills")));
    Some(skills)
}

// ---------------------------------------------------------------------------
// 安全工具
// ---------------------------------------------------------------------------

/// 验证 skill 名称不含路径穿越字符
pub fn validate_skill_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Skill 名称不能为空".to_string());
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(format!("Skill 名称包含非法字符: {}", name));
    }
    if name.len() > 64 {
        return Err(format!("Skill 名称过长（最多 64 字符）: {}", name));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 文件 I/O
// ---------------------------------------------------------------------------

/// 从 AgentSkill 构建 SKILL.md 内容（含双语 frontmatter）
pub fn build_skill_md(skill: &AgentSkill) -> String {
    let mut md = String::new();
    md.push_str("---\n");
    md.push_str(&format!("name: {}\n", skill.name));
    md.push_str(&format!("description: {}\n", skill.description));
    if !skill.description_en.is_empty() {
        md.push_str(&format!("description_en: {}\n", skill.description_en));
    }
    if !skill.category.is_empty() {
        md.push_str(&format!("category: {}\n", skill.category));
    }
    if !skill.author.is_empty() {
        md.push_str(&format!("author: {}\n", skill.author));
    }
    md.push_str(&format!("version: {}\n", skill.version));
    md.push_str(&format!("enabled: {}\n", skill.enabled));
    // 只在声明了才写：缺省即"没声明"，补一行 `live-data: false` 只会让手写
    // SKILL.md 的人误以为这行是必须的。
    // ★ 这两行必须与 parse_skill_file 严格对称——界面上存了却没落盘，
    //   就是"以为配好了其实没生效"，正是这次事故的同款失败模式。
    if skill.live_data {
        md.push_str("live-data: true\n");
    }
    if !skill.requires_tools.is_empty() {
        md.push_str(&format!(
            "requires-tools: [{}]\n",
            skill.requires_tools.join(", ")
        ));
    }
    md.push_str("---\n\n");
    md.push_str(&skill.content_zh);
    if !skill.content_en.is_empty() {
        md.push_str(&format!(
            "\n\n{}\n{}",
            SKILL_LANG_EN_MARKER, skill.content_en
        ));
    }
    md.push('\n');
    md
}

/// 原子写入：先写 .tmp，再 rename（防止写入中断导致文件损坏）
pub fn atomic_write(path: &Path, content: &str) -> Result<(), std::io::Error> {
    let tmp = path.with_extension("md.tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// 将 skill 写入磁盘（自动选择单文件或文件夹格式）
pub fn write_skill_file(skill: &AgentSkill, base_dir: &Path) -> Result<PathBuf, std::io::Error> {
    let content = build_skill_md(skill);

    if let Some(ref dir) = skill.dir {
        // 文件夹式：写 <dir>/SKILL.md
        std::fs::create_dir_all(dir)?;
        let skill_md = dir.join("SKILL.md");
        atomic_write(&skill_md, &content)?;
        Ok(skill_md)
    } else {
        // 单文件：写 <base_dir>/<name>.md
        std::fs::create_dir_all(base_dir)?;
        let file = base_dir.join(format!("{}.md", skill.name));
        atomic_write(&file, &content)?;
        Ok(file)
    }
}

/// 读取 skill 的 .stats.json 使用统计（缺失不报错）
pub fn read_skill_stats() -> HashMap<String, serde_json::Value> {
    let stats_path = skills_stats_path();
    std::fs::read_to_string(&stats_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// 记录 skill 使用统计
pub fn record_skill_usage(skill_name: &str, success: bool) {
    use std::sync::Mutex;
    static STATS_MUTEX: Mutex<()> = Mutex::new(());

    let _lock = match STATS_MUTEX.lock() {
        Ok(l) => l,
        Err(_) => return,
    };

    let stats_path = skills_stats_path();
    if let Some(parent) = stats_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let mut stats: HashMap<String, serde_json::Value> = std::fs::read_to_string(&stats_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let entry = stats
        .entry(skill_name.to_string())
        .or_insert_with(|| serde_json::json!({"usage_count": 0, "success_count": 0}));

    if let Some(obj) = entry.as_object_mut() {
        if let Some(count) = obj.get("usage_count").and_then(|v| v.as_i64()) {
            obj.insert("usage_count".to_string(), serde_json::json!(count + 1));
        }
        if success {
            if let Some(count) = obj.get("success_count").and_then(|v| v.as_i64()) {
                obj.insert("success_count".to_string(), serde_json::json!(count + 1));
            }
        }
    }

    if let Ok(json) = serde_json::to_string_pretty(&stats) {
        let tmp = stats_path.with_extension("json.tmp");
        let _ = std::fs::write(&tmp, &json);
        let _ = std::fs::rename(&tmp, &stats_path);
    }
}

fn skills_stats_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("FastClaw")
        .join("skills")
        .join(".stats.json")
}

// ---------------------------------------------------------------------------
// 模板展开
// ---------------------------------------------------------------------------

/// 获取 skill 模板内容（根据语言选择正文）
pub fn get_skill_template(skill: &AgentSkill, lang: Option<&str>) -> String {
    let body = if lang == Some("en") && !skill.content_en.is_empty() {
        &skill.content_en
    } else {
        &skill.content_zh
    };

    // 对 $ARGUMENTS 占位符，如果没有 args 会保留原样让模型自行处理
    body.to_string()
}

/// 展开 skill 模板，替换 $ARGUMENTS
pub fn expand_skill_template(skill: &AgentSkill, args: Option<&str>, lang: Option<&str>) -> String {
    let template = get_skill_template(skill, lang);
    if let Some(args) = args {
        template.replace("$ARGUMENTS", args)
    } else {
        template.replace("$ARGUMENTS", "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn parse(md: &str) -> AgentSkill {
        parse_skill_file(md, Path::new("weather.md")).expect("应能解析")
    }

    const DECLARED: &str = r#"---
name: weather_search
description: 查询实时天气
live-data: true
requires-tools: [get_weather]
---

请查询 $ARGUMENTS 的天气。
"#;

    #[test]
    fn parses_kebab_case_declaration() {
        let s = parse(DECLARED);
        assert!(s.live_data);
        assert_eq!(s.requires_tools, vec!["get_weather"]);
    }

    #[test]
    fn parses_snake_case_aliases() {
        // 手写 frontmatter 的人不会去查我们认哪种写法，两种都得认
        let s = parse(
            "---\nname: x\nlive_data: true\nrequires_tools: get_weather, web_search\n---\n\n正文\n",
        );
        assert!(s.live_data);
        assert_eq!(s.requires_tools, vec!["get_weather", "web_search"]);
    }

    #[test]
    fn missing_declaration_is_conservative_default() {
        let s = parse("---\nname: x\ndescription: d\n---\n\n正文\n");
        assert!(!s.live_data, "没声明 live-data 时必须是 false");
        assert!(s.requires_tools.is_empty());
    }

    #[test]
    fn malformed_declaration_does_not_panic() {
        // 空列表、只有半边括号、多余的逗号、大小写混杂 —— 都不该炸
        for raw in ["[]", "[", "]", "[,]", "[get_weather,]", "[\"get_weather\"]"] {
            let md = format!("---\nname: x\nrequires-tools: {raw}\n---\n\n正文\n");
            let s = parse(&md);
            for t in &s.requires_tools {
                assert!(!t.is_empty(), "空工具名会污染强制注入名单: {raw:?}");
            }
        }
        // 值畸形时按"没声明"处理，绝不能当成 true —— 读歪了会凭空拦下一整轮回答
        assert!(!parse("---\nname: x\nlive-data: maybe\n---\n\n正文\n").live_data);
        assert!(parse("---\nname: x\nlive-data: yes\n---\n\n正文\n").live_data, "yes 也该认");
    }

    /// ★ 这一条是本次事故的同款防线：界面上配了、写盘时丢了，
    ///   用户以为生效了、其实没有。序列化与解析必须严格对称。
    #[test]
    fn build_skill_md_roundtrips_declaration() {
        let original = parse(DECLARED);
        let md = build_skill_md(&original);
        assert!(md.contains("live-data: true"), "写盘漏了声明:\n{md}");
        assert!(md.contains("requires-tools: [get_weather]"), "写盘漏了声明:\n{md}");

        let reparsed = parse_skill_file(&md, Path::new("weather.md")).expect("回读应能解析");
        assert!(reparsed.live_data, "回读丢了 live_data");
        assert_eq!(reparsed.requires_tools, original.requires_tools);
    }

    #[test]
    fn build_skill_md_omits_undeclared_fields() {
        // 没声明的 skill 不该多出两行噪声——那会让人以为这两项是必填的
        let md = build_skill_md(&parse("---\nname: x\ndescription: d\n---\n\n正文\n"));
        assert!(!md.contains("live-data"), "{md}");
        assert!(!md.contains("requires-tools"), "{md}");
    }

    #[test]
    fn multi_tool_declaration_roundtrips() {
        let s = parse("---\nname: x\nrequires-tools: [get_weather, web_search]\n---\n\n正文\n");
        assert_eq!(s.requires_tools, vec!["get_weather", "web_search"]);
        let back = parse(&build_skill_md(&s));
        assert_eq!(back.requires_tools, s.requires_tools);
    }

    #[test]
    fn expand_replaces_arguments_placeholder() {
        let s = parse("---\nname: x\n---\n\n查 $ARGUMENTS 的天气\n");
        // 注意：parse_skill_file 入口会 trim 掉首尾空白，正文末尾没有换行
        assert_eq!(expand_skill_template(&s, Some("北京"), None), "查 北京 的天气");
        // 不给 args 时占位符消失，而不是原样留在 prompt 里
        assert_eq!(expand_skill_template(&s, None, None), "查  的天气");
    }
}
