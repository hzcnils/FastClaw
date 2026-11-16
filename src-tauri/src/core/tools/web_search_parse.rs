// FastClaw · SERP 解析（DOM 版）与 Query 预处理
//
// 问题背景（「汇智信科如何」搜成了「汇」字的字典解释）：
//   原实现用 `html.split("class=\"b_algo\"")` / `html.split("class=\"result")`
//   做字符串切割。搜索引擎前端每天都在变（动态类名、CSS Modules、A/B 分流），
//   一旦切割失效，代码会跳过主结果区，意外匹配到右侧「汉字字典/拆字建议」
//   之类的小组件，抓回一堆噪音。
//
//   本模块改用 scraper 的 DOM + CSS 选择器解析（scraper 0.20 已是现有依赖，
//   web_fetch.rs 里已经在用），并做三件事：
//     ① Query 降噪：剥离「如何/怎么样」这类口语助词，避免分词权重跑偏
//     ② DOM 解析：多套选择器依次回退，免疫大部分类名变更
//     ③ 百度跳转链接还原：baidu.com/link?url= 是 JS/meta-refresh 跳转，
//        不是 302，reqwest 的自动重定向救不了，必须显式解析

use scraper::{Html, Selector};

/// 搜索结果条目（与 web_search.rs 中的 SearchResult 字段保持一致）
#[derive(Debug, Clone)]
pub struct ParsedResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

// ── ① Query 降噪 ────────────────────────────────────────────────────

/// 中文口语助词/疑问后缀。带着这些词去请求 SERP，
/// 会让分词权重严重倾斜（「汇智信科如何」→ 命中「汇」的字典条目）。
const TRAILING_STOP_WORDS: &[&str] = &[
    "怎么样啊",
    "是怎么样的",
    "怎么样",
    "怎么回事",
    "怎么做",
    "如何样",
    "咋样",
    "是什么意思",
    "是什么样的",
    "是什么",
    "有哪些",
    "都有哪些",
    "多少钱",
    "贵不贵",
    "好不好",
    "行不行",
    "靠谱吗",
    "怎样",
    "如何",
    "呢",
    "吗",
    "啊",
];

/// 通用前缀噪音
const LEADING_STOP_WORDS: &[&str] = &[
    "请问",
    "帮我查一下",
    "帮我查查",
    "帮我搜索",
    "帮我搜",
    "查一下",
    "查查",
    "我想知道",
    "想了解一下",
    "了解一下",
    "介绍一下",
    "搜索",
    "搜一下",
];

/// 清洗查询词：剥离口语助词，保留核心实体。
///
/// 刻意只做**前后缀**剥离，不做中间替换 —— 原方案里的
/// `clean_query.replace(sw, "")` 会把「如何评价 X 公司」中间的词也删掉，
/// 甚至破坏正常实体名（比如公司名里真的含「多少钱」三个字的极端情况）。
///
/// 清洗后若为空或过短（<2 字符），退回原始 query，避免过度清洗。
pub fn sanitize_query(query: &str) -> String {
    let mut q = query.trim().to_string();

    // 去掉首尾的中英文标点
    let trim_punct = |s: &str| -> String {
        s.trim_matches(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    '?' | '？' | '。' | '，' | ',' | '!' | '！' | '、' | '~' | '～'
                )
        })
        .to_string()
    };
    q = trim_punct(&q);

    // 前缀剥离（可能叠加，循环到不再变化）
    let mut changed = true;
    while changed {
        changed = false;
        for sw in LEADING_STOP_WORDS {
            if q.starts_with(sw) && q.chars().count() > sw.chars().count() + 1 {
                q = q[sw.len()..].to_string();
                q = trim_punct(&q);
                changed = true;
                break;
            }
        }
    }

    // 后缀剥离
    changed = true;
    while changed {
        changed = false;
        for sw in TRAILING_STOP_WORDS {
            if q.ends_with(sw) && q.chars().count() > sw.chars().count() + 1 {
                let cut = q.len() - sw.len();
                q = q[..cut].to_string();
                q = trim_punct(&q);
                changed = true;
                break;
            }
        }
    }

    // 过度清洗保护
    if q.chars().count() < 2 {
        return query.trim().to_string();
    }
    q
}

