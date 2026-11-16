//! L-1：直连器（answerer）。
//!
//! 客户端本地直接处理的高频、确定性查询，不经过大模型、不发起外网检索，
//! 毫秒级返回结构化结果 —— 老板说的「很多工具内置到客户端处理逻辑中，
//! 越来越快准智」的落地层。
//!
//! 分两类：
//!   * **本地类**（time / calc / country）：零网络、零 Key、纯函数。
//!   * **网络类**（stock / fx / dns / ip）：国内免 Key 稳定接口，直连取数。
//!
//! 设计约束：
//!   * 检测必须**保守**（宁可漏判、不可误判）；执行失败返回 `None`，
//!     由调用方落回正常检索兜底 —— 直连是增益项，绝不单点故障。
//!
//! 可维护性约定：网络端点元数据（URL/编码/Referer）集中在本文件顶部
//! 「端点元数据」区，改源/换源/禁用只改那一处；解析逻辑各归各函数。
//! 不稳定的个人免费 API（如 Timor 节假日，实测被 Cloudflare 拦截）不硬编码，
//! 改为本地静态数据兜底。

use chrono::{Datelike, FixedOffset, Local, Timelike, Weekday};
use std::time::Duration;

// ── 端点元数据（改源 / 换源 / 禁用只改这里）──────────────────────────────
// 这些是国内十年稳定的免 Key 接口。若某个源失效，改这里的常量即可，
// 无需改动解析逻辑。常量只是**默认值**：可用 env 覆盖（JMIS_STOCK_API 等），
// 「内置不行就配一个网站」无需改代码。
/// 新浪股票行情：{code} 如 sh600519 / sz000001。GBK，需 Referer。
const STOCK_API: &str = "http://hq.sinajs.cn/list={code}";
/// 新浪外汇：{pair} 如 usdcny（美元兑人民币）。GBK，需 Referer。
const FX_API: &str = "http://hq.sinajs.cn/list=fx_{pair}";
/// 阿里 DoH：标准 DNS-over-HTTPS JSON。UTF-8。
const DNS_API: &str = "https://223.5.5.5/resolve?name={domain}&type=A";
/// IP 出口：纯文本返回本机公网 IP。UTF-8。
const IP_API: &str = "http://ip.3322.net";
const REFERER_SINA: &str = "https://finance.sina.com.cn";

/// 和风天气（需 QWEATHER_API_KEY）：城市地理信息查询（location → id）。
const QWEATHER_GEO_API: &str = "https://geoapi.qweather.com/v2/city/lookup";
/// 和风天气：实时天气。
const QWEATHER_NOW_API: &str = "https://devapi.qweather.com/v7/weather/now";

/// env 优先、硬编码默认兜底。env 未设置或为空时回退 default。
/// 「内置不行就配一个网站」：改 env 即可换源，无需改代码。
fn endpoint(env_key: &str, default: &str) -> String {
    std::env::var(env_key)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

// ── 类型 ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DirectKind {
    Time,
    Calc,
    Country,
    Stock,
    Fx,
    Dns,
    Ip,
    Weather,
}

