// Stub: FastClaw frontend AskUserQuestion UI not yet wired.

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};

pub struct AskUserTool;

#[async_trait::async_trait]
impl Tool for AskUserTool {
    fn name(&self) -> &str {
        "ask_user"
    }
    fn description(&self) -> &str {
        "Ask the user a question to resolve a decision. Currently not available in FastClaw."
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "question": {"type": "string", "description": "The question to ask"},
                "options": {"type": "array", "items": {"type": "object"}, "description": "Available options"}
            },
            "required": ["question"]
        })
    }
    async fn execute(&self, _args: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        ToolResult::success("AskUser 交互能力尚未接入 FastClaw。// TODO(infra): AskUserQuestion UI")
    }
}
