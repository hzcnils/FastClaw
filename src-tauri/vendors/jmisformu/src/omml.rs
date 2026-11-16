//! MathML -> OMML（Office Math Markup Language）。
//!
//! Word 可编辑公式链路的核心。不使用微软的 `MML2OMML.XSL`（那是 XSLT 2.0，
//! Rust 生态唯一成熟的 libxslt 只支持 1.0），而是纯 Rust 做 MathML DOM -> OMML 树映射。
//!
//! ## 覆盖的结构
//! ```text
//! mi mn mo mtext ms       -> m:r / m:t（并按 mathvariant 映射 m:sty / m:scr）
//! mrow                    -> 透传；并在此层做「括号 -> m:d」与「大算符 -> m:nary」识别
//! mfrac                   -> m:f（linethickness=0 -> noBar，即二项式系数）
//! msup msub msubsup       -> m:sSup / m:sSub / m:sSubSup
//! msqrt mroot             -> m:rad
//! munder mover munderover -> m:limLow / m:limUpp / m:nary
//! mtable mtr mtd          -> m:m（矩阵）
//! ```
//!
//! ## 相对旧版的三处关键修正
//! 1. **`m:limLoc` 不再恒为 `undOvr`**：`msub/msup/msubsup`（LaTeX 的 `\int_a^b`，
//!    上下限在符号**右侧**）映射为 `subSup`；`munder/mover/munderover`
//!    （`\sum_{k=1}^n`，上下限在符号**上下**）才是 `undOvr`。
//! 2. **`m:nary` 的 `<m:e>` 不再恒为空**：MathML 里求和体是大算符节点的**兄弟**
//!    （`<munderover>∑…</munderover><mi>k</mi>`），逐节点翻译取不到。现在在 `mrow`
//!    层做前瞻，把大算符之后、到下一个关系符/低优先级运算符之前的节点收进 `<m:e>`。
//! 3. **`mathvariant` 不再丢失**：Word 的数学 run 默认斜体，不映射会导致
//!    `\text{}`、`\mathrm{d}x`、`\sin`、数字统统变斜体。
//!
//! 另外：MathML Core 已移除 `mfenced`，`pulldown-latex` 输出的是
//! `<mrow><mo>(</mo>…<mo>)</mo></mrow>`。旧版只认 `mfenced`，那条分支是死代码，
//! 括号因此全部退化成普通文本、不随内容伸缩。现在在 `mrow` 层识别配对括号并
//! 生成带 `m:grow` 的 `m:d`；`mfenced` 分支保留以兼容其它 MathML 来源。

use crate::error::{JmisformuError, Result};
use crate::xml::esc;
use roxmltree::Node;
use std::fmt::Write as _;

/// OMML 数学命名空间。
pub const OMML_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/math";

/// 把 MathML 字符串转换为 `<m:oMath>...</m:oMath>` 片段（带命名空间声明）。
pub fn mathml_to_omml(mathml: &str) -> Result<String> {
    let inner = mathml_to_omml_inner(mathml)?;
    Ok(format!("<m:oMath xmlns:m=\"{OMML_NS}\">{inner}</m:oMath>"))
}

/// 仅返回 `m:oMath` 内部内容（用于嵌入已声明命名空间的上下文）。
pub fn mathml_to_omml_inner(mathml: &str) -> Result<String> {
    let doc = roxmltree::Document::parse(mathml).map_err(|e| JmisformuError::Mathml(e.to_string()))?;
    let mut inner = String::with_capacity(mathml.len());
    convert_node(doc.root_element(), &mut inner);
    Ok(inner)
}

// ---------------------------------------------------------------------------
// 基础工具
// ---------------------------------------------------------------------------

fn elem_children<'a>(node: Node<'a, 'a>) -> Vec<Node<'a, 'a>> {
    node.children().filter(roxmltree::Node::is_element).collect()
}

/// 收集节点下**所有文本节点**的内容。
///
/// 不能用 `node.text()` —— 它只返回「第一个子节点是文本时的那一段」，
/// 实体拆分或混合内容会丢字。也不能对 `descendants()` 直接 `filter_map(text)`，
/// 那样元素节点和它的文本子节点会各贡献一次，导致重复。
fn node_text(node: Node) -> String {
    node.descendants()
        .filter(roxmltree::Node::is_text)
        .filter_map(|n| n.text())
        .collect()
}

