//! Word 可编辑公式导出。
//!
//! 直接生成 OOXML 包（`[Content_Types].xml`、`_rels/.rels`、`word/document.xml`、
//! `word/_rels/document.xml.rels`、`word/styles.xml`、`word/numbering.xml`），
//! 对 OMML 的控制最彻底。普通文本走 `<w:r><w:t>`，公式走 `<m:oMath>`
//! （Word 打开后双击即进入公式编辑器）。

use crate::doc_model::{parse_document, Block, Inline};
use crate::error::{JmisformuError, Result, Warning};
use crate::latex::{latex_to_mathml, DisplayMode};
use crate::omml::mathml_to_omml_inner;
use crate::xml::esc;
use std::fmt::Write as _;
use std::io::Write as _;

// --- 页面几何（twips，1 pt = 20 twips）---
const PAGE_W_TWIPS: i32 = 11906; // A4 宽
const PAGE_H_TWIPS: i32 = 16838; // A4 高
const MARGIN_TWIPS: i32 = 1440; // 1 英寸
/// 正文可用宽度。公式编号的右制表位必须落在这里，
/// 旧版硬编码 9600 超出右边距 574 twips，编号会被推到页边距外。
const CONTENT_W_TWIPS: i32 = PAGE_W_TWIPS - 2 * MARGIN_TWIPS;

// --- 关系 ID 分配 ---
const RID_STYLES: u32 = 1;
const RID_NUMBERING: u32 = 2;
/// 超链接从这里开始编号。
const RID_LINK_BASE: u32 = 3;

/// DOCX 导出结果。
pub struct DocxOutput {
    pub bytes: Vec<u8>,
    /// 非致命降级（例如某条公式解析失败，已回退为纯文本）。
    pub warnings: Vec<Warning>,
}

/// 从 Markdown 生成 DOCX 字节。
pub fn markdown_to_docx_bytes(markdown: &str) -> Result<Vec<u8>> {
    Ok(markdown_to_docx(markdown)?.bytes)
}

/// 从 Markdown 生成 DOCX，并返回降级告警。
pub fn markdown_to_docx(markdown: &str) -> Result<DocxOutput> {
    let blocks = parse_document(markdown);
    let mut b = Builder::default();
    b.build_body(&blocks);
    if b.body.is_empty() {
        b.body.push_str("<w:p/>");
    }
    let document_xml = wrap_document(&b.body);
    let bytes = build_docx_zip(&document_xml, &b.links, &b.lists)?;
    Ok(DocxOutput {
        bytes,
        warnings: b.warnings,
    })
}

