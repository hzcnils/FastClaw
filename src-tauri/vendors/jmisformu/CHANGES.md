# jmisformu 修复说明（v0.1.0 → v0.2.0）

对应审查报告 `REVIEW.md` 的全部条目。**状态**列含义：
✅ 已修复并有回归测试 · ⚠️ 已修复但有已知局限 · ❌ 未修复

---

## 一、验证方式

本机 Ubuntu 只有 rustc **1.75**（crate 声明 MSRV 1.82），RaTeX 的传递依赖
`crypto-common 0.2.2` 需要 edition2024 拉不动。因此采用 **`pdf-ratex` feature 关闭**
的方式做全量编译验证：`pdf.rs` 的布局 / 分页 / 缓存 / 批量绘制引擎（本次改动的全部内容）
仍然逐行过类型检查，只有约 30 行 RaTeX 调用走桩。

| 项目 | 结果 |
|---|---|
| `cargo test` | **73 单元 + 6 集成 + 2 文档测试，全部通过，零编译警告** |
| DOCX 产物（Python `zipfile` 独立校验） | CRC 通过；6 个 part 全部 deflate（`document.xml` 7798→1237 字节，≈6×）；DOS 日期 `1980-01-01` 合法；**所有 part 严格 XML 合法**；3 个独立 `w:num`；1 个真 `<w:hyperlink>`；`xmlns:m` 只声明 1 次 |
| PDF 产物（poppler / qpdf 独立校验） | `qpdf --check` 无错误；7 页；`pdftotext` 确认 40 段正文 / 80 行代码 / 60 行表格**一条不少**；`pdftoppm` 目视确认代码块灰底跨页、**表头逐页重绘**、表格闭合、有序列表显示 `1./2./3.` |
| 体积 | 开启 `font_subsetting` 后 **21 MB → 145 KB** |

**未跑成**：`cargo clippy`（Ubuntu 未打包 clippy）。

---

## 二、会话中新发现的问题（原报告没有）

这两条是实际编译运行后才暴露的，严重程度高于报告里的多数条目。

### N1【严重】pulldown-latex 输出的 MathML 不是合法 XML ✅

`pulldown-latex` 面向 **HTML 嵌入**设计，文本位置的 `<` `>` `&` 一律不转义：

```
a < b                →  <math ...><mi>a</mi><mo><</mo><mi>b</mi>...
\text{含 \& 与 < 的文本}  →  <mtext>含 \& 与 < 的文本</mtext>
```

HTML 解析器容忍这些，但 `roxmltree` 是 **XML** 解析器，直接报
`invalid name token` / `malformed entity reference`。

而旧代码 `mathml_to_omml(&mml).unwrap_or_else(|_| fallback_omml_inner(latex))`
把这个错误**吞掉了** —— 结果是：

> **任何含 `<` `>` `&` 的公式在 Word 里从来没有变成过真正的可编辑公式，一直是降级纯文本。**

这是个完全静默的质量缺陷，不跑起来看不出来。

**修复**：`latex.rs` 新增 `sanitize_mathml()` 三态状态机（Text / Tag / Attr），
只对文本位置和属性值内的裸 `<` `>` `&` 做转义；实体引用只放行 XML 预定义的 5 个
和数值引用（`&alpha;` 之类 XML 无定义，放行会变成 "unknown entity"）。
无需修补时返回 `Cow::Borrowed`，零分配。

**回归测试**：`mathml_output_is_valid_xml`、`bare_lt_is_escaped`、
`sanitizer_*` 系列，以及集成测试 `common_formulas_do_not_degrade`
（`$a < b$`、`$x > y$` 现在零告警产出真 OMML）。

### N2【中】`<merror>` 的错误说明被当作正文写进文档 ✅

`pulldown-latex` 用 `<merror>` 承载解析错误，内容是给人看的说明文字。
旧版 `omml.rs` 对未知元素透传子节点，于是用户会在 Word 正文里看到：

```
parsing error: alignment not allowed in current environment
╭─► context: ...
```

**修复**：`latex_to_mathml` 检测到 `<merror` 直接返回 `Err`，让调用方走
「原样显示 LaTeX 源码」的正常降级路径。回归测试 `merror_becomes_error_not_text`。

---

## 三、健壮性

