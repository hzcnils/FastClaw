// SendMessageTool: send a message to another agent or broadcast to all.
//
// Uses an in-process DashMap inbox for inter-agent messaging within
// a single FastClaw process.

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// In-process inbox
// ---------------------------------------------------------------------------

/// A single message in the inbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub from: String,
    pub to: String,
    pub content: String,
    pub timestamp: u64,
}

/// Global inbox: recipient_id → queued messages.
static INBOX: std::sync::LazyLock<Mutex<HashMap<String, Vec<AgentMessage>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Remove and return all messages queued for `recipient`.
pub fn drain_inbox(recipient: &str) -> Vec<AgentMessage> {
    INBOX
        .lock()
        .ok()
        .and_then(|mut m| m.remove(recipient))
        .unwrap_or_default()
}

/// Read (without removing) all messages queued for `recipient`.
pub fn peek_inbox(recipient: &str) -> Vec<AgentMessage> {
    INBOX
        .lock()
        .ok()
        .and_then(|m| m.get(recipient).cloned())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Tool
// ---------------------------------------------------------------------------

pub struct SendMessageTool;

#[derive(Debug, Deserialize)]
struct SendMessageInput {
    /// Recipient name, or "*" for broadcast.
    to: String,
    /// Message body.
    message: String,
    /// Short preview text shown in the UI.
    #[serde(default)]
    summary: Option<String>,
}

#[async_trait]
impl Tool for SendMessageTool {
    fn name(&self) -> &str {
        "SendMessage"
    }

    fn description(&self) -> &str {
        "Send a message to another agent by name, or broadcast to all active agents with to=\"*\". \
         Recipients accumulate messages in their inbox and can retrieve them. \
         Use this for coordination between concurrent sub-agents."
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::None
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "to": {
                    "type": "string",
                    "description": "Recipient agent name or session ID. Use \"*\" to broadcast to all."
                },
                "message": {
                    "type": "string",
                    "description": "Message content"
                },
                "summary": {
                    "type": "string",
                    "description": "5–10 word preview for the UI (optional)"
                }
            },
            "required": ["to", "message"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let params: SendMessageInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => {
                return ToolResult::error(
                    crate::core::tools::_support::helpers::format_tool_input_error(
                        "SendMessage",
                        r#####""to": "...", "message": "...""#####,
                        &e,
                    ),
                )
            }
        };

        if params.message.is_empty() {
            return ToolResult::error("Message cannot be empty.".to_string());
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let msg = AgentMessage {
            from: ctx.session_id.clone(),
            to: params.to.clone(),
            content: params.message.clone(),
            timestamp: now,
        };

        let preview = params.summary.as_deref().unwrap_or_else(|| {
            let s = params.message.as_str();
            let pos = crate::core::tools::_support::helpers::floor_boundary(s, 60);
            &s[..pos]
        });

        {
            let mut guard = match INBOX.lock() {
                Ok(g) => g,
                Err(_) => return ToolResult::err("Inbox lock poisoned".to_string()),
            };
            if params.to == "*" {
                let recipients: Vec<String> = guard.keys().cloned().collect();
                if recipients.is_empty() {
                    return ToolResult::ok(
                        "Broadcast queued (no active recipient inboxes yet).".to_string(),
                    );
                }
                for key in &recipients {
                    guard.entry(key.clone()).or_default().push(msg.clone());
                }
                return ToolResult::ok(format!(
                    "Broadcast to {} agent(s): {}",
                    recipients.len(),
                    preview
                ));
            }
            guard.entry(params.to.clone()).or_default().push(msg);
        }

        ToolResult::success(format!("Message sent to '{}': {}", params.to, preview))
    }
}