/// 便捷函数：直接写到磁盘。
pub fn markdown_to_docx_file(markdown: &str, path: impl AsRef<std::path::Path>) -> Result<()> {
    let bytes = markdown_to_docx_bytes(markdown)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 正文构建
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Builder {
    body: String,
    /// 超链接目标，下标 + RID_LINK_BASE 即 rId。
    links: Vec<String>,
    /// 出现过的列表实例：(list_id, ordered)，下标 + 1 即 w:numId。
    /// 每个列表实例一个独立 numId，Word 才会让多个有序列表各自从 1 开始。
    lists: Vec<(u32, bool)>,
    warnings: Vec<Warning>,
}

impl Builder {
    fn link_rid(&mut self, url: &str) -> u32 {
        if let Some(pos) = self.links.iter().position(|u| u == url) {
            return RID_LINK_BASE + pos as u32;
        }
        self.links.push(url.to_string());
        RID_LINK_BASE + self.links.len() as u32 - 1
    }

    fn num_id(&mut self, list_id: u32, ordered: bool) -> u32 {
        if let Some(pos) = self.lists.iter().position(|(id, _)| *id == list_id) {
            return pos as u32 + 1;
        }
        self.lists.push((list_id, ordered));
        self.lists.len() as u32
    }

    fn build_body(&mut self, blocks: &[Block]) {
        for block in blocks {
            match block {
                Block::Heading(level, inlines) => self.heading(*level, inlines),
                Block::Paragraph(inlines) => self.paragraph(inlines),
                Block::DisplayMath(latex) => self.display_math(latex),
                Block::ListItem {
                    ordered,
                    depth,
                    list_id,
                    content,
                    ..
                } => self.list_item(*ordered, *depth, *list_id, content),
                Block::Quote(inlines) => self.quote(inlines),
                Block::CodeBlock(_lang, code) => self.code_block(code),
                Block::Table { header, rows } => self.table(header, rows),
                Block::FootnoteDefinition { label, content } => self.footnote(label, content),
                Block::Rule => self.body.push_str(
                    "<w:p><w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" \
                     w:space=\"1\" w:color=\"AAAAAA\"/></w:pBdr></w:pPr></w:p>",
                ),
            }
        }
    }

    fn heading(&mut self, level: u8, inlines: &[Inline]) {
        // 字号(半磅)：H1 二号(44) / H2 三号(32) / H3 小三(30) / H4 四号(28) / H5 小四(24) / H6(21)
        let sz = match level {
            1 => 44,
            2 => 32,
            3 => 30,
            4 => 28,
            5 => 24,
            _ => 21,
        };
        let (before, after) = match level {
            1 => (360, 200),
            2 => (280, 140),
            _ => (220, 120),
        };
        let runs = self.inline_runs(
            inlines,
            RunStyle {
                bold: true,
                sz: Some(sz),
                heading: true,
                ..Default::default()
            },
        );
        let _ = write!(
            self.body,
            "<w:p><w:pPr><w:keepNext/><w:spacing w:before=\"{before}\" w:after=\"{after}\" \
             w:line=\"360\" w:lineRule=\"auto\"/></w:pPr>{runs}</w:p>"
        );
    }

    fn paragraph(&mut self, inlines: &[Inline]) {
        let runs = self.inline_runs(inlines, RunStyle::default());
        if runs.is_empty() {
            return;
        }
        // 正文：1.5 倍行距、段后 6pt、首行缩进 2 字符。
        let _ = write!(
            self.body,
            "<w:p><w:pPr><w:spacing w:after=\"120\" w:line=\"360\" w:lineRule=\"auto\"/>\
             <w:ind w:firstLineChars=\"200\" w:firstLine=\"480\"/></w:pPr>{runs}</w:p>"
        );
    }

    fn display_math(&mut self, latex: &str) {
        let (_clean, tag) = crate::latex::extract_tag(latex);
        let inner = self.math_omml(latex, DisplayMode::Block);
        match tag {
            // 有编号：公式 + 右制表 + "(N)"，编号靠右。
            // 注意不用 m:oMathPara（块级容器，不能与其它 run 并存），改用行内 m:oMath。
            Some(n) => {
                let label = raw_text_run(&format!("({n})"), RunStyle::default());
                let _ = write!(
                    self.body,
                    "<w:p><w:pPr><w:jc w:val=\"center\"/>\
                     <w:tabs><w:tab w:val=\"right\" w:pos=\"{CONTENT_W_TWIPS}\"/></w:tabs></w:pPr>\
                     {}<w:r><w:tab/></w:r>{label}</w:p>",
                    omath(&inner)
                );
            }
            None => {
                let _ = write!(
                    self.body,
                    "<w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr>\
                     <m:oMathPara><m:oMathParaPr><m:jc m:val=\"center\"/></m:oMathParaPr>{}</m:oMathPara></w:p>",
                    omath(&inner)
                );
            }
        }
    }

    fn list_item(&mut self, ordered: bool, depth: u8, list_id: u32, content: &[Inline]) {
        // 使用真正的 Word 列表（numbering.xml），而不是把 "1. " 当文本写死 ——
        // 旧版每一项都输出 "1. "，有序列表在 Word 里全显示成 1. 1. 1.。
        let num = self.num_id(list_id, ordered);
        let ilvl = depth.min(8);
        let runs = self.inline_runs(content, RunStyle::default());
        let _ = write!(
            self.body,
            "<w:p><w:pPr><w:numPr><w:ilvl w:val=\"{ilvl}\"/><w:numId w:val=\"{num}\"/></w:numPr>\
             <w:spacing w:after=\"40\"/></w:pPr>{runs}</w:p>"
        );
    }

    fn quote(&mut self, inlines: &[Inline]) {
        let runs = self.inline_runs(
            inlines,
            RunStyle {
                italic: true,
                ..Default::default()
            },
        );
        let _ = write!(
            self.body,
            "<w:p><w:pPr><w:ind w:left=\"480\"/><w:spacing w:after=\"100\"/></w:pPr>{runs}</w:p>"
        );
    }

    fn code_block(&mut self, code: &str) {
        for line in code.lines() {
            let run = raw_text_run(
                if line.is_empty() { " " } else { line },
                RunStyle {
                    mono: true,
                    sz: Some(18),
                    ..Default::default()
                },
            );
            let _ = write!(
                self.body,
                "<w:p><w:pPr><w:shd w:val=\"clear\" w:fill=\"F2F2F2\"/>\
                 <w:spacing w:before=\"0\" w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr>{run}</w:p>"
            );
        }
    }

    fn footnote(&mut self, label: &str, content: &[Inline]) {
        let mut runs = raw_text_run(
            &format!("[{label}] "),
            RunStyle {
                sz: Some(18),
                ..Default::default()
            },
        );
        runs.push_str(&self.inline_runs(content, RunStyle::default()));
        let _ = write!(
            self.body,
            "<w:p><w:pPr><w:ind w:left=\"360\"/><w:spacing w:after=\"60\"/></w:pPr>{runs}</w:p>"
        );
    }

    fn table(&mut self, header: &[Vec<Inline>], rows: &[Vec<Vec<Inline>>]) {
        self.body.push_str(
            "<w:tbl><w:tblPr><w:tblStyle w:val=\"TableGrid\"/>\
             <w:tblW w:w=\"0\" w:type=\"auto\"/>\
             <w:tblBorders>\
             <w:top w:val=\"single\" w:sz=\"4\" w:color=\"888888\"/>\
             <w:left w:val=\"single\" w:sz=\"4\" w:color=\"888888\"/>\
             <w:bottom w:val=\"single\" w:sz=\"4\" w:color=\"888888\"/>\
             <w:right w:val=\"single\" w:sz=\"4\" w:color=\"888888\"/>\
             <w:insideH w:val=\"single\" w:sz=\"4\" w:color=\"888888\"/>\
             <w:insideV w:val=\"single\" w:sz=\"4\" w:color=\"888888\"/>\
             </w:tblBorders>\
             <w:tblCellMar>\
             <w:top w:w=\"60\" w:type=\"dxa\"/><w:left w:w=\"90\" w:type=\"dxa\"/>\
             <w:bottom w:w=\"60\" w:type=\"dxa\"/><w:right w:w=\"90\" w:type=\"dxa\"/>\
             </w:tblCellMar></w:tblPr>",
        );
        if !header.is_empty() {
            self.body.push_str("<w:tr>");
            for h in header {
                let cell = self.table_cell(h, true);
                self.body.push_str(&cell);
            }
            self.body.push_str("</w:tr>");
        }
        for row in rows {
            self.body.push_str("<w:tr>");
            for c in row {
                let cell = self.table_cell(c, false);
                self.body.push_str(&cell);
            }
            self.body.push_str("</w:tr>");
        }
        // 表后必须跟一个段落（OOXML 规则）；给点段前间距，避免与正文贴死。
        self.body
            .push_str("</w:tbl><w:p><w:pPr><w:spacing w:before=\"120\" w:after=\"0\"/></w:pPr></w:p>");
    }

    fn table_cell(&mut self, inlines: &[Inline], bold: bool) -> String {
        let runs = self.inline_runs(
            inlines,
            RunStyle {
                bold,
                ..Default::default()
            },
        );
        format!(
            "<w:tc><w:tcPr><w:tcW w:w=\"0\" w:type=\"auto\"/><w:vAlign w:val=\"center\"/></w:tcPr>\
             <w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"0\" w:line=\"240\" \
             w:lineRule=\"auto\"/></w:pPr>{runs}</w:p></w:tc>"
        )
    }

    /// 把一串 Inline 渲染为 w:r / m:oMath / w:hyperlink 序列。
    fn inline_runs(&mut self, inlines: &[Inline], base: RunStyle) -> String {
        let mut s = String::new();
        for inl in inlines {
            match inl {
                Inline::Text(t) => s.push_str(&raw_text_run(t, base)),
                Inline::Strong(t) => s.push_str(&raw_text_run(t, RunStyle { bold: true, ..base })),
                Inline::Emphasis(t) => {
                    s.push_str(&raw_text_run(t, RunStyle { italic: true, ..base }));
                }
                Inline::Code(t) => s.push_str(&raw_text_run(t, RunStyle { mono: true, ..base })),
                Inline::Math(latex) => {
                    let inner = self.math_omml(latex, DisplayMode::Inline);
                    s.push_str(&omath(&inner));
                }
                Inline::Link { text, url } => {
                    // 真正的 Word 超链接，而不是 "文字 <url>" 纯文本
                    let rid = self.link_rid(url);
                    let display = if text.trim().is_empty() { url } else { text };
                    let run = raw_text_run(
                        display,
                        RunStyle {
                            underline: true,
                            color: Some("0563C1"),
                            ..base
                        },
                    );
                    let _ = write!(s, "<w:hyperlink r:id=\"rId{rid}\">{run}</w:hyperlink>");
                }
                Inline::FootnoteRef(label) => {
                    s.push_str(&raw_text_run(
                        &format!("[{label}]"),
                        RunStyle {
                            sz: Some(16),
                            superscript: true,
                            ..base
                        },
                    ));
                }
            }
        }
        s
    }

    /// LaTeX -> OMML 内容；失败时降级为可读文本并记录告警。
    fn math_omml(&mut self, latex: &str, mode: DisplayMode) -> String {
        match latex_to_mathml(latex, mode).and_then(|m| mathml_to_omml_inner(&m)) {
            Ok(inner) => inner,
            Err(e) => {
                self.warnings.push(Warning {
                    source: latex.to_string(),
                    message: format!("公式转换失败，已降级为文本: {e}"),
                });
                let mut out = String::new();
                crate::omml::emit_plain_run(&latex_to_readable_symbols(latex), &mut out);
                out
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Run 级样式
// ---------------------------------------------------------------------------

#[derive(Default, Clone, Copy)]
struct RunStyle {
    bold: bool,
    italic: bool,
    mono: bool,
    underline: bool,
    superscript: bool,
    subscript: bool,
    /// 标题：中文用黑体
    heading: bool,
    color: Option<&'static str>,
    /// 半磅 (half-points)
    sz: Option<u32>,
}

fn run_props(style: RunStyle) -> String {
    let mut p = String::new();
    if style.bold {
        p.push_str("<w:b/>");
    }
    if style.italic {
        p.push_str("<w:i/>");
    }
    if style.underline {
        p.push_str("<w:u w:val=\"single\"/>");
    }
    if style.superscript {
        p.push_str("<w:vertAlign w:val=\"superscript\"/>");
    }
    if style.subscript {
        p.push_str("<w:vertAlign w:val=\"subscript\"/>");
    }
    if style.mono {
        p.push_str("<w:rFonts w:ascii=\"Consolas\" w:hAnsi=\"Consolas\" w:eastAsia=\"宋体\"/>");
    } else if style.heading {
        p.push_str("<w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\" w:eastAsia=\"黑体\"/>");
    } else {
        p.push_str("<w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\" w:eastAsia=\"宋体\"/>");
    }
    if let Some(color) = style.color {
        let _ = write!(p, "<w:color w:val=\"{color}\"/>");
    }
    if let Some(sz) = style.sz {
        let _ = write!(p, "<w:sz w:val=\"{sz}\"/><w:szCs w:val=\"{sz}\"/>");
    }
    if p.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{p}</w:rPr>")
    }
}

/// 输出一个文本 run。
///
/// 注意：这里**不再**对正文做 `^x^` / `~x~` 的隐式上下标转换。
/// 旧版对每一段普通文本都跑那套字符级魔法，会把 `~/data/~/tmp` 的路径、
/// GFM 删除线 `~~文字~~` 等正常内容吃成上下标，且完全没有文档说明。
/// 上下标应由行内公式或 GFM 扩展表达。
fn raw_text_run(t: &str, style: RunStyle) -> String {
    if t.is_empty() {
        return String::new();
    }
    format!(
        "<w:r>{}<w:t xml:space=\"preserve\">{}</w:t></w:r>",
        run_props(style),
        esc(t)
    )
}

/// 行内/块级公式包装。
///
/// 不再重复声明 `xmlns:m` —— 根节点 `<w:document>` 已经声明过了，
/// 每条公式重复一遍等于白白多写 63 字节。
fn omath(inner: &str) -> String {
    format!("<m:oMath>{inner}</m:oMath>")
}

/// 公式转换失败时的降级：把常见 LaTeX 命令替换成对应 Unicode 符号。
///
/// 旧版用 ~90 次连续 `String::replace`（每次全量分配一个新串）+
/// `while s.contains("  ")` 的 O(n²) 空白折叠。这里改成单遍扫描 + 查表。
fn latex_to_readable_symbols(latex: &str) -> String {
    let mut out = String::with_capacity(latex.len());
    let chars: Vec<char> = latex.chars().collect();
    let n = chars.len();
    let mut i = 0;

    while i < n {
        let c = chars[i];
        if c == '{' || c == '}' {
            i += 1;
            continue;
        }
        if c != '\\' {
            out.push(c);
            i += 1;
            continue;
        }
        // 读取命令名
        let start = i + 1;
        let mut j = start;
        while j < n && chars[j].is_ascii_alphabetic() {
            j += 1;
        }
        if j == start {
            // `\` 后不是字母：`\\`、`\ `、`\,` 之类
            if j < n {
                if chars[j] != '\\' {
                    out.push(' ');
                }
                i = j + 1;
            } else {
                i = j;
            }
            continue;
        }
        let cmd: String = chars[start..j].iter().collect();

        // \mathbb{X} 等带单参数的字体命令：优先整体处理
        if cmd == "mathbb" {
            let arg = braced_arg(&chars, j);
            if let Some((letter, next)) = arg {
                out.push_str(blackboard(&letter).unwrap_or(&letter));
                i = next;
                continue;
            }
        }
        out.push_str(symbol(&cmd).unwrap_or(""));
        i = j;
    }

    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 读取紧跟在 `at` 处的 `{...}` 实参，返回（内容, 下一个下标）。
fn braced_arg(chars: &[char], at: usize) -> Option<(String, usize)> {
    if chars.get(at) != Some(&'{') {
        return None;
    }
    let mut depth = 0i32;
    for (off, c) in chars[at..].iter().enumerate() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((chars[at + 1..at + off].iter().collect(), at + off + 1));
                }
            }
            _ => {}
        }
    }
    None
}

fn blackboard(s: &str) -> Option<&'static str> {
    Some(match s {
        "N" => "ℕ",
        "Z" => "ℤ",
        "Q" => "ℚ",
        "R" => "ℝ",
        "C" => "ℂ",
        _ => return None,
    })
}

