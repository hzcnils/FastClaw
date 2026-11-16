//! 保留块级结构的文档模型。
//!
//! 用 pulldown-cmark 的事件流重建一棵**块级文档树**，每个块内部再用行内片段
//! （文本 / 行内公式 / 加粗 / 行内代码）表示。导出器据此映射段落样式与字号层级。

use crate::markdown::{split_text_math, Segment};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Parser, Tag, TagEnd};

/// 行内级内容（块内部）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Strong(String),
    Emphasis(String),
    Code(String),
    /// 行内公式（LaTeX 源码）
    Math(String),
    Link {
        text: String,
        url: String,
    },
    FootnoteRef(String),
}

/// 块级元素。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// 标题：级别 1-6 + 行内内容
    Heading(u8, Vec<Inline>),
    Paragraph(Vec<Inline>),
    /// 块级公式（独占一行，LaTeX 源码）
    DisplayMath(String),
    /// 代码块：语言（可空）+ 原始文本
    CodeBlock(Option<String>, String),
    /// 列表项（已扁平化）。
    ///
    /// `list_id` 标识它属于哪一个列表实例 —— DOCX 侧据此为每个列表生成独立的
    /// `w:num`，使多个有序列表各自从 1 开始；`index` 是它在本层的序号，
    /// 供没有自动编号能力的 PDF 侧使用。
    ListItem {
        ordered: bool,
        depth: u8,
        list_id: u32,
        index: u64,
        content: Vec<Inline>,
    },
    Quote(Vec<Inline>),
    Table {
        header: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    FootnoteDefinition {
        label: String,
        content: Vec<Inline>,
    },
    Rule,
}

/// 把 Markdown 解析为块级文档树。
#[must_use]
pub fn parse_document(markdown: &str) -> Vec<Block> {
    // 预处理：把 LaTeX 风格分隔符 \( \) \[ \] 归一化为 $ $$，再交给 cmark。
    // 原因：CommonMark 把 `\[` `\(` 视为「转义的标点」，会吞掉反斜杠只留 `[` `(`，
    // 等文本流到下游时公式边界已丢失。故必须在 cmark 之前完成替换。
    let normalized = normalize_latex_delimiters(markdown);
    parse_document_inner(&normalized)
}

/// 将 `\( \) \[ \]` 替换为 `$ $$`，但保留围栏代码块与行内代码原样。
///
/// 已知局限：只识别围栏代码块（``` / ~~~），不识别 4 空格缩进代码块。
fn normalize_latex_delimiters(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut in_fence = false;
    let mut fence_marker = ' ';
    // 跨行追踪当前是否处在 \( 或 \[ 之内：避免把正文里孤立的 `\)` `\]` 误替换。
    let mut open: Option<bool> = None; // Some(true)=\[ 已打开, Some(false)=\( 已打开

    for line in src.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let marker = trimmed.as_bytes()[0] as char;
            if in_fence {
                if marker == fence_marker {
                    in_fence = false;
                }
            } else {
                in_fence = true;
                fence_marker = marker;
            }
            out.push_str(line);
            continue;
        }
        if in_fence {
            out.push_str(line);
            continue;
        }
        normalize_line_into(line, &mut out, &mut open);
    }
    out
}

