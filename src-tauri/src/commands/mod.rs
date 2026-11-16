// FastClaw · Tauri Command 层
// 每个子模块对应一组前端 invoke() 命令

pub mod agents;
pub mod assistants;
pub mod chat;
pub mod cloud_providers;
pub mod config;
pub mod cost;
pub mod export;
pub mod fastplan_connection;
pub mod file2md;
pub mod local_knowledge;
pub mod mcp;
pub mod models;
pub mod prompts;
pub mod session_plan;
pub mod screenshot;
pub mod settings;
pub mod skills; // Phase 2 重写：Agent Skill CRUD（基于文件系统）
pub mod system;
pub mod tools;
pub mod system_tray;
pub mod wsp;
mod fastplan_origins;
