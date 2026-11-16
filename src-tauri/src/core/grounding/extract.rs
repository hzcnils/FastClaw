//! 从文本里抽取「明确的具体值」，并判定它能否在某段证据文本里命中。
//!
//! 移植自 FastPlan `src/executor/grounding.rs`，方法完全一致：**确定性的正则抽取 +
//! 子串包含判定，绝不叫 LLM 判断** —— 那是用一个可能编造的东西去检查另一个可能
//! 编造的东西。
//!
//! # 与 FastPlan 的两处定向收紧（不是随手改的）
//!
//! FastPlan 的主力数据是合同编号、金额、日期这类**长标识符**；FastClaw 的实时数据
//! 是天气、汇率、股价这类**短数值**（`18`、`26`、`9.1`）。同一套判据放到短数值上会
//! 大量误命中，两个方向都出过问题：
//!
//! 1. **单字符值一律不抽**（见 [`count_digits`]）。`风速 3-4 级` 里的 `3`/`4` 会
//!    在几乎任何证据文本里撞上（`32.3°C`、`20:30`…）。单个数字本身不构成
//!    「有出处的数据」这一主张。注意**计数断言不受此限**——`共 3 条` 里的 `3`
//!    由 [`extract_count_claims`] 单独抽取，那里它是有意义的。
//! 2. **数值要求数字边界**（见 [`contains_at_boundary`]）。`26` 不该因为在
//!    `2026-09-23` 里作为子串出现就算「找到了」。
//!
//! 两处都只让判定**更严**：更严的代价是可能把真话标成「未经检索」（无害，只是多一句
//! 提示），反方向的代价是编造数据被放过（有害）。方向性取舍在这里是明确的。

use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::Regex;

/// 找不到的具体值最多保留几条（供 UI 展示"哪些值没有出处"）。
pub const MAX_SAMPLES: usize = 5;
/// 找不到的比例超过该阈值即降级（对齐 FastPlan `grounding.rs` 的 50%）。
pub const UNGROUNDED_RATIO_THRESHOLD: f64 = 0.5;

/// 抽取"具体值"的组合正则。
///
/// 交替顺序有讲究：`regex` 是 leftmost-first，date/code 排在 number 之前，
/// 这样 `2024-01-31`、`HT-2024-001` 会被整体消费成**一个**值，内部的
/// `2024`/`01`/`31` 不会再被拆出来重复计数。
///
/// - `date`：ISO（`-`/`/`）、中文（`2024年1月31日`）、日/月/年（斜杠）。
/// - `code`：含分隔符（`-`/`_`）且以字母开头的代码式标识符；再叠加"至少两位数字"
///   的过滤（在 [`extract_concrete_values`] 里做），排除 `GPT-4`
///   这类英文连词/型号，只留 `HT-2024-001` 这类明确的具体 ID。
/// - `number`：整数/小数/千分位数字（`123`、`1,234`、`3.14`）。
static VALUE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?P<date>\d{4}[-/]\d{1,2}[-/]\d{1,2}|\d{4}年\d{1,2}月\d{1,2}日?|\d{1,2}/\d{1,2}/\d{2,4})|(?P<code>[A-Za-z][A-Za-z0-9]*(?:[-_][A-Za-z0-9]+)+)|(?P<number>\d+(?:[.,]\d+)*)",
    )
    .expect("grounding VALUE_RE 必须合法")
});

