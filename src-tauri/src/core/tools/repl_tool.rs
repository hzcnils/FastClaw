// Stub: REPL execution not yet wired (needs python3/node subprocess infra).

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::json;

pub struct ReplTool;

#[async_trait::async_trait]
impl Tool for ReplTool {
    fn name(&self) -> &str {
        "repl"
    }
    fn description(&self) -> &str {
        "Execute code in a persistent REPL session. Currently not available in FastClaw."
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Execute
    }
    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "language": {"type": "string", "enum": ["python", "javascript", "bash"]},
                "code": {"type": "string", "description": "Code to execute"}
            },
            "required": ["language", "code"]
        })
    }
    async fn execute(&self, _a: serde_json::Value, _c: &ToolContext) -> ToolResult {
        ToolResult::success("REPL 执行尚未接入 FastClaw。// TODO(infra): python3/node subprocess")
    }
}
