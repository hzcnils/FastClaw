// Stub: FastClaw background task system not yet wired.

use crate::core::tools::_support::constants::{
    TOOL_NAME_TASK_CREATE, TOOL_NAME_TASK_GET, TOOL_NAME_TASK_LIST, TOOL_NAME_TASK_OUTPUT,
    TOOL_NAME_TASK_STOP, TOOL_NAME_TASK_UPDATE,
};
use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};

fn task_schema(verb: &str, desc: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "task_id": {"type": "string", "description": desc}
        },
        "required": if verb == "list" || verb == "create" { &[] as &[&str] } else { &["task_id"] as &[&str] }
    })
}

macro_rules! task_stub {
    ($name:ident, $tool_name:expr, $desc:expr) => {
        pub struct $name;
        #[async_trait::async_trait]
        impl Tool for $name {
            fn name(&self) -> &str { $tool_name }
            fn description(&self) -> &str { const { $desc } }
            fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }
            fn input_schema(&self) -> serde_json::Value {
                serde_json::json!({"type": "object", "properties": {}})
            }
            async fn execute(&self, _args: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
                ToolResult::success("后台任务系统尚未接入 FastClaw。// TODO(infra): background tasks")
            }
        }
    };
}

task_stub!(
    TaskCreateTool,
    TOOL_NAME_TASK_CREATE,
    "Create a new background task"
);
task_stub!(TaskGetTool, TOOL_NAME_TASK_GET, "Get task details by ID");
task_stub!(TaskListTool, TOOL_NAME_TASK_LIST, "List all tasks");
task_stub!(TaskOutputTool, TOOL_NAME_TASK_OUTPUT, "Get task output");
task_stub!(TaskStopTool, TOOL_NAME_TASK_STOP, "Stop a running task");
task_stub!(TaskUpdateTool, TOOL_NAME_TASK_UPDATE, "Update task status");