/// 若节点是 `mo`，返回其去空白后的文本。
fn mo_text(node: Node) -> Option<String> {
    if node.is_element() && node.tag_name().name() == "mo" {
        let t = node_text(node);
        let t = t.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// 运行（run）与样式
// ---------------------------------------------------------------------------

/// OMML `m:sty` 取值：`p`=正体 `b`=粗 `i`=斜（Word 默认）`bi`=粗斜。
fn style_of(node: Node) -> &'static str {
    match node.attribute("mathvariant") {
        Some("bold") | Some("bold-script") | Some("bold-fraktur") | Some("bold-sans-serif") => "b",
        Some("italic") => "i",
        Some("bold-italic") | Some("sans-serif-bold-italic") => "bi",
        Some(_) => "p", // normal / double-struck / script / fraktur / monospace ...
        None => match node.tag_name().name() {
            // MathML: 单字符 mi 默认斜体，多字符（sin、lim、max）默认正体
            "mi" if node_text(node).chars().count() == 1 => "i",
            _ => "p", // mn / mo / mtext / ms 一律正体
        },
    }
}

/// OMML `m:scr` 取值（字体族）。
fn script_of(node: Node) -> Option<&'static str> {
    match node.attribute("mathvariant") {
        Some("double-struck") => Some("double-struck"),
        Some("script") | Some("bold-script") => Some("script"),
        Some("fraktur") | Some("bold-fraktur") => Some("fraktur"),
        Some("sans-serif") | Some("bold-sans-serif") | Some("sans-serif-italic")
        | Some("sans-serif-bold-italic") => Some("sans-serif"),
        Some("monospace") => Some("monospace"),
        _ => None,
    }
}

fn emit_run(text: &str, sty: &str, scr: Option<&str>, out: &mut String) {
    if text.is_empty() {
        return;
    }
    let mut rpr = String::new();
    if let Some(s) = scr {
        let _ = write!(rpr, "<m:scr m:val=\"{s}\"/>");
    }
    // "i" 是 Word 数学 run 的默认值，可省略以减小体积
    if sty != "i" {
        let _ = write!(rpr, "<m:sty m:val=\"{sty}\"/>");
    }
    let _ = write!(out, "<m:r>");
    if !rpr.is_empty() {
        let _ = write!(out, "<m:rPr>{rpr}</m:rPr>");
    }
    let _ = write!(out, "<m:t xml:space=\"preserve\">{}</m:t></m:r>", esc(text));
}

/// 纯文本 run（无样式），供降级路径使用。
pub(crate) fn emit_plain_run(text: &str, out: &mut String) {
    emit_run(text, "p", None, out);
}

// ---------------------------------------------------------------------------
// 括号 -> m:d
// ---------------------------------------------------------------------------

const OPEN_FENCES: &[&str] = &["(", "[", "{", "⟨", "⌊", "⌈", "⟦", "〈", "｢"];
const CLOSE_FENCES: &[&str] = &[")", "]", "}", "⟩", "⌋", "⌉", "⟧", "〉", "｣"];
/// 开闭同形的定界符。
const SYMMETRIC_FENCES: &[&str] = &["|", "‖", "‖"];

fn fence_pair(o: &str, c: &str) -> bool {
    if o == c {
        return SYMMETRIC_FENCES.contains(&o);
    }
    OPEN_FENCES
        .iter()
        .position(|x| *x == o)
        .is_some_and(|i| CLOSE_FENCES.get(i).is_some_and(|x| *x == c))
}

