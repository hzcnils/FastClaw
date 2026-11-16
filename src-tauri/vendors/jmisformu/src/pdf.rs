//! PDF 矢量公式导出（支持中文 + 精确公式布局 + 跨页分页）。
//!
//! ## 架构
//! 排版拆成两个互不耦合的阶段，测量与绘制共用同一份逻辑，不会走偏：
//!   1. `layout_lines`：纯布局。把行内片段折成 `Vec<Line>`，不涉及页面、不绘制。
//!   2. `draw_lines`：按行绘制，**逐行判断是否需要翻页**。
//!
//! 旧版把两者揉在 `layout_inlines_in_box(Option<&layer>, ...)` 里，且换行只做
//! `*y -= line_h`，既没有页面下沿检查、也拿不到 `doc` 去新建页 —— 超过一页的
//! 段落 / 代码块 / 表格会被直接画到页面外，内容静默丢失。
//!
//! ## 其它关键改动
//!   * **字体度量缓存**：旧版 `char_width_mm` 每个字符都 `Face::parse` 一遍整份字体
//!     （2 万字中文 = 2 万次完整字体解析）。现在字体只解析一次，字形宽度按字符缓存。
//!   * **公式渲染缓存**：表格里一条公式旧版会被 RaTeX 渲染 5 次（测宽 / 测高 /
//!     `compute_table_metrics` 被整体重算一遍 / 绘制）。现在按 (latex, display) memo。
//!   * **批量绘制**：旧版逐字符 `use_text`，每个字符要写一整组 BT/Tf/Td/Tj/ET 算子。
//!     现在同一行、同一字体的连续字符合并成一次调用。
//!   * **必须有真字体**：找不到字体时直接报错，而不是静默回退到内置 Helvetica ——
//!     那样非 ASCII 会渲染成乱码，且排版宽度也全是错的。

use crate::doc_model::{parse_document, Block, Inline};
use crate::error::{JmisformuError, Result, Warning};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

const PT_TO_MM: f32 = 0.352_778;
const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN_L: f32 = 20.0;
const MARGIN_R: f32 = 20.0;
const CONTENT_W: f32 = PAGE_W - MARGIN_L - MARGIN_R;
const INLINE_MATH_GAP_MM: f32 = 0.9;
const BODY_SIZE: f32 = 11.0;
const CODE_SIZE: f32 = 9.0;

/// 导出参数。
#[derive(Debug, Clone, Default)]
pub struct PdfOptions {
    /// 显式指定正文字体（.ttf/.otf/.ttc）。不指定则按平台探测系统中文字体。
    pub font_path: Option<std::path::PathBuf>,
    /// 显式指定粗体字面。不指定则探测；探测不到时加粗降级为常规字重。
    pub bold_font_path: Option<std::path::PathBuf>,
}

/// PDF 导出结果。
pub struct PdfOutput {
    pub bytes: Vec<u8>,
    pub warnings: Vec<Warning>,
}

/// 一个渲染好的公式：SVG 源 + 真实尺寸（mm）。
pub struct RenderedFormula {
    pub svg: String,
    pub width_mm: f32,
    /// 基线以上高度
    pub height_mm: f32,
    /// 基线以下深度
    pub depth_mm: f32,
}

// ---------------------------------------------------------------------------
// 公式渲染
// ---------------------------------------------------------------------------

/// 渲染公式并返回真实尺寸（尺寸来自 RaTeX 的 DisplayList，精确）。
#[cfg(feature = "pdf-ratex")]
pub fn render_formula(latex: &str, display: bool) -> Result<RenderedFormula> {
    use ratex_layout::{layout, to_display_list, LayoutOptions};
    use ratex_parser::parse;
    use ratex_svg::{render_to_svg, SvgOptions};
    use ratex_types::color::Color;
    use ratex_types::math_style::MathStyle;

    let ast = parse(&crate::latex::extract_tag(latex).0)
        .map_err(|e| JmisformuError::Pdf(format!("RaTeX parse: {e}")))?;
    let style = if display {
        MathStyle::Display
    } else {
        MathStyle::Text
    };
    let opts = LayoutOptions::default()
        .with_style(style)
        .with_color(Color::BLACK);
    let lbox = layout(&ast, &opts);
    let dl = to_display_list(&lbox);

    let font_size: f64 = if display { 11.0 } else { 9.5 };
    let padding: f64 = 1.0;
    let svg_opts = SvgOptions {
        font_size,
        padding,
        stroke_width: 0.6,
        // 必须为 true：配合 Cargo 的 embed-fonts 特性，RaTeX 会用内置 KaTeX 字体把
        // 每个字形预渲染成矢量 <path>，SVG 里没有 <text>，svg2pdf 转 PDF 时就不嵌
        // 任何字体 —— 体积小、缩放不失真。若为 false，字形回退成 <text>，
        // svg2pdf 会为每条公式各嵌一份字体子集，体积爆炸。
        embed_glyphs: true,
        font_dir: String::new(),
    };
    let svg = render_to_svg(&dl, &svg_opts);

    // 尺寸直接取自 DisplayList，不额外 Svg::parse（那一步内部要跑 usvg，是最重的环节）。
    let em = font_size;
    let pad = padding;
    Ok(RenderedFormula {
        svg,
        width_mm: (dl.width * em + 2.0 * pad) as f32 * PT_TO_MM,
        height_mm: (dl.height * em + pad) as f32 * PT_TO_MM,
        depth_mm: (dl.depth * em + pad) as f32 * PT_TO_MM,
    })
}

