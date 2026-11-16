// ReadFileTool — reads a file from disk with optional offset and limit.
// Returns content with line numbers (1-based).

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};

pub struct ReadFileTool;

impl ReadFileTool {
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
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read a file from the local filesystem. Returns the content with 1-based line numbers.\n\
         Use `offset` and `limit` to restrict which lines to read (especially handy for long files), \
         but it's recommended to read the whole file by not providing these parameters."
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute or relative path to the file to read."
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (1-based, inclusive)."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read."
                }
            },
            "required": ["file_path"]
        })
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let file_path = match args.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing required parameter: file_path"),
        };
        // 敏感文件保护
        let lower = file_path.to_lowercase();
        let sensitive: &[&str] = &[
            ".ssh/id_rsa",
            ".ssh/authorized_keys",
            ".ssh/known_hosts",
            "id_rsa",
            "id_ed25519",
            "private key",
            ".pem",
            ".env",
            ".credentials",
            ".secrets",
            "credentials.json",
            "web.config",
            "appsettings.json",
            "/etc/shadow",
            "/etc/passwd",
            "/etc/sudoers",
            "ntds.dit",
            "sam",
            "system",
            "security", // Windows SAM
        ];
        for s in sensitive {
            if lower.contains(s) {
                return ToolResult::err("安全限制：无法读取敏感文件");
            }
        }

        let path = Self::relative_to_cwd(file_path, ctx);

        // docx/xlsx 走文档文本抽取，其余走纯文本读取
        let content = match crate::core::doc_extract::extract_document_text(&path) {
            Some(t) => t,
            None => match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => return ToolResult::err(format!("Cannot read {}: {e}", path.display())),
            },
        };

        let lines: Vec<&str> = content.lines().collect();
        let offset = args
            .get("offset")
            .and_then(|v| v.as_u64())
            .map(|n| (n.max(1) - 1) as usize)
            .unwrap_or(0);
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);

        if offset >= lines.len() {
            return ToolResult::ok(format!(
                "Offset {offset} exceeds file length {} for {}",
                lines.len(),
                path.display()
            ));
        }

        let end = limit
            .map(|l| (offset + l).min(lines.len()))
            .unwrap_or(lines.len());
        let selected = &lines[offset..end];

        // Format with line numbers
        let start_num = offset + 1;
        let width = (end).to_string().len().max(3);
        let numbered: String = selected
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let num = start_num + i;
                format!("{:>width$}\t{line}", num, width = width)
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Record observation for external-change detection
        if let Ok(mut obs) = ctx.file_observations.lock() {
            obs.insert(path.display().to_string(), content);
        }

        ToolResult::ok(numbered)
    }
}
