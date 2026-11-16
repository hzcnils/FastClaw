// FastClaw · 联网搜索服务
// 统一入口 + 多服务商适配（博查 Bocha / Tavily / Exa / Google / Bing / Baidu）。
use serde::Deserialize;
use sqlx::SqlitePool;
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct WebHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Default)]
pub struct WebSearchCfg {
    pub provider: String,   // bocha | tavily | exa | google | bing | baidu
    pub max_results: usize, // 1-10
    pub time_range: String, // none | year | month | week
    pub safe: bool,
    pub blacklist: Vec<String>,
    pub tavily_key: String,
    pub exa_key: String,
    pub bocha_key: String,
}

/// 从 settings 表读取 websearch 配置，并用本次请求的 provider 覆盖
pub async fn load_cfg(pool: &SqlitePool, provider_override: Option<String>) -> WebSearchCfg {
    let raw: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM settings WHERE category = 'websearch' AND key = 'config' LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let v: serde_json::Value = raw
        .and_then(|(s,)| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    let keys = v.get("keys").cloned().unwrap_or_default();
    let mut cfg = WebSearchCfg {
        provider: v
            .get("provider")
            .and_then(|x| x.as_str())
            .unwrap_or("bocha")
            .to_string(),
        max_results: v
            .get("maxResults")
            .and_then(|x| x.as_u64())
            .unwrap_or(5)
            .clamp(1, 10) as usize,
        time_range: v
            .get("timeRange")
            .and_then(|x| x.as_str())
            .unwrap_or("year")
            .to_string(),
        safe: v
            .get("safeSearch")
            .and_then(|x| x.as_bool())
            .unwrap_or(true),
        blacklist: v
            .get("blacklist")
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        tavily_key: keys
            .get("tavily")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        exa_key: keys
            .get("exa")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        bocha_key: keys
            .get("bocha")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
    };
    if let Some(p) = provider_override {
        if !p.is_empty() {
            cfg.provider = p;
        }
    }
    cfg
}

/// 统一入口：按 provider 分发，过滤黑名单，截断到 max_results
pub async fn web_search(query: &str, cfg: &WebSearchCfg) -> anyhow::Result<Vec<WebHit>> {
    tracing::info!("🌐 web_search provider={} query={:?}", cfg.provider, query);
    let mut hits = match cfg.provider.as_str() {
        "bocha" => bocha(query, cfg).await?,
        "exa" => exa(query, cfg).await?,
        "google" => google(query, cfg).await?,
        "bing" => bing(query, cfg).await?,
        "baidu" => baidu(query, cfg).await?,
        "tavily" => tavily(query, cfg).await?,
        _ => bocha(query, cfg).await?,
    };
    if !cfg.blacklist.is_empty() {
        hits.retain(|h| {
            !cfg.blacklist
                .iter()
                .any(|b| !b.is_empty() && h.url.contains(b.as_str()))
        });
    }
    hits.truncate(cfg.max_results);
    // 诊断：打印实际命中的标题，便于排查“搜出来的东西对不对”
    for (i, h) in hits.iter().enumerate() {
        tracing::info!("   [{}] {} | {}", i + 1, h.title, h.url);
    }
    Ok(hits)
}

// ① Tavily（推荐默认，为 LLM 优化）
async fn tavily(query: &str, cfg: &WebSearchCfg) -> anyhow::Result<Vec<WebHit>> {
    if cfg.tavily_key.is_empty() {
        anyhow::bail!("Tavily API Key 未配置");
    }
    #[derive(Deserialize)]
    struct Res {
        results: Vec<Item>,
    }
    #[derive(Deserialize)]
    struct Item {
        #[serde(default)]
        title: String,
        #[serde(default)]
        url: String,
        #[serde(default)]
        content: String,
    }
    let body = serde_json::json!({
        "api_key": cfg.tavily_key,
        "query": query,
        "max_results": cfg.max_results,
        "search_depth": "basic",
        "time_range": if cfg.time_range == "none" { serde_json::Value::Null } else { serde_json::Value::String(cfg.time_range.clone()) },
    });
    let res: Res = http_client()
        .post(crate::core::constants::TAVILY_API_URL)
        .timeout(Duration::from_secs(8))
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(res
        .results
        .into_iter()
        .map(|i| WebHit {
            title: i.title,
            url: i.url,
            snippet: i.content,
        })
        .collect())
}

// ② Exa.ai（AI 原生搜索，为 LLM 优化）
async fn exa(query: &str, cfg: &WebSearchCfg) -> anyhow::Result<Vec<WebHit>> {
    if cfg.exa_key.is_empty() {
        anyhow::bail!("Exa API Key 未配置");
    }
    #[derive(Deserialize)]
    struct Res {
        results: Vec<Item>,
    }
    #[derive(Deserialize)]
    struct Item {
        #[serde(default)]
        title: String,
        #[serde(default)]
        url: String,
        #[serde(default)]
        text: String,
    }
    let body = serde_json::json!({
        "query": query,
        "numResults": cfg.max_results,
        "type": "auto",
        "contents": { "text": true },
    });
    let res: Res = http_client()
        .post(crate::core::constants::EXA_API_URL)
        .timeout(Duration::from_secs(10))
        .header("x-api-key", &cfg.exa_key)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(res
        .results
        .into_iter()
        .map(|i| WebHit {
            title: i.title,
            url: i.url,
            snippet: i.text,
        })
        .collect())
}

// ③ 博查 Web Search API（国内主推，API 直返、为 AI 优化）
async fn bocha(query: &str, cfg: &WebSearchCfg) -> anyhow::Result<Vec<WebHit>> {
    if cfg.bocha_key.is_empty() {
        anyhow::bail!("博查 API Key 未配置");
    }
    let freshness = match cfg.time_range.as_str() {
        "week" => "oneWeek",
        "month" => "oneMonth",
        "year" => "oneYear",
        _ => "noLimit",
    };

    let body = serde_json::json!({
        "query": query,
        "count": cfg.max_results,
        "summary": true,
        "freshness": freshness,
    });

    let resp = http_client()
        .post(crate::core::constants::BOCHA_API_URL)
        .timeout(Duration::from_secs(8))
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", cfg.bocha_key),
        )
        .json(&body)
        .send()
        .await?
        .error_for_status()?;

    let raw_text = resp.text().await?;
    let truncated: String = raw_text.chars().take(500).collect();
    tracing::info!("博查原始响应(截断500字): {}", truncated);

    // 博查响应有 {code, msg, data} 外层包裹，data 内才是 Bing 兼容格式
    let v: serde_json::Value = serde_json::from_str(&raw_text)?;
    let web_pages = v
        .get("data")
        .and_then(|d| d.get("webPages"))
        .or_else(|| v.get("webPages"));

    let items: Vec<BochaItem> = web_pages
        .and_then(|wp| wp.get("value"))
        .map(|val| serde_json::from_value(val.clone()).unwrap_or_default())
        .unwrap_or_default();

    tracing::info!("博查返回 {} 条结果", items.len());
    Ok(items
        .into_iter()
        .map(|i| {
            let snippet = if i.summary.trim().is_empty() {
                i.snippet
            } else {
                i.summary
            };
            WebHit {
                title: i.name,
                url: i.url,
                snippet,
            }
        })
        .collect())
}

