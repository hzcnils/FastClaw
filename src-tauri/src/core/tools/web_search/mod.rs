//! WebSearch 工具：多引擎并发检索 + 融合重排 + 正文抽取。
//!
//! 管线：
//!   L0 query   查询规划（降噪 / 实体抽取 / 引号精确 / site 限定）
//!   L1 engine  引擎池（API 优先，HTML 降级；可用性 + 熔断 + 中文路由）
//!   L2 exec    并发执行（每引擎超时 + 全局 deadline + 够数即返回）
//!   L3 fusion  URL 归一化去重 + RRF 融合
//!   L4 fusion  词面打分 + 域名先验 + 噪音过滤 + 权威域二段钻取
//!   L5 extract 跳板还原 + 编码识别 + DOM 密度抽正文
//!   L6 cache   singleflight + 正负样本分离 TTL
//!   L7 health  引擎健康度 + 熔断 + 诊断输出

// ════════════════════════════════════════════════════════════════
// 以下为管线各级的实现，按依赖顺序内联。
// 想拆成目录模块的话，把每个 `pub mod X { .. }` 的花括号内容
// 原样挪进 web_search/X.rs，再把这里换成 `pub mod X;` 即可。
// ════════════════════════════════════════════════════════════════

// ── FastClaw 迁移：answerer/closure/domains/search_planner 拆到本目录 ──
pub mod answerer;
pub mod closure;
pub mod domains;
pub mod llm_adapter;
pub mod search_planner;


pub mod dates {
    //! 日期抽取。
    //!
    //! 这是「何祖军已去世却报成 2024 年退休」的直接修复点：旧实现返回的结构里
    //! **没有任何时间字段**，模型看到一条 2025-01 发布、讲 2024-06 座谈会的消息，
    //! 无从判断它是不是最新的，于是把它当成了当前状态。
    //! 把日期抽出来交给模型，它才有可能意识到「这条可能已经过时」。

    /// 从标题/摘要/正文里抽第一个像样的日期，归一化成 YYYY-MM-DD 或 YYYY-MM。
    pub fn extract(text: &str) -> Option<String> {
        let cs: Vec<char> = text.chars().collect();
        let n = cs.len();
        let mut i = 0;
        while i + 3 < n {
            if !cs[i].is_ascii_digit() {
                i += 1;
                continue;
            }
            // 必须是恰好 4 位数字构成的年份
            let mut j = i;
            while j < n && cs[j].is_ascii_digit() {
                j += 1;
            }
            if j - i != 4 {
                i = j.max(i + 1);
                continue;
            }
            let year: i32 = cs[i..j].iter().collect::<String>().parse().ok()?;
            if !(1990..=2100).contains(&year) {
                i = j;
                continue;
            }
            if let Some((m, d, end)) = tail(&cs, j) {
                let out = match d {
                    Some(d) => format!("{year:04}-{m:02}-{d:02}"),
                    None => format!("{year:04}-{m:02}"),
                };
                let _ = end;
                return Some(out);
            }
            i = j;
        }
        None
    }

    /// 解析年份之后的「-月-日 / 年月日 / /月/日」部分。
    fn tail(cs: &[char], mut k: usize) -> Option<(u32, Option<u32>, usize)> {
        let n = cs.len();
        if k >= n {
            return None;
        }
        let sep = cs[k];
        if !matches!(sep, '-' | '/' | '.' | '年') {
            return None;
        }
        k += 1;
        let (m, nk) = num(cs, k, 2)?;
        if !(1..=12).contains(&m) {
            return None;
        }
        k = nk;
        if k >= n {
            return Some((m, None, k));
        }
        let sep2 = cs[k];
        if !matches!(sep2, '-' | '/' | '.' | '月') {
            return Some((m, None, k));
        }
        k += 1;
        let Some((d, nk)) = num(cs, k, 2) else { return Some((m, None, k)) };
        if !(1..=31).contains(&d) {
            return Some((m, None, k));
        }
        Some((m, Some(d), nk))
    }

    fn num(cs: &[char], start: usize, max: usize) -> Option<(u32, usize)> {
        let mut k = start;
        let mut v: u32 = 0;
        let mut cnt = 0;
        while k < cs.len() && cs[k].is_ascii_digit() && cnt < max {
            v = v * 10 + cs[k].to_digit(10)?;
            k += 1;
            cnt += 1;
        }
        if cnt == 0 {
            None
        } else {
            Some((v, k))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn 抽取常见中文日期() {
            assert_eq!(extract("发布于2026年2月7日 讣告"), Some("2026-02-07".into()));
            assert_eq!(extract("2025年1月8日"), Some("2025-01-08".into()));
            assert_eq!(extract("2024年6月"), Some("2024-06".into()));
        }

        #[test]
        fn 抽取数字日期() {
            assert_eq!(extract("2026-02-07 逝世"), Some("2026-02-07".into()));
            assert_eq!(extract("2025/01/08 更新"), Some("2025-01-08".into()));
        }

        #[test]
        fn 不把无关数字当日期() {
            assert_eq!(extract("电话 12345678"), None);
            assert_eq!(extract("享年 62 岁"), None);
            assert_eq!(extract("编号 1743285"), None);
        }

        #[test]
        fn 越界的月日被拒() {
            assert_eq!(extract("2026-13-45"), None);
        }
    }
}

pub mod rerank {
    //! Cross-encoder 重排。
    //!
    //! **这是检索质量的最大单点杠杆。** 词面打分只能看「字对上了没」，
    //! cross-encoder 是把 query 和文档拼在一起过一遍模型，能判断
    //! 「这一页到底回答了问题没有」。bge-reranker-v2-m3 这类模型在中文上
    //! 效果尤其明显。
    //!
    //! 配置（任选其一即可，没配就自动退回词面打分）：
    //!   RERANKER_URL   = http://127.0.0.1:8080/rerank     （TEI / Xinference）
    //!                  或 https://api.jina.ai/v1/rerank   （Jina / Cohere 兼容）
    //!   RERANKER_MODEL = bge-reranker-v2-m3
    //!   RERANKER_API_KEY = ...（可选）
    //!
    //! 两种响应格式都自动适配。

    use std::time::Duration;

    pub fn configured() -> Option<String> {
        std::env::var("RERANKER_URL").ok().filter(|v| !v.trim().is_empty())
    }

    /// 返回与 `docs` 等长的分数数组（越大越相关）。任何异常都返回 None，
    /// 由调用方退回词面打分——重排是增益项，绝不能变成单点故障。
    pub async fn score(query: &str, docs: &[String]) -> Option<Vec<f64>> {
        if docs.is_empty() {
            return None;
        }
        let url = configured()?;
        let model = std::env::var("RERANKER_MODEL").unwrap_or_else(|_| "bge-reranker-v2-m3".into());

        // 两种主流入参格式一起给，服务端各取所需
        let body = serde_json::json!({
            "query": query,
            "texts": docs,            // TEI / Xinference
            "documents": docs,        // Jina / Cohere
            "model": model,
            "top_n": docs.len(),
            "return_documents": false,
        });

        let mut req = crate::core::tools::web_search::http::serp_client()
            .post(&url)
            .timeout(Duration::from_millis(2500))
            .json(&body);
        if let Ok(key) = std::env::var("RERANKER_API_KEY") {
            if !key.trim().is_empty() {
                req = req.bearer_auth(key);
            }
        }

        let resp = match req.send().await {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                tracing::warn!(status = %r.status(), "重排服务返回非 2xx，退回词面打分");
                return None;
            }
            Err(e) => {
                tracing::warn!(error = %e, "重排服务不可达，退回词面打分");
                return None;
            }
        };
        let v: serde_json::Value = resp.json().await.ok()?;
        parse_scores(&v, docs.len())
    }

    /// 纯函数，便于单测：兼容 TEI 的裸数组和 Jina/Cohere 的 {results:[...]}。
    pub fn parse_scores(v: &serde_json::Value, n: usize) -> Option<Vec<f64>> {
        let arr = v
            .as_array()
            .cloned()
            .or_else(|| v.get("results").and_then(|r| r.as_array()).cloned())
            .or_else(|| v.get("data").and_then(|r| r.as_array()).cloned())?;

        let mut out = vec![f64::NEG_INFINITY; n];
        let mut any = false;
        for item in arr {
            let idx = item
                .get("index")
                .or_else(|| item.get("corpus_id"))
                .and_then(|x| x.as_u64())
                .map(|x| x as usize);
            let sc = item
                .get("score")
                .or_else(|| item.get("relevance_score"))
                .and_then(|x| x.as_f64());
            if let (Some(i), Some(s)) = (idx, sc) {
                if i < n {
                    out[i] = s;
                    any = true;
                }
            }
        }
        if !any {
            return None;
        }
        // 没被服务端返回的条目给个下限，避免 NEG_INFINITY 污染排序
        let min = out.iter().copied().filter(|x| x.is_finite()).fold(f64::MAX, f64::min);
        for x in out.iter_mut() {
            if !x.is_finite() {
                *x = min - 1.0;
            }
        }
        Some(out)
    }

    /// 把重排分归一化到 0..1，便于和其他分数线性叠加。
    pub fn normalize(scores: &[f64]) -> Vec<f64> {
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for s in scores {
            lo = lo.min(*s);
            hi = hi.max(*s);
        }
        if !(hi > lo) {
            return vec![0.5; scores.len()];
        }
        scores.iter().map(|s| (s - lo) / (hi - lo)).collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn 解析tei裸数组格式() {
            let v: serde_json::Value =
                serde_json::from_str(r#"[{"index":1,"score":0.9},{"index":0,"score":0.2}]"#).unwrap();
            assert_eq!(parse_scores(&v, 2), Some(vec![0.2, 0.9]));
        }

        #[test]
        fn 解析jina格式() {
            let v: serde_json::Value = serde_json::from_str(
                r#"{"results":[{"index":0,"relevance_score":0.71},{"index":1,"relevance_score":0.11}]}"#,
            )
            .unwrap();
            assert_eq!(parse_scores(&v, 2), Some(vec![0.71, 0.11]));
        }

        #[test]
        fn 服务端漏返的条目不会污染排序() {
            let v: serde_json::Value = serde_json::from_str(r#"[{"index":0,"score":0.5}]"#).unwrap();
            let s = parse_scores(&v, 3).unwrap();
            assert!(s[1].is_finite() && s[1] < s[0]);
        }

        #[test]
        fn 格式不认识时返回none() {
            let v: serde_json::Value = serde_json::from_str(r#"{"error":"boom"}"#).unwrap();
            assert_eq!(parse_scores(&v, 2), None);
        }

        #[test]
        fn 归一化处理全等分数() {
            assert_eq!(normalize(&[0.3, 0.3]), vec![0.5, 0.5]);
            assert_eq!(normalize(&[0.0, 1.0]), vec![0.0, 1.0]);
        }
    }
}

pub mod evidence {
    //! 证据片段抽取。
    //!
    //! 把整页前 3000 字丢给模型，等于让它自己在噪音里找答案。
    //! 这里按句切分，挑真正命中查询实体的句子交上去——
    //! 同样的 token 预算，信息密度高一个量级。

    use crate::core::tools::web_search::query::is_cjk;

    /// 中英文混合断句。
    pub fn sentences(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        for c in text.chars() {
            cur.push(c);
            let end = matches!(c, '。' | '！' | '？' | '；' | '\n' | '.' | '!' | '?' | ';');
            if end && cur.chars().filter(|x| !x.is_whitespace()).count() >= 8 {
                out.push(cur.trim().to_string());
                cur.clear();
            } else if cur.chars().count() > 400 {
                // 没有标点的长段落也得切，否则整页变成一句
                out.push(cur.trim().to_string());
                cur.clear();
            }
        }
        if cur.trim().chars().filter(|x| !x.is_whitespace()).count() >= 8 {
            out.push(cur.trim().to_string());
        }
        out
    }

    /// 挑出最能作为证据的若干句。
    pub fn pick(text: &str, entities: &[String], max: usize, budget_chars: usize) -> Vec<String> {
        let sents = sentences(text);
        if sents.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(f64, usize, &String)> = sents
            .iter()
            .enumerate()
            .map(|(i, s)| (sentence_score(s, entities), i, s))
            .filter(|(sc, _, _)| *sc > 0.0)
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max);
        // 按原文顺序输出，保持可读性
        scored.sort_by_key(|(_, i, _)| *i);

        let mut out = Vec::new();
        let mut used = 0usize;
        for (_, _, s) in scored {
            let n = s.chars().count();
            if used + n > budget_chars {
                break;
            }
            used += n;
            out.push(s.clone());
        }
        out
    }

    fn sentence_score(s: &str, entities: &[String]) -> f64 {
        let mut sc = 0.0;
        for e in entities {
            if e.chars().count() < 2 {
                continue;
            }
            if s.contains(e.as_str()) {
                sc += 2.0 + e.chars().count() as f64 * 0.1;
            }
        }
        if sc == 0.0 {
            return 0.0;
        }
        // 含日期的句子往往是事实性陈述，值钱
        if crate::core::tools::web_search::dates::extract(s).is_some() {
            sc += 1.0;
        }
        // 过短/过长都降权
        let n = s.chars().filter(|c| !c.is_whitespace()).count() as f64;
        let len_penalty = if n < 15.0 {
            0.5
        } else if n > 200.0 {
            0.7
        } else {
            1.0
        };
        // 中文密度高的句子更可能是正文而不是导航
        let cjk = s.chars().filter(|c| is_cjk(*c)).count() as f64;
        let density = if n > 0.0 { (cjk / n).max(0.3) } else { 0.3 };
        sc * len_penalty * (0.7 + 0.3 * density)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn 中文断句() {
            let s = sentences(
                "何祖军是江苏科技大学教授。他从事船舶控制研究！这个方向的成果多不多呢？",
            );
            assert_eq!(s.len(), 3, "{s:?}");
        }

        #[test]
        fn 过短的碎片不作为句子() {
            // 「好的。」「是吗？」这类碎片当证据没有价值，直接丢弃
            assert!(sentences("好的。是吗？").is_empty());
        }

        #[test]
        fn 无标点长段也会被切开() {
            let long = "何".repeat(1000);
            assert!(sentences(&long).len() > 1);
        }

        #[test]
        fn 挑出含实体的句子() {
            let text = "网站导航 首页 学院概况。\
                        何祖军，江苏科技大学自动化学院教授，主要研究船舶综合控制技术。\
                        版权所有 保留一切权利。";
            let ents = vec!["何祖军".to_string(), "江苏科技大学".to_string()];
            let picked = pick(text, &ents, 3, 500);
            assert_eq!(picked.len(), 1, "{picked:?}");
            assert!(picked[0].contains("自动化学院"));
        }

        #[test]
        fn 含日期的句子优先() {
            let text = "何祖军参加了活动。何祖军于2026年2月7日因病逝世。";
            let ents = vec!["何祖军".to_string()];
            let picked = pick(text, &ents, 1, 500);
            assert!(picked[0].contains("2026"), "{picked:?}");
        }

        #[test]
        fn 没有命中实体时返回空() {
            assert!(pick("完全无关的内容在这里出现。", &["何祖军".into()], 3, 500).is_empty());
        }

        #[test]
        fn 预算能截住输出() {
            let text = "何祖军教授在江苏科技大学工作多年成绩斐然。".repeat(20);
            let picked = pick(&text, &["何祖军".into()], 10, 50);
            let total: usize = picked.iter().map(|s| s.chars().count()).sum();
            assert!(total <= 50, "实际 {total}");
        }
    }
}

pub mod http {
    //! 全局共享的 HTTP 客户端。
    //!
    //! 旧实现每次调用都 `reqwest::Client::new()`：连接池、TLS 配置、DNS 解析器
    //! 全部重建，每个引擎白白多付一次 TCP+TLS 握手（~100–300ms）。
    //! 这里改为进程级单例，连接与 TLS session 全程复用。

    use std::sync::OnceLock;
    use std::time::Duration;

    pub const DESKTOP_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                                  (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

    /// 给搜索引擎用：cookie_store 打开（Bing/百度首访要种 cookie，否则会被丢到
    /// 同意页 / 安全验证页），重定向跟随 5 跳。
    pub fn serp_client() -> &'static reqwest::Client {
        static C: OnceLock<reqwest::Client> = OnceLock::new();
        C.get_or_init(|| build(true, Duration::from_secs(8)))
    }

    /// 给正文抓取用：单独一个池，避免慢站把搜索请求的连接位挤掉。
    pub fn page_client() -> &'static reqwest::Client {
        static C: OnceLock<reqwest::Client> = OnceLock::new();
        C.get_or_init(|| build(false, Duration::from_secs(12)))
    }

    fn build(cookies: bool, timeout: Duration) -> reqwest::Client {
        let b = reqwest::Client::builder()
            .user_agent(DESKTOP_UA)
            .redirect(reqwest::redirect::Policy::limited(5))
            .connect_timeout(Duration::from_secs(3))
            .timeout(timeout)
            .pool_max_idle_per_host(8)
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(60))
            .tcp_nodelay(true);
        let b = if cookies { b.cookie_store(true) } else { b };
        b.build().unwrap_or_else(|e| {
            tracing::error!(error = %e, "构建共享 HTTP 客户端失败，退化为默认客户端");
            reqwest::Client::new()
        })
    }

}

pub mod health {
    //! L7：引擎健康度 + 熔断器 + 延迟统计。
    //!
    //! 目的：任何一个引擎改版/被封/超时，不再拖累整体，也不再表现为
    //! 「静默返回 0 条」。连续失败到阈值后主动摘除一段时间，并在最终输出里
    //! 明确告诉调用方哪条通道挂了。

    use dashmap::DashMap;
    use std::sync::OnceLock;
    use std::time::{Duration, Instant};

    const TRIP_AFTER: u32 = 3; // 连续失败几次后熔断
    const BASE_COOLDOWN: Duration = Duration::from_secs(60);
    const MAX_COOLDOWN: Duration = Duration::from_secs(15 * 60);

    #[derive(Debug, Clone)]
    struct State {
        consecutive_failures: u32,
        open_until: Option<Instant>,
        ewma_ms: f64,
        total: u64,
        ok: u64,
    }

    impl Default for State {
        fn default() -> Self {
            Self { consecutive_failures: 0, open_until: None, ewma_ms: 400.0, total: 0, ok: 0 }
        }
    }

    fn table() -> &'static DashMap<&'static str, State> {
        static T: OnceLock<DashMap<&'static str, State>> = OnceLock::new();
        T.get_or_init(DashMap::new)
    }

    /// 熔断是否打开（打开 = 本轮跳过这个引擎）。
    pub fn is_open(engine: &'static str) -> bool {
        match table().get(engine) {
            Some(s) => s.open_until.map(|t| Instant::now() < t).unwrap_or(false),
            None => false,
        }
    }

    /// 熔断冷却还剩多久。用于半开探测：池子被抽干时，按剩余时间从短到长
    /// 把引擎放回来试探，而不是让池子单调收缩到空。
    pub fn cooldown_remaining(engine: &'static str) -> Duration {
        match table().get(engine) {
            Some(s) => s
                .open_until
                .map(|t| t.saturating_duration_since(Instant::now()))
                .unwrap_or_default(),
            None => Duration::ZERO,
        }
    }

    /// EWMA 延迟，用于给引擎排序：快的先发，慢的当补充。
    pub fn latency_ms(engine: &'static str) -> f64 {
        table().get(engine).map(|s| s.ewma_ms).unwrap_or(400.0)
    }

    pub fn record_ok(engine: &'static str, elapsed_ms: u64) {
        let mut e = table().entry(engine).or_default();
        e.consecutive_failures = 0;
        e.open_until = None;
        e.ewma_ms = e.ewma_ms * 0.7 + elapsed_ms as f64 * 0.3;
        e.total += 1;
        e.ok += 1;
    }