/// 对单行做替换，跳过反引号包裹的行内代码。
///
/// 关键细节：pulldown-cmark 的行内公式要求开 `$` 后面、闭 `$` 前面**都不是空白**
/// （否则不当作公式）。而源文里常写成 `\( \mathcal{D} \)`（分隔符两侧带空格），
/// 直接替换成 `$ \mathcal{D} $` 会被 cmark 当普通文本。
/// 因此：开分隔符替换后要吃掉其后的空白；闭分隔符替换前要回删已写入的尾部空白。
fn normalize_line_into(line: &str, out: &mut String, open: &mut Option<bool>) {
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut in_code = false; // 行内代码 `...`

    while i < n {
        let c = chars[i];
        if c == '`' {
            in_code = !in_code;
            out.push(c);
            i += 1;
            continue;
        }
        if !in_code && c == '\\' && i + 1 < n {
            match chars[i + 1] {
                // 开分隔符：写入 $/$$，再跳过其后紧邻的空白
                '(' | '[' if open.is_none() => {
                    let display = chars[i + 1] == '[';
                    out.push_str(if display { "$$" } else { "$" });
                    *open = Some(display);
                    i += 2;
                    while i < n && chars[i].is_whitespace() {
                        i += 1;
                    }
                    continue;
                }
                // 闭分隔符：仅在确实有对应开分隔符时才处理
                ')' | ']' => {
                    let display = chars[i + 1] == ']';
                    if *open == Some(display) {
                        while out.ends_with(|ch: char| ch.is_whitespace()) {
                            out.pop();
                        }
                        out.push_str(if display { "$$" } else { "$" });
                        *open = None;
                        i += 2;
                        continue;
                    }
                }
                _ => {}
            }
        }
        out.push(c);
        i += 1;
    }
}

/// 一个列表实例的累积状态。
struct ListState {
    ordered: bool,
    next_index: u64,
    id: u32,
}

