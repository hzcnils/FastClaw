//! 分层字符串匹配，用于 Edit / BatchEdit。
//!
//! 设计目标：在不破坏文件原始字节（尤其 Windows 的 CRLF 与 Tab 缩进）的前提下，
//! 容忍模型最常见的几类"近似"误差——换行符风格、行尾空白、以及**前导缩进
//! （Tab/空格、空格个数）**不一致——同时保持"精确、唯一、否则报错"的安全语义。
//!
//! 四级匹配（命中即停，逐级放宽）：
//!   Tier 0 Exact      ：原文逐字节精确。
//!   Tier 1 LineEnding ：把两侧归一化成 LF 再匹配（修 CRLF/LF）。
//!   Tier 2 Whitespace ：整行锚定 + 忽略每行**行尾**空白。
//!   Tier 3 Indent     ：整行锚定 + 忽略**全部前导/行尾**空白（Tab↔空格、缩进个数无关）；
//!                       替换时把 new_string 重新缩进到文件实际的基准缩进，避免破坏缩进。
//!
//! 所有命中都映射回原文字节区间，替换在原文上切片拼接，原文其余字节原样保留。

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchTier {
    Exact,
    LineEnding,
    Whitespace,
    Indent,
}

#[derive(Debug)]
pub enum MatchOutcome {
    Unique {
        start: usize,
        end: usize,
        tier: MatchTier,
    },
    Ambiguous {
        count: usize,
        tier: MatchTier,
    },
    NotFound,
}

/// 编辑结果：已构建好的整文件新内容，或歧义/未命中。
#[derive(Debug)]
pub enum EditOutcome {
    Ready {
        new_content: String,
        tier: MatchTier,
    },
    Ambiguous {
        count: usize,
        tier: MatchTier,
    },
    NotFound,
}

fn normalize_lf(s: &str) -> (String, Vec<usize>) {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut map: Vec<usize> = Vec::with_capacity(b.len() + 1);
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\r' {
            map.push(i);
            out.push(b'\n');
            i += if i + 1 < b.len() && b[i + 1] == b'\n' {
                2
            } else {
                1
            };
        } else {
            map.push(i);
            out.push(b[i]);
            i += 1;
        }
    }
    map.push(b.len());
    (
        String::from_utf8(out).expect("normalize_lf preserves utf-8"),
        map,
    )
}

fn count_find(hay: &str, needle: &str) -> (usize, Option<usize>) {
    if needle.is_empty() {
        return (0, None);
    }
    let mut count = 0usize;
    let mut first = None;
    let mut start = 0usize;
    while let Some(rel) = hay[start..].find(needle) {
        let abs = start + rel;
        if first.is_none() {
            first = Some(abs);
        }
        count += 1;
        start = abs + needle.len();
    }
    (count, first)
}

/// 每行 (起始字节偏移, 行内容字节长度)，行内容不含末尾 `\n`。
fn line_spans(nh: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut off = 0usize;
    for line in nh.split('\n') {
        spans.push((off, line.len()));
        off += line.len() + 1;
    }
    spans
}

/// 取前导空白（仅空格/Tab）。
fn leading_ws(s: &str) -> &str {
    let n = s.bytes().take_while(|b| *b == b' ' || *b == b'\t').count();
    &s[..n]
}

pub fn dominant_eol(s: &str) -> &'static str {
    if s.contains("\r\n") {
        "\r\n"
    } else if s.contains('\r') {
        "\r"
    } else {
        "\n"
    }
}

fn convert_eol(s: &str, eol: &str) -> String {
    s.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', eol)
}

pub fn splice(content: &str, start: usize, end: usize, new_string: &str, eol: &str) -> String {
    let new_eol = convert_eol(new_string, eol);
    let mut out = String::with_capacity(content.len() + new_eol.len());
    out.push_str(&content[..start]);
    out.push_str(&new_eol);
    out.push_str(&content[end..]);
    out
}

