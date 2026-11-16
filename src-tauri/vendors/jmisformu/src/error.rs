//! 统一错误类型。

use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum JmisformuError {
    #[error("LaTeX 解析/渲染失败: {0}")]
    Latex(String),

    #[error("MathML 解析失败: {0}")]
    Mathml(String),

    #[error("PDF 生成失败: {0}")]
    Pdf(String),

    #[error("DOCX 生成失败: {0}")]
    Docx(String),

    /// PDF 排版需要一份真实字体来取字形宽度；找不到时必须明确报错，
    /// 不能静默回退到内置 Helvetica —— 那样非 ASCII 字符会渲染成乱码。
    #[error(
        "未找到可用字体。请安装中文字体（如 Noto Sans CJK / 文泉驿），\
         或通过 ExportOptions::font_path 显式指定一个 .ttf/.otf/.ttc 文件"
    )]
    FontNotFound,

    #[error("字体解析失败: {0}")]
    FontParse(String),

    #[error("不支持的导出类型: {0}")]
    UnsupportedExportType(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, JmisformuError>;

/// 非致命问题：导出成功，但某处发生了降级。
///
/// 旧版本对公式渲染失败的处理是矛盾的 —— 块级公式用 `?` 让整篇导出失败，
/// 行内公式却 `if let Ok` 静默丢弃（公式凭空消失）。现在统一为「降级 + 记录」，
/// 由调用方决定是否呈现给用户。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// 出问题的原始 LaTeX / 内容片段。
    pub source: String,
    /// 人类可读的说明。
    pub message: String,
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.source, self.message)
    }
}
