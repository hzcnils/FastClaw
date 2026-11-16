//! # jmisformu
//!
//! 纯 Rust 后端：把含 LaTeX 公式的 Markdown 导出为
//!   1. **PDF**：公式为矢量图（RaTeX -> SVG -> 嵌入 PDF），放大不失真；
//!   2. **Word(.docx)**：公式为 OMML 原生公式，Word 双击可编辑。
//!
//! 全程不依赖前端、浏览器、Node、Pandoc。
//!
//! ## 技术链路
//! - LaTeX -> MathML：[`pulldown_latex`]（输出会被清洗成合法 XML，见 [`latex`] 模块）
//! - MathML -> OMML：自写纯 Rust 树映射器（见 [`omml`]）
//!   —— 不用微软 `MML2OMML.XSL`，因其为 XSLT 2.0，libxslt 只支持 1.0
//! - OMML -> DOCX：自写最小 OOXML 打包器（deflate 压缩）
//! - LaTeX -> SVG -> PDF：RaTeX + printpdf(svg)
//!
//! ## 顶层用法
//! ```no_run
//! use jmisformu::{export, ExportType};
//! let md = r"勾股定理：$$a^2 + b^2 = c^2$$";
//! let pdf  = export(md, ExportType::Pdf).unwrap();
//! let docx = export(md, ExportType::Docx).unwrap();
//! std::fs::write("out.pdf", pdf).unwrap();
//! std::fs::write("out.docx", docx).unwrap();
//! ```
//!
//! 需要知道「哪些公式降级了」时用 [`export_detailed`]：
//! ```no_run
//! # use jmisformu::{export_detailed, ExportType};
//! let out = export_detailed("$$\\badcmd$$", ExportType::Docx).unwrap();
//! for w in &out.warnings {
//!     eprintln!("降级: {w}");
//! }
//! ```
//!
//! ## 线程与阻塞
//! 所有导出函数都是**同步阻塞**的（公式渲染是 CPU 密集型）。在 async 服务里
//! 请用 `tokio::task::spawn_blocking` 之类包一层，否则会占死 runtime 线程。
//!
//! ## PDF 对字体的要求
//! PDF 排版需要一份真实字体来取字形宽度。找不到任何可用字体时会返回
//! [`JmisformuError::FontNotFound`]，而不是静默回退到内置 Helvetica
//! （那样非 ASCII 会渲染成乱码）。可用 [`pdf::PdfOptions::font_path`] 显式指定。

#![forbid(unsafe_code)]

pub mod doc_model;
pub mod docx;
pub mod error;
pub mod latex;
pub mod markdown;
pub mod omml;
pub mod pdf;
pub mod xml;

pub use error::{JmisformuError, Result, Warning};

use serde::{Deserialize, Serialize};

/// 导出目标类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportType {
    /// PDF（矢量公式）
    Pdf,
    /// Word .docx（可编辑 OMML 公式）
    Docx,
}

impl ExportType {
    #[must_use]
    pub fn mime(self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Docx => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            }
        }
    }

    #[must_use]
    pub fn ext(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Docx => "docx",
        }
    }
}

impl std::str::FromStr for ExportType {
    type Err = JmisformuError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "pdf" => Ok(Self::Pdf),
            "docx" | "word" => Ok(Self::Docx),
            other => Err(JmisformuError::UnsupportedExportType(other.to_string())),
        }
    }
}

/// 导出结果：字节 + 降级告警。
pub struct ExportOutput {
    pub bytes: Vec<u8>,
    /// 非致命降级（例如某条公式解析失败，已回退为纯文本）。
    pub warnings: Vec<Warning>,
}

/// **统一导出入口**：Markdown -> 字节。供 HTTP / MCP / CLI / Tauri command 共用。
pub fn export(markdown: &str, kind: ExportType) -> Result<Vec<u8>> {
    Ok(export_detailed(markdown, kind)?.bytes)
}

/// 同 [`export`]，但同时返回降级告警。
pub fn export_detailed(markdown: &str, kind: ExportType) -> Result<ExportOutput> {
    match kind {
        ExportType::Pdf => {
            let o = pdf::markdown_to_pdf(markdown, &pdf::PdfOptions::default())?;
            Ok(ExportOutput {
                bytes: o.bytes,
                warnings: o.warnings,
            })
        }
        ExportType::Docx => {
            let o = docx::markdown_to_docx(markdown)?;
            Ok(ExportOutput {
                bytes: o.bytes,
                warnings: o.warnings,
            })
        }
    }
}

/// 导出并写文件。
pub fn export_to_file(
    markdown: &str,
    kind: ExportType,
    path: impl AsRef<std::path::Path>,
) -> Result<()> {
    let bytes = export(markdown, kind)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// 仅转换单条公式 -> OMML 片段（供需要细粒度控制的调用方）。
pub fn latex_to_omml(latex: &str, display: bool) -> Result<String> {
    let mode = if display {
        latex::DisplayMode::Block
    } else {
        latex::DisplayMode::Inline
    };
    let mml = latex::latex_to_mathml(latex, mode)?;
    omml::mathml_to_omml(&mml)
}

/// 仅转换单条公式 -> SVG 矢量（供需要细粒度控制的调用方）。
pub fn latex_to_svg(latex: &str, display: bool) -> Result<String> {
    pdf::latex_to_svg(latex, display)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn export_type_from_str() {
        assert_eq!(ExportType::from_str("PDF").unwrap(), ExportType::Pdf);
        assert_eq!(ExportType::from_str("word").unwrap(), ExportType::Docx);
        assert!(ExportType::from_str("epub").is_err());
    }
}
