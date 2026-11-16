// DownloadFileTool — download a file from a URL to a local path.
// Uses reqwest directly.

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde::Deserialize;
use serde_json::{json, Value};

pub struct DownloadFileTool;

#[derive(Debug, Deserialize)]
struct DownloadFileInput {
    url: String,
    #[serde(alias = "dest_path")]
    file_path: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    checksum: Option<String>,
    #[serde(default = "default_wait")]
    wait: bool,
}

fn default_wait() -> bool {
    true
}

#[async_trait::async_trait]
impl Tool for DownloadFileTool {
    fn name(&self) -> &str {
        "download_file"
    }

    fn description(&self) -> &str {
        "Download a file from a URL into the workspace. \
         PREFER this over shell commands like curl/wget for downloading files. \
         By default it blocks until the download completes (like curl); the result \
         is only returned after the file is fully written to file_path. \
         In background mode (`wait: false`) the download runs async."
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Execute
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The http/https URL of the file to download."
                },
                "file_path": {
                    "type": "string",
                    "description": "Destination file path (absolute, or relative to the working directory)."
                },
                "name": {
                    "type": "string",
                    "description": "Optional human-readable name (defaults to the URL's last path segment)."
                },
                "checksum": {
                    "type": "string",
                    "description": "Optional sha256 checksum (hex) to verify the downloaded file."
                },
                "wait": {
                    "type": "boolean",
                    "description": "Whether to block until the download completes. Default is true.",
                    "default": true
                }
            },
            "required": ["url", "file_path"]
        })
    }

    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult {
        let params: DownloadFileInput = match serde_json::from_value(args) {
            Ok(p) => p,
            Err(e) => {
                return ToolResult::error(
                    crate::core::tools::_support::helpers::format_tool_input_error(
                        "DownloadFile",
                        r#####""url": "...", "file_path": "...""#####,
                        &e,
                    ),
                )
            }
        };

        // URL basic validation
        if !(params.url.starts_with("http://") || params.url.starts_with("https://")) {
            return ToolResult::error(format!(
                "Refusing to download non-http(s) URL: {}",
                params.url
            ));
        }

        let path = ctx.resolve_path(&params.file_path);

        // Create parent directories
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return ToolResult::error(format!("Failed to create parent directories: {e}"));
            }
        }

        let client = match reqwest::Client::builder()
            .user_agent("FastClaw/2.0")
            .timeout(std::time::Duration::from_secs(120))
            .build()
        {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("Failed to create HTTP client: {e}")),
        };

        let resp = match client.get(&params.url).send().await {
            Ok(r) => r,
            Err(e) => return ToolResult::error(format!("Failed to download {}: {e}", params.url)),
        };

        if !resp.status().is_success() {
            return ToolResult::error(format!(
                "HTTP {} when downloading {}",
                resp.status(),
                params.url
            ));
        }

        let bytes = match resp.bytes().await {
            Ok(b) => b,
            Err(e) => return ToolResult::error(format!("Failed to read download body: {e}")),
        };

        let size = bytes.len();

        match std::fs::write(&path, &bytes) {
            Ok(()) => {
                ToolResult::success(format!("Downloaded {} bytes to {}", size, path.display()))
                    .with_metadata(json!({
                        "dest_path": path.display().to_string(),
                        "downloaded_bytes": size,
                    }))
            }
            Err(e) => ToolResult::error(format!("Failed to write {}: {e}", path.display())),
        }
    }
}
