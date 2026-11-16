// WebFetchTool — fetch a URL and extract readable text content.

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};

pub struct WebFetchTool;

#[async_trait::async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }
    fn description(&self) -> &str {
        "Fetches a URL, extracts readable text, and returns the content.\n\
         Useful for reading documentation, articles, or API responses.\n\
         HTTP is upgraded to HTTPS automatically."
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {"type": "string", "description": "The URL to fetch", "format": "uri"},
                "prompt": {"type": "string", "description": "Optional: what information to extract from the page"}
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, args: serde_json::Value, _ctx: &ToolContext) -> ToolResult {
        let url = match args.get("url").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => return ToolResult::err("Missing required parameter: url"),
        };

        // Upgrade HTTP → HTTPS
        let url = if url.starts_with("http://") {
            format!("https://{}", &url[7..])
        } else if !url.starts_with("https://") {
            format!("https://{url}")
        } else {
            url.to_string()
        };

        let client = match reqwest::Client::builder()
            .user_agent("FastClaw/2.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
        {
            Ok(c) => c,
            Err(e) => return ToolResult::err(format!("Failed to create HTTP client: {e}")),
        };

        let resp = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => return ToolResult::err(format!("Failed to fetch {url}: {e}")),
        };

        let status = resp.status();
        if !status.is_success() {
            return ToolResult::err(format!("HTTP {status} when fetching {url}"));
        }

        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/html")
            .to_string();

        let body = match resp.text().await {
            Ok(b) => b,
            Err(e) => return ToolResult::err(format!("Failed to read response body: {e}")),
        };

        // Extract text from HTML; pass through plain text / JSON
        let is_html = ct.contains("text/html") || ct.contains("application/xhtml");
        let text = if is_html {
            // Simple HTML → text: remove tags
            let doc = scraper::Html::parse_document(&body);
            doc.root_element().text().collect::<Vec<_>>().join(" ")
        } else {
            body
        };

        // Truncate to avoid overwhelming the LLM
        let max_len = 50_000;
        let truncated = if text.len() > max_len {
            format!("{}...(truncated at {max_len} chars)", &text[..max_len])
        } else {
            text
        };

        ToolResult::ok(truncated)
    }
}
