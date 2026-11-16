//! 会话事件类型（AcpEvent / SessionEvent）
//!
//! 桌面端与规划链路共用的事件载荷定义。

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum AcpEvent {
    /// LLM 逐 token 流式输出
    #[serde(rename = "stream_delta")]
    StreamDelta {
        role: String,
        text: String,
        #[serde(default)]
        sequence: Option<u64>,
    },

    /// 完整文本消息
    #[serde(rename = "text")]
    Text {
        role: String,
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// 桌面气泡回显用（含 preview_url）。出站到手机时会剥掉，避免把原图再传回 App。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attachments: Option<Value>,
    },

    /// 思考过程
    #[serde(rename = "thinking")]
    Thinking { text: String },

    /// 工具调用开始
    #[serde(rename = "tool_use")]
    ToolUse {
        #[serde(rename = "toolId")]
        tool_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        #[serde(rename = "toolInput")]
        tool_input: Value,
        #[serde(default, rename = "parentToolUseId")]
        parent_tool_use_id: Option<String>,
        #[serde(default, rename = "agentLabel")]
        agent_label: Option<String>,
    },

    /// 工具执行结果
    #[serde(rename = "tool_result")]
    ToolResult {
        #[serde(rename = "toolId")]
        tool_id: String,
        #[serde(rename = "toolResult")]
        tool_result: Value,
        #[serde(default, rename = "parentToolUseId")]
        parent_tool_use_id: Option<String>,
        #[serde(default, rename = "agentLabel")]
        agent_label: Option<String>,
    },

    /// 权限请求（移动端审批）
    #[serde(rename = "permission_request")]
    PermissionRequest {
        #[serde(rename = "requestId")]
        request_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        input: Value,
        #[serde(default)]
        context: Option<Value>,
    },

    /// 权限请求已取消
    #[serde(rename = "permission_cancelled")]
    PermissionCancelled {
        #[serde(rename = "requestId")]
        request_id: String,
        reason: String,
    },

    /// 状态更新
    #[serde(rename = "status")]
    Status {
        text: String,
        #[serde(default, rename = "tokenBudget")]
        token_budget: Option<Value>,
        #[serde(default)]
        tokens: Option<u64>,
        #[serde(default, rename = "canInterrupt")]
        can_interrupt: Option<bool>,
    },

    /// 本轮结束
    #[serde(rename = "complete")]
    Complete {
        #[serde(rename = "exitCode")]
        exit_code: i32,
        #[serde(default, rename = "isNewSession")]
        is_new_session: Option<bool>,
    },

    /// 错误
    #[serde(rename = "error")]
    Error { content: String },

    /// 会话通知（持久化）
    #[serde(rename = "notice")]
    Notice { text: String },

    /// 会话元数据变更（创建 / 重命名 / 删除 / 截断），用于桌面与手机列表同步
    #[serde(rename = "session_updated")]
    SessionUpdated {
        action: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "taskPlan")]
        task_plan: Option<bool>,
        /// 从普通对话点开任务规划后，新规划会话的 id。
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "successorSessionId")]
        successor_session_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "executionMode")]
        execution_mode: Option<String>,
    },

    /// FastPlan 规划过程（原样转发 sidecar WS 事件）
    #[serde(rename = "plan")]
    Plan { event: Value },

    /// Token 用量
    #[serde(rename = "usage")]
    Usage {
        #[serde(rename = "inputTokens")]
        input_tokens: u64,
        #[serde(rename = "outputTokens")]
        output_tokens: u64,
        #[serde(default, rename = "costUSD")]
        cost_usd: Option<f64>,
        #[serde(default, rename = "cacheTokens")]
        cache_tokens: Option<u64>,
        #[serde(default, rename = "cumulativeInputTokens")]
        cumulative_input_tokens: Option<u64>,
        #[serde(default, rename = "cumulativeOutputTokens")]
        cumulative_output_tokens: Option<u64>,
    },
}

// ---------------------------------------------------------------------------
// 会话事件包装
// ---------------------------------------------------------------------------

/// 携带 session_id 上下文的事件
#[derive(Debug, Clone)]
pub struct SessionEvent {
    pub session_id: String,
    pub event: AcpEvent,
    /// 事件来源。desktop 事件需要上行同步到移动端，但不应再回灌桌面 ChatStore。
    pub origin: &'static str,
}

// ---------------------------------------------------------------------------
// 权限决策（移动端 → 桌面）
// ---------------------------------------------------------------------------

