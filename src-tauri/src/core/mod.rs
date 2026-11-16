// FastClaw · 全局应用状态
// 注入 Tauri，所有 Command 通过 State<AppState> 访问

// pub mod state; // 已迁移到顶层 state.rs

pub mod config;
pub mod constants;
pub mod dependency_services;
pub mod doc_extract;
pub mod error;
pub mod file2md;
pub mod grounding;

pub mod intent;
pub mod llm;
pub mod mcp;
pub mod models;
pub mod session_plan;
pub mod skills;
pub mod time_inject;
pub mod topic_title;
pub mod tools;
pub mod wsp;