#[derive(Deserialize, Default)]
struct BochaItem {
    #[serde(default)]
    name: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    snippet: String,
    #[serde(default)]
    summary: String,
}

// ===================== 免 Key 网页抓取：Google / Bing / Baidu =====================
//
// 说明：以下三家无官方免费 API，走「抓取搜索结果页 + 解析 HTML」的方式（与 Cherry Studio 的
// 免费来源同思路）。优点是免配置；缺点是依赖页面结构、可能被反爬（尤其 Google），稳定性弱于
// Tavily/Exa。失败时上层已做「降级无搜索」处理。

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                  (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

/// 拉取搜索结果页 HTML（带浏览器 UA / 语言头，降低被拦概率）
async fn fetch_html(url: &str) -> anyhow::Result<String> {
    let res = http_client()
        .get(url)
        .timeout(Duration::from_secs(10))
        .header(reqwest::header::USER_AGENT, UA)
        .header(reqwest::header::ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en;q=0.8")
        .header(
            reqwest::header::ACCEPT,
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .send()
        .await?
        .error_for_status()?;
    Ok(res.text().await?)
}

/// 取第一个能命中的选择器文本/属性
fn pick_text(el: &scraper::ElementRef, sels: &[&str]) -> String {
    for s in sels {
        if let Ok(sel) = scraper::Selector::parse(s) {
            if let Some(e) = el.select(&sel).next() {
                let t = e.text().collect::<String>().trim().to_string();
                if !t.is_empty() {
                    return t;
                }
            }
        }
    }
    String::new()
}
fn pick_href(el: &scraper::ElementRef, sels: &[&str]) -> String {
    for s in sels {
        if let Ok(sel) = scraper::Selector::parse(s) {
            if let Some(e) = el.select(&sel).next() {
                if let Some(h) = e.value().attr("href") {
                    if !h.is_empty() {
                        return h.to_string();
                    }
                }
            }
        }
    }
    String::new()
}

/// 通用解析：给定「条目选择器 + 标题选择器 + 链接选择器 + 摘要选择器」抽取结果
fn parse_results(
    html: &str,
    item_sel: &str,
    title_sels: &[&str],
    link_sels: &[&str],
    snippet_sels: &[&str],
    limit: usize,
) -> Vec<WebHit> {
    let doc = scraper::Html::parse_document(html);
    let item = match scraper::Selector::parse(item_sel) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for node in doc.select(&item) {
        let title = pick_text(&node, title_sels);
        let url = pick_href(&node, link_sels);
        if title.is_empty() || url.is_empty() || !url.starts_with("http") {
            continue;
        }
        let snippet = pick_text(&node, snippet_sels);
        out.push(WebHit {
            title,
            url,
            snippet,
        });
        if out.len() >= limit {
            break;
        }
    }
    out
}

// ④ Google（抓取，免 Key；反爬较强，可能失败）
async fn google(query: &str, cfg: &WebSearchCfg) -> anyhow::Result<Vec<WebHit>> {
    let url = format!(
        "{}?q={}&num={}&hl=zh-CN",
        crate::core::constants::GOOGLE_SEARCH_URL,
        urlencoding(query),
        cfg.max_results.max(8)
    );
    let html = fetch_html(&url).await?;
    let hits = parse_results(
        &html,
        "div.tF2Cxc, div.g div.yuRUbf, div.MjjYud div.g",
        &["h3"],
        &["a"],
        &["div.VwiC3b", "div.aCOpRe", ".lEBKkf", ".s3v9rd"],
        cfg.max_results,
    );
    if hits.is_empty() {
        anyhow::bail!("Google 未解析到结果（可能被反爬，建议改用 Bing/Baidu 或 Tavily）");
    }
    Ok(hits)
}

// ⑤ Bing（抓取，免 Key；较稳定）
async fn bing(query: &str, cfg: &WebSearchCfg) -> anyhow::Result<Vec<WebHit>> {
    let url = format!(
        "{}?q={}&setlang=zh-CN&count={}",
        crate::core::constants::BING_SEARCH_URL,
        urlencoding(query),
        cfg.max_results.max(10)
    );
    let html = fetch_html(&url).await?;
    let hits = parse_results(
        &html,
        "li.b_algo",
        &["h2 a", "h2"],
        &["h2 a", "a.tilk"],
        &[".b_caption p", ".b_algoSlug", "p"],
        cfg.max_results,
    );
    if hits.is_empty() {
        anyhow::bail!("Bing 未解析到结果");
    }
    Ok(hits)
}

// ⑥ Baidu（抓取，免 Key；链接为百度跳转链，点击可正常跳转）
async fn baidu(query: &str, cfg: &WebSearchCfg) -> anyhow::Result<Vec<WebHit>> {
    let url = format!(
        "{}?wd={}&rn={}",
        crate::core::constants::BAIDU_SEARCH_URL,
        urlencoding(query),
        cfg.max_results.max(10)
    );
    let html = fetch_html(&url).await?;
    let hits = parse_results(
        &html,
        "div.result, div.result-op, div.c-container",
        &["h3 a", "h3", ".t a"],
        &["h3 a", ".t a", "a"],
        &[
            ".c-abstract",
            "[class*=content-right_]",
            ".c-span-last",
            ".c-color-text",
        ],
        cfg.max_results,
    );
    if hits.is_empty() {
        anyhow::bail!("Baidu 未解析到结果");
    }
    Ok(hits)
}

/// 极简 URL 编码（仅对查询词，避免引入额外依赖）
fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
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

/// 拼成可注入系统提示的文本（带编号 + 链接，便于溯源）
pub fn format_web_context(hits: &[WebHit]) -> String {
    if hits.is_empty() {
        return String::new();
    }
    let mut s = String::new();
    for (i, h) in hits.iter().enumerate() {
        s.push_str(&format!(
            "[{}] {}\n{}\n来源: {}\n\n",
            i + 1,
            h.title,
            h.snippet,
            h.url
        ));
    }
    s
}
