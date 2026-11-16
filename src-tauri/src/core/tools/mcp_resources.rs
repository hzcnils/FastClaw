// Stub: MCP resource listing not yet wired.

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::json;

pub struct ListMcpResourcesTool;
pub struct ReadMcpResourceTool;

#[async_trait::async_trait]
impl Tool for ListMcpResourcesTool {
    fn name(&self) -> &str {
        "list_mcp_resources"
    }
    fn description(&self) -> &str {
        "List resources from an MCP server. Currently not available."
    }
    fn input_schema(&self) -> serde_json::Value {
        json!({"type":"object","properties":{"server_id":{"type":"string"}},"required":["server_id"]})
    }
    async fn execute(&self, _a: serde_json::Value, _c: &ToolContext) -> ToolResult {
        ToolResult::success("MCP resource listing not yet wired. // TODO(infra)")
    }
}

#[async_trait::async_trait]
impl Tool for ReadMcpResourceTool {
    fn name(&self) -> &str {
        "read_mcp_resource"
    }
    fn description(&self) -> &str {
        "Read an MCP resource. Currently not available."
    }
    fn input_schema(&self) -> serde_json::Value {
        json!({"type":"object","properties":{"server_id":{"type":"string"},"uri":{"type":"string"}},"required":["server_id","uri"]})
    }
    async fn execute(&self, _a: serde_json::Value, _c: &ToolContext) -> ToolResult {
        ToolResult::success("MCP resource reading not yet wired. // TODO(infra)")
    }
}
