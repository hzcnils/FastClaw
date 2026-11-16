// FastClaw · get_crypto_price —— 加密货币行情（结构化）
//
// 数据源：CoinGecko Keyless Public API
//   - **无需 API key**，无需注册
//   - 官方文档明确说明：不要带 x-cg-demo-api-key / x-cg-pro-api-key 头，会被忽略
//   - 限流约 10-30 次/分钟；触发 429 应退避，不要立即重试
//   - 官方定位：原型/开源/教学用途，不适合高频轮询
//
// 之所以单独做工具而不是让模型搜网页：币价是秒级变动的结构化数据，
// SERP 上的数字要么是 JS 渲染的抓不到，要么是几小时前的缓存。

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct CryptoPriceTool;

#[derive(Debug, Deserialize)]
struct CryptoInput {
    /// 币种，接受常见名称/符号（bitcoin、BTC、比特币）或 CoinGecko id
    coins: String,
    /// 计价货币，默认 usd。可传 "usd,cny"
    #[serde(default = "default_vs")]
    vs_currency: String,
}

fn default_vs() -> String {
    "usd,cny".to_string()
}

/// 常见符号/中文名 → CoinGecko coin id。
///
/// CoinGecko 的 /simple/price 用的是 **id** 而不是 symbol（比如 BTC 的 id 是
/// "bitcoin"），模型很容易传错，这里做一层映射兜底。
fn to_coin_id(s: &str) -> String {
    let t = s.trim().to_lowercase();
    let mapped = match t.as_str() {
        "btc" | "比特币" | "大饼" => "bitcoin",
        "eth" | "以太坊" | "以太币" => "ethereum",
        "usdt" | "泰达币" => "tether",
        "usdc" => "usd-coin",
        "bnb" | "币安币" => "binancecoin",
        "sol" | "索拉纳" => "solana",
        "xrp" | "瑞波" | "瑞波币" => "ripple",
        "ada" | "艾达币" => "cardano",
        "doge" | "狗狗币" => "dogecoin",
        "trx" | "波场" => "tron",
        "ton" => "the-open-network",
        "avax" => "avalanche-2",
        "dot" | "波卡" => "polkadot",
        "matic" | "polygon" => "matic-network",
        "ltc" | "莱特币" => "litecoin",
        "shib" | "柴犬币" => "shiba-inu",
        "link" | "chainlink" => "chainlink",
        "bch" => "bitcoin-cash",
        other => other, // 已经是 id 就原样使用
    };
    mapped.to_string()
}

fn normalize_vs(s: &str) -> String {
    let t = s.trim().to_lowercase();
    match t.as_str() {
        "人民币" | "rmb" | "元" => "cny".to_string(),
        "美元" | "美金" => "usd".to_string(),
        "欧元" => "eur".to_string(),
        "日元" => "jpy".to_string(),
        other => other.to_string(),
    }
}

#[async_trait]
impl Tool for CryptoPriceTool {
    fn name(&self) -> &str {
        "get_crypto_price"
    }

