// FastClaw · load_tool —— 渐进式工具披露的「阶段二」入口
//
// 模型在 system prompt 的「可按需加载的工具」清单里看到某个工具的名称+摘要后，
// 调用本工具来请求它的完整参数定义。加载后，该工具的 JSON Schema 会在
// **后续每一轮**请求中被注入 tools 数组，模型就能正常调用它了。
//
// 为什么需要这个而不是直接全量注入：
//   40 个工具的完整 schema 约 8000-9000 token/轮，6 轮就是 5 万 token 的纯开销。
//   而"名称 + 一行摘要"的清单只要约 600 token，且只需注入一次。
//
// 注意：本工具只负责"登记"，真正的注入发生在 chat.rs 的工具收集阶段
// （见 disclosure::get_loaded 的调用点）。

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct LoadToolTool;

#[derive(Debug, Deserialize)]
struct LoadToolInput {
    /// 要加载的工具名，可传多个
    names: Vec<String>,
}

#[async_trait]
impl Tool for LoadToolTool {
    fn name(&self) -> &str {
        "load_tool"
    }

    fn description(&self) -> &str {
        "加载指定工具的完整参数定义，之后你才能调用它们。\n\
         用法：从 system prompt 的「可按需加载的工具」清单里挑出确实需要的，\
         把名称传进 names（可一次传多个，建议一次传齐以免多轮往返）。\n\
         加载后本轮不能立即调用该工具，需要等下一轮 —— 所以请在明确知道要用什么时才调用。\n\
         不要为「看看有什么」而加载；不确定就不要加载。"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::None
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "names": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "要加载的工具名列表，例如 [\"cron\", \"monitor\"]"
                }
            },
            "required": ["names"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let params: LoadToolInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => {
                return ToolResult::err(format!(
                    "load_tool 参数错误：需要 {{\"names\": [\"工具名\", ...]}}。详情：{e}"
                ))
            }
        };

        if params.names.is_empty() {
            return ToolResult::err("names 不能为空。请指定至少一个要加载的工具名。".to_string());
        }

        // ToolContext 里承载会话 id 的字段是 session_id
        // （chat.rs 的 execute_tool_with 已改为传入真实 conversation_id）
        let conv_id = if ctx.session_id.trim().is_empty() {
            "default".to_string()
        } else {
            ctx.session_id.clone()
        };

        crate::core::tools::disclosure::mark_loaded(&conv_id, &params.names);

        ToolResult::ok(format!(
            "已请求加载以下工具的完整定义：{}\n\
             它们的参数定义将在**下一轮**对话中可用。\n\
             请在下一轮直接调用它们，不要重复调用 load_tool。",
            params.names.join(", ")
        ))
    }
}
