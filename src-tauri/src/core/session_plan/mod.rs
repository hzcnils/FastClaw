//! FastClaw · 会话规划桥接层
//!
//! 桌面端「任务规划」开关、执行模式、规划确认（confirm / resume / cancel）
//! 与 ACP 会话事件的共享实现。

pub mod config;
pub mod fastplan;
pub mod handler;
pub mod plan_control;
pub mod protocol;