    fn description(&self) -> &str {
        "查询加密货币实时价格、24小时涨跌幅与市值，返回结构化数据。\n\
         **查币价必须用这个工具，不要用 web_search**。\n\
         coins 接受常见写法（BTC / bitcoin / 比特币），可逗号分隔多个。\n\
         vs_currency 默认同时给出 usd 和 cny。"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "coins": {
                    "type": "string",
                    "description": "币种，可逗号分隔，如 \"bitcoin,ethereum\" 或 \"BTC,ETH\""
                },
                "vs_currency": {
                    "type": "string",
                    "description": "计价货币，默认 \"usd,cny\""
                }
            },
            "required": ["coins"]
        })
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let params: CryptoInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => {
                return ToolResult::err(format!(
                    "get_crypto_price 参数错误：需要 {{\"coins\":\"bitcoin\"}}。详情：{e}"
                ))
            }
        };

        let ids: Vec<String> = params
            .coins
            .split(',')
            .map(to_coin_id)
            .filter(|s| !s.is_empty())
            .collect();
        let vs: Vec<String> = params
            .vs_currency
            .split(',')
            .map(normalize_vs)
            .filter(|s| !s.is_empty())
            .collect();

        if ids.is_empty() || vs.is_empty() {
            return ToolResult::err("coins / vs_currency 不能为空。".to_string());
        }

        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
        {
            Ok(c) => c,
            Err(e) => return ToolResult::err(format!("HTTP 客户端创建失败: {e}")),
        };

        let url = format!(
            "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies={}\
             &include_24hr_change=true&include_market_cap=true&include_last_updated_at=true",
            ids.join(","),
            vs.join(",")
        );

        // 注意：keyless 端点**不要**带任何 api-key 头
        let resp = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                return ToolResult::err(format!(
                    "CRYPTO_NETWORK_ERROR: 无法连接行情服务: {e}\n\
                     请告知用户网络不可用，不要改用 web_search 查币价。"
                ))
            }
        };

        if resp.status().as_u16() == 429 {
            return ToolResult::err(
                "CRYPTO_RATE_LIMITED: 行情接口触发限流（免费额度约 10-30 次/分钟）。\n\
                 请告诉用户稍后再试，**不要立即重试，也不要改用 web_search**。"
                    .to_string(),
            );
        }
        if !resp.status().is_success() {
            return ToolResult::err(format!("行情服务返回 {}。", resp.status()));
        }

        let data: Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => return ToolResult::err(format!("行情响应解析失败: {e}")),
        };

        let obj = match data.as_object() {
            Some(o) if !o.is_empty() => o,
            _ => {
                return ToolResult::err(format!(
                    "没有查到「{}」的行情。CoinGecko 用的是 coin id 而非符号，\
                     常见的如 bitcoin / ethereum / solana。请确认币种名称。",
                    params.coins
                ))
            }
        };

        let mut out = String::from("【加密货币行情】\n\n");
        for (coin, v) in obj {
            out.push_str(&format!("{coin}\n"));
            for c in &vs {
                if let Some(p) = v.get(c).and_then(|x| x.as_f64()) {
                    let unit = c.to_uppercase();
                    // 小额币种保留更多小数位
                    if p >= 1.0 {
                        out.push_str(&format!("  价格：{p:.2} {unit}\n"));
                    } else {
                        out.push_str(&format!("  价格：{p:.8} {unit}\n"));
                    }
                }
                let chg_key = format!("{c}_24h_change");
                if let Some(ch) = v.get(&chg_key).and_then(|x| x.as_f64()) {
                    let arrow = if ch >= 0.0 { "▲" } else { "▼" };
                    out.push_str(&format!("  24h 涨跌：{arrow} {ch:+.2}%\n"));
                }
                let mc_key = format!("{c}_market_cap");
                if let Some(mc) = v.get(&mc_key).and_then(|x| x.as_f64()) {
                    out.push_str(&format!(
                        "  市值：{:.2} 亿 {}\n",
                        mc / 100_000_000.0,
                        c.to_uppercase()
                    ));
                }
            }
            if let Some(ts) = v.get("last_updated_at").and_then(|x| x.as_i64()) {
                out.push_str(&format!("  更新时间戳：{ts}\n"));
            }
            out.push('\n');
        }

        out.push_str("数据来源：CoinGecko（免费公开端点，限流约 10-30 次/分钟）");
        ToolResult::ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_symbols_to_ids() {
        assert_eq!(to_coin_id("BTC"), "bitcoin");
        assert_eq!(to_coin_id("比特币"), "bitcoin");
        assert_eq!(to_coin_id("ethereum"), "ethereum");
        assert_eq!(to_coin_id("solana"), "solana");
    }

    #[test]
    fn normalizes_vs_currency() {
        assert_eq!(normalize_vs("人民币"), "cny");
        assert_eq!(normalize_vs("USD"), "usd");
    }
}
