// FastClaw · Tool name constants
//
// Each tool defines its own name() → &str; these constants provide the canonical
// source-of-truth strings so that tool files can reference them consistently.

pub const TOOL_NAME_FILE_READ: &str = "read_file";
pub const TOOL_NAME_FILE_WRITE: &str = "write_file";
pub const TOOL_NAME_FILE_EDIT: &str = "edit_file";
pub const TOOL_NAME_BATCH_EDIT: &str = "batch_edit";
pub const TOOL_NAME_APPLY_PATCH: &str = "apply_patch";
pub const TOOL_NAME_GLOB: &str = "glob";
pub const TOOL_NAME_GREP: &str = "grep";
pub const TOOL_NAME_BASH: &str = "bash";
pub const TOOL_NAME_WEB_FETCH: &str = "web_fetch";
pub const TOOL_NAME_WEB_SEARCH: &str = "web_search";
pub const TOOL_NAME_TODO_WRITE: &str = "todo_write";
pub const TOOL_NAME_ASK_USER: &str = "ask_user";
pub const TOOL_NAME_ENTER_PLAN_MODE: &str = "enter_plan_mode";
pub const TOOL_NAME_EXIT_PLAN_MODE: &str = "exit_plan_mode";
pub const TOOL_NAME_TASK_CREATE: &str = "task_create";
pub const TOOL_NAME_TASK_GET: &str = "task_get";
pub const TOOL_NAME_TASK_LIST: &str = "task_list";
pub const TOOL_NAME_TASK_OUTPUT: &str = "task_output";
pub const TOOL_NAME_TASK_STOP: &str = "task_stop";
pub const TOOL_NAME_TASK_UPDATE: &str = "task_update";
