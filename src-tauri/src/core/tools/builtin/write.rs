// WriteFileTool — writes or overwrites a file on disk.
// Creates parent directories if they don't exist.

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};

pub struct WriteFileTool;

impl WriteFileTool {
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
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Writes a file to the local filesystem. Creates parent directories if needed.\n\
         This tool will overwrite the existing file if there is one at the provided path.\n\
         If this is an existing file, you MUST use the read_file tool first to read the file's contents."
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
                    "description": "Absolute or relative path of the file to write."
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file."
                }
            },
            "required": ["file_path", "content"]
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
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::err("Missing required parameter: content"),
        };

        let path = Self::relative_to_cwd(file_path, ctx);

        // Create parent directories
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return ToolResult::err(format!(
                    "Failed to create parent directories for {}: {e}",
                    path.display()
                ));
            }
        }

        match std::fs::write(&path, content) {
            Ok(()) => ToolResult::ok(format!(
                "Successfully wrote {} bytes to {}",
                content.len(),
                path.display()
            )),
            Err(e) => ToolResult::err(format!("Failed to write {}: {e}", path.display())),
        }
    }
}