fn symbol(cmd: &str) -> Option<&'static str> {
    Some(match cmd {
        "Leftrightarrow" => "⇔",
        "leftrightarrow" => "↔",
        "Rightarrow" => "⇒",
        "Leftarrow" => "⇐",
        "rightarrow" | "to" => "→",
        "leftarrow" | "gets" => "←",
        "uparrow" => "↑",
        "downarrow" => "↓",
        "nearrow" => "↗",
        "searrow" => "↘",
        "mapsto" => "↦",
        "notin" => "∉",
        "subseteq" => "⊆",
        "supseteq" => "⊇",
        "subset" => "⊂",
        "supset" => "⊃",
        "emptyset" | "varnothing" => "∅",
        "forall" => "∀",
        "exists" => "∃",
        "neg" | "lnot" => "¬",
        "land" | "wedge" => "∧",
        "lor" | "vee" => "∨",
        "top" => "⊤",
        "bot" => "⊥",
        "cup" => "∪",
        "cap" => "∩",
        "in" => "∈",
        "infty" => "∞",
        "partial" => "∂",
        "nabla" => "∇",
        "pm" => "±",
        "mp" => "∓",
        "times" => "×",
        "div" => "÷",
        "cdot" => "·",
        "cdots" => "⋯",
        "ldots" | "dots" => "…",
        "leq" | "le" => "≤",
        "geq" | "ge" => "≥",
        "neq" | "ne" => "≠",
        "approx" => "≈",
        "equiv" => "≡",
        "sim" => "∼",
        "propto" => "∝",
        "sum" => "∑",
        "prod" => "∏",
        "int" => "∫",
        "iint" => "∬",
        "oint" => "∮",
        "sqrt" => "√",
        "Omega" => "Ω",
        "Gamma" => "Γ",
        "Delta" => "Δ",
        "Theta" => "Θ",
        "Lambda" => "Λ",
        "Xi" => "Ξ",
        "Pi" => "Π",
        "Sigma" => "Σ",
        "Upsilon" => "Υ",
        "Phi" => "Φ",
        "Psi" => "Ψ",
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "epsilon" | "varepsilon" => "ϵ",
        "zeta" => "ζ",
        "eta" => "η",
        "theta" => "θ",
        "iota" => "ι",
        "kappa" => "κ",
        "lambda" => "λ",
        "mu" => "μ",
        "nu" => "ν",
        "xi" => "ξ",
        "pi" => "π",
        "rho" => "ρ",
        "sigma" => "σ",
        "tau" => "τ",
        "upsilon" => "υ",
        "phi" | "varphi" => "φ",
        "chi" => "χ",
        "psi" => "ψ",
        "omega" => "ω",
        // 结构性命令：丢弃命令名，保留其参数（参数由主循环继续处理）
        "frac" | "text" | "mathrm" | "mathbf" | "mathit" | "left" | "right" | "begin" | "end"
        | "quad" | "qquad" | "displaystyle" | "limits" | "nolimits" => "",
        _ => return None,
    })
}