/// 检测结果：`arg` 为提取出的参数（股票代码 / 货币对 / 域名 / 城市名），本地类为 None。
#[derive(Debug, Clone)]
pub struct Detected {
    pub kind: DirectKind,
    pub arg: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DirectAnswer {
    /// 类别标签（time / calc / country / stock / fx / dns / ip），用于日志与渲染。
    pub kind: &'static str,
    pub text: String,
}

// ── 入口 ──

/// 同步检测 query 属于哪类直连查询。None = 交给正常检索。
pub fn detect(query: &str) -> Option<Detected> {
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    detect_time(q).then(|| Detected { kind: DirectKind::Time, arg: None })
        .or_else(|| detect_calc(q).then(|| Detected { kind: DirectKind::Calc, arg: None }))
        .or_else(|| detect_country(q).then(|| Detected { kind: DirectKind::Country, arg: None }))
        .or_else(|| detect_stock(q).map(|code| Detected { kind: DirectKind::Stock, arg: Some(code) }))
        .or_else(|| detect_fx(q).map(|pair| Detected { kind: DirectKind::Fx, arg: Some(pair) }))
        .or_else(|| detect_dns(q).map(|domain| Detected { kind: DirectKind::Dns, arg: Some(domain) }))
        .or_else(|| detect_ip(q).then(|| Detected { kind: DirectKind::Ip, arg: None }))
        .or_else(|| detect_weather(q).map(|city| Detected { kind: DirectKind::Weather, arg: Some(city) }))
}

/// 执行直连。本地类同步算完；网络类发请求，失败返回 None（调用方落回检索）。
pub async fn execute(d: &Detected, query: &str) -> Option<DirectAnswer> {
    match d.kind {
        DirectKind::Time => answer_time(query),
        DirectKind::Calc => answer_calc(query),
        DirectKind::Country => answer_country(query),
        DirectKind::Stock => answer_stock(d.arg.as_deref()?).await,
        DirectKind::Fx => answer_fx(d.arg.as_deref()?).await,
        DirectKind::Dns => answer_dns(d.arg.as_deref()?).await,
        DirectKind::Ip => answer_ip().await,
        DirectKind::Weather => answer_weather(d.arg.as_deref().unwrap_or("")).await,
    }
}

// ───────────────────────── 时间（本地） ─────────────────────────

fn detect_time(q: &str) -> bool {
    const MARKS: &[&str] = &[
        "现在几点", "现在时间", "当前时间", "几点了", "现在是什么时间", "几点几分",
        "今天几号", "今天日期", "今天是星期几", "星期几", "今天周几", "今天星期几",
        "现在日期", "今天是什么日子", "北京时间", "what time", "current time",
    ];
    // 业务时刻类词：出现说明问的是「几点开门/发车」，不是「现在几点」。
    const NEGATORS: &[&str] = &[
        "开门", "关门", "营业", "上班", "下班", "发车", "起飞", "降落", "截止",
        "开始", "结束", "直播", "比赛", "考试", "报到", "熄灯",
    ];
    MARKS.iter().any(|m| q.contains(m))
        && q.chars().count() <= 16
        && !NEGATORS.iter().any(|n| q.contains(n))
}

fn answer_time(q: &str) -> Option<DirectAnswer> {
    if !detect_time(q) {
        return None;
    }
    let now = Local::now();
    let bj = now.with_timezone(&FixedOffset::east_opt(8 * 3600).expect("UTC+8 恒合法"));
    let date = format!("{}-{:02}-{:02}", bj.year(), bj.month(), bj.day());
    let clock = format!("{:02}:{:02}:{:02}", bj.hour(), bj.minute(), bj.second());
    let mut text = format!("现在是北京时间 {date}（{}）{clock}。", weekday_cn(bj.weekday()));

    let local_date = format!("{}-{:02}-{:02}", now.year(), now.month(), now.day());
    let local_clock = format!("{:02}:{:02}:{:02}", now.hour(), now.minute(), now.second());
    if local_date != date || local_clock != clock {
        text.push_str(&format!(" 系统本地时间 {local_date} {local_clock}。"));
    }
    Some(DirectAnswer { kind: "time", text })
}

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

// ───────────────────────── 计算器（本地） ─────────────────────────

fn detect_calc(q: &str) -> bool {
    strip_calc_wrappers(q).is_some_and(|e| {
        e.chars().any(|c| c.is_ascii_digit()) && e.chars().any(|c| "+-*/%".contains(c))
    })
}

fn answer_calc(q: &str) -> Option<DirectAnswer> {
    let expr = strip_calc_wrappers(q)?;
    if !expr.chars().any(|c| c.is_ascii_digit()) || !expr.chars().any(|c| "+-*/%".contains(c)) {
        return None;
    }
    let value = eval(&expr)?;
    Some(DirectAnswer { kind: "calc", text: format!("{expr} = {}", format_num(value)) })
}

/// 从「123*456」「计算 123*456」「123*456 等于多少」里剥出纯算术表达式。
fn strip_calc_wrappers(q: &str) -> Option<String> {
    let t = q.trim();
    if is_pure_expr(t) {
        return Some(t.to_string());
    }
    for suf in ["等于多少", "是多少", "等于几", "=?", "=？", "等于"] {
        if let Some(s) = t.strip_suffix(suf) {
            let s = s.trim();
            if !s.is_empty() && is_pure_expr(s) {
                return Some(s.to_string());
            }
        }
    }
    for pre in ["计算", "算一下"] {
        if let Some(s) = t.strip_prefix(pre) {
            let s = s.trim();
            if !s.is_empty() && is_pure_expr(s) {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn is_pure_expr(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || "+-*/%(). ".contains(c))
}

fn format_num(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "∞".into() } else { "-∞".into() };
    }
    if v.fract().abs() < 1e-9 && v.abs() < 9e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{:.8}", v);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// 递归下降求值：`+ - * / %`、括号、十进制数；除零返回 None。
fn eval(expr: &str) -> Option<f64> {
    struct P<'a> {
        s: &'a [char],
        i: usize,
    }
    impl P<'_> {
        fn peek(&self) -> Option<char> {
            self.s.get(self.i).copied()
        }
        fn skip_ws(&mut self) {
            while self.peek() == Some(' ') {
                self.i += 1;
            }
        }
        fn expr(&mut self) -> Option<f64> {
            let mut v = self.term()?;
            loop {
                self.skip_ws();
                match self.peek() {
                    Some('+') => {
                        self.i += 1;
                        v += self.term()?;
                    }
                    Some('-') => {
                        self.i += 1;
                        v -= self.term()?;
                    }
                    _ => break,
                }
            }
            Some(v)
        }
        fn term(&mut self) -> Option<f64> {
            let mut v = self.unary()?;
            loop {
                self.skip_ws();
                match self.peek() {
                    Some('*') => {
                        self.i += 1;
                        v *= self.unary()?;
                    }
                    Some('/') => {
                        self.i += 1;
                        let d = self.unary()?;
                        if d == 0.0 {
                            return None;
                        }
                        v /= d;
                    }
                    Some('%') => {
                        self.i += 1;
                        let d = self.unary()?;
                        if d == 0.0 {
                            return None;
                        }
                        v %= d;
                    }
                    _ => break,
                }
            }
            Some(v)
        }
        fn unary(&mut self) -> Option<f64> {
            self.skip_ws();
            match self.peek() {
                Some('-') => {
                    self.i += 1;
                    Some(-self.unary()?)
                }
                Some('+') => {
                    self.i += 1;
                    self.unary()
                }
                _ => self.primary(),
            }
        }
        fn primary(&mut self) -> Option<f64> {
            self.skip_ws();
            match self.peek() {
                Some('(') => {
                    self.i += 1;
                    let v = self.expr()?;
                    self.skip_ws();
                    if self.peek() == Some(')') {
                        self.i += 1;
                        Some(v)
                    } else {
                        None
                    }
                }
                Some(c) if c.is_ascii_digit() || c == '.' => self.number(),
                _ => None,
            }
        }
        fn number(&mut self) -> Option<f64> {
            let start = self.i;
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.i += 1;
            }
            if self.peek() == Some('.') {
                self.i += 1;
                while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                    self.i += 1;
                }
            }
            self.s[start..self.i].iter().collect::<String>().parse().ok()
        }
    }

    let chars: Vec<char> = expr.chars().collect();
    let mut p = P { s: &chars, i: 0 };
    let v = p.expr()?;
    p.skip_ws();
    if p.peek().is_some() {
        return None;
    }
    Some(v)
}