/// Tier 0–2 定位（前导缩进**敏感**）。
pub fn locate(content: &str, old: &str) -> MatchOutcome {
    let (c0, f0) = count_find(content, old);
    if c0 == 1 {
        let s = f0.unwrap();
        return MatchOutcome::Unique {
            start: s,
            end: s + old.len(),
            tier: MatchTier::Exact,
        };
    }
    if c0 > 1 {
        return MatchOutcome::Ambiguous {
            count: c0,
            tier: MatchTier::Exact,
        };
    }

    let (nh, map) = normalize_lf(content);
    let (nn, _) = normalize_lf(old);
    let (c1, f1) = count_find(&nh, &nn);
    if c1 == 1 {
        let p = f1.unwrap();
        return MatchOutcome::Unique {
            start: map[p],
            end: map[p + nn.len()],
            tier: MatchTier::LineEnding,
        };
    }
    if c1 > 1 {
        return MatchOutcome::Ambiguous {
            count: c1,
            tier: MatchTier::LineEnding,
        };
    }

    let hay = line_spans(&nh);
    let needle_lines: Vec<&str> = nn.split('\n').collect();
    let trimmed: Vec<&str> = needle_lines.iter().map(|l| l.trim_end()).collect();
    let n = needle_lines.len();
    let mut hits: Vec<(usize, usize)> = Vec::new();
    if n > 0 && hay.len() >= n {
        for i in 0..=(hay.len() - n) {
            let all = (0..n).all(|k| {
                let (so, len) = hay[i + k];
                nh[so..so + len].trim_end() == trimmed[k]
            });
            if all {
                let (lso, llen) = hay[i + n - 1];
                hits.push((map[hay[i].0], map[lso + llen]));
            }
        }
    }
    match hits.len() {
        1 => MatchOutcome::Unique {
            start: hits[0].0,
            end: hits[0].1,
            tier: MatchTier::Whitespace,
        },
        x if x > 1 => MatchOutcome::Ambiguous {
            count: x,
            tier: MatchTier::Whitespace,
        },
        _ => MatchOutcome::NotFound,
    }
}

/// Tier 3：忽略全部前导/行尾空白的整行块匹配。
/// 返回每个命中的 (原文 start, 原文 end, 文件首个非空匹配行的前导缩进)。
fn locate_indent_agnostic(content: &str, old: &str) -> Vec<(usize, usize, String)> {
    let (nh, map) = normalize_lf(content);
    let (nn, _) = normalize_lf(old);
    let hay = line_spans(&nh);
    let needle_lines: Vec<&str> = nn.split('\n').collect();
    let trimmed: Vec<&str> = needle_lines.iter().map(|l| l.trim()).collect();
    let n = needle_lines.len();
    let mut hits = Vec::new();
    if n == 0 || hay.len() < n {
        return hits;
    }
    for i in 0..=(hay.len() - n) {
        let all = (0..n).all(|k| {
            let (so, len) = hay[i + k];
            nh[so..so + len].trim() == trimmed[k]
        });
        if all {
            // 文件侧首个非空匹配行的前导缩进，作为重缩进基准。
            let mut file_base = String::new();
            for k in 0..n {
                let (so, len) = hay[i + k];
                let txt = &nh[so..so + len];
                if !txt.trim().is_empty() {
                    file_base = leading_ws(txt).to_string();
                    break;
                }
            }
            let (lso, llen) = hay[i + n - 1];
            hits.push((map[hay[i].0], map[lso + llen], file_base));
        }
    }
    hits
}

/// 把 `new` 从 `old` 的基准缩进迁移到文件的 `file_base` 缩进：
/// 每行若以 needle 基准缩进开头，则把该前缀替换为文件基准缩进，保留更深层的相对缩进。
fn reindent(new: &str, old: &str, file_base: &str) -> String {
    let needle_base = old
        .split('\n')
        .find(|l| !l.trim().is_empty())
        .map(leading_ws)
        .unwrap_or("");
    let lines: Vec<String> = new
        .split('\n')
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else if !needle_base.is_empty() && line.starts_with(needle_base) {
                format!("{}{}", file_base, &line[needle_base.len()..])
            } else if needle_base.is_empty() {
                format!("{}{}", file_base, line)
            } else {
                line.to_string()
            }
        })
        .collect();
    lines.join("\n")
}