| 编号 | 问题 | 状态 | 处理方式 |
|---|---|---|---|
| **R1** | `extract_tag` 在 `&[u8]` 上扫描却用字节下标切 `&str`，`bytes[i] as char` 把 UTF-8 续字节当 Latin-1（`0xA0`→NBSP 判为空白、`0xC0-0xFF` 判为字母）。`\tag` 后紧跟任意非 ASCII 即 panic | ✅ | 全部按 `char` 边界推进；无花括号形式只接受 **ASCII** 字母数字，顺带修掉「`\tag中文` 吞掉整段 CJK」的语义问题。回归测试覆盖 `\tag你` `\tag好` `\tag中文` `\tagé` `\tag½` `\tag{未闭合` 等 10 例 |
| **R2** | `esc()` 只转义 `& < >`，未过滤 XML 1.0 非法 C0 控制字符（Word 报「文件已损坏」）；未转义引号但结果被塞进 `m:val="..."` 属性；`docx.rs`/`omml.rs` 各有一份重复实现 | ✅ | 新建 `src/xml.rs`：单遍扫描，转义 5 个实体 + 丢弃非法字符，无特殊字符时零分配。两个模块统一引用 |
| **R3** | PDF `need!` 宏只在块开始前判断分页，`layout_inlines_in_box` 内换行只做 `*y -= line_h`，既无下沿检查也拿不到 `doc` 翻页 → 超过一页的段落/代码块/表格静默画到页面外 | ✅ | `pdf.rs` 改为两阶段：`layout_lines()` 纯布局产出 `Vec<Line>`（不涉页面不绘制）→ `draw_lines()` 逐行判断翻页。测量与绘制共用同一份逻辑，不会走偏。表格与代码块额外实现分页块 |
| **R4** | 块级公式失败用 `?` 中断整篇，行内公式 `if let Ok` 静默吞掉，行为矛盾 | ✅ | 统一为「降级 + 记录 `Warning`」。新增 `export_detailed()` / `ExportOutput.warnings` 让调用方拿到降级清单 |
| **R5** | `split_text_math` 无 `$` 邻接规则，`我花了 $5 买书，又花了 $10 吃饭` 被吃成公式 | ✅ | 加 CommonMark math 扩展的邻接规则：开 `$` 右邻非空白、闭 `$` 左邻非空白；`$$` 不受限。回归测试 `currency_amounts_are_not_math` |
| **R6** | RaTeX 是无 rev 的 git 依赖，构建不可复现 | ⚠️ | 四个 ratex 依赖全部钉死 `rev = "a0527b7b3b2b512063ea3d4607adf58322bdfcaf"`。**但**「项目跑着 pulldown-latex 与 ratex-parser 两个独立 LaTeX 前端、支持度可能分歧」这一点未收敛，也未加交叉一致性测试 |
| **R7** | feature 关闭时 PDF 静默产出把 LaTeX 塞进 `<text>`、宽度靠字符数估算的假 SVG | ✅ | `#[cfg(not(feature = "pdf-ratex"))] render_formula` 直接返回 `Err`。那不是降级，是静默产出错误文档 |
| **R8** | ZIP 时间戳全 0（1980 年 0 月 0 日，非法日期）、无 ZIP64 检查、无 UTF-8 flag | ✅ | 合法 DOS 时间戳 `0x0021`（1980-01-01）+ 通用标志位 bit 11（UTF-8 文件名）+ >4 GiB 显式报错 |
| **R9** | `from_str` 未实现 `FromStr` trait、`path: &str` 未用 `AsRef<Path>`、`anyhow` 声明未用 | ✅ | `impl std::str::FromStr for ExportType`；`export_to_file(path: impl AsRef<Path>)`；`#![forbid(unsafe_code)]`；`anyhow` 已从依赖删除。文档注明同步阻塞需 `spawn_blocking` |

---

## 四、格式保真

