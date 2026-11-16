// FastClaw · builtin tools — module index
// Each sub-module implements the `Tool` trait; `all_builtin_tools()` returns
// every tool that should be pre-loaded into the ToolRegistry on startup.

mod bash;
mod crypto_price;
mod edit;
mod exchange_rate;
mod glob;
mod grep;
mod load_tool;
mod location_info;
mod read;
mod run_skill;
mod skill;
mod weather;
mod write;

use super::Tool;

/// Return every builtin tool that should be available at startup.
/// Tools registered here are discoverable by LLMs and callable from both local
/// and remote chat sessions.
pub fn all_builtin_tools() -> Vec<Box<dyn Tool>> {
    vec![
        // ── v7 builtin (in this directory) ──
        Box::new(read::ReadFileTool),
        Box::new(write::WriteFileTool),
        Box::new(edit::EditFileTool),
        Box::new(glob::GlobTool),
        Box::new(grep::GrepTool),
        Box::new(bash::BashTool),
        Box::new(run_skill::RunSkillTool::new()),
        Box::new(skill::SkillTool::new()),
        // ── v8 Tier 1 file tools ──
        Box::new(super::apply_patch::ApplyPatchTool),
        Box::new(super::batch_edit::BatchEditTool),
        Box::new(super::download_file::DownloadFileTool),
        // ── v8 Tier 0 self-contained ──
        Box::new(super::brief::BriefTool),
        Box::new(super::sleep::SleepTool),
        Box::new(super::synthetic_output::SyntheticOutputTool),
        Box::new(super::todo_write::TodoWriteTool),
        Box::new(super::web_fetch::WebFetchTool),
        // ── v8 Tier 1 network ──
        Box::new(super::web_search::WebSearchTool),
        Box::new(super::tool_search::ToolSearchTool),
        Box::new(super::tasks::TaskCreateTool),
        Box::new(super::tasks::TaskGetTool),
        Box::new(super::tasks::TaskListTool),
        Box::new(super::tasks::TaskOutputTool),
        Box::new(super::tasks::TaskStopTool),
        Box::new(super::tasks::TaskUpdateTool),
        Box::new(super::send_message::SendMessageTool),
        Box::new(super::mcp_resources::ListMcpResourcesTool),
        Box::new(super::mcp_resources::ReadMcpResourceTool),
        Box::new(super::repl_tool::ReplTool),
        Box::new(super::ask_user::AskUserTool),
        Box::new(super::config_tool::ConfigTool),
        Box::new(super::goal_complete::GoalCompleteTool),
        Box::new(super::enter_plan_mode::EnterPlanModeTool),
        Box::new(super::exit_plan_mode::ExitPlanModeTool),
        // ★ 渐进式工具披露的阶段二入口
        Box::new(load_tool::LoadToolTool),
        // ★ 结构化天气查询（SERP 抓不到实时温度）
        Box::new(weather::WeatherTool),
        Box::new(exchange_rate::ExchangeRateTool),
        Box::new(crypto_price::CryptoPriceTool),
        Box::new(location_info::LocationInfoTool),
    ]
}