/// feature 关闭时明确报错。
///
/// 旧版在这里返回一个把 LaTeX 源码塞进 `<text>`、宽度靠字符数估算的假 SVG，
/// 那不是降级，是静默产出错误文档。
#[cfg(not(feature = "pdf-ratex"))]
pub fn render_formula(_latex: &str, _display: bool) -> Result<RenderedFormula> {
    Err(JmisformuError::Pdf(
        "本次构建未启用 pdf-ratex feature，无法渲染公式".into(),
    ))
}

/// 兼容旧接口：仅返回 SVG 字符串。
pub fn latex_to_svg(latex: &str, display: bool) -> Result<String> {
    Ok(render_formula(latex, display)?.svg)
}

// ---------------------------------------------------------------------------
// 字体
// ---------------------------------------------------------------------------

fn font_candidates(bold: bool) -> Vec<&'static str> {
    // 单一字面（.ttf/.otf）优先于字体集合（.ttc）：printpdf 的子集化对单字面最稳，
    // .ttc 偶有子集失败而静默回退成整字体嵌入，会把文件撑大。
    if cfg!(target_os = "windows") {
        if bold {
            vec![r"C:\Windows\Fonts\msyhbd.ttc", r"C:\Windows\Fonts\simhei.ttf"]
        } else {
            vec![
                r"C:\Windows\Fonts\simhei.ttf",
                r"C:\Windows\Fonts\Deng.ttf",
                r"C:\Windows\Fonts\msyh.ttf",
                r"C:\Windows\Fonts\msyh.ttc",
                r"C:\Windows\Fonts\simsun.ttc",
                r"C:\Windows\Fonts\arial.ttf",
            ]
        }
    } else if cfg!(target_os = "macos") {
        if bold {
            vec!["/System/Library/Fonts/PingFang.ttc"]
        } else {
            vec![
                "/Library/Fonts/Arial Unicode.ttf",
                "/System/Library/Fonts/PingFang.ttc",
                "/System/Library/Fonts/STHeiti Light.ttc",
                "/System/Library/Fonts/Helvetica.ttc",
            ]
        }
    } else if bold {
        vec![
            "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Bold.otf",
            "/usr/share/fonts/truetype/noto/NotoSansCJKsc-Bold.otf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
        ]
    } else {
        vec![
            "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf",
            "/usr/share/fonts/truetype/noto/NotoSansCJKsc-Regular.otf",
            "/usr/share/fonts/opentype/noto/NotoSerifCJKsc-Regular.otf",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
            "/usr/share/fonts/truetype/arphic/uming.ttc",
            // 最后兜底：至少保证纯西文文档能出图
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
        ]
    }
}

fn find_font(explicit: Option<&std::path::Path>, bold: bool) -> Option<std::path::PathBuf> {
    if let Some(p) = explicit {
        return p.exists().then(|| p.to_path_buf());
    }
    font_candidates(bold)
        .iter()
        .map(std::path::PathBuf::from)
        .find(|p| p.exists())
}

/// 单个字面的度量：**字体只解析一次**，字形推进量按字符缓存。
struct FaceMetrics<'a> {
    face: ttf_parser::Face<'a>,
    units_per_em: f32,
    cache: RefCell<HashMap<char, Option<u16>>>,
}

impl<'a> FaceMetrics<'a> {
    fn new(data: &'a [u8]) -> Result<Self> {
        let face = ttf_parser::Face::parse(data, 0)
            .map_err(|e| JmisformuError::FontParse(format!("{e:?}")))?;
        let units_per_em = f32::from(face.units_per_em());
        Ok(Self {
            face,
            units_per_em,
            cache: RefCell::new(HashMap::new()),
        })
    }

    fn advance_units(&self, c: char) -> Option<u16> {
        if let Some(v) = self.cache.borrow().get(&c) {
            return *v;
        }
        let v = self
            .face
            .glyph_index(c)
            .and_then(|g| self.face.glyph_hor_advance(g));
        self.cache.borrow_mut().insert(c, v);
        v
    }

    fn char_width_mm(&self, c: char, size_pt: f32) -> f32 {
        if let Some(adv) = self.advance_units(c) {
            return f32::from(adv) / self.units_per_em * size_pt * PT_TO_MM;
        }
        // 字体没有该字形：给一个不至于把版面撑坏的估算
        let em = size_pt * PT_TO_MM;
        if (c as u32) > 0x2E80 {
            em
        } else if c == ' ' {
            em * 0.3
        } else {
            em * 0.5
        }
    }
}

struct Metrics<'a> {
    regular: FaceMetrics<'a>,
    bold: Option<FaceMetrics<'a>>,
}

impl<'a> Metrics<'a> {
    /// 按样式取字符宽度。等宽字体（Courier）的 ASCII 推进量恒为 0.6 em，
    /// 与实际绘制一致，所以可以安全批量绘制。
    fn width(&self, c: char, size: f32, style: TextStyle) -> f32 {
        if style.mono && c.is_ascii() {
            return size * PT_TO_MM * 0.6;
        }
        match (style.bold, &self.bold) {
            (true, Some(b)) => b.char_width_mm(c, size),
            _ => self.regular.char_width_mm(c, size),
        }
    }

    fn text_width_mm(&self, t: &str, size: f32, style: TextStyle) -> f32 {
        t.chars().map(|c| self.width(c, size, style)).sum()
    }
}

// ---------------------------------------------------------------------------
// 渲染上下文（公式缓存 + 告警）
// ---------------------------------------------------------------------------

struct RenderCtx {
    /// `None` 表示这条公式渲染失败，已记录过告警，不再重试。
    cache: HashMap<(String, bool), Option<Arc<RenderedFormula>>>,
    warnings: Vec<Warning>,
}