// ───────────────────────── 国家元数据（本地） ─────────────────────────

#[derive(Debug, Clone, Copy)]
struct Country {
    name: &'static str,
    capital: &'static str,
    currency: &'static str,
    dial: &'static str,
    iso2: &'static str,
    population: &'static str,
}

const COUNTRIES: &[Country] = &[
    Country { name: "中国", capital: "北京", currency: "人民币（CNY）", dial: "+86", iso2: "CN", population: "约 14.1 亿" },
    Country { name: "美国", capital: "华盛顿", currency: "美元（USD）", dial: "+1", iso2: "US", population: "约 3.3 亿" },
    Country { name: "日本", capital: "东京", currency: "日元（JPY）", dial: "+81", iso2: "JP", population: "约 1.25 亿" },
    Country { name: "韩国", capital: "首尔", currency: "韩元（KRW）", dial: "+82", iso2: "KR", population: "约 5200 万" },
    Country { name: "英国", capital: "伦敦", currency: "英镑（GBP）", dial: "+44", iso2: "GB", population: "约 6800 万" },
    Country { name: "法国", capital: "巴黎", currency: "欧元（EUR）", dial: "+33", iso2: "FR", population: "约 6800 万" },
    Country { name: "德国", capital: "柏林", currency: "欧元（EUR）", dial: "+49", iso2: "DE", population: "约 8400 万" },
    Country { name: "俄罗斯", capital: "莫斯科", currency: "卢布（RUB）", dial: "+7", iso2: "RU", population: "约 1.44 亿" },
    Country { name: "印度", capital: "新德里", currency: "印度卢比（INR）", dial: "+91", iso2: "IN", population: "约 14.3 亿" },
    Country { name: "新加坡", capital: "新加坡", currency: "新加坡元（SGD）", dial: "+65", iso2: "SG", population: "约 590 万" },
    Country { name: "澳大利亚", capital: "堪培拉", currency: "澳元（AUD）", dial: "+61", iso2: "AU", population: "约 2600 万" },
    Country { name: "加拿大", capital: "渥太华", currency: "加元（CAD）", dial: "+1", iso2: "CA", population: "约 4000 万" },
    Country { name: "巴西", capital: "巴西利亚", currency: "雷亚尔（BRL）", dial: "+55", iso2: "BR", population: "约 2.15 亿" },
    Country { name: "意大利", capital: "罗马", currency: "欧元（EUR）", dial: "+39", iso2: "IT", population: "约 5900 万" },
    Country { name: "西班牙", capital: "马德里", currency: "欧元（EUR）", dial: "+34", iso2: "ES", population: "约 4800 万" },
    Country { name: "越南", capital: "河内", currency: "越南盾（VND）", dial: "+84", iso2: "VN", population: "约 1.0 亿" },
    Country { name: "泰国", capital: "曼谷", currency: "泰铢（THB）", dial: "+66", iso2: "TH", population: "约 7200 万" },
    Country { name: "马来西亚", capital: "吉隆坡", currency: "林吉特（MYR）", dial: "+60", iso2: "MY", population: "约 3400 万" },
    Country { name: "印度尼西亚", capital: "雅加达", currency: "印尼盾（IDR）", dial: "+62", iso2: "ID", population: "约 2.77 亿" },
    Country { name: "荷兰", capital: "阿姆斯特丹", currency: "欧元（EUR）", dial: "+31", iso2: "NL", population: "约 1800 万" },
];

