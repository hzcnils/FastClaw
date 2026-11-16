// Stub: FastClaw goal/session management not yet wired.

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};

pub struct GoalCompleteTool;

#[async_trait::async_trait]
impl Tool for GoalCompleteTool {
    fn name(&self) -> &str {
        "goal_complete"
    }
    fn description(&self) -> &str {
        "Mark the current goal as completed. Currently not available in FastClaw."
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "summary": {"type": "string", "description": "Summary of what was accomplished"}
            },
            "required": ["summary"]
        })
    }
    async fn execute(&self, _args: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        ToolResult::success("Goal 管理尚未接入 FastClaw。// TODO(infra): session management")
    }
}
