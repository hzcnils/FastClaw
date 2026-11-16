// FastClaw · 统一路径管理
// 所有项目中用到的路径都应在此文件中定义，方便统一管理和修改

use std::path::PathBuf;

/// 依赖服务使用的五位数首选端口。
///
/// 端口按服务预留 40 个连续候选，现有分配器仍会在首选端口被占用时逐位递增。
pub const QDRANT_DEFAULT_PORT: u16 = 23060;
pub const LLAMA_SERVER_DEFAULT_PORT: u16 = 23100;
pub const FASTKG_DEFAULT_PORT: u16 = 23140;
pub const FASTPLAN_DEFAULT_PORT: u16 = 23180;

/// 项目根目录（src-tauri/）
pub fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// 应用数据根目录（仅开发模式使用：{项目根}/data/）
fn data_root() -> PathBuf {
    project_root()
        .parent()
        .map(|p| p.join("data"))
        .unwrap_or_else(|| PathBuf::from(""))
}

/// 是否为开发模式（Tauri 在 `cargo tauri dev` 时设置 cfg(dev)）
fn is_dev() -> bool {
    cfg!(dev)
}

/// 服务文件基础目录
/// - 开发模式：{项目根}/data/
/// - 打包模式：{exe所在目录}/service/
pub fn service_base_dir() -> PathBuf {
    if is_dev() {
        data_root()
    } else {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .map(|p| p.join("service"))
            .unwrap_or_else(|| PathBuf::from("service"))
    }
}

/// FastKG 服务根目录，可通过环境变量 FASTCLAW_FASTKG_DIR 自定义
pub fn fastkg_service_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("FASTCLAW_FASTKG_DIR") {
        let path = PathBuf::from(&dir);
        if path.is_dir() {
            return path;
        }
        eprintln!(
            "[paths] FASTCLAW_FASTKG_DIR 设置的目录不存在或不是目录: {}，回退到默认路径",
            dir
        );
    }
    // 开发模式：data/FastKGService/; 打包模式：{exe}/service/FastKG/
    service_base_dir().join(if is_dev() { "FastKGService" } else { "FastKG" })
}

/// Cross-application FastKG coordination directory.
/// Kept under LocalAppData so local FastKG instances can discover each other.
pub fn kg_coordination_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| service_base_dir())
        .join("FastKG")
        .join("coordination")
        .join("v1")
}

pub fn kg_runtime_registry_path() -> PathBuf {
    kg_coordination_dir().join("runtime.json")
}

pub fn kg_runtime_lock_path() -> PathBuf {
    kg_coordination_dir().join("runtime.lock")
}

/// FastKG 服务可执行文件路径
pub fn fastkg_server_path() -> PathBuf {
    fastkg_service_dir().join("FastKG.exe")
}

/// Qdrant 可执行文件路径
pub fn qdrant_path() -> PathBuf {
    fastkg_service_dir().join("qdrant.exe")
}

/// llama-server 可执行文件路径
pub fn llama_server_path() -> PathBuf {
    fastkg_service_dir()
        .join("ebinding_start")
        .join("llamacpp")
        .join("llama-server.exe")
}

/// bge-m3 模型文件路径
pub fn bge_m3_model_path() -> PathBuf {
    fastkg_service_dir()
        .join("ebinding_start")
        .join("models")
        .join("bge-m3-Q8_0.gguf")
}

/// ebinding_start 目录路径
pub fn ebinding_start_path() -> PathBuf {
    fastkg_service_dir().join("ebinding_start")
}

/// Shimmy 服务根目录，可通过环境变量 FASTCLAW_SHIMMY_DIR 自定义
pub fn shimmy_service_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("FASTCLAW_SHIMMY_DIR") {
        let path = PathBuf::from(&dir);
        if path.is_dir() {
            return path;
        }
        eprintln!(
            "[paths] FASTCLAW_SHIMMY_DIR 设置的目录不存在或不是目录: {}，回退到默认路径",
            dir
        );
    }
    if is_dev() {
        // 开发模式：{项目根}/data/shimmy/
        project_root()
            .parent()
            .map(|p| p.join("data").join("shimmy"))
            .unwrap_or_else(|| PathBuf::from("data/shimmy"))
    } else {
        // 打包模式：{exe}/service/Shimmy/
        service_base_dir().join("Shimmy")
    }
}

/// Shimmy 可执行文件路径
pub fn shimmy_server_path() -> PathBuf {
    shimmy_service_dir().join("shimmy-windows-x86_64.exe")
}

/// FastKG 服务默认端口
pub fn fastkg_default_port() -> u16 {
    FASTKG_DEFAULT_PORT
}

/// Qdrant 服务默认端口
pub fn qdrant_default_port() -> u16 {
    QDRANT_DEFAULT_PORT
}

/// llama-server 嵌入服务端口
pub fn llama_server_default_port() -> u16 {
    LLAMA_SERVER_DEFAULT_PORT
}

/// FastPlan 默认端口
pub fn fastplan_default_port() -> u16 {
    FASTPLAN_DEFAULT_PORT
}

/// FastPlan 可执行文件路径
/// - 可通过环境变量 FASTCLAW_FASTPLAN_PATH 自定义
/// - 默认：{exe目录}/fastplan.exe
pub fn fastplan_server_path() -> PathBuf {
    if let Ok(p) = std::env::var("FASTCLAW_FASTPLAN_PATH") {
        let path = PathBuf::from(&p);
        if path.exists() {
            return path;
        }
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default();
    let name = if cfg!(windows) {
        "fastplan.exe"
    } else {
        "fastplan"
    };
    exe_dir.join(name)
}

/// File2Md OCR 服务根目录
/// - 开发模式：{项目根}/data/File2Md/
/// - 打包模式：{exe}/service/File2Md/
pub fn file2md_service_dir() -> PathBuf {
    service_base_dir().join("File2Md")
}

/// File2Md OCR 可执行文件路径
pub fn file2md_ocr_exe_path() -> PathBuf {
    file2md_service_dir().join("file2md-ocr-tiny.exe")
}
