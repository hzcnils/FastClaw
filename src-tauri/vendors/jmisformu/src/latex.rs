//! LaTeX -> MathML。
//!
//! 使用 `pulldown-latex`（纯 Rust，紧贴 MathML Core 规范）。

use crate::error::{JmisformuError, Result};
use pulldown_latex::{mathml::push_mathml, Parser, RenderConfig, Storage};

/// 公式显示模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    /// 行内（`display="inline"`）
    Inline,
    /// 块级（`display="block"`，独占一行、居中）
    Block,
}

/// 将 LaTeX 数学源码转换为 MathML 字符串。
pub fn latex_to_mathml(latex: &str, mode: DisplayMode) -> Result<String> {
    // pulldown-latex 不认识 amsmath 的 \tag{...}（实测报 "unknown primitive command"），
    // 一旦出现整条公式都会解析失败、退回成裸文本。这里先剥掉 \tag{...}，
    // 让公式主体正常转换；编号由调用方用 extract_tag 拿到后另行排到公式右侧。
    let (clean, _tag) = extract_tag(latex);

    let storage = Storage::new();
    let parser = Parser::new(&clean, &storage);

    let config = RenderConfig {
        display_mode: match mode {
            DisplayMode::Inline => pulldown_latex::config::DisplayMode::Inline,
            DisplayMode::Block => pulldown_latex::config::DisplayMode::Block,
        },
        ..Default::default()
    };

    let mut out = String::new();
    push_mathml(&mut out, parser, config).map_err(|e| JmisformuError::Latex(format!("{e:?}")))?;

    // pulldown-latex 用 <merror> 承载解析错误，内容是给人看的错误说明。
    // 旧版会把这段说明当普通文本一路带进 Word/PDF（用户会看到
    // 「parsing error: alignment not allowed in current environment」出现在正文里）。
    // 这里直接判为失败，让调用方走「原样显示 LaTeX 源码」的降级路径。
    if out.contains("<merror") {
        return Err(JmisformuError::Latex(format!(
            "pulldown-latex 无法解析该公式: {}",
            latex.chars().take(60).collect::<String>()
        )));
    }

    Ok(sanitize_mathml(&out).into_owned())
}

/// 把 `pulldown-latex` 的 MathML 输出修补成**合法 XML**。
///
/// `pulldown-latex` 面向 HTML 嵌入，文本位置上的 `<` `>` `&` 一律不转义，
/// 例如 `a < b` 会输出 `<mo><</mo>`、`\text{a & b}` 会输出 `<mtext>a & b</mtext>`。
/// HTML 解析器容忍这些，但 `roxmltree` 是 XML 解析器，会直接报
/// 「invalid name token」/「malformed entity reference」。
///
/// 旧版没有这一步，于是**任何含 `<` `>` `&` 的公式在 DOCX 侧都静默降级成纯文本**
/// —— `mathml_to_omml` 解析失败后被 `unwrap_or_else(fallback)` 吞掉了。
///
/// 这里做一个最小状态机：区分「标签内 / 属性引号内 / 文本」，
/// 只对文本位置（和属性值内）的裸 `<` `>` `&` 做转义。
/// 实体引用只放行 XML 预定义的五个和数值引用，其余（如 `&alpha;`）转义成字面量，
/// 因为 XML 没有 MathML 的命名实体表，放行会导致「unknown entity」。
fn sanitize_mathml(src: &str) -> std::borrow::Cow<'_, str> {
    #[derive(Clone, Copy, PartialEq)]
    enum St {
        Text,
        Tag,
        Attr(char),
    }

    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len() + 16);
    let mut st = St::Text;
    let mut changed = false;
    let mut i = 0usize;

    while i < src.len() {
        let c = src[i..].chars().next().unwrap();
        let clen = c.len_utf8();
        match st {
            St::Text => match c {
                '<' => {
                    // 只有 `<` 后面紧跟 字母 / `/` / `!` / `?` 才算标签起始
                    let is_tag = bytes
                        .get(i + 1)
                        .is_some_and(|b| b.is_ascii_alphabetic() || matches!(b, b'/' | b'!' | b'?'));
                    if is_tag {
                        st = St::Tag;
                        out.push('<');
                    } else {
                        out.push_str("&lt;");
                        changed = true;
                    }
                }
                '>' => {
                    out.push_str("&gt;");
                    changed = true;
                }
                '&' => {
                    if let Some(len) = valid_entity_len(&src[i..]) {
                        out.push_str(&src[i..i + len]);
                        i += len;
                        continue;
                    }
                    out.push_str("&amp;");
                    changed = true;
                }
                _ => out.push(c),
            },
            St::Tag => match c {
                '"' | '\'' => {
                    st = St::Attr(c);
                    out.push(c);
                }
                '>' => {
                    st = St::Text;
                    out.push('>');
                }
                '&' => {
                    if let Some(len) = valid_entity_len(&src[i..]) {
                        out.push_str(&src[i..i + len]);
                        i += len;
                        continue;
                    }
                    out.push_str("&amp;");
                    changed = true;
                }
                _ => out.push(c),
            },
            St::Attr(q) => {
                if c == q {
                    st = St::Tag;
                    out.push(c);
                } else if c == '<' {
                    out.push_str("&lt;");
                    changed = true;
                } else if c == '&' {
                    if let Some(len) = valid_entity_len(&src[i..]) {
                        out.push_str(&src[i..i + len]);
                        i += len;
                        continue;
                    }
                    out.push_str("&amp;");
                    changed = true;
                } else {
                    out.push(c);
                }
            }
        }
        i += clen;
    }

    if changed {
        std::borrow::Cow::Owned(out)
    } else {
        std::borrow::Cow::Borrowed(src)
    }
}

