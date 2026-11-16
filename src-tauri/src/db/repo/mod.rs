// FastClaw · 数据库仓库层（Repository）
// 每个子模块对应一组数据库表的 CRUD 操作
// 所有方法使用 sqlx 的编译时类型检查

pub mod agents;
pub mod assistants;
pub mod clips;
pub mod cloud_providers;
pub mod conversations;
pub mod conversation_branch;
pub mod conversation_deletion;
pub mod cost;
pub mod mcp;
pub mod message_favorites;
pub mod message_deletion;
pub mod message_translation;
pub mod messages;
pub mod model_feedback;
pub mod models;
pub mod plan;
pub mod prompts;
pub mod quality_feedback;
pub mod settings;
// pub mod skills; // Phase 1 移除 ES；Agent Skills 用文件存储而非 DB
pub mod tools;
pub mod wsp;