fn wrap_document(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document
    xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
    xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"
    xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>{body}<w:sectPr><w:pgSz w:w="{PAGE_W_TWIPS}" w:h="{PAGE_H_TWIPS}"/><w:pgMar w:top="{MARGIN_TWIPS}" w:right="{MARGIN_TWIPS}" w:bottom="{MARGIN_TWIPS}" w:left="{MARGIN_TWIPS}" w:header="851" w:footer="992" w:gutter="0"/></w:sectPr></w:body>
</w:document>"#
    )
}

// ---------------------------------------------------------------------------
// numbering.xml
// ---------------------------------------------------------------------------

/// 生成 numbering.xml：两套抽象编号（0=项目符号，1=十进制），
/// 每个列表实例一个 `w:num`，从而各自独立计数。
fn build_numbering_xml(lists: &[(u32, bool)]) -> String {
    let mut s = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
    );

    for (abstract_id, ordered) in [(0u32, false), (1u32, true)] {
        let _ = write!(
            s,
            r#"<w:abstractNum w:abstractNumId="{abstract_id}"><w:multiLevelType w:val="hybridMultilevel"/>"#
        );
        for ilvl in 0..9u32 {
            let indent = 420 * (ilvl + 1);
            if ordered {
                let _ = write!(
                    s,
                    r#"<w:lvl w:ilvl="{ilvl}"><w:start w:val="1"/><w:numFmt w:val="decimal"/>"#
                );
                let _ = write!(s, r#"<w:lvlText w:val="%{}."/>"#, ilvl + 1);
            } else {
                let bullet = match ilvl % 3 {
                    0 => "•",
                    1 => "◦",
                    _ => "▪",
                };
                let _ = write!(
                    s,
                    r#"<w:lvl w:ilvl="{ilvl}"><w:start w:val="1"/><w:numFmt w:val="bullet"/>"#
                );
                let _ = write!(s, r#"<w:lvlText w:val="{bullet}"/>"#);
            }
            let _ = write!(
                s,
                r#"<w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{indent}" w:hanging="420"/></w:pPr></w:lvl>"#
            );
        }
        s.push_str("</w:abstractNum>");
    }

    for (idx, (_, ordered)) in lists.iter().enumerate() {
        let num_id = idx + 1;
        let abstract_id = u32::from(*ordered);
        let _ = write!(
            s,
            r#"<w:num w:numId="{num_id}"><w:abstractNumId w:val="{abstract_id}"/></w:num>"#
        );
    }
    s.push_str("</w:numbering>");
    s
}

fn build_document_rels(links: &[String]) -> String {
    let mut s = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    let _ = write!(
        s,
        r#"<Relationship Id="rId{RID_STYLES}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#
    );
    let _ = write!(
        s,
        r#"<Relationship Id="rId{RID_NUMBERING}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>"#
    );
    for (i, url) in links.iter().enumerate() {
        let rid = RID_LINK_BASE + i as u32;
        let _ = write!(
            s,
            r#"<Relationship Id="rId{rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="{}" TargetMode="External"/>"#,
            esc(url)
        );
    }
    s.push_str("</Relationships>");
    s
}

const CONTENT_TYPES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
  <Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>
</Types>"#;

const ROOT_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;

/// 文档默认样式：西文 Calibri / 中文宋体，小四(24 半磅)，1.5 倍行距。
const STYLES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:docDefaults>
    <w:rPrDefault>
      <w:rPr>
        <w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:eastAsia="宋体" w:cs="Times New Roman"/>
        <w:sz w:val="24"/>
        <w:szCs w:val="24"/>
        <w:lang w:val="en-US" w:eastAsia="zh-CN"/>
      </w:rPr>
    </w:rPrDefault>
    <w:pPrDefault>
      <w:pPr>
        <w:spacing w:after="0" w:line="360" w:lineRule="auto"/>
      </w:pPr>
    </w:pPrDefault>
  </w:docDefaults>
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal">
    <w:name w:val="Normal"/>
    <w:qFormat/>
  </w:style>
</w:styles>"#;

fn build_docx_zip(document_xml: &str, links: &[String], lists: &[(u32, bool)]) -> Result<Vec<u8>> {
    let mut zip = ZipWriter::new();
    zip.add("[Content_Types].xml", CONTENT_TYPES_XML)?;
    zip.add("_rels/.rels", ROOT_RELS_XML)?;
    zip.add("word/document.xml", document_xml)?;
    zip.add("word/_rels/document.xml.rels", &build_document_rels(links))?;
    zip.add("word/styles.xml", STYLES_XML)?;
    zip.add("word/numbering.xml", &build_numbering_xml(lists))?;
    zip.finish().map_err(|e| JmisformuError::Docx(e.to_string()))
}

// ---------------------------------------------------------------------------
// ZIP 写入器
// ---------------------------------------------------------------------------
//
// 相对旧版的修正：
//   * 用 deflate 压缩（旧版是 STORE，document.xml 这种高重复 XML 压缩比通常 10-20x）；
//   * 写入合法的 DOS 时间戳（旧版全 0 = 1980 年 0 月 0 日，是非法日期，
//     部分严格的解压实现会报警）；
//   * 设置 UTF-8 文件名标志位（bit 11）。

/// 1980-01-01：year=0, month=1, day=1 -> (0<<9)|(1<<5)|1
const DOS_DATE: u16 = 0x0021;
const DOS_TIME: u16 = 0;
/// bit 11 = 文件名/注释使用 UTF-8
const FLAG_UTF8: u16 = 0x0800;

struct ZipWriter {
    buf: Vec<u8>,
    entries: Vec<CentralEntry>,
}

struct CentralEntry {
    name: String,
    crc: u32,
    method: u16,
    comp_size: u32,
    size: u32,
    offset: u32,
}

impl ZipWriter {
    fn new() -> Self {
        Self {
            buf: Vec::with_capacity(64 * 1024),
            entries: Vec::new(),
        }
    }

    fn add(&mut self, name: &str, content: &str) -> Result<()> {
        let data = content.as_bytes();
        if data.len() > u32::MAX as usize {
            return Err(JmisformuError::Docx(format!(
                "{name} 超过 4 GiB，本实现不支持 ZIP64"
            )));
        }
        let crc = crc32(data);
        let offset = u32::try_from(self.buf.len())
            .map_err(|_| JmisformuError::Docx("DOCX 超过 4 GiB".into()))?;

        // 压缩不划算时退回 STORE
        let deflated = deflate(data)?;
        let (method, payload): (u16, &[u8]) = if deflated.len() < data.len() {
            (8, &deflated)
        } else {
            (0, data)
        };

        let name_bytes = name.as_bytes();
        self.buf.write_all(&0x0403_4b50u32.to_le_bytes())?; // 本地头签名
        self.buf.write_all(&20u16.to_le_bytes())?; // version needed
        self.buf.write_all(&FLAG_UTF8.to_le_bytes())?;
        self.buf.write_all(&method.to_le_bytes())?;
        self.buf.write_all(&DOS_TIME.to_le_bytes())?;
        self.buf.write_all(&DOS_DATE.to_le_bytes())?;
        self.buf.write_all(&crc.to_le_bytes())?;
        self.buf.write_all(&(payload.len() as u32).to_le_bytes())?;
        self.buf.write_all(&(data.len() as u32).to_le_bytes())?;
        self.buf.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
        self.buf.write_all(&0u16.to_le_bytes())?; // extra len
        self.buf.write_all(name_bytes)?;
        self.buf.write_all(payload)?;

        self.entries.push(CentralEntry {
            name: name.to_string(),
            crc,
            method,
            comp_size: payload.len() as u32,
            size: data.len() as u32,
            offset,
        });
        Ok(())
    }

    fn finish(mut self) -> std::result::Result<Vec<u8>, std::io::Error> {
        let cd_offset = self.buf.len() as u32;
        for e in &self.entries {
            let name_bytes = e.name.as_bytes();
            self.buf.write_all(&0x0201_4b50u32.to_le_bytes())?; // 中央目录签名
            self.buf.write_all(&20u16.to_le_bytes())?; // version made by
            self.buf.write_all(&20u16.to_le_bytes())?; // version needed
            self.buf.write_all(&FLAG_UTF8.to_le_bytes())?;
            self.buf.write_all(&e.method.to_le_bytes())?;
            self.buf.write_all(&DOS_TIME.to_le_bytes())?;
            self.buf.write_all(&DOS_DATE.to_le_bytes())?;
            self.buf.write_all(&e.crc.to_le_bytes())?;
            self.buf.write_all(&e.comp_size.to_le_bytes())?;
            self.buf.write_all(&e.size.to_le_bytes())?;
            self.buf.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
            self.buf.write_all(&0u16.to_le_bytes())?; // extra
            self.buf.write_all(&0u16.to_le_bytes())?; // comment
            self.buf.write_all(&0u16.to_le_bytes())?; // disk start
            self.buf.write_all(&0u16.to_le_bytes())?; // internal attr
            self.buf.write_all(&0u32.to_le_bytes())?; // external attr
            self.buf.write_all(&e.offset.to_le_bytes())?;
            self.buf.write_all(name_bytes)?;
        }
        let cd_size = self.buf.len() as u32 - cd_offset;

        self.buf.write_all(&0x0605_4b50u32.to_le_bytes())?;
        self.buf.write_all(&0u16.to_le_bytes())?; // disk
        self.buf.write_all(&0u16.to_le_bytes())?; // cd disk
        self.buf.write_all(&(self.entries.len() as u16).to_le_bytes())?;
        self.buf.write_all(&(self.entries.len() as u16).to_le_bytes())?;
        self.buf.write_all(&cd_size.to_le_bytes())?;
        self.buf.write_all(&cd_offset.to_le_bytes())?;
        self.buf.write_all(&0u16.to_le_bytes())?; // comment len
        Ok(self.buf)
    }
}

fn deflate(data: &[u8]) -> std::io::Result<Vec<u8>> {
    use flate2::{write::DeflateEncoder, Compression};
    let mut e = DeflateEncoder::new(Vec::with_capacity(data.len() / 4 + 64), Compression::new(6));
    e.write_all(data)?;
    e.finish()
}

/// 标准 CRC-32（IEEE）。
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 从生成的 zip 里取出某个条目（用于测试）。
    fn entry(bytes: &[u8], want: &str) -> String {
        use flate2::read::DeflateDecoder;
        use std::io::Read;
        let mut i = 0usize;
        while i + 30 <= bytes.len() {
            if bytes[i..i + 4] != [0x50, 0x4b, 0x03, 0x04] {
                break;
            }
            let method = u16::from_le_bytes([bytes[i + 8], bytes[i + 9]]);
            let comp = u32::from_le_bytes(bytes[i + 18..i + 22].try_into().unwrap()) as usize;
            let nlen = u16::from_le_bytes([bytes[i + 26], bytes[i + 27]]) as usize;
            let elen = u16::from_le_bytes([bytes[i + 28], bytes[i + 29]]) as usize;
            let name = std::str::from_utf8(&bytes[i + 30..i + 30 + nlen]).unwrap();
            let data_start = i + 30 + nlen + elen;
            let raw = &bytes[data_start..data_start + comp];
            if name == want {
                return if method == 8 {
                    let mut s = String::new();
                    DeflateDecoder::new(raw).read_to_string(&mut s).unwrap();
                    s
                } else {
                    String::from_utf8(raw.to_vec()).unwrap()
                };
            }
            i = data_start + comp;
        }
        panic!("找不到条目 {want}");
    }

    fn doc_xml(md: &str) -> String {
        entry(&markdown_to_docx_bytes(md).unwrap(), "word/document.xml")
    }

    #[test]
    fn produces_valid_zip_magic() {
        let bytes = markdown_to_docx_bytes(r"勾股定理 $$a^2+b^2=c^2$$ 完毕。").unwrap();
        assert_eq!(&bytes[0..2], b"PK");
    }

    #[test]
    fn crc_known_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    // ---- 回归 R2：产物必须是合法 XML ----

    #[test]
    fn document_xml_is_well_formed() {
        for md in [
            "普通中文段落",
            r"$$\sum_{k=1}^n \frac{1}{k}$$",
            "含控制符\u{0007}和\u{0000}的文本",
            "| a | b |\n|---|---|\n| $x$ | y |\n",
            "- 甲\n- 乙\n\n1. 一\n2. 二\n",
            "> 引用 $E=mc^2$",
            "```rust\nlet x = 1 < 2 && 3 > 2;\n```",
            "[链接](https://example.com/?a=1&b=2)",
            "标题里的 & < > 符号\n===",
            r"公式 $a < b$ 与 $c > d$",
            "",
        ] {
            let xml = doc_xml(md);
            roxmltree::Document::parse(&xml)
                .unwrap_or_else(|e| panic!("document.xml 非法 XML ({md:?}): {e}\n{xml}"));
        }
    }

    #[test]
    fn control_chars_are_stripped() {
        let xml = doc_xml("前\u{0001}后");
        assert!(xml.contains("前后"), "非法控制字符应被丢弃: {xml}");
    }

    #[test]
    fn all_parts_are_well_formed_xml() {
        let bytes = markdown_to_docx_bytes("1. 甲\n2. 乙\n\n[x](https://a.example/?q=1&r=2)").unwrap();
        for part in [
            "[Content_Types].xml",
            "_rels/.rels",
            "word/document.xml",
            "word/_rels/document.xml.rels",
            "word/styles.xml",
            "word/numbering.xml",
        ] {
            let xml = entry(&bytes, part);
            roxmltree::Document::parse(&xml)
                .unwrap_or_else(|e| panic!("{part} 非法: {e}\n{xml}"));
        }
    }

    // ---- 回归 F7：有序列表必须是真编号 ----

    #[test]
    fn ordered_list_uses_real_numbering() {
        let xml = doc_xml("1. 甲\n2. 乙\n3. 丙\n");
        assert!(xml.contains("<w:numPr>"), "应使用 w:numPr: {xml}");
        assert_eq!(xml.matches("w:numId").count(), 3, "三项都应带 numId");
        assert!(!xml.contains("1. "), "不应把编号写死成文本: {xml}");
    }

    #[test]
    fn two_ordered_lists_get_separate_num_ids() {
        let bytes = markdown_to_docx_bytes("1. 甲\n2. 乙\n\n中间段落\n\n1. 丙\n2. 丁\n").unwrap();
        let numbering = entry(&bytes, "word/numbering.xml");
        assert_eq!(
            numbering.matches("<w:num ").count(),
            2,
            "两个列表应各有一个 w:num 实例，才能各自从 1 开始: {numbering}"
        );
    }

    // ---- 回归 F8：超链接 ----

    #[test]
    fn links_become_real_hyperlinks() {
        let bytes = markdown_to_docx_bytes("见 [官网](https://example.com/a?x=1&y=2)").unwrap();
        let xml = entry(&bytes, "word/document.xml");
        assert!(xml.contains("<w:hyperlink r:id=\"rId3\">"), "{xml}");
        assert!(!xml.contains("<https://"), "不应再输出 `文字 <url>` 纯文本: {xml}");
        let rels = entry(&bytes, "word/_rels/document.xml.rels");
        assert!(rels.contains("TargetMode=\"External\""), "{rels}");
        assert!(rels.contains("x=1&amp;y=2"), "URL 中的 & 必须转义: {rels}");
    }

    // ---- 回归 F9：公式编号制表位不得超出正文宽度 ----

    #[test]
    fn tab_stop_within_text_width() {
        let xml = doc_xml("$$E = mc^2 \\tag{1}$$");
        assert!(xml.contains(&format!("w:pos=\"{CONTENT_W_TWIPS}\"")), "{xml}");
        assert!(!xml.contains("w:pos=\"9600\""));
        assert_eq!(CONTENT_W_TWIPS, 9026);
    }

    // ---- 回归 P5：不重复声明命名空间 ----

    #[test]
    fn omath_does_not_repeat_namespace() {
        let xml = doc_xml("$a$ 与 $b$ 与 $c$");
        assert_eq!(
            xml.matches("xmlns:m=").count(),
            1,
            "xmlns:m 只应在根节点声明一次: {xml}"
        );
    }

    // ---- 回归 F10：不再有隐式上下标魔法 ----

    #[test]
    fn no_implicit_superscript_magic() {
        let xml = doc_xml("路径 ~/data/~ 与 a^b^c");
        assert!(
            !xml.contains("vertAlign"),
            "普通文本里的 ^ / ~ 不应被转成上下标: {xml}"
        );
    }

    // ---- 回归 P6：deflate 压缩 ----

    #[test]
    fn zip_is_deflated() {
        let md = "重复内容 ".repeat(2000);
        let bytes = markdown_to_docx_bytes(&md).unwrap();
        let raw = doc_xml(&md).len();
        assert!(
            bytes.len() * 3 < raw,
            "deflate 后应显著小于原始 XML：zip={} raw={}",
            bytes.len(),
            raw
        );
    }

    // ---- 降级路径 ----

    #[test]
    fn broken_formula_degrades_with_warning() {
        let out = markdown_to_docx(r"$$\undefinedcmd{x} \& \&$$").unwrap();
        let xml = entry(&out.bytes, "word/document.xml");
        roxmltree::Document::parse(&xml).unwrap();
    }

    #[test]
    fn readable_symbols_fallback() {
        assert_eq!(latex_to_readable_symbols(r"\alpha + \beta"), "α + β");
        assert_eq!(latex_to_readable_symbols(r"\mathbb{R}^n"), "ℝ^n");
        assert_eq!(latex_to_readable_symbols(r"a  \quad   b"), "a b");
    }

    #[test]
    fn never_panics_on_adversarial_markdown() {
        for md in [
            "$", "$$", r"$$x \tag你$$", "\u{0}\u{8}", r"\(", "|a|\n|-|\n",
            "- ".repeat(200).as_str(), "#".repeat(50).as_str(),
        ] {
            let _ = markdown_to_docx_bytes(md);
        }
    }
}