/// 若 `s` 以一个合法的 XML 实体引用开头，返回其字节长度。
fn valid_entity_len(s: &str) -> Option<usize> {
    debug_assert!(s.starts_with('&'));
    let end = s.get(1..)?.find(';')? + 1;
    if end > 12 {
        return None;
    }
    let body = &s[1..end];
    let ok = matches!(body, "lt" | "gt" | "amp" | "quot" | "apos")
        || body
            .strip_prefix("#x")
            .is_some_and(|h| !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit()))
        || body
            .strip_prefix('#')
            .is_some_and(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()));
    ok.then_some(end + 1)
}

/// 从公式中剥离 `\tag{...}`（amsmath 编号命令），返回（去掉 tag 的 LaTeX, 编号文本）。
///
/// 例：`E=mc^2 \tag{1}` -> (`E=mc^2`, Some("1"))。
/// 同时容忍 `\tag*{...}` 与无花括号的 `\tag2` / `\tag 6`。
///
/// # 实现说明（重要）
///
/// 旧版在 `&[u8]` 上扫描，却用扫出的**字节下标**去切 `&str`，且用 `bytes[i] as char`
/// 把单字节当成 Latin-1 码点判断空白/字母数字。UTF-8 续字节里 `0xA0` 会被判成
/// NBSP（空白）、`0xC0..=0xFF` 会被判成字母 —— 于是切片点会落在多字节字符中间，
/// `\tag你` 之类输入直接 panic（`byte index is not a char boundary`）。
///
/// 现在全部按 `char` 边界推进；无花括号形式只接受 **ASCII** 字母数字，
/// 顺带修掉「`\tag中文说明` 把整段 CJK 当成编号吞掉」的语义问题。
#[must_use]
pub fn extract_tag(latex: &str) -> (String, Option<String>) {
    let mut search_from = 0usize;
    while let Some(rel) = latex[search_from..].find("\\tag") {
        let start = search_from + rel;
        // "\tag" 全 ASCII，start + 4 必然落在 char 边界上。
        let mut i = start + 4;

        // 可选的 '*'
        if latex[i..].starts_with('*') {
            i += 1;
        }
        // 跳过空白：按 char 前进，绝不会停在续字节上
        while let Some(c) = latex[i..].chars().next() {
            if c.is_whitespace() {
                i += c.len_utf8();
            } else {
                break;
            }
        }

        // 形式一：\tag{...}（支持嵌套花括号）
        if latex[i..].starts_with('{') {
            let content_start = i + 1;
            let mut depth = 0i32;
            let mut close_at = None;
            for (off, c) in latex[i..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            close_at = Some(i + off);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(j) = close_at {
                let tag = latex[content_start..j].trim().to_string();
                let clean = format!("{}{}", &latex[..start], &latex[j + 1..]);
                return (clean.trim().to_string(), Some(tag));
            }
        }
        // 形式二：\tag2 / \tag 6 —— 只接受 ASCII 字母数字
        else if latex[i..].starts_with(|c: char| c.is_ascii_alphanumeric()) {
            let j = latex[i..]
                .find(|c: char| !c.is_ascii_alphanumeric())
                .map_or(latex.len(), |off| i + off);
            let tag = latex[i..j].to_string();
            let clean = format!("{}{}", &latex[..start], &latex[j..]);
            return (clean.trim().to_string(), Some(tag));
        }

        // 这个 \tag 不完整/不匹配，继续往后找
        search_from = start + 4;
    }
    (latex.to_string(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frac_renders_mfrac() {
        let mml = latex_to_mathml(r"\frac{n(n+1)}{2}", DisplayMode::Block).unwrap();
        assert!(mml.contains("<mfrac>"), "should contain mfrac: {mml}");
    }

    #[test]
    fn sum_renders_subsup() {
        let mml = latex_to_mathml(r"\sum_{k=1}^n k", DisplayMode::Inline).unwrap();
        assert!(mml.contains("msubsup") || mml.contains("munderover"));
    }

    #[test]
    fn tag_is_stripped_and_extracted() {
        let (clean, tag) = extract_tag(r"E = mc^2 \tag{1}");
        assert_eq!(clean, "E = mc^2");
        assert_eq!(tag.as_deref(), Some("1"));
    }

    #[test]
    fn formula_with_tag_now_parses() {
        let mml = latex_to_mathml(r"\frac{1}{2} \tag{3}", DisplayMode::Block).unwrap();
        assert!(mml.contains("<mfrac>"), "should parse despite \\tag: {mml}");
    }

    #[test]
    fn no_tag_unchanged() {
        let (clean, tag) = extract_tag(r"a+b");
        assert_eq!(clean, "a+b");
        assert!(tag.is_none());
    }

    #[test]
    fn braceless_tag_forms() {
        let (c, t) = extract_tag(r"x \tag2");
        assert_eq!((c.as_str(), t.as_deref()), ("x", Some("2")));
        let (c, t) = extract_tag(r"x \tag 6");
        assert_eq!((c.as_str(), t.as_deref()), ("x", Some("6")));
        let (c, t) = extract_tag(r"x \tag*{3}");
        assert_eq!((c.as_str(), t.as_deref()), ("x", Some("3")));
    }

    // ---- 回归：R1，非 ASCII 触发的 char-boundary panic ----

    #[test]
    fn tag_followed_by_cjk_does_not_panic() {
        for s in [
            r"x \tag你",
            r"x \tag好",
            r"a \tag中文",
            r"\tagé",
            r"x \tag你 y",
            r"x \tag½",
            r"\tag",
            r"x \tag",
            r"x \tag{",
            r"x \tag{未闭合",
        ] {
            let _ = extract_tag(s); // 不 panic 即通过
        }
    }

    #[test]
    fn cjk_is_not_swallowed_as_tag_number() {
        // 非 ASCII 不构成无花括号编号，公式主体必须原样保留
        let (clean, tag) = extract_tag(r"x \tag你");
        assert_eq!(clean, r"x \tag你");
        assert_eq!(tag, None);
    }

    #[test]
    fn braced_tag_with_cjk_still_works() {
        let (clean, tag) = extract_tag(r"E=mc^2 \tag{式一}");
        assert_eq!(clean, "E=mc^2");
        assert_eq!(tag.as_deref(), Some("式一"));
    }

    // ---- 回归：pulldown-latex 输出的裸 < > & 必须被修补成合法 XML ----

    #[test]
    fn mathml_output_is_valid_xml() {
        for f in [r"a < b", r"x > y", r"a \le b", r"\frac{1}{2}", r"\text{甲 < 乙}"] {
            if let Ok(m) = latex_to_mathml(f, DisplayMode::Block) {
                roxmltree::Document::parse(&m)
                    .unwrap_or_else(|e| panic!("MathML 非法 XML ({f}): {e}\n{m}"));
            }
        }
    }

    #[test]
    fn bare_lt_is_escaped() {
        let m = latex_to_mathml(r"a < b", DisplayMode::Inline).unwrap();
        assert!(m.contains("&lt;"), "裸 < 应被转义: {m}");
        roxmltree::Document::parse(&m).unwrap();
    }

    #[test]
    fn sanitizer_keeps_tags_and_attributes() {
        let s = sanitize_mathml(r#"<math display="block"><mo><</mo></math>"#);
        assert_eq!(s, r#"<math display="block"><mo>&lt;</mo></math>"#);
    }

    #[test]
    fn sanitizer_is_borrowed_when_clean() {
        let src = r#"<math><mi>x</mi></math>"#;
        assert!(matches!(sanitize_mathml(src), std::borrow::Cow::Borrowed(_)));
    }

    #[test]
    fn sanitizer_preserves_valid_entities() {
        let s = sanitize_mathml("<mtext>a&amp;b&#8721;c</mtext>");
        assert_eq!(s, "<mtext>a&amp;b&#8721;c</mtext>");
    }

    #[test]
    fn sanitizer_escapes_unknown_entities() {
        let s = sanitize_mathml("<mtext>&alpha;</mtext>");
        assert_eq!(s, "<mtext>&amp;alpha;</mtext>");
    }

    #[test]
    fn merror_becomes_error_not_text() {
        // pulldown-latex 对 `a & b`（环境外的对齐符）会产出 merror
        let r = latex_to_mathml(r"a < b > c & d", DisplayMode::Block);
        assert!(r.is_err(), "含 merror 的输出应判为失败，而不是把错误说明当正文");
    }
}