impl RenderCtx {
    fn new() -> Self {
        Self {
            cache: HashMap::new(),
            warnings: Vec::new(),
        }
    }

    fn formula(&mut self, latex: &str, display: bool) -> Option<Arc<RenderedFormula>> {
        let key = (latex.to_owned(), display);
        if let Some(v) = self.cache.get(&key) {
            return v.clone();
        }
        let v = match render_formula(latex, display) {
            Ok(f) => Some(Arc::new(f)),
            Err(e) => {
                self.warnings.push(Warning {
                    source: latex.to_string(),
                    message: format!("公式渲染失败，已降级为源码文本: {e}"),
                });
                None
            }
        };
        self.cache.insert(key, v.clone());
        v
    }
}

// ---------------------------------------------------------------------------
// 行布局（纯计算，不涉及页面）
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
struct TextStyle {
    bold: bool,
    mono: bool,
    link: bool,
}

enum Piece {
    Text { text: String, style: TextStyle },
    Math { f: Arc<RenderedFormula>, left_gap: f32, right_gap: f32 },
}

struct PlacedText {
    x: f32,
    text: String,
    style: TextStyle,
}

enum Item {
    Text(PlacedText),
    Math {
        x: f32,
        f: Arc<RenderedFormula>,
        scale: f32,
    },
}

struct Line {
    items: Vec<Item>,
    height: f32,
}

fn lines_height(lines: &[Line]) -> f32 {
    lines.iter().map(|l| l.height).sum()
}

/// 把行内片段折行。宽度单位 mm，`width` 是可用宽度（不是右边界）。
fn layout_lines(pieces: &[Piece], width: f32, size: f32, m: &Metrics) -> Vec<Line> {
    let base_h = size * PT_TO_MM * 1.5;
    let mut lines: Vec<Line> = Vec::new();
    let mut cur = Line {
        items: Vec::new(),
        height: base_h,
    };
    let mut x = 0.0f32;
    let mut run: Option<(f32, String, TextStyle)> = None;

    macro_rules! flush_run {
        () => {
            if let Some((rx, s, st)) = run.take() {
                if !s.is_empty() {
                    cur.items.push(Item::Text(PlacedText {
                        x: rx,
                        text: s,
                        style: st,
                    }));
                }
            }
        };
    }
    macro_rules! newline {
        () => {{
            flush_run!();
            let finished = std::mem::replace(
                &mut cur,
                Line {
                    items: Vec::new(),
                    height: base_h,
                },
            );
            lines.push(finished);
            x = 0.0;
        }};
    }

    for piece in pieces {
        match piece {
            Piece::Text { text, style } => {
                for ch in text.chars() {
                    if ch == '\n' {
                        newline!();
                        continue;
                    }
                    // 等宽字体只对 ASCII 生效；CJK 必须换回正文字体，否则字形缺失
                    let eff = TextStyle {
                        mono: style.mono && ch.is_ascii(),
                        ..*style
                    };
                    let cw = m.width(ch, size, eff);
                    if x + cw > width && x > 0.0 {
                        newline!();
                    }
                    let same = matches!(&run, Some((_, _, st)) if *st == eff);
                    if same {
                        if let Some((_, s, _)) = run.as_mut() {
                            s.push(ch);
                        }
                    } else {
                        flush_run!();
                        run = Some((x, ch.to_string(), eff));
                    }
                    x += cw;
                }
            }
            Piece::Math {
                f,
                left_gap,
                right_gap,
            } => {
                // 单条公式比整行还宽时按比例缩小，避免冲出右边距
                let scale = if f.width_mm > width && f.width_mm > 0.0 {
                    width / f.width_mm
                } else {
                    1.0
                };
                let w = f.width_mm * scale;
                if x + left_gap + w + right_gap > width && x > 0.0 {
                    newline!();
                }
                flush_run!();
                let lg = if x > 0.0 { *left_gap } else { 0.0 };
                x += lg;
                cur.items.push(Item::Math {
                    x,
                    f: Arc::clone(f),
                    scale,
                });
                x += w + right_gap;
                cur.height = cur
                    .height
                    .max((f.height_mm + f.depth_mm) * scale + 0.4);
            }
        }
    }
    flush_run!();
    lines.push(cur);
    lines
}

/// 一串片段不折行时的自然宽度（表格列宽估算用）。
fn natural_width(pieces: &[Piece], size: f32, m: &Metrics) -> f32 {
    pieces
        .iter()
        .map(|p| match p {
            Piece::Text { text, style } => text
                .chars()
                .map(|c| {
                    m.width(
                        c,
                        size,
                        TextStyle {
                            mono: style.mono && c.is_ascii(),
                            ..*style
                        },
                    )
                })
                .sum(),
            Piece::Math {
                f,
                left_gap,
                right_gap,
            } => f.width_mm + left_gap + right_gap,
        })
        .sum()
}

// ---------------------------------------------------------------------------
// 页面游标
// ---------------------------------------------------------------------------

struct Cursor<'d> {
    doc: &'d printpdf::PdfDocumentReference,
    layer: printpdf::PdfLayerReference,
    y: f32,
}

impl<'d> Cursor<'d> {
    const TOP: f32 = PAGE_H - 20.0;
    const BOTTOM: f32 = 18.0;
    /// 一页可用的最大垂直空间。
    const USABLE: f32 = Self::TOP - Self::BOTTOM;

    fn new_page(&mut self) {
        use printpdf::Mm;
        let (p, l) = self.doc.add_page(Mm(PAGE_W), Mm(PAGE_H), "Layer");
        self.layer = self.doc.get_page(p).get_layer(l);
        self.y = Self::TOP;
    }

