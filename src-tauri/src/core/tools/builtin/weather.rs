// FastClaw · get_weather —— 结构化天气查询
//
// 为什么需要一个专门的工具，而不是让模型用 web_search 查天气：
//
//   实测日志里，「今天西安天气怎么样」打满了 6 轮工具调用，
//   搜到的却是「西安市_百度百科」「20个去西安必打卡的景点」——
//   最后模型只能说"没能获取到实时天气数据"，然后给了一段气候常识。
//
//   根因：天气是**高度结构化的实时数据**，而 SERP 里的温度数字
//   通常由 JavaScript 动态渲染，静态抓 HTML 根本拿不到。
//   再多轮搜索也解决不了 —— 这是数据源选错了，不是搜索次数不够。
//
// 本工具走 Open-Meteo：
//   - 完全免费，**无需 API key**（这点很关键，不增加部署负担）
//   - 返回 JSON，无需解析 HTML，不受版面变动影响
//   - 自带地理编码，支持中文城市名
//   - 数据源是各国官方气象机构

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct WeatherTool;

#[derive(Debug, Deserialize)]
struct WeatherInput {
    /// 城市名，支持中文（如「西安」「上海」）或英文
    location: String,
    /// 预报天数，1 = 仅今天，默认 3
    #[serde(default = "default_days")]
    days: u8,
}

fn default_days() -> u8 {
    3
}