    pub fn record_fail(engine: &'static str) {
        let mut e = table().entry(engine).or_default();
        e.consecutive_failures = e.consecutive_failures.saturating_add(1);
        e.total += 1;
        if e.consecutive_failures >= TRIP_AFTER {
            let shift = (e.consecutive_failures - TRIP_AFTER).min(8);
            let cd = BASE_COOLDOWN
                .checked_mul(1u32 << shift)
                .unwrap_or(MAX_COOLDOWN)
                .min(MAX_COOLDOWN);
            e.open_until = Some(Instant::now() + cd);
            tracing::warn!(
                engine,
                failures = e.consecutive_failures,
                cooldown_s = cd.as_secs(),
                "引擎连续失败，熔断器打开"
            );
        }
    }

    /// 供诊断输出/健康检查接口使用。
    pub fn snapshot() -> Vec<(String, u64, u64, u64, bool)> {
        table()
            .iter()
            .map(|kv| {
                let s = kv.value();
                (
                    kv.key().to_string(),
                    s.total,
                    s.ok,
                    s.ewma_ms as u64,
                    s.open_until.map(|t| Instant::now() < t).unwrap_or(false),
                )
            })
            .collect()
    }

    #[cfg(test)]
    pub fn reset_for_test() {
        table().clear();
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn 连续失败达到阈值才熔断() {
            reset_for_test();
            assert!(!is_open("t1"));
            record_fail("t1");
            record_fail("t1");
            assert!(!is_open("t1"), "两次失败不应熔断");
            record_fail("t1");
            assert!(is_open("t1"), "三次失败应熔断");
        }

        #[test]
        fn 成功会复位熔断与失败计数() {
            reset_for_test();
            for _ in 0..5 {
                record_fail("t2");
            }
            assert!(is_open("t2"));
            record_ok("t2", 100);
            assert!(!is_open("t2"));
        }

        #[test]
        fn 延迟走ewma不会被单次尖刺带偏() {
            reset_for_test();
            record_ok("t3", 100);
            let a = latency_ms("t3");
            record_ok("t3", 5000);
            let b = latency_ms("t3");
            assert!(b > a && b < 5000.0, "EWMA 应平滑，实际 {b}");
        }
    }

}

pub mod cache {
    //! L6：带 singleflight 的 TTL 缓存。
    //!
    //! 相对旧实现修掉的三件事：
    //!   1. 旧版把「空结果」也按 5 分钟缓存 —— 一次解析失败，接下来五分钟所有
    //!      重试全部命中空缓存，一个 HTTP 请求都不会发出去。这里正/负样本
    //!      TTL 分离，失败只留很短时间。
    //!   2. 旧版没有 singleflight，同一 query 并发进来会重复打所有引擎。
    //!      这里用 `tokio::sync::OnceCell` 做请求合并。
    //!   3. 旧版超容量直接 `clear()` 全清。这里按创建时间淘汰最老的一批。

    use dashmap::DashMap;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::sync::OnceCell;

    struct Slot<T> {
        created: Instant,
        cell: Arc<OnceCell<Arc<T>>>,
    }

    impl<T> Clone for Slot<T> {
        fn clone(&self) -> Self {
            Self { created: self.created, cell: Arc::clone(&self.cell) }
        }
    }

    pub struct FlightCache<T> {
        map: DashMap<String, Slot<T>>,
        cap: usize,
    }

    impl<T: Send + Sync + 'static> FlightCache<T> {
        pub fn new(cap: usize) -> Self {
            Self { map: DashMap::new(), cap }
        }

        pub fn len(&self) -> usize {
            self.map.len()
        }

        pub fn is_empty(&self) -> bool {
            self.map.is_empty()
        }

        pub fn invalidate(&self, key: &str) {
            self.map.remove(key);
        }

        /// 取或算。`ttl_of` 根据算出来的值决定这一条能活多久
        /// （命中给长 TTL，空结果给短 TTL）。
        pub async fn get_or_init<F, Fut>(
            &self,
            key: &str,
            ttl_of: impl Fn(&T) -> Duration,
            mk: F,
        ) -> Arc<T>
        where
            F: Fn() -> Fut,
            Fut: std::future::Future<Output = T>,
        {
            // 最多重来一次：第一次可能拿到刚好过期的槽位。
            for _ in 0..2 {
                let slot = {
                    // 注意：DashMap 的 guard 绝不能跨 await 持有，这里立刻 clone 出来。
                    let entry = self.map.entry(key.to_string()).or_insert_with(|| Slot {
                        created: Instant::now(),
                        cell: Arc::new(OnceCell::new()),
                    });
                    entry.value().clone()
                };
                self.evict_if_needed();

                let cell = Arc::clone(&slot.cell);
                let value: Arc<T> = cell.get_or_init(|| async { Arc::new(mk().await) }).await.clone();

                if slot.created.elapsed() <= ttl_of(&value) {
                    return value;
                }
                // 过期：只有当 map 里还是同一个槽位时才移除，避免踩掉别人刚放进去的新值。
                self.map
                    .remove_if(key, |_, cur| Arc::ptr_eq(&cur.cell, &slot.cell));
            }
            Arc::new(mk().await)
        }

        fn evict_if_needed(&self) {
            if self.map.len() <= self.cap {
                return;
            }
            // 先按年龄砍掉最老的四分之一，而不是像旧实现那样整表 clear。
            let mut ages: Vec<(String, Instant)> =
                self.map.iter().map(|kv| (kv.key().clone(), kv.value().created)).collect();
            ages.sort_by_key(|(_, t)| *t);
            for (k, _) in ages.into_iter().take(self.cap / 4 + 1) {
                self.map.remove(&k);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::atomic::{AtomicUsize, Ordering};

        fn rt() -> tokio::runtime::Runtime {
            tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap()
        }

        #[test]
        fn 并发同key只计算一次() {
            rt().block_on(async {
                let c: Arc<FlightCache<u32>> = Arc::new(FlightCache::new(64));
                let calls = Arc::new(AtomicUsize::new(0));
                let mut hs = Vec::new();
                for _ in 0..16 {
                    let c = Arc::clone(&c);
                    let calls = Arc::clone(&calls);
                    hs.push(tokio::spawn(async move {
                        c.get_or_init(
                            "k",
                            |_| Duration::from_secs(60),
                            || {
                                let calls = Arc::clone(&calls);
                                async move {
                                    calls.fetch_add(1, Ordering::SeqCst);
                                    tokio::time::sleep(Duration::from_millis(30)).await;
                                    7u32
                                }
                            },
                        )
                        .await
                    }));
                }
                for h in hs {
                    assert_eq!(*h.await.unwrap(), 7);
                }
                assert_eq!(calls.load(Ordering::SeqCst), 1, "singleflight 未生效");
            });
        }

        #[test]
        fn 空结果用短ttl会尽快重算() {
            rt().block_on(async {
                let c: FlightCache<Vec<u32>> = FlightCache::new(64);
                let calls = Arc::new(AtomicUsize::new(0));
                let ttl = |v: &Vec<u32>| {
                    if v.is_empty() {
                        Duration::from_millis(20)
                    } else {
                        Duration::from_secs(300)
                    }
                };
                let mk = || {
                    let calls = Arc::clone(&calls);
                    async move {
                        let n = calls.fetch_add(1, Ordering::SeqCst);
                        if n == 0 {
                            Vec::new()
                        } else {
                            vec![1u32]
                        }
                    }
                };
                assert!(c.get_or_init("q", ttl, mk.clone()).await.is_empty());
                tokio::time::sleep(Duration::from_millis(40)).await;
                assert_eq!(*c.get_or_init("q", ttl, mk.clone()).await, vec![1]);
                // 有结果后进入长 TTL，不再重算
                assert_eq!(*c.get_or_init("q", ttl, mk).await, vec![1]);
                assert_eq!(calls.load(Ordering::SeqCst), 2);
            });
        }

        #[test]
        fn 超容量按年龄淘汰而非清空() {
            rt().block_on(async {
                let c: FlightCache<u32> = FlightCache::new(16);
                for i in 0..40u32 {
                    c.get_or_init(&format!("k{i}"), |_| Duration::from_secs(300), || async move { i })
                        .await;
                }
                assert!(c.len() <= 16 + 1, "实际 {}", c.len());
                assert!(!c.is_empty(), "不应被整表清空");
            });
        }
    }

}

pub mod query {
    //! L0：查询规划器。
    //!
    //! 一条自然语言 query 直接丢给 SERP 往往是最差的打法：中文分词会把
    //! 「何祖军 江苏科技大学」拆成单字，权重全跑到无关的字典条目上。
    //! 这里把 1 条 query 展开成若干条互补的子查询，后面并发打出去再融合。

    /// 句尾口语助词：只在结尾剥离。
    /// （不能无脑全局删——「如何学习 Rust」的「如何」在句首是有意义的。）
    const TAIL_FILLERS: &[&str] = &[
        "是什么意思", "是什么", "怎么样", "怎么办", "咋样", "如何", "呢", "吗", "啊",
        "的信息", "相关信息", "的资料", "介绍一下", "简介", "的简介", "是谁",
    ];

    /// 句首口语前缀。
    const HEAD_FILLERS: &[&str] = &[
        "请帮我查一下", "请帮我搜一下", "帮我查一下", "帮我搜一下", "帮我找一下",
        "我想知道", "请问一下", "搜索一下", "查一下", "搜一下", "帮我查", "帮我搜",
        "帮我找", "请问", "请帮我",
    ];

    /// 常见顶级/二级域后缀，用来识别 query 里自带的站点提示。
    const TLD_HINTS: &[&str] = &[
        ".edu.cn", ".gov.cn", ".ac.cn", ".org.cn", ".com.cn", ".net.cn",
        ".com", ".cn", ".org", ".net", ".edu", ".gov", ".io", ".dev",
    ];

    #[derive(Debug, Clone)]
    pub struct QueryPlan {
        /// 用户原始输入，用于最终排序时判断噪音。
        pub original: String,
        /// 降噪后的主查询。
        pub primary: String,
        /// 实际下发的子查询（primary 一定是第 0 条）。
        pub queries: Vec<String>,
        /// 抽出的实体（人名/机构名/引号短语），排序与噪音过滤都靠它。
        pub entities: Vec<String>,
        /// query 里自带的站点限定（如用户写了 just.edu.cn）。
        pub site_hint: Option<String>,
        pub has_cjk: bool,
        /// 时效性要求。用户问「最新/近况」时自动置为 Year，
        /// 也可由调用方显式指定。
        pub freshness: Freshness,
    }

    /// 时效窗口。落到各引擎的原生时间过滤参数上。
    ///
    /// 这是「结果已过时却看不出来」的正解：**按时间检索**，而不是去猜
    /// 「这个人是不是出了什么事」。同一套机制，不预设任何结论 ——
    /// 有新动态就返回新动态，没有就返回没有。
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum Freshness {
        #[default]
        Any,
        Day,
        Week,
        Month,
        Year,
    }

    impl Freshness {
        pub fn parse(s: &str) -> Freshness {
            match s.trim().to_ascii_lowercase().as_str() {
                "day" | "d" | "24h" => Freshness::Day,
                "week" | "w" => Freshness::Week,
                "month" | "m" => Freshness::Month,
                "year" | "y" => Freshness::Year,
                _ => Freshness::Any,
            }
        }

        pub fn wants_recency(self) -> bool {
            self != Freshness::Any
        }

        /// 近似窗口天数，用于给结果打新鲜度分。
        pub fn days(self) -> Option<i64> {
            match self {
                Freshness::Any => None,
                Freshness::Day => Some(1),
                Freshness::Week => Some(7),
                Freshness::Month => Some(31),
                Freshness::Year => Some(365),
            }
        }
    }

    impl QueryPlan {
        /// 是否像在查一个具体的人。
        pub fn looks_like_person(&self) -> bool {
            self.person_entity().is_some()
        }

        /// 取那个最像人名的实体。
        pub fn person_entity(&self) -> Option<&String> {
            self.entities.iter().find(|e| is_person_like(e))
        }
    }

    const NOT_PERSON: &[&str] = &[
        "公司", "大学", "学院", "医院", "集团", "中心", "研究所", "工程", "系统", "技术",
        "平台", "协议", "算法", "模型", "框架", "语言", "数据库",
    ];

    /// 2–4 字纯中文、且不带机构/概念后缀 —— 大概率是人名。
    pub fn is_person_like(e: &str) -> bool {
        let n = e.chars().count();
        (2..=4).contains(&n) && e.chars().all(is_cjk) && !NOT_PERSON.iter().any(|k| e.contains(k))
    }

    /// 用户措辞里的时效信号。命中就默认收敛到近一年。
    const RECENCY_MARKS: &[&str] = &[
        "最新", "近况", "现状", "目前", "现在", "如今", "近期", "还在", "是否还",
        "latest", "current", "recent", "still", "now",
    ];

    pub fn infer_freshness(q: &str) -> Freshness {
        if RECENCY_MARKS.iter().any(|m| q.contains(m)) {
            Freshness::Year
        } else {
            Freshness::Any
        }
    }

    pub fn is_cjk(c: char) -> bool {
        ('\u{4e00}'..='\u{9fff}').contains(&c)
            || ('\u{3400}'..='\u{4dbf}').contains(&c)
            || ('\u{f900}'..='\u{faff}').contains(&c)
    }

    pub fn contains_cjk(s: &str) -> bool {
        s.chars().any(is_cjk)
    }

    /// 剥离口语助词。保守：只动首尾，且不把 query 削到空。
    pub fn sanitize(raw: &str) -> String {
        let mut s = raw.trim().to_string();

        let mut changed = true;
        while changed {
            changed = false;
            for f in HEAD_FILLERS {
                if s.starts_with(f) && s.chars().count() > f.chars().count() + 1 {
                    s = s[f.len()..].trim_start_matches(['，', ',', '：', ':', ' ']).to_string();
                    changed = true;
                    break;
                }
            }
            for f in TAIL_FILLERS {
                if s.ends_with(f) && s.chars().count() > f.chars().count() + 1 {
                    s = s[..s.len() - f.len()].trim_end_matches(['的', '，', ',', ' ']).to_string();
                    changed = true;
                    break;
                }
            }
        }

        let s = s.trim().to_string();
        if s.is_empty() {
            raw.trim().to_string()
        } else {
            s
        }
    }

    /// 抽实体：引号短语 > 纯中文词块（2–8 字）> 连续大写开头的英文词组。
    pub fn entities(q: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();

        // 1) 显式引号短语优先
        let mut rest = String::new();
        let mut buf = String::new();
        let mut in_quote = false;
        for c in q.chars() {
            match c {
                '"' | '“' | '”' | '「' | '」' => {
                    if in_quote {
                        if !buf.trim().is_empty() {
                            out.push(buf.trim().to_string());
                        }
                        buf.clear();
                        in_quote = false;
                    } else {
                        in_quote = true;
                    }
                }
                _ => {
                    if in_quote {
                        buf.push(c)
                    } else {
                        rest.push(c)
                    }
                }
            }
        }
        if !buf.trim().is_empty() {
            rest.push_str(&buf);
        }

        // 2) 空白/标点切块，取纯中文词块
        for tok in rest.split(|c: char| c.is_whitespace() || "，,、；;：:（）()【】[]?？!！".contains(c)) {
            let tok = tok.trim();
            if tok.is_empty() {
                continue;
            }
            let n = tok.chars().count();
            if tok.chars().all(is_cjk) && (2..=10).contains(&n) {
                out.push(tok.to_string());
            } else if !contains_cjk(tok)
                && n >= 3
                && tok.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false)
            {
                out.push(tok.to_string());
            }
        }

        out.dedup();
        out.truncate(4);
        out
    }

    fn detect_site_hint(q: &str) -> Option<String> {
        for tok in q.split_whitespace() {
            let t = tok.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '-');
            if let Some(rest) = t.strip_prefix("site:") {
                if !rest.is_empty() {
                    return Some(rest.to_string());
                }
            }
            if t.contains('.') && !t.contains('/') && t.len() >= 5 {
                if TLD_HINTS.iter().any(|s| t.ends_with(s)) {
                    return Some(t.to_string());
                }
            }
        }
        None
    }

    pub fn plan(raw: &str) -> QueryPlan {
        let original = raw.trim().to_string();
        let primary = sanitize(&original);
        let ents = entities(&primary);
        let site_hint = detect_site_hint(&original);
        let has_cjk = contains_cjk(&primary);

        let mut queries = vec![primary.clone()];

        // 变体 A：把实体加引号做精确匹配。这一条专治「中文被拆成单字」。
        if primary.find('"').is_none() && ents.len() >= 1 {
            let mut exact = primary.clone();
            let mut applied = false;
            for e in ents.iter().filter(|e| e.chars().count() >= 2 && e.chars().count() <= 6) {
                if exact.contains(e.as_str()) {
                    exact = exact.replacen(e.as_str(), &format!("\"{e}\""), 1);
                    applied = true;
                }
            }
            if applied && exact != primary {
                queries.push(exact);
            }
        }

        // 变体 B：用户自己给了域名 → 直接 site: 限定，这是命中率最高的一条。
        if let Some(site) = &site_hint {
            if !original.contains("site:") {
                let entity_part = ents
                    .iter()
                    .filter(|e| !e.contains('.'))
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ");
                let q = if entity_part.is_empty() {
                    format!("site:{site} {primary}")
                } else {
                    format!("site:{site} {entity_part}")
                };
                queries.push(q);
            }
        }

        // 人物查询补一条「{名字} 简介」。
        // 豆包查「惠晓滨」用的两个关键词就是「惠晓滨 简介」和「惠晓滨」——
        // 「简介」直接命中人物聚合页/工商登记/机构师资页，
        // 比堆砌「教授 研究方向 论文」这类词有效得多。
        if !primary.contains("简介") {
            if let Some(name) = ents.iter().find(|e| is_person_like(e)) {
                queries.push(format!("{name} 简介"));
            }
        }

        queries.dedup();
        // 上限 3：豆包只发 2 个关键词就够了。请求数不是越多越准，
        // 反而拉长尾延迟、更容易触发反爬。
        queries.truncate(3);

        let freshness = infer_freshness(&original);

        QueryPlan { original, primary, queries, entities: ents, site_hint, has_cjk, freshness }
    }

    /// 二段钻取用：对某个权威域名再打一发定向查询。
    pub fn drill_query(plan: &QueryPlan, host: &str) -> String {
        let entity_part = plan
            .entities
            .iter()
            .filter(|e| !e.contains('.'))
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        if entity_part.trim().is_empty() {
            format!("site:{host} {}", plan.primary)
        } else {
            format!("site:{host} {entity_part}")
        }
    }

    /// 缓存 key：对大小写、首尾空格、内部多空格不敏感。
    pub fn cache_key(plan: &QueryPlan, engines: &str, num: usize) -> String {
        let norm: String = plan.primary.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
        format!("{norm}|{engines}|{num}|{:?}", plan.freshness)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn 剥离句尾口语助词() {
            assert_eq!(sanitize("汇智信科如何"), "汇智信科");
            assert_eq!(sanitize("何祖军 江苏科技大学 的信息"), "何祖军 江苏科技大学");
            assert_eq!(sanitize("帮我查一下 何祖军 江苏科技大学"), "何祖军 江苏科技大学");
        }

        #[test]
        fn 句首如何不会被误删() {
            // 「如何」在句首是实义的，不能当助词剥掉
            assert_eq!(sanitize("如何学习 Rust"), "如何学习 Rust");
        }

        #[test]
        fn 不会把query削成空() {
            assert_eq!(sanitize("如何"), "如何");
            assert_eq!(sanitize("请问"), "请问");
        }

        #[test]
        fn 抽出中文实体() {
            let e = entities("何祖军 江苏科技大学");
            assert!(e.contains(&"何祖军".to_string()), "{e:?}");
            assert!(e.contains(&"江苏科技大学".to_string()), "{e:?}");
        }

        #[test]
        fn 引号短语优先作为实体() {
            let e = entities("\"何祖军\" 教授");
            assert_eq!(e[0], "何祖军");
        }

        #[test]
        fn 生成精确匹配变体() {
            let p = plan("何祖军 江苏科技大学");
            assert_eq!(p.queries[0], "何祖军 江苏科技大学");
            assert!(
                p.queries.iter().any(|q| q.contains("\"何祖军\"")),
                "应生成引号精确变体，实际 {:?}",
                p.queries
            );
        }

        #[test]
        fn 识别域名并生成site变体() {
            let p = plan("何祖军 just.edu.cn");
            assert_eq!(p.site_hint.as_deref(), Some("just.edu.cn"));
            assert!(
                p.queries.iter().any(|q| q.starts_with("site:just.edu.cn")),
                "{:?}",
                p.queries
            );
        }

        #[test]
        fn 钻取查询带上实体而不是整句() {
            let p = plan("何祖军 江苏科技大学 教授主页");
            let d = drill_query(&p, "just.edu.cn");
            assert!(d.starts_with("site:just.edu.cn"));
            assert!(d.contains("何祖军"));
        }

        #[test]
        fn 缓存key归一化() {
            let a = plan("  何祖军   江苏科技大学 ");
            let b = plan("何祖军 江苏科技大学");
            assert_eq!(cache_key(&a, "all", 5), cache_key(&b, "all", 5));
        }

        #[test]
        fn 子查询数量有上限() {
            let p = plan("何祖军 江苏科技大学 just.edu.cn 教授");
            assert!(p.queries.len() <= 3);
        }
    }

}

