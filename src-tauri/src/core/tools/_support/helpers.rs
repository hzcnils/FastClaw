// FastClaw · Helper functions shared across tools
//

use serde_json::Value;

/// Format a tool input deserialization error with helpful guidance.
pub fn format_tool_input_error(
    tool_name: &str,
    example_fields: &str,
    e: &serde_json::Error,
) -> String {
    let msg = e.to_string();
    if msg.contains("missing field") || msg.contains("invalid type") {
        format!(
            "参数错误：{} 工具接收到格式不正确的输入。正确格式示例：{{{}}}。原始错误：{}",
            tool_name, example_fields, msg
        )
    } else {
        format!("参数错误：{}", msg)
    }
}

/// Derive a JSON Schema from a type that implements schemars::JsonSchema.
/// NOTE: schemars is not available in FastClaw; use manual schemas or inline json!({...}).
#[allow(dead_code)]
pub fn derive_schema_from_example(example: serde_json::Value) -> serde_json::Value {
    example
}

/// 系统路径保护：阻止写入/修改系统关键目录。
/// 返回 Some(reason) 应被拒绝，None 表示放行。
pub fn check_system_path(path: &str) -> Option<&'static str> {
    let upper = path.to_uppercase();
    // Windows 系统目录
    if upper.contains("C:\\WINDOWS") || upper.contains("C:\\WINNT") {
        return Some("拒绝写入 Windows 系统目录");
    }
    if upper.contains("\\SYSTEM32") || upper.contains("\\SYSWOW64") {
        return Some("拒绝写入 System32/SysWOW64");
    }
    if upper.contains("C:\\PROGRAM FILES") || upper.contains("C:\\PROGRAMDATA") {
        return Some("拒绝写入 Program Files");
    }
    if upper.contains("\\BOOT\\") || upper.contains("\\EFI\\") {
        return Some("拒绝写入引导分区");
    }
    // Linux 系统目录
    if path.starts_with("/etc/") || path == "/etc" {
        return Some("拒绝写入 /etc 系统配置目录");
    }
    if path.starts_with("/boot/") || path == "/boot" {
        return Some("拒绝写入 /boot 引导目录");
    }
    if path.starts_with("/sys/") || path.starts_with("/proc/") || path.starts_with("/dev/") {
        return Some("拒绝写入虚拟文件系统");
    }
    None
}

/// Find a UTF-8 character boundary at or before `pos` in `s`.
/// vendored-from: claurst_core::truncate::floor_boundary
pub fn floor_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        s.len()
    } else {
        let mut p = pos;
        while p > 0 && !s.is_char_boundary(p) {
            p -= 1;
        }
        p
    }
}