/// WMO 天气代码 → 中文描述
fn wmo_to_text(code: i64) -> &'static str {
    match code {
        0 => "晴",
        1 => "大部晴朗",
        2 => "局部多云",
        3 => "阴",
        45 | 48 => "雾",
        51 | 53 | 55 => "毛毛雨",
        56 | 57 => "冻毛毛雨",
        61 => "小雨",
        63 => "中雨",
        65 => "大雨",
        66 | 67 => "冻雨",
        71 => "小雪",
        73 => "中雪",
        75 => "大雪",
        77 => "雪粒",
        80 => "小阵雨",
        81 => "中阵雨",
        82 => "强阵雨",
        85 | 86 => "阵雪",
        95 => "雷阵雨",
        96 | 99 => "雷阵雨伴冰雹",
        _ => "未知",
    }
}

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self) -> &str {
        "get_weather"
    }

    fn description(&self) -> &str {
        "查询指定城市的实时天气与未来几天预报，返回结构化数据（温度、天气状况、降水、风速、湿度）。\n\
         **查天气必须用这个工具，不要用 web_search** —— 搜索引擎返回的是网页 HTML，\
         温度数字由 JS 动态渲染，静态抓取拿不到，只会浪费轮次。\n\
         支持中文城市名，如 location=\"西安\"。无需 API key。"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "城市名，支持中文，例如 \"西安\"、\"北京\"、\"Tokyo\""
                },
                "days": {
                    "type": "number",
                    "description": "预报天数，1-7，默认 3。只问「今天天气」时传 1"
                }
            },
            "required": ["location"]
        })
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let params: WeatherInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => {
                return ToolResult::err(format!(
                    "get_weather 参数错误：需要 {{\"location\": \"城市名\"}}。详情：{e}"
                ))
            }
        };

        let days = params.days.clamp(1, 7);
        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
        {
            Ok(c) => c,
            Err(e) => return ToolResult::err(format!("HTTP 客户端创建失败: {e}")),
        };

        // ── ① 地理编码：城市名 → 经纬度 ──
        let geo_url = format!(
            "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=zh",
            urlencode(&params.location)
        );

        let geo: Value = match client.get(&geo_url).send().await {
            Ok(r) => match r.json().await {
                Ok(v) => v,
                Err(e) => return ToolResult::err(format!("地理编码响应解析失败: {e}")),
            },
            Err(e) => {
                return ToolResult::err(format!(
                    "WEATHER_NETWORK_ERROR: 无法连接天气服务: {e}\n\
                     请告知用户网络不可用，不要改用 web_search 查天气（搜不到实时数据）。"
                ))
            }
        };

        let first = geo
            .get("results")
            .and_then(|r| r.as_array())
            .and_then(|a| a.first());

        let (lat, lon, resolved_name, admin) = match first {
            Some(f) => (
                f.get("latitude").and_then(|v| v.as_f64()).unwrap_or(0.0),
                f.get("longitude").and_then(|v| v.as_f64()).unwrap_or(0.0),
                f.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&params.location)
                    .to_string(),
                f.get("admin1")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            ),
            None => {
                return ToolResult::err(format!(
                    "没有找到城市「{}」。请让用户确认城市名，或换一个更常见的写法。",
                    params.location
                ))
            }
        };

        // ── ② 查询天气 ──
        let wx_url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}\
             &current=temperature_2m,relative_humidity_2m,apparent_temperature,weather_code,wind_speed_10m\
             &daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max\
             &timezone=auto&forecast_days={days}"
        );

        let wx: Value = match client.get(&wx_url).send().await {
            Ok(r) => match r.json().await {
                Ok(v) => v,
                Err(e) => return ToolResult::err(format!("天气响应解析失败: {e}")),
            },
            Err(e) => return ToolResult::err(format!("天气查询失败: {e}")),
        };

        // ── ③ 格式化输出 ──
        let mut out = String::new();
        let place = if admin.is_empty() || admin == resolved_name {
            resolved_name.clone()
        } else {
            format!("{admin}{resolved_name}")
        };
        out.push_str(&format!("【{place} 天气】\n\n"));

        if let Some(cur) = wx.get("current") {
            let t = cur.get("temperature_2m").and_then(|v| v.as_f64());
            let feels = cur.get("apparent_temperature").and_then(|v| v.as_f64());
            let hum = cur.get("relative_humidity_2m").and_then(|v| v.as_f64());
            let wind = cur.get("wind_speed_10m").and_then(|v| v.as_f64());
            let code = cur
                .get("weather_code")
                .and_then(|v| v.as_i64())
                .unwrap_or(-1);
            let time = cur.get("time").and_then(|v| v.as_str()).unwrap_or("");

            out.push_str(&format!("实况（{time}）：{}\n", wmo_to_text(code)));
            if let Some(t) = t {
                out.push_str(&format!("  气温：{t}°C"));
                if let Some(f) = feels {
                    out.push_str(&format!("（体感 {f}°C）"));
                }
                out.push('\n');
            }
            if let Some(h) = hum {
                out.push_str(&format!("  湿度：{h}%\n"));
            }
            if let Some(w) = wind {
                out.push_str(&format!("  风速：{w} km/h\n"));
            }
            out.push('\n');
        }

        if let Some(daily) = wx.get("daily") {
            let dates = daily.get("time").and_then(|v| v.as_array());
            let codes = daily.get("weather_code").and_then(|v| v.as_array());
            let tmax = daily.get("temperature_2m_max").and_then(|v| v.as_array());
            let tmin = daily.get("temperature_2m_min").and_then(|v| v.as_array());
            let pop = daily
                .get("precipitation_probability_max")
                .and_then(|v| v.as_array());

            if let Some(dates) = dates {
                out.push_str("预报：\n");
                for (i, d) in dates.iter().enumerate() {
                    let date = d.as_str().unwrap_or("");
                    let code = codes
                        .and_then(|a| a.get(i))
                        .and_then(|v| v.as_i64())
                        .unwrap_or(-1);
                    let hi = tmax.and_then(|a| a.get(i)).and_then(|v| v.as_f64());
                    let lo = tmin.and_then(|a| a.get(i)).and_then(|v| v.as_f64());
                    let p = pop.and_then(|a| a.get(i)).and_then(|v| v.as_f64());

                    out.push_str(&format!("  {date}  {}", wmo_to_text(code)));
                    if let (Some(lo), Some(hi)) = (lo, hi) {
                        out.push_str(&format!("  {lo}~{hi}°C"));
                    }
                    if let Some(p) = p {
                        if p > 0.0 {
                            out.push_str(&format!("  降水概率 {p}%"));
                        }
                    }
                    out.push('\n');
                }
            }
        }

        out.push_str("\n数据来源：Open-Meteo（各国官方气象机构）");
        ToolResult::ok(out)
    }
}

/// 最小 URL 编码（避免引入新依赖）。
/// 供同目录下其他工具（location_info 等）复用。
pub(crate) fn urlencode_pub(s: &str) -> String {
    urlencode(s)
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencode_handles_chinese() {
        // 西安 → UTF-8 percent-encoded
        assert_eq!(urlencode("西安"), "%E8%A5%BF%E5%AE%89");
        assert_eq!(urlencode("Tokyo"), "Tokyo");
    }

    #[test]
    fn wmo_mapping() {
        assert_eq!(wmo_to_text(0), "晴");
        assert_eq!(wmo_to_text(95), "雷阵雨");
    }
}
