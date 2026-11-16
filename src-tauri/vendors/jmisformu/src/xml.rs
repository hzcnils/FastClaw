//! 共用的 XML 文本处理：转义 + 非法字符过滤。
//!
//! 之前 `docx.rs` / `omml.rs` 各有一份只处理 `& < >` 的 `esc`，导致两个问题：
//!   1. XML 1.0 禁止的 C0 控制字符（`U+0000-08 / 0B / 0C / 0E-1F`）会被原样写进
//!      `<w:t>`，Word 打开直接报「文件已损坏」；这类字符从 PDF 复制、日志片段里很常见。
//!   2. `omml.rs` 把转义结果塞进 `m:val="..."` 属性，而引号没被转义。
//!
//! 这里统一成一份单遍扫描实现：无特殊字符时返回 `Cow::Borrowed`（零分配）。

use std::borrow::Cow;

/// 是否属于 XML 1.0 的 `Char` 产生式（合法字符）。
#[inline]
#[must_use]
pub fn is_xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r')
        || ('\u{20}'..='\u{D7FF}').contains(&c)
        || ('\u{E000}'..='\u{FFFD}').contains(&c)
        || c >= '\u{10000}'
}

/// 转义 XML 文本节点 / 属性值，并**丢弃** XML 1.0 非法字符。
///
/// 同时转义 `"` 与 `'`，因此返回值可以安全地放进属性值。
#[must_use]
pub fn esc(s: &str) -> Cow<'_, str> {
    let needs_work = s
        .chars()
        .any(|c| matches!(c, '&' | '<' | '>' | '"' | '\'') || !is_xml_char(c));
    if !needs_work {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if is_xml_char(c) => out.push(c),
            _ => {} // 静默丢弃非法控制字符
        }
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_borrowed() {
        assert!(matches!(esc("普通文本 abc"), Cow::Borrowed(_)));
    }

    #[test]
    fn escapes_all_five_entities() {
        assert_eq!(esc(r#"a<b>c&d"e'f"#), "a&lt;b&gt;c&amp;d&quot;e&apos;f");
    }

    #[test]
    fn drops_illegal_control_chars() {
        let dirty = "a\u{0}b\u{1}c\u{8}d\u{b}e\u{c}f\u{1f}g";
        assert_eq!(esc(dirty), "abcdefg");
    }

    #[test]
    fn keeps_legal_whitespace() {
        assert_eq!(esc("a\tb\nc\rd"), "a\tb\nc\rd");
    }

    #[test]
    fn keeps_astral_plane() {
        assert_eq!(esc("emoji 😀 数学 𝕆"), "emoji 😀 数学 𝕆");
    }
}
