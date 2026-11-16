//! 值级校验：回答里的具体值 → 逐个在证据全文里找 → 汇总判定。

use super::extract::{
    appears_in, extract_concrete_values, extract_count_claims, UNGROUNDED_RATIO_THRESHOLD,
};
use super::{ClaimTrace, EvidenceSource, GroundingManifest, GroundingState, ToolCall};

/// 入参摘要在台账里保留多少字符。
const QUERY_CAP: usize = 120;
/// 返回内容摘要在台账里保留多少字符（对齐 FastPlan `provenance.rs` 的口径）。
const EXCERPT_CAP: usize = 200;
/// 台账最多收录几次调用（超出部分不参与溯源，只计入日志）。
const MAX_SOURCES: usize = 20;
/// 最多追踪多少条 claim。
const MAX_CLAIMS: usize = 40;

/// 构建证据台账。
///
/// `calls` 是本轮**全部**工具调用（成功与失败都算）；`answer` 是最终回答正文。
///
/// 判定基于 `result` **全文**，台账里存的是截断后的 `excerpt`——两者不能混用，
/// 否则被截断掉的那半段里的数值会被误判成"无出处"。
pub fn build(answer: &str, calls: &[ToolCall]) -> GroundingManifest {
    let sources: Vec<EvidenceSource> = calls
        .iter()
        .take(MAX_SOURCES)
        .enumerate()
        .map(|(i, c)| EvidenceSource {
            index: i + 1,
            tool: c.tool.clone(),
            query: take_chars(&normalize_ws(&c.query), QUERY_CAP),
            excerpt: take_chars(&normalize_ws(&c.result), EXCERPT_CAP),
            success: c.success,
            error: c.error.as_deref().map(|e| take_chars(&normalize_ws(e), EXCERPT_CAP)),
        })
        .collect();

    // 计数断言优先（用户一眼会信的总数，必须排在前列，不能被展示上限截断），
    // 之后再补上通用具体值。两者用同一套判据，避免"清单说有来源、判定说无出处"。
    let mut values = extract_count_claims(answer);
    for v in extract_concrete_values(answer) {
        if !values.contains(&v) {
            values.push(v);
        }
    }

    let mut claims: Vec<ClaimTrace> = Vec::new();
    for v in values.into_iter().take(MAX_CLAIMS) {
        // 只有**成功**的调用能作为出处：失败调用的 result 是空串或错误信息，
        // 拿它当证据等于把"没查到"洗成"查到了"。
        let hits: Vec<usize> = calls
            .iter()
            .take(MAX_SOURCES)
            .enumerate()
            .filter(|(_, c)| c.success && appears_in(&v, &c.result))
            .map(|(i, _)| i + 1)
            .collect();
        claims.push(ClaimTrace { value: v, sources: hits });
    }

    let grounding = judge(&claims);

    GroundingManifest {
        sources,
        claims,
        grounding,
    }
}

/// 按"找不到的比例"给三档判定（阈值对齐 FastPlan）。
///
/// - 无具体值 → `Skipped`（没判定，不是判定通过）；
/// - 一个都对不上 → `Ungrounded`；
/// - 对不上的比例超阈值 → `Partial`；
/// - 否则 → `Grounded`。
fn judge(claims: &[ClaimTrace]) -> GroundingState {
    let total = claims.len();
    if total == 0 {
        return GroundingState::Skipped;
    }
    let found = claims.iter().filter(|c| c.is_grounded()).count();
    if found == 0 {
        return GroundingState::Ungrounded;
    }
    let not_found_ratio = (total - found) as f64 / total as f64;
    if not_found_ratio > UNGROUNDED_RATIO_THRESHOLD {
        GroundingState::Partial
    } else {
        GroundingState::Grounded
    }
}

