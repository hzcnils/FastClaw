// FastClaw · get_location_info —— 地理编码 + 时区/当地时间
//
// 数据源：
//   ① Nominatim（OpenStreetMap）—— 地名 → 经纬度/行政区划，**无需 API key**
//   ② Open-Meteo geocoding —— 备用地理编码，同样无需 key，中文支持更好
//   ③ Open-Meteo forecast 的 timezone 字段 —— 拿到 IANA 时区与 UTC 偏移
//
// ⚠️ Nominatim 使用政策（务必遵守，否则会被封）：
//   - 公共实例限流 **1 次/秒**
//   - **必须**提供能标识应用的 User-Agent，否则直接 403
//   - 禁止大批量地理编码
//   本工具默认优先走 Open-Meteo（限制更宽松），Nominatim 仅作补充详情。

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct LocationInfoTool;

/// Nominatim 要求的应用标识
const NOMINATIM_UA: &str = "FastClaw/2.0 (desktop AI assistant; contact: support@fastclaw.local)";

#[derive(Debug, Deserialize)]
struct LocationInput {
    /// 地名，支持中文
    location: String,
    /// 是否附带当地时间与时区，默认 true
    #[serde(default = "default_true")]
    include_time: bool,
}

fn default_true() -> bool {
    true
}

#[async_trait]
impl Tool for LocationInfoTool {
    fn name(&self) -> &str {
        "get_location_info"
    }

    fn description(&self) -> &str {
        "查询地点的经纬度、所属行政区划、时区与当地当前时间。\n\
         适用于：「东京现在几点」「上海的经纬度」「洛杉矶时区」这类问题。\n\
         **不要用 web_search 查这类信息**，本工具返回结构化数据且更准确。\n\
         支持中文地名。"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "location": { "type": "string", "description": "地名，支持中文，如 \"东京\"、\"西安\"" },
                "include_time": { "type": "boolean", "description": "是否返回当地时间与时区，默认 true" }
            },
            "required": ["location"]
        })
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let params: LocationInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => {
                return ToolResult::err(format!(
                    "get_location_info 参数错误：需要 {{\"location\":\"东京\"}}。详情：{e}"
                ))
            }
        };

        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent(NOMINATIM_UA)
            .build()
        {
            Ok(c) => c,
            Err(e) => return ToolResult::err(format!("HTTP 客户端创建失败: {e}")),
        };

        // ── ① 地理编码（优先 Open-Meteo，中文支持好且限流宽松）──
        let geo_url = format!(
            "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=zh",
            super::weather::urlencode_pub(&params.location)
        );

        let geo: Value = match client.get(&geo_url).send().await {
            Ok(r) => match r.json().await {
                Ok(v) => v,
                Err(e) => return ToolResult::err(format!("地理编码响应解析失败: {e}")),
            },
            Err(e) => {
                return ToolResult::err(format!(
                    "GEO_NETWORK_ERROR: 无法连接地理编码服务: {e}\n\
                     请告知用户网络不可用，不要改用 web_search。"
                ))
            }
        };

        let first = geo
            .get("results")
            .and_then(|r| r.as_array())
            .and_then(|a| a.first());

        let f = match first {
            Some(f) => f,
            None => {
                return ToolResult::err(format!(
                    "没有找到地点「{}」。请让用户确认地名写法。",
                    params.location
                ))
            }
        };

        let lat = f.get("latitude").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let lon = f.get("longitude").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let name = f
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&params.location);
        let country = f.get("country").and_then(|v| v.as_str()).unwrap_or("");
        let admin1 = f.get("admin1").and_then(|v| v.as_str()).unwrap_or("");
        let tz = f.get("timezone").and_then(|v| v.as_str()).unwrap_or("");
        let population = f.get("population").and_then(|v| v.as_i64());
        let elevation = f.get("elevation").and_then(|v| v.as_f64());

        let mut out = format!("【{name}】\n\n");
        if !country.is_empty() {
            out.push_str(&format!("  国家/地区：{country}\n"));
        }
        if !admin1.is_empty() {
            out.push_str(&format!("  一级行政区：{admin1}\n"));
        }
        out.push_str(&format!("  坐标：{lat:.4}, {lon:.4}\n"));
        if let Some(e) = elevation {
            out.push_str(&format!("  海拔：{e:.0} 米\n"));
        }
        if let Some(p) = population {
            out.push_str(&format!("  人口：约 {p}\n"));
        }

        // ── ② 时区与当地时间 ──
        if params.include_time && !tz.is_empty() {
            out.push_str(&format!("  时区：{tz}\n"));

            let t_url = format!(
                "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}\
                 &current=temperature_2m&timezone=auto"
            );
            if let Ok(r) = client.get(&t_url).send().await {
                if let Ok(tv) = r.json::<Value>().await {
                    let local = tv
                        .get("current")
                        .and_then(|c| c.get("time"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let offset = tv
                        .get("utc_offset_seconds")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    if !local.is_empty() {
                        let h = offset / 3600;
                        let sign = if h >= 0 { "+" } else { "" };
                        out.push_str(&format!("  当地时间：{local}（UTC{sign}{h}）\n"));
                    }
                }
            }
        }

        out.push_str("\n数据来源：Open-Meteo Geocoding / OpenStreetMap");
        ToolResult::ok(out)
    }
}
