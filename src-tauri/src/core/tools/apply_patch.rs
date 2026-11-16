// ApplyPatchTool — apply a unified diff patch to a file.

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};

pub struct ApplyPatchTool;

#[async_trait::async_trait]
impl Tool for ApplyPatchTool {
    fn name(&self) -> &str {
        "apply_patch"
    }
    fn description(&self) -> &str {
        "Apply a unified diff patch to a file. The patch must use unified diff format \
         (lines starting with @@). The file is read, patch applied, and written back."
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Write
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {"type": "string", "description": "Path to the file to patch"},
                "patch": {"type": "string", "description": "Unified diff patch content"}
            },
            "required": ["file_path", "patch"]
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
        let patch = match args.get("patch").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing required parameter: patch"),
        };

        let path = ctx.resolve_path(file_path);

        let original = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => return ToolResult::err(format!("Cannot read {}: {e}", path.display())),
        };

        // Simple unified diff parser
        let mut result = String::new();
        let mut applied = false;

        for chunk in patch.split("\n@@") {
            if !chunk.contains("@@") && result.is_empty() {
                // First segment before any @@ — skip
                let after_first_at = patch.find("@@").unwrap_or(patch.len());
                if chunk.as_ptr() == patch.as_ptr() {
                    continue;
                }
            }

            let chunk = if result.is_empty() {
                // First chunk — prepend the @@ that split removed
                format!("@@{chunk}")
            } else {
                format!("\n@@{chunk}")
            };

            let mut chunk_lines: Vec<&str> = chunk.lines().collect();
            if chunk_lines.is_empty() {
                continue;
            }

            // Parse @@ header
            let header = chunk_lines[0];
            if !header.starts_with("@@") {
                result.push_str(&chunk);
                continue;
            }

            // Extract context lines to locate the hunk
            let context_lines: Vec<&str> = chunk_lines[1..]
                .iter()
                .filter(|l| l.starts_with(' '))
                .copied()
                .collect();

            // Find the hunk position by matching context in original
            if let Some(pos) = find_hunk_position(&original, &context_lines) {
                // Apply the hunk
                let mut patched = String::new();
                patched.push_str(&original[..pos]);

                for &line in &chunk_lines[1..] {
                    if line.starts_with('+') {
                        patched.push_str(&line[1..]);
                        patched.push('\n');
                    } else if !line.starts_with('-') {
                        patched.push_str(&line[1..]);
                        patched.push('\n');
                    }
                    // '-' lines are skipped
                }

                // Find end of replaced section
                let old_end = find_old_section_end(&original, pos, &chunk_lines);
                patched.push_str(&original[old_end..]);

                result = patched;
                applied = true;
            } else {
                result.push_str(&chunk);
            }
        }

        if !applied {
            // Fall back to simple patch: add new lines, remove old ones
            result = apply_simple_patch(&original, patch);
        }

        match std::fs::write(&path, &result) {
            Ok(()) => ToolResult::ok(format!("Patch applied to {}", path.display())),
            Err(e) => ToolResult::err(format!("Failed to write {}: {e}", path.display())),
        }
    }
}

fn find_hunk_position(original: &str, context_lines: &[&str]) -> Option<usize> {
    if context_lines.is_empty() {
        return Some(0);
    }
    original.find(context_lines[0])
}

fn find_old_section_end(original: &str, start: usize, chunk_lines: &[&str]) -> usize {
    let old_line_count = chunk_lines[1..]
        .iter()
        .filter(|l| l.starts_with(' ') || l.starts_with('-'))
        .count();
    let mut pos = start;
    for _ in 0..old_line_count {
        if let Some(next) = original[pos..].find('\n') {
            pos += next + 1;
        } else {
            break;
        }
    }
    pos.min(original.len())
}

fn apply_simple_patch(original: &str, patch: &str) -> String {
    let mut result = original.to_string();
    for line in patch.lines() {
        if let Some(stripped) = line.strip_prefix('+') {
            if !result.contains(stripped) {
                result.push('\n');
                result.push_str(stripped);
            }
        }
        if let Some(stripped) = line.strip_prefix('-') {
            if result.contains(stripped) {
                result = result.replacen(stripped, "", 1);
            }
        }
    }
    result
}