#[derive(Debug, Clone, Copy)]
enum Attr {
    Capital,
    Currency,
    Dial,
    Population,
    Iso,
    Flag,
}

fn detect_country(q: &str) -> bool {
    country_attr(q).is_some() && COUNTRIES.iter().any(|c| q.contains(c.name))
}

fn answer_country(q: &str) -> Option<DirectAnswer> {
    let attr = country_attr(q)?;
    let c = COUNTRIES.iter().find(|c| q.contains(c.name))?;
    let (label, val) = match attr {
        Attr::Capital => ("首都", c.capital.to_string()),
        Attr::Currency => ("货币", c.currency.to_string()),
        Attr::Dial => ("国际电话区号", c.dial.to_string()),
        Attr::Population => ("人口", c.population.to_string()),
        Attr::Iso => ("ISO 国家代码", c.iso2.to_string()),
        Attr::Flag => ("国旗", flag_emoji(c.iso2)),
    };
    Some(DirectAnswer { kind: "country", text: format!("{}的{}是{}。", c.name, label, val) })
}

fn country_attr(q: &str) -> Option<Attr> {
    if q.contains("首都") {
        Some(Attr::Capital)
    } else if q.contains("货币") || q.contains("用什么钱") {
        Some(Attr::Currency)
    } else if q.contains("区号") {
        Some(Attr::Dial)
    } else if q.contains("人口") {
        Some(Attr::Population)
    } else if q.contains("ISO") || q.contains("国码") || q.contains("国家代码") {
        Some(Attr::Iso)
    } else if q.contains("国旗") {
        Some(Attr::Flag)
    } else {
        None
    }
}

fn flag_emoji(iso2: &str) -> String {
    iso2.chars()
        .filter_map(|c| {
            if c.is_ascii_alphabetic() {
                char::from_u32(0x1F1E6 + (c.to_ascii_uppercase() as u32 - 'A' as u32))
            } else {
                None
            }
        })
        .collect()
}

// ───────────────────────── 股票（网络，新浪） ─────────────────────────

fn detect_stock(q: &str) -> Option<String> {
    let lower = q.to_ascii_lowercase();
    // 显式 sh/sz/bj + 6 位代码
    for prefix in ["sh", "sz", "bj"] {
        if let Some(i) = lower.find(prefix) {
            let tail: Vec<char> = lower[i + prefix.len()..].chars().collect();
            if tail.len() >= 6 && tail[..6].iter().all(|c| c.is_ascii_digit()) {
                let code: String = tail[..6].iter().collect();
                return Some(format!("{prefix}{code}"));
            }
        }
    }
    // 纯 6 位数字 + 股票意图词
    if q.contains("股") || q.contains("行情") || q.contains("股价") {
        if let Some(code) = six_digit_code(q) {
            return Some(with_exchange_prefix(&code));
        }
    }
    None
}

fn six_digit_code(q: &str) -> Option<String> {
    let cs: Vec<char> = q.chars().collect();
    cs.windows(6)
        .find(|w| w.iter().all(|c| c.is_ascii_digit()))
        .map(|w| w.iter().collect())
}

fn with_exchange_prefix(code: &str) -> String {
    match code.chars().next() {
        Some('6') | Some('5') => format!("sh{code}"),
        Some('0') | Some('3') | Some('1') => format!("sz{code}"),
        Some('4') | Some('8') => format!("bj{code}"),
        _ => format!("sh{code}"),
    }
}

async fn answer_stock(code: &str) -> Option<DirectAnswer> {
    let url = endpoint("JMIS_STOCK_API", STOCK_API).replace("{code}", code);
    let bytes = get_bytes(&url, Some(REFERER_SINA)).await?;
    let text = encoding_rs::GBK.decode(&bytes).0.into_owned();
    let body = parse_sina_stock(&text)?;
    Some(DirectAnswer { kind: "stock", text: body })
}