/// 从结论文本里抽取"明确的具体值"（数值、日期、代码式标识符），去重后按字典序
/// 返回（字典序保证输出确定性，便于测试与前后端比对）。
pub fn extract_concrete_values(text: &str) -> Vec<String> {
    let mut vals: BTreeSet<String> = BTreeSet::new();
    for caps in VALUE_RE.captures_iter(text) {
        let matched = caps
            .name("date")
            .or_else(|| caps.name("code"))
            .or_else(|| caps.name("number"));
        let Some(m) = matched else { continue };
        let s = m.as_str().trim();
        if s.is_empty() {
            continue;
        }
        // ★ 裸年份是时间标签，不是数据值（FastPlan 2026-09-19 V23 的教训：标题里写
        //   《2024 年度销售工作总结》，`2024` 成了唯一的"具体值"，而任何工具返回里都
        //   不会有它 → 误判编造）。年份来自用户请求本身，不该要求工具返回里必须有。
        //   完整日期（`2024-01-31`）走 date 分支、长度不是 4，不受影响。
        if is_year_label(s) {
            continue;
        }
        // ★ FastClaw 收紧：单个数字不作为"具体值"参与校验（见模块头）。
        if count_digits(s) < 2 {
            continue;
        }
        vals.insert(s.to_string());
    }
    vals.into_iter().collect()
}

/// 提取结论里的「计数断言」（`共/共计/总共有/总计` 语境下紧跟数字与量词的短语）。
///
/// 这类断言必须**优先**进入溯源展示：它们是用户一眼会信的总数，若与证据不符
/// （如证据里 76 行、结论写「共 58 项」）必须被标出来，而不能因为通用具体值抽取
/// 按字典序排到了几十位之外、被展示上限截断而逃过标注（FastPlan 2026-09-03 实测）。
///
/// 也正因为走这条路，**单字符数字在这里是有意义的**——`共 3 条` 的 `3` 是真实主张，
/// 不受 [`extract_concrete_values`] 的单字符过滤影响。
pub fn extract_count_claims(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // 定位「共/共计/总共有/总计」语境（长前缀优先，避免「共计」被「共」抢先消费）
        let mut head_end = None;
        for prefix in ["共计", "总共有", "共有", "总计", "共"] {
            if bytes[i..].starts_with(prefix.as_bytes()) {
                head_end = Some(i + prefix.len());
                break;
            }
        }
        let Some(mut j) = head_end else {
            i += 1;
            continue;
        };
        // 跳过空白后必须紧跟数字
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() || !bytes[j].is_ascii_digit() {
            i += 1;
            continue;
        }
        let num_start = j;
        while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == b',') {
            j += 1;
        }
        // 数字后必须是量词（项/条/个/人/笔/份/位/张/次），避免「共 X 的 Y」语境误伤
        let mut k = j;
        while k < bytes.len() && bytes[k].is_ascii_whitespace() {
            k += 1;
        }
        let unit = ["项", "条", "个", "人", "笔", "份", "位", "张", "次"]
            .iter()
            .find(|u| bytes[k..].starts_with(u.as_bytes()));
        let Some(u) = unit else {
            i += 1;
            continue;
        };
        let raw = &text[num_start..j];
        if !out.contains(&raw.to_string()) {
            out.push(raw.to_string());
        }
        i = k + u.len();
    }
    out
}

/// 判定 `value`（结论里抽出的具体值）是否能在 `haystack`（工具返回文本）里命中。
///
/// 先按原文匹配，失败后做千分位归一化再匹配一次（结论写 `1,234`、工具返回 `1234`）。
pub fn appears_in(value: &str, haystack: &str) -> bool {
    if value.is_empty() || value.len() > haystack.len() {
        return false;
    }
    if contains_at_boundary(value, haystack) {
        return true;
    }
    if value.contains(',') {
        let stripped: String = value.chars().filter(|c| *c != ',').collect();
        if !stripped.is_empty() && contains_at_boundary(&stripped, haystack) {
            return true;
        }
    }
    false
}