/// 一站式：定位 + 构建整文件新内容（含 Tier 3 重缩进 + EOL 归一）。
/// Edit / BatchEdit 直接用它，拿到 `Ready{new_content}` 即可写盘。
pub fn locate_edit(content: &str, old: &str, new: &str) -> EditOutcome {
    let eol = dominant_eol(content);
    match locate(content, old) {
        MatchOutcome::Unique { start, end, tier } => EditOutcome::Ready {
            new_content: splice(content, start, end, new, eol),
            tier,
        },
        MatchOutcome::Ambiguous { count, tier } => EditOutcome::Ambiguous { count, tier },
        MatchOutcome::NotFound => {
            // Tier 3：缩进无关匹配 + 重缩进。
            let hits = locate_indent_agnostic(content, old);
            match hits.len() {
                1 => {
                    let (start, end, file_base) = &hits[0];
                    let reindented = reindent(new, old, file_base);
                    EditOutcome::Ready {
                        new_content: splice(content, *start, *end, &reindented, eol),
                        tier: MatchTier::Indent,
                    }
                }
                x if x > 1 => EditOutcome::Ambiguous {
                    count: x,
                    tier: MatchTier::Indent,
                },
                _ => EditOutcome::NotFound,
            }
        }
    }
}

/// replace_all 的容错版本：精确 → 行尾归一化两级回退。
pub fn replace_all_tolerant(content: &str, old: &str, new: &str) -> Option<(String, usize)> {
    let (c0, _) = count_find(content, old);
    if c0 > 0 {
        return Some((content.replace(old, new), c0));
    }
    let eol = dominant_eol(content);
    let (nh, _) = normalize_lf(content);
    let (nn, _) = normalize_lf(old);
    let (c1, _) = count_find(&nh, &nn);
    if c1 > 0 {
        let nnew = convert_eol(new, "\n");
        let replaced = nh.replace(&nn, &nnew);
        return Some((convert_eol(&replaced, eol), c1));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready(content: &str, old: &str, new: &str) -> (String, MatchTier) {
        match locate_edit(content, old, new) {
            EditOutcome::Ready { new_content, tier } => (new_content, tier),
            other => panic!("期望 Ready，实际 {other:?}"),
        }
    }

    #[test]
    fn crlf_lf_mismatch() {
        let file = "<t>\r\n  <a/>\r\n  <b/>\r\n</t>\r\n";
        let (out, tier) = ready(file, "  <a/>\n  <b/>", "  <b/>\n  <a/>");
        assert_eq!(tier, MatchTier::LineEnding);
        assert_eq!(out, "<t>\r\n  <b/>\r\n  <a/>\r\n</t>\r\n");
    }

    #[test]
    fn tab_file_space_needle_reindents_to_tabs() {
        // 文件是 Tab 缩进，模型用 8 空格 —— Tier 0/1/2 全失败，Tier 3 救回并保持 Tab。
        let file = "<template>\r\n\t<view>\r\n\t\t<text>欢迎使用</text>\r\n\t\t<button>登录</button>\r\n\t</view>\r\n</template>\r\n";
        let old = "        <text>欢迎使用</text>\n        <button>登录</button>";
        let new = "        <button>登录</button>\n        <text>欢迎使用</text>";
        let (out, tier) = ready(file, old, new);
        assert_eq!(tier, MatchTier::Indent);
        assert_eq!(
            out,
            "<template>\r\n\t<view>\r\n\t\t<button>登录</button>\r\n\t\t<text>欢迎使用</text>\r\n\t</view>\r\n</template>\r\n"
        );
    }

    #[test]
    fn indent_agnostic_duplicate_is_ambiguous() {
        let file = "\tx()\r\n\tx()\r\n";
        assert!(matches!(
            locate_edit(file, "    x()", "    y()"),
            EditOutcome::Ambiguous {
                tier: MatchTier::Indent,
                ..
            }
        ));
    }

    #[test]
    fn exact_single_line_still_exact() {
        let (_, tier) = ready("let a = 1;\nlet b = 2;\n", "let a = 1;", "let a = 9;");
        assert_eq!(tier, MatchTier::Exact);
    }

    #[test]
    fn truly_absent_is_not_found() {
        assert!(matches!(
            locate_edit("let a = 1;\n", "zzz", "q"),
            EditOutcome::NotFound
        ));
    }
}