/// 解析新浪 `var hq_str_xxx="名称,今开,昨收,现价,最高,最低,...,日期,时间";`
fn parse_sina_stock(text: &str) -> Option<String> {
    let start = text.find('"')? + 1;
    let end = text.rfind('"')?;
    let f: Vec<&str> = text[start..end].split(',').collect();
    if f.len() < 32 {
        return None;
    }
    let name = f[0];
    let open = f[1];
    let prev_close = f[2];
    let price = f[3];
    let high = f[4];
    let low = f[5];
    let volume = f[8];
    let amount = f[9].parse::<f64>().ok().map(|a| format!("{:.2} 亿", a / 1e8));
    let date = f[30];
    let time = f[31];
    let mut s = format!(
        "{name}：现价 {price}，今开 {open}，昨收 {prev_close}，最高 {high}，最低 {low}，\
         成交量 {volume} 手"
    );
    if let Some(a) = amount {
        s.push_str(&format!("，成交额 {a}"));
    }
    s.push_str(&format!("。数据时间 {date} {time}。"));
    Some(s)
}

// ───────────────────────── 汇率（网络，新浪外汇） ─────────────────────────

fn detect_fx(q: &str) -> Option<String> {
    let lower = q.to_lowercase();
    let is_fx = q.contains("汇率") || q.contains("兑") || lower.contains("usd") || lower.contains("cny");
    if !is_fx {
        return None;
    }
    if q.contains("美元") || q.contains("美金") || q.contains("人民币") || lower.contains("usd") || lower.contains("cny") {
        return Some("usdcny".into());
    }
    None
}

async fn answer_fx(pair: &str) -> Option<DirectAnswer> {
    let url = endpoint("JMIS_FX_API", FX_API).replace("{pair}", pair);
    let bytes = get_bytes(&url, Some(REFERER_SINA)).await?;
    let text = encoding_rs::GBK.decode(&bytes).0.into_owned();
    let body = parse_sina_fx(&text)?;
    Some(DirectAnswer { kind: "fx", text: body })
}

/// 解析新浪外汇 `var hq_str_fx_xxx="时间,最新价,买入,卖出,...,名称,...";`
fn parse_sina_fx(text: &str) -> Option<String> {
    let start = text.find('"')? + 1;
    let end = text.rfind('"')?;
    let f: Vec<&str> = text[start..end].split(',').collect();
    if f.len() < 10 {
        return None;
    }
    let time = f[0];
    let rate = f[1];
    let name = f[9];
    Some(format!("美元兑人民币（{name}）参考汇率 1 USD ≈ {rate} CNY。数据时间 {time}。"))
}

// ───────────────────────── DNS（网络，阿里 DoH） ─────────────────────────

fn detect_dns(q: &str) -> Option<String> {
    let lower = q.to_lowercase();
    if !(q.contains("解析") || lower.contains("dns") || q.contains("IP地址") || q.contains("ip地址") || lower.starts_with("dig ")) {
        return None;
    }
    extract_domain(q)
}

fn extract_domain(q: &str) -> Option<String> {
    let bytes = q.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_alphanumeric() {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.' || bytes[i] == b'-') {
                i += 1;
            }
            let tok = &q[start..i];
            if tok.contains('.') && !tok.starts_with('.') && !tok.ends_with('.') && tok.len() >= 3 {
                return Some(tok.to_string());
            }
        } else {
            i += 1;
        }
    }
    None
}

async fn answer_dns(domain: &str) -> Option<DirectAnswer> {
    let url = endpoint("JMIS_DNS_API", DNS_API).replace("{domain}", domain);
    let bytes = get_bytes(&url, None).await?;
    let body = parse_dns_json(&bytes, domain)?;
    Some(DirectAnswer { kind: "dns", text: body })
}

fn parse_dns_json(bytes: &[u8], domain: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let answers = v.get("Answer")?.as_array()?;
    let ips: Vec<&str> = answers.iter().filter_map(|a| a.get("data")?.as_str()).collect();
    if ips.is_empty() {
        return None;
    }
    Some(format!("{domain} 的 A 记录：{}", ips.join(", ")))
}

// ───────────────────────── IP 出口（网络） ─────────────────────────

fn detect_ip(q: &str) -> bool {
    let lower = q.to_lowercase();
    const MARKS: &[&str] = &[
        "我的ip", "本机ip", "公网ip", "我的公网ip", "出口ip", "我的出口ip",
        "我的 ip", "本机 ip", "what is my ip", "what's my ip", "my ip",
    ];
    MARKS.iter().any(|m| lower.contains(m))
}