/// 子串命中，但对**纯数值**额外要求它不是一个更大数字的碎片。
///
/// 没有这道检查时，`26` 会在 `2026-09-23` 里"找到"，`3` 会在 `32.3` 里"找到"——
/// 天气场景下几乎任何两位数都能在证据文本里撞上，校验就形同虚设。
///
/// 边界规则**刻意是不对称的**：
///
/// - 前一个字符不能是数字、`.`、`,` —— 命中不得是某个数的**中段或尾段**
///   （`3` 不该匹配 `32.3` 末尾那个 3，`26` 不该匹配 `2026` 里的 26）。
/// - 后一个字符只要不是数字 —— 命中**可以**是某个更精确数值的前缀
///   （结论里的 `29` 对得上证据里的 `29.1`，那是四舍五入，完全正当）。
///
/// 换句话说：允许"结论比证据粗"，不允许"结论里的数是证据里某个数切下来的一块"。
fn contains_at_boundary(needle: &str, haystack: &str) -> bool {
    if !is_numeric(needle) {
        // 日期 `2024-01-31`、编号 `HT-2024-001` 这类带结构的值，子串命中即可
        return haystack.contains(needle);
    }
    // match_indices 只会在合法 UTF-8 字符边界上给出命中，下面按字节取相邻字符做
    // 边界判断是安全的（UTF-8 续字节永远不是 ASCII 数字，判为"非数字/非分隔符"
    // 正是想要的结果）
    let hb = haystack.as_bytes();
    haystack.match_indices(needle).any(|(start, _)| {
        let end = start + needle.len();
        let before_ok = start == 0 || !is_numeric_byte(hb[start - 1]);
        let after_ok = end >= hb.len() || !hb[end].is_ascii_digit();
        before_ok && after_ok
    })
}

/// 数字 token 的构成字节（数字、千分位逗号、小数点）。
fn is_numeric_byte(b: u8) -> bool {
    b.is_ascii_digit() || b == b',' || b == b'.'
}

/// 纯数值（只由 ASCII 数字、`,`、`.` 组成且至少含一位数字）——只有这类才需要数字边界。
fn is_numeric(s: &str) -> bool {
    s.bytes().any(|b| b.is_ascii_digit())
        && s.bytes()
            .all(|b| b.is_ascii_digit() || b == b',' || b == b'.')
}

/// 数字字符个数。`9.1` → 2，`3` → 1。
fn count_digits(s: &str) -> usize {
    s.bytes().filter(|b| b.is_ascii_digit()).count()
}