fn parse_document_inner(markdown: &str) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let parser = Parser::new_ext(markdown, pulldown_cmark::Options::all());

    let mut inlines: Vec<Inline> = Vec::new();
    let mut ctx: Vec<Ctx> = Vec::new();
    let mut list_stack: Vec<ListState> = Vec::new();
    let mut next_list_id: u32 = 0;
    let mut code_lang: Option<String> = None;
    let mut code_buf = String::new();
    let mut table_header: Vec<Vec<Inline>> = Vec::new();
    let mut table_rows: Vec<Vec<Vec<Inline>>> = Vec::new();
    let mut current_row: Vec<Vec<Inline>> = Vec::new();
    let mut current_cell: Vec<Inline> = Vec::new();
    let mut in_table_head = false;
    let mut current_link: Option<(String, bool, Vec<Inline>)> = None;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                inlines.clear();
                ctx.push(Ctx::Heading(heading_to_u8(level)));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(Ctx::Heading(lvl)) = ctx.pop() {
                    blocks.push(Block::Heading(
                        lvl,
                        finalize_math(std::mem::take(&mut inlines)),
                    ));
                }
            }

            Event::Start(Tag::Paragraph) => {
                if !in_footnote_definition(&ctx) {
                    inlines.clear();
                }
                ctx.push(Ctx::Paragraph);
            }
            Event::End(TagEnd::Paragraph) => {
                ctx.pop();
                if in_footnote_definition(&ctx) || matches!(ctx.last(), Some(Ctx::Item)) {
                    continue;
                }
                let parts = finalize_math(std::mem::take(&mut inlines));
                push_paragraph_or_display(&mut blocks, parts);
            }

            Event::Start(Tag::List(start)) => {
                let id = next_list_id;
                next_list_id += 1;
                list_stack.push(ListState {
                    ordered: start.is_some(),
                    next_index: start.unwrap_or(1),
                    id,
                });
            }
            Event::End(TagEnd::List(_)) => {
                list_stack.pop();
            }
            Event::Start(Tag::Item) => {
                inlines.clear();
                ctx.push(Ctx::Item);
            }
            Event::End(TagEnd::Item) => {
                ctx.pop();
                let depth = list_stack.len().saturating_sub(1) as u8;
                let (ordered, list_id, index) = match list_stack.last_mut() {
                    Some(st) => {
                        let idx = st.next_index;
                        st.next_index += 1;
                        (st.ordered, st.id, idx)
                    }
                    None => (false, 0, 1),
                };
                let content = finalize_math(std::mem::take(&mut inlines));
                blocks.push(Block::ListItem {
                    ordered,
                    depth,
                    list_id,
                    index,
                    content,
                });
            }

            Event::Start(Tag::BlockQuote(_)) => {
                inlines.clear();
                ctx.push(Ctx::Quote);
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                ctx.pop();
                blocks.push(Block::Quote(finalize_math(std::mem::take(&mut inlines))));
            }

            Event::Start(Tag::CodeBlock(kind)) => {
                code_buf.clear();
                code_lang = match kind {
                    CodeBlockKind::Fenced(s) if !s.is_empty() => Some(s.to_string()),
                    _ => None,
                };
                ctx.push(Ctx::Code);
            }
            Event::End(TagEnd::CodeBlock) => {
                ctx.pop();
                blocks.push(Block::CodeBlock(
                    code_lang.take(),
                    std::mem::take(&mut code_buf),
                ));
            }
            Event::Start(Tag::FootnoteDefinition(label)) => {
                inlines.clear();
                ctx.push(Ctx::FootnoteDefinition(label.to_string()));
            }
            Event::End(TagEnd::FootnoteDefinition) => {
                if let Some(Ctx::FootnoteDefinition(label)) = ctx.pop() {
                    blocks.push(Block::FootnoteDefinition {
                        label,
                        content: finalize_math(std::mem::take(&mut inlines)),
                    });
                }
            }

            Event::Start(Tag::Table(_)) => {
                table_header.clear();
                table_rows.clear();
                ctx.push(Ctx::Table);
            }
            Event::End(TagEnd::Table) => {
                ctx.pop();
                blocks.push(Block::Table {
                    header: std::mem::take(&mut table_header),
                    rows: std::mem::take(&mut table_rows),
                });
            }
            Event::Start(Tag::TableHead) => in_table_head = true,
            Event::End(TagEnd::TableHead) => {
                in_table_head = false;
                table_header = std::mem::take(&mut current_row);
            }
            Event::Start(Tag::TableRow) => current_row.clear(),
            Event::End(TagEnd::TableRow) => {
                if !in_table_head {
                    table_rows.push(std::mem::take(&mut current_row));
                }
            }
            Event::Start(Tag::TableCell) => current_cell.clear(),
            Event::End(TagEnd::TableCell) => {
                current_row.push(finalize_math(std::mem::take(&mut current_cell)));
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                let in_table = in_table_cell(&ctx);
                current_link = Some((dest_url.to_string(), in_table, Vec::new()));
                ctx.push(Ctx::Link);
            }
            Event::End(TagEnd::Link) => {
                ctx.retain(|c| !matches!(c, Ctx::Link));
                if let Some((url, in_table, content)) = current_link.take() {
                    let text = inlines_plain_text(&content).trim().to_string();
                    let link = Inline::Link {
                        text: if text.is_empty() { url.clone() } else { text },
                        url,
                    };
                    push_inline(in_table, &mut inlines, &mut current_cell, link);
                }
            }

            Event::Start(Tag::Strong) => ctx.push(Ctx::Strong),
            Event::End(TagEnd::Strong) => {
                ctx.pop();
            }
            Event::Start(Tag::Emphasis) => ctx.push(Ctx::Emphasis),
            Event::End(TagEnd::Emphasis) => {
                ctx.pop();
            }

            Event::Rule => blocks.push(Block::Rule),

            Event::Text(t) => {
                if ctx.iter().any(|c| matches!(c, Ctx::Code)) {
                    code_buf.push_str(&t);
                } else {
                    let in_table = in_table_cell(&ctx);
                    let inl = match strongest_emphasis(&ctx) {
                        Some(Ctx::Strong) => Inline::Strong(t.to_string()),
                        Some(Ctx::Emphasis) => Inline::Emphasis(t.to_string()),
                        _ => Inline::Text(t.to_string()),
                    };
                    if let Some((_, _, link_content)) = current_link.as_mut() {
                        link_content.push(inl);
                    } else {
                        push_inline(in_table, &mut inlines, &mut current_cell, inl);
                    }
                }
            }
            Event::Code(c) => {
                let in_table = in_table_cell(&ctx);
                let inl = Inline::Code(c.to_string());
                if let Some((_, _, link_content)) = current_link.as_mut() {
                    link_content.push(inl);
                } else {
                    push_inline(in_table, &mut inlines, &mut current_cell, inl);
                }
            }

            // cmark 在 Options::all() 下原生识别 $...$ 与 $$...$$
            Event::InlineMath(m) => {
                let in_table = in_table_cell(&ctx);
                let inl = Inline::Math(m.to_string());
                if let Some((_, _, link_content)) = current_link.as_mut() {
                    link_content.push(inl);
                } else {
                    push_inline(in_table, &mut inlines, &mut current_cell, inl);
                }
            }
            Event::DisplayMath(m) => {
                push_inline(
                    in_table_cell(&ctx),
                    &mut inlines,
                    &mut current_cell,
                    Inline::Math(m.to_string()),
                );
            }
            Event::FootnoteReference(name) => push_inline(
                in_table_cell(&ctx),
                &mut inlines,
                &mut current_cell,
                Inline::FootnoteRef(name.to_string()),
            ),
            Event::TaskListMarker(checked) => push_inline(
                in_table_cell(&ctx),
                &mut inlines,
                &mut current_cell,
                Inline::Text(if checked { "[x] ".into() } else { "[ ] ".into() }),
            ),
            Event::SoftBreak | Event::HardBreak => {
                if ctx.iter().any(|c| matches!(c, Ctx::Code)) {
                    code_buf.push('\n');
                } else {
                    let inl = Inline::Text(" ".into());
                    if let Some((_, _, link_content)) = current_link.as_mut() {
                        link_content.push(inl);
                    } else {
                        push_inline(in_table_cell(&ctx), &mut inlines, &mut current_cell, inl);
                    }
                }
            }
            _ => {}
        }
    }

    blocks
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Ctx {
    Heading(u8),
    Paragraph,
    Item,
    Quote,
    Code,
    Table,
    FootnoteDefinition(String),
    Link,
    Strong,
    Emphasis,
}