    /// 确保还有 `h` mm 的垂直空间，不够就翻页。
    fn ensure(&mut self, h: f32) {
        if self.y - h < Self::BOTTOM {
            self.new_page();
        }
    }
}

// ---------------------------------------------------------------------------
// 绘制
// ---------------------------------------------------------------------------

struct Fonts {
    regular: printpdf::IndirectFontRef,
    bold: Option<printpdf::IndirectFontRef>,
    mono: printpdf::IndirectFontRef,
}

impl Fonts {
    fn pick(&self, style: TextStyle) -> &printpdf::IndirectFontRef {
        if style.mono {
            &self.mono
        } else if style.bold {
            self.bold.as_ref().unwrap_or(&self.regular)
        } else {
            &self.regular
        }
    }
}

/// 在固定位置绘制若干行（不翻页）。调用方须保证空间足够。
fn draw_lines_at(
    layer: &printpdf::PdfLayerReference,
    lines: &[Line],
    x_start: f32,
    y_start: f32,
    size: f32,
    fonts: &Fonts,
) {
    use printpdf::{Color, Greyscale, Mm, Rgb};
    let mut y = y_start;
    for line in lines {
        for item in &line.items {
            match item {
                Item::Text(t) => {
                    if t.text.is_empty() {
                        continue;
                    }
                    if t.style.link {
                        layer.set_fill_color(Color::Rgb(Rgb::new(0.02, 0.39, 0.76, None)));
                    }
                    layer.use_text(&t.text, size, Mm(x_start + t.x), Mm(y), fonts.pick(t.style));
                    if t.style.link {
                        // printpdf 的填充色是图层级粘性状态，必须复位
                        layer.set_fill_color(Color::Greyscale(Greyscale::new(0.0, None)));
                    }
                }
                Item::Math { x, f, scale } => {
                    place_svg(layer, &f.svg, x_start + x, y - f.depth_mm * scale, *scale);
                }
            }
        }
        y -= line.height;
    }
}

/// 按行绘制并在需要时翻页。
fn draw_lines(cur: &mut Cursor, lines: &[Line], x_start: f32, size: f32, fonts: &Fonts) {
    for line in lines {
        cur.ensure(line.height);
        draw_lines_at(&cur.layer, std::slice::from_ref(line), x_start, cur.y, size, fonts);
        cur.y -= line.height;
    }
}

/// 把 SVG 矢量嵌入，使其**底边**位于 `bottom_mm`（与 use_text 同一套自底向上坐标）。
///
/// 两个关键点：
///   1. printpdf 用 `Svg::parse` 读 BBox 的**像素**尺寸再按 dpi 换算 pt；默认 dpi=300
///      会把公式放大约 4.17 倍撑满页面。RaTeX 的 SVG viewBox 单位是 pt，故设 72。
///   2. SVG 经「先 Scale 再 Translate」放置，锚点在左下角，PDF 坐标自底向上。
fn place_svg(layer: &printpdf::PdfLayerReference, svg_str: &str, x_mm: f32, bottom_mm: f32, scale: f32) {
    use printpdf::{Mm, Svg, SvgTransform};
    if let Ok(svg) = Svg::parse(svg_str) {
        let xobj = svg.into_xobject(layer);
        xobj.add_to_layer(
            layer,
            SvgTransform {
                translate_x: Some(Mm(x_mm).into()),
                translate_y: Some(Mm(bottom_mm).into()),
                scale_x: Some(scale),
                scale_y: Some(scale),
                dpi: Some(72.0),
                ..Default::default()
            },
        );
    }
}

// ---------------------------------------------------------------------------
// 片段展开
// ---------------------------------------------------------------------------

fn link_display_text(text: &str, url: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed == url {
        url.to_string()
    } else {
        format!("{trimmed} <{url}>")
    }
}

fn inline_text_repr(inl: &Inline) -> Option<String> {
    match inl {
        Inline::Text(t) | Inline::Strong(t) | Inline::Emphasis(t) | Inline::Code(t) => {
            Some(t.clone())
        }
        Inline::Link { text, url } => Some(link_display_text(text, url)),
        Inline::FootnoteRef(label) => Some(format!("[{label}]")),
        Inline::Math(_) => None,
    }
}

fn needs_gap_before_math(prev: Option<&Inline>, idx: usize) -> bool {
    if idx == 0 {
        return false;
    }
    match prev.and_then(last_non_whitespace_char) {
        Some(ch) => !matches!(ch, '(' | '[' | '{' | '（' | '【' | '《' | '“' | '"' | '\''),
        None => false,
    }
}

fn needs_gap_after_math(next: Option<&Inline>) -> bool {
    match next.and_then(first_non_whitespace_char) {
        Some(ch) => !matches!(
            ch,
            ')' | ']' | '}' | '）' | '】' | '》' | '”' | '"' | '\'' | ',' | '.' | '!' | '?'
                | ':' | ';' | '，' | '。' | '！' | '？' | '：' | '；'
        ),
        None => false,
    }
}

fn last_non_whitespace_char(inl: &Inline) -> Option<char> {
    match inl {
        Inline::Math(_) => Some('x'),
        _ => inline_text_repr(inl).and_then(|t| t.chars().rev().find(|c| !c.is_whitespace())),
    }
}

fn first_non_whitespace_char(inl: &Inline) -> Option<char> {
    match inl {
        Inline::Math(_) => Some('x'),
        _ => inline_text_repr(inl).and_then(|t| t.chars().find(|c| !c.is_whitespace())),
    }
}

