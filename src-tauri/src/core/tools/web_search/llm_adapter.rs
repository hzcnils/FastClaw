//! `PlannerLlm` 适配器：把 fastclaw 的 `LlmManager` 包装成
//! `web_search::search_planner::PlannerLlm`，恢复 L0.5 LLM 搜索扩展 / LLM 推荐站点 / deep 闭环。
//!
//! 设计约束：LLM 不可用 / 超时 / 返回空 → 返回 None，调用方静默退回确定式 plan，绝不 panic。

use std::sync::Arc;
use std::time::Duration;

use crate::core::llm::LlmManager;
use crate::core::tools::web_search::search_planner::PlannerLlm;

/// 单次 LLM 调用超时：搜索扩展 / 推荐站点 / 闭环评估都走这条支路，绝不让它阻塞主链路。
const LLM_TIMEOUT: Duration = Duration::from_secs(8);

/// 把 `Arc<LlmManager>` 包装成 `PlannerLlm`。
///
/// model_id 传 None 时使用数据库里的默认模型（`is_default = 1`）。
#[derive(Clone)]
pub struct LlmManagerPlannerLlm {
    pub llm: Arc<LlmManager>,
}

impl LlmManagerPlannerLlm {
    pub fn new(llm: Arc<LlmManager>) -> Self {
        Self { llm }
    }
}

#[async_trait::async_trait]
impl PlannerLlm for LlmManagerPlannerLlm {
    async fn complete(&self, system: &str, user: &str) -> Option<String> {
        // LlmManager::chat 内部走 /chat/completions（stream 形式），此处只取最终 content。
        let fut = self.llm.chat(None, system, &[], user);
        match tokio::time::timeout(LLM_TIMEOUT, fut).await {
            Ok(Ok(resp)) => {
                let c = resp.content.trim().to_string();
                if c.is_empty() {
                    None
                } else {
                    Some(c)
                }
            }
            Ok(Err(e)) => {
                tracing::debug!(error = %e, "jmis_search LLM 扩展失败，退回确定式 plan");
                None
            }
            Err(_) => {
                tracing::debug!("jmis_search LLM 扩展超时，退回确定式 plan");
                None
            }
        }
    }
}
