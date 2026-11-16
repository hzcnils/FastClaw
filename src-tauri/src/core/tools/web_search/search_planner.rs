//! L0.5：LLM 搜索扩展（planner）。
//!
//! 在确定式 `query::plan` 之上，问大模型「这个领域该搜哪些站点、构建哪些搜索词、
//! 必须填满哪些要素（required_slots）」，把垂域站点元数据注入 prompt。
//! 产出的 `extra_queries` 并入检索计划，`required_slots` 供迭代闭环（A5）判定是否闭环。
//!
//! 设计约束：未配置 LLM / 调用失败 / 解析失败 → 静默退回确定式 plan，绝不阻断检索。
//! 纯逻辑（build_prompt / parse_expansion / should_expand）与 LLM 调用分离，可 mock 单测。

use crate::core::tools::web_search::domains::Site;

// ★ FastClaw 适配：删除 `AnthropicPlannerLlm`（依赖 claurst_api::AnthropicClient，
//   fastclaw 无此 crate）。保留 PlannerLlm trait + default_model + 纯逻辑
//   （build_prompt / parse_expansion / should_expand / expand）。后续接入
//   fastclaw 的 LlmManager 时，实现一个 PlannerLlm 适配器即可恢复 LLM 扩展。

#[derive(Debug, Clone, Default)]
pub struct ExpansionPlan {
    /// 追加的搜索词（含 `site:域名` 定向）。
    pub extra_queries: Vec<String>,
    /// LLM 挑中的垂域站点（本版只透传，供诊断/后续路由）。
    pub target_sites: Vec<String>,
    /// 该问题必须填满的闭环要素（供 A5 闭环判定）。
    pub required_slots: Vec<String>,
}

/// LLM 完成接口（可 mock）。
#[async_trait::async_trait]
pub trait PlannerLlm: Send + Sync {
    async fn complete(&self, system: &str, user: &str) -> Option<String>;
}

/// 扩展用的模型，默认 haiku 类（快、便宜），`JMIS_PLANNER_MODEL` 可覆盖。
/// 保留：后续接入 fastclaw LlmManager 时据此选择模型。
pub fn default_model() -> String {
    std::env::var("JMIS_PLANNER_MODEL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "claude-haiku-4-5".into())
}

/// 构建搜索扩展 prompt，注入垂域站点元数据。
pub fn build_prompt(query: &str, sites: &[Site]) -> String {
    let site_lines: String = sites
        .iter()
        .map(|s| format!("- {} [{}] 权重 {}", s.domain, s.categories.join(","), s.weight))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"你是资深情报与搜索规划专家。针对用户问题做搜索扩展与站点分析。

【系统已知垂域/内网站点元数据】：
{site_lines}

【用户问题】：{query}

请输出严格 JSON（不要任何解释、不要 markdown 代码块、不要尾逗号）：
{{
  "target_sites": ["<从上述元数据挑最匹配的站点域名，没有则空数组>"],
  "search_queries": ["<扩展出的搜索词 2~4 条，含 site:域名 定向>"],
  "required_slots": ["<该问题必须填满的闭环要素，如 所在单位/现任职务/研究方向>"]
}}"#
    )
}

/// 解析 LLM 返回的 JSON。容忍 ```json 代码块与前后噪声；解析不出查询/站点则返回 None。
pub fn parse_expansion(text: &str) -> Option<ExpansionPlan> {
    let t = strip_code_fence(text);
    let v: serde_json::Value = serde_json::from_str(t).ok()?;
    let mut plan = ExpansionPlan::default();
    if let Some(arr) = v.get("search_queries").and_then(|a| a.as_array()) {
        plan.extra_queries = arr.iter().filter_map(|x| x.as_str()).map(|s| s.to_string()).collect();
    }
    if let Some(arr) = v.get("target_sites").and_then(|a| a.as_array()) {
        plan.target_sites = arr.iter().filter_map(|x| x.as_str()).map(|s| s.to_string()).collect();
    }
    if let Some(arr) = v.get("required_slots").and_then(|a| a.as_array()) {
        plan.required_slots = arr.iter().filter_map(|x| x.as_str()).map(|s| s.to_string()).collect();
    }
    if plan.extra_queries.is_empty() && plan.target_sites.is_empty() {
        return None;
    }
    Some(plan)
}

