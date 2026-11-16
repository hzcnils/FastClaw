// FastClaw · 系统时间强制注入模块
//
// 在所有 LLM 请求的 System Prompt 头部注入真实东八区北京时间，
// 防止模型使用训练截止时间或历史记忆中的错误日期。
//
// 设计要点：
//   - **固定使用 FixedOffset::east_opt(8*3600)**，不依赖系统 Local 时区
//   - 所有时间运算、格式化、存储统一用东八区
//   - 前置最高优先级约束，覆盖模型自带时间
//   - 内置防错乱校验规则

use chrono::{Datelike, FixedOffset, TimeZone, Timelike, Weekday};

/// 东八区时区偏移 (+08:00)
const CST_OFFSET_SECS: i32 = 8 * 3600;

/// 获取东八区固定偏移（全局唯一时区基准）
fn cst() -> FixedOffset {
    FixedOffset::east_opt(CST_OFFSET_SECS).expect("CST offset 8*3600 is always valid")
}

/// 获取当前东八区时间（带时区信息的 DateTime<FixedOffset>）
///
/// 实现：取系统 UTC 时间 → 转换为东八区时间。
/// 即使服务器系统时区设为 UTC，也会得到正确的北京时间。
pub fn cst_now() -> chrono::DateTime<FixedOffset> {
    let utc_now = chrono::Utc::now();
    cst().from_utc_datetime(&utc_now.naive_utc())
}

/// 获取标准北京时间字符串，如 "2026-06-30 星期二 14:30"
pub fn beijing_now() -> String {
    let now = cst_now();
    let weekday = weekday_cn(now.weekday());
    format!(
        "{:04}-{:02}-{:02} {} {:02}:{:02}",
        now.year(),
        now.month(),
        now.day(),
        weekday,
        now.hour(),
        now.minute(),
    )
}

/// 获取仅日期字符串，如 "2026-06-30 星期二"
pub fn beijing_today() -> String {
    let now = cst_now();
    let weekday = weekday_cn(now.weekday());
    format!(
        "{:04}-{:02}-{:02} {}",
        now.year(),
        now.month(),
        now.day(),
        weekday,
    )
}

/// 获取东八区 ISO-8601 格式字符串，用于存储
/// 如 "2026-06-30T14:30:00+08:00"
pub fn cst_iso_now() -> String {
    cst_now().to_rfc3339()
}

/// 将 UTC ISO-8601 时间字符串转换为东八区时间字符串
///
/// 输入: "2026-06-30T06:45:00Z" 或 "2026-06-30T06:45:00.123Z"
/// 输出: "2026-06-30 14:45"
pub fn utc_iso_to_cst_display(utc_str: &str) -> String {
    // 解析 UTC 时间
    let parsed = chrono::DateTime::parse_from_rfc3339(utc_str)
        .or_else(|_| {
            // 如果 parse_from_rfc3339 失败，尝试 NaiveDateTime
            chrono::NaiveDateTime::parse_from_str(utc_str, "%Y-%m-%dT%H:%M:%S%.fZ")
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(utc_str, "%Y-%m-%dT%H:%M:%SZ"))
                .map(|naive| chrono::Utc.from_utc_datetime(&naive).into())
        })
        .unwrap_or_else(|_| {
            // 最终降级：当作当前时间
            cst_now().into()
        });

    // 转为东八区
    let cst_time = parsed.with_timezone(&cst());
    cst_time.format("%Y-%m-%d %H:%M").to_string()
}

/// 递归转换 JSON 中的时间字段（created_at, updated_at, recorded_at）
/// 将所有 UTC 时间字符串替换为东八区显示格式
pub fn convert_json_timestamps_to_cst(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            // 转换已知时间字段
            for key in &["created_at", "updated_at", "recorded_at", "valid_from"] {
                if let Some(serde_json::Value::String(ts)) = map.get(*key) {
                    let display = utc_iso_to_cst_display(ts);
                    map.insert(key.to_string(), serde_json::Value::String(display));
                }
            }
            // 递归处理子对象
            for (_, v) in map.iter_mut() {
                convert_json_timestamps_to_cst(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                convert_json_timestamps_to_cst(v);
            }
        }
        _ => {}
    }
}