/// 把 `Inline` 序列展开成排版片段；公式渲染失败时降级为等宽源码文本。
fn flatten(inlines: &[Inline], base: TextStyle, ctx: &mut RenderCtx) -> Vec<Piece> {
    let mut out = Vec::with_capacity(inlines.len());
    let push_text = |out: &mut Vec<Piece>, text: String, style: TextStyle| {
        if text.is_empty() {
            return;
        }
        if let Some(Piece::Text { text: prev, style: st }) = out.last_mut() {
            if *st == style {
                prev.push_str(&text);
                return;
            }
        }
        out.push(Piece::Text { text, style });
    };

    for (idx, inl) in inlines.iter().enumerate() {
        match inl {
            Inline::Text(t) => push_text(&mut out, t.clone(), base),
            // 注意：没有斜体字面时不做合成倾斜，Emphasis 按常规字重绘制。
            Inline::Emphasis(t) => push_text(&mut out, t.clone(), base),
            Inline::Strong(t) => push_text(&mut out, t.clone(), TextStyle { bold: true, ..base }),
            Inline::Code(t) => push_text(&mut out, t.clone(), TextStyle { mono: true, ..base }),
            Inline::Link { text, url } => push_text(
                &mut out,
                link_display_text(text, url),
                TextStyle { link: true, ..base },
            ),
            Inline::FootnoteRef(label) => push_text(&mut out, format!("[{label}]"), base),
            Inline::Math(latex) => match ctx.formula(latex, false) {
                Some(f) => {
                    let left_gap = if needs_gap_before_math(inlines.get(idx.wrapping_sub(1)), idx) {
                        INLINE_MATH_GAP_MM
                    } else {
                        0.0
                    };
                    let right_gap = if needs_gap_after_math(inlines.get(idx + 1)) {
                        INLINE_MATH_GAP_MM
                    } else {
                        0.0
                    };
                    out.push(Piece::Math {
                        f,
                        left_gap,
                        right_gap,
                    });
                }
                None => push_text(
                    &mut out,
                    format!(" {latex} "),
                    TextStyle { mono: true, ..base },
                ),
            },
        }
    }
    out
}

// ---------------------------------------------------------------------------
// 主流程
// ---------------------------------------------------------------------------

/// 从 Markdown 生成 PDF 字节。
pub fn markdown_to_pdf_bytes(markdown: &str) -> Result<Vec<u8>> {
    Ok(markdown_to_pdf(markdown, &PdfOptions::default())?.bytes)
}