async fn answer_ip() -> Option<DirectAnswer> {
    let bytes = get_bytes(&endpoint("JMIS_IP_API", IP_API), None).await?;
    let ip = String::from_utf8_lossy(&bytes).trim().to_string();
    if ip.is_empty() {
        return None;
    }
    Some(DirectAnswer { kind: "ip", text: format!("本机公网出口 IP：{ip}") })
}

// ───────────────────────── 天气（网络，和风天气） ─────────────────────────

/// 命中天气类查询则返回提取出的城市名（可能为空字符串），否则 None。
/// 城市提取是纯逻辑：「北京天气」→ Some("北京")；「今天天气」→ Some("")，
/// 空城市交由 `answer_weather` 用 `JMIS_WEATHER_CITY` 兜底或提示指定城市。
fn detect_weather(q: &str) -> Option<String> {
    let lower = q.to_ascii_lowercase();
    const MARKS: &[&str] = &[
        "天气", "气温", "温度", "下雨", "下不下雨", "穿什么", "冷不冷", "热不热",
        "几度", "多少度", "降雨", "降温", "升温", "weather", "temperature",
    ];
    if !MARKS.iter().any(|m| lower.contains(m)) {
        return None;
    }
    Some(extract_weather_city(q))
}

/// 从天气 query 里剥离天气/时间/语气词，剩下的就是城市名（可能为空）。
fn extract_weather_city(q: &str) -> String {
    let mut s = q.to_string();
    // 长词在前：避免「下不下雨」被「下雨」先剥成「不」再残留。
    const STOPWORDS: &[&str] = &[
        "下不下雨", "多少度", "天气预报", "气温", "温度", "天气", "降雨", "降温", "升温",
        "下雨", "穿什么", "冷不冷", "热不热", "几度", "今天", "明天", "后天", "昨天",
        "现在", "怎么样", "如何", "是什么", "吗", "呢", "啊", "吧", "了", "的",
        "请问", "帮我查", "查一下", "weather", "temperature",
    ];
    for w in STOPWORDS {
        s = s.replace(w, "");
    }
    s.chars()
        .filter(|c| !c.is_whitespace() && !"的，,。？！!？:：;；~～".contains(*c))
        .collect()
}

async fn answer_weather(city: &str) -> Option<DirectAnswer> {
    // 无 Key 走降级链（LLM 推荐 → 通用搜索）。
    let key = std::env::var("QWEATHER_API_KEY").ok().filter(|v| !v.trim().is_empty())?;
    let city = city.trim().to_string();
    if city.is_empty() {
        // 无城市：用默认城市 env，否则提示指定城市（短路，不再往下走检索）。
        return match std::env::var("JMIS_WEATHER_CITY").ok().filter(|v| !v.trim().is_empty()) {
            Some(c) => answer_weather_with_key(&key, &c).await,
            None => Some(DirectAnswer {
                kind: "weather",
                text: "请指定城市（如「北京天气」），或设置 JMIS_WEATHER_CITY 作为默认城市。".into(),
            }),
        };
    }
    answer_weather_with_key(&key, &city).await
}

async fn answer_weather_with_key(key: &str, city: &str) -> Option<DirectAnswer> {
    let id = qweather_location_id(key, city).await?;
    let detail = qweather_now(key, &id).await?;
    Some(DirectAnswer { kind: "weather", text: format!("{city} 当前 {detail}") })
}

/// 城市 → 地理 id：先取 location[0].id，退回 adm2 / adm1。
async fn qweather_location_id(key: &str, city: &str) -> Option<String> {
    let city_enc = url::form_urlencoded::byte_serialize(city.as_bytes()).collect::<String>();
    let url = format!(
        "{}?location={city_enc}&key={key}",
        endpoint("JMIS_QWEATHER_GEO_API", QWEATHER_GEO_API)
    );
    let bytes = get_bytes(&url, None).await?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    parse_qweather_location(&v)
}

fn parse_qweather_location(v: &serde_json::Value) -> Option<String> {
    if v.get("code")?.as_str()? != "200" {
        return None;
    }
    let loc = v.get("location")?.as_array()?.first()?;
    ["id", "adm2", "adm1"]
        .iter()
        .find_map(|k| loc.get(k).and_then(|x| x.as_str()).map(str::to_string))
}

async fn qweather_now(key: &str, location_id: &str) -> Option<String> {
    let loc_enc = url::form_urlencoded::byte_serialize(location_id.as_bytes()).collect::<String>();
    let url = format!(
        "{}?location={loc_enc}&key={key}",
        endpoint("JMIS_QWEATHER_NOW_API", QWEATHER_NOW_API)
    );
    let bytes = get_bytes(&url, None).await?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    parse_qweather_now(&v)
}

