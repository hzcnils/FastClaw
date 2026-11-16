// FastClaw · run_skill — delegate skill script execution to FastPlan sandbox
//
// Registered as "run_skill". The LLM calls this when a skill has executable steps
// (Python/shell/http/node/copy_file) that need sandbox isolation.
// Pure-prompt skills without scripts continue to use the "skill" tool for template expansion.

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};

pub struct RunSkillTool {
    /// FastPlan base URL (defaults to the Tauri proxy at 127.0.0.1:5179/FastPlan).
    pub fastplan_base: String,
}

impl RunSkillTool {
    pub fn new() -> Self {
        Self {
            // Default: Tauri HTTP proxy → FastPlan (matches config/index.js getFastPlanUrl)
            fastplan_base: "http://127.0.0.1:5179/FastPlan".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl Tool for RunSkillTool {
    fn name(&self) -> &str {
        "run_skill"
    }

    fn description(&self) -> &str {
        "Execute a skill's bundled scripts in FastPlan's secure sandbox.\n\
         Use this when a skill has executable steps (Python/shell/Node.js code).\n\
         For pure-instruction skills, use the 'skill' tool instead.\n\
         Parameters:\n\
         - skill_id: the skill slug (e.g. \"xlsx\", \"data-analysis\")\n\
         - input: optional JSON object passed to the skill executor\n\
         Returns the execution result with any generated output files."
    }

    fn permission_level(&self) -> PermissionLevel {
        // Real isolation is in FastPlan; FastClaw only proxies the request.
        PermissionLevel::ReadOnly
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "skill_id": {
                    "type": "string",
                    "description": "Skill slug / ID to execute (e.g. 'xlsx')."
                },
                "input": {
                    "type": "object",
                    "description": "Optional input data passed to the skill executor."
                }
            },
            "required": ["skill_id"]
        })
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let skill_id = match args.get("skill_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => return ToolResult::err("Missing required parameter: skill_id"),
        };

        // ★「本次对话」的 Skill 白名单，与 SkillTool（模板展开）同一套语义。
        //   两边都得拦：只拦模板展开的话，关掉开关仍能靠 run_skill 跑这个 skill 的脚本。
        //   None = 不限制（老路径行为不变）；Some(list) = 只放行名单内的（空 list = 全拦）。
        if let Some(list) = ctx.enabled_skills.as_deref() {
            if !list.iter().any(|n| n == skill_id) {
                return ToolResult::err(format!(
                    "Skill '{}' is not enabled for this conversation. Turn it on in the composer's Tools popover.",
                    skill_id
                ));
            }
        }

        let input = args.get("input").cloned().unwrap_or(serde_json::json!({}));

        match crate::core::skills::exec::execute_skill_via_fastplan(
            &self.fastplan_base,
            skill_id,
            input,
            Some(ctx.session_id.clone()),
            ctx.lang.clone(),
        )
        .await
        {
            Ok(out) => {
                if !out.success {
                    let msg = out
                        .message
                        .unwrap_or_else(|| "Skill 执行失败（业务错误）".to_string());
                    return ToolResult::err(msg);
                }

                let mut content = String::new();
                if let Some(ref msg) = out.message {
                    content.push_str(msg);
                }
                if let Some(ref result) = out.result {
                    if !content.is_empty() {
                        content.push('\n');
                    }
                    content.push_str(&format!(
                        "Result: {}",
                        serde_json::to_string_pretty(result).unwrap_or_default()
                    ));
                }
                if !out.data.is_empty() {
                    let files: Vec<&str> = out.data.iter().map(|d| d.name.as_str()).collect();
                    if !content.is_empty() {
                        content.push('\n');
                    }
                    content.push_str(&format!("Generated files: {}", files.join(", ")));
                }

                ToolResult {
                    content: if content.is_empty() {
                        "Skill executed successfully.".to_string()
                    } else {
                        content
                    },
                    is_error: false,
                    metadata: Some(out.metadata),
                }
            }
            Err(e) => ToolResult::err(format!("run_skill 失败: {e}")),
        }
    }
}