/// 便捷函数：直接写到磁盘。
pub fn markdown_to_pdf_file(markdown: &str, path: impl AsRef<std::path::Path>) -> Result<()> {
    let bytes = markdown_to_pdf_bytes(markdown)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// 从 Markdown 生成 PDF，并返回降级告警。
pub fn markdown_to_pdf(markdown: &str, opts: &PdfOptions) -> Result<PdfOutput> {
    use printpdf::{BuiltinFont, Mm, PdfDocument};

    let blocks = parse_document(markdown);

    // 字体：找不到就明确报错，不静默回退到内置 Helvetica。
    let regular_path = find_font(opts.font_path.as_deref(), false).ok_or(JmisformuError::FontNotFound)?;
    let regular_data = std::fs::read(&regular_path)?;
    let bold_path = find_font(opts.bold_font_path.as_deref(), true);
    let bold_data = bold_path.as_ref().and_then(|p| std::fs::read(p).ok());

    let metrics = Metrics {
        regular: FaceMetrics::new(&regular_data)?,
        bold: bold_data.as_deref().and_then(|d| FaceMetrics::new(d).ok()),
    };

    let (doc, page1, layer1) = PdfDocument::new("Math Export", Mm(PAGE_W), Mm(PAGE_H), "Layer 1");
    let regular = doc
        .add_external_font(std::io::Cursor::new(&regular_data))
        .map_err(|e| JmisformuError::Pdf(e.to_string()))?;
    let bold = if metrics.bold.is_some() {
        bold_data
            .as_ref()
            .and_then(|d| doc.add_external_font(std::io::Cursor::new(d)).ok())
    } else {
        None
    };
    let mono = doc
        .add_builtin_font(BuiltinFont::Courier)
        .map_err(|e| JmisformuError::Pdf(e.to_string()))?;
    let fonts = Fonts {
        regular,
        bold,
        mono,
    };

    let mut ctx = RenderCtx::new();
    #[cfg(feature = "parallel")]
    prerender_formulas(&blocks, &mut ctx);

    let mut cur = Cursor {
        doc: &doc,
        layer: doc.get_page(page1).get_layer(layer1),
        y: Cursor::TOP,
    };

    render_blocks(&mut cur, &blocks, &fonts, &metrics, &mut ctx);

    let bytes = doc
        .save_to_bytes()
        .map_err(|e| JmisformuError::Pdf(e.to_string()))?;
    Ok(PdfOutput {
        bytes,
        warnings: ctx.warnings,
    })
}

/// 并行预渲染所有去重后的公式，填充缓存。绘制阶段仍是串行的
/// （printpdf 的 `PdfLayerReference` 不是 `Sync`）。
#[cfg(feature = "parallel")]
fn prerender_formulas(blocks: &[Block], ctx: &mut RenderCtx) {
    use rayon::prelude::*;

    fn collect(inlines: &[Inline], keys: &mut Vec<(String, bool)>) {
        for i in inlines {
            if let Inline::Math(m) = i {
                keys.push((m.clone(), false));
            }
        }
    }

    let mut keys: Vec<(String, bool)> = Vec::new();
    for b in blocks {
        match b {
            Block::Heading(_, i)
            | Block::Paragraph(i)
            | Block::Quote(i)
            | Block::ListItem { content: i, .. }
            | Block::FootnoteDefinition { content: i, .. } => collect(i, &mut keys),
            Block::DisplayMath(m) => keys.push((m.clone(), true)),
            Block::Table { header, rows } => {
                for c in header {
                    collect(c, &mut keys);
                }
                for r in rows {
                    for c in r {
                        collect(c, &mut keys);
                    }
                }
            }
            _ => {}
        }
    }
    keys.sort();
    keys.dedup();

    let rendered: Vec<((String, bool), Option<Arc<RenderedFormula>>)> = keys
        .into_par_iter()
        .map(|k| {
            let v = render_formula(&k.0, k.1).ok().map(Arc::new);
            (k, v)
        })
        .collect();

    for (k, v) in rendered {
        if v.is_none() {
            ctx.warnings.push(Warning {
                source: k.0.clone(),
                message: "公式渲染失败，已降级为源码文本".into(),
            });
        }
        ctx.cache.insert(k, v);
    }
}

fn render_blocks(
    cur: &mut Cursor,
    blocks: &[Block],
    fonts: &Fonts,
    m: &Metrics,
    ctx: &mut RenderCtx,
) {
    for block in blocks {
        match block {
            Block::Heading(level, inlines) => {
                let size = match level {
                    1 => 19.0,
                    2 => 15.0,
                    3 => 13.0,
                    _ => 11.5,
                };
                // 标题同样走完整行布局：既能折行，也能正确渲染标题里的公式
                // （旧版直接 use_text(inlines_plain(..))，长标题冲出页面，
                //  公式则被打印成原始 LaTeX 源码）。
                let pieces = flatten(inlines, TextStyle { bold: true, ..Default::default() }, ctx);
                let lines = layout_lines(&pieces, CONTENT_W, size, m);
                cur.ensure(lines.first().map_or(size * 0.6, |l| l.height) + size * 0.5);
                cur.y -= size * 0.5;
                draw_lines(cur, &lines, MARGIN_L, size, fonts);
                cur.y -= 2.0;
            }
            Block::Paragraph(inlines) => {
                let pieces = flatten(inlines, TextStyle::default(), ctx);
                let lines = layout_lines(&pieces, CONTENT_W, BODY_SIZE, m);
                draw_lines(cur, &lines, MARGIN_L, BODY_SIZE, fonts);
                cur.y -= 2.0;
            }
            Block::DisplayMath(latex) => render_display_math(cur, latex, fonts, m, ctx),
            Block::ListItem {
                ordered,
                depth,
                index,
                content,
                ..
            } => {
                let indent = MARGIN_L + 4.0 + f32::from(*depth) * 6.0;
                // 有序列表要显示真实序号；旧版对 ordered 用「•」、对无序用「-」，正好反了
                let marker = if *ordered {
                    format!("{index}.")
                } else {
                    match depth % 3 {
                        0 => "•".to_string(),
                        1 => "◦".to_string(),
                        _ => "▪".to_string(),
                    }
                };
                let marker_w = m.text_width_mm(&marker, BODY_SIZE, TextStyle::default());
                let text_x = indent + marker_w.max(4.0) + 1.5;
                let pieces = flatten(content, TextStyle::default(), ctx);
                let lines = layout_lines(&pieces, PAGE_W - MARGIN_R - text_x, BODY_SIZE, m);
                cur.ensure(lines.first().map_or(6.0, |l| l.height));
                draw_lines_at(
                    &cur.layer,
                    &[Line {
                        items: vec![Item::Text(PlacedText {
                            x: 0.0,
                            text: marker,
                            style: TextStyle::default(),
                        })],
                        height: 0.0,
                    }],
                    indent,
                    cur.y,
                    BODY_SIZE,
                    fonts,
                );
                draw_lines(cur, &lines, text_x, BODY_SIZE, fonts);
                cur.y -= 1.5;
            }
            Block::Quote(inlines) => {
                let pieces = flatten(inlines, TextStyle::default(), ctx);
                let lines = layout_lines(&pieces, CONTENT_W - 6.0, BODY_SIZE, m);
                draw_lines(cur, &lines, MARGIN_L + 6.0, BODY_SIZE, fonts);
                cur.y -= 2.0;
            }
            Block::CodeBlock(_lang, code) => render_code_block(cur, code, fonts, m),
            Block::Table { header, rows } => render_table(cur, header, rows, fonts, m, ctx),
            Block::FootnoteDefinition { label, content } => {
                let mut all = vec![Inline::Text(format!("[{label}] "))];
                all.extend(content.iter().cloned());
                let pieces = flatten(&all, TextStyle::default(), ctx);
                let lines = layout_lines(&pieces, CONTENT_W - 4.0, 9.5, m);
                draw_lines(cur, &lines, MARGIN_L + 4.0, 9.5, fonts);
                cur.y -= 1.5;
            }
            Block::Rule => {
                use printpdf::{Color, Greyscale, Line as PLine, Mm, Point};
                cur.ensure(5.0);
                cur.y -= 2.0;
                cur.layer
                    .set_outline_color(Color::Greyscale(Greyscale::new(0.65, None)));
                cur.layer.set_outline_thickness(0.35);
                let mut line = PLine::from_iter(vec![
                    (Point::new(Mm(MARGIN_L), Mm(cur.y)), false),
                    (Point::new(Mm(PAGE_W - MARGIN_R), Mm(cur.y)), false),
                ]);
                line.set_closed(false);
                cur.layer.add_line(line);
                cur.y -= 3.0;
            }
        }
    }
}

fn render_display_math(
    cur: &mut Cursor,
    latex: &str,
    fonts: &Fonts,
    m: &Metrics,
    ctx: &mut RenderCtx,
) {
    let Some(f) = ctx.formula(latex, true) else {
        // 降级：把原始 LaTeX 当等宽文本画出来，而不是让整篇导出失败
        let pieces = vec![Piece::Text {
            text: latex.to_string(),
            style: TextStyle {
                mono: true,
                ..Default::default()
            },
        }];
        let lines = layout_lines(&pieces, CONTENT_W, CODE_SIZE, m);
        draw_lines(cur, &lines, MARGIN_L, CODE_SIZE, fonts);
        cur.y -= 2.0;
        return;
    };

    // 过宽 / 过高的公式按比例缩放，避免冲出版心或被截断
    let mut scale = 1.0f32;
    if f.width_mm > CONTENT_W && f.width_mm > 0.0 {
        scale = CONTENT_W / f.width_mm;
    }
    let raw_h = f.height_mm + f.depth_mm;
    let max_h = Cursor::USABLE - 8.0;
    if raw_h * scale > max_h && raw_h > 0.0 {
        scale = scale.min(max_h / raw_h);
    }
    let h = raw_h * scale;
    let w = f.width_mm * scale;

    cur.ensure(h + 8.0);
    cur.y -= 4.0;
    let bottom = cur.y - h;
    let x = MARGIN_L + (CONTENT_W - w).max(0.0) / 2.0;
    place_svg(&cur.layer, &f.svg, x, bottom, scale);

    // 公式编号 \tag{N} -> 右侧 "(N)"，与公式基线对齐
    if let (_, Some(n)) = crate::latex::extract_tag(latex) {
        let label = format!("({n})");
        let label_w = m.text_width_mm(&label, BODY_SIZE, TextStyle::default());
        let lx = MARGIN_L + CONTENT_W - label_w;
        let ly = bottom + f.depth_mm * scale;
        draw_lines_at(
            &cur.layer,
            &[Line {
                items: vec![Item::Text(PlacedText {
                    x: 0.0,
                    text: label,
                    style: TextStyle::default(),
                })],
                height: 0.0,
            }],
            lx,
            ly,
            BODY_SIZE,
            fonts,
        );
    }
    cur.y = bottom - 4.0;
}

fn draw_code_bg(
    layer: &printpdf::PdfLayerReference,
    x_mm: f32,
    top_mm: f32,
    width_mm: f32,
    height_mm: f32,
) {
    use printpdf::{path, Color, Greyscale, Mm, Point, Polygon};
    layer.set_fill_color(Color::Greyscale(Greyscale::new(0.95, None)));
    let poly = Polygon {
        rings: vec![vec![
            (Point::new(Mm(x_mm), Mm(top_mm)), false),
            (Point::new(Mm(x_mm + width_mm), Mm(top_mm)), false),
            (Point::new(Mm(x_mm + width_mm), Mm(top_mm - height_mm)), false),
            (Point::new(Mm(x_mm), Mm(top_mm - height_mm)), false),
        ]],
        mode: path::PaintMode::Fill,
        winding_order: path::WindingOrder::NonZero,
    };
    layer.add_polygon(poly);
    // printpdf 的填充色是图层级粘性状态：画完灰底必须复位为黑色，
    // 否则随后所有 use_text 的字形都会继承 0.95 灰，看起来像「空灰框盖住文字」。
    layer.set_fill_color(Color::Greyscale(Greyscale::new(0.0, None)));
}

fn render_code_block(cur: &mut Cursor, code: &str, fonts: &Fonts, m: &Metrics) {
    let pieces = vec![Piece::Text {
        text: code.trim_end_matches('\n').to_string(),
        style: TextStyle {
            mono: true,
            ..Default::default()
        },
    }];
    let lines = layout_lines(&pieces, CONTENT_W - 8.0, CODE_SIZE, m);

    let mut i = 0usize;
    while i < lines.len() {
        cur.ensure(lines[i].height + 3.0);
        // 本页能放下多少行
        let mut n = 0usize;
        let mut h = 0.0f32;
        while i + n < lines.len() && cur.y - (h + lines[i + n].height) >= Cursor::BOTTOM {
            h += lines[i + n].height;
            n += 1;
        }
        if n == 0 {
            // 单行高于整页：硬画一行，避免死循环
            n = 1;
            h = lines[i].height;
        }
        draw_code_bg(&cur.layer, MARGIN_L + 1.0, cur.y + CODE_SIZE * PT_TO_MM, CONTENT_W - 2.0, h + 1.5);
        draw_lines_at(&cur.layer, &lines[i..i + n], MARGIN_L + 4.0, cur.y, CODE_SIZE, fonts);
        cur.y -= h + 2.0;
        i += n;
        if i < lines.len() {
            cur.new_page();
        }
    }
    cur.y -= 1.0;
}

// ---------------------------------------------------------------------------
// 表格（支持跨页 + 逐页重绘表头）
// ---------------------------------------------------------------------------

struct RowLayout {
    cells: Vec<Vec<Line>>,
    height: f32,
}

fn render_table(
    cur: &mut Cursor,
    header: &[Vec<Inline>],
    rows: &[Vec<Vec<Inline>>],
    fonts: &Fonts,
    m: &Metrics,
    ctx: &mut RenderCtx,
) {
    use printpdf::{Color, Greyscale, Line as PLine, Mm, Point};

    let col_count = header.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
    if col_count == 0 {
        return;
    }

    let cell_pad_x = 2.0f32;
    let min_col_w = 18.0f32;
    let font_size = BODY_SIZE;

    // 展开所有单元格（公式已被缓存，重复展开不会重复渲染）
    let header_pieces: Vec<Vec<Piece>> = header
        .iter()
        .map(|c| flatten(c, TextStyle { bold: true, ..Default::default() }, ctx))
        .collect();
    let row_pieces: Vec<Vec<Vec<Piece>>> = rows
        .iter()
        .map(|r| {
            r.iter()
                .map(|c| flatten(c, TextStyle::default(), ctx))
                .collect()
        })
        .collect();

    // 列宽
    let mut col_widths = vec![min_col_w; col_count];
    for (i, c) in header_pieces.iter().enumerate() {
        col_widths[i] = col_widths[i].max(natural_width(c, font_size, m) + cell_pad_x * 2.0);
    }
    for r in &row_pieces {
        for (i, c) in r.iter().enumerate() {
            col_widths[i] = col_widths[i].max(natural_width(c, font_size, m) + cell_pad_x * 2.0);
        }
    }
    let total: f32 = col_widths.iter().sum();
    if total > CONTENT_W {
        let k = CONTENT_W / total;
        for w in &mut col_widths {
            *w = (*w * k).max(14.0);
        }
    }

    // 行布局
    let build_row = |cells: &[Vec<Piece>]| -> RowLayout {
        let mut laid = Vec::with_capacity(col_count);
        let mut h: f32 = 7.0;
        for i in 0..col_count {
            let empty: Vec<Piece> = Vec::new();
            let p = cells.get(i).unwrap_or(&empty);
            let w = (col_widths[i] - cell_pad_x * 2.0).max(6.0);
            let lines = layout_lines(p, w, font_size, m);
            h = h.max(lines_height(&lines) + 2.6);
            laid.push(lines);
        }
        RowLayout {
            cells: laid,
            height: h,
        }
    };

    let header_row = (!header.is_empty()).then(|| build_row(&header_pieces));
    let body_rows: Vec<RowLayout> = row_pieces.iter().map(|r| build_row(r)).collect();

    let table_left = MARGIN_L;
    let table_w: f32 = col_widths.iter().sum();
    let mut x_positions = Vec::with_capacity(col_count + 1);
    let mut acc = table_left;
    x_positions.push(acc);
    for w in &col_widths {
        acc += *w;
        x_positions.push(acc);
    }

    let draw_row = |cur: &mut Cursor, row: &RowLayout| {
        for (i, lines) in row.cells.iter().enumerate() {
            draw_lines_at(
                &cur.layer,
                lines,
                x_positions[i] + cell_pad_x,
                cur.y - 4.8,
                font_size,
                fonts,
            );
        }
        cur.y -= row.height;
    };

    let mut i = 0usize;
    loop {
        let hdr_h = header_row.as_ref().map_or(0.0, |h| h.height);
        let first_h = body_rows.get(i).map_or(0.0, |r| r.height);
        cur.ensure(hdr_h + first_h);

        let seg_top = cur.y;
        let mut seg_heights: Vec<f32> = Vec::new();

        if let Some(h) = &header_row {
            draw_row(cur, h);
            seg_heights.push(h.height);
        }
        let seg_start = i;
        while i < body_rows.len() {
            let r = &body_rows[i];
            if cur.y - r.height < Cursor::BOTTOM {
                break;
            }
            draw_row(cur, r);
            seg_heights.push(r.height);
            i += 1;
        }
        // 单行高于整页：硬画一行，避免死循环
        if i == seg_start && i < body_rows.len() {
            let r = &body_rows[i];
            draw_row(cur, r);
            seg_heights.push(r.height);
            i += 1;
        }

        // 本段边框
        let seg_h: f32 = seg_heights.iter().sum();
        cur.layer
            .set_outline_color(Color::Greyscale(Greyscale::new(0.55, None)));
        cur.layer.set_outline_thickness(0.4);
        for x in &x_positions {
            let mut l = PLine::from_iter(vec![
                (Point::new(Mm(*x), Mm(seg_top)), false),
                (Point::new(Mm(*x), Mm(seg_top - seg_h)), false),
            ]);
            l.set_closed(false);
            cur.layer.add_line(l);
        }
        let mut yy = seg_top;
        for k in 0..=seg_heights.len() {
            let mut l = PLine::from_iter(vec![
                (Point::new(Mm(table_left), Mm(yy)), false),
                (Point::new(Mm(table_left + table_w), Mm(yy)), false),
            ]);
            l.set_closed(false);
            cur.layer.add_line(l);
            if k < seg_heights.len() {
                yy -= seg_heights[k];
            }
        }

        if i >= body_rows.len() {
            break;
        }
        cur.new_page();
    }
    cur.y -= 2.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn math_gets_gap_between_cjk_text() {
        assert!(needs_gap_before_math(Some(&Inline::Text("质量".into())), 1));
        assert!(needs_gap_after_math(Some(&Inline::Text("是能量".into()))));
    }

    #[test]
    fn math_does_not_get_gap_next_to_punctuation() {
        assert!(!needs_gap_before_math(Some(&Inline::Text("(".into())), 1));
        assert!(!needs_gap_after_math(Some(&Inline::Text("，继续".into()))));
        assert!(!needs_gap_after_math(Some(&Inline::Text(")".into()))));
    }

    #[test]
    fn cursor_geometry_is_sane() {
        assert!(Cursor::USABLE > 200.0);
        assert!(Cursor::TOP < PAGE_H);
        assert!(Cursor::BOTTOM > 0.0);
    }
}
