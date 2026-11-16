// Stub: FastClaw planning infra not yet wired.

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};

pub struct EnterPlanModeTool;

#[async_trait::async_trait]
impl Tool for EnterPlanModeTool {
    fn name(&self) -> &str {
        "enter_plan_mode"
    }
    fn description(&self) -> &str {
        "Enter plan mode to design an implementation approach before writing code. \
         Currently not available in FastClaw."
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object", "properties": {}})
    }
    async fn execute(&self, _args: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        ToolResult::success("Plan mode 尚未接入 FastClaw。// TODO(infra): planning")
    }
}
