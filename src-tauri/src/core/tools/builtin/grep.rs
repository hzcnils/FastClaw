// GrepTool — content search via regex. Uses regex + walkdir (no ripgrep crate dep needed).

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};

pub struct GrepTool;

#[async_trait::async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Content search using regular expressions. Searches files under a directory recursively.\n\
         Returns matching lines with file paths and line numbers.\n\
         Use `glob` to filter files (e.g. \"*.rs\"), `path` to limit the search scope.\n\
         Defaults to 200 matches max; set `head_limit` to override (0 = unlimited, use sparingly)."
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
                    "description": "The regex pattern to search for in file contents."
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search in. Defaults to working directory."
                },
                "glob": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g. \"*.js\", \"*.{ts,tsx}\")."
                },
                "head_limit": {
                    "type": "integer",
                    "description": "Max matching lines to return (default 200, 0 = unlimited)."
                },
                "-i": {
                    "type": "boolean",
                    "description": "Case insensitive search (default false)."
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let pattern_str = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing required parameter: pattern"),
        };

        let case_insensitive = args.get("-i").and_then(|v| v.as_bool()).unwrap_or(false);

        let re = match regex::RegexBuilder::new(pattern_str)
            .case_insensitive(case_insensitive)
            .build()
        {
            Ok(r) => r,
            Err(e) => return ToolResult::err(format!("Invalid regex pattern: {e}")),
        };

        let search_root = args
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

        let file_glob = args
            .get("glob")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let head_limit: usize = args
            .get("head_limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(200);

        if !search_root.exists() {
            return ToolResult::err(format!("Path not found: {}", search_root.display()));
        }

        let mut results: Vec<String> = Vec::new();
        let mut total_matches = 0usize;

        let walker = walkdir::WalkDir::new(&search_root)
            .follow_links(false)
            .max_depth(20)
            .into_iter()
            .filter_entry(|e| {
                // Skip hidden directories and common ignores
                let name = e.file_name().to_string_lossy();
                if name.starts_with('.') && e.file_type().is_dir() {
                    return name == "." || name == "..";
                }
                if e.file_type().is_dir() {
                    return !matches!(
                        name.as_ref(),
                        "node_modules" | "target" | "dist" | ".git" | "__pycache__"
                    );
                }
                true
            });

        for entry in walker {
            if head_limit > 0 && total_matches >= head_limit {
                break;
            }

            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            if !entry.file_type().is_file() {
                continue;
            }

            // Apply file-glob filter
            if let Some(ref glob_pattern) = file_glob {
                if let Some(file_name) = entry.file_name().to_str() {
                    let pattern = glob::Pattern::new(glob_pattern);
                    if !pattern.map_or(false, |p| p.matches(file_name)) {
                        continue;
                    }
                }
            }

            // Read and search file
            let content = match std::fs::read_to_string(entry.path()) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let rel_path = if let Ok(rel) = entry.path().strip_prefix(&search_root) {
                rel.display().to_string()
            } else {
                entry.path().display().to_string()
            };

            for (line_num, line) in content.lines().enumerate() {
                if head_limit > 0 && total_matches >= head_limit {
                    break;
                }
                if re.is_match(line) {
                    results.push(format!("{}:{}:{}", rel_path, line_num + 1, line));
                    total_matches += 1;
                }
            }
        }

        if results.is_empty() {
            return ToolResult::ok("No matches found.");
        }

        let truncated = if head_limit > 0 && total_matches >= head_limit {
            format!(
                "\n\n(Results truncated at {head_limit} matches. Use head_limit=0 for unlimited.)"
            )
        } else {
            String::new()
        };

        ToolResult::ok(format!(
            "Found {} match(es) for \"{}\":\n{}{}",
            total_matches,
            pattern_str,
            results.join("\n"),
            truncated
        ))
    }
}