| 编号 | 问题 | 状态 | 处理方式 |
|---|---|---|---|
| **F1** | `m:limLoc` 硬编码 `undOvr`，`\int_a^b`（msubsup，限在右侧）应为 `subSup` | ✅ | `as_nary()` 区分：`msub/msup/msubsup` → `subSup`；`munder/mover/munderover` → `undOvr`。测试 `integral_uses_side_limits`、`sum_uses_under_over_limits` |
| **F2** | `emit_nary` 的 `<m:e></m:e>` 恒为空 —— 求和体在 MathML 里是大算符的**兄弟节点**，逐节点翻译取不到 | ✅ | `convert_seq` 在 mrow 层做前瞻：`nary_body_end()` 扫到关系符 / 低优先级运算符 / 另一个大算符为止，把体装进 `<m:e>`。测试 `nary_body_is_not_empty`、`nary_body_stops_at_equals` |
| **F3** | 丢失 `mathvariant`，Word 数学 run 默认斜体 → `\text{}`/`\mathrm{d}x`/`\sin`/数字全变斜体 | ✅ | `style_of()` → `m:sty`(p/b/i/bi)，`script_of()` → `m:scr`(double-struck/script/fraktur/sans-serif/monospace)。单字符 `mi` 默认斜体，多字符及 `mn`/`mo`/`mtext` 默认正体；`i` 是 Word 默认值故省略以减小体积 |
| **F4** | `node.text()` 只取第一个文本子节点，实体拆分会丢字 | ✅ | 改用 `descendants().filter(is_text)`（不能对 `descendants()` 直接 `filter_map(text)`，元素与其文本子节点会各贡献一次导致重复）。测试 `multi_text_node_content_is_not_lost` |
| **F5** | MathML Core 已移除 `mfenced`，该分支是死代码；括号退化成普通文本、不随内容伸缩 | ✅ | 在 mrow 层识别配对括号 → 生成带 `m:grow`/`m:shp` 的 `m:d`。带**平衡性检查**（`(a+b)(c+d)` 不会被合并）+ 对称定界符特判（`|x|`）。`mfenced` 分支保留以兼容其它 MathML 来源 |
| **F6** | `mover` 未走 nary 判断（与 munder/munderover 不一致） | ✅ | 统一在 `as_nary()` 中处理 |
| **F7** | DOCX 有序列表编号全写死 `"1. "`，Word 里显示成 1. 1. 1. | ✅ | 生成真正的 `word/numbering.xml`（abstractNum 0=bullet / 1=decimal，各 9 级），**每个列表实例一个 `w:num`**，多个有序列表各自从 1 开始。为此在 `doc_model` 的 `Block::ListItem` 新增 `list_id` / `index` 字段 |
| **F8** | 超链接渲染成 `文字 <url>` 纯文本 | ✅ | 动态生成 `document.xml.rels`（rId1=styles、rId2=numbering、rId3+ = hyperlink，`TargetMode="External"`），正文用 `<w:hyperlink r:id>`。URL 中的 `&` 正确转义 |
| **F9** | 公式编号制表位 `9600` 超出正文宽度 `9026`（11906 − 2×1440） | ✅ | 改用常量 `CONTENT_W_TWIPS = PAGE_W - 2*MARGIN` 计算 |
| **F10** | `text_run_styled` 对所有正文做隐式 `^x^`/`~x~` 上下标转换，会吃掉 `~/path/~`、GFM 删除线 | ✅ | 整套字符级魔法删除。上下标应由行内公式或 GFM 扩展表达 |
| **F11** | PDF 标题走 `inlines_plain` → 公式被打印成原始 LaTeX；且不换行会冲出页面 | ✅ | 标题走完整行布局，可折行、公式正确渲染 |
| **F12** | PDF 不区分粗体/斜体 | ⚠️ | **粗体已真实现**：单独探测粗体字面并单独加载度量（度量必须跟着字面走，否则折行位置会漂）。**斜体未实现** —— 无斜体字面时 `Emphasis` 按常规字重绘制，不做合成倾斜，已在代码注释说明 |
| **F13** | 无中文字体时静默回退 Helvetica → 乱码 | ✅ | 找不到字体返回 `JmisformuError::FontNotFound`（附可操作的提示文案），并新增 `PdfOptions::font_path` / `bold_font_path` 显式指定。候选表扩充 DejaVu / Liberation / FreeSans 兜底 |

> 关于 F13 的设计取舍：批量绘制要求「我们计算的宽度」与「PDF 实际字形推进量」一致。
> 内置 Helvetica 的度量拿不到，硬凑估算值会导致折行位置和公式落点全部偏移。
> 因此改为**必须有真字体**，这同时也是 F13 的正确解法。等宽字体只对 ASCII 生效
> （Courier 推进量恒为 0.6 em，与测量一致），CJK 自动换回正文字体。

---

## 五、性能

