// EditFileTool — performs exact string replacement in an existing file.
// Requires uniqueness of the old_string match.

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};

pub struct EditFileTool;

impl EditFileTool {
    fn relative_to_cwd(path: &str, ctx: &ToolContext) -> std::path::PathBuf {
        let p = std::path::Path::new(path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            ctx.working_dir.join(p)
        }
    }
}

#[async_trait::async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Performs exact string replacement in an existing file.\n\
         The `old_string` must match exactly one location in the file (including whitespace and indentation).\n\
         If `replace_all` is true, every occurrence is replaced instead.\n\
         You must use the read_file tool first to read the file's contents if you haven't already."
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute or relative path to the file to modify."
                },
                "old_string": {
                    "type": "string",
                    "description": "The text to replace."
                },
                "new_string": {
                    "type": "string",
                    "description": "The text to replace it with (must be different from old_string)."
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default false).",
                    "default": false
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        })
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let file_path = match args.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing required parameter: file_path"),
        };
        if let Some(reason) = crate::core::tools::_support::helpers::check_system_path(file_path) {
            return ToolResult::err(reason);
        }
        let old = match args.get("old_string").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return ToolResult::err("Missing required parameter: old_string"),
        };
        let new = match args.get("new_string").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return ToolResult::err("Missing required parameter: new_string"),
        };
        if old == new {
            return ToolResult::err("old_string and new_string must be different");
        }

        let replace_all = args
            .get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let path = Self::relative_to_cwd(file_path, ctx);

        let original = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => return ToolResult::err(format!("Cannot read {}: {e}", path.display())),
        };

        if !replace_all && original.matches(old).count() > 1 {
            return ToolResult::err(format!(
                "old_string is not unique in {} — found {} matches. \
                 Use a larger string with more surrounding context to make it unique, \
                 or set replace_all to true.",
                path.display(),
                original.matches(old).count()
            ));
        }

        if !original.contains(old) {
            return ToolResult::err(format!(
                "old_string not found in {}. The file may have been modified externally — \
                 re-read it before retrying.",
                path.display()
            ));
        }

        let modified = if replace_all {
            original.replace(old, new)
        } else {
            original.replacen(old, new, 1)
        };

        match std::fs::write(&path, &modified) {
            Ok(()) => ToolResult::ok(format!("Successfully edited {}", path.display())),
            Err(e) => ToolResult::err(format!("Failed to write {}: {e}", path.display())),
        }
    }
}
