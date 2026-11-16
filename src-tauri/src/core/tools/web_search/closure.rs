//! L9：迭代闭环（closure）。A5。
//!
//! 搜索内容集成一部分后，问大模型「required_slots 填满了吗？缺什么？下一步搜什么？」
//! 未闭环则用 `next_queries` 再搜一轮，有界迭代（硬上限 MAX_ROUNDS）。
//! 这是老板「反复迭代直到内容闭环」的落地：工具保证闭环，而非把循环甩给 agent。
//!
//! 设计约束：LLM 失败 / 解析失败 → 视为「已闭环」，交付已有证据，绝不卡死循环。

use crate::core::tools::web_search::search_planner::PlannerLlm;

/// 闭环判定结果。
#[derive(Debug, Clone, Default)]
pub struct ClosureEval {
    pub is_closed: bool,
    /// 未闭环时，下一轮要搜的搜索词（含 site: 定向）。
    pub next_queries: Vec<String>,
    /// 缺口描述（诊断/日志用）。
    pub gap: String,
}

/// 有界迭代的硬上限（PRD：最多 2~3 轮，防止对搜索引擎/LLM 造成压测）。
pub const MAX_ROUNDS: usize = 3;

/// 构建闭环评估 prompt。
pub fn build_eval_prompt(query: &str, evidence: &[String], slots: &[String]) -> String {
    let evidence_text = if evidence.is_empty() {
        "（本轮无证据）".to_string()
    } else {
        evidence
            .iter()
            .enumerate()
            .map(|(i, e)| format!("[{i}] {}", e.chars().take(300).collect::<String>()))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let slots_text = if slots.is_empty() {
        "（无明确闭环要素）".to_string()
    } else {
        slots.join("、")
    };

    format!(
        r#"你是情报检索闭环判定助手。判断当前证据是否已足够回答用户问题。

【用户问题】：{query}

【必须填满的闭环要素】：{slots_text}

【已集成的证据】：
{evidence_text}

请输出严格 JSON（不要解释、不要 markdown）：
{{
  "is_closed": <true/false，证据是否已足以回答且要素基本填满>,
  "gap": "<若未闭环，一句话说明缺什么；已闭环则为空字符串>",
  "next_queries": ["<若未闭环，下一步要搜的 1~3 个搜索词，含 site:域名 定向；已闭环则为空数组>"]
}}"#
    )
}

/// 解析闭环判定 JSON。
pub fn parse_eval(text: &str) -> Option<ClosureEval> {
    let t = text.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    let v: serde_json::Value = serde_json::from_str(t).ok()?;
    let is_closed = v.get("is_closed").and_then(|x| x.as_bool()).unwrap_or(true);
    let gap = v.get("gap").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let next_queries = v
        .get("next_queries")
        .and_then(|a| a.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| s.to_string()).collect())
        .unwrap_or_default();
    Some(ClosureEval { is_closed, next_queries, gap })
}

/// 异步闭环判定。LLM 失败返回 None（调用方视为已闭环）。
pub async fn evaluate(
    llm: &dyn PlannerLlm,
    query: &str,
    evidence: &[String],
    slots: &[String],
) -> Option<ClosureEval> {
    let system = "你输出严格 JSON，不要任何解释或 markdown。";
    let text = llm.complete(system, &build_eval_prompt(query, evidence, slots)).await?;
    parse_eval(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 解析未闭环() {
        let json = r#"{"is_closed":false,"gap":"缺少研究方向","next_queries":["何祖军 研究方向 site:just.edu.cn"]}"#;
        let e = parse_eval(json).unwrap();
        assert!(!e.is_closed);
        assert_eq!(e.gap, "缺少研究方向");
        assert_eq!(e.next_queries, vec!["何祖军 研究方向 site:just.edu.cn"]);
    }

    #[test]
    fn 解析已闭环() {
        let json = r#"{"is_closed":true,"gap":"","next_queries":[]}"#;
        let e = parse_eval(json).unwrap();
        assert!(e.is_closed);
        assert!(e.next_queries.is_empty());
    }

    #[test]
    fn 解析失败默认闭环() {
        assert!(parse_eval("垃圾").is_none());
        // is_closed 缺失时默认 true，宁可提前交付也不卡死
        let e = parse_eval(r#"{"next_queries":[]}"#).unwrap();
        assert!(e.is_closed);
    }

    #[test]
    fn prompt注入证据与要素() {
        let p = build_eval_prompt(
            "何祖军",
            &["何祖军是江苏科技大学教授".into()],
            &["所在单位".into()],
        );
        assert!(p.contains("何祖军是江苏科技大学教授"), "{p}");
        assert!(p.contains("所在单位"), "{p}");
    }

    struct MockLlm {
        answer: Option<String>,
    }
    #[async_trait::async_trait]
    impl PlannerLlm for MockLlm {
        async fn complete(&self, _s: &str, user: &str) -> Option<String> {
            assert!(user.contains("何祖军"), "{user}");
            self.answer.clone()
        }
    }

    #[tokio::test]
    async fn 评估成功() {
        let llm = MockLlm {
            answer: Some(r#"{"is_closed":false,"gap":"缺","next_queries":["q"]}"#.into()),
        };
        let e = evaluate(&llm, "何祖军", &["e".into()], &["s".into()]).await.unwrap();
        assert!(!e.is_closed);
    }

    #[tokio::test]
    async fn llm失败返回none() {
        let llm = MockLlm { answer: None };
        assert!(evaluate(&llm, "何祖军", &[], &[]).await.is_none());
    }
}
