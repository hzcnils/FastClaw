// BashTool — executes shell commands.
// Cross-platform: uses PowerShell on Windows, $SHELL (or /bin/sh) on Unix.
// Windows output is decoded via GBK to avoid Chinese mojibake.
// Includes a stopgap dangerous-command classifier (hard-blocks Critical patterns).
// Full bash_classifier.rs port is deferred to a future security phase.

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};

pub struct BashTool;

/// Stopgap: hard-block obviously destructive commands.
/// A fuller command classifier can replace this in a later security phase.
fn is_dangerous(command: &str) -> Option<&'static str> {
    let trimmed = command.trim();

    // Fork bomb
    if trimmed.contains(":(){:|:&};:") {
        return Some("Fork bomb detected.");
    }

    // Block PowerShell cmdlet bypasses
    let upper_ps = trimmed.to_uppercase().replace(" ", "");
    if upper_ps.contains("REMOVE-ITEM")
        && (upper_ps.contains("-RECURSE") || upper_ps.contains("-R"))
        && upper_ps.contains("-FORCE")
    {
        return Some("Destructive PowerShell Remove-Item is forbidden.");
    }
    if upper_ps.contains("RMDIR") && upper_ps.contains("-RECURSE") {
        return Some("Destructive rmdir is forbidden.");
    }
    if upper_ps.contains("DEL")
        && (upper_ps.contains("/F") || upper_ps.contains("-FORCE"))
        && upper_ps.contains("/S")
    {
        return Some("Force recursive delete is forbidden.");
    }

    // Windows drive root protection
    let upper = trimmed.to_uppercase();
    for drive in &["C:", "D:", "E:", "F:", "G:"] {
        if upper.contains(&format!("RM -RF {}", drive))
            || upper.contains(&format!("RM -RF /{}/", drive))
            || upper.contains(&format!("RMDIR /S /Q {}", drive))
            || upper.contains(&format!("DEL /F /S {}", drive))
            || upper.contains(&format!("FORMAT {}", drive))
        {
            return Some("Destructive operation on drive root is forbidden.");
        }
    }

    let patterns: &[(&str, &str)] = &[
        ("rm -rf /", "rm -rf / is forbidden."),
        ("rm -rf /*", "rm -rf /* is forbidden."),
        ("rm -rf ~", "rm -rf ~ is forbidden."),
        ("mkfs.", "Disk formatting is forbidden."),
        (
            "dd of=/dev/",
            "Writing directly to block devices is forbidden.",
        ),
        (">/dev/sda", "Writing to block devices is forbidden."),
        ("chmod 777 /", "Recursive chmod on root is forbidden."),
        ("> /etc/", "Writing to /etc/ is forbidden."),
    ];

    for (pattern, reason) in patterns {
        if trimmed.contains(pattern) {
            return Some(reason);
        }
    }

    None
}

// ── Cross-platform shell spawn ──

#[cfg(target_os = "windows")]
fn spawn_shell(command: &str, cwd: &std::path::Path) -> tokio::process::Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut c = tokio::process::Command::new("powershell");
    c.args(["-NoProfile", "-NonInteractive", "-Command", command])
        .current_dir(cwd)
        .creation_flags(CREATE_NO_WINDOW);
    c
}

#[cfg(not(target_os = "windows"))]
fn spawn_shell(command: &str, cwd: &std::path::Path) -> tokio::process::Command {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let mut c = tokio::process::Command::new(shell);
    c.args(["-c", command]).current_dir(cwd);
    c
}

// ── Cross-platform output decoding (GBK on Windows) ──

#[cfg(target_os = "windows")]
fn decode(bytes: &[u8]) -> String {
    encoding_rs::GBK.decode(bytes).0.into_owned()
}

#[cfg(not(target_os = "windows"))]
fn decode(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[async_trait::async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Executes a shell command. Returns stdout, stderr, and exit code.\n\
         On Windows the command runs in PowerShell; on Unix it uses $SHELL or /bin/sh.\n\
         Commands touching files outside the working directory require confirmation.\n\
         Dangerous commands (rm -rf /, mkfs, dd to block devices, fork bombs) are hard-blocked."
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Execute
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute."
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Optional timeout in milliseconds (default 120000, max 600000)."
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let command = match args.get("command").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::err("Missing required parameter: command"),
        };

        // Stopgap dangerous-command check
        if let Some(reason) = is_dangerous(command) {
            return ToolResult::err(format!("Command blocked: {reason}"));
        }

        let timeout_ms: u64 = args
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(120_000)
            .min(600_000);

        // Resolve working directory
        let cwd = &ctx.working_dir;

        let output = match tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            spawn_shell(command, cwd).output(),
        )
        .await
        {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => return ToolResult::err(format!("Failed to execute command: {e}")),
            Err(_) => return ToolResult::err(format!("Command timed out after {timeout_ms}ms")),
        };

        let stdout = decode(&output.stdout);
        let stderr = decode(&output.stderr);

        let mut result = String::new();
        if !stdout.is_empty() {
            result.push_str(&format!("STDOUT:\n{stdout}"));
        }
        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&format!("STDERR:\n{stderr}"));
        }
        if result.is_empty() {
            result.push_str("(no output)");
        }

        let exit_code = output.status.code().unwrap_or(-1);

        ToolResult {
            content: format!("[exit {exit_code}]\n{result}"),
            is_error: exit_code != 0,
            metadata: Some(serde_json::json!({"exit_code": exit_code})),
        }
    }
}
