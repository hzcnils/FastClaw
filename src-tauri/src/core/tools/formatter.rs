// formatter.rs — run a configured file formatter after writes/edits.
//
// NOTE: ctx.config.formatter is not available in FastClaw. This module
// provides a no-op stub; formatter integration requires future infra.

pub async fn try_format_file(_path: &str, _ctx: &crate::core::tools::ToolContext) {
    // Formatter configuration is not available in FastClaw v8.
    // This stub preserves the API so that file_write/file_edit call sites
    // compile unchanged. Add formatter config support in a future phase.
}
