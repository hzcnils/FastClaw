// FastClaw · Tool registry — HashMap-backed registry of builtin + vendored tools

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{Tool, ToolContext, ToolResult};
use crate::core::error::AppError;
use crate::core::llm::ToolDefinition;

// ── Shared type alias ──

pub type SharedToolRegistry = Arc<RwLock<ToolRegistry>>;

// ── Registry ──

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Create a new registry pre-loaded with all builtin tools.
    pub fn new() -> Self {
        let mut tools: HashMap<String, Box<dyn Tool>> = HashMap::new();
        for t in super::builtin::all_builtin_tools() {
            tools.insert(t.name().to_string(), t);
        }
        Self { tools }
    }

    /// Check whether a tool with the given name is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// List all registered tools as OpenAI-compatible function definitions.
    pub fn list_definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.to_definition()).collect()
    }

    /// Execute a tool by name. Returns `Err(NotFound)` if the tool is not registered;
    /// tool-level errors are returned as `Ok(ToolResult { is_error: true })`.
    pub async fn execute(
        &self,
        name: &str,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, AppError> {
        match self.tools.get(name) {
            Some(tool) => Ok(tool.execute(args, ctx).await),
            None => Err(AppError::NotFound(format!("tool not found: {name}"))),
        }
    }

    /// Register an additional tool at runtime (e.g. SkillTool needs skills dir).
    #[allow(dead_code)]
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }
}

/// Create the shared (Arc<RwLock<…>>) registry used by `AppState`.
pub fn create_shared_tool_registry() -> SharedToolRegistry {
    Arc::new(RwLock::new(ToolRegistry::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tools::ToolContext;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn registry_is_not_empty() {
        let reg = ToolRegistry::new();
        assert!(!reg.tools.is_empty(), "builtin tools should be loaded");
    }

    #[tokio::test]
    async fn contains_and_list() {
        let reg = ToolRegistry::new();
        assert!(reg.contains("read_file"));
        let defs = reg.list_definitions();
        assert!(defs.iter().any(|d| d.function.name == "read_file"));
    }

    #[tokio::test]
    async fn unknown_tool_returns_not_found() {
        let reg = ToolRegistry::new();
        let ctx = ToolContext::minimal(PathBuf::from("."), "test".into());
        let result = reg
            .execute("no_such_tool", serde_json::json!({}), &ctx)
            .await;
        assert!(result.is_err());
    }
}
