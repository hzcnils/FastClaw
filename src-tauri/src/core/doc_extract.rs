// FastClaw · 文档文本抽取（docx/xlsx → 纯文本）
//
// 复用项目已有的 `zip`（备份归档）+ `roxmltree`（XML 解析，传递依赖已缓存）两个依赖，
// 纯 Rust 解析 OOXML（docx/xlsx 本质是 zip 包内的一组 XML），无需任何外部进程/OCR。
//
// 用途：
//   1. 本地知识库（read_local_knowledge）：单文件 / 文件夹内的 docx、xlsx 抽取正文注入 prompt
//   2. 内置 read_file 工具：让模型能直接"读" docx/xlsx 文档

use std::io::Read;
use std::path::Path;

use anyhow::{bail, Context, Result};

/// 从 docx 提取纯文本（遍历 word/document.xml 中的 <w:t> 文本片段）
pub fn extract_docx_text(path: &Path) -> Result<String> {
    let mut archive = open_zip(path)?;
    let xml = read_zip_entry_xml(&mut archive, "word/document.xml")?;
    let doc = roxmltree::Document::parse(&xml).context("解析 docx XML 失败")?;

    let mut out = String::new();
    for node in doc.root().descendants() {
        if !node.is_element() {
            continue;
        }
        match node.tag_name().name() {
            // 段落边界（w:p / a:p）
            "p" => out.push('\n'),
            // 文本片段（w:t / a:t）
            "t" => out.push_str(&text_of(&node)),
            // 制表符 / 换行
            "tab" => out.push('\t'),
            "br" | "cr" => out.push('\n'),
            _ => {}
        }
    }

    let out = tidy(&out);
    if out.trim().is_empty() {
        bail!("docx 未提取到文本内容");
    }
    Ok(out)
}

/// 从 xlsx 提取纯文本（sharedStrings 共享字符串表 + 各工作表单元格）
pub fn extract_xlsx_text(path: &Path) -> Result<String> {
    let mut archive = open_zip(path)?;

    // 1. 读取共享字符串表（若存在）
    let mut shared: Vec<String> = Vec::new();
    if let Ok(xml) = read_zip_entry_xml(&mut archive, "xl/sharedStrings.xml") {
        if let Ok(doc) = roxmltree::Document::parse(&xml) {
            for node in doc.root().descendants() {
                if node.is_element() && node.tag_name().name() == "si" {
                    shared.push(text_of(&node));
                }
            }
        }
    }

    // 2. 收集所有工作表文件名（先克隆，避免与 archive 的借用冲突）
    let mut sheets: Vec<String> = archive
        .file_names()
        .filter(|n| n.starts_with("xl/worksheets/sheet") && n.ends_with(".xml"))
        .map(|s| s.to_string())
        .collect();
    // 按 sheet 后面的数字自然排序（sheet2 在 sheet10 之前）
    sheets.sort_by_key(|s| {
        s.trim_start_matches("xl/worksheets/sheet")
            .trim_end_matches(".xml")
            .parse::<u32>()
            .unwrap_or(0)
    });

    let mut out = String::new();
    for sheet in sheets {
        out.push_str(&format!("\n=== 工作表: {sheet} ===\n"));
        let xml = read_zip_entry_xml(&mut archive, &sheet)?;
        let doc = roxmltree::Document::parse(&xml)?;

        for row in doc
            .root()
            .descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "row")
        {
            let mut cells: Vec<String> = Vec::new();
            for c in row
                .children()
                .filter(|n| n.is_element() && n.tag_name().name() == "c")
            {
                cells.push(cell_value(&c, &shared));
            }
            let line = cells.join("\t");
            if !line.trim().is_empty() {
                out.push_str(&line);
                out.push('\n');
            }
        }
    }

    if out.trim().is_empty() {
        bail!("xlsx 未提取到文本内容");
    }
    Ok(out)
}

/// 按扩展名分发：支持 docx/xlsx；其它类型返回 None（交由调用方走纯文本读取）
pub fn extract_document_text(path: &Path) -> Option<String> {
    let ext = path.extension().and_then(|e| e.to_str())?.to_lowercase();
    match ext.as_str() {
        "docx" => extract_docx_text(path).ok(),
        "xlsx" => extract_xlsx_text(path).ok(),
        _ => None,
    }
}

// ── 内部工具函数 ──────────────────────────────────────────────────────

fn open_zip(path: &Path) -> Result<zip::ZipArchive<std::fs::File>> {
    let file = std::fs::File::open(path).context("打开文件失败")?;
    zip::ZipArchive::new(file).context("解压 zip（OOXML）失败")
}

fn read_zip_entry_xml(archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<String> {
    let mut entry = archive
        .by_name(name)
        .with_context(|| format!("zip 中缺少条目 {name}"))?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    Ok(decode_xml_bytes(&bytes))
}

/// 去除 BOM 后按 UTF-8 解码（OOXML 基本为 UTF-8）
fn decode_xml_bytes(bytes: &[u8]) -> String {
    let b = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        bytes
    };
    String::from_utf8_lossy(b).into_owned()
}

/// 收集节点下所有文本节点内容（比 node.text() 更稳，覆盖嵌套结构）
fn text_of(node: &roxmltree::Node) -> String {
    node.descendants()
        .filter(|n| n.is_text())
        .map(|n| n.text().unwrap_or(""))
        .collect()
}

/// 单元格值：区分共享字符串 / 内联字符串 / 数值等类型
fn cell_value(c: &roxmltree::Node, shared: &[String]) -> String {
    let t = c.attribute("t").unwrap_or("");
    match t {
        // 共享字符串：<v> 是 sharedStrings 的下标
        "s" => c
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "v")
            .and_then(|v| text_of(&v).trim().parse::<usize>().ok())
            .and_then(|idx| shared.get(idx))
            .cloned()
            .unwrap_or_default(),
        // 内联字符串：<is><t>text</t></is>
        "inlineStr" => c
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "is")
            .map(|is| text_of(&is))
            .unwrap_or_default(),
        // 数值 / 布尔 / 错误 / 公式字符串：直接取 <v>
        _ => c
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "v")
            .map(|v| text_of(&v))
            .unwrap_or_default(),
    }
}

/// 整理：去掉首尾空白、合并连续空行、去掉行尾空格
fn tidy(s: &str) -> String {
    let mut out = String::new();
    let mut blank = false;
    for line in s.lines() {
        let t = line.trim_end();
        if t.trim().is_empty() {
            if !blank {
                out.push('\n');
                blank = true;
            }
        } else {
            out.push_str(t);
            out.push('\n');
            blank = false;
        }
    }
    out.trim().to_string()
}