fn take_chars(s: &str, n: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= n {
        t.to_string()
    } else {
        let mut out: String = t.chars().take(n).collect();
        out.push('…');
        out
    }
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ★ 2026-09-23 甲方现场的真实数据形态：编造的天气回答 vs 真实的工具返回。
    ///   两个城市相隔 800 公里却给出同一组数字，必须被判为编造。
    #[test]
    fn fabricated_weather_answer_is_ungrounded() {
        let real = "【北京市北京 天气】\n\n实况（2026-09-23T20:30）：阴\n  气温：28°C（体感 29.1°C）\n  \
                    湿度：51%\n  风速：9.1 km/h\n\n预报：\n  2026-09-23  阴  21.6~32.3°C  降水概率 2%";
        let fabricated = "天气状况：多云转晴\n气温：18℃ ~ 26℃\n湿度：55%\n风速：3-4 级\n降水概率：10%";

        let m = build(fabricated, &[ToolCall::ok("get_weather", r#"{"location":"北京"}"#, real)]);
        assert_eq!(
            m.grounding,
            GroundingState::Ungrounded,
            "编造数值不该在真实证据里命中: {:?}",
            m.claims
        );
        let unsourced = m.unsourced_values();
        assert!(unsourced.contains(&"18"), "18 应被列为无出处: {unsourced:?}");
        assert!(unsourced.contains(&"55"), "55 应被列为无出处: {unsourced:?}");
        assert!(m.has_successful_source(), "工具本身是成功的");
    }

    /// 反向：基于真实返回写的回答应当判为有据。
    #[test]
    fn answer_based_on_real_return_is_grounded() {
        let real = "【北京市北京 天气】\n\n实况（2026-09-23T20:30）：阴\n  气温：28°C（体感 29.1°C）\n  \
                    湿度：51%\n  风速：9.1 km/h";
        let answer = "北京今天阴，气温 28°C，体感 29°C，湿度 51%。";
        let m = build(answer, &[ToolCall::ok("get_weather", r#"{"location":"北京"}"#, real)]);
        assert_eq!(m.grounding, GroundingState::Grounded, "claims: {:?}", m.claims);
        assert!(m.annotation(true).is_none(), "有据可查不该加提示");
    }

    /// 回答里没有可校验的具体值 → 未判定，而不是判定通过。
    #[test]
    fn prose_without_values_is_skipped_not_grounded() {
        let m = build("今天天气不错，适合出门走走。", &[]);
        assert_eq!(m.grounding, GroundingState::Skipped);
        assert!(m.claims.is_empty());
    }

    /// 本该有数据却没有 → 必须提示"未经工具检索"，哪怕回答里没有数字。
    #[test]
    fn expected_live_data_without_tools_gets_disclaimer() {
        let m = build("今天天气不错，适合出门走走。", &[]);
        let note = m.annotation(true).expect("本该检索却一次没调，必须加提示");
        assert!(note.contains("未经工具检索"), "{note}");
    }

    /// ★ 反向闸门：不需要检索的对话**不该**被挂上提示。
    ///   否则「帮我写首诗」也带一句"未经检索"，用户很快会无视所有提示。
    #[test]
    fn chat_that_needs_no_retrieval_is_not_warned() {
        let m = build("床前明月光，疑是地上霜。", &[]);
        assert!(m.annotation(false).is_none(), "免工具对话不该有提示: {:?}", m.annotation(false));
    }

    /// ★ 派生值不该触发提示：模型基于证据算出来的增幅，本来就不在证据里。
    ///   只要对不上的没超过半数，就按有据处理。
    #[test]
    fn derived_value_among_grounded_ones_does_not_warn() {
        let real = r#"{"a":100,"b":200,"c":300}"#;
        // 100/200/300 都在证据里；15 是算出来的增幅
        let m = build("三项分别为 100、200、300，环比增长 15。", &[ToolCall::ok("q", "x", real)]);
        assert_eq!(m.grounding, GroundingState::Grounded, "claims: {:?}", m.claims);
        assert!(m.annotation(true).is_none(), "不该因为一个派生值就报可疑");
    }

    /// 部分对不上：证据只有一条，回答里混进三个编造值 → Partial。
    #[test]
    fn partially_fabricated_is_partial() {
        let real = r#"{"records":[{"id":1,"amount":100}]}"#;
        let m = build(
            "记录金额为 100 元，另有 999、888、777 三笔",
            &[ToolCall::ok("query", "x", real)],
        );
        assert_eq!(m.grounding, GroundingState::Partial, "claims: {:?}", m.claims);
        let unsourced = m.unsourced_values();
        assert_eq!(unsourced.len(), 3, "{unsourced:?}");
    }

    /// 失败的调用留在台账里（带错误原因），但**不能**作为出处。
    #[test]
    fn failed_calls_are_recorded_but_do_not_ground() {
        let calls = vec![ToolCall::failed("get_weather", r#"{"location":"北京"}"#, "连接超时")];
        let m = build("北京今天 28°C。", &calls);
        assert_eq!(m.sources.len(), 1);
        assert!(!m.sources[0].success);
        assert_eq!(m.sources[0].error.as_deref(), Some("连接超时"));
        assert!(!m.has_successful_source());
        assert_eq!(m.grounding, GroundingState::Ungrounded, "28 不该被认为有出处");
    }

    /// 放行判定用的三个查询：成功与否 / 指定工具成没成 / 哪些没成。
    #[test]
    fn required_tool_queries() {
        let calls = vec![ToolCall::ok("get_weather", "q", "【北京市北京 天气】28°C")];
        let m = build("气温 28°C", &calls);
        assert!(m.tool_succeeded("get_weather"));
        assert!(!m.tool_succeeded("get_exchange_rate"));
        let required = vec!["get_weather".to_string(), "get_exchange_rate".to_string()];
        assert_eq!(m.missing_tools(&required), vec!["get_exchange_rate"]);
    }

    /// 没有对应工具时，声明的工具应全部报缺——这是硬拦放行的判据。
    #[test]
    fn all_required_tools_missing_when_nothing_ran() {
        let m = build("气温 18℃", &[]);
        let required = vec!["get_weather".to_string()];
        assert_eq!(m.missing_tools(&required), vec!["get_weather"]);
    }

    /// 计数断言优先排在 claims 最前面（FastPlan 2026-09-03 回归）。
    #[test]
    fn count_claim_comes_first() {
        let calls = vec![ToolCall::ok(
            "query",
            "x",
            r#"{"rows":[{"cnt":2},{"task_id":100353262443561646}]}"#,
        )];
        let m = build("查询到共有58项任务，另有编号 100353262443561646。", &calls);
        assert_eq!(m.claims[0].value, "58", "计数断言必须排首位: {:?}", m.claims);
        assert!(!m.claims[0].is_grounded(), "58 不在证据里应标无出处");
    }

    /// 台账里的摘要要截断，但判定必须基于全文——被截断掉的值仍应算有出处。
    #[test]
    fn judgement_uses_full_text_not_truncated_excerpt() {
        let padding = "填充".repeat(200);
        let full = format!(r#"{{"pad":"{padding}","temp":28}}"#);
        let m = build("气温 28°C", &[ToolCall::ok("get_weather", "q", &full)]);
        assert!(
            m.sources[0].excerpt.chars().count() < full.chars().count(),
            "摘要应当被截断"
        );
        assert_eq!(m.grounding, GroundingState::Grounded, "截断不该影响判定");
    }

    /// 大面积对不上时，提示要逐个列出没有出处的值；有出处的值不能被列进去。
    #[test]
    fn annotation_lists_unsourced_values() {
        let real = r#"{"total":100}"#;
        let m = build(
            "总额 100 元，另有 999、888、777 元为预付款。",
            &[ToolCall::ok("q", "x", real)],
        );
        assert_eq!(m.grounding, GroundingState::Partial, "claims: {:?}", m.claims);
        let note = m.annotation(true).expect("大面积编造必须提示");
        assert!(note.contains("999"), "{note}");
        assert!(note.contains("888"), "{note}");
        assert!(!note.contains("100、"), "有出处的值不该被列进去: {note}");
    }
}