/// 判断一串兄弟节点是否整体被一对配对括号包裹（且中间平衡）。
fn whole_seq_fence(ch: &[Node]) -> Option<(String, String)> {
    if ch.len() < 2 {
        return None;
    }
    let open = mo_text(ch[0])?;
    let close = mo_text(ch[ch.len() - 1])?;
    if !fence_pair(&open, &close) {
        return None;
    }

    if open == close {
        // 对称定界符：中间不能再出现同一个字符，否则 |a|+|b| 会被整体吞掉
        let dup = ch[1..ch.len() - 1]
            .iter()
            .any(|n| mo_text(*n).as_deref() == Some(open.as_str()));
        return if dup { None } else { Some((open, close)) };
    }

    // 平衡性检查：深度不能在中途归零，否则 (a+b)(c+d) 会被当成一对括号
    let mut depth = 0i32;
    for (k, n) in ch.iter().enumerate() {
        let Some(t) = mo_text(*n) else { continue };
        if OPEN_FENCES.contains(&t.as_str()) {
            depth += 1;
        } else if CLOSE_FENCES.contains(&t.as_str()) {
            depth -= 1;
            if depth < 0 || (depth == 0 && k + 1 != ch.len()) {
                return None;
            }
        }
    }
    if depth == 0 {
        Some((open, close))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// 大算符 -> m:nary
// ---------------------------------------------------------------------------

struct Nary<'a> {
    chr: String,
    sub: Option<Node<'a, 'a>>,
    sup: Option<Node<'a, 'a>>,
    /// `subSup` = 限在右侧（\int_a^b）；`undOvr` = 限在上下（\sum_{k=1}^n）
    lim_loc: &'static str,
}

/// 判断一个 `mo` 是否为大算符（求和/积分/连乘等）。
fn nary_char(node: Node) -> Option<String> {
    let t = mo_text(node)?;
    let mut it = t.chars();
    let c = it.next()?;
    if it.next().is_some() {
        return None; // 多字符不是大算符
    }
    matches!(
        c,
        '∑' | '∏' | '∐' | '∫' | '∬' | '∭' | '⨌' | '∮' | '∯' | '∰'
            | '⋃' | '⋂' | '⋁' | '⋀' | '⨆' | '⨅' | '⨁' | '⨂' | '⨀'
    )
    .then(|| c.to_string())
}

fn as_nary<'a>(n: Node<'a, 'a>) -> Option<Nary<'a>> {
    if !n.is_element() {
        return None;
    }
    let name = n.tag_name().name();
    let ch = elem_children(n);
    let (sub, sup, lim_loc) = match name {
        // 上下限在符号右侧
        "msubsup" => (ch.get(1).copied(), ch.get(2).copied(), "subSup"),
        "msub" => (ch.get(1).copied(), None, "subSup"),
        "msup" => (None, ch.get(1).copied(), "subSup"),
        // 上下限在符号上下
        "munderover" => (ch.get(1).copied(), ch.get(2).copied(), "undOvr"),
        "munder" => (ch.get(1).copied(), None, "undOvr"),
        "mover" => (None, ch.get(1).copied(), "undOvr"),
        // 裸大算符，无上下限
        "mo" => {
            return nary_char(n).map(|chr| Nary {
                chr,
                sub: None,
                sup: None,
                lim_loc: "subSup",
            })
        }
        _ => return None,
    };
    let chr = nary_char(*ch.first()?)?;
    Some(Nary {
        chr,
        sub,
        sup,
        lim_loc,
    })
}

/// 大算符作用域的右边界：遇到关系符 / 低优先级二元运算符 / 另一个大算符即停。
fn is_nary_boundary(n: Node) -> bool {
    let Some(t) = mo_text(n) else { return false };
    let mut it = t.chars();
    let Some(c) = it.next() else { return false };
    if it.next().is_some() {
        return false;
    }
    matches!(
        c,
        '=' | '≠' | '<' | '>' | '≤' | '≥' | '≈' | '≡' | '∼' | '≅' | '≐' | '≃'
            | '∈' | '∉' | '⊂' | '⊃' | '⊆' | '⊇'
            | '→' | '⇒' | '↔' | '⇔' | '↦' | '⟶' | '⟹'
            | '+' | '−' | '-' | '±' | '∓' | '∧' | '∨'
            | ',' | ';' | ')' | ']' | '}' | '⟩'
    )
}

fn nary_body_end(ch: &[Node], from: usize) -> usize {
    let mut k = from;
    while k < ch.len() {
        // 体至少含一个元素，故 k == from 时不检查边界
        if k > from && (is_nary_boundary(ch[k]) || as_nary(ch[k]).is_some()) {
            return k;
        }
        k += 1;
    }
    ch.len()
}

fn emit_nary(n: &Nary, body: &[Node], out: &mut String) {
    let sub_hide = if n.sub.is_some() { "0" } else { "1" };
    let sup_hide = if n.sup.is_some() { "0" } else { "1" };
    let _ = write!(
        out,
        "<m:nary><m:naryPr><m:chr m:val=\"{}\"/><m:limLoc m:val=\"{}\"/>\
         <m:subHide m:val=\"{}\"/><m:supHide m:val=\"{}\"/></m:naryPr><m:sub>",
        esc(&n.chr),
        n.lim_loc,
        sub_hide,
        sup_hide
    );
    if let Some(s) = n.sub {
        convert_node(s, out);
    }
    out.push_str("</m:sub><m:sup>");
    if let Some(s) = n.sup {
        convert_node(s, out);
    }
    out.push_str("</m:sup><m:e>");
    convert_seq(body, out);
    out.push_str("</m:e></m:nary>");
}

// ---------------------------------------------------------------------------
// 主转换
// ---------------------------------------------------------------------------

/// 转换一串兄弟元素节点：先看是否整体被括号包裹，再逐个处理（含大算符前瞻）。
fn convert_seq(ch: &[Node], out: &mut String) {
    if let Some((open, close)) = whole_seq_fence(ch) {
        let _ = write!(
            out,
            "<m:d><m:dPr><m:begChr m:val=\"{}\"/><m:endChr m:val=\"{}\"/>\
             <m:grow m:val=\"1\"/><m:shp m:val=\"match\"/></m:dPr><m:e>",
            esc(&open),
            esc(&close)
        );
        convert_seq(&ch[1..ch.len() - 1], out);
        out.push_str("</m:e></m:d>");
        return;
    }

    let mut i = 0;
    while i < ch.len() {
        if let Some(nary) = as_nary(ch[i]) {
            let end = nary_body_end(ch, i + 1);
            emit_nary(&nary, &ch[i + 1..end], out);
            i = end;
        } else {
            convert_node(ch[i], out);
            i += 1;
        }
    }
}

fn convert_children(node: Node, out: &mut String) {
    convert_seq(&elem_children(node), out);
}

/// 把单个子节点作为一个 OMML 参数槽的内容输出。
fn push_child(ch: &[Node], idx: usize, out: &mut String) {
    if let Some(n) = ch.get(idx) {
        convert_node(*n, out);
    }
}

fn convert_node(node: Node, out: &mut String) {
    if !node.is_element() {
        return;
    }

    match node.tag_name().name() {
        // 容器：透传
        "math" | "mrow" | "semantics" | "mstyle" | "mpadded" | "menclose" | "merror" => {
            convert_children(node, out);
        }

        // 语义标注里存的是原始 LaTeX，绝不能当文本输出
        "annotation" | "annotation-xml" | "mphantom" | "mglyph" => {}

        // 标识符 / 数字 / 运算符 / 文本
        "mi" | "mn" | "mo" | "mtext" | "ms" => {
            let txt = node_text(node);
            emit_run(&txt, style_of(node), script_of(node), out);
        }

        "mspace" => emit_run(" ", "p", None, out),

        // 分式（linethickness="0" 表示二项式系数，无横线）
        "mfrac" => {
            let ch = elem_children(node);
            let no_bar = node
                .attribute("linethickness")
                .is_some_and(|t| t == "0" || t.starts_with("0p") || t == "0px");
            let ty = if no_bar { "noBar" } else { "bar" };
            let _ = write!(out, "<m:f><m:fPr><m:type m:val=\"{ty}\"/></m:fPr><m:num>");
            push_child(&ch, 0, out);
            out.push_str("</m:num><m:den>");
            push_child(&ch, 1, out);
            out.push_str("</m:den></m:f>");
        }

        // 上标 / 下标 / 上下标（大算符已在 convert_seq 里被拦截）
        "msup" => {
            let ch = elem_children(node);
            out.push_str("<m:sSup><m:e>");
            push_child(&ch, 0, out);
            out.push_str("</m:e><m:sup>");
            push_child(&ch, 1, out);
            out.push_str("</m:sup></m:sSup>");
        }
        "msub" => {
            let ch = elem_children(node);
            out.push_str("<m:sSub><m:e>");
            push_child(&ch, 0, out);
            out.push_str("</m:e><m:sub>");
            push_child(&ch, 1, out);
            out.push_str("</m:sub></m:sSub>");
        }
        "msubsup" => {
            let ch = elem_children(node);
            out.push_str("<m:sSubSup><m:e>");
            push_child(&ch, 0, out);
            out.push_str("</m:e><m:sub>");
            push_child(&ch, 1, out);
            out.push_str("</m:sub><m:sup>");
            push_child(&ch, 2, out);
            out.push_str("</m:sup></m:sSubSup>");
        }

        // 平方根 / n 次根
        "msqrt" => {
            out.push_str("<m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/><m:e>");
            convert_children(node, out);
            out.push_str("</m:e></m:rad>");
        }
        "mroot" => {
            let ch = elem_children(node);
            out.push_str("<m:rad><m:radPr><m:degHide m:val=\"0\"/></m:radPr><m:deg>");
            push_child(&ch, 1, out); // 次数
            out.push_str("</m:deg><m:e>");
            push_child(&ch, 0, out); // 被开方
            out.push_str("</m:e></m:rad>");
        }

        // 上下方标注（大算符已在 convert_seq 里被拦截，这里只剩 lim / overline 之类）
        "munder" => {
            let ch = elem_children(node);
            out.push_str("<m:limLow><m:limLowPr/><m:e>");
            push_child(&ch, 0, out);
            out.push_str("</m:e><m:lim>");
            push_child(&ch, 1, out);
            out.push_str("</m:lim></m:limLow>");
        }
        "mover" => {
            let ch = elem_children(node);
            out.push_str("<m:limUpp><m:limUppPr/><m:e>");
            push_child(&ch, 0, out);
            out.push_str("</m:e><m:lim>");
            push_child(&ch, 1, out);
            out.push_str("</m:lim></m:limUpp>");
        }
        "munderover" => {
            let ch = elem_children(node);
            out.push_str("<m:limUpp><m:limUppPr/><m:e><m:limLow><m:limLowPr/><m:e>");
            push_child(&ch, 0, out);
            out.push_str("</m:e><m:lim>");
            push_child(&ch, 1, out);
            out.push_str("</m:lim></m:limLow></m:e><m:lim>");
            push_child(&ch, 2, out);
            out.push_str("</m:lim></m:limUpp>");
        }

        // 括号（MathML Core 已不产出，保留以兼容其它来源）
        "mfenced" => {
            let open = node.attribute("open").unwrap_or("(");
            let close = node.attribute("close").unwrap_or(")");
            let _ = write!(
                out,
                "<m:d><m:dPr><m:begChr m:val=\"{}\"/><m:endChr m:val=\"{}\"/>\
                 <m:grow m:val=\"1\"/></m:dPr><m:e>",
                esc(open),
                esc(close)
            );
            convert_children(node, out);
            out.push_str("</m:e></m:d>");
        }

        // 矩阵 / 表格
        "mtable" => {
            out.push_str("<m:m><m:mPr/>");
            for row in elem_children(node) {
                if row.tag_name().name() != "mtr" && row.tag_name().name() != "mlabeledtr" {
                    continue;
                }
                out.push_str("<m:mr>");
                for cell in elem_children(row) {
                    if cell.tag_name().name() == "mtd" {
                        out.push_str("<m:e>");
                        convert_children(cell, out);
                        out.push_str("</m:e>");
                    }
                }
                out.push_str("</m:mr>");
            }
            out.push_str("</m:m>");
        }

        // 未知元素：透传子节点，保证不丢内容
        _ => convert_children(node, out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::latex::{latex_to_mathml, DisplayMode};

    fn omml(latex: &str) -> String {
        let mml = latex_to_mathml(latex, DisplayMode::Block).unwrap();
        mathml_to_omml(&mml).unwrap()
    }

    fn wellformed(s: &str) {
        roxmltree::Document::parse(s).unwrap_or_else(|e| panic!("非法 XML: {e}\n{s}"));
    }

    #[test]
    fn frac_to_mf() {
        let o = omml(r"\frac{n(n+1)}{2}");
        assert!(o.contains("<m:f>") && o.contains("<m:num>") && o.contains("<m:den>"));
        wellformed(&o);
    }

    #[test]
    fn pow_to_ssup() {
        assert!(omml(r"E = mc^2").contains("<m:sSup>"));
    }

    #[test]
    fn sqrt_to_rad() {
        assert!(omml(r"\sqrt{a^2+b^2}").contains("<m:rad>"));
    }

    // ---- 回归 F1：limLoc 必须区分 subSup / undOvr ----

    #[test]
    fn integral_uses_side_limits() {
        let o = omml(r"\int_0^1 x \, dx");
        assert!(o.contains("<m:nary>"), "积分应映射为 nary: {o}");
        assert!(
            o.contains(r#"<m:limLoc m:val="subSup"/>"#),
            "\\int_a^b 的上下限应在右侧(subSup)，而不是 undOvr: {o}"
        );
    }

    #[test]
    fn sum_uses_under_over_limits() {
        let o = omml(r"\sum_{k=1}^n k");
        assert!(o.contains("<m:nary>"), "求和应映射为 nary: {o}");
        assert!(
            o.contains(r#"<m:limLoc m:val="undOvr"/>"#),
            "\\sum_{{k=1}}^n 的上下限应在上下(undOvr): {o}"
        );
    }

    // ---- 回归 F2：nary 的 m:e 必须装进求和体 ----

    #[test]
    fn nary_body_is_not_empty() {
        let o = omml(r"\sum_{k=1}^n k");
        assert!(!o.contains("<m:e></m:e>"), "求和体不应为空: {o}");
        assert!(!o.contains("<m:e/>"), "求和体不应为空: {o}");
    }

    #[test]
    fn nary_body_stops_at_equals() {
        // \sum_{i=1}^n i = \frac{...} -> 体只应是 i，等号及右边留在 nary 外
        let o = omml(r"\sum_{i=1}^n i = \frac{n(n+1)}{2}");
        let nary_end = o.find("</m:nary>").expect("应有 nary");
        let after = &o[nary_end..];
        assert!(after.contains("<m:f>"), "分式应在 nary 之外: {o}");
        wellformed(&o);
    }

    // ---- 回归 F3：mathvariant / 默认正体 ----

    #[test]
    fn text_is_upright_not_italic() {
        let o = omml(r"\text{当且仅当}");
        assert!(
            o.contains(r#"<m:sty m:val="p"/>"#),
            "\\text{{}} 内容必须是正体，否则 Word 会排成斜体: {o}"
        );
    }

    #[test]
    fn mathrm_is_upright() {
        let o = omml(r"\mathrm{d}x");
        assert!(o.contains(r#"<m:sty m:val="p"/>"#), "\\mathrm 应为正体: {o}");
    }

    #[test]
    fn numbers_are_upright() {
        let o = omml(r"12345");
        assert!(o.contains(r#"<m:sty m:val="p"/>"#), "数字应为正体: {o}");
    }

    #[test]
    fn single_letter_variable_stays_italic() {
        // 斜体是 Word 默认值，故不应显式写出 m:sty
        let o = omml(r"x");
        assert!(!o.contains(r#"<m:sty m:val="p"/>"#), "单字母变量应保持斜体: {o}");
    }

    // ---- 回归 F5：括号应生成可伸缩的 m:d ----

    #[test]
    fn parens_become_stretchy_delimiter() {
        let o = omml(r"\left( \frac{a}{b} \right)");
        assert!(o.contains("<m:d>"), "\\left(..\\right) 应生成 m:d: {o}");
        assert!(o.contains(r#"<m:begChr m:val="("/>"#), "{o}");
        assert!(o.contains(r#"<m:endChr m:val=")"/>"#), "{o}");
    }

    #[test]
    fn unbalanced_parens_are_not_merged() {
        // (a+b)(c+d) 不能被当成一对括号整体包裹
        let o = omml(r"(a+b)(c+d)");
        let count = o.matches("<m:d>").count();
        assert!(count != 1, "两组括号不应被合并成一个 m:d: {o}");
        wellformed(&o);
    }

    // ---- 通用健壮性 ----

    #[test]
    fn output_is_always_wellformed() {
        for f in [
            r"\frac{1}{2}",
            r"\sqrt[3]{x}",
            r"\int_0^\infty e^{-x^2} dx",
            r"\begin{matrix} a & b \\ c & d \end{matrix}",
            r"\lim_{x \to 0} \frac{\sin x}{x}",
            r"\mathbb{R}^n",
            r"a < b > c & d",
            r"\text{含 \& 与 < 的文本}",
            r"\prod_{i=1}^{n} \left( 1 + x_i \right)",
        ] {
            if let Ok(mml) = latex_to_mathml(f, DisplayMode::Block) {
                let o = mathml_to_omml(&mml).unwrap();
                wellformed(&o);
            }
        }
    }

    #[test]
    fn annotation_is_not_leaked_as_text() {
        let mml = r#"<math xmlns="http://www.w3.org/1998/Math/MathML"><semantics>
            <mrow><mi>x</mi></mrow>
            <annotation encoding="application/x-tex">\frac{1}{2}</annotation>
        </semantics></math>"#;
        let o = mathml_to_omml(mml).unwrap();
        assert!(!o.contains("frac"), "annotation 里的原始 LaTeX 不能被输出: {o}");
    }

    #[test]
    fn multi_text_node_content_is_not_lost() {
        let mml = r#"<math xmlns="http://www.w3.org/1998/Math/MathML"><mtext>a&amp;b</mtext></math>"#;
        let o = mathml_to_omml(mml).unwrap();
        assert!(o.contains("a&amp;b"), "被实体拆分的文本不应丢失: {o}");
    }
}