// ── ② DOM 解析 ──────────────────────────────────────────────────────

/// 依次尝试多个选择器，返回第一个能选中元素的。
/// 类名变更时不至于一次全废。
fn first_matching<'a>(doc: &'a Html, candidates: &[&str]) -> Option<(Selector, usize)> {
    for c in candidates {
        if let Ok(sel) = Selector::parse(c) {
            let n = doc.select(&sel).count();
            if n > 0 {
                return Some((sel, n));
            }
        }
    }
    None
}

fn text_of(el: &scraper::ElementRef) -> String {
    // .text() 会自然穿透 <em> 等高亮标签，不存在原实现「标签嵌套失配截断」的问题
    let s: String = el.text().collect::<Vec<_>>().join("");
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Bing SERP 解析。
pub fn parse_bing(html: &str, max: usize) -> Vec<ParsedResult> {
    let doc = Html::parse_document(html);
    let mut out = Vec::new();

    // 主结果容器：b_algo 是长期稳定的，但仍准备回退
    let (item_sel, _) = match first_matching(
        &doc,
        &[
            "li.b_algo",
            "div.b_algo",
            "#b_results > li:not(.b_ad)",
            "li.b_ans",
        ],
    ) {
        Some(v) => v,
        None => return out,
    };

    let title_sels = ["h2 > a", "h2 a", "a.tilk", "h3 a"];
    let snip_sels = ["div.b_caption p", "p.b_algoSlug", "div.b_snippet", "p"];

    for el in doc.select(&item_sel) {
        if out.len() >= max {
            break;
        }

        let a = title_sels
            .iter()
            .filter_map(|s| Selector::parse(s).ok())
            .find_map(|s| el.select(&s).next());

        let a = match a {
            Some(a) => a,
            None => continue,
        };

        let url = a.value().attr("href").unwrap_or("").trim().to_string();
        if url.is_empty() || url.starts_with("javascript:") || url.starts_with('#') {
            continue;
        }

        let title = text_of(&a);
        if title.is_empty() {
            continue;
        }

        let snippet = snip_sels
            .iter()
            .filter_map(|s| Selector::parse(s).ok())
            .find_map(|s| el.select(&s).next().map(|n| text_of(&n)))
            .unwrap_or_default();

        out.push(ParsedResult {
            title,
            url,
            snippet,
        });
    }

    out
}

/// 百度 SERP 解析。
///
/// 百度主结果容器类名是 `result c-container`，但会附加各种变体
/// （`result-op`、`c-container new-pmd` 等）。这里用属性前缀选择器覆盖，
/// 并**显式排除**右侧栏/推荐区 —— 那正是「汇」字字典条目的来源。
pub fn parse_baidu(html: &str, max: usize) -> Vec<ParsedResult> {
    let doc = Html::parse_document(html);
    let mut out = Vec::new();

    // #content_left 限定主结果区，天然排除右侧 #content_right（字典/百科小组件）
    let (item_sel, _) = match first_matching(
        &doc,
        &[
            "#content_left div.result",
            "#content_left div.c-container",
            "#content_left div[class*='result']",
            "div.result.c-container",
            "div[tpl][mu]",
        ],
    ) {
        Some(v) => v,
        None => return out,
    };

    let title_sels = ["h3 > a", "h3 a", "a.c-title-link", ".t > a"];
    let snip_sels = [
        "div.c-abstract",
        "span.content-right_2s-H4",
        "div[class*='abstract']",
        "div.c-span-last span",
    ];

    for el in doc.select(&item_sel) {
        if out.len() >= max {
            break;
        }

        // 跳过明显的非自然结果组件
        let cls = el.value().attr("class").unwrap_or("");
        if cls.contains("result-op") && cls.contains("c-container") {
            // result-op 是「阿拉丁」卡片（字典/天气/日历等），大多不是网页结果。
            // 只有当它带有可用的外链标题时才保留，否则跳过。
            let has_link = Selector::parse("h3 a")
                .ok()
                .map(|s| el.select(&s).next().is_some())
                .unwrap_or(false);
            if !has_link {
                continue;
            }
        }

        let a = title_sels
            .iter()
            .filter_map(|s| Selector::parse(s).ok())
            .find_map(|s| el.select(&s).next());

        let a = match a {
            Some(a) => a,
            None => continue,
        };

        let url = a.value().attr("href").unwrap_or("").trim().to_string();
        if url.is_empty() || url.starts_with("javascript:") || url.starts_with('#') {
            continue;
        }

        let title = text_of(&a);
        if title.is_empty() {
            continue;
        }

        let snippet = snip_sels
            .iter()
            .filter_map(|s| Selector::parse(s).ok())
            .find_map(|s| el.select(&s).next().map(|n| text_of(&n)))
            .unwrap_or_default();

        out.push(ParsedResult {
            title,
            url,
            snippet,
        });
    }

    out
}

// ── ③ 百度跳转链接还原 ──────────────────────────────────────────────

/// 判断是否为百度加密跳转链接。
pub fn is_baidu_redirect(url: &str) -> bool {
    url.contains("baidu.com/link?url=")
}

/// 从百度跳转页还原真实 URL。
///
/// 关键：百度的 `/link?url=` **不是 302**，而是返回一个含
/// `window.location.replace("...")` 或 `<meta http-equiv="refresh">` 的
/// HTML 跳板页。所以 reqwest 的自动重定向（默认最多 10 次）完全不起作用，
/// 必须把跳板页抓下来解析。
///
/// 失败时返回 None，调用方保留原 URL 即可（不至于丢结果）。
pub async fn resolve_baidu_redirect(client: &reqwest::Client, url: &str) -> Option<String> {
    let resp = client
        .get(url)
        .header("User-Agent", UA)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .ok()?;

    // 少数情况确实走 302，此时 final_url 已经是真实地址
    let final_url = resp.url().to_string();
    if !is_baidu_redirect(&final_url) {
        return Some(final_url);
    }

    let body = resp.text().await.ok()?;
    extract_redirect_target(&body)
}

/// 从跳板页 HTML 里提取真实 URL。
pub fn extract_redirect_target(body: &str) -> Option<String> {
    // ① window.location.replace("...")  /  window.location.href="..."
    for marker in [
        "window.location.replace(\"",
        "window.location.href=\"",
        "window.location.href =\"",
        "location.replace(\"",
    ] {
        if let Some(p) = body.find(marker) {
            let rest = &body[p + marker.len()..];
            if let Some(end) = rest.find('"') {
                let u = &rest[..end];
                if u.starts_with("http") {
                    return Some(u.to_string());
                }
            }
        }
    }

    // ② <meta http-equiv="refresh" content="0;url=...">
    //
    // ★ 不能用 `body.to_lowercase().find(..)` 的下标去切 `body`：
    //   to_lowercase() 可能改变字节长度，两个字符串下标并不对应。
    //   用 ASCII 大小写无关搜索直接在 body 上定位。
    let mut from = 0usize;
    while let Some(start) = find_ascii_ci(body, "url=", from) {
        let after = start + 4; // "url=" 全 ASCII，边界安全
        let rest = &body[after..];
        let end = rest
            .find(|c: char| c == '"' || c == '\'' || c == '>' || c == ' ')
            .unwrap_or(rest.len());
        let u = rest[..end].trim();
        if u.starts_with("http") {
            return Some(u.to_string());
        }
        from = after;
    }

    None
}

/// 大小写无关地在 `hay` 中查找 ASCII 子串 `needle`，从字节位置 `from` 开始。
/// 返回的下标一定落在字符边界上（因为 needle 全为 ASCII，只可能匹配到
/// ASCII 字节序列的起点）。
fn find_ascii_ci(hay: &str, needle: &str, from: usize) -> Option<usize> {
    if from >= hay.len() {
        return None;
    }
    let h = hay.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() || h.len() < n.len() {
        return None;
    }
    for i in from..=(h.len() - n.len()) {
        // 只在字符边界上尝试匹配，返回值必然安全
        if !hay.is_char_boundary(i) {
            continue;
        }
        if h[i..i + n.len()].eq_ignore_ascii_case(n) {
            return Some(i);
        }
    }
    None
}

/// 统一 User-Agent
pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                      (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

// ── 结果质量过滤 ────────────────────────────────────────────────────

/// 判断一条结果是否明显是噪音（字典/拆字/纯百科单字条目等）。
///
/// 这是针对「汇智信科如何 → 汇 (汉语文字)_百度百科」这类失败的最后一道防线：
/// 即便 DOM 解析被绕过，也能在输出前把这类条目滤掉。
pub fn is_noise_result(r: &ParsedResult, original_query: &str) -> bool {
    let t = r.title.trim();

    // 标题形如「X (汉语文字)_百度百科」「X (汉字)_百度汉语」
    let dict_markers = [
        "(汉语文字)",
        "（汉语文字）",
        "(汉字)",
        "（汉字）",
        "_百度汉语",
        "_汉语词典",
        "汉语字典",
        "在线新华字典",
        "(字词释义)",
        "的意思_",
        "怎么读_",
    ];
    if dict_markers.iter().any(|m| t.contains(m)) {
        // 只有当查询本身不是单字时才判为噪音
        // （用户真去查「汇」字的话，这些结果是对的）
        if original_query
            .chars()
            .filter(|c| !c.is_whitespace())
            .count()
            > 2
        {
            return true;
        }
    }

    // 标题主体只有 1-2 个汉字，而查询有 4 个字以上 → 大概率是拆字组件
    let core: String = t
        .chars()
        .take_while(|c| !matches!(c, '(' | '（' | '_' | '-' | ' '))
        .collect();
    let core_len = core.chars().filter(|c| !c.is_whitespace()).count();
    let q_len = original_query
        .chars()
        .filter(|c| !c.is_whitespace())
        .count();
    if core_len > 0 && core_len <= 2 && q_len >= 4 {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_trailing_particles() {
        assert_eq!(sanitize_query("汇智信科如何"), "汇智信科");
        assert_eq!(sanitize_query("汇智信科怎么样？"), "汇智信科");
        assert_eq!(
            sanitize_query("请问汇智信科有哪些业务"),
            "汇智信科有哪些业务"
        );
    }

    #[test]
    fn sanitize_does_not_over_strip() {
        // 清洗后过短 → 退回原始
        assert_eq!(sanitize_query("如何"), "如何");
        // 不做中间替换，实体名完整保留
        assert_eq!(sanitize_query("西安汇智信息科技"), "西安汇智信息科技");
    }

    #[test]
    fn detects_dictionary_noise() {
        let r = ParsedResult {
            title: "汇 (汉语文字)_百度百科".into(),
            url: "https://baike.baidu.com/item/汇".into(),
            snippet: String::new(),
        };
        assert!(is_noise_result(&r, "汇智信科如何"));
        // 用户真的查单字时不算噪音
        assert!(!is_noise_result(&r, "汇"));
    }

    #[test]
    fn extracts_js_redirect() {
        let body = r#"<script>window.location.replace("https://www.example.com/a?b=1")</script>"#;
        assert_eq!(
            extract_redirect_target(body).as_deref(),
            Some("https://www.example.com/a?b=1")
        );
    }

    #[test]
    fn baidu_redirect_detected() {
        assert!(is_baidu_redirect("https://www.baidu.com/link?url=abc123"));
        assert!(!is_baidu_redirect("https://www.example.com"));
    }
}
