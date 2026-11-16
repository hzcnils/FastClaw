use jmisformu::{export, ExportType};

/// 导出 Markdown 为 PDF（含矢量公式渲染）。
/// 改为 async 并在独立阻塞线程执行，避免前端 UI 卡顿。
#[tauri::command]
pub async fn export_pdf(markdown: String) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || {
        export(&markdown, ExportType::Pdf).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 导出 Markdown 为 DOCX（含可编辑 OMML 公式）
#[tauri::command]
pub fn export_docx(markdown: String) -> Result<Vec<u8>, String> {
    export(&markdown, ExportType::Docx).map_err(|e| e.to_string())
}
