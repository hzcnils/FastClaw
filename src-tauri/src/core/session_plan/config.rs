//! 桌面端 UI 模型 / 思考开关（跨模块共享）
//!
//! ChatView 写入当前选中的模型与思考开关，供会话侧栏与规划链路读取。

use std::sync::RwLock;

/// 桌面当前选中的模型（ChatView 写入）
static CURRENT_UI_MODEL: RwLock<Option<String>> = RwLock::new(None);
static CURRENT_UI_THINKING: RwLock<bool> = RwLock::new(false);

pub fn set_current_ui_model(model_id: Option<String>) {
    if let Ok(mut g) = CURRENT_UI_MODEL.write() {
        *g = model_id.filter(|s| !s.trim().is_empty());
    }
}

pub fn current_ui_model() -> Option<String> {
    CURRENT_UI_MODEL.read().ok().and_then(|g| g.clone())
}

pub fn set_current_ui_thinking(enabled: bool) {
    if let Ok(mut g) = CURRENT_UI_THINKING.write() {
        *g = enabled;
    }
}

