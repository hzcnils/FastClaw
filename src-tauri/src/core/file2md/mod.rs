// FastClaw · File2Md OCR 管理器
// 负责对图片进行 CLI 模式的 OCR 文字提取
// 采用按需调用模式（每次调用启动 CLI 进程，完成后清理临时文件）

use crate::utils::paths;
use anyhow::{Context, Result};
use base64::Engine;
use std::path::PathBuf;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

pub struct File2MdManager;

impl File2MdManager {
    pub fn new() -> Self {
        Self
    }

    /// 对 base64 图片进行 OCR，返回 Markdown 文本
    pub async fn ocr_image(&self, base64_data: &str) -> Result<String> {
        // 1. 解析 base64 data URL
        let (mime_type, bytes) = Self::decode_base64_data_url(base64_data)?;

        // 2. 确定文件扩展名
        let ext = Self::mime_to_ext(&mime_type);

        // 3. 写入临时文件
        let input_path =
            std::env::temp_dir().join(format!("fastclaw_ocr_{}.{}", uuid::Uuid::new_v4(), ext));
        let output_path =
            std::env::temp_dir().join(format!("fastclaw_ocr_{}.md", uuid::Uuid::new_v4()));

        tokio::fs::write(&input_path, &bytes)
            .await
            .context("写入临时图片文件失败")?;

        // 4. 调用 CLI（带清理保证）
        let result = Self::run_cli(&input_path, &output_path).await;

        // 5. 清理临时文件（无论成功失败）
        let _ = tokio::fs::remove_file(&input_path).await;
        let ocr_text = match result {
            Ok(text) => text,
            Err(e) => {
                let _ = tokio::fs::remove_file(&output_path).await;
                return Err(e);
            }
        };
        let _ = tokio::fs::remove_file(&output_path).await;

        Ok(ocr_text)
    }

    /// 对本地图片文件进行 OCR（路径版，无需 base64 编解码）
    /// 注意：调用方负责传入文件的管理（OCR 完成后不会删除传入文件）
    pub async fn ocr_image_path(&self, image_path: &str) -> Result<String> {
        let input_path = std::path::PathBuf::from(image_path);
        if !input_path.exists() {
            return Err(anyhow::anyhow!("图片文件不存在: {}", image_path));
        }

        let output_path =
            std::env::temp_dir().join(format!("fastclaw_ocr_{}.md", uuid::Uuid::new_v4()));

        let result = Self::run_cli(&input_path, &output_path).await;

        let ocr_text = match result {
            Ok(text) => text,
            Err(e) => {
                let _ = tokio::fs::remove_file(&output_path).await;
                return Err(e);
            }
        };
        let _ = tokio::fs::remove_file(&output_path).await;

        Ok(ocr_text)
    }

    /// 解析 base64 data URL，返回 (MIME type, bytes)
    fn decode_base64_data_url(data_url: &str) -> Result<(String, Vec<u8>)> {
        // 格式: data:image/png;base64,xxxxx
        if let Some(comma_pos) = data_url.find(',') {
            let header = &data_url[..comma_pos];
            let base64_part = &data_url[comma_pos + 1..];

            // 从 header 提取 MIME type: "data:image/png;base64" -> "image/png"
            let mime = if let Some(colon_pos) = header.find(':') {
                let after_colon = &header[colon_pos + 1..];
                if let Some(semi_pos) = after_colon.find(';') {
                    after_colon[..semi_pos].to_string()
                } else {
                    after_colon.to_string()
                }
            } else {
                "image/png".to_string()
            };

            let bytes = base64::engine::general_purpose::STANDARD
                .decode(base64_part)
                .context("Base64 解码失败")?;

            Ok((mime, bytes))
        } else {
            // 没有逗号分隔符，尝试直接作为 base64 解码
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data_url)
                .context("Base64 解码失败（数据格式不正确，需要 data: URL 或纯 base64）")?;
            Ok(("image/png".to_string(), bytes))
        }
    }

    /// MIME type 到文件扩展名
    fn mime_to_ext(mime: &str) -> &str {
        match mime {
            "image/jpeg" | "image/jpg" => "jpg",
            "image/png" => "png",
            "image/webp" => "webp",
            "image/bmp" => "bmp",
            _ => "png", // 默认
        }
    }

    /// 运行 CLI：file2md-ocr-tiny.exe <input> <output>
    async fn run_cli(input_path: &PathBuf, output_path: &PathBuf) -> Result<String> {
        let exe_path = paths::file2md_ocr_exe_path();

        if !exe_path.exists() {
            return Err(anyhow::anyhow!(
                "File2Md OCR 可执行文件不存在: {}",
                exe_path.display()
            ));
        }

        let mut cmd = Command::new(&exe_path);
        cmd.arg(input_path.as_os_str());
        cmd.arg(output_path.as_os_str());

        // Windows: 隐藏控制台窗口
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        let child = cmd
            .spawn()
            .with_context(|| format!("启动 File2Md OCR 进程失败: {}", exe_path.display()))?;

        // 等待进程完成，60 秒超时
        let output = timeout(Duration::from_secs(60), child.wait_with_output())
            .await
            .context("File2Md OCR 进程超时（60秒）")?
            .context("File2Md OCR 进程异常退出")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!(
                "File2Md OCR 进程返回错误 (exit code: {:?}): {}",
                output.status.code(),
                stderr.trim()
            ));
        }

        // 读取输出文件
        let ocr_text = tokio::fs::read_to_string(output_path)
            .await
            .context("读取 OCR 输出文件失败")?;

        if ocr_text.trim().is_empty() {
            return Err(anyhow::anyhow!("OCR 结果为空，图片中可能没有可识别的文字"));
        }

        Ok(ocr_text)
    }
}