fn strip_code_fence(text: &str) -> &str {
    text.trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
}

/// 是否值得触发 LLM 扩展：极短 / 已有 site: 限定 / 空查询跳过，省 token 省延迟。
pub fn should_expand(query: &str) -> bool {
    let q = query.trim();
    !q.is_empty() && q.chars().count() >= 4 && !q.contains("site:")
}

/// 异步扩展入口。LLM 失败 / 解析失败返回 None（调用方退回确定式 plan）。
pub async fn expand(llm: &dyn PlannerLlm, query: &str, sites: &[Site]) -> Option<ExpansionPlan> {
    if !should_expand(query) {
        return None;
    }
    let system = "你输出严格 JSON，不要任何解释或 markdown。";
    let text = llm.complete(system, &build_prompt(query, sites)).await?;
    parse_expansion(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_sites() -> Vec<Site> {
        vec![
            Site { domain: "afeu.edu.cn".into(), categories: vec!["military".into()], weight: 0.9 },
            Site { domain: "aiqicha.baidu.com".into(), categories: vec!["enterprise".into()], weight: 0.55 },
        ]
    }

    #[test]
    fn prompt注入站点与查询() {
        let p = build_prompt("何祖军 江苏科技大学", &sample_sites());
        assert!(p.contains("afeu.edu.cn"), "{p}");
        assert!(p.contains("何祖军 江苏科技大学"), "{p}");
        assert!(p.contains("required_slots"), "{p}");
    }

    #[test]
    fn 解析标准json() {
        let json = r#"{"target_sites":["afeu.edu.cn"],"search_queries":["何祖军 江苏科技大学 简介"],"required_slots":["所在单位"]}"#;
        let plan = parse_expansion(json).unwrap();
        assert_eq!(plan.extra_queries, vec!["何祖军 江苏科技大学 简介"]);
        assert_eq!(plan.required_slots, vec!["所在单位"]);
    }

    #[test]
    fn 解析带代码块包裹的json() {
        let json = "```json\n{\"search_queries\":[\"a b\"],\"required_slots\":[]}\n```";
        let plan = parse_expansion(json).unwrap();
        assert_eq!(plan.extra_queries, vec!["a b"]);
    }

    #[test]
    fn 解析失败返回none() {
        assert!(parse_expansion("这不是 JSON").is_none());
        assert!(parse_expansion(r#"{"foo":1}"#).is_none());
    }

    #[test]
    fn 短查询不扩展() {
        assert!(!should_expand("abc"));
        assert!(!should_expand("何祖军 site:just.edu.cn"));
        assert!(should_expand("何祖军 江苏科技大学 教授"));
    }

    struct MockLlm {
        answer: Option<String>,
    }
    #[async_trait::async_trait]
    impl PlannerLlm for MockLlm {
        async fn complete(&self, _s: &str, user: &str) -> Option<String> {
            assert!(user.contains("何祖军"), "prompt 应含 query：{user}");
            self.answer.clone()
        }
    }

    #[tokio::test]
    async fn 扩展成功并入() {
        let llm = MockLlm {
            answer: Some(r#"{"search_queries":["何祖军 简介"],"required_slots":["单位"]}"#.into()),
        };
        let plan = expand(&llm, "何祖军 江苏科技大学", &sample_sites()).await.unwrap();
        assert_eq!(plan.extra_queries, vec!["何祖军 简介"]);
    }

    #[tokio::test]
    async fn llm失败退回none() {
        let llm = MockLlm { answer: None };
        assert!(expand(&llm, "何祖军 江苏科技大学", &sample_sites()).await.is_none());
    }
}
