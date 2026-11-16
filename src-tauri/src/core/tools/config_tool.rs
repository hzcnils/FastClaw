// Stub: FastClaw settings/config not yet accessible from tool context.

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};

pub struct ConfigTool;

#[async_trait::async_trait]
impl Tool for ConfigTool {
    fn name(&self) -> &str {
        "config"
    }
    fn description(&self) -> &str {
        "Read or update configuration settings. Currently not available in FastClaw."
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Write
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "key": {"type": "string", "description": "Config key to read/write"},
                "value": {"description": "Value to set (omit to read)"}
            },
            "required": ["key"]
        })
    }
    async fn execute(&self, _args: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        ToolResult::success("Config 配置管理尚未接入 FastClaw。// TODO(infra): settings access")
    }
}