/// 解析实时天气，拼成「{text}，气温 {temp}℃，体感 {feelsLike}℃，湿度 {humidity}%，{windDir}{windScale}级」。
fn parse_qweather_now(v: &serde_json::Value) -> Option<String> {
    if v.get("code")?.as_str()? != "200" {
        return None;
    }
    let now = v.get("now")?;
    let text = now.get("text")?.as_str()?;
    let temp = now.get("temp")?.as_str()?;
    let feels = now.get("feelsLike")?.as_str()?;
    let humidity = now.get("humidity")?.as_str()?;
    let wind_dir = now.get("windDir")?.as_str()?;
    let wind_scale = now.get("windScale")?.as_str()?;
    Some(format!(
        "{text}，气温 {temp}℃，体感 {feels}℃，湿度 {humidity}%，{wind_dir}{wind_scale}级"
    ))
}

// ───────────────────────── 网络公共层 ─────────────────────────

async fn get_bytes(url: &str, referer: Option<&str>) -> Option<Vec<u8>> {
    let mut req = crate::core::tools::web_search::http::page_client()
        .get(url)
        .timeout(Duration::from_secs(6));
    if let Some(r) = referer {
        req = req.header(reqwest::header::REFERER, r);
    }
    let resp = req.send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.bytes().await.ok().map(|b| b.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 本地类 ──

    #[test]
    fn 时间查询命中() {
        let d = detect("现在几点了").unwrap();
        assert_eq!(d.kind, DirectKind::Time);
        assert!(answer_time("现在几点了").unwrap().text.contains("北京时间"));
    }

    #[test]
    fn 星期查询命中() {
        assert!(answer_time("今天是星期几").unwrap().text.contains("星期"));
    }

    #[test]
    fn 业务时刻类词不误判为时间() {
        assert!(detect("这家店现在几点开门").is_none());
    }

    #[test]
    fn 纯算术表达式求值() {
        let a = answer_calc("123*456").unwrap();
        assert!(a.text.contains("56088"), "{a:?}");
    }

    #[test]
    fn 带括号与优先级() {
        assert!(answer_calc("(1+2)*3").unwrap().text.contains('9'));
    }

    #[test]
    fn 中文尾缀剥离() {
        assert!(answer_calc("100/3等于多少").unwrap().text.contains("33.333"));
    }

    #[test]
    fn 除零拒绝() {
        // detect 识别为算术查询（结构正确），但 eval 阶段拒绝除零 → 执行返回 None
        assert_eq!(detect("1/0").map(|d| d.kind), Some(DirectKind::Calc));
        assert!(answer_calc("1/0").is_none());
    }

    #[test]
    fn 非算术不误判() {
        assert!(detect("帮我算算项目工期").is_none());
        assert!(detect("何祖军 江苏科技大学").is_none());
        assert!(detect("100的20%是多少").is_none());
    }

    #[test]
    fn 国家首都查询() {
        let d = detect("中国的首都是什么").unwrap();
        assert_eq!(d.kind, DirectKind::Country);
        assert!(answer_country("中国的首都是什么").unwrap().text.contains("北京"));
    }

    #[test]
    fn 国旗emoji正确() {
        assert_eq!(flag_emoji("CN"), "🇨🇳");
        assert_eq!(flag_emoji("US"), "🇺🇸");
    }

    #[test]
    fn 数字格式化() {
        assert_eq!(format_num(9.0), "9");
        assert_eq!(format_num(0.1 + 0.2), "0.3");
    }

    // ── 检测层（网络类，不发起请求） ──

    #[test]
    fn 显式股票代码命中() {
        let d = detect("sh600519 股价").unwrap();
        assert_eq!(d.kind, DirectKind::Stock);
        assert_eq!(d.arg.as_deref(), Some("sh600519"));
    }

    #[test]
    fn 纯六位代码补前缀() {
        let d = detect("600519 股票行情").unwrap();
        assert_eq!(d.arg.as_deref(), Some("sh600519"));
    }

    #[test]
    fn 汇率命中() {
        let d = detect("美元兑人民币汇率").unwrap();
        assert_eq!(d.kind, DirectKind::Fx);
        assert_eq!(d.arg.as_deref(), Some("usdcny"));
    }

    #[test]
    fn dns命中() {
        let d = detect("解析 baidu.com 的IP").unwrap();
        assert_eq!(d.kind, DirectKind::Dns);
        assert_eq!(d.arg.as_deref(), Some("baidu.com"));
    }

    #[test]
    fn ip出口命中() {
        let d = detect("我的公网IP是多少").unwrap();
        assert_eq!(d.kind, DirectKind::Ip);
    }

    #[test]
    fn 非直连查询不误判() {
        assert!(detect("何祖军 江苏科技大学").is_none());
        assert!(detect("人工智能最新进展").is_none());
    }

    // ── 解析层（用真实接口返回样本锁定字段映射） ──

    #[test]
    fn 解析新浪股票样本() {
        let raw = r#"var hq_str_sh600519="贵州茅台,1291.000,1293.090,1290.690,1298.860,1285.170,1290.240,1290.830,2297174,2963670094.000,400,1290.240,100,1289.710,200,1289.680,100,1289.650,1100,1289.570,1100,1290.830,100,1290.840,400,1291.000,100,1291.450,100,1291.850,2026-08-18,11:30:00,00,";"#;
        let out = parse_sina_stock(raw).unwrap();
        assert!(out.contains("贵州茅台"), "{out}");
        assert!(out.contains("1290.690"), "应含现价字段[3]，{out}");
        assert!(out.contains("29.64 亿"), "应含成交额换算，{out}");
    }

    #[test]
    fn 解析新浪外汇样本() {
        let raw = r#"var hq_str_fx_susdcny="11:57:34,6.7424000000,6.7434000000,6.7417000000,132.0000000000,6.7394000000,6.7434000000,6.7302000000,6.7429000000,在岸人民币,0.0178,0.0012,0.0132,此行情由新浪财经计算得出,0.0000,0.0000,,2026-08-18";"#;
        let out = parse_sina_fx(raw).unwrap();
        assert!(out.contains("6.7424"), "{out}");
        assert!(out.contains("在岸人民币"), "{out}");
    }

    #[test]
    fn 解析dns样本() {
        let raw = br#"{"Status":0,"Answer":[{"name":"baidu.com.","type":1,"data":"111.63.65.247"},{"name":"baidu.com.","type":1,"data":"110.242.74.102"}]}"#;
        let out = parse_dns_json(raw, "baidu.com").unwrap();
        assert!(out.contains("111.63.65.247"), "{out}");
        assert!(out.contains("110.242.74.102"), "{out}");
    }

    #[test]
    fn 域名提取健壮() {
        assert_eq!(extract_domain("解析 baidu.com"), Some("baidu.com".into()));
        assert_eq!(extract_domain("baidu.com 的IP地址"), Some("baidu.com".into()));
        assert!(extract_domain("何祖军 江苏科技大学").is_none());
    }

    // ── 天气（检测层 + 解析层） ──

    #[test]
    fn 天气城市提取() {
        assert_eq!(
            detect("北京天气").map(|d| (d.kind, d.arg)),
            Some((DirectKind::Weather, Some("北京".into())))
        );
        assert_eq!(
            detect("北京今天气温多少度").map(|d| d.arg),
            Some(Some("北京".into()))
        );
        assert_eq!(
            detect("上海明天穿什么").map(|d| d.arg),
            Some(Some("上海".into()))
        );
        // 无城市：命中天气但城市为空 → 交由 answer_weather 兜底/提示
        assert_eq!(
            detect("今天天气怎么样").map(|d| (d.kind, d.arg)),
            Some((DirectKind::Weather, Some(String::new())))
        );
    }

    #[test]
    fn 非天气查询不误判() {
        assert!(detect("人工智能最新进展").is_none());
        assert!(detect("何祖军 江苏科技大学").is_none());
        // 时间/计算等其它直连器不受天气检测影响，仍走各自通道
        assert_eq!(detect("现在几点了").map(|d| d.kind), Some(DirectKind::Time));
        assert_eq!(detect("123*456").map(|d| d.kind), Some(DirectKind::Calc));
    }

    #[test]
    fn 和风天气城市解析() {
        let raw = r#"{"code":"200","location":[{"name":"北京","id":"101010100","adm1":"北京市","adm2":"北京"}]}"#;
        let v: serde_json::Value = serde_json::from_str(raw).unwrap();
        assert_eq!(parse_qweather_location(&v).as_deref(), Some("101010100"));
    }

    #[test]
    fn 和风天气错误码拒绝() {
        let raw = r#"{"code":"404","location":[]}"#;
        let v: serde_json::Value = serde_json::from_str(raw).unwrap();
        assert!(parse_qweather_location(&v).is_none());
    }

    #[test]
    fn 和风天气实时解析() {
        let raw = r#"{"code":"200","now":{"text":"晴","temp":"26","feelsLike":"28","humidity":"60","windDir":"北风","windScale":"3"}}"#;
        let v: serde_json::Value = serde_json::from_str(raw).unwrap();
        let out = parse_qweather_now(&v).unwrap();
        assert!(out.contains("晴"), "{out}");
        assert!(out.contains("26℃"), "{out}");
        assert!(out.contains("28℃"), "{out}");
        assert!(out.contains("60%"), "{out}");
        assert!(out.contains("北风3级"), "{out}");
    }
}
