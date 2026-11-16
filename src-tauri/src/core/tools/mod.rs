// FastClaw · Tool context, trait, and result types
//
// ToolContext / ToolResult / Tool trait 在此统一，
//     使迁移工具只需改 use 路径 + 处理外部依赖，不改主体逻辑。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub mod _support;
pub mod builtin;
pub mod registry;

// ── tools ──
pub mod apply_patch;
pub mod ask_user;
pub mod batch_edit;
pub mod block_match;
pub mod brief;
pub mod bundled_skills;
pub mod config_tool;
pub mod disclosure;
pub mod download_file;
pub mod enter_plan_mode;
pub mod exit_plan_mode;
pub mod formatter;
pub mod goal_complete;
pub mod mcp_resources;
pub mod repl_tool;
pub mod selection;
pub mod send_message;
pub mod sleep;
pub mod synthetic_output;
pub mod tasks;
pub mod todo_write;
pub mod tool_search;
pub mod web_fetch;
pub mod web_search;
pub mod web_search_parse;

// ── Permission Level ──

/// Tool permission / risk level.
/// Used by the security layer and frontend rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PermissionLevel {
    /// Safe read-only operations — no confirmation needed.
    None = 0,
    /// Reads from disk / network without side-effects.
    ReadOnly = 1,
    /// Writes / edits files; may require confirmation in restricted modes.
    Write = 2,
    /// Runs a subprocess — should show confirmation dialog (future Part D).
    Execute = 3,
    /// High-risk destructive operation (e.g. rm -rf). Currently hard-blocked.
    Dangerous = 4,
    /// Explicitly disabled. Never allowed.
    Forbidden = 5,
}

// ── Permission Mode ──

/// User-level permission mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    Ask,
    AcceptEdits,
    BypassPermissions,
}

impl Default for PermissionMode {
    fn default() -> Self {
        PermissionMode::Ask
    }
}

// ── Tool Trait ──

/// Every built-in or vendored tool implements this trait.
/// Registered in `ToolRegistry` and discoverable by LLMs via `list_definitions()`.
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// Unique name used in tool-call JSON (e.g. "read_file", "bash").
    fn name(&self) -> &str;

    /// Human-readable description surfaced to the LLM.
    fn description(&self) -> &str;

    /// Default risk level (overridable per-tool).
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    /// JSON Schema for the `arguments` object.
    fn input_schema(&self) -> serde_json::Value;

    /// Execute the tool. Errors that should be surfaced to the model
    /// should set `ToolResult::is_error = true` rather than returning `Err(...)`.
    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> ToolResult;

    /// Tools that spawn subprocesses / manage their own lifecycle (bash/pty/agent)
    /// return true so the caller doesn't double-wrap with timeout.
    fn manages_own_lifecycle(&self) -> bool {
        false
    }

    /// Convert to an OpenAI-compatible function definition.
    fn to_definition(&self) -> crate::core::llm::ToolDefinition {
        crate::core::llm::ToolDefinition {
            r#type: "function".into(),
            function: crate::core::llm::ToolFunctionDef {
                name: self.name().into(),
                description: self.description().into(),
                parameters: Some(self.input_schema()),
            },
        }
    }
}

// ── Tool Context ──

/// Context passed to all tool executions.
/// 字段带默认值，调用方可以只填部分上下文。
#[derive(Clone)]
pub struct ToolContext {
    pub working_dir: PathBuf,
    pub session_id: String,
    /// Reserved for tools that need DB/LLM access. Will become `Arc<AppState>` later.
    pub app: Option<()>,
    /// LLM 管理器句柄（用于 web_search 的 LLM 扩展 / 推荐站点 / deep 闭环）。
    /// None 时相关支路静默退回确定式路径。
    pub llm: Option<Arc<crate::core::llm::LlmManager>>,
    pub lang: Option<String>,
    pub non_interactive: bool,
    pub file_observations: Arc<Mutex<HashMap<String, String>>>,

    // ── v8 兼容扩展（全部带默认值）──
    pub permission_mode: PermissionMode,
    /// Roots the tool is allowed to write within (default = [working_dir]).
    pub workspace_roots: Vec<PathBuf>,
    /// Optional MCP manager handle for mcp_wrapper / mcp_auth_tool.
    pub mcp_manager: Option<crate::core::mcp::SharedMcpManager>,
    /// 「本次对话」允许使用的 Skill 名称白名单。
    /// None = 不限制（沿用 skill 自身的 enabled 状态）；Some(list) = 只允许名单内的
    /// （空 list = 一个都不允许）。
    pub enabled_skills: Option<Vec<String>>,
}

impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext")
            .field("working_dir", &self.working_dir)
            .field("session_id", &self.session_id)
            .field("app", &self.app)
            .field("llm", &self.llm.as_ref().map(|_| "LlmManager"))
            .field("lang", &self.lang)
            .field("non_interactive", &self.non_interactive)
            .field("permission_mode", &self.permission_mode)
            .field("workspace_roots", &self.workspace_roots)
            .field(
                "mcp_manager",
                &self.mcp_manager.as_ref().map(|_| "McpManager"),
            )
            .finish()
    }
}

impl ToolContext {
    /// Minimal constructor — chat.rs / handler.rs use this to avoid field-count breakage.
    pub fn minimal(working_dir: PathBuf, session_id: String) -> Self {
        Self {
            working_dir: working_dir.clone(),
            session_id,
            app: None,
            llm: None,
            lang: None,
            non_interactive: false,
            file_observations: Arc::new(Mutex::new(HashMap::new())),
            permission_mode: PermissionMode::default(),
            workspace_roots: vec![working_dir],
            mcp_manager: None,
            enabled_skills: None,
        }
    }

    /// 注入「本次对话」的 Skill 白名单（builder 风格）。
    pub fn with_enabled_skills(mut self, skills: Vec<String>) -> Self {
        self.enabled_skills = Some(skills);
        self
    }

    /// 注入 LLM 管理器句柄（builder 风格，返回新实例）。
    pub fn with_llm(mut self, llm: Arc<crate::core::llm::LlmManager>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Resolve a relative or absolute path to an absolute one.
    pub fn resolve_path(&self, path: &str) -> PathBuf {
        let p = PathBuf::from(path);
        let joined = if p.is_absolute() {
            p
        } else {
            self.working_dir.join(p)
        };
        std::fs::canonicalize(&joined).unwrap_or(joined)
    }

    /// Check whether a path is within any workspace root.
    pub fn path_is_within_workspace(&self, path: &Path) -> bool {
        self.workspace_roots.iter().any(|r| path.starts_with(r))
    }

    /// Record that a file was read (for external-change detection later).
    pub fn record_file_change(&self, path: &Path, content: &str) {
        if let Ok(mut m) = self.file_observations.lock() {
            m.insert(path.display().to_string(), content.to_string());
        }
    }

    // ── Permission gates (stopgap: workspace Write/ReadOnly pass; Forbidden blocks) ──

    pub fn check_permission(&self, level: PermissionLevel, desc: &str) -> Result<(), String> {
        self.gate(level, desc, None)
    }

    pub fn check_permission_for_path(
        &self,
        level: PermissionLevel,
        desc: &str,
        path: &Path,
    ) -> Result<(), String> {
        self.gate(level, desc, Some(path))
    }

    pub fn check_permission_with_details(
        &self,
        level: PermissionLevel,
        desc: &str,
        details: &str,
    ) -> Result<(), String> {
        self.gate(level, &format!("{desc}: {details}"), None)
    }

    pub fn check_permission_with_details_and_path(
        &self,
        level: PermissionLevel,
        desc: &str,
        details: &str,
        path: &Path,
    ) -> Result<(), String> {
        self.gate(level, &format!("{desc}: {details}"), Some(path))
    }

    pub fn require_confirmation(&self, desc: &str) -> Result<(), String> {
        self.gate(PermissionLevel::Execute, desc, None)
    }

    fn gate(&self, level: PermissionLevel, desc: &str, _path: Option<&Path>) -> Result<(), String> {
        match level {
            PermissionLevel::Forbidden => Err(format!("已禁止: {desc}")),
            PermissionLevel::None | PermissionLevel::ReadOnly | PermissionLevel::Write => Ok(()),
            PermissionLevel::Dangerous => Err(format!("高危操作已禁止（需用户显式授权）: {desc}")),
            PermissionLevel::Execute => {
                if self.non_interactive {
                    return Ok(());
                }
                // TODO(security): 接入 Tauri show_permission_dialog 同步等待
                tracing::error!("安全门控: 放行Execute但无确认弹窗 — {desc}");
                Ok(())
            }
        }
    }
}

// ── Tool Result ──

/// Result of a tool execution.
/// metadata 为 Option<Value>，并提供 success、error、with_metadata 构造。
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub content: String,
    pub is_error: bool,
    pub metadata: Option<serde_json::Value>,
}

impl ToolResult {
    // ── v7 API (kept for existing builtin/ tools) ──

    /// Convenience: successful result with plain-text content.
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
            metadata: None,
        }
    }

    /// Convenience: error surfaced to the model (not a Rust Err).
    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            content: msg.into(),
            is_error: true,
            metadata: None,
        }
    }


    pub fn success(content: impl Into<String>) -> Self {
        Self::ok(content)
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self::err(content)
    }

    pub fn with_metadata(mut self, m: serde_json::Value) -> Self {
        self.metadata = Some(m);
        self
    }
}
