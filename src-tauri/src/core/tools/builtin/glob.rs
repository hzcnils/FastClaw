// GlobTool — fast file pattern matching. Returns matching file paths sorted
// by modification time (most recent first).

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};

pub struct GlobTool;

#[async_trait::async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Fast file pattern matching. Supports glob patterns like \"**/*.js\" or \"src/**/*.ts\".\n\
         Returns matching file paths sorted by modification time (most recent first).\n\
         Use `path` to limit the search to a specific directory (defaults to the working directory)."
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The glob pattern to match files against (e.g. \"**/*.rs\", \"src/*.vue\")."
                },
                "path": {
                    "type": "string",
                    "description": "The directory to search in. Defaults to the current working directory."
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing required parameter: pattern"),
        };

        let search_dir = args
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| {
                let path = std::path::Path::new(p);
                if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    ctx.working_dir.join(p)
                }
            })
            .unwrap_or_else(|| ctx.working_dir.clone());

        if !search_dir.is_dir() {
            return ToolResult::err(format!("Directory not found: {}", search_dir.display()));
        }

        let full_pattern = search_dir.join(pattern);
        let pattern_str = full_pattern.display().to_string();

        let mut paths: Vec<std::path::PathBuf> = match glob::glob(&pattern_str) {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(e) => return ToolResult::err(format!("Invalid glob pattern: {e}")),
        };

        // Sort by modification time (newest first)
        paths.sort_by_key(|p| {
            std::fs::metadata(p)
                .and_then(|m| m.modified())
                .map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0)
                })
                .unwrap_or(0)
        });
        paths.reverse();

        if paths.is_empty() {
            return ToolResult::ok("No matching files found.");
        }

        // Filter out the search dir itself
        let listing: Vec<String> = paths
            .iter()
            .filter(|p| p.is_file() || p.is_dir())
            .map(|p| {
                // Show relative path when possible
                if let Ok(rel) = p.strip_prefix(&search_dir) {
                    rel.display().to_string()
                } else {
                    p.display().to_string()
                }
            })
            .collect();

        ToolResult::ok(format!(
            "Found {} file(s) matching \"{}\":\n{}",
            listing.len(),
            pattern,
            listing.join("\n")
        ))
    }
}
