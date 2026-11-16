// Stub: FastClaw planning infra not yet wired.

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};

pub struct ExitPlanModeTool;

#[async_trait::async_trait]
impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str {
        "exit_plan_mode"
    }
    fn description(&self) -> &str {
        "Exit plan mode and proceed with implementation. Currently not available in FastClaw."
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "plan": {"type": "string", "description": "Plan content"}
            },
            "required": ["plan"]
        })
    }
    async fn execute(&self, _args: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        ToolResult::success("Plan mode 尚未接入 FastClaw。// TODO(infra): planning")
    }
}
