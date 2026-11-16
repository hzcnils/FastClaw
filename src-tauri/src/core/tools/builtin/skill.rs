// FastClaw · SkillTool — makes Agent Skills callable by the LLM at runtime
//
// Registered as a builtin tool named "skill". The LLM can:
//   skill({ name: "list" }) — list all enabled local skills
//   skill({ name: "<slug>", arguments: "..." }) — expand a skill template
//
// Reuses Phase 2's `core::skills` discovery + template expansion.

use crate::core::skills::{discover_skills, expand_skill_template, read_skill_stats};
use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};
use std::path::PathBuf;

pub struct SkillTool {
    skills_dir: PathBuf,
}

impl SkillTool {
    pub fn new() -> Self {
        Self {
            skills_dir: crate::core::skills::skills_base_dir(),
        }
    }

    /// allow: 本次对话的 Skill 白名单。
    ///   None      = 不限制，沿用 skill 自身的 enabled 状态
    ///   Some(list)= 只允许名单内的（空 list = 一个都不允许）
    fn discover(&self, allow: Option<&[String]>) -> Vec<crate::core::skills::AgentSkill> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let all = discover_skills(&cwd, &[self.skills_dir.clone()]);
        all.into_values()
            // 工具调用只需已启用的 skill
            .filter(|s| s.enabled)
            .filter(|s| match allow {
                None => true,
                Some(list) => list.iter().any(|n| n == &s.name),
            })
            .collect::<Vec<_>>()
    }
}

#[async_trait::async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        "skill"
    }

    fn description(&self) -> &str {
        "Call an Agent Skill by name. Skills are domain-specific instruction templates.\n\
         - Use `name: \"list\"` to see all enabled skills.\n\
         - Use `name: \"<skill-name>\"` with optional `arguments` to expand a skill template.\n\
         The expanded skill content will guide you in performing the requested task."
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Skill name (slug), or 'list' to list all enabled skills."
                },
                "arguments": {
                    "type": "string",
                    "description": "Optional arguments to pass to the skill template ($ARGUMENTS replacement)."
                }
            },
            "required": ["name"]
        })
    }

    async fn execute(&self, args: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let name = match args.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => return ToolResult::err("Missing required parameter: name"),
        };
        let allow = _ctx.enabled_skills.as_deref();

        // "list" command
        if name == "list" {
            let skills = self.discover(allow);
            if skills.is_empty() {
                return ToolResult::ok(
                    "No enabled skills found. Create skills in the Skills Management page.",
                );
            }

            let stats = read_skill_stats();
            let listing: Vec<String> = skills
                .iter()
                .map(|s| {
                    let usage = stats
                        .get(&s.name)
                        .and_then(|v| v.get("usage_count"))
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    format!(
                        "  • {} [{}] — {} (used {}×)",
                        s.name, s.category, s.description, usage
                    )
                })
                .collect();

            return ToolResult::ok(format!(
                "Enabled skills ({} total):\n{}",
                listing.len(),
                listing.join("\n")
            ));
        }

        // Lookup and expand a specific skill
        let skills = self.discover(allow);
        let skill = match skills.iter().find(|s| s.name == name) {
            Some(s) => s,
            None => {
                return ToolResult::err(format!(
                    "Skill '{}' not found. Use skill(name: \"list\") to see available skills.",
                    name
                ));
            }
        };

        let arguments = args.get("arguments").and_then(|v| v.as_str()).unwrap_or("");

        let expanded = expand_skill_template(
            skill,
            if arguments.is_empty() {
                None
            } else {
                Some(arguments)
            },
            None, // lang: None = use Chinese content by default
        );

        ToolResult::ok(format!(
            "# Skill: {}\n\n{}\n\n---\n*Follow the instructions above to complete the task.*",
            skill.name, expanded
        ))
    }
}
