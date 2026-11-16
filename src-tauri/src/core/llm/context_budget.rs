// FastClaw · 上下文预算与压缩
//
// 问题背景：
//   工具调用循环里的 `extra_messages` 只增不减、不截断、不摘要，
//   第 N 轮的请求包含第 1..N-1 轮所有工具的**完整原始输出**，
//   且每轮全量重发。配合 web_fetch 原来 5 万字符的上限，
//   一个「今天西安天气」的查询就烧掉了 121,520 token。
//
// 本模块提供三层防线：
//   ① 单条工具结果硬上限（防单点爆炸）
//   ② 历史轮次工具结果降级为摘要（防累积爆炸）
//   ③ 请求级 token 预算熔断（最后兜底）

use serde_json::Value;

/// 粗略估算 token 数。
///
/// 中文约 1 token/字，英文约 1 token/4 字符。这里按混合文本取
/// "字符数 / 2.2" 的保守估计——宁可高估触发压缩，也不要低估爆预算。
/// 精确计数需要 tokenizer，代价不值得。
pub fn estimate_tokens(s: &str) -> usize {
    let chars = s.chars().count();
    let ascii = s.chars().filter(|c| c.is_ascii()).count();
    let cjk = chars - ascii;
    // ASCII 约 4 字符/token，CJK 约 1 字符/token
    ascii / 4 + cjk + 1
}

/// 单条工具结果注入上下文的硬上限（字符）。
pub const MAX_TOOL_RESULT_CHARS: usize = 6_000;

/// 历史轮次（非最近一轮）工具结果压缩后的上限（字符）。
pub const MAX_STALE_TOOL_RESULT_CHARS: usize = 800;

/// 单次请求 extra_messages 的总 token 预算。
/// 超过后从最旧的开始压缩，仍超则丢弃最旧的轮次。
pub const MAX_EXTRA_MESSAGES_TOKENS: usize = 24_000;

/// 保持完整（不压缩）的最近轮次数。
/// 模型主要依赖最近一轮的工具结果作答，更早的只需要知道"做过什么"。
const KEEP_FULL_RECENT_MESSAGES: usize = 4;

/// 截断单条工具结果，附带明确的截断说明。
pub fn truncate_tool_result(content: &str, max_chars: usize) -> String {
    let total = content.chars().count();
    if total <= max_chars {
        return content.to_string();
    }
    let head: String = content.chars().take(max_chars).collect();
    format!("{head}\n...[工具输出已截断：原始 {total} 字符，此处保留前 {max_chars} 字符]")
}

/// 把一条历史工具消息压缩成摘要形式。
fn compact_tool_message(msg: &Value) -> Value {
    let mut out = msg.clone();
    if out.get("role").and_then(|v| v.as_str()) == Some("tool") {
        if let Some(content) = out.get("content").and_then(|v| v.as_str()) {
            let compacted = truncate_tool_result(content, MAX_STALE_TOOL_RESULT_CHARS);
            out["content"] = Value::String(compacted);
        }
    }
    // assistant 的 tool_calls 消息本身很短，且删掉会破坏 tool_call_id 配对，保持原样
    out
}

/// 对 extra_messages 做预算治理。
///
/// 策略（按代价从低到高）：
///   1. 若总量在预算内 → 原样返回
///   2. 压缩「较早轮次」的 tool 结果为摘要，保留最近若干条完整
///   3. 仍超预算 → 从最旧开始成对丢弃，并插入一条说明，让模型知道上下文被裁过
///
/// 关键约束：**绝不能破坏 assistant(tool_calls) 与 tool(tool_call_id) 的配对**，
/// 否则 OpenAI 兼容接口会直接 400。因此丢弃时以 assistant 消息为边界成组丢弃。
pub fn govern_extra_messages(extra: &[Value]) -> Vec<Value> {
    if extra.is_empty() {
        return Vec::new();
    }

    let total: usize = extra.iter().map(|m| estimate_tokens(&m.to_string())).sum();

    if total <= MAX_EXTRA_MESSAGES_TOKENS {
        return extra.to_vec();
    }

    // ── 第 2 层：压缩较早的 tool 结果 ──
    let split = extra.len().saturating_sub(KEEP_FULL_RECENT_MESSAGES);
    let mut governed: Vec<Value> = Vec::with_capacity(extra.len());
    for (i, msg) in extra.iter().enumerate() {
        if i < split {
            governed.push(compact_tool_message(msg));
        } else {
            governed.push(msg.clone());
        }
    }

    let after_compact: usize = governed
        .iter()
        .map(|m| estimate_tokens(&m.to_string()))
        .sum();

    if after_compact <= MAX_EXTRA_MESSAGES_TOKENS {
        tracing::info!(
            "🗜️ 上下文压缩：{} → {} tokens（压缩了 {} 条历史工具结果）",
            total,
            after_compact,
            split
        );
        return governed;
    }

    // ── 第 3 层：从最旧开始成组丢弃 ──
    // 以 assistant 消息为组边界：一个 assistant(tool_calls) + 其后连续的 tool 消息 = 一组
    let mut groups: Vec<Vec<Value>> = Vec::new();
    for msg in governed {
        let is_assistant = msg.get("role").and_then(|v| v.as_str()) == Some("assistant");
        if is_assistant || groups.is_empty() {
            groups.push(vec![msg]);
        } else {
            groups.last_mut().unwrap().push(msg);
        }
    }

    // 从最旧的组开始丢，直到进入预算
    let mut dropped = 0usize;
    while groups.len() > 1 {
        let cur: usize = groups
            .iter()
            .flatten()
            .map(|m| estimate_tokens(&m.to_string()))
            .sum();
        if cur <= MAX_EXTRA_MESSAGES_TOKENS {
            break;
        }
        groups.remove(0);
        dropped += 1;
    }

    let mut result: Vec<Value> = groups.into_iter().flatten().collect();

    if dropped > 0 {
        tracing::warn!(
            "🗜️ 上下文超预算，丢弃了最早的 {} 轮工具调用记录（原 {} tokens）",
            dropped,
            total
        );
        // 插一条系统级说明，避免模型以为自己从没调过那些工具
        result.insert(
            0,
            serde_json::json!({
                "role": "system",
                "content": format!(
                    "[上下文管理] 为控制长度，最早 {dropped} 轮的工具调用详情已省略。\
                     如果你已经获得了足够信息，请直接作答，不要重新调用这些工具。"
                ),
            }),
        );
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_context_passes_through() {
        let msgs = vec![serde_json::json!({"role":"tool","tool_call_id":"1","content":"ok"})];
        assert_eq!(govern_extra_messages(&msgs).len(), 1);
    }

    #[test]
    fn huge_context_gets_compacted() {
        let big = "x".repeat(200_000);
        let msgs: Vec<_> = (0..8)
            .map(|i| {
                serde_json::json!({
                    "role": "tool", "tool_call_id": i.to_string(), "content": big
                })
            })
            .collect();
        let out = govern_extra_messages(&msgs);
        let total: usize = out.iter().map(|m| estimate_tokens(&m.to_string())).sum();
        assert!(total < MAX_EXTRA_MESSAGES_TOKENS * 2, "got {total}");
    }

    #[test]
    fn truncation_is_utf8_safe() {
        let s = "中文测试".repeat(1000);
        let out = truncate_tool_result(&s, 10);
        assert!(out.starts_with("中文测试"));
    }
}