fn in_table_cell(ctx: &[Ctx]) -> bool {
    ctx.iter().any(|c| matches!(c, Ctx::Table))
}

fn in_footnote_definition(ctx: &[Ctx]) -> bool {
    ctx.iter().any(|c| matches!(c, Ctx::FootnoteDefinition(_)))
}

fn strongest_emphasis(ctx: &[Ctx]) -> Option<&Ctx> {
    ctx.iter()
        .rev()
        .find(|c| matches!(c, Ctx::Strong | Ctx::Emphasis))
}

fn push_inline(
    in_table: bool,
    inlines: &mut Vec<Inline>,
    current_cell: &mut Vec<Inline>,
    inl: Inline,
) {
    if in_table {
        current_cell.push(inl);
    } else {
        inlines.push(inl);
    }
}

fn heading_to_u8(l: HeadingLevel) -> u8 {
    match l {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// 合并相邻 `Inline::Text`，再对文本做一次公式兜底扫描。
fn finalize_math(inlines: Vec<Inline>) -> Vec<Inline> {
    let mut merged: Vec<Inline> = Vec::with_capacity(inlines.len());
    for inl in inlines {
        match (merged.last_mut(), &inl) {
            (Some(Inline::Text(prev)), Inline::Text(cur)) => prev.push_str(cur),
            _ => merged.push(inl),
        }
    }
    // 兜底：某些上下文（如表格单元格）里 cmark 可能不产出 InlineMath 事件。
    let mut rescanned: Vec<Inline> = Vec::with_capacity(merged.len());
    for inl in merged {
        match inl {
            Inline::Text(t) => {
                for seg in split_text_math(&t) {
                    match seg {
                        Segment::Text(s) => rescanned.push(Inline::Text(s)),
                        Segment::InlineMath(m) | Segment::DisplayMath(m) => {
                            rescanned.push(Inline::Math(m));
                        }
                    }
                }
            }
            other => rescanned.push(other),
        }
    }

    rescanned
        .into_iter()
        .filter(|i| !matches!(i, Inline::Text(t) if t.is_empty()))
        .collect()
}

fn inlines_plain_text(inlines: &[Inline]) -> String {
    inlines
        .iter()
        .map(|i| match i {
            Inline::Text(t) | Inline::Strong(t) | Inline::Emphasis(t) | Inline::Code(t) => {
                t.clone()
            }
            Inline::Math(m) => format!(" {m} "),
            Inline::Link { text, .. } => text.clone(),
            Inline::FootnoteRef(label) => format!("[{label}]"),
        })
        .collect()
}

/// 若一个段落其实是「整段一个块级公式」，拆成 `DisplayMath` 块。
fn push_paragraph_or_display(blocks: &mut Vec<Block>, parts: Vec<Inline>) {
    let non_space: Vec<&Inline> = parts
        .iter()
        .filter(|i| !matches!(i, Inline::Text(t) if t.trim().is_empty()))
        .collect();

    if non_space.len() == 1 {
        if let Inline::Math(m) = non_space[0] {
            blocks.push(Block::DisplayMath(m.clone()));
            return;
        }
    }
    if !parts.is_empty() {
        blocks.push(Block::Paragraph(parts));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norm_line(s: &str) -> String {
        let mut out = String::new();
        let mut open = None;
        normalize_line_into(s, &mut out, &mut open);
        out
    }

    #[test]
    fn inline_latex_delims_hug_dollars() {
        let got = norm_line(r"模型 \( \mathcal{M} \) 于 \( \theta \)，文本 \( x \) 中");
        assert_eq!(got, r"模型 $\mathcal{M}$ 于 $\theta$，文本 $x$ 中");
    }

    #[test]
    fn inline_math_is_parsed_not_left_as_text() {
        let blocks = parse_document(r"训练语料 \( \mathcal{D} \)，定义如下。");
        let has_math = blocks.iter().any(|b| {
            matches!(b, Block::Paragraph(inls) if inls.iter().any(|i| matches!(i, Inline::Math(_))))
        });
        assert!(has_math, "inline \\(..\\) should become Inline::Math: {blocks:?}");
    }

    #[test]
    fn display_math_with_tag_becomes_display_block() {
        let md = "\\[\nE = mc^2 \\tag{1}\n\\]";
        let blocks = parse_document(md);
        assert!(
            blocks.iter().any(|b| matches!(b, Block::DisplayMath(_))),
            "should produce a DisplayMath block: {blocks:?}"
        );
    }

    #[test]
    fn fenced_code_is_not_touched() {
        let md = "```python\nx = arr\\[i\\]  # not math\n```";
        let out = normalize_latex_delimiters(md);
        assert!(out.contains("arr\\[i\\]"), "code block must stay literal: {out}");
    }

    #[test]
    fn unpaired_close_delimiter_is_left_alone() {
        // 正文里孤立的 \] 不该被替换成 $$
        assert_eq!(norm_line(r"数组 a\]"), r"数组 a\]");
    }

    // ---- 列表编号 ----

    #[test]
    fn ordered_lists_get_independent_ids_and_indices() {
        let blocks = parse_document("1. 甲\n2. 乙\n\n段落\n\n1. 丙\n2. 丁\n");
        let items: Vec<_> = blocks
            .iter()
            .filter_map(|b| match b {
                Block::ListItem { list_id, index, ordered, .. } => Some((*list_id, *index, *ordered)),
                _ => None,
            })
            .collect();
        assert_eq!(items.len(), 4);
        assert!(items.iter().all(|i| i.2), "都应是有序列表");
        assert_eq!(items[0].1, 1);
        assert_eq!(items[1].1, 2);
        assert_eq!(items[2].1, 1, "第二个列表应从 1 重新开始");
        assert_ne!(items[0].0, items[2].0, "两个列表应有不同的 list_id");
    }

    #[test]
    fn ordered_list_respects_start_number() {
        let blocks = parse_document("5. 五\n6. 六\n");
        let idx: Vec<_> = blocks
            .iter()
            .filter_map(|b| match b {
                Block::ListItem { index, .. } => Some(*index),
                _ => None,
            })
            .collect();
        assert_eq!(idx, vec![5, 6]);
    }

    #[test]
    fn nested_list_depth_and_id() {
        let blocks = parse_document("- 甲\n  - 乙\n  - 丙\n- 丁\n");
        let items: Vec<_> = blocks
            .iter()
            .filter_map(|b| match b {
                Block::ListItem { depth, index, .. } => Some((*depth, *index)),
                _ => None,
            })
            .collect();
        assert!(items.iter().any(|(d, _)| *d == 1), "应存在 depth=1 的嵌套项: {items:?}");
    }
}
