// BatchEditTool — apply multiple edits to a file in one call.

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};

pub struct BatchEditTool;

#[async_trait::async_trait]
impl Tool for BatchEditTool {
    fn name(&self) -> &str {
        "batch_edit"
    }
    fn description(&self) -> &str {
        "Apply multiple edits to a file at once. Each edit has old_string, new_string, and replace_all.\n\
         Edits are applied in order; each edit sees the result of previous edits."
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Write
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {"type": "string", "description": "Path to the file to edit"},
                "edits": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old_string": {"type": "string", "description": "Text to replace"},
                            "new_string": {"type": "string", "description": "Replacement text"},
                            "replace_all": {"type": "boolean", "default": false}
                        },
                        "required": ["old_string", "new_string"]
                    },
                    "description": "List of edits to apply in order"
                }
            },
            "required": ["file_path", "edits"]
        })
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let file_path = match args.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing required parameter: file_path"),
        };
        let edits = match args.get("edits").and_then(|v| v.as_array()) {
            Some(e) => e,
            None => return ToolResult::err("Missing required parameter: edits"),
        };

        let path = ctx.resolve_path(file_path);

        let mut content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => return ToolResult::err(format!("Cannot read {}: {e}", path.display())),
        };

        let mut applied = 0usize;
        for (i, edit) in edits.iter().enumerate() {
            let old = match edit.get("old_string").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return ToolResult::err(format!("Edit #{}: missing old_string", i + 1)),
            };
            let new = match edit.get("new_string").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return ToolResult::err(format!("Edit #{}: missing new_string", i + 1)),
            };
            let replace_all = edit
                .get("replace_all")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if old == new {
                continue;
            }

            if !replace_all && content.matches(old).count() > 1 {
                return ToolResult::err(format!(
                    "Edit #{}: old_string is not unique ({} matches in {}). \
                     Use more context or set replace_all: true.",
                    i + 1,
                    content.matches(old).count(),
                    path.display()
                ));
            }

            if !content.contains(old) {
                return ToolResult::err(format!(
                    "Edit #{}: old_string not found in {}.",
                    i + 1,
                    path.display()
                ));
            }

            if replace_all {
                content = content.replace(old, new);
            } else {
                content = content.replacen(old, new, 1);
            }
            applied += 1;
        }

        match std::fs::write(&path, &content) {
            Ok(()) => ToolResult::ok(format!("Applied {} edit(s) to {}", applied, path.display())),
            Err(e) => ToolResult::err(format!("Failed to write {}: {e}", path.display())),
        }
    }
}