| 编号 | 问题 | 状态 | 处理方式 |
|---|---|---|---|
| **P1** | `CjkMetrics::char_width_mm` **每个字符**重新 `Face::parse` 整份字体（2 万字中文 = 2 万次完整字体解析） | ✅ | `FaceMetrics` 持有已解析的 `ttf_parser::Face` + `RefCell<HashMap<char, Option<u16>>>` 推进量缓存，字体只解析一次 |
| **P2** | 表格里一条公式被 RaTeX 渲染 **5 次**（`estimate_table_height_mm` 与 `render_table` 各自完整跑一遍 `compute_table_metrics`） | ✅ | `RenderCtx` 按 `(latex, display)` memo；`None` 表示已失败不重试。两阶段架构本身也消除了「测量时重算一遍」的结构性重复 |
| **P3** | 逐字符 `use_text`，每字符写一整组 BT/Tf/Td/Tj/ET 算子（约 10× 膨胀） | ✅ | 同一行、同一字体的连续字符合并成一次调用 |
| **P4** | 五个未用依赖：`docx-rs`、`svg2pdf`、`pdf-writer`、`anyhow`、`serde_json` | ✅ | 全部删除（`svg2pdf`/`pdf-writer` 由 printpdf 的 svg feature 传递引入，无需直接声明） |
| **P5** | `push_str(&format!())` 满地、`esc` 三次 replace、每条公式重复声明 `xmlns:m`（63 字节 × N） | ✅ | 全面改用 `write!`；`esc` 单遍扫描；`xmlns:m` 只在 `<w:document>` 根节点声明一次 |
| **P6** | ZIP 用 STORE 不压缩 | ✅ | 改 deflate（压缩不划算时自动回退 STORE）。实测 `document.xml` 7798→1237 字节 |
| **P7** | `render_table` 循环内前缀和 O(n²)；`latex_to_readable_symbols` 约 90 次连续 replace + `while contains("  ")` | ✅ | 列位置预计算成 `x_positions`；`latex_to_readable_symbols` 改单遍扫描 + `symbol()` 查表 + `split_whitespace().join()` |
| **P8** | 未用并行 | ✅ | 新增可选 `parallel` feature：`prerender_formulas()` 用 rayon 并行预渲染**去重后**的公式填充缓存（绘制阶段仍串行，`PdfLayerReference` 非 `Sync`）。为此 `RenderedFormula` 的句柄从 `Rc` 改为 `Arc` |

---

## 六、其它已知局限（未修，如实告知）

1. **`pdf-ratex` 开启时的实机编译运行未验证**。本机 rustc 1.75 拉不动 RaTeX 传递依赖。
   `render_formula` 内部的 RaTeX API 调用逻辑基本保持原样未改动，风险较低，但
   `PdfOptions` / `Arc` / `prerender_formulas` 这几条新路径建议在 rustc ≥ 1.82 环境
   跑一次 `cargo test --all-features` 确认。
2. **`cargo clippy` 未跑**（Ubuntu 未打包 clippy）。
3. `normalize_latex_delimiters` 只识别围栏代码块（``` / ~~~），**不识别 4 空格缩进代码块** —— 缩进代码块里的 `\[` `\(` 仍会被误替换成 `$$` `$`。已在函数文档注明。
4. PDF 斜体降级为常规字重（见 F12）。
5. R6 的「双 LaTeX 前端支持度分歧」只做了 rev 钉死，未收敛到单一前端，也未加交叉一致性测试。

---

## 七、公开 API 变化（v0.1 → v0.2）

新增：

```rust
pub struct ExportOutput { pub bytes: Vec<u8>, pub warnings: Vec<Warning> }
pub fn export_detailed(md: &str, kind: ExportType) -> Result<ExportOutput>;
pub fn export_to_file(md: &str, kind: ExportType, path: impl AsRef<Path>) -> Result<()>;
impl std::str::FromStr for ExportType;

pub struct pdf::PdfOptions { pub font_path: Option<PathBuf>, pub bold_font_path: Option<PathBuf> }
pub fn pdf::markdown_to_pdf(md: &str, opts: &PdfOptions) -> Result<PdfOutput>;
pub fn docx::markdown_to_docx(md: &str) -> Result<DocxOutput>;
pub mod xml;                       // 共用 XML 转义
pub struct error::Warning;         // 降级上报
```

破坏性变化：

- `JmisformuError` 标记 `#[non_exhaustive]`，新增 `FontNotFound` / `FontParse` 变体。
- `doc_model::Block::ListItem` 新增 `list_id: u32` 与 `index: u64` 字段。
- PDF 导出在**找不到任何可用字体时会返回 `Err`**（旧版静默回退 Helvetica 出乱码）。
- feature `pdf-ratex` 关闭时 `render_formula` 返回 `Err`（旧版返回假 SVG）。
- 新增可选 feature `parallel`。

`export()` / `markdown_to_pdf_bytes()` / `markdown_to_docx_bytes()` 签名保持不变。
