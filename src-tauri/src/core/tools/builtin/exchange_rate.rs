// FastClaw · get_exchange_rate —— 汇率查询（结构化）
//
// 数据源：Frankfurter（https://frankfurter.dev）
//   - 完全免费、开源、**无需 API key**
//   - 数据来自欧洲央行等 84 家央行的参考汇率
//   - 覆盖 30+ 主要货币（v2 端点覆盖更广）
//
// 重要限制（必须让模型知道，否则会误导用户）：
//   ECB 参考汇率**每个工作日约 16:00 CET 更新一次**，周末与节假日不发布。
//   这是"中间价"，没有买卖价差，适合记账、报销、报价参考，
//   **不适合**外汇交易决策。工具输出里会带上这个说明。

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct ExchangeRateTool;

#[derive(Debug, Deserialize)]
struct ExchangeRateInput {
    /// 源货币代码，如 "USD"
    from: String,
    /// 目标货币代码，可多个，如 "CNY" 或 "CNY,JPY,EUR"
    to: String,
    /// 换算金额，默认 1
    #[serde(default = "default_amount")]
    amount: f64,
    /// 指定日期 YYYY-MM-DD，留空取最新
    #[serde(default)]
    date: Option<String>,
}

fn default_amount() -> f64 {
    1.0
}

/// 常见中文货币名 → ISO 4217 代码。
/// 模型有时会直接传中文，这里做一层兜底映射。
fn normalize_currency(s: &str) -> String {
    let t = s.trim();
    let mapped = match t {
        "人民币" | "元" | "块" | "rmb" | "RMB" => "CNY",
        "美元" | "美金" => "USD",
        "欧元" => "EUR",
        "日元" | "日圆" => "JPY",
        "英镑" => "GBP",
        "港币" | "港元" => "HKD",
        "韩元" | "韩币" => "KRW",
        "新台币" | "台币" => "TWD",
        "澳元" | "澳币" => "AUD",
        "加元" | "加币" => "CAD",
        "瑞郎" | "瑞士法郎" => "CHF",
        "新加坡元" | "新币" => "SGD",
        "卢布" => "RUB",
        "泰铢" => "THB",
        "印度卢比" => "INR",
        other => return other.to_uppercase(),
    };
    mapped.to_string()
}

#[async_trait]
impl Tool for ExchangeRateTool {
    fn name(&self) -> &str {
        "get_exchange_rate"
    }

    fn description(&self) -> &str {
        "查询货币汇率并换算金额，返回结构化数据。\n\
         **查汇率必须用这个工具，不要用 web_search** —— 搜索引擎抓不到实时汇率数字。\n\
         参数用 ISO 代码（USD/CNY/EUR/JPY…），也接受中文（美元/人民币）。\n\
         注意：数据是欧洲央行参考中间价，每工作日更新一次，周末不变；\
         适合记账报销与一般换算，不适合外汇交易决策。"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "from": { "type": "string", "description": "源货币代码，如 \"USD\"" },
                "to": { "type": "string", "description": "目标货币，可多个逗号分隔，如 \"CNY,JPY\"" },
                "amount": { "type": "number", "description": "换算金额，默认 1" },
                "date": { "type": "string", "description": "可选，指定日期 YYYY-MM-DD，留空取最新" }
            },
            "required": ["from", "to"]
        })
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let params: ExchangeRateInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => {
                return ToolResult::err(format!(
                "get_exchange_rate 参数错误：需要 {{\"from\":\"USD\",\"to\":\"CNY\"}}。详情：{e}"
            ))
            }
        };

        let from = normalize_currency(&params.from);
        let to: Vec<String> = params
            .to
            .split(',')
            .map(normalize_currency)
            .filter(|s| !s.is_empty())
            .collect();

        if from.is_empty() || to.is_empty() {
            return ToolResult::err("from / to 不能为空。".to_string());
        }
        let to_joined = to.join(",");

        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
        {
            Ok(c) => c,
            Err(e) => return ToolResult::err(format!("HTTP 客户端创建失败: {e}")),
        };

        // Frankfurter v1 端点（稳定、字段简单）
        let path = params.date.as_deref().unwrap_or("latest");
        let url = format!("https://api.frankfurter.app/{path}?from={from}&to={to_joined}");

        let resp = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                return ToolResult::err(format!(
                    "FX_NETWORK_ERROR: 无法连接汇率服务: {e}\n\
                     请告知用户网络不可用，不要改用 web_search 查汇率。"
                ))
            }
        };

        if !resp.status().is_success() {
            return ToolResult::err(format!(
                "汇率服务返回 {}。请检查货币代码是否有效（需为 ISO 4217，如 USD/CNY/EUR）。",
                resp.status()
            ));
        }

        let data: Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => return ToolResult::err(format!("汇率响应解析失败: {e}")),
        };

        let date = data.get("date").and_then(|v| v.as_str()).unwrap_or("未知");
        let rates = match data.get("rates").and_then(|v| v.as_object()) {
            Some(r) if !r.is_empty() => r,
            _ => {
                return ToolResult::err(format!(
                    "没有拿到 {from} → {to_joined} 的汇率。可能是货币代码无效，\
                     或该货币不在欧洲央行覆盖范围内（约 30 种主要货币）。"
                ))
            }
        };

        let mut out = format!("【汇率】基准日期：{date}\n\n");
        for (cur, rate) in rates {
            if let Some(r) = rate.as_f64() {
                let converted = r * params.amount;
                if (params.amount - 1.0).abs() < f64::EPSILON {
                    out.push_str(&format!("  1 {from} = {r:.4} {cur}\n"));
                } else {
                    out.push_str(&format!(
                        "  {} {from} = {converted:.2} {cur}   （汇率 {r:.4}）\n",
                        params.amount
                    ));
                }
            }
        }

        out.push_str(
            "\n说明：欧洲央行参考中间价，每工作日约 16:00 CET 更新一次，周末/节假日不发布。\
             适合记账与一般换算，不含买卖价差，不适合外汇交易决策。\n\
             数据来源：Frankfurter / ECB",
        );

        ToolResult::ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_chinese_names() {
        assert_eq!(normalize_currency("人民币"), "CNY");
        assert_eq!(normalize_currency("美元"), "USD");
        assert_eq!(normalize_currency("usd"), "USD");
        assert_eq!(normalize_currency("JPY"), "JPY");
    }
}
