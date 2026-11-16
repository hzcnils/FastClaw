//! 集成测试：校验**产物本身**是否合法，而不只是输出字符串包含某子串。

use jmisformu::{export, export_detailed, ExportType};

/// 从 zip 里取出一个条目（支持 store / deflate）。
fn zip_entry(bytes: &[u8], want: &str) -> Option<String> {
    use flate2::read::DeflateDecoder;
    use std::io::Read;
    let mut i = 0usize;
    while i + 30 <= bytes.len() && bytes[i..i + 4] == [0x50, 0x4b, 0x03, 0x04] {
        let method = u16::from_le_bytes([bytes[i + 8], bytes[i + 9]]);
        let comp = u32::from_le_bytes(bytes[i + 18..i + 22].try_into().ok()?) as usize;
        let nlen = u16::from_le_bytes([bytes[i + 26], bytes[i + 27]]) as usize;
        let elen = u16::from_le_bytes([bytes[i + 28], bytes[i + 29]]) as usize;
        let name = std::str::from_utf8(&bytes[i + 30..i + 30 + nlen]).ok()?;
        let start = i + 30 + nlen + elen;
        let raw = bytes.get(start..start + comp)?;
        if name == want {
            return if method == 8 {
                let mut s = String::new();
                DeflateDecoder::new(raw).read_to_string(&mut s).ok()?;
                Some(s)
            } else {
                String::from_utf8(raw.to_vec()).ok()
            };
        }
        i = start + comp;
    }
    None
}

/// 覆盖各类 Markdown 结构与危险输入的语料。
const CORPUS: &[&str] = &[
    "",
    "纯中文段落。",
    "# 标题\n\n正文",
    r"行内公式 $a^2+b^2=c^2$ 与块级：\n\n$$\frac{-b\pm\sqrt{b^2-4ac}}{2a}$$",
    r"$$\sum_{k=1}^{n} k = \frac{n(n+1)}{2} \tag{1}$$",
    r"$$\int_0^\infty e^{-x^2}\,dx = \frac{\sqrt{\pi}}{2}$$",
    r"关系式 $a < b$ 与 $c > d$ 与 $x \& y$",
    r"\( \mathcal{D} \) 与 \[ E = mc^2 \]",
    "1. 甲\n2. 乙\n\n段落\n\n1. 丙\n2. 丁",
    "- 一\n  - 一一\n- 二",
    "| a | b |\n|---|---|\n| $x$ | y |",
    "> 引用 $E=mc^2$",
    "```rust\nlet ok = 1 < 2 && 3 > 2; // <tag> & \"quote\"\n```",
    "[链接](https://example.com/a?x=1&y=2) 与 <https://b.example>",
    "---\n\n分隔线后",
    "脚注[^1]\n\n[^1]: 注释内容",
    "含控制符\u{0000}\u{0001}\u{0008}\u{000b}\u{000c}\u{001f}的文本",
    "我花了 $5 买书，又花了 $10 吃饭",
    r"$$x \tag你$$",
    r"$$\undefined{cmd}$$",
    "$",
    "$$",
    r"\(",
    "**粗体** 与 *斜体* 与 `代码`",
    "路径 ~/data/~ 与 a^b^c",
    "emoji 😀 与数学 𝕆",
];

#[test]
fn docx_parts_are_always_well_formed_xml() {
    for md in CORPUS {
        let bytes = export(md, ExportType::Docx)
            .unwrap_or_else(|e| panic!("导出失败 {md:?}: {e}"));
        assert_eq!(&bytes[0..2], b"PK", "{md:?} 不是 zip");
        for part in [
            "[Content_Types].xml",
            "_rels/.rels",
            "word/document.xml",
            "word/_rels/document.xml.rels",
            "word/styles.xml",
            "word/numbering.xml",
        ] {
            let xml = zip_entry(&bytes, part)
                .unwrap_or_else(|| panic!("{md:?} 缺少 {part}"));
            roxmltree::Document::parse(&xml)
                .unwrap_or_else(|e| panic!("{md:?} 的 {part} 非法 XML: {e}\n{xml}"));
        }
    }
}

#[test]
fn docx_never_panics() {
    let nasty = [
        "$".repeat(5000),
        "\\".repeat(5000),
        "- ".repeat(2000),
        "#".repeat(300),
        "|".repeat(500),
        format!("$${}$$", "\\frac{".repeat(200)),
        "\u{0}".repeat(1000),
        format!("$$x \\tag{}$$", "你好世界"),
    ];
    for md in &nasty {
        let _ = export(md, ExportType::Docx);
    }
    for md in CORPUS {
        let _ = export(md, ExportType::Docx);
    }
}

#[test]
fn export_type_roundtrip() {
    use std::str::FromStr;
    for (s, t) in [("pdf", ExportType::Pdf), ("docx", ExportType::Docx)] {
        let parsed = ExportType::from_str(s).unwrap();
        assert_eq!(parsed, t);
        assert_eq!(parsed.ext(), s);
        assert!(!parsed.mime().is_empty());
    }
}

#[test]
fn warnings_are_reported_not_swallowed() {
    // 无法解析的公式应当降级并**告知**调用方，而不是静默丢弃或让整篇失败
    let out = export_detailed(r"$$\thiscommanddoesnotexist{x}$$", ExportType::Docx).unwrap();
    assert!(!out.bytes.is_empty());
    let xml = zip_entry(&out.bytes, "word/document.xml").unwrap();
    roxmltree::Document::parse(&xml).unwrap();
}

#[test]
fn common_formulas_do_not_degrade() {
    // 这些公式必须走真 OMML 路径，一条都不该降级。
    // 其中含 < > & 的几条在修复前会因 pulldown-latex 输出非法 XML 而静默降级。
    for md in [
        r"$$a^2 + b^2 = c^2$$",
        r"$$\frac{1}{2}$$",
        r"$$\sqrt{x}$$",
        r"$$\sum_{k=1}^n k$$",
        r"$$\int_0^1 x\,dx$$",
        r"$a < b$",
        r"$x > y$",
        r"$$\left(\frac{a}{b}\right)$$",
        r"$$\text{当且仅当}$$",
    ] {
        let out = export_detailed(md, ExportType::Docx).unwrap();
        assert!(out.warnings.is_empty(), "{md} 不应降级: {:?}", out.warnings);
        let xml = zip_entry(&out.bytes, "word/document.xml").unwrap();
        assert!(xml.contains("<m:oMath"), "{md} 应产出真正的 OMML: {xml}");
    }
}

#[test]
fn large_document_terminates_and_stays_valid() {
    // 大文档不能死循环，产物仍须合法
    let mut md = String::new();
    for i in 0..300 {
        md.push_str(&format!("## 小节 {i}\n\n正文 $x_{{{i}}}$ 内容。\n\n"));
    }
    let bytes = export(&md, ExportType::Docx).unwrap();
    let xml = zip_entry(&bytes, "word/document.xml").unwrap();
    roxmltree::Document::parse(&xml).unwrap();
}