/// 裸年份（`2024`）——时间标签，不参与接地判定。
///
/// 只收 1900..=2100 的**四位纯数字**：完整日期（`2024-01-31`、`2024年1月31日`）
/// 走 date 分支、长度不为 4，不受影响。
fn is_year_label(s: &str) -> bool {
    s.len() == 4
        && s.bytes().all(|b| b.is_ascii_digit())
        && (1900..=2100).contains(&s.parse::<u32>().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 编造数值绝不应在工具返回里命中。
    ///
    /// 注意 `2000` **不在**抽取结果里：它落在年份区间，被 [`is_year_label`] 当成时间标签
    /// 豁免了（沿用 FastPlan 的取舍——那条规则救的是「标题写着 2024，就被要求工具返回里
    /// 必须也有 2024」这类系统性误杀）。代价是 1900~2100 的四位金额会漏检；
    /// 漏检的方向是"少判一个数"，而不是"把编造的判成有据"，可接受。
    #[test]
    fn fabricated_numbers_are_not_found() {
        let tool = r#"{"records":[{"id":1,"amount":100},{"id":2,"amount":200},{"id":3,"amount":300}]}"#;
        let vals = extract_concrete_values("共找到 5 条记录，金额分别为 1000、2000、3000、4000、5000 元");
        assert!(vals.len() >= 4, "应抽出至少 4 个具体数值, 实际 {vals:?}");
        assert!(
            !vals.contains(&"2000".to_string()),
            "2000 落在年份区间会被豁免: {vals:?}"
        );
        assert!(
            vals.iter().all(|v| !appears_in(v, tool)),
            "编造数值不应命中: {vals:?}"
        );
    }

    /// 有出处的数值应当命中。
    #[test]
    fn grounded_numbers_are_found() {
        let tool = r#"{"records":[{"id":1,"amount":100},{"id":2,"amount":200},{"id":3,"amount":300}]}"#;
        for v in ["100", "200", "300"] {
            assert!(appears_in(v, tool), "{v} 应命中");
        }
    }

    /// ★ 短数值场景的核心防线：子串不算命中。
    #[test]
    fn numeric_match_requires_digit_boundary() {
        let tool = "实况（2026-09-23T20:30）气温：32.3°C";
        // `26` 是 `2026` 的子串，但不是它自己的一个数
        assert!(!appears_in("26", tool), "26 不该因为在 2026 里出现就算命中");
        // `3` 是 `32.3` / `20:30` 的子串
        assert!(!appears_in("3", "32.3"));
        // 真正的独立出现仍然命中
        assert!(appears_in("32", tool));
        assert!(appears_in("30", tool));
    }

    /// 但 `.` 不算边界阻断：结论里的 29 应当匹配证据里的 29.1。
    #[test]
    fn decimal_prefix_still_matches() {
        assert!(appears_in("29", "体感 29.1°C"));
    }

    /// ★ 单个数字不参与校验：`3-4 级` 的 3 和 4 会被丢弃。
    #[test]
    fn single_digit_values_are_not_extracted() {
        let vals = extract_concrete_values("风速：3-4 级，降水概率：10%");
        assert_eq!(vals, vec!["10".to_string()], "只应保留两位数: {vals:?}");
    }

    /// 但小数不受影响：`9.1` 有两位数字，仍是具体值。
    #[test]
    fn small_decimals_are_still_extracted() {
        assert_eq!(extract_concrete_values("风速 9.1 km/h"), vec!["9.1".to_string()]);
    }

    /// 计数断言里的单字符数字必须保留（`共 3 条` 的 3 是真实主张）。
    #[test]
    fn count_claims_keep_single_digits() {
        assert_eq!(extract_count_claims("共 3 条记录"), vec!["3".to_string()]);
        assert_eq!(extract_count_claims("共计 58 项任务"), vec!["58".to_string()]);
        assert_eq!(extract_count_claims("总共有 7 人"), vec!["7".to_string()]);
        // 无 `共` 语境 / 数字后无量词 → 不抽
        assert!(extract_count_claims("已有 3 条记录").is_empty());
        assert!(extract_count_claims("共 3 天").is_empty());
    }

    /// 裸年份是时间标签，不参与接地判定（FastPlan V23 误杀回归）。
    #[test]
    fn bare_years_are_time_labels_not_data_values() {
        let vals = extract_concrete_values("《2024 年度销售工作总结》已完成");
        assert!(vals.is_empty(), "年份不该被当作待校验的具体值: {vals:?}");
        // 年份豁免不得连带放过真正的数据值
        let vals2 = extract_concrete_values("2024 年销售额 8800 万元");
        assert_eq!(vals2, vec!["8800".to_string()]);
        // 完整日期仍照常校验（走 date 分支）
        assert_eq!(
            extract_concrete_values("统计截止 2024-01-31"),
            vec!["2024-01-31".to_string()]
        );
    }

    /// 代码式实体 ID：grounded 的命中，编造的不命中；`GPT-4` 这类单数字型号不抽。
    #[test]
    fn code_identifiers_are_checked() {
        let tool = r#"{"contract":"HT-2024-001"}"#;
        assert!(appears_in("HT-2024-001", tool));
        assert!(!appears_in("HT-2024-999", tool));
        assert!(extract_concrete_values("使用 GPT-4 生成").is_empty());
    }

    /// 千分位归一化：结论写 `1,234,567`，工具返回 `1234567` → 命中。
    #[test]
    fn thousands_separator_is_normalized() {
        assert!(appears_in("1,234,567", r#"{"total":1234567}"#));
    }

    /// 中文日期被整体消费成一个值，不拆成 2024/1/31 三个数。
    #[test]
    fn chinese_date_is_a_single_value() {
        assert_eq!(
            extract_concrete_values("发布日期 2024年1月31日"),
            vec!["2024年1月31日".to_string()]
        );
    }
}