/// 中文星期
fn weekday_cn(w: Weekday) -> &'static str {
    match w {
        Weekday::Mon => "星期一",
        Weekday::Tue => "星期二",
        Weekday::Wed => "星期三",
        Weekday::Thu => "星期四",
        Weekday::Fri => "星期五",
        Weekday::Sat => "星期六",
        Weekday::Sun => "星期日",
    }
}

/// 生成 System Prompt 头部时间约束块（最高优先级）
///
/// 此块应放在 system prompt 的最开头，不可被后续内容覆盖。
pub fn build_time_constraint_block() -> String {
    let today = beijing_today();
    let now = beijing_now();

    format!(
        "\
=====【最高优先级硬性时间约束 — 禁止篡改或忽略】=====
1. 当前真实北京时间（东八区 UTC+8）：{now}
2. 标准今日日期：{today}
3. 时区：全程 Asia/Shanghai 东八区 (UTC+8)，禁止自动转换为 UTC 或任何其他时区。
4. 完全禁用模型内部训练截止时间、服务器虚拟时间、历史对话记忆中的过期错误日期。
   所有时间运算（年龄计算、日期差值、倒计时、年份对比、生日推算、节假日判断等）
   必须以第 2 条给出的「标准今日日期」作为唯一基准，不可使用任何其他日期来源。
5. 前置校验规则：若用户提供的生日/事件日期晚于标准今日日期，
   必须主动提示时间逻辑冲突，禁止直接输出「未出生」「未发生」等错误结论。
6. 计算输出规范：
   - 年龄计算精确到「X 年 Y 月 Z 天」，同时输出总间隔天数。
   - 日期差值为用户直接给出天数、周数、月数。
   - 倒计时标明剩余天数及目标日期。
7. 若用户询问「今天是什么日期」「现在是几月」「当前年份」等时间相关问题，
   只能使用第 2 条的「{today}」作答，禁止引用训练数据中的任何其他时间。
=====【时间约束结束 — 以下为业务 prompt】=====\n\n",
        now = now,
        today = today,
    )
}

/// 生成轻量版时间注入（用于非聊天场景：搜索重写、翻译、记忆提取等）
pub fn build_light_time_hint() -> String {
    let today = beijing_today();
    format!(
        "【时间基准：当前东八区北京时间为 {today}，时区 UTC+8。所有时间判断以此为准。】\n",
        today = today,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cst_now_is_east_8() {
        let now = cst_now();
        assert_eq!(now.offset().local_minus_utc(), CST_OFFSET_SECS);
    }

    #[test]
    fn beijing_now_has_expected_format() {
        let s = beijing_now();
        assert!(s.contains("星期"), "should contain Chinese weekday: {s}");
        assert!(s.len() >= 18, "too short: {s}");
    }

    #[test]
    fn beijing_today_has_expected_format() {
        let s = beijing_today();
        assert!(s.contains("星期"), "should contain Chinese weekday: {s}");
        assert!(!s.contains(":"), "should not contain time: {s}");
    }

    #[test]
    fn time_constraint_block_contains_key_info() {
        let block = build_time_constraint_block();
        assert!(block.contains("最高优先级"));
        assert!(block.contains("北京时间"));
        assert!(block.contains("东八区"));
        assert!(block.contains("UTC+8"));
        assert!(block.contains("标准今日日期"));
    }

    #[test]
    fn light_time_hint_is_compact() {
        let hint = build_light_time_hint();
        assert!(hint.contains("时间基准"));
        assert!(hint.contains("UTC+8"));
        assert!(
            hint.len() < 250,
            "light hint should be short: {}",
            hint.len()
        );
    }

    #[test]
    fn cst_iso_has_offset() {
        let iso = cst_iso_now();
        assert!(
            iso.contains("+08:00") || iso.contains("+0800"),
            "ISO should contain +08:00 offset: {iso}"
        );
    }
}