pub mod serp {
    //! SERP 解析层。
    //!
    //! 旧实现用 `html.split("class=\"b_algo\"")` 这类字符串切割：
    //!   * Bing 一旦输出 `class="b_algo b_algoBorder"`，带闭合引号的子串就不存在了 → 0 条；
    //!   * 百度按 `class="result` 全页切，右栏「百度汉语」拆字卡片被当成搜索结果
    //!     —— 这就是「何祖军」被拆成单字的直接原因。
    //!
    //! 这里改成：**限定作用域的 DOM 选择 + 每个字段多套 fallback 选择器**。
    //! 任何一套失配，还有下一套；全部失配才判定为解析器失效（而不是「没结果」）。

    use scraper::{ElementRef, Html, Node, Selector};

    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
    pub struct RawResult {
        pub title: String,
        pub url: String,
        pub snippet: String,
    }

    /// 一个引擎的 SERP 结构描述。改版时只需要在这里加一条选择器，
    /// 不用碰任何解析逻辑。
    pub struct SerpSpec {
        pub engine: &'static str,
        /// 用于解析相对 href。搜狗/百度部分结果给的是 `/link?url=...`，
        /// 旧的 `usable_href` 直接丢弃，等于白丢一半结果。
        pub base: &'static str,
        /// 结果区容器。**必须限定**，否则会把侧栏/广告/推荐位一起吃进来。
        pub scope: &'static [&'static str],
        pub item: &'static [&'static str],
        pub link: &'static [&'static str],
        pub title: &'static [&'static str],
        pub snippet: &'static [&'static str],
    }

    pub const BING: SerpSpec = SerpSpec {
        engine: "bing",
        base: "https://www.bing.com/",
        scope: &["#b_results", "ol#b_results", "main"],
        item: &["li.b_algo", "li[class~=\"b_algo\"]", "li.b_ans + li", ".b_algo"],
        link: &["h2 a[href]", "a.tilk[href]", "h2 > a", "a[href]"],
        title: &["h2 a", "h2", ".b_topTitle"],
        snippet: &[".b_caption p", "p.b_lineclamp4", ".b_snippet", "p"],
    };

    pub const BAIDU: SerpSpec = SerpSpec {
        engine: "baidu",
        base: "https://www.baidu.com/",
        // 关键：只认 #content_left。右栏 #content_right 里的汉语词典/拆字卡片不进来。
        scope: &["#content_left", "div#content_left"],
        item: &["div.result", "div.c-container", "div[tpl]", "div[mu]"],
        link: &["h3 a[href]", "a.c-title[href]", "h3 > a", "a[href]"],
        title: &["h3 a", "h3", "a.c-title", ".t"],
        snippet: &[
            "[class*=content-right]",
            ".c-abstract",
            "[class*=abstract]",
            ".c-span-last span",
            "span.content-right_2s-H4",
        ],
    };

    pub const DUCKDUCKGO: SerpSpec = SerpSpec {
        engine: "duckduckgo",
        base: "https://html.duckduckgo.com/",
        scope: &["div#links", ".results", ".serp__results", "body"],
        item: &["div.result", "div.web-result", "div.results_links"],
        link: &["a.result__a[href]", "h2 a[href]", "a[href]"],
        title: &["a.result__a", "h2 a", "h2"],
        snippet: &["a.result__snippet", ".result__snippet", ".result__extras"],
    };

    /// 搜狗：与百度是两套不同的索引，对中文长尾覆盖差异明显。
    pub const SOGOU: SerpSpec = SerpSpec {
        engine: "sogou",
        base: "https://www.sogou.com/",
        scope: &["div.results", "#main .results", "#wrapper"],
        item: &["div.vrwrap", "div.rb", "div.results > div"],
        link: &["h3 a[href]", "a.pt[href]", "h3 > a", "a[href]"],
        title: &["h3 a", "h3.vr-title", "h3"],
        snippet: &[".str-info", ".ft", ".str_info", "div.fz-mid"],
    };

    /// 搜狗微信：公众号是中文机构通知的主要出口之一，
    /// 大量高校/单位的公告只在公众号发，主流网页索引里根本没有。
    /// 这是逼近豆包覆盖面最有效的一条增量通道。
    pub const WEIXIN: SerpSpec = SerpSpec {
        engine: "weixin",
        base: "https://weixin.sogou.com/",
        scope: &["ul.news-list", ".news-box", "#main"],
        item: &["ul.news-list > li", "li"],
        link: &["h3 a[href]", "a[href]"],
        title: &["h3 a", "h3"],
        snippet: &["p.txt-info", ".txt-box p", "p"],
    };

    /// 360 搜索：第三套独立中文索引。
    pub const SO360: SerpSpec = SerpSpec {
        engine: "so360",
        base: "https://www.so.com/",
        scope: &["ul.result", "#main", ".result"],
        item: &["li.res-list", "div.res-list", "ul.result > li"],
        link: &["h3 a[href]", "a[href]"],
        title: &["h3 a", "h3"],
        snippet: &["p.res-desc", ".res-desc", "p"],
    };

    /// 解析结果三态：**「解析不出来」和「确实没有」必须分开**，
    /// 否则页面改版会被静默当成「网上没有这个人」。
    #[derive(Debug)]
    pub enum ParseOutcome {
        Ok(Vec<RawResult>),
        /// HTML 非空且体量像正常 SERP，却一条都没解析出来 → 解析器失效
        ParserFailed { html_len: usize, engine: &'static str },
        /// HTML 为空 / 体量太小 → 请求本身没拿到东西
        Empty,
    }

    const SERP_MIN_LEN: usize = 5_000;

    pub fn parse(html: &str, spec: &SerpSpec, max: usize) -> ParseOutcome {
        if html.trim().is_empty() {
            return ParseOutcome::Empty;
        }
        let results = parse_inner(html, spec, max);
        if !results.is_empty() {
            return ParseOutcome::Ok(results);
        }
        if html.len() > SERP_MIN_LEN {
            tracing::error!(
                engine = spec.engine,
                html_len = html.len(),
                "SERP 解析失败 —— 页面结构可能已变更，请更新 SerpSpec 并补 fixture"
            );
            dump_fixture(spec.engine, html);
            ParseOutcome::ParserFailed { html_len: html.len(), engine: spec.engine }
        } else {
            ParseOutcome::Empty
        }
    }

    /// 解析失败时把原始 HTML 落盘，方便直接转成回归 fixture。
    /// 设 `WEB_SEARCH_DUMP_DIR` 才生效。
    fn dump_fixture(engine: &str, html: &str) {
        let Ok(dir) = std::env::var("WEB_SEARCH_DUMP_DIR") else { return };
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = std::path::Path::new(&dir).join(format!("{engine}-{ts}.html"));
        let _ = std::fs::create_dir_all(&dir);
        if std::fs::write(&path, html).is_ok() {
            tracing::warn!(?path, "已保存失败 SERP 快照");
        }
    }

    fn parse_inner(html: &str, spec: &SerpSpec, max: usize) -> Vec<RawResult> {
        // 注意：`Html` 不是 Send，整段解析必须是同步的，绝不能跨 .await。
        let doc = Html::parse_document(html);

        // 1) 定作用域：第一个有命中的 scope 选择器胜出
        let mut scopes: Vec<ElementRef> = Vec::new();
        for s in spec.scope {
            if let Ok(sel) = Selector::parse(s) {
                let found: Vec<ElementRef> = doc.select(&sel).collect();
                if !found.is_empty() {
                    scopes = found;
                    break;
                }
            }
        }

        // 2) 找条目：同样是第一个有命中的 item 选择器胜出
        let mut items: Vec<ElementRef> = Vec::new();
        for s in spec.item {
            let Ok(sel) = Selector::parse(s) else { continue };
            let found: Vec<ElementRef> = if scopes.is_empty() {
                doc.select(&sel).collect()
            } else {
                scopes.iter().flat_map(|r| r.select(&sel)).collect()
            };
            if !found.is_empty() {
                items = found;
                break;
            }
        }
        if items.is_empty() {
            return Vec::new();
        }

        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for item in items {
            if out.len() >= max {
                break;
            }
            let Some((url, anchor)) = pick_link(&item, spec.link, spec.base) else { continue };
            if !seen.insert(url.clone()) {
                continue;
            }

            let title = pick_text(&item, spec.title)
                .or_else(|| anchor.map(|a| visible_text(a)))
                .map(|t| squeeze(&t))
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| "(无标题)".to_string());

            let snippet = pick_text(&item, spec.snippet)
                .map(|s| squeeze(&s))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    // 兜底：拿条目正文减掉标题，截断
                    let all = squeeze(&visible_text(item));
                    let body = all.strip_prefix(title.as_str()).unwrap_or(&all).trim();
                    body.chars().take(200).collect()
                });

            out.push(RawResult { title, url, snippet });
        }
        out
    }

    fn pick_link<'a>(
        item: &ElementRef<'a>,
        sels: &[&str],
        base: &str,
    ) -> Option<(String, Option<ElementRef<'a>>)> {
        for s in sels {
            let Ok(sel) = Selector::parse(s) else { continue };
            for a in item.select(&sel) {
                let Some(href) = a.value().attr("href") else { continue };
                let href = href.trim();
                if let Some(u) = usable_href(href, base) {
                    return Some((u, Some(a)));
                }
            }
        }
        None
    }

    fn usable_href(href: &str, base: &str) -> Option<String> {
        if href.is_empty() || href.starts_with('#') {
            return None;
        }
        let lower = href.to_ascii_lowercase();
        if lower.starts_with("javascript:")
            || lower.starts_with("mailto:")
            || lower.starts_with("data:")
        {
            return None;
        }
        if href.starts_with("//") {
            return Some(format!("https:{href}"));
        }
        if lower.starts_with("http://") || lower.starts_with("https://") {
            return Some(href.to_string());
        }
        // 相对路径：拿引擎 base 补全（搜狗的 /link?url= 就是这种）
        if href.starts_with('/') {
            if let Ok(b) = url::Url::parse(base) {
                if let Ok(joined) = b.join(href) {
                    return Some(joined.to_string());
                }
            }
        }
        None
    }

    fn pick_text(item: &ElementRef, sels: &[&str]) -> Option<String> {
        for s in sels {
            let Ok(sel) = Selector::parse(s) else { continue };
            if let Some(n) = item.select(&sel).next() {
                let t = visible_text(n);
                if !t.trim().is_empty() {
                    return Some(t);
                }
            }
        }
        None
    }

    /// 不该出现在可见文本里的标签。
    pub const INVISIBLE_TAGS: &[&str] = &[
        "script", "style", "noscript", "template", "svg", "iframe", "head", "title", "meta", "link",
    ];

    /// 取元素的可见文本：跳过 script/style 等子树。
    /// （`ElementRef::text()` 会把 `<script>` 里的 JS 也吐出来。）
    pub fn visible_text(el: ElementRef) -> String {
        let mut out = String::new();
        let mut stack: Vec<_> = el.children().rev().collect();
        while let Some(n) = stack.pop() {
            match n.value() {
                Node::Text(t) => {
                    out.push_str(t);
                    out.push(' ');
                }
                Node::Element(e) => {
                    if INVISIBLE_TAGS.contains(&e.name()) {
                        continue;
                    }
                    for c in n.children().rev() {
                        stack.push(c);
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// 折叠空白。按 char 迭代，天然 UTF-8 安全 —— 旧实现在这里用 char 下标去切
    /// byte 切片，中文页面必 panic。
    pub fn squeeze(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut last_ws = false;
        for c in s.chars() {
            if c.is_whitespace() {
                if !last_ws && !out.is_empty() {
                    out.push(' ');
                }
                last_ws = true;
            } else {
                out.push(c);
                last_ws = false;
            }
        }
        out.trim().to_string()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn pad(s: &str) -> String {
            // 撑到 SERP_MIN_LEN 以上，模拟真实页面体量
            format!("{s}<!--{}-->", "x".repeat(6000))
        }

        const BING_HTML: &str = r#"
        <html><body>
          <ol id="b_results">
            <li class="b_ad"><h2><a href="https://ad.example.com">广告位</a></h2></li>
            <li class="b_algo b_algoBorder">
              <h2><a href="https://just.edu.cn/a.htm">何祖军 - 江苏科技大学</a></h2>
              <div class="b_caption"><p>计算机学院教师主页。</p></div>
            </li>
            <li class="b_algo">
              <h2><a href="https://m.kaoyan.com/x">导师介绍：何祖军</a></h2>
              <div class="b_caption"><p>考研帮导师库</p></div>
            </li>
          </ol>
        </body></html>"#;

        #[test]
        fn bing带附加class也能解析() {
            // 旧实现 split("class=\"b_algo\"") 在 b_algo b_algoBorder 上直接 0 条
            match parse(&pad(BING_HTML), &BING, 5) {
                ParseOutcome::Ok(r) => {
                    assert_eq!(r.len(), 2, "{r:#?}");
                    assert_eq!(r[0].url, "https://just.edu.cn/a.htm");
                    assert!(r[0].title.contains("何祖军"));
                    assert!(r[0].snippet.contains("计算机学院"));
                }
                o => panic!("应解析成功，实际 {o:?}"),
            }
        }

        #[test]
        fn bing广告位不进结果() {
            if let ParseOutcome::Ok(r) = parse(&pad(BING_HTML), &BING, 5) {
                assert!(!r.iter().any(|x| x.url.contains("ad.example.com")));
            } else {
                panic!()
            }
        }

        const BAIDU_HTML: &str = r#"
        <html><body>
          <div id="content_left">
            <div class="result c-container new-pmd" tpl="se_com_default">
              <h3 class="t c-title"><a href="https://www.baidu.com/link?url=ABC">何祖军_江苏科技大学</a></h3>
              <div class="c-abstract">江苏科技大学计算机学院。</div>
            </div>
          </div>
          <div id="content_right">
            <div class="result-op c-container">
              <h3 class="t"><a href="https://hanyu.baidu.com/zici/s?wd=何">何 (汉语文字)_百度汉语</a></h3>
              <div class="c-abstract">何，汉语一级字，读作hé…</div>
            </div>
          </div>
        </body></html>"#;

        #[test]
        fn 百度只取左栏不吃右栏拆字卡片() {
            // 这一条就是「何祖军被拆成单字」的回归测试
            match parse(&pad(BAIDU_HTML), &BAIDU, 5) {
                ParseOutcome::Ok(r) => {
                    assert_eq!(r.len(), 1, "{r:#?}");
                    assert!(r[0].title.contains("何祖军"));
                    assert!(
                        !r.iter().any(|x| x.url.contains("hanyu.baidu.com")),
                        "右栏汉语词典卡片不应进入结果"
                    );
                }
                o => panic!("应解析成功，实际 {o:?}"),
            }
        }

        #[test]
        fn 非空大页面零结果判定为解析失效() {
            let html = format!("<html><body>{}</body></html>", "x".repeat(20_000));
            match parse(&html, &BING, 5) {
                ParseOutcome::ParserFailed { engine, .. } => assert_eq!(engine, "bing"),
                o => panic!("应判定 ParserFailed，实际 {o:?}"),
            }
        }

        #[test]
        fn 空或小页面判定为empty() {
            assert!(matches!(parse("", &BING, 5), ParseOutcome::Empty));
            assert!(matches!(parse("   ", &BING, 5), ParseOutcome::Empty));
            assert!(matches!(parse("<html></html>", &BING, 5), ParseOutcome::Empty));
        }

        #[test]
        fn 可见文本跳过script和style() {
            let doc = Html::parse_document(
                "<div>前<script>var x=1;</script><style>.a{}</style>后</div>",
            );
            let sel = Selector::parse("div").unwrap();
            let t = squeeze(&visible_text(doc.select(&sel).next().unwrap()));
            assert_eq!(t, "前 后");
        }

        #[test]
        fn 中文页面折叠空白不会panic() {
            let s = "改\u{00a0}\u{00a0}名   测试\n\n中文";
            assert_eq!(squeeze(s), "改 名 测试 中文");
        }

        #[test]
        fn html实体由解析器自动解码() {
            let html = pad(
                r#"<ol id="b_results"><li class="b_algo"><h2><a href="https://e.com/?a=1&amp;b=2">A &amp; B</a></h2></li></ol>"#,
            );
            if let ParseOutcome::Ok(r) = parse(&html, &BING, 5) {
                assert_eq!(r[0].title, "A & B");
                assert_eq!(r[0].url, "https://e.com/?a=1&b=2");
            } else {
                panic!()
            }
        }

        #[test]
        fn 协议相对链接补全为https() {
            let html = pad(
                r#"<ol id="b_results"><li class="b_algo"><h2><a href="//x.com/p">T</a></h2></li></ol>"#,
            );
            if let ParseOutcome::Ok(r) = parse(&html, &BING, 5) {
                assert_eq!(r[0].url, "https://x.com/p");
            } else {
                panic!()
            }
        }
    }

}

pub mod fusion {
    //! L3 融合 + L4 重排。
    //!
    //! 旧实现的合并策略是「谁先返回谁占坑」（`seen_urls.insert` 先到先得），
    //! 结果质量完全取决于哪个引擎网络快 —— 等于把排序权交给了抖动。
    //!
    //! 这里换成 RRF（Reciprocal Rank Fusion）：每个引擎的名次贡献 1/(K+rank)，
    //! 多引擎共识的结果自然上浮。再叠一层词面打分 + 域名先验 + 噪音过滤。

    use crate::core::tools::web_search::query::{is_cjk, QueryPlan};
    use crate::core::tools::web_search::serp::RawResult;
    use std::collections::HashMap;
    use url::Url;

    /// RRF 的平滑常数，业界经验值 60。
    const RRF_K: f64 = 60.0;

    /// 仅用于 dedup key 的激进清洗参数。
    const KEY_STRIP: &[&str] = &[
        "utm_source", "utm_medium", "utm_campaign", "utm_term", "utm_content", "spm", "from",
        "ref", "referrer", "fr", "bd_source", "share_source", "share_medium", "gclid", "fbclid",
        "msclkid", "ved", "ei", "usg", "sourceid", "rsv_pq", "rsv_t", "rsv_spt", "hisfilter", "ie",
    ];

    /// 会写进最终 URL 的保守清洗参数（不敢乱删，避免打不开）。
    const DISPLAY_STRIP: &[&str] = &[
        "utm_source", "utm_medium", "utm_campaign", "utm_term", "utm_content", "gclid", "fbclid",
        "msclkid", "spm", "share_source", "share_medium",
    ];

    /// 该域名是否值得发起正文抓取。黑名单（需登录/JS 渲染的站）来自 data/deny.txt。
    pub fn fetchable(host: &str) -> bool {
        crate::core::tools::web_search::domains::registry()
            .unfetchable
            .iter()
            .all(|h| host != h.as_str() && !host.ends_with(&format!(".{h}")))
    }

    // 人物实体源 / 字典站 / 低质站 / 不可抓站已抽到 data/domains.txt 与
    // data/deny.txt，由 crate::core::tools::web_search::domains::registry() 加载（见下方 fetchable/domain_prior/is_noise）。
    // 此处只保留「规则/模式」类常量（权威后缀、字典标题标记）。

    const DICT_TITLE_MARKS: &[&str] = &[
        "_百度汉语", "百度汉语", "_汉语词典", "汉语字典", "新华字典", "康熙字典", "在线字典",
        "的意思、", "怎么读", "部首", "笔画", "五笔", "拼音", "组词", "近义词", "反义词",
    ];

    const AUTH_SUFFIX: &[(&str, f64)] = &[
        (".edu.cn", 0.60),
        (".gov.cn", 0.60),
        (".ac.cn", 0.55),
        (".org.cn", 0.25),
        (".edu", 0.40),
        (".gov", 0.40),
        (".ac.uk", 0.40),
        (".org", 0.12),
    ];

    #[derive(Debug, Clone, serde::Serialize)]
    pub struct Ranked {
        pub title: String,
        pub url: String,
        pub snippet: String,
        pub content: Option<String>,
        pub host: String,
        pub engines: Vec<&'static str>,
        /// 是否是跳板链接（百度 link?url= 之类），需要二次解析真实地址
        pub needs_resolve: bool,
        /// 从标题/摘要抽出的发布日期，让上层能判断结果是否已过时
        pub date: Option<String>,
        /// 正文里真正命中查询实体的句子
        pub evidence: Vec<String>,
        /// cross-encoder 重排分（未启用重排时为 None）
        pub ce: Option<f64>,
        pub rrf: f64,
        pub lex: f64,
        pub prior: f64,
        pub score: f64,
    }

    // ───────────────────────── URL 归一化 ─────────────────────────

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Norm {
        /// 写进最终结果、用于 deep_fetch 的地址
        pub display: String,
        /// 仅用于去重比对的规范化 key
        pub key: String,
        pub host: String,
        pub needs_resolve: bool,
    }

    /// 归一化并解开引擎跳板链接。
    pub fn normalize(raw: &str) -> Option<Norm> {
        normalize_depth(raw, 0)
    }

    fn normalize_depth(raw: &str, depth: u8) -> Option<Norm> {
        if depth > 2 {
            return None;
        }
        let u = Url::parse(raw).ok()?;
        if !matches!(u.scheme(), "http" | "https") {
            return None;
        }
        let host = u.host_str()?.to_ascii_lowercase();

        // DuckDuckGo 的 /l/?uddg=<encoded> 跳板：query_pairs 会自动百分号解码
        if host.ends_with("duckduckgo.com") && u.path().starts_with("/l/") {
            if let Some((_, v)) = u.query_pairs().find(|(k, _)| k == "uddg") {
                return normalize_depth(&v, depth + 1);
            }
        }

        // 百度 link?url=<加密> 与 Bing ck/a?u=<base64>：本地解不开，标记为待二次解析。
        // 旧实现直接把跳板 URL 交给 deep_fetch，抓回来的全是跳板页里的 JS。
        let needs_resolve = (host.ends_with("baidu.com") && u.path().starts_with("/link"))
            || (host.ends_with("bing.com") && u.path().starts_with("/ck/"))
            || (host.ends_with("sogou.com") && u.path().starts_with("/link"))
            || (host.ends_with("so.com") && u.path().starts_with("/link"));

        Some(Norm {
            display: strip_params(&u, DISPLAY_STRIP, false),
            key: strip_params(&u, KEY_STRIP, true),
            host,
            needs_resolve,
        })
    }

    fn strip_params(u: &Url, strip: &[&str], canonical: bool) -> String {
        let mut out = u.clone();
        out.set_fragment(None);

        let kept: Vec<(String, String)> = u
            .query_pairs()
            .filter(|(k, _)| !strip.iter().any(|s| k.eq_ignore_ascii_case(s)))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        if kept.is_empty() {
            out.set_query(None);
        } else {
            let mut ser = out.query_pairs_mut();
            ser.clear();
            for (k, v) in &kept {
                ser.append_pair(k, v);
            }
            drop(ser);
        }

        if canonical {
            if let Some(h) = out.host_str() {
                let h = h.to_ascii_lowercase();
                let bare = h
                    .strip_prefix("www.")
                    .or_else(|| h.strip_prefix("m."))
                    .or_else(|| h.strip_prefix("mobile."))
                    .unwrap_or(&h)
                    .to_string();
                let _ = out.set_host(Some(&bare));
            }
            let _ = out.set_scheme("https");
            let p = out.path().trim_end_matches('/').to_string();
            out.set_path(if p.is_empty() { "/" } else { &p });
            return out.as_str().to_string();
        }

        let s = out.as_str().to_string();
        // 只去掉纯根路径以外的尾斜杠，保持可访问
        if s.ends_with('/') && out.path() != "/" {
            s.trim_end_matches('/').to_string()
        } else {
            s
        }
    }

    // ───────────────────────── 词面打分 ─────────────────────────

    /// 把 query 拆成带权重的匹配单元。中文没有分词器，用「整实体 + 相邻二元组」
    /// 逼近，既能匹配「何祖军」整体，也能容忍标题里写成「何祖军教授」。
    fn grams(plan: &QueryPlan) -> Vec<(String, f64)> {
        let mut g: Vec<(String, f64)> = Vec::new();
        for e in &plan.entities {
            let n = e.chars().count();
            if n >= 2 {
                g.push((e.to_lowercase(), 3.0 + n as f64 * 0.2));
            }
            if e.chars().all(is_cjk) && n >= 2 {
                let cs: Vec<char> = e.chars().collect();
                for w in cs.windows(2) {
                    g.push((w.iter().collect::<String>(), 1.0));
                }
            }
        }
        for tok in plan.primary.split_whitespace() {
            let t = tok.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
            if t.len() >= 2 && !t.chars().any(is_cjk) {
                g.push((t, 2.0));
            }
        }
        g.sort_by(|a, b| a.0.cmp(&b.0));
        g.dedup_by(|a, b| a.0 == b.0);
        g
    }

    /// 在候选集内部算文档频率，把「大学」「学院」这种到处都是的片段降权。
    /// 无需外部语料，成本几乎为零，但对中文长查询的区分度提升明显。
    pub fn idf_weights(grams: &[(String, f64)], corpus: &[String]) -> Vec<(String, f64)> {
        if corpus.len() < 3 {
            return grams.to_vec();
        }
        let n = corpus.len() as f64;
        grams
            .iter()
            .map(|(g, w)| {
                let df = corpus.iter().filter(|d| d.to_lowercase().contains(g.as_str())).count() as f64;
                let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln().max(0.15);
                (g.clone(), w * idf)
            })
            .collect()
    }

    fn coverage(grams: &[(String, f64)], text: &str) -> f64 {
        if grams.is_empty() {
            return 0.0;
        }
        let t = text.to_lowercase();
        let total: f64 = grams.iter().map(|(_, w)| *w).sum();
        let hit: f64 = grams.iter().filter(|(g, _)| t.contains(g.as_str())).map(|(_, w)| *w).sum();
        hit / total
    }

    fn lex_score(g: &[(String, f64)], r: &Ranked) -> f64 {
        0.55 * coverage(g, &r.title) + 0.25 * coverage(g, &r.snippet) + 0.20 * coverage(g, &r.host)
    }

    // ───────────────────────── 噪音与先验 ─────────────────────────

    /// 用户是不是真的在查字/词本身（那样字典站就是正确答案，不能滤）。
    fn looks_like_dict_query(plan: &QueryPlan) -> bool {
        const MARKS: &[&str] =
            &["什么意思", "怎么读", "拼音", "读音", "笔画", "部首", "组词", "释义", "怎么写"];
        MARKS.iter().any(|m| plan.original.contains(m))
            || plan.primary.chars().filter(|c| is_cjk(*c)).count() <= 1
    }

    pub fn is_noise(plan: &QueryPlan, title: &str, host: &str, snippet: &str) -> bool {
        if looks_like_dict_query(plan) {
            return false;
        }

        if crate::core::tools::web_search::domains::registry()
            .dict_hosts
            .iter()
            .any(|h| host == h.as_str() || host.ends_with(&format!(".{h}")))
        {
            return true;
        }

        // 拆字陷阱：标题一眼是字典条目，且 query 里 ≥2 字的实体一个都没出现。
        let dictish = DICT_TITLE_MARKS.iter().any(|m| title.contains(m));
        if dictish {
            let hay = format!("{title} {snippet}");
            let any_entity_present = plan
                .entities
                .iter()
                .filter(|e| e.chars().count() >= 2)
                .any(|e| hay.contains(e.as_str()));
            if !any_entity_present {
                return true;
            }
        }
        false
    }

    fn domain_prior(plan: &QueryPlan, host: &str) -> f64 {
        let mut p = 0.0;
        for (suf, w) in AUTH_SUFFIX {
            if host.ends_with(suf) {
                p += w;
                break;
            }
        }
        let reg = crate::core::tools::web_search::domains::registry();
        if reg.low_quality_hosts.iter().any(|h| host.ends_with(h.as_str())) {
            p -= 0.45;
        }
        // 人物查询才启用实体源加成，避免污染普通检索
        if plan.looks_like_person() {
            for (h, w) in &reg.entity_sources {
                if host.ends_with(h.as_str()) {
                    p += *w;
                    break;
                }
            }
        }
        // 用户点名了域名 → 该域名的结果直接上浮
        if let Some(site) = &plan.site_hint {
            if host.ends_with(site.as_str()) {
                p += 0.8;
            }
        }
        p
    }

    // ───────────────────────── 融合主流程 ─────────────────────────

    pub struct EngineHits<'a> {
        pub engine: &'static str,
        pub weight: f64,
        pub results: &'a [RawResult],
    }

    pub fn fuse(hits: &[EngineHits], plan: &QueryPlan, want: usize) -> Vec<Ranked> {
        let mut agg: HashMap<String, Ranked> = HashMap::new();

        for h in hits {
            for (rank, r) in h.results.iter().enumerate() {
                let Some(norm) = normalize(&r.url) else { continue };
                let bump = h.weight / (RRF_K + rank as f64 + 1.0);

                let e = agg.entry(norm.key.clone()).or_insert_with(|| Ranked {
                    title: r.title.clone(),
                    url: norm.display.clone(),
                    snippet: r.snippet.clone(),
                    content: None,
                    host: norm.host.clone(),
                    engines: Vec::new(),
                    needs_resolve: norm.needs_resolve,
                    date: None,
                    evidence: Vec::new(),
                    ce: None,
                    rrf: 0.0,
                    lex: 0.0,
                    prior: 0.0,
                    score: 0.0,
                });
                e.rrf += bump;
                if !e.engines.contains(&h.engine) {
                    e.engines.push(h.engine);
                }
                // 保留信息量更大的标题/摘要
                if r.title.chars().count() > e.title.chars().count() && r.title != "(无标题)" {
                    e.title = r.title.clone();
                }
                if r.snippet.chars().count() > e.snippet.chars().count() {
                    e.snippet = r.snippet.clone();
                }
                // 已经拿到直链就别再用跳板链接
                if !norm.needs_resolve && e.needs_resolve {
                    e.url = norm.display.clone();
                    e.needs_resolve = false;
                }
            }
        }

        let g0 = grams(plan);
        let mut out: Vec<Ranked> = agg
            .into_values()
            .filter(|r| !is_noise(plan, &r.title, &r.host, &r.snippet))
            .collect();

        // 用候选集自身做 IDF 语料
        let corpus: Vec<String> =
            out.iter().map(|r| format!("{} {} {}", r.title, r.snippet, r.host)).collect();
        let g = idf_weights(&g0, &corpus);

        let max_rrf = out.iter().map(|r| r.rrf).fold(f64::MIN_POSITIVE, f64::max);
        for r in out.iter_mut() {
            r.date = crate::core::tools::web_search::dates::extract(&r.title)
                .or_else(|| crate::core::tools::web_search::dates::extract(&r.snippet));
            r.lex = lex_score(&g, r);
            r.prior = domain_prior(plan, &r.host);
            // 多引擎共识是很强的质量信号
            let agreement = ((r.engines.len().saturating_sub(1)) as f64 * 0.15).min(0.3);
            r.score = r.rrf / max_rrf + 0.9 * r.lex + 0.5 * r.prior + agreement;
        }

        out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(want);
        out
    }

    /// 二段钻取的目标域名：优先权威后缀，其次出现频次。
    pub fn drill_host(ranked: &[Ranked], plan: &QueryPlan) -> Option<String> {
        if plan.site_hint.is_some() {
            return None; // 用户已经指定过了
        }
        let mut best: Option<(f64, String)> = None;
        for (i, r) in ranked.iter().take(8).enumerate() {
            let auth = AUTH_SUFFIX
                .iter()
                .find(|(s, _)| r.host.ends_with(s))
                .map(|(_, w)| *w)
                .unwrap_or(0.0);
            if auth <= 0.0 {
                continue;
            }
            let s = auth + 1.0 / (i as f64 + 1.0);
            if best.as_ref().map(|(bs, _)| s > *bs).unwrap_or(true) {
                best = Some((s, r.host.clone()));
            }
        }
        best.map(|(_, h)| h)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::core::tools::web_search::query;

        fn raw(t: &str, u: &str, s: &str) -> RawResult {
            RawResult { title: t.into(), url: u.into(), snippet: s.into() }
        }

        #[test]
        fn 去重忽略www与追踪参数与尾斜杠() {
            let a = normalize("https://www.Example.com/a/?utm_source=x&id=7#frag").unwrap();
            let b = normalize("http://example.com/a?id=7").unwrap();
            assert_eq!(a.key, b.key, "a={} b={}", a.key, b.key);
        }

        #[test]
        fn display保留必要参数且去掉追踪参数() {
            let n = normalize("https://e.com/p?id=7&utm_campaign=ads").unwrap();
            assert!(n.display.contains("id=7"));
            assert!(!n.display.contains("utm_campaign"));
        }

        #[test]
        fn ddg跳板链接会被解开() {
            let n = normalize("https://duckduckgo.com/l/?uddg=https%3A%2F%2Fjust.edu.cn%2Fa.htm&rut=x")
                .unwrap();
            assert_eq!(n.host, "just.edu.cn");
            assert!(!n.needs_resolve);
        }

        #[test]
        fn 百度跳板链接被标记为待解析() {
            let n = normalize("https://www.baidu.com/link?url=ABCDEF").unwrap();
            assert!(n.needs_resolve, "百度 link?url= 必须标记二次解析");
        }

        #[test]
        fn 非http协议被丢弃() {
            assert!(normalize("ftp://x.com/a").is_none());
            assert!(normalize("javascript:void(0)").is_none());
        }

        #[test]
        fn rrf让多引擎共识的结果上浮() {
            let p = query::plan("何祖军 江苏科技大学");
            let a = vec![raw("无关页", "https://x.com/1", ""), raw("何祖军 - 江苏科技大学", "https://just.edu.cn/p", "")];
            let b = vec![raw("何祖军 - 江苏科技大学", "https://just.edu.cn/p", "")];
            let hits = vec![
                EngineHits { engine: "bing", weight: 0.9, results: &a },
                EngineHits { engine: "baidu", weight: 0.8, results: &b },
            ];
            let out = fuse(&hits, &p, 10);
            assert_eq!(out[0].host, "just.edu.cn", "{out:#?}");
            assert_eq!(out[0].engines.len(), 2);
        }

        #[test]
        fn 字典拆字条目被过滤() {
            let p = query::plan("何祖军 江苏科技大学");
            assert!(is_noise(&p, "何 (汉语文字)_百度汉语", "hanyu.baidu.com", "何，汉语一级字"));
            assert!(is_noise(&p, "祖的意思、组词、拼音", "zdic.net", "汉语字典"));
        }

        #[test]
        fn 真查字词时字典站不算噪音() {
            let p = query::plan("何 是什么意思");
            assert!(!is_noise(&p, "何 (汉语文字)_百度汉语", "hanyu.baidu.com", ""));
        }

        #[test]
        fn 正常结果不会被误判噪音() {
            let p = query::plan("何祖军 江苏科技大学");
            assert!(!is_noise(&p, "何祖军 - 江苏科技大学计算机学院", "just.edu.cn", "教师主页"));
        }

        #[test]
        fn 教育域名有先验加成() {
            let p = query::plan("何祖军 江苏科技大学");
            assert!(domain_prior(&p, "just.edu.cn") > domain_prior(&p, "blog.example.com"));
            assert!(domain_prior(&p, "wenku.baidu.com") < 0.0);
        }

        #[test]
        fn 词面打分认得整实体也认得二元组() {
            let p = query::plan("何祖军 江苏科技大学");
            let g = grams(&p);
            assert!(coverage(&g, "何祖军教授 江苏科技大学计算机学院") > 0.8);
            assert!(coverage(&g, "完全无关的一段文本") < 0.1);
        }

        #[test]
        fn 钻取选权威域名() {
            let p = query::plan("何祖军 江苏科技大学");
            let a = vec![
                raw("论坛帖子", "https://bbs.example.com/1", ""),
                raw("师资队伍", "https://just.edu.cn/list.htm", ""),
            ];
            let hits = vec![EngineHits { engine: "bing", weight: 0.9, results: &a }];
            let out = fuse(&hits, &p, 10);
            assert_eq!(drill_host(&out, &p).as_deref(), Some("just.edu.cn"));
        }

        #[test]
        fn 用户已指定site时不再钻取() {
            let p = query::plan("何祖军 just.edu.cn");
            let a = vec![raw("师资队伍", "https://just.edu.cn/list.htm", "")];
            let hits = vec![EngineHits { engine: "bing", weight: 0.9, results: &a }];
            let out = fuse(&hits, &p, 10);
            assert!(drill_host(&out, &p).is_none());
        }
    }

}

pub mod engine {
    //! L1：引擎池。
    //!
    //! 分两档：
    //!   * Tier A（结构化 API）：SearXNG 自建实例、Brave API。稳定、不会因页面
    //!     改版失效，优先用。
    //!   * Tier B（HTML 抓取）：Bing / DuckDuckGo / 百度。免费但脆，只做补充和兜底。
    //!
    //! 强烈建议至少配一个 Tier A —— 手工维护 SERP 解析器是无底洞。
    //! 自建 SearXNG 一个 JSON 接口就能打 70+ 引擎，`SEARXNG_URL` 一配即用。

    use crate::core::tools::web_search::http;
    use crate::core::tools::web_search::serp::{self, ParseOutcome, RawResult, SerpSpec};
    use std::time::Instant;
    use url::Url;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum Engine {
        SearxNG,
        Brave,
        Bing,
        DuckDuckGo,
        Baidu,
        Sogou,
        /// 搜狗微信：公众号内容，中文机构通知的主要出口
        Weixin,
        So360,
    }

    pub const ALL: &[Engine] = &[
        Engine::SearxNG,
        Engine::Brave,
        Engine::Bing,
        Engine::DuckDuckGo,
        Engine::Baidu,
        Engine::Sogou,
        Engine::Weixin,
        Engine::So360,
    ];

    impl Engine {
        pub fn name(self) -> &'static str {
            match self {
                Engine::SearxNG => "searxng",
                Engine::Brave => "brave",
                Engine::Bing => "bing",
                Engine::DuckDuckGo => "duckduckgo",
                Engine::Baidu => "baidu",
                Engine::Sogou => "sogou",
                Engine::Weixin => "weixin",
                Engine::So360 => "so360",
            }
        }

        /// RRF 里的引擎权重。
        pub fn weight(self) -> f64 {
            match self {
                Engine::SearxNG => 1.00,
                Engine::Brave => 0.95,
                Engine::Bing => 0.90,
                Engine::DuckDuckGo => 0.85,
                Engine::Baidu => 0.80,
                Engine::Sogou => 0.78,
                // 公众号覆盖独特，但正文常在 JS 里，权重略低
                Engine::Weixin => 0.72,
                Engine::So360 => 0.70,
            }
        }

        /// Tier A = 结构化接口，不会因为页面改版而失效。
        pub fn is_api(self) -> bool {
            matches!(self, Engine::SearxNG | Engine::Brave)
        }

        /// 只在含中文的查询里启用（百度对英文查询基本没有增量价值，
        /// 还会拖慢整体）。
        pub fn cjk_only(self) -> bool {
            matches!(self, Engine::Baidu | Engine::Sogou | Engine::Weixin | Engine::So360)
        }

        pub fn available(self) -> bool {
            match self {
                Engine::SearxNG => env_nonempty("SEARXNG_URL").is_some(),
                Engine::Brave => env_nonempty("BRAVE_SEARCH_API_KEY").is_some(),
                _ => true,
            }
        }

        pub fn parse(name: &str) -> Option<Engine> {
            ALL.iter().copied().find(|e| e.name() == name.to_ascii_lowercase())
        }
    }

    fn env_nonempty(k: &str) -> Option<String> {
        std::env::var(k).ok().filter(|v| !v.trim().is_empty())
    }

    #[derive(Debug, Clone)]
    pub enum EngineStatus {
        Ok,
        /// 请求成功、页面正常，但确实没有匹配结果
        Empty,
        /// 页面拿到了却一条都解析不出来 —— 解析器失效，需要修 SerpSpec
        ParserFailed { html_len: usize },
        /// 被反爬拦截 / 要求验证码
        Blocked,
        Http(u16),
        Transport(String),
        Timeout,
    }

    impl EngineStatus {
        /// 是否计入熔断器的失败次数。「确实没结果」不算引擎的错。
        pub fn is_failure(&self) -> bool {
            !matches!(self, EngineStatus::Ok | EngineStatus::Empty)
        }

        pub fn label(&self) -> String {
            match self {
                EngineStatus::Ok => "ok".into(),
                EngineStatus::Empty => "无结果".into(),
                EngineStatus::ParserFailed { html_len } => format!("解析失效(html {html_len}B)"),
                EngineStatus::Blocked => "被拦截".into(),
                EngineStatus::Http(c) => format!("HTTP {c}"),
                EngineStatus::Transport(e) => format!("网络错误({e})"),
                EngineStatus::Timeout => "超时".into(),
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct EngineOutcome {
        pub engine: Engine,
        pub query: String,
        pub status: EngineStatus,
        pub results: Vec<RawResult>,
        pub elapsed_ms: u64,
    }

    pub async fn run(engine: Engine, query: &str, n: usize, fresh: crate::core::tools::web_search::query::Freshness) -> EngineOutcome {
        let t0 = Instant::now();
        let (status, results) = match engine {
            Engine::SearxNG => searxng(query, n, fresh).await,
            Engine::Brave => brave(query, n, fresh).await,
            Engine::Bing => html_engine(bing_url(query, n, fresh), &serp::BING, n, "zh-CN,zh;q=0.9,en;q=0.8").await,
            Engine::DuckDuckGo => {
                html_engine(ddg_url(query, fresh), &serp::DUCKDUCKGO, n, "en-US,en;q=0.9,zh-CN;q=0.8").await
            }
            Engine::Baidu => html_engine(baidu_url(query, n, fresh), &serp::BAIDU, n, "zh-CN,zh;q=0.9").await,
            Engine::Sogou => html_engine(sogou_url(query), &serp::SOGOU, n, "zh-CN,zh;q=0.9").await,
            Engine::Weixin => html_engine(weixin_url(query), &serp::WEIXIN, n, "zh-CN,zh;q=0.9").await,
            Engine::So360 => html_engine(so360_url(query, n), &serp::SO360, n, "zh-CN,zh;q=0.9").await,
        };
        let elapsed_ms = t0.elapsed().as_millis() as u64;

        if status.is_failure() {
            crate::core::tools::web_search::health::record_fail(engine.name());
        } else {
            crate::core::tools::web_search::health::record_ok(engine.name(), elapsed_ms);
        }

        EngineOutcome { engine, query: query.to_string(), status, results, elapsed_ms }
    }

    // ───────────────────────── URL 构造 ─────────────────────────
    // 全部走 url crate 的 query_pairs_mut，不再手写百分号编码。
    // 旧的 urlencoding_simple 把空格编成 '+'，在 path 里是错的，且不处理
    // 已编码字符串的双重编码。

    /// 各引擎的时间过滤参数长得都不一样，统一在这里落地。
    /// 这比往 query 里拼事件词干净得多：不预设任何结论，只是把窗口收窄。
    fn bing_url(q: &str, n: usize, fresh: crate::core::tools::web_search::query::Freshness) -> Url {
        let mut u = Url::parse("https://www.bing.com/search").expect("const url");
        {
            let mut qp = u.query_pairs_mut();
            qp.append_pair("q", q)
                .append_pair("count", &n.max(10).to_string())
                .append_pair("form", "QBLH");
            // Bing HTML 端的时间过滤：ez1=24 小时，ez2=近一周，ez3=近一月
            let ez = match fresh {
                crate::core::tools::web_search::query::Freshness::Day => Some("ez1"),
                crate::core::tools::web_search::query::Freshness::Week => Some("ez2"),
                crate::core::tools::web_search::query::Freshness::Month | crate::core::tools::web_search::query::Freshness::Year => Some("ez3"),
                crate::core::tools::web_search::query::Freshness::Any => None,
            };
            if let Some(ez) = ez {
                qp.append_pair("filters", &format!("ex1:\"{ez}\""));
            }
        }
        u
    }

    fn baidu_url(q: &str, n: usize, fresh: crate::core::tools::web_search::query::Freshness) -> Url {
        let mut u = Url::parse("https://www.baidu.com/s").expect("const url");
        {
            let mut qp = u.query_pairs_mut();
            qp.append_pair("wd", q)
                .append_pair("rn", &n.max(10).min(50).to_string())
                .append_pair("ie", "utf-8");
            // 百度用 gpc=stf=<起>,<止>|stftype=1 表示时间区间（秒级时间戳）
            if let Some(days) = fresh.days() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                let from = now - days * 86_400;
                qp.append_pair("gpc", &format!("stf={from},{now}|stftype=1"));
            }
        }
        u
    }

    fn sogou_url(q: &str) -> Url {
        let mut u = Url::parse("https://www.sogou.com/web").expect("const url");
        u.query_pairs_mut().append_pair("query", q);
        u
    }

    fn weixin_url(q: &str) -> Url {
        let mut u = Url::parse("https://weixin.sogou.com/weixin").expect("const url");
        // type=2 是搜文章（type=1 搜公众号）
        u.query_pairs_mut().append_pair("type", "2").append_pair("query", q);
        u
    }

    fn so360_url(q: &str, n: usize) -> Url {
        let mut u = Url::parse("https://www.so.com/s").expect("const url");
        u.query_pairs_mut().append_pair("q", q).append_pair("rn", &n.max(10).min(50).to_string());
        u
    }

    fn ddg_url(q: &str, fresh: crate::core::tools::web_search::query::Freshness) -> Url {
        let mut u = Url::parse("https://html.duckduckgo.com/html/").expect("const url");
        {
            let mut qp = u.query_pairs_mut();
            qp.append_pair("q", q);
            let df = match fresh {
                crate::core::tools::web_search::query::Freshness::Day => Some("d"),
                crate::core::tools::web_search::query::Freshness::Week => Some("w"),
                crate::core::tools::web_search::query::Freshness::Month => Some("m"),
                crate::core::tools::web_search::query::Freshness::Year => Some("y"),
                crate::core::tools::web_search::query::Freshness::Any => None,
            };
            if let Some(df) = df {
                qp.append_pair("df", df);
            }
        }
        u
    }

    // ───────────────────────── Tier B：HTML 抓取 ─────────────────────────

    async fn html_engine(
        url: Url,
        spec: &SerpSpec,
        n: usize,
        accept_lang: &str,
    ) -> (EngineStatus, Vec<RawResult>) {
        let resp = match http::serp_client()
            .get(url.clone())
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Accept-Language", accept_lang)
            .header("Sec-Fetch-Dest", "document")
            .header("Sec-Fetch-Mode", "navigate")
            .header("Upgrade-Insecure-Requests", "1")
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) if e.is_timeout() => return (EngineStatus::Timeout, Vec::new()),
            Err(e) => return (EngineStatus::Transport(short_err(&e)), Vec::new()),
        };

        let code = resp.status().as_u16();
        if !resp.status().is_success() {
            return (EngineStatus::Http(code), Vec::new());
        }

        let ctype = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let bytes = match resp.bytes().await {
            Ok(b) => b,
            Err(e) => return (EngineStatus::Transport(short_err(&e)), Vec::new()),
        };
        // SERP 也要正确解码：百度在某些出口仍会回 GBK。
        let html = crate::core::tools::web_search::extract::decode_body(&bytes, ctype.as_deref());

        if let Some(reason) = blocked_reason(&html) {
            tracing::warn!(engine = spec.engine, reason, "SERP 被反爬拦截");
            return (EngineStatus::Blocked, Vec::new());
        }

        match serp::parse(&html, spec, n) {
            ParseOutcome::Ok(r) => (EngineStatus::Ok, r),
            ParseOutcome::ParserFailed { html_len, .. } => {
                (EngineStatus::ParserFailed { html_len }, Vec::new())
            }
            ParseOutcome::Empty => (EngineStatus::Empty, Vec::new()),
        }
    }

    /// 反爬拦截识别。这类页面 HTML 也是几十 KB，不识别出来就会被
    /// 当成「解析器失效」，永远查不出真实原因。
    pub fn blocked_reason(html: &str) -> Option<&'static str> {
        const MARKS: &[(&str, &str)] = &[
            ("百度安全验证", "baidu-verify"),
            ("wappass.baidu.com", "baidu-verify"),
            ("网络不给力，请稍后重试", "baidu-throttle"),
            ("请输入验证码", "captcha"),
            ("/sorry/index", "google-sorry"),
            ("anomaly.js", "ddg-anomaly"),
            ("unfortunately, bots use duckduckgo too", "ddg-anomaly"),
            ("Verifying you are human", "cf-challenge"),
            ("cf-browser-verification", "cf-challenge"),
            ("captcha-container", "captcha"),
            ("请输入验证码以便正常访问", "sogou-verify"),
            ("antispider", "sogou-verify"),
        ];
        let lower = html.to_lowercase();
        MARKS
            .iter()
            .find(|(needle, _)| lower.contains(&needle.to_lowercase()))
            .map(|(_, tag)| *tag)
    }

    fn short_err(e: &reqwest::Error) -> String {
        let s = e.to_string();
        s.chars().take(80).collect()
    }

    // ───────────────────────── Tier A：结构化接口 ─────────────────────────

    async fn searxng(q: &str, n: usize, fresh: crate::core::tools::web_search::query::Freshness) -> (EngineStatus, Vec<RawResult>) {
        let Some(base) = env_nonempty("SEARXNG_URL") else {
            return (EngineStatus::Transport("SEARXNG_URL 未配置".into()), Vec::new());
        };
        let mut u = match Url::parse(base.trim_end_matches('/')).and_then(|b| b.join("search")) {
            Ok(u) => u,
            Err(e) => return (EngineStatus::Transport(e.to_string()), Vec::new()),
        };
        {
            let mut qp = u.query_pairs_mut();
            qp.append_pair("q", q).append_pair("format", "json").append_pair("safesearch", "0");
            let tr = match fresh {
                crate::core::tools::web_search::query::Freshness::Day => Some("day"),
                crate::core::tools::web_search::query::Freshness::Week => Some("week"),
                crate::core::tools::web_search::query::Freshness::Month => Some("month"),
                crate::core::tools::web_search::query::Freshness::Year => Some("year"),
                crate::core::tools::web_search::query::Freshness::Any => None,
            };
            if let Some(tr) = tr {
                qp.append_pair("time_range", tr);
            }
        }

        let mut req = http::serp_client().get(u);
        // Basic Auth：SearXNG 实例加了密码防护时，用环境变量注入用户名/密码（不写死在代码里）。
        // 未配置用户名/密码则不带鉴权（兼容裸奔的 SearXNG 实例）。
        if let (Some(user), Some(pass)) = (
            env_nonempty("SEARXNG_USERNAME"),
            env_nonempty("SEARXNG_PASSWORD"),
        ) {
            req = req.basic_auth(user, Some(pass));
        }
        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) if e.is_timeout() => return (EngineStatus::Timeout, Vec::new()),
            Err(e) => return (EngineStatus::Transport(short_err(&e)), Vec::new()),
        };
        if !resp.status().is_success() {
            return (EngineStatus::Http(resp.status().as_u16()), Vec::new());
        }
        let v: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => return (EngineStatus::Transport(short_err(&e)), Vec::new()),
        };
        let out = json_results(&v, "results", "url", "title", "content", n);
        if out.is_empty() {
            (EngineStatus::Empty, out)
        } else {
            (EngineStatus::Ok, out)
        }
    }

    async fn brave(q: &str, n: usize, fresh: crate::core::tools::web_search::query::Freshness) -> (EngineStatus, Vec<RawResult>) {
        let Some(key) = env_nonempty("BRAVE_SEARCH_API_KEY") else {
            return (EngineStatus::Transport("BRAVE_SEARCH_API_KEY 未配置".into()), Vec::new());
        };
        let mut u = Url::parse("https://api.search.brave.com/res/v1/web/search").expect("const url");
        {
            let mut qp = u.query_pairs_mut();
            qp.append_pair("q", q).append_pair("count", &n.min(20).to_string());
            let f = match fresh {
                crate::core::tools::web_search::query::Freshness::Day => Some("pd"),
                crate::core::tools::web_search::query::Freshness::Week => Some("pw"),
                crate::core::tools::web_search::query::Freshness::Month => Some("pm"),
                crate::core::tools::web_search::query::Freshness::Year => Some("py"),
                crate::core::tools::web_search::query::Freshness::Any => None,
            };
            if let Some(f) = f {
                qp.append_pair("freshness", f);
            }
        }

        let resp = match http::serp_client()
            .get(u)
            .header("Accept", "application/json")
            .header("X-Subscription-Token", key)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) if e.is_timeout() => return (EngineStatus::Timeout, Vec::new()),
            Err(e) => return (EngineStatus::Transport(short_err(&e)), Vec::new()),
        };
        if !resp.status().is_success() {
            return (EngineStatus::Http(resp.status().as_u16()), Vec::new());
        }
        let v: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => return (EngineStatus::Transport(short_err(&e)), Vec::new()),
        };
        let arr = v.get("web").cloned().unwrap_or(serde_json::Value::Null);
        let out = json_results(&arr, "results", "url", "title", "description", n);
        if out.is_empty() {
            (EngineStatus::Empty, out)
        } else {
            (EngineStatus::Ok, out)
        }
    }

    fn json_results(
        v: &serde_json::Value,
        arr_key: &str,
        url_key: &str,
        title_key: &str,
        snip_key: &str,
        n: usize,
    ) -> Vec<RawResult> {
        v.get(arr_key)
            .and_then(|a| a.as_array())
            .map(|items| {
                items
                    .iter()
                    .take(n)
                    .filter_map(|it| {
                        let url = it.get(url_key)?.as_str()?.to_string();
                        if url.is_empty() {
                            return None;
                        }
                        Some(RawResult {
                            title: it
                                .get(title_key)
                                .and_then(|x| x.as_str())
                                .unwrap_or("(无标题)")
                                .to_string(),
                            url,
                            snippet: it
                                .get(snip_key)
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 按 `engines` 参数 + 可用性 + 熔断状态 + 中文路由挑引擎，
    /// 并按历史延迟从快到慢排序。
    pub fn select(pref: &str, has_cjk: bool) -> Vec<Engine> {
        let pref = pref.trim().to_ascii_lowercase();
        let mut chosen: Vec<Engine> = if pref == "all" || pref.is_empty() {
            ALL.to_vec()
        } else {
            pref.split(|c| c == ',' || c == '+')
                .filter_map(|s| Engine::parse(s.trim()))
                .collect()
        };
        if chosen.is_empty() {
            chosen = ALL.to_vec();
        }

        let explicit = !(pref == "all" || pref.is_empty());

        // 先按可用性和语言路由过滤（这一层与熔断无关）
        chosen.retain(|e| {
            if !e.available() {
                return false;
            }
            if !explicit && e.cjk_only() && !has_cjk {
                return false;
            }
            true
        });

        // 中文查询里 DuckDuckGo 覆盖弱、且从境内出口经常不可达，
        // 中文引擎够用时就不带它拖慢整轮。
        if !explicit && has_cjk {
            let cn = chosen.iter().filter(|e| e.cjk_only()).count();
            if cn >= 3 {
                chosen.retain(|e| *e != Engine::DuckDuckGo);
            }
        }

        if !explicit {
            let (mut live, mut tripped): (Vec<Engine>, Vec<Engine>) = chosen
                .iter()
                .copied()
                .partition(|e| !crate::core::tools::web_search::health::is_open(e.name()));

            // ── 半开探测 ──
            // 只有 open/closed 两态时，引擎会陆续掉线、池子单调收缩，
            // 最后剩的那一个偶发抖动就等于全线崩盘（线上就是这么炸的）。
            // 这里保证池子不低于 MIN_POOL：按冷却剩余时间从短到长放回来试探。
            const MIN_POOL: usize = 2;
            if live.len() < MIN_POOL && !tripped.is_empty() {
                tripped.sort_by_key(|e| crate::core::tools::web_search::health::cooldown_remaining(e.name()));
                for e in tripped {
                    if live.len() >= MIN_POOL {
                        break;
                    }
                    tracing::debug!(engine = e.name(), "池子不足，半开探测放行");
                    live.push(e);
                }
            }
            chosen = live;
        }

        // 熔断器的本意是防止个别引擎拖累整体，但在持续被拦的场景下会把
        // 引擎池一路抽干，最后只剩一个通道，它一超时就报「全部失败」——
        // 故障被自己放大了。这里兜底：可用通道少于 2 个时，把熔断中的
        // 引擎按「半开」放回去（照常发请求，权重打折），宁可慢也不能没有。
        if chosen.len() < 2 && !explicit {
            let mut halfopen: Vec<Engine> = ALL
                .iter()
                .copied()
                .filter(|e| {
                    e.available()
                        && !chosen.contains(e)
                        && (!e.cjk_only() || has_cjk)
                })
                .collect();
            halfopen.sort_by(|a, b| {
                crate::core::tools::web_search::health::latency_ms(a.name())
                    .partial_cmp(&crate::core::tools::web_search::health::latency_ms(b.name()))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for e in halfopen.into_iter().take(3) {
                tracing::debug!(engine = e.name(), "可用通道不足，半开放回");
                chosen.push(e);
            }
        }

        chosen.sort_by(|a, b| {
            // API 通道优先，然后按历史延迟
            b.is_api()
                .cmp(&a.is_api())
                .then_with(|| {
                    crate::core::tools::web_search::health::latency_ms(a.name())
                        .partial_cmp(&crate::core::tools::web_search::health::latency_ms(b.name()))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });
        chosen
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn url构造正确编码中文与空格() {
            let u = bing_url("何祖军 江苏科技大学", 10, crate::core::tools::web_search::query::Freshness::Any);
            let s = u.as_str();
            assert!(s.contains("q=%E4%BD%95%E7%A5%96%E5%86%9B"), "{s}");
            assert!(!s.contains(' '));
            assert!(s.contains("count=10"));
        }

        #[test]
        fn 百度url带rn与utf8() {
            let s = baidu_url("测试", 8, crate::core::tools::web_search::query::Freshness::Any).to_string();
            assert!(s.contains("wd=%E6%B5%8B%E8%AF%95"), "{s}");
            assert!(s.contains("rn=10"));
            assert!(s.contains("ie=utf-8"));
        }

        #[test]
        fn site限定不会被破坏() {
            let s = bing_url("site:just.edu.cn 何祖军", 10, crate::core::tools::web_search::query::Freshness::Any).to_string();
            assert!(s.contains("site%3Ajust.edu.cn"), "{s}");
        }

        #[test]
        fn 时效窗口落到各引擎原生参数() {
            use crate::core::tools::web_search::query::Freshness as F;
            assert!(bing_url("q", 10, F::Week).to_string().contains("filters="));
            assert!(ddg_url("q", F::Year).to_string().contains("df=y"));
            assert!(baidu_url("q", 10, F::Month).to_string().contains("gpc="));
            // Any 时不加任何时间参数
            assert!(!bing_url("q", 10, F::Any).to_string().contains("filters="));
            assert!(!ddg_url("q", F::Any).to_string().contains("df="));
            assert!(!baidu_url("q", 10, F::Any).to_string().contains("gpc="));
        }

        #[test]
        fn 识别百度安全验证页() {
            assert_eq!(blocked_reason("<html>百度安全验证</html>"), Some("baidu-verify"));
            assert_eq!(blocked_reason("<html>正常页面</html>"), None);
        }

        #[test]
        fn 无结果不计入熔断失败() {
            assert!(!EngineStatus::Empty.is_failure());
            assert!(!EngineStatus::Ok.is_failure());
            assert!(EngineStatus::ParserFailed { html_len: 1 }.is_failure());
            assert!(EngineStatus::Blocked.is_failure());
        }

        #[test]
        fn 英文查询不启用百度() {
            crate::core::tools::web_search::health::reset_for_test();
            let e = select("all", false);
            assert!(!e.contains(&Engine::Baidu));
            let e = select("all", true);
            assert!(e.contains(&Engine::Baidu));
        }

        #[test]
        fn 显式点名单个引擎() {
            crate::core::tools::web_search::health::reset_for_test();
            assert_eq!(select("baidu", false), vec![Engine::Baidu]);
            let e = select("bing,duckduckgo", false);
            assert_eq!(e.len(), 2);
        }

        #[test]
        fn 熔断的引擎在通道充足时被跳过() {
            crate::core::tools::web_search::health::reset_for_test();
            for _ in 0..3 {
                crate::core::tools::web_search::health::record_fail("bing");
            }
            // 中文场景下可用通道多，摘掉 bing 不影响覆盖
            let cn = select("all", true);
            assert!(!cn.contains(&Engine::Bing));
            assert!(cn.len() >= 2);
            // 显式点名仍然放行，便于定向排障
            assert!(select("bing", true).contains(&Engine::Bing));
            crate::core::tools::web_search::health::reset_for_test();
        }

        #[test]
        fn 通道不足时熔断让位于可用性() {
            crate::core::tools::web_search::health::reset_for_test();
            // 英文场景本来就只有 bing / duckduckgo 两条通道
            for _ in 0..5 {
                crate::core::tools::web_search::health::record_fail("bing");
            }
            let en = select("all", false);
            assert!(
                en.contains(&Engine::Bing),
                "只剩一条通道时必须半开放回，否则它一超时就报『全部失败』"
            );
            crate::core::tools::web_search::health::reset_for_test();
        }

        #[test]
        fn 解析brave响应结构() {
            let v: serde_json::Value = serde_json::from_str(
                r#"{"results":[{"url":"https://a.com","title":"T","description":"D"}]}"#,
            )
            .unwrap();
            let r = json_results(&v, "results", "url", "title", "description", 5);
            assert_eq!(r.len(), 1);
            assert_eq!(r[0].snippet, "D");
        }
    }

}

pub mod exec {
    //! L2：并发执行器。
    //!
    //! 旧实现的问题不是「不并发」（它确实 spawn 了），而是：
    //!   * 两个 HTML 引擎的 `send()` 完全没有超时 —— 一个引擎卡住，整个工具卡住；
    //!   * 逐个 `handle.await`，没有全局 deadline，总耗时被最慢的那个引擎决定；
    //!   * 没有并发上限，子查询一多就变成对搜索引擎的小型压测。
    //!
    //! 这里：每任务超时 + 全局 deadline + 信号量限流 + 够数就提前收工
    //! （剩余 future 直接 drop 掉，连接由客户端池回收）。

    use crate::core::tools::web_search::engine::{self, Engine, EngineOutcome, EngineStatus};
    use futures::stream::{FuturesUnordered, StreamExt};
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::sync::Semaphore;

    #[derive(Debug, Clone)]
    pub struct Task {
        pub engine: Engine,
        pub query: String,
        pub n: usize,
        pub fresh: crate::core::tools::web_search::query::Freshness,
    }

    #[derive(Debug, Clone)]
    pub struct WaveCfg {
        /// 单个引擎的硬超时
        pub per_task: Duration,
        /// 整轮的硬 deadline：到点就用手上已有的结果作答
        pub global: Duration,
        pub max_parallel: usize,
        /// 至少等这么久再考虑提前收工，给慢一点的引擎一个机会
        pub min_wait: Duration,
        /// 收到这么多条不同 URL 就可以提前收工
        pub enough: usize,
    }

    impl Default for WaveCfg {
        fn default() -> Self {
            Self {
                per_task: Duration::from_millis(2500),
                global: Duration::from_millis(4000),
                max_parallel: 8,
                min_wait: Duration::from_millis(600),
                enough: 20,
            }
        }
    }

    pub async fn run_wave(tasks: Vec<Task>, cfg: &WaveCfg) -> Vec<EngineOutcome> {
        if tasks.is_empty() {
            return Vec::new();
        }
        let started = Instant::now();
        let deadline = started + cfg.global;
        let sem = Arc::new(Semaphore::new(cfg.max_parallel.max(1)));

        let mut futs = FuturesUnordered::new();
        for t in tasks {
            let sem = Arc::clone(&sem);
            let per_task = cfg.per_task;
            futs.push(async move {
                let _permit = sem.acquire_owned().await.ok();
                match tokio::time::timeout(per_task, engine::run(t.engine, &t.query, t.n, t.fresh)).await {
                    Ok(o) => o,
                    Err(_) => {
                        crate::core::tools::web_search::health::record_fail(t.engine.name());
                        tracing::warn!(engine = t.engine.name(), q = %t.query, "引擎超时");
                        EngineOutcome {
                            engine: t.engine,
                            query: t.query,
                            status: EngineStatus::Timeout,
                            results: Vec::new(),
                            elapsed_ms: per_task.as_millis() as u64,
                        }
                    }
                }
            });
        }

        let mut out: Vec<EngineOutcome> = Vec::new();
        let mut unique: HashSet<String> = HashSet::new();
        let mut ok_engines: HashSet<&'static str> = HashSet::new();

        loop {
            let now = Instant::now();
            if now >= deadline {
                tracing::debug!(collected = out.len(), "触达全局 deadline，用已有结果作答");
                break;
            }
            let remaining = deadline - now;
            match tokio::time::timeout(remaining, futs.next()).await {
                Err(_) => break,          // 全局 deadline
                Ok(None) => break,        // 全部完成
                Ok(Some(o)) => {
                    if matches!(o.status, EngineStatus::Ok) {
                        ok_engines.insert(o.engine.name());
                    }
                    for r in &o.results {
                        unique.insert(r.url.clone());
                    }
                    out.push(o);

                    let enough = unique.len() >= cfg.enough && ok_engines.len() >= 2;
                    if enough && started.elapsed() >= cfg.min_wait {
                        tracing::debug!(unique = unique.len(), "结果已足够，提前收工");
                        break;
                    }
                }
            }
        }
        // futs 在此 drop，未完成的请求随之取消
        out
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn rt() -> tokio::runtime::Runtime {
            tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap()
        }

        #[test]
        fn 空任务直接返回() {
            rt().block_on(async {
                assert!(run_wave(Vec::new(), &WaveCfg::default()).await.is_empty());
            });
        }

        #[test]
        fn 全局deadline会截断长尾() {
            // 用一个不可达地址造慢请求：全局 deadline 必须兜住
            rt().block_on(async {
                let cfg = WaveCfg {
                    per_task: Duration::from_secs(30),
                    global: Duration::from_millis(300),
                    max_parallel: 4,
                    min_wait: Duration::from_millis(10),
                    enough: 100,
                };
                let tasks = vec![Task {
                    engine: Engine::Bing,
                    query: "x".into(),
                    n: 5,
                    fresh: crate::core::tools::web_search::query::Freshness::Any,
                }];
                let t0 = Instant::now();
                let _ = run_wave(tasks, &cfg).await;
                assert!(
                    t0.elapsed() < Duration::from_millis(1500),
                    "应在 deadline 附近返回，实际 {:?}",
                    t0.elapsed()
                );
            });
        }

        #[test]
        fn 默认配置是合理的() {
            let c = WaveCfg::default();
            assert!(c.per_task <= c.global, "单任务超时不应超过全局 deadline");
            assert!(c.min_wait < c.global);
            assert!(c.max_parallel >= 1);
        }
    }

}

pub mod extract {
    //! L5：正文抓取与抽取。
    //!
    //! 这里修掉三个老 bug：
    //!   1. **UTF-8 panic**：旧实现按 char 下标遍历，却用同一个下标去切 byte
    //!      切片（`html_lower[i..i+7]`）。ASCII 页面下两者恰好相等，中文页面
    //!      立刻发散并 panic；panic 发生在 `tokio::spawn` 里，被
    //!      `if let Ok(..)` 静默吞掉 —— 于是中文网页的正文永远是空的。
    //!      改用 DOM 遍历，彻底不碰字节下标。
    //!   2. **编码**：旧实现用 `resp.text()`，只认响应头 charset。大量 .edu.cn
    //!      老站是 GB2312/GBK 且响应头不带 charset，抓回来全是乱码。
    //!      这里 BOM → 响应头 → meta charset → chardetng 四级识别。
    //!   3. **跳板页**：百度 `link?url=` 不是 302 而是 JS 跳板页，
    //!      reqwest 的自动重定向救不了，必须显式解析。

    use crate::core::tools::web_search::http;
    use crate::core::tools::web_search::serp::{squeeze, visible_text};
    use futures::StreamExt;
    use scraper::{ElementRef, Html, Selector};
    use std::time::Duration;

    /// 单页最多读这么多字节，防止被超大页面拖死。
    const MAX_PAGE_BYTES: usize = 2 * 1024 * 1024;
    /// 抽出的正文超过这个字符数就截断（控 token 成本）。
    pub const CONTENT_CHAR_CAP: usize = 3_000;
    /// 短于这个长度基本是跳板页/拦截页，不算有效正文。
    const MIN_USEFUL_CHARS: usize = 80;

    // ───────────────────────── 编码识别 ─────────────────────────

    pub fn decode_body(bytes: &[u8], content_type: Option<&str>) -> String {
        // 1) BOM 最可靠
        if let Some((enc, _)) = encoding_rs::Encoding::for_bom(bytes) {
            return enc.decode(bytes).0.into_owned();
        }
        // 2) 响应头 charset
        if let Some(label) = content_type.and_then(charset_from_content_type) {
            if let Some(enc) = encoding_rs::Encoding::for_label(label.as_bytes()) {
                return enc.decode(bytes).0.into_owned();
            }
        }
        // 3) <meta charset> —— 只在前 4KB 里按 ASCII 扫，不做任何多字节切片
        if let Some(label) = meta_charset(&bytes[..bytes.len().min(4096)]) {
            if let Some(enc) = encoding_rs::Encoding::for_label(label.as_bytes()) {
                return enc.decode(bytes).0.into_owned();
            }
        }
        // 4) 统计检测兜底
        let mut det = chardetng::EncodingDetector::new();
        det.feed(&bytes[..bytes.len().min(64 * 1024)], true);
        det.guess(None, true).decode(bytes).0.into_owned()
    }

    fn charset_from_content_type(ct: &str) -> Option<String> {
        let lower = ct.to_ascii_lowercase();
        let pos = lower.find("charset=")?;
        let rest = &lower[pos + "charset=".len()..];
        let v: String = rest
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != ';' && *c != '"' && *c != '\'')
            .collect();
        if v.is_empty() {
            None
        } else {
            Some(v)
        }
    }

    fn meta_charset(head: &[u8]) -> Option<String> {
        let ascii: String = head.iter().map(|b| *b as char).collect::<String>().to_ascii_lowercase();
        let pos = ascii.find("charset")?;
        let rest = &ascii[pos + "charset".len()..];
        let rest = rest.trim_start_matches([' ', '=', '"', '\'']);
        let v: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if v.is_empty() {
            None
        } else {
            Some(v)
        }
    }

    // ───────────────────────── 正文抽取 ─────────────────────────

    /// 语义容器优先，其次按「文本量 − 链接文本量」的密度挑最佳容器。
    /// 这样能把导航栏、页脚、侧边栏那种「短文本 + 高链接密度」的块排除掉。
    pub fn main_text(html: &str) -> String {
        // 注意：Html 不是 Send，本函数必须保持同步，不能出现 .await。
        let doc = Html::parse_document(html);

        const CANDIDATES: &[&str] = &[
            "article",
            "main",
            "[role=main]",
            "#content",
            "#main",
            "#main-content",
            ".article",
            ".article-content",
            ".post-content",
            ".entry-content",
            ".content",
            "div.wp_articlecontent",
            "body",
        ];

        let mut best: Option<(f64, String)> = None;
        for c in CANDIDATES {
            let Ok(sel) = Selector::parse(c) else { continue };
            for node in doc.select(&sel).take(4) {
                let text = squeeze(&visible_text(node));
                let n = text.chars().count();
                if n < 40 {
                    continue;
                }
                let link_chars = link_text_len(node);
                let score = n as f64 - 0.6 * link_chars as f64;
                if best.as_ref().map(|(bs, _)| score > *bs).unwrap_or(true) {
                    best = Some((score, text));
                }
            }
            // 语义标签一旦命中且够长就不再往下找了
            if let Some((_, t)) = &best {
                if *c == "article" || *c == "main" {
                    if t.chars().count() > 400 {
                        break;
                    }
                }
            }
        }

        let text = best.map(|(_, t)| t).unwrap_or_default();
        if text.is_empty() {
            // 极端兜底：整棵树的可见文本
            let sel = Selector::parse("html").ok();
            if let Some(sel) = sel {
                if let Some(root) = doc.select(&sel).next() {
                    return squeeze(&visible_text(root));
                }
            }
        }
        text
    }

    fn link_text_len(el: ElementRef) -> usize {
        let Ok(a) = Selector::parse("a") else { return 0 };
        el.select(&a).map(|n| visible_text(n).chars().count()).sum()
    }

    /// 截断到字符边界，并明确告知被截断（旧实现用 byte 切片，中文会 panic）。
    pub fn cap_chars(s: &str, cap: usize) -> String {
        if s.chars().count() <= cap {
            return s.to_string();
        }
        let head: String = s.chars().take(cap).collect();
        format!("{head}…〔正文过长已截断，需要完整内容请用 WebFetch 单独抓取〕")
    }

    // ───────────────────────── 网络部分 ─────────────────────────

    /// 还原引擎跳板链接（百度 link?url= / Bing ck/a?u=）。
    pub async fn resolve_redirect(url: &str) -> Option<String> {
        let resp = http::page_client()
            .get(url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .ok()?;
        let final_url = resp.url().clone();
        let host = final_url.host_str().unwrap_or("").to_string();
        if !host.ends_with("baidu.com") && !host.ends_with("bing.com") {
            return Some(final_url.to_string()); // 已经 302 到真实站点
        }
        // 还停在跳板域：从 JS/meta refresh 里把真实地址抠出来
        let bytes = resp.bytes().await.ok()?;
        let body = decode_body(&bytes, None);
        sniff_redirect_target(&body)
    }

    /// 从跳板页正文里找真实地址。纯字符串处理，独立可测。
    pub fn sniff_redirect_target(body: &str) -> Option<String> {
        const PATTERNS: &[&str] = &[
            "window.location.replace(\"",
            "window.location.href=\"",
            "window.location.replace('",
            "URL='",
            "URL=\"",
            "url=",
        ];
        for p in PATTERNS {
            let Some(i) = body.find(p) else { continue };
            let rest = &body[i + p.len()..];
            let end = rest.find(['"', '\'', '>', ' ', '\n']).unwrap_or(rest.len());
            let cand = rest[..end].trim();
            if cand.starts_with("http://") || cand.starts_with("https://") {
                return Some(cand.to_string());
            }
        }
        None
    }

    /// 抓一个页面并抽正文。返回 None 表示这一页没有可用内容。
    pub async fn fetch_page(url: &str) -> Option<String> {
        let resp = http::page_client().get(url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }

        let ctype = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        // 非 HTML（PDF/图片/压缩包）直接放弃，别喂给 LLM 一堆二进制噪音
        if let Some(ct) = &ctype {
            let l = ct.to_ascii_lowercase();
            if !(l.contains("html") || l.contains("xml") || l.starts_with("text/")) {
                return None;
            }
        }

        // 流式读取并封顶，避免被超大页面拖死
        let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let Ok(chunk) = chunk else { break };
            let room = MAX_PAGE_BYTES.saturating_sub(buf.len());
            if room == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..chunk.len().min(room)]);
        }
        if buf.is_empty() {
            return None;
        }

        let html = decode_body(&buf, ctype.as_deref());
        // 拦截页/跳板页会解出一小撮字，没有价值
        if crate::core::tools::web_search::engine::blocked_reason(&html).is_some() {
            return None;
        }
        let text = main_text(&html);
        if text.chars().count() < MIN_USEFUL_CHARS {
            return None;
        }
        Some(cap_chars(&text, CONTENT_CHAR_CAP))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn 优先用响应头charset() {
            let gbk = encoding_rs::GBK.encode("江苏科技大学").0.into_owned();
            let s = decode_body(&gbk, Some("text/html; charset=gb2312"));
            assert_eq!(s, "江苏科技大学");
        }

        #[test]
        fn 响应头没charset时用meta() {
            let head = b"<html><head><meta http-equiv=\"Content-Type\" content=\"text/html; charset=gbk\">";
            let mut bytes = head.to_vec();
            bytes.extend_from_slice(&encoding_rs::GBK.encode("何祖军 教授").0);
            let s = decode_body(&bytes, None);
            assert!(s.contains("何祖军 教授"), "实际: {s}");
        }

        #[test]
        fn 都没有时统计检测兜底不产生乱码() {
            let gbk = encoding_rs::GBK.encode("江苏科技大学计算机学院教师主页何祖军副教授").0.into_owned();
            let s = decode_body(&gbk, None);
            assert!(s.contains("江苏科技大学"), "实际: {s}");
        }

        #[test]
        fn utf8内容原样通过() {
            assert_eq!(decode_body("中文 abc".as_bytes(), Some("text/html; charset=utf-8")), "中文 abc");
        }

        #[test]
        fn 中文页面抽正文不会panic() {
            // 这就是旧实现 char/byte 下标混用会 panic 的输入形状
            let html = "<html><head><script>var 改='x';</script><style>.改{}</style></head>\
                        <body><nav><a href=/>首页</a><a href=/x>师资</a></nav>\
                        <article>何祖军，江苏科技大学计算机学院。主要研究方向为软件工程与数据挖掘，\
                        主持国家自然科学基金项目若干，发表论文数十篇，长期从事研究生培养工作。</article>\
                        <footer><a href=/1>版权</a></footer></body></html>";
            let t = main_text(html);
            assert!(t.contains("何祖军"), "实际: {t}");
            assert!(!t.contains("var 改"), "script 内容不应进入正文");
            assert!(!t.contains(".改{"), "style 内容不应进入正文");
        }

        #[test]
        fn 高链接密度的导航块不会被当成正文() {
            let html = "<html><body>\
              <div id=content><a href=/1>栏目一</a><a href=/2>栏目二</a><a href=/3>栏目三</a></div>\
              <article>这是真正的正文段落，包含足够多的自然语言文字用于密度判定，\
              而不是一堆导航链接堆在一起的模板区块内容。</article></body></html>";
            let t = main_text(html);
            assert!(t.contains("真正的正文"), "实际: {t}");
        }

        #[test]
        fn 截断按字符边界且有提示() {
            let s = "中".repeat(100);
            let c = cap_chars(&s, 10);
            assert!(c.starts_with(&"中".repeat(10)));
            assert!(c.contains("已截断"));
            assert_eq!(cap_chars("短", 10), "短");
        }

        #[test]
        fn 从跳板页抠出真实地址() {
            assert_eq!(
                sniff_redirect_target(r#"<script>window.location.replace("https://just.edu.cn/a.htm")</script>"#),
                Some("https://just.edu.cn/a.htm".into())
            );
            assert_eq!(
                sniff_redirect_target(r#"<meta http-equiv="refresh" content="0;URL='https://x.com/p'">"#),
                Some("https://x.com/p".into())
            );
            assert_eq!(sniff_redirect_target("<html>什么都没有</html>"), None);
        }

        #[test]
        fn script里的不可见标签清单被复用() {
            assert!(crate::core::tools::web_search::serp::INVISIBLE_TAGS.contains(&"script"));
        }
    }

}

pub mod recommend {
    //! L-1.5：直连失败后，问大模型「该领域最权威的数据源站点」，拼 site: 定向再搜一次。
    //!
    //! 三级降级链的第 3 级：内置直连 → 配置换源（env）→ LLM 推荐 → 通用 search()。
    //! 设计约束：LLM 不可用 / 超时 / 解析为空 → 返回空，静默落回下一级，绝不 panic。

    use crate::core::tools::web_search::search_planner::PlannerLlm;
    use std::time::Duration;

    /// LLM 推荐的单次调用超时：绝不让这条支路阻塞主链路。
    const LLM_TIMEOUT: Duration = Duration::from_secs(8);

    /// 纯逻辑：把 LLM 返回的域名列表解析成干净、去重、最多 5 个的域名 Vec。
    /// 容忍逗号/换行/分号/空白分隔，容忍 `https://`、`www.`、`site:`、markdown 噪声。
    pub fn parse_sites(text: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for raw in text.split(|c: char| c == ',' || c == '\n' || c == ';' || c.is_whitespace()) {
            let mut t = raw.trim().to_string();
            if t.is_empty() {
                continue;
            }
            // 剥掉 markdown/列表/引号噪声
            t = t.trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | '*' | '-' | '>' | '(' | ')' | '[' | ']'))
                .to_string();
            // 剥 scheme
            if let Some(rest) = t.strip_prefix("https://").or_else(|| t.strip_prefix("http://")) {
                t = rest.to_string();
            }
            // 剥 site: 前缀
            if let Some(rest) = t.strip_prefix("site:") {
                t = rest.to_string();
            }
            // 剥 www.
            if let Some(rest) = t.strip_prefix("www.") {
                t = rest.to_string();
            }
            // 域名只留到第一个 '/'（丢弃路径）
            if let Some(slash) = t.find('/') {
                t.truncate(slash);
            }
            // 剥尾部标点/点号
            t = t.trim_end_matches(|c: char| matches!(c, '.' | ',' | '。' | '！' | '？' | '，' | '；' | ':' | ' '))
                .to_string();
            // 只收像域名的 token：含点、无空白、≥3 字符
            if t.contains('.') && !t.chars().any(|c| c.is_whitespace()) && t.chars().count() >= 3 {
                let lower = t.to_ascii_lowercase();
                if !out.contains(&lower) {
                    out.push(lower);
                }
            }
            if out.len() >= 5 {
                break;
            }
        }
        out
    }

    /// 纯逻辑：把推荐域名拼成 `site:a OR site:b {query}`（单域名则 `site:a {query}`）。
    pub fn build_site_query(domains: &[String], query: &str) -> Option<String> {
        let domains: Vec<&str> = domains.iter().map(String::as_str).filter(|d| !d.trim().is_empty()).collect();
        match domains.len() {
            0 => None,
            1 => Some(format!("site:{} {}", domains[0], query.trim())),
            _ => {
                let sites = domains.iter().map(|d| format!("site:{d}")).collect::<Vec<_>>().join(" OR ");
                Some(format!("{sites} {}", query.trim()))
            }
        }
    }

    /// 问大模型推荐站点。失败 / 超时 / 解析为空 → 空 Vec，绝不 panic。
    pub async fn recommend_sites_via_llm(llm: &dyn PlannerLlm, query: &str) -> Vec<String> {
        let system = "你是搜索规划助手，只输出域名列表。";
        let user = format!(
            "对于查询「{query}」，最权威、最可靠的数据源网站有哪些？只输出域名列表，逗号分隔，最多 5 个，不要解释。"
        );
        let text = match tokio::time::timeout(LLM_TIMEOUT, llm.complete(system, &user)).await {
            Ok(Some(t)) => t,
            _ => return Vec::new(),
        };
        parse_sites(&text)
    }

    /// 第 3 级完整入口：LLM 推荐站点 → site: 定向搜索 → 非空即返回 ToolResult。
    /// LLM 不可用 / 无域名 / 搜索无结果 → None（调用方落回通用 search()）。
    pub(super) async fn try_llm_sites(
        params: &super::WebSearchInput,
        ctx: &crate::core::tools::ToolContext,
        num: usize,
        read_pages: usize,
    ) -> Option<crate::core::tools::ToolResult> {
        // LLM 未注入（ToolContext 无 llm handle）→ 直接落回通用 search()。
        let llm = ctx.llm.clone()?;
        let adapter = super::llm_adapter::LlmManagerPlannerLlm::new(llm);

        let sites = recommend_sites_via_llm(&adapter, &params.query).await;
        if sites.is_empty() {
            tracing::debug!("LLM 未推荐出权威站点，落回通用 search()");
            return None;
        }

        let q = build_site_query(&sites, &params.query)?;
        let mut plan = super::query::plan(&q);
        if let Some(f) = &params.freshness {
            plan.freshness = super::query::Freshness::parse(f);
        }

        let payload = super::search(&plan, &params.engines, num, params.drill_down, read_pages).await;
        if payload.results.is_empty() {
            tracing::debug!("site: 定向搜索无结果，落回通用 search()");
            return None;
        }

        let markdown = super::render(&payload.results, &payload.diagnostics, &plan);
        Some(crate::core::tools::ToolResult::success(markdown).with_metadata(serde_json::json!({
            "queries": plan.queries,
            "results": payload.results,
        })))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::core::tools::web_search::search_planner::PlannerLlm;

        #[test]
        fn 解析逗号分隔域名() {
            assert_eq!(
                parse_sites("example.com, sina.com.cn, www.xinhuanet.com"),
                vec!["example.com", "sina.com.cn", "xinhuanet.com"]
            );
        }

        #[test]
        fn 解析带噪声的域名() {
            assert_eq!(
                parse_sites("1. https://www.example.com/path\n2. site:gov.cn; - baidu.com."),
                vec!["example.com", "gov.cn", "baidu.com"]
            );
        }

        #[test]
        fn 解析去重并截断到5个() {
            let out = parse_sites("a.com, A.COM, b.com, c.com, d.com, e.com, f.com");
            assert_eq!(out, vec!["a.com", "b.com", "c.com", "d.com", "e.com"]);
        }

        #[test]
        fn 拼单个站点查询() {
            assert_eq!(
                build_site_query(&["example.com".into()], "贵州茅台 股价"),
                Some("site:example.com 贵州茅台 股价".into())
            );
        }

        #[test]
        fn 拼多个站点OR查询() {
            assert_eq!(
                build_site_query(&["example.com".into(), "gov.cn".into()], "贵州茅台 股价"),
                Some("site:example.com OR site:gov.cn 贵州茅台 股价".into())
            );
        }

        #[test]
        fn 空域名返回none() {
            assert_eq!(build_site_query(&[], "q"), None);
        }

        struct MockLlm {
            answer: Option<String>,
        }
        #[async_trait::async_trait]
        impl PlannerLlm for MockLlm {
            async fn complete(&self, _s: &str, user: &str) -> Option<String> {
                assert!(user.contains("最权威"), "prompt 应含推荐请求：{user}");
                self.answer.clone()
            }
        }

        #[tokio::test]
        async fn llm推荐成功返回域名() {
            let llm = MockLlm { answer: Some("sina.com.cn, eastmoney.com".into()) };
            assert_eq!(
                recommend_sites_via_llm(&llm, "贵州茅台 股价").await,
                vec!["sina.com.cn", "eastmoney.com"]
            );
        }

        #[tokio::test]
        async fn llm失败返回空() {
            let llm = MockLlm { answer: None };
            assert!(recommend_sites_via_llm(&llm, "贵州茅台 股价").await.is_empty());
        }
    }
}

use crate::core::tools::{PermissionLevel, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use engine::EngineOutcome;
use fusion::{EngineHits, Ranked};
use futures::stream::{FuturesUnordered, StreamExt};
use query::QueryPlan;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

pub struct WebSearchTool;

#[derive(Debug, Deserialize)]
struct WebSearchInput {
    query: String,
    #[serde(default = "d_num")]
    num_results: usize,
    #[serde(default = "d_engines")]
    engines: String,
    #[serde(default)]
    deep_fetch: bool,
    /// 深度检索：LLM 扩展 + 迭代闭环，直到闭环要素填满或达轮次上限（默认 false）。
    #[serde(default)]
    deep: bool,
    #[serde(default = "d_dfc")]
    deep_fetch_count: usize,
    /// 结果偏少时，自动对权威域名再打一发 site: 定向查询
    #[serde(default = "d_true")]
    drill_down: bool,
    #[serde(default)]
    no_cache: bool,
    /// 时效窗口：day/week/month/year。留空则按 query 措辞自动推断。
    #[serde(default)]
    freshness: Option<String>,
    /// 排序前先读取多少个候选页的正文。0 = 关闭（只按摘要排）。
    /// 这是准确度的主要来源：摘要是被引擎截断过的二手信息。
    #[serde(default = "d_read")]
    read_pages: usize,
}

fn d_read() -> usize {
    5
}

fn d_num() -> usize {
    5
}
fn d_engines() -> String {
    "all".into()
}
fn d_dfc() -> usize {
    3
}
fn d_true() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct Payload {
    pub results: Vec<Ranked>,
    /// 每个引擎这一轮发生了什么，用于诊断输出
    pub diagnostics: Vec<String>,
    /// 所有通道都挂了（而不是「确实没搜到」）
    pub all_channels_failed: bool,
}

fn search_cache() -> &'static cache::FlightCache<Payload> {
    static C: OnceLock<cache::FlightCache<Payload>> = OnceLock::new();
    C.get_or_init(|| cache::FlightCache::new(256))
}

fn page_cache() -> &'static cache::FlightCache<Option<String>> {
    static C: OnceLock<cache::FlightCache<Option<String>>> = OnceLock::new();
    C.get_or_init(|| cache::FlightCache::new(256))
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        crate::core::tools::_support::constants::TOOL_NAME_WEB_SEARCH
    }

    fn description(&self) -> &str {
        "并发检索多个搜索引擎（SearXNG / Brave / Bing / DuckDuckGo / 百度），\
         对结果做去重、RRF 融合与重排后返回。可选 deep_fetch 直接抓取并抽取网页正文。"
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "搜索词" },
                "num_results": { "type": "number", "description": "返回条数，默认 5，最大 20" },
                "engines": {
                    "type": "string",
                    "description": "引擎选择：'all'（默认，按可用性与健康度并发调度）或逗号分隔的 searxng/brave/bing/duckduckgo/baidu"
                },
                "deep_fetch": { "type": "boolean", "description": "是否抓取前若干条结果的网页正文，默认 false" },
                "deep": { "type": "boolean", "description": "深度检索：LLM 搜索扩展 + 迭代闭环，直到闭环要素填满或达轮次上限，默认 false" },
                "deep_fetch_count": { "type": "number", "description": "抓正文的条数，默认 3，最大 8" },
                "drill_down": { "type": "boolean", "description": "结果偏少时自动对权威域名做 site: 定向补搜，默认 true" },
                "no_cache": { "type": "boolean", "description": "跳过缓存强制重查，默认 false" },
                "read_pages": { "type": "number", "description": "排序前先读取多少个候选页正文（默认 5，上限 10）。设 0 则只按搜索摘要排序，快但准确度明显下降" },
                "freshness": { "type": "string", "description": "时效窗口 day/week/month/year，收窄到该时间段内的内容。留空则按 query 措辞自动推断（问『最新/近况』时默认近一年）" }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let params: WebSearchInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => {
                return ToolResult::error(crate::core::tools::_support::helpers::format_tool_input_error(
                    "jmis_search",
                    r#""query": "...""#,
                    &e,
                ))
            }
        };
        if params.query.trim().is_empty() {
            return ToolResult::error("jmis_search：query 不能为空");
        }

        let num = params.num_results.clamp(1, 20);
        let read_pages = params.read_pages.min(10);
        let dfc = params.deep_fetch_count.clamp(1, 8);

        // L-1 直连器：时间/计算/国家元数据/股票/汇率/DNS/IP/天气 等经典工具
        // 由客户端本地/直连处理，不经大模型；命中即短路返回。
        // 网络类直连失败（接口不可达/解析失败）则进入下一级。
        if let Some(d) = crate::core::tools::web_search::answerer::detect(&params.query) {
            if let Some(direct) = crate::core::tools::web_search::answerer::execute(&d, &params.query).await {
                tracing::debug!(kind = direct.kind, "直连器命中，短路返回");
                return ToolResult::success(direct.text);
            }
            // L-1.5（三级降级链第 3 级）：直连失败 → 问大模型推荐权威站点，
            // 拼 site: 定向再打一发；无域名 / LLM 不可用 / 搜索无结果 → 落回通用 search()。
            tracing::debug!(kind = ?d.kind, "直连执行失败，进入 LLM 推荐站点一级");
            if let Some(md) = recommend::try_llm_sites(&params, ctx, num, read_pages).await {
                return md;
            }
        }

        let mut plan = query::plan(&params.query);
        if let Some(f) = &params.freshness {
            plan.freshness = query::Freshness::parse(f);
        }

        // L0.5 LLM 搜索扩展（可选）：问大模型「该领域哪些站点/哪些搜索词」，注入垂域元数据。
        // LLM 不可用 / 失败则静默退回确定式 plan，绝不阻断检索。
        let planner = ctx.llm.clone().map(llm_adapter::LlmManagerPlannerLlm::new);
        let mut slots: Vec<String> = Vec::new();
        if let Some(p) = planner.as_ref() {
            if let Some(exp) = search_planner::expand(p, &params.query, &domains::registry().sites).await {
                for q in &exp.extra_queries {
                    if !q.trim().is_empty() && !plan.queries.iter().any(|e| e == q) {
                        plan.queries.push(q.clone());
                    }
                }
                slots = exp.required_slots;
                tracing::debug!(
                    extra = ?exp.extra_queries,
                    slots = ?slots,
                    "jmis_search LLM 扩展已并入"
                );
            }
        }

        tracing::debug!(
            original = %plan.original,
            primary = %plan.primary,
            queries = ?plan.queries,
            entities = ?plan.entities,
            engines = %params.engines,
            "jmis_search 开始"
        );

        let payload: Arc<Payload> = if params.deep && !slots.is_empty() {
            // 深度检索：LLM 迭代闭环，直到闭环要素填满或达轮次上限。
            // slots 非空意味着上面的 LLM 扩展成功，planner 必可用；None 兜底单发。
            match planner.as_ref() {
                Some(p) => Arc::new(deep_search(&plan, &params.engines, num, params.drill_down, read_pages, p, &slots).await),
                None => Arc::new(search(&plan, &params.engines, num, params.drill_down, read_pages).await),
            }
        } else if params.no_cache {
            Arc::new(search(&plan, &params.engines, num, params.drill_down, read_pages).await)
        } else {
            let key = format!("{}|r{read_pages}", query::cache_key(&plan, &params.engines, num));
            search_cache()
                .get_or_init(
                    &key,
                    // 关键：失败/空结果只缓存 30 秒。旧实现把空结果按 5 分钟缓存，
                    // 一次解析失败会让接下来五分钟的所有重试连请求都不发。
                    |p: &Payload| {
                        if p.results.is_empty() {
                            Duration::from_secs(30)
                        } else {
                            Duration::from_secs(300)
                        }
                    },
                    || search(&plan, &params.engines, num, params.drill_down, read_pages),
                )
                .await
        };

        let mut results = payload.results.clone();

        if payload.all_channels_failed && results.is_empty() {
            return ToolResult::error(format!(
                "本次检索未取得结果（通道状态：{}）。这不等于「网上没有」。\n\
                 已尝试全部可用通道，无需重复调用本工具；换一组关键词更可能有效。\n\
                 若持续如此：配置 SEARXNG_URL 或 BRAVE_SEARCH_API_KEY 走稳定通道。",
                payload.diagnostics.join("；")
            ));
        }

        if params.deep_fetch && !results.is_empty() {
            hydrate(&mut results, dfc).await;
        }

        // Markdown 字符串保留不动（旧路径 / 调试兜底），额外挂结构化 metadata：
        // 前端 SearchResultsContent 据此渲染「折叠列表 + 右侧预览」，无 metadata 时回退平铺。
        let markdown = render(&results, &payload.diagnostics, &plan);
        ToolResult::success(markdown).with_metadata(serde_json::json!({
            "queries": plan.queries,
            "results": results,
        }))
    }
}

// ───────────────────────── 管线主体 ─────────────────────────

async fn search(
    plan: &QueryPlan,
    engines_pref: &str,
    num: usize,
    drill: bool,
    read_pages: usize,
) -> Payload {
    let chosen = engine::select(engines_pref, plan.has_cjk);
    if chosen.is_empty() {
        return Payload {
            results: Vec::new(),
            diagnostics: vec!["没有可用引擎（全部熔断或未配置）".into()],
            all_channels_failed: true,
        };
    }

    let fresh = plan.freshness;
    // 主查询打所有引擎；改写变体只打前两个最快/最可靠的引擎，控住总请求量。
    let mut tasks = Vec::new();
    for (qi, q) in plan.queries.iter().enumerate() {
        for (ei, e) in chosen.iter().enumerate() {
            if qi > 0 && !(e.is_api() || ei < 2) {
                continue;
            }
            tasks.push(exec::Task { engine: *e, query: q.clone(), n: num.max(8), fresh });
        }
    }

    let cfg = exec::WaveCfg { enough: num * 4, ..Default::default() };
    let mut outcomes = exec::run_wave(tasks, &cfg).await;
    let mut ranked = fuse(&outcomes, plan, num * 4);

    // 二段钻取：主轮命中权威域名但条数不够时，对该域名再打一发 site: 查询。
    // 「何祖军 江苏科技大学」这类人名+机构查询，命中率提升最明显的就是这一步。
    if drill && ranked.len() < num {
        if let Some(host) = fusion::drill_host(&ranked, plan) {
            let dq = query::drill_query(plan, &host);
            tracing::debug!(%dq, "触发二段钻取");
            let drill_tasks: Vec<exec::Task> = chosen
                .iter()
                .take(3)
                .map(|e| exec::Task { engine: *e, query: dq.clone(), n: num.max(8), fresh })
                .collect();
            let cfg2 = exec::WaveCfg {
                global: Duration::from_millis(2500),
                enough: num * 2,
                ..Default::default()
            };
            outcomes.extend(exec::run_wave(drill_tasks, &cfg2).await);
            ranked = fuse(&outcomes, plan, num * 4);
        }
    }

    // 跳板链接还原：必须在读正文之前做，否则抓回来的全是跳板页。
    resolve_redirects(&mut ranked, read_pages.max(num)).await;

    // ── 读页再排 ──
    // 豆包那种「参考 11 篇资料」的准确度来自**读完正文才排序**。
    // 摘要是被引擎截断过的二手信息，拿它排序天然吃亏。
    if read_pages > 0 && !ranked.is_empty() {
        let k = read_pages.min(ranked.len());
        read_and_rerank(&mut ranked, k, plan).await;
    }
    ranked.truncate(num);

    let diagnostics = diagnose(&outcomes);
    let all_channels_failed = !outcomes.is_empty()
        && outcomes.iter().all(|o| o.status.is_failure());

    Payload { results: ranked, diagnostics, all_channels_failed }
}

/// 深度检索：LLM 迭代闭环。有界 MAX_ROUNDS 轮，每轮 检索 → 集成证据 → 判定闭环，
/// 未闭环则用 next_queries 再搜。LLM 评估失败视为闭环，交付已有证据，绝不卡死。
async fn deep_search(
    plan: &QueryPlan,
    engines_pref: &str,
    num: usize,
    drill: bool,
    read_pages: usize,
    llm: &dyn crate::core::tools::web_search::search_planner::PlannerLlm,
    slots: &[String],
) -> Payload {
    let mut all: Vec<Ranked> = Vec::new();
    let mut diags: Vec<String> = Vec::new();
    let mut evidence: Vec<String> = Vec::new();
    let mut queries: Vec<String> = plan.queries.clone();

    for round in 0..crate::core::tools::web_search::closure::MAX_ROUNDS {
        let mut p = plan.clone();
        p.queries = queries.clone();
        let payload = search(&p, engines_pref, num, drill, read_pages).await;

        for r in &payload.results {
            if !all.iter().any(|x| x.url == r.url) {
                all.push(r.clone());
            }
        }
        diags.extend(payload.diagnostics.iter().cloned());
        for r in &payload.results {
            let e = if !r.evidence.is_empty() { r.evidence.join(" ") } else { r.snippet.clone() };
            if !e.is_empty() {
                evidence.push(e);
            }
        }

        if slots.is_empty() || evidence.is_empty() {
            break;
        }

        match crate::core::tools::web_search::closure::evaluate(llm, &plan.primary, &evidence, slots).await {
            Some(e) if !e.is_closed && !e.next_queries.is_empty() => {
                tracing::debug!(round = round + 1, gap = %e.gap, next = ?e.next_queries, "未闭环，继续迭代");
                queries = e.next_queries;
                queries.truncate(3);
            }
            Some(_) => {
                tracing::debug!(round = round + 1, "已闭环");
                break;
            }
            None => {
                tracing::debug!(round = round + 1, "闭环评估失败，视为闭环交付");
                break;
            }
        }
    }

    all.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    all.truncate(num);

    Payload { results: all, diagnostics: diags, all_channels_failed: false }
}

fn fuse(outcomes: &[EngineOutcome], plan: &QueryPlan, want: usize) -> Vec<Ranked> {
    let hits: Vec<EngineHits> = outcomes
        .iter()
        .map(|o| EngineHits {
            engine: o.engine.name(),
            weight: o.engine.weight(),
            results: &o.results,
        })
        .collect();
    fusion::fuse(&hits, plan, want)
}

async fn resolve_redirects(ranked: &mut [Ranked], top: usize) {
    let targets: Vec<(usize, String)> = ranked
        .iter()
        .take(top.max(5))
        .enumerate()
        .filter(|(_, r)| r.needs_resolve)
        .map(|(i, r)| (i, r.url.clone()))
        .collect();
    if targets.is_empty() {
        return;
    }

    let mut futs = FuturesUnordered::new();
    for (i, u) in targets {
        futs.push(async move { (i, extract::resolve_redirect(&u).await) });
    }
    let deadline = Duration::from_millis(2000);
    let collected: Vec<(usize, Option<String>)> =
        match tokio::time::timeout(deadline, futs.by_ref().collect()).await {
            Ok(v) => v,
            Err(_) => Vec::new(),
        };
    for (i, real) in collected {
        if let Some(real) = real {
            if let Some(n) = fusion::normalize(&real) {
                ranked[i].url = n.display;
                ranked[i].host = n.host;
                ranked[i].needs_resolve = false;
            }
        }
    }
}

/// 并发抓正文。每页单独走页面缓存，超预算的直接放弃（不影响已有结果）。
async fn hydrate(results: &mut [Ranked], count: usize) {
    let n = count.min(results.len());
    let urls: Vec<(usize, String)> =
        results.iter().take(n).enumerate().map(|(i, r)| (i, r.url.clone())).collect();

    let mut futs = FuturesUnordered::new();
    for (i, u) in urls {
        futs.push(async move {
            let got = page_cache()
                .get_or_init(
                    &u,
                    |v: &Option<String>| {
                        if v.is_some() {
                            Duration::from_secs(600)
                        } else {
                            Duration::from_secs(60)
                        }
                    },
                    || extract::fetch_page(&u),
                )
                .await;
            (i, (*got).clone())
        });
    }

    let collected: Vec<(usize, Option<String>)> =
        match tokio::time::timeout(Duration::from_millis(6000), futs.by_ref().collect()).await {
            Ok(v) => v,
            Err(_) => {
                tracing::warn!("deep_fetch 触达预算上限，部分正文缺失");
                Vec::new()
            }
        };
    for (i, c) in collected {
        results[i].content = c;
    }
}

fn diagnose(outcomes: &[EngineOutcome]) -> Vec<String> {
    let mut per: std::collections::BTreeMap<&'static str, (usize, usize, Vec<String>)> =
        Default::default();
    for o in outcomes {
        let e = per.entry(o.engine.name()).or_default();
        e.0 += o.results.len();
        e.1 = e.1.max(o.elapsed_ms as usize);
        if o.status.is_failure() {
            e.2.push(o.status.label());
        }
    }
    per.into_iter()
        .map(|(name, (hits, ms, mut fails))| {
            if fails.is_empty() {
                format!("{name} {hits}条/{ms}ms")
            } else {
                fails.dedup();
                format!("{name} {}", fails.join(","))
            }
        })
        .collect()
}

// ───────────────────────── 输出 ─────────────────────────

/// 并发读取候选页正文 → 抽证据 → 用全文重排。
///
/// 三件事一起做：
///   1. 抓取统一走 `web_fetch::fetch`，复用它的降级阶梯与编码识别，
///      不再各留一套实现；
///   2. 从正文里挑出真正含实体的句子作为证据（而不是整页前 3000 字）；
///   3. 有 cross-encoder 就用全文重排，没有就用全文做词面打分。
async fn read_and_rerank(ranked: &mut Vec<Ranked>, k: usize, plan: &QueryPlan) {
    // 明知抓不到的域名直接跳过：抓了必失败，白耗一次超时预算、
    // 产生一条噪音错误、还把通道健康度拉低。摘要已经够用。
    let urls: Vec<(usize, String)> = ranked
        .iter()
        .take(k)
        .enumerate()
        .filter(|(_, r)| {
            if fusion::fetchable(&r.host) {
                true
            } else {
                tracing::debug!(host = %r.host, "已知需登录/JS 渲染，跳过抓取");
                false
            }
        })
        .map(|(i, r)| (i, r.url.clone()))
        .collect();

    let mut futs = FuturesUnordered::new();
    for (i, u) in urls {
        futs.push(async move {
            let text = page_cache()
                .get_or_init(
                    &u,
                    |v: &Option<String>| {
                        if v.is_some() {
                            Duration::from_secs(600)
                        } else {
                            Duration::from_secs(60)
                        }
                    },
                    || async {
                        extract::fetch_page(&u).await
                    },
                )
                .await;
            (i, (*text).clone())
        });
    }

    let collected: Vec<(usize, Option<String>)> =
        match tokio::time::timeout(Duration::from_millis(7000), futs.by_ref().collect()).await {
            Ok(v) => v,
            Err(_) => {
                tracing::warn!("读页阶段触达预算上限，按已有内容排序");
                Vec::new()
            }
        };

    for i in 0..k.min(ranked.len()) {
        if !fusion::fetchable(&ranked[i].host) {
            ranked[i].content = Some("〔该站需要登录态或 JS 渲染，未抓取正文；以下结论仅基于检索摘要〕".into());
        }
    }
    for (i, text) in collected {
        let Some(text) = text else { continue };
        let ev = evidence::pick(&text, &plan.entities, 3, 600);
        // 正文里也可能有更准确的日期
        if ranked[i].date.is_none() {
            ranked[i].date = dates::extract(&text);
        }
        ranked[i].evidence = ev;
        ranked[i].content = Some(extract::cap_chars(&text, 1200));
    }

    // ── cross-encoder 重排 ──
    let docs: Vec<String> = ranked
        .iter()
        .take(k)
        .map(|r| {
            let body = if !r.evidence.is_empty() {
                r.evidence.join(" ")
            } else {
                r.content.clone().unwrap_or_else(|| r.snippet.clone())
            };
            // 重排模型的输入窗口有限，标题 + 证据比整页更有效
            let d = format!("{} {}", r.title, body);
            d.chars().take(900).collect()
        })
        .collect();

    if let Some(scores) = rerank::score(&plan.primary, &docs).await {
        let norm = rerank::normalize(&scores);
        for (i, sc) in norm.iter().enumerate() {
            ranked[i].ce = Some(*sc);
            // 重排分权重给得高：它比词面打分可靠得多
            ranked[i].score += 1.8 * sc;
        }
        tracing::debug!(k, "cross-encoder 重排已应用");
    } else {
        // 没有重排服务：用正文证据补一次词面分，也比只看摘要强
        for i in 0..k.min(ranked.len()) {
            if !ranked[i].evidence.is_empty() {
                ranked[i].score += 0.35;
            }
        }
    }

    ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
}

/// 结果里最新的日期，用来告诉上层「我能看到的最新信息截止到哪」。
fn newest_date(results: &[Ranked]) -> Option<String> {
    results.iter().filter_map(|r| r.date.clone()).max()
}

/// 供 web_fetch 在直连失败时回退取索引摘要用。
pub async fn quick_search(q: &str, num: usize) -> Vec<Ranked> {
    let plan = query::plan(q);
    search(&plan, "all", num, false, 0).await.results
}

/// 供外部（HTTP 搜索端点 / FastPlan 直调）用：完整搜索管线，返回结构化结果。
/// `read_pages` 控制排序前读正文的条数（0 = 只按摘要排，质量较低）。
pub async fn search_ranked(q: &str, num: usize, read_pages: usize) -> Vec<Ranked> {
    let plan = query::plan(q);
    search(&plan, "all", num, true, read_pages).await.results
}

/// 供外部（HTTP 搜索端点）用：深度检索（LLM 扩展 + 迭代闭环）。
/// `llm` 由调用方注入；LLM 缺失/失败时退化为单发 search。
pub async fn search_deep(
    llm: &dyn crate::core::tools::web_search::search_planner::PlannerLlm,
    q: &str,
    num: usize,
    read_pages: usize,
) -> Vec<Ranked> {
    let mut plan = query::plan(q);
    let mut slots: Vec<String> = Vec::new();
    if let Some(exp) =
        crate::core::tools::web_search::search_planner::expand(llm, q, &crate::core::tools::web_search::domains::registry().sites).await
    {
        for query in exp.extra_queries {
            if !plan.queries.contains(&query) && plan.queries.len() < 6 {
                plan.queries.push(query);
            }
        }
        slots = exp.required_slots;
    }
    if slots.is_empty() {
        return search(&plan, "all", num, true, read_pages).await.results;
    }
    deep_search(&plan, "all", num, true, read_pages, llm, &slots).await.results
}

fn render(results: &[Ranked], diagnostics: &[String], plan: &QueryPlan) -> String {
    if results.is_empty() {
        return format!(
            "未找到与「{}」相关的结果。\n通道状态：{}\n\
             注意：这表示各引擎均正常返回但无匹配，而非工具故障。",
            plan.primary,
            diagnostics.join("；")
        );
    }

    let mut out = String::new();
    for (i, r) in results.iter().enumerate() {
        out.push_str(&format!("{}. **{}**\n", i + 1, r.title));
        out.push_str(&format!("   URL: {}\n", r.url));
        out.push_str(&format!("   来源: {}\n", r.engines.join(", ")));
        if let Some(d) = &r.date {
            out.push_str(&format!("   日期: {d}\n"));
        }
        if !r.snippet.is_empty() {
            out.push_str(&format!("   摘要: {}\n", r.snippet));
        }
        if !r.evidence.is_empty() {
            out.push_str("   证据:\n");
            for e in &r.evidence {
                out.push_str(&format!("   · {e}\n"));
            }
        } else if let Some(c) = &r.content {
            out.push_str("   正文:\n   ---\n");
            out.push_str(&format!("   {c}\n"));
            out.push_str("   ---\n");
        }
        out.push('\n');
    }

    if plan.freshness.wants_recency() {
        match newest_date(results) {
            Some(d) => out.push_str(&format!(
                "[已按时效性收窄检索窗口；结果中可见的最新日期为 {d}。\
                 若该日期明显偏旧，说明公开渠道在此之后没有新的索引内容，\
                 请勿据此推断当前状态]\n"
            )),
            None => out.push_str(
                "[已按时效性收窄检索窗口，但结果中未能解析出发布日期，无法判断新旧]\n",
            ),
        }
    }

    if !diagnostics.is_empty() {
        out.push_str(&format!("[通道覆盖：{}]\n", diagnostics.join("；")));
    }

    let degraded: Vec<&String> = diagnostics
        .iter()
        .filter(|d| !d.contains("条/"))
        .collect();
    if !degraded.is_empty() {
        out.push_str(&format!(
            "[以下通道本轮未产出，已由其他通道补足：{}]\n",
            degraded.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("；")
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Engine, EngineStatus};

    fn ranked(t: &str, u: &str, engines: Vec<&'static str>) -> Ranked {
        Ranked {
            title: t.into(),
            url: u.into(),
            snippet: "摘要".into(),
            content: None,
            host: "x.com".into(),
            engines,
            needs_resolve: false,
            date: None,
            evidence: Vec::new(),
            ce: None,
            rrf: 0.1,
            lex: 0.5,
            prior: 0.0,
            score: 1.0,
        }
    }

    #[test]
    fn 空结果的文案要说明不是故障() {
        let p = query::plan("何祖军");
        let s = render(&[], &["bing 0条/120ms".into()], &p);
        assert!(s.contains("而非工具故障"), "{s}");
    }

    #[test]
    fn 渲染包含来源引擎() {
        let p = query::plan("q");
        let s = render(&[ranked("T", "https://a.com", vec!["bing", "baidu"])], &[], &p);
        assert!(s.contains("来源: bing, baidu"), "{s}");
    }

    #[test]
    fn 有降级通道时追加提示() {
        let p = query::plan("q");
        let s = render(
            &[ranked("T", "https://a.com", vec!["bing"])],
            &["bing 3条/100ms".into(), "baidu 被拦截".into()],
            &p,
        );
        assert!(s.contains("已由其他通道补足"), "{s}");
        assert!(s.contains("baidu 被拦截"), "{s}");
    }

    #[test]
    fn 全部正常时不追加提示() {
        let p = query::plan("q");
        let s = render(
            &[ranked("T", "https://a.com", vec!["bing"])],
            &["bing 3条/100ms".into()],
            &p,
        );
        assert!(!s.contains("已由其他通道补足"), "{s}");
    }

    #[test]
    fn 诊断按引擎聚合() {
        let outcomes = vec![
            EngineOutcome {
                engine: Engine::Bing,
                query: "a".into(),
                status: EngineStatus::Ok,
                results: vec![serp::RawResult {
                    title: "t".into(),
                    url: "https://a.com".into(),
                    snippet: String::new(),
                }],
                elapsed_ms: 120,
            },
            EngineOutcome {
                engine: Engine::Baidu,
                query: "a".into(),
                status: EngineStatus::Blocked,
                results: vec![],
                elapsed_ms: 300,
            },
        ];
        let d = diagnose(&outcomes);
        assert!(d.iter().any(|s| s.starts_with("bing 1条/")), "{d:?}");
        assert!(d.iter().any(|s| s.contains("被拦截")), "{d:?}");
    }

    #[test]
    fn 入参上限被夹紧() {
        let v = json!({"query":"a","num_results":999,"deep_fetch_count":99});
        let p: WebSearchInput = serde_json::from_value(v).unwrap();
        assert_eq!(p.num_results.clamp(1, 20), 20);
        assert_eq!(p.deep_fetch_count.clamp(1, 8), 8);
    }

    #[test]
    fn 人物查询会生成简介变体() {
        // 豆包查「惠晓滨」用的就是「惠晓滨 简介」+「惠晓滨」
        let p = query::plan("惠晓滨");
        assert!(p.looks_like_person());
        assert!(
            p.queries.iter().any(|q| q.contains("简介")),
            "人物查询应补一条『简介』，实际 {:?}",
            p.queries
        );
    }

    #[test]
    fn 机构与概念不会被当成人名() {
        for q in ["江苏科技大学", "西安汇智信息科技有限公司", "快速排序算法", "PostgreSQL 数据库"] {
            assert!(!query::plan(q).looks_like_person(), "{q} 不应判为人物查询");
        }
    }

    #[test]
    fn 已知抓不到的站点不发起抓取() {
        for h in ["xueshu.baidu.com", "www.tianyancha.com", "zhuanlan.zhihu.com", "kns.cnki.net"] {
            assert!(!fusion::fetchable(h), "{h} 应跳过抓取");
        }
        for h in ["just.edu.cn", "www.afeu.edu.cn", "mp.weixin.qq.com"] {
            assert!(fusion::fetchable(h), "{h} 应正常抓取");
        }
    }

    #[test]
    fn 实体源只在人物查询时加权() {
        let person = query::plan("惠晓滨");
        let thing = query::plan("快速排序算法");
        assert!(person.looks_like_person() && !thing.looks_like_person());
    }

    #[test]
    fn 通道被抽干时会半开放回() {
        health::reset_for_test();
        // 把所有中文引擎打到熔断
        for e in ["baidu", "bing", "duckduckgo", "sogou", "weixin", "so360"] {
            for _ in 0..5 {
                health::record_fail(e);
            }
        }
        let chosen = engine::select("all", true);
        assert!(
            chosen.len() >= 2,
            "熔断器不能把引擎池抽干，否则单通道超时就报『全部失败』，实际 {chosen:?}"
        );
        health::reset_for_test();
    }

    #[test]
    fn 读页数默认开启且有上限() {
        let p: WebSearchInput = serde_json::from_value(json!({"query":"a"})).unwrap();
        assert_eq!(p.read_pages, 5, "默认应先读 5 页正文再排序");
        let p: WebSearchInput =
            serde_json::from_value(json!({"query":"a","read_pages":999})).unwrap();
        assert_eq!(p.read_pages.min(10), 10);
    }

    #[test]
    fn 中文引擎全部就位() {
        use engine::Engine as E;
        for e in [E::Baidu, E::Sogou, E::Weixin, E::So360] {
            assert!(e.cjk_only(), "{} 应只在中文查询启用", e.name());
        }
        // 英文查询不该被这些引擎拖慢
        health::reset_for_test();
        let en = engine::select("all", false);
        assert!(!en.contains(&E::Weixin));
        assert!(en.contains(&E::Bing));
    }

    #[test]
    fn idf让高频片段降权() {
        // 「大学」出现在每一条候选里，「祖军」只在一条里 —— 后者才有区分度
        let g = vec![("大学".to_string(), 1.0), ("祖军".to_string(), 1.0)];
        let corpus: Vec<String> = vec![
            "某某大学首页".into(),
            "另一所大学".into(),
            "第三所大学".into(),
            "何祖军 大学".into(),
        ];
        let w = fusion::idf_weights(&g, &corpus);
        let da = w.iter().find(|(k, _)| k == "大学").unwrap().1;
        let zu = w.iter().find(|(k, _)| k == "祖军").unwrap().1;
        assert!(zu > da, "高区分度片段权重应更高：祖军={zu} 大学={da}");
    }

    #[test]
    fn 默认值符合预期() {
        let p: WebSearchInput = serde_json::from_value(json!({"query":"a"})).unwrap();
        assert_eq!(p.num_results, 5);
        assert_eq!(p.engines, "all");
        assert!(!p.deep_fetch);
        assert!(p.drill_down, "钻取默认开启");
        assert!(!p.no_cache);
    }
}
