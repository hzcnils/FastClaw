// FastClaw · 系统级截图命令
// 多显示器合成 + Windows 窗口枚举 + 智能截图
// WDA_EXCLUDEFROMCAPTURE 原生排除自身窗口，无需 hide/show 消除闪烁

use crate::core::error::AppError;
use base64::Engine as _;
use image::codecs::jpeg::JpegEncoder;
use image::ImageEncoder;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::Manager;

// ─── WDA_EXCLUDEFROMCAPTURE（Windows 10 2004+） ───
#[cfg(windows)]
const WDA_EXCLUDEFROMCAPTURE: u32 = 0x0000_0011;

/// 隐藏窗口后等待 DWM 完成重绘的时长（毫秒）。
/// 仅作 DwmFlush 失败时的回退值。正常路径使用 DwmFlush 精确同步（~16ms）。
const HIDE_SETTLE_MS: u64 = 200;

/// 「隐藏窗口截图」专用：WebView2 / 无边框窗体在 SW_HIDE 后，DWM 合成缓冲里
/// 仍可能残留半透明残影；仅 DwmFlush+30ms 不够，需额外固定等待。
const HIDE_SELF_CAPTURE_SETTLE_MS: u64 = 400;

/// true = 已在主窗口上成功启用，截图时无需 hide/show
static DISPLAY_AFFINITY_ACTIVE: AtomicBool = AtomicBool::new(false);

/// 隐藏窗口截图时把主窗口挪出屏幕前的位置 (x, y, w, h)，供 show_main_window 还原。
#[cfg(windows)]
static HIDDEN_MAIN_PLACEMENT: Mutex<Option<(i32, i32, i32, i32)>> = Mutex::new(None);

#[cfg(windows)]
extern "system" {
    fn SetWindowDisplayAffinity(hwnd: isize, affinity: u32) -> i32;
}

/// 在应用启动时调用一次，将 FastClaw 主窗口从屏幕捕获中排除。
/// 成功时后续截图无需隐藏窗口，彻底消除闪烁。
/// 返回 true 表示 API 可用且成功。
#[cfg(windows)]
pub fn apply_display_affinity(hwnd: isize) -> bool {
    let result = unsafe { SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) };
    let active = result != 0;
    DISPLAY_AFFINITY_ACTIVE.store(active, Ordering::SeqCst);
    if active {
        log::info!("[Screenshot] WDA_EXCLUDEFROMCAPTURE 已启用，截图自动排除 FastClaw 窗口");
    } else {
        log::warn!("[Screenshot] WDA_EXCLUDEFROMCAPTURE 不可用 (需 Win10 2004+)，回退到 hide/show");
    }
    active
}

#[cfg(not(windows))]
pub fn apply_display_affinity(_hwnd: usize) -> bool {
    false
}

/// 显示器信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonitorInfo {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
    pub name: String,
}

/// 窗口信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WindowInfo {
    pub handle: u64,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub title: String,
    pub is_foreground: bool,
}

/// 截图结果（兼容旧接口）
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ScreenshotResult {
    pub width: u32,
    pub height: u32,
    pub base64_data: String,
}

/// 增强截图结果（含窗口列表 + 多显示器信息）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CaptureResult {
    /// JPEG base64 data URL（JPEG 质量 95，比 PNG 小 5-10x）
    pub base64_data: String,
    pub width: u32,
    pub height: u32,
    /// 虚拟桌面原点 X（所有显示器最小 x）
    pub offset_x: i32,
    /// 虚拟桌面原点 Y（所有显示器最小 y）
    pub offset_y: i32,
    pub monitors: Vec<MonitorInfo>,
    pub windows: Vec<WindowInfo>,
}

/// 两步 IPC 方案的元数据（不含像素，体积小，用于 Step 1）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CaptureMetadata {
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub presentation: Option<String>,
    pub width: u32,
    pub height: u32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub monitors: Vec<MonitorInfo>,
    pub windows: Vec<WindowInfo>,
}

// ============================================================================
// 旧接口：capture_fullscreen（保留向后兼容，仅截取主显示器）
// ============================================================================

#[tauri::command]
pub async fn capture_fullscreen() -> Result<ScreenshotResult, AppError> {
    let monitors =
        xcap::Monitor::all().map_err(|e| AppError::Internal(anyhow::anyhow!("截图失败: {}", e)))?;

    let monitor = monitors
        .into_iter()
        .next()
        .ok_or_else(|| AppError::NotFound("未检测到显示器".into()))?;

    let image = monitor
        .capture_image()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("截图失败: {}", e)))?;

    let width = image.width();
    let height = image.height();

    let mut png_buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png_buf);
    encoder
        .write_image(
            image.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!("编码失败: {}", e)))?;

    let base64_str = base64::engine::general_purpose::STANDARD.encode(&png_buf);

    Ok(ScreenshotResult {
        width,
        height,
        base64_data: format!("data:image/png;base64,{}", base64_str),
    })
}

// ============================================================================
// Windows HDR 支持
// ============================================================================

#[cfg(windows)]
mod hdr {
    use crate::core::error::AppError;
    use std::sync::atomic::AtomicBool;
    use std::sync::OnceLock;

    /// HDR 检测失败或 windows-capture 不可用时降级到 xcap
    static HDR_CAPTURE_FAILED: AtomicBool = AtomicBool::new(false);
    static HDR_DETECTED: OnceLock<bool> = OnceLock::new();

    // ── Win32 HDR 检测 ──────────────────────────────────────────────────────
    #[repr(C)]
    struct DisplayConfigDeviceInfoHeader {
        r#type: u32,
        size: u32,
        adapter_id_low: u32,
        adapter_id_high: u32,
        id: u32,
    }

    #[repr(C)]
    struct DisplayConfigGetAdvancedColorInfo {
        header: DisplayConfigDeviceInfoHeader,
        value: u32, // bit1 = advancedColorEnabled
        color_encoding: u32,
        bits_per_color_channel: u32,
    }

    const DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO: u32 = 9;
    const QDC_ONLY_ACTIVE_PATHS: u32 = 0x2;

    extern "system" {
        fn GetDisplayConfigBufferSizes(flags: u32, num_path: *mut u32, num_mode: *mut u32) -> i32;
        fn QueryDisplayConfig(
            flags: u32,
            num_path: *mut u32,
            paths: *mut u8,
            num_mode: *mut u32,
            modes: *mut u8,
            current_topology_id: *mut u32,
        ) -> i32;
        fn DisplayConfigGetDeviceInfo(packet: *mut DisplayConfigDeviceInfoHeader) -> i32;
    }

    const DISPLAYCONFIG_PATH_INFO_SIZE: usize = 72;
    const DISPLAYCONFIG_MODE_INFO_SIZE: usize = 64;

    fn detect_hdr_via_win32() -> bool {
        unsafe {
            let mut num_paths: u32 = 0;
            let mut num_modes: u32 = 0;
            if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut num_paths, &mut num_modes)
                != 0
            {
                return false;
            }
            if num_paths == 0 {
                return false;
            }

            let mut paths = vec![0u8; num_paths as usize * DISPLAYCONFIG_PATH_INFO_SIZE];
            let mut modes = vec![0u8; num_modes as usize * DISPLAYCONFIG_MODE_INFO_SIZE];
            let mut topology_id: u32 = 0;

            if QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut num_paths,
                paths.as_mut_ptr(),
                &mut num_modes,
                modes.as_mut_ptr(),
                &mut topology_id,
            ) != 0
            {
                return false;
            }

            // 每个 path 的 targetInfo.id 在偏移 44 处（u32），adapterLuid 在 36/40 处（u32×2）
            for i in 0..num_paths as usize {
                let base = i * DISPLAYCONFIG_PATH_INFO_SIZE;
                if base + 48 > paths.len() {
                    break;
                }
                let adapter_low =
                    u32::from_ne_bytes(paths[base + 36..base + 40].try_into().unwrap());
                let adapter_high =
                    u32::from_ne_bytes(paths[base + 40..base + 44].try_into().unwrap());
                let target_id = u32::from_ne_bytes(paths[base + 44..base + 48].try_into().unwrap());

                let mut aci = DisplayConfigGetAdvancedColorInfo {
                    header: DisplayConfigDeviceInfoHeader {
                        r#type: DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
                        size: std::mem::size_of::<DisplayConfigGetAdvancedColorInfo>() as u32,
                        adapter_id_low: adapter_low,
                        adapter_id_high: adapter_high,
                        id: target_id,
                    },
                    value: 0,
                    color_encoding: 0,
                    bits_per_color_channel: 0,
                };
                if DisplayConfigGetDeviceInfo(&mut aci.header as *mut DisplayConfigDeviceInfoHeader)
                    == 0
                {
                    // bit1 = advancedColorEnabled
                    if aci.value & 0x2 != 0 {
                        return true;
                    }
                }
            }
            false
        }
    }

    pub fn is_hdr_enabled() -> bool {
        *HDR_DETECTED.get_or_init(detect_hdr_via_win32)
    }

    // ── Tone mapping ──────────────────────────────────────────────────────────

    /// sRGB gamma 编码（线性光 → 8 位 sRGB）
    fn linear_to_srgb(v: f32) -> u8 {
        let srgb = if v <= 0.0031308 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        };
        (srgb.clamp(0.0, 1.0) * 255.0).round() as u8
    }

    /// Rgba16F 原始字节 → RGBA8（rayon 并行，每 8 字节一个像素）
    pub fn convert_rgba16f_to_rgba8(raw: &[u8], sdr_white_nits: f32) -> Vec<u8> {
        use half::f16;
        use rayon::prelude::*;
        let scale = 1000.0 / sdr_white_nits;
        raw.par_chunks(8)
            .flat_map_iter(|px| {
                let r = f16::from_le_bytes([px[0], px[1]]).to_f32() * scale;
                let g = f16::from_le_bytes([px[2], px[3]]).to_f32() * scale;
                let b = f16::from_le_bytes([px[4], px[5]]).to_f32() * scale;
                [
                    linear_to_srgb(r),
                    linear_to_srgb(g),
                    linear_to_srgb(b),
                    255u8,
                ]
            })
            .collect()
    }

    // ── 带降级的显示器捕获 ────────────────────────────────────────────────────

    pub fn capture_monitor_with_hdr_fallback(
        index: usize,
        monitors: &[xcap::Monitor],
    ) -> Result<image::RgbaImage, AppError> {
        // windows-capture HDR 路径（集成后取消注释）
        // if is_hdr_enabled() && !HDR_CAPTURE_FAILED.load(Ordering::Relaxed) {
        //     match capture_hdr_monitor_via_wgc(index) {
        //         Ok(img) => return Ok(img),
        //         Err(e) => {
        //             log::warn!("[HDR] 捕获失败，降级至 xcap: {}", e);
        //             HDR_CAPTURE_FAILED.store(true, Ordering::Relaxed);
        //         }
        //     }
        // }
        monitors[index]
            .capture_image()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("截图失败: {}", e)))
    }
}

// ============================================================================
// macOS EDR（Extended Dynamic Range）亮度修正
// ============================================================================

#[cfg(target_os = "macos")]
mod macos_edr {
    use std::sync::OnceLock;

    static EDR_RATIO: OnceLock<f32> = OnceLock::new();

    /// 查询主显示器 EDR 最大亮度系数（1.0 = SDR，>1.0 = EDR/HDR）
    pub fn get_edr_ratio() -> f32 {
        *EDR_RATIO.get_or_init(|| query_edr_ratio())
    }

    fn query_edr_ratio() -> f32 {
        unsafe {
            use objc2_app_kit::NSScreen;
            use objc2_foundation::MainThreadMarker;
            // MainThreadMarker::new_unchecked()：此函数在 Tauri setup 的主线程上调用
            let mtm = MainThreadMarker::new_unchecked();
            NSScreen::mainScreen(mtm)
                .map(|s| s.maximumExtendedDynamicRangeColorComponentValue() as f32)
                .unwrap_or(1.0)
        }
    }

    /// 若 EDR 比率 > 1.0，对 xcap 捕获的 sRGB 像素做近似亮度修正。
    /// xcap（CoreGraphics）将 HDR 高亮区域压缩到 255，此修正将整体收缩以还原对比度。
    /// 注：这是 best-effort 方案，完整 HDR 需要 ScreenCaptureKit SCStream（暂无稳定 Rust 绑定）。
    pub fn apply_edr_correction(img: &mut image::RgbaImage) {
        let ratio = get_edr_ratio();
        if ratio <= 1.01 {
            return;
        }
        let scale = 1.0 / ratio;
        for pixel in img.pixels_mut() {
            pixel[0] = (pixel[0] as f32 * scale).clamp(0.0, 255.0) as u8;
            pixel[1] = (pixel[1] as f32 * scale).clamp(0.0, 255.0) as u8;
            pixel[2] = (pixel[2] as f32 * scale).clamp(0.0, 255.0) as u8;
        }
    }
}

// ============================================================================
// 多显示器合成
// ============================================================================

/// 捕获原始合成图像（无 PNG 编码），最小化窗口隐藏时间
fn capture_all_monitors_raw(
) -> Result<(image::RgbaImage, u32, u32, i32, i32, Vec<MonitorInfo>), AppError> {
    let monitors = xcap::Monitor::all()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("获取显示器列表失败: {}", e)))?;

    if monitors.is_empty() {
        return Err(AppError::NotFound("未检测到显示器".into()));
    }

    struct MonMeta {
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        is_primary: bool,
        name: String,
    }

    let mut metas: Vec<MonMeta> = Vec::new();
    for mon in &monitors {
        let x = mon.x().unwrap_or(0);
        let y = mon.y().unwrap_or(0);
        let w = mon.width().unwrap_or(1920);
        let h = mon.height().unwrap_or(1080);
        metas.push(MonMeta {
            x,
            y,
            w,
            h,
            is_primary: mon.is_primary().unwrap_or(false),
            name: mon.name().unwrap_or_default(),
        });
    }

    // 单显示器短路：无虚拟桌面偏移，直接捕获，省略多显示器合成开销
    if metas.len() == 1 {
        #[cfg(windows)]
        let img = hdr::capture_monitor_with_hdr_fallback(0, &monitors)?;
        #[cfg(not(windows))]
        let mut img = monitors[0]
            .capture_image()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("截图失败: {}", e)))?;
        #[cfg(target_os = "macos")]
        macos_edr::apply_edr_correction(&mut img);
        let (w, h) = (img.width(), img.height());
        let info = MonitorInfo {
            x: metas[0].x,
            y: metas[0].y,
            width: metas[0].w,
            height: metas[0].h,
            is_primary: metas[0].is_primary,
            name: metas[0].name.clone(),
        };
        return Ok((img, w, h, metas[0].x, metas[0].y, vec![info]));
    }

    let min_x = metas.iter().map(|m| m.x).min().unwrap_or(0);
    let min_y = metas.iter().map(|m| m.y).min().unwrap_or(0);
    let max_right = metas.iter().map(|m| m.x + m.w as i32).max().unwrap_or(0);
    let max_bottom = metas.iter().map(|m| m.y + m.h as i32).max().unwrap_or(0);

    let virt_w = (max_right - min_x) as u32;
    let virt_h = (max_bottom - min_y) as u32;

    let num_monitors = metas.len();
    let captures: Vec<(image::RgbaImage, i64, i64, MonitorInfo)> = std::thread::scope(|s| {
        let mut handles = Vec::new();
        for i in 0..num_monitors {
            let meta = &metas[i];
            let offset_x = (meta.x - min_x) as i64;
            let offset_y = (meta.y - min_y) as i64;
            let info = MonitorInfo {
                x: meta.x,
                y: meta.y,
                width: meta.w,
                height: meta.h,
                is_primary: meta.is_primary,
                name: meta.name.clone(),
            };
            handles.push(s.spawn(move || {
                let all = xcap::Monitor::all()
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("截图失败: {}", e)))?;
                #[cfg(windows)]
                let img = hdr::capture_monitor_with_hdr_fallback(i, &all)?;
                #[cfg(not(windows))]
                let mut img = all[i]
                    .capture_image()
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("截图失败: {}", e)))?;
                #[cfg(target_os = "macos")]
                macos_edr::apply_edr_correction(&mut img);
                Ok::<_, AppError>((img, offset_x, offset_y, info))
            }));
        }
        handles
            .into_iter()
            .map(|h| {
                h.join().unwrap_or_else(|e| {
                    Err(AppError::Internal(anyhow::anyhow!(
                        "截图线程 panic: {:?}",
                        e
                    )))
                })
            })
            .collect::<Result<Vec<_>, _>>()
    })?;

    let mut composite = image::RgbaImage::new(virt_w, virt_h);
    let mut monitor_infos = Vec::with_capacity(captures.len());
    for (img, off_x, off_y, info) in captures {
        image::imageops::overlay(&mut composite, &img, off_x, off_y);
        monitor_infos.push(info);
    }

    Ok((composite, virt_w, virt_h, min_x, min_y, monitor_infos))
}

/// 将合成图像编码为原始 JPEG 字节（无 base64）。供两步 IPC 使用。
fn encode_jpeg_raw(composite: &image::RgbaImage) -> Result<Vec<u8>, AppError> {
    let rgb = image::DynamicImage::ImageRgba8(composite.clone()).to_rgb8();
    let mut jpeg_buf = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut jpeg_buf, 95);
    encoder
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!("JPEG 编码失败: {}", e)))?;
    Ok(jpeg_buf)
}

/// 将合成图像编码为 JPEG base64 data URL。
/// JPEG 质量 95：减少文字/代码边缘块状伪影，提升大模型 OCR 识别精度。
fn encode_jpeg_to_base64(composite: &image::RgbaImage) -> Result<String, AppError> {
    // JPEG 不支持 alpha 通道，RGBA → RGB
    let rgb = image::DynamicImage::ImageRgba8(composite.clone()).to_rgb8();

    let mut jpeg_buf = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut jpeg_buf, 95);
    encoder
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| AppError::Internal(anyhow::anyhow!("JPEG 编码失败: {}", e)))?;

    let base64_str = base64::engine::general_purpose::STANDARD.encode(&jpeg_buf);
    Ok(format!("data:image/jpeg;base64,{}", base64_str))
}

// ============================================================================
// Windows 窗口枚举（使用原始 Win32 FFI，不依赖 windows crate）
// ============================================================================

#[cfg(windows)]
mod win32_enumerate {
    use crate::commands::screenshot::WindowInfo;
    use crate::utils::win32::{
        DwmFlush, DwmGetWindowAttribute, EnumWindows, GetForegroundWindow, GetWindowLongW,
        GetWindowRect, GetWindowTextW, IsWindowVisible, SetWindowPos, ShowWindow, Bool, Hwnd,
        Lparam, Rect, TRUE,
    };
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_TOOLWINDOW: i32 = 0x0000_0080;
    // DwmGetWindowAttribute: 返回窗口可见区域（不含 DWM 透明阴影边框）
    const DWMWA_EXTENDED_FRAME_BOUNDS: u32 = 9;

    pub const SW_HIDE: i32 = 0;
    pub const SW_SHOW: i32 = 5;
    /// SetWindowPos: 保持尺寸 / 不改 Z-order / 隐藏 / 显示
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_SHOWWINDOW: u32 = 0x0040;
    const SWP_HIDEWINDOW: u32 = 0x0080;

    struct EnumContext {
        windows: Vec<WindowInfo>,
    }

    unsafe extern "system" fn enum_proc(hwnd: Hwnd, lparam: Lparam) -> Bool {
        let ctx = &mut *(lparam as *mut EnumContext);

        // 过滤不可见窗口（隐藏的主窗口在此被过滤）
        if IsWindowVisible(hwnd) == 0 {
            return TRUE;
        }

        // 过滤工具窗口
        if GetWindowLongW(hwnd, GWL_EXSTYLE) & WS_EX_TOOLWINDOW != 0 {
            return TRUE;
        }

        // 获取窗口标题
        let mut buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        if len == 0 {
            return TRUE;
        }
        let title = OsString::from_wide(&buf[..len as usize])
            .to_string_lossy()
            .into_owned();

        // 优先使用 DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS) 获取可见边框
        // 该 API 排除了 Windows 10/11 窗口四周的透明 DWM 阴影边距（通常每侧约 7px）
        // GetWindowRect 返回的坐标包含该透明区域，导致高亮框偏大
        let mut rect = Rect::default();
        let hr = DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect,
            std::mem::size_of::<Rect>() as u32,
        );
        if hr != 0 {
            // DWM 调用失败（如最小化窗口），降级使用 GetWindowRect
            if GetWindowRect(hwnd, &mut rect) == 0 {
                return TRUE;
            }
        }

        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        if w <= 0 || h <= 0 {
            return TRUE;
        }

        let foreground = GetForegroundWindow();

        ctx.windows.push(WindowInfo {
            handle: hwnd as u64,
            x: rect.left,
            y: rect.top,
            width: w,
            height: h,
            title,
            is_foreground: hwnd == foreground,
        });

        TRUE
    }

    pub fn enumerate() -> Vec<WindowInfo> {
        let mut ctx = EnumContext {
            windows: Vec::new(),
        };

        unsafe {
            EnumWindows(Some(enum_proc), &mut ctx as *mut _ as Lparam);
        }

        ctx.windows
    }

    pub fn show_window(hwnd: isize, cmd: i32) -> bool {
        unsafe { ShowWindow(hwnd, cmd) != 0 }
    }

    pub fn foreground_window() -> isize {
        unsafe { GetForegroundWindow() }
    }

    /// 调用 DwmFlush() 阻塞直到 DWM 完成所有待处理的合成帧。
    /// 返回 true 表示 DWM 刷新成功，false 表示调用失败（需回退到固定延迟）。
    pub fn dwm_flush_sync() -> bool {
        unsafe { DwmFlush() == 0 }
    }

    /// 隐藏窗口截图：移出屏幕 + SW_HIDE，避免 WebView2 在 DWM 缓冲中留下半透明残影。
    /// 返回隐藏前的 (x, y, w, h)，供之后 restore_from_offscreen 还原。
    pub fn hide_offscreen(hwnd: isize) -> Option<(i32, i32, i32, i32)> {
        unsafe {
            let mut rect = Rect::default();
            if GetWindowRect(hwnd, &mut rect) == 0 {
                let _ = ShowWindow(hwnd, SW_HIDE);
                return None;
            }
            let x = rect.left;
            let y = rect.top;
            let w = rect.right - rect.left;
            let h = rect.bottom - rect.top;
            // 先挪出屏幕再隐藏：BitBlt/xcap 即使读到合成帧也碰不到本窗口像素
            let _ = SetWindowPos(
                hwnd,
                0,
                -32000,
                -32000,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_HIDEWINDOW,
            );
            let _ = ShowWindow(hwnd, SW_HIDE);
            Some((x, y, w, h))
        }
    }

    /// 将 hide_offscreen 移走的窗口还原到原位置并显示。
    pub fn restore_from_offscreen(hwnd: isize, placement: (i32, i32, i32, i32)) {
        let (x, y, w, h) = placement;
        unsafe {
            let _ = SetWindowPos(hwnd, 0, x, y, w, h, SWP_NOZORDER | SWP_SHOWWINDOW);
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
    }
}

#[cfg(windows)]
fn enumerate_windows() -> Result<Vec<WindowInfo>, AppError> {
    Ok(win32_enumerate::enumerate())
}

#[cfg(not(windows))]
fn enumerate_windows() -> Result<Vec<WindowInfo>, AppError> {
    Ok(Vec::new())
}

#[tauri::command]
pub async fn enumerate_window_list() -> Result<Vec<WindowInfo>, AppError> {
    tokio::task::spawn_blocking(enumerate_windows)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("窗口枚举线程异常: {}", e)))?
}

// ============================================================================
// 原始截图（不隐藏任何窗口，包含自身）
// ============================================================================

/// 临时禁用 WDA，截图后恢复。仅在 affinity 已启用时生效。
#[cfg(windows)]
fn with_affinity_disabled<F, T>(hwnd: isize, f: F) -> T
where
    F: FnOnce() -> T,
{
    let was_active = DISPLAY_AFFINITY_ACTIVE.load(Ordering::SeqCst);
    if was_active {
        unsafe { SetWindowDisplayAffinity(hwnd, 0) };
    }
    let result = f();
    if was_active {
        unsafe { SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) };
    }
    result
}

#[cfg(not(windows))]
fn with_affinity_disabled<F, T>(_hwnd: isize, f: F) -> T
where
    F: FnOnce() -> T,
{
    f()
}

// ─── 临时启用 WDA_EXCLUDEFROMCAPTURE（方案B：按需启用，截图后关闭）───

/// 截图前临时启用 WDA_EXCLUDEFROMCAPTURE，使 FastClaw 窗口从本次屏幕捕获中排除。
/// 返回 true 表示 WDA 已启用（截图时无需 hide），false 表示不可用（需要 hide 回退）。
/// 调用方在截图完成后必须调用 `disable_affinity_for_screenshot(hwnd)` 关闭，
/// 否则 FastClaw 对外部截屏工具不可见。
#[cfg(windows)]
fn enable_affinity_for_screenshot(hwnd: isize) -> bool {
    let applied = unsafe { SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) != 0 };
    DISPLAY_AFFINITY_ACTIVE.store(applied, Ordering::SeqCst);
    if applied {
        log::info!("[Screenshot] WDA_EXCLUDEFROMCAPTURE 临时启用，本次截图无需 hide");
    } else {
        log::warn!("[Screenshot] WDA_EXCLUDEFROMCAPTURE 不可用，回退到 hide/show");
    }
    applied
}

/// 截图完成后关闭 WDA_EXCLUDEFROMCAPTURE，恢复 FastClaw 对外部截屏工具的可见性。
#[cfg(windows)]
fn disable_affinity_for_screenshot(hwnd: isize) {
    unsafe { SetWindowDisplayAffinity(hwnd, 0) };
    DISPLAY_AFFINITY_ACTIVE.store(false, Ordering::SeqCst);
}

#[cfg(not(windows))]
fn enable_affinity_for_screenshot(_hwnd: isize) -> bool {
    false
}

#[cfg(not(windows))]
fn disable_affinity_for_screenshot(_hwnd: isize) {}

#[tauri::command]
pub async fn capture_raw(app_handle: tauri::AppHandle) -> Result<CaptureResult, AppError> {
    // 获取主窗口 HWND 用于临时禁用 affinity
    let hwnd: isize = {
        #[cfg(windows)]
        {
            app_handle
                .get_webview_window("main")
                .and_then(|w| w.hwnd().ok().map(|h| h.0 as isize))
                .unwrap_or(0)
        }
        #[cfg(not(windows))]
        {
            0
        }
    };

    let (raw_result, windows_result) = tokio::join!(
        tokio::task::spawn_blocking(move || {
            with_affinity_disabled(hwnd, capture_all_monitors_raw)
        }),
        tokio::task::spawn_blocking(enumerate_windows),
    );

    let (composite, virt_w, virt_h, off_x, off_y, monitors) =
        raw_result.map_err(|e| AppError::Internal(anyhow::anyhow!("截图线程异常: {}", e)))??;

    let windows = windows_result
        .map_err(|e| AppError::Internal(anyhow::anyhow!("窗口枚举线程异常: {}", e)))??;

    let base64_data = tokio::task::spawn_blocking(move || encode_jpeg_to_base64(&composite))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("编码线程异常: {}", e)))??;

    Ok(CaptureResult {
        base64_data,
        width: virt_w,
        height: virt_h,
        offset_x: off_x,
        offset_y: off_y,
        monitors,
        windows,
    })
}

// ============================================================================
// 增强版命令：capture_fullscreen_with_windows
// ============================================================================

#[tauri::command]
pub async fn capture_fullscreen_with_windows(
    app_handle: tauri::AppHandle,
) -> Result<CaptureResult, AppError> {
    // 获取主窗口 HWND 用于临时启用/关闭 WDA
    let hwnd: isize = {
        #[cfg(windows)]
        {
            app_handle
                .get_webview_window("main")
                .and_then(|w| w.hwnd().ok().map(|h| h.0 as isize))
                .unwrap_or(0)
        }
        #[cfg(not(windows))]
        {
            0
        }
    };

    // 截图前临时启用 WDA
    let wda_applied = enable_affinity_for_screenshot(hwnd);
    let use_affinity = DISPLAY_AFFINITY_ACTIVE.load(Ordering::SeqCst);

    // 1. 若未启用 WDA_EXCLUDEFROMCAPTURE，则回退到 hide 方式
    if !use_affinity {
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.hide();
            // hide() 仅投递消息，DWM 合成是异步的；使用 DwmFlush 精确同步
            let _ = tokio::task::spawn_blocking(sync_after_hide).await;
        }
    }

    // 2. 并行：捕获原始像素 + 枚举窗口
    let (raw_result, windows_result) = tokio::join!(
        tokio::task::spawn_blocking(capture_all_monitors_raw),
        tokio::task::spawn_blocking(enumerate_windows),
    );

    let (composite, virt_w, virt_h, off_x, off_y, monitors) =
        raw_result.map_err(|e| AppError::Internal(anyhow::anyhow!("截图线程异常: {}", e)))??;

    let mut windows = windows_result
        .map_err(|e| AppError::Internal(anyhow::anyhow!("窗口枚举线程异常: {}", e)))??;
    // 排除 FastClaw 自身窗口
    windows.retain(|w| w.handle != hwnd as u64);

    // 3. 恢复窗口（仅回退模式需要）
    if !use_affinity {
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }

    // 4. JPEG 编码 + base64（JPEG 比 PNG 小 5-10x，base64 传输快得多）
    let base64_data = tokio::task::spawn_blocking(move || encode_jpeg_to_base64(&composite))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("编码线程异常: {}", e)))??;

    // 截图完成，关闭 WDA 恢复对外部截屏工具的可见性
    if wda_applied {
        disable_affinity_for_screenshot(hwnd);
    }

    Ok(CaptureResult {
        base64_data,
        width: virt_w,
        height: virt_h,
        offset_x: off_x,
        offset_y: off_y,
        monitors,
        windows,
    })
}

// ============================================================================
// 隐藏指定窗口后重新截图
// ============================================================================

#[cfg(windows)]
fn hide_target_window(handle: u64) {
    let ok = win32_enumerate::show_window(handle as isize, win32_enumerate::SW_HIDE);
    if !ok {
        log::warn!("[Screenshot] ShowWindow(SW_HIDE) 失败 handle={}", handle);
    }
    sync_after_hide();
}

#[cfg(windows)]
fn show_target_window(handle: u64) {
    let ok = win32_enumerate::show_window(handle as isize, win32_enumerate::SW_SHOW);
    if !ok {
        log::warn!("[Screenshot] ShowWindow(SW_SHOW) 失败 handle={}", handle);
    }
}

#[cfg(not(windows))]
fn hide_target_window(_handle: u64) {}

#[cfg(not(windows))]
fn show_target_window(_handle: u64) {}

/// 等待 DWM 完成所有待处理的合成帧（精确同步，比固定 sleep 可靠）。
/// 在 Windows 上调用 DwmFlush() 阻塞至 DWM 完成渲染；
/// 非 Windows 或 DwmFlush 失败时回退到 HIDE_SETTLE_MS 固定延迟。
#[cfg(windows)]
fn sync_after_hide() {
    if win32_enumerate::dwm_flush_sync() {
        // DwmFlush 成功：DWM 已确认完成合成，附加极小安全边际（30ms）兜底
        std::thread::sleep(std::time::Duration::from_millis(30));
        return;
    }
    log::warn!(
        "[Screenshot] DwmFlush 失败，回退到固定延迟 {}ms",
        HIDE_SETTLE_MS
    );
    std::thread::sleep(std::time::Duration::from_millis(HIDE_SETTLE_MS));
}

/// 「隐藏窗口截图」专用 settle：多次 DwmFlush + 固定等待，清掉 WebView2 半透明残影。
#[cfg(windows)]
fn sync_after_hide_self_for_capture() {
    for _ in 0..3 {
        let _ = win32_enumerate::dwm_flush_sync();
    }
    std::thread::sleep(std::time::Duration::from_millis(HIDE_SELF_CAPTURE_SETTLE_MS));
    let _ = win32_enumerate::dwm_flush_sync();
}

#[cfg(not(windows))]
fn sync_after_hide() {
    // 非 Windows 平台不使用 DwmFlush，回退到固定延迟
    std::thread::sleep(std::time::Duration::from_millis(HIDE_SETTLE_MS));
}

#[cfg(not(windows))]
fn sync_after_hide_self_for_capture() {
    std::thread::sleep(std::time::Duration::from_millis(HIDE_SETTLE_MS));
}

/// 还原「隐藏窗口截图」移出屏幕的主窗口（位置 + 显示）。
/// capture_start(restore_self=true) 与前端 show_main_window 共用。
pub fn restore_main_after_hide_capture(app_handle: &tauri::AppHandle) {
    if let Some(window) = app_handle.get_webview_window("main") {
        #[cfg(windows)]
        {
            if let Ok(hwnd) = window.hwnd() {
                let hwnd = hwnd.0 as isize;
                let placement = HIDDEN_MAIN_PLACEMENT.lock().unwrap().take();
                if let Some(p) = placement {
                    win32_enumerate::restore_from_offscreen(hwnd, p);
                } else {
                    let _ = win32_enumerate::show_window(hwnd, win32_enumerate::SW_SHOW);
                }
            } else {
                let _ = window.show();
            }
        }
        #[cfg(not(windows))]
        {
            let _ = window.show();
        }
        let _ = window.set_focus();
    }
}

#[tauri::command]
pub async fn recapture_with_window_hidden(
    app_handle: tauri::AppHandle,
    handle: u64,
) -> Result<CaptureResult, AppError> {
    // 获取主窗口 HWND 用于临时启用/关闭 WDA
    let hwnd: isize = {
        #[cfg(windows)]
        {
            app_handle
                .get_webview_window("main")
                .and_then(|w| w.hwnd().ok().map(|h| h.0 as isize))
                .unwrap_or(0)
        }
        #[cfg(not(windows))]
        {
            0
        }
    };

    // 截图前临时启用 WDA
    let wda_applied = enable_affinity_for_screenshot(hwnd);
    let use_affinity = DISPLAY_AFFINITY_ACTIVE.load(Ordering::SeqCst);

    // 1. 隐藏目标窗口
    hide_target_window(handle);

    // 2. 若未启用 WDA_EXCLUDEFROMCAPTURE，回退到 hide 方式隐藏 FastClaw
    if !use_affinity {
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.hide();
            let _ = tokio::task::spawn_blocking(sync_after_hide).await;
        }
    }

    // 3. 并行捕获
    let (raw_result, windows_result) = tokio::join!(
        tokio::task::spawn_blocking(capture_all_monitors_raw),
        tokio::task::spawn_blocking(enumerate_windows),
    );

    let (composite, virt_w, virt_h, off_x, off_y, monitors) =
        raw_result.map_err(|e| AppError::Internal(anyhow::anyhow!("截图线程异常: {}", e)))??;

    let mut windows = windows_result
        .map_err(|e| AppError::Internal(anyhow::anyhow!("窗口枚举线程异常: {}", e)))??;
    // 排除 FastClaw 自身窗口
    windows.retain(|w| w.handle != hwnd as u64);

    // 4. 恢复 FastClaw 窗口
    if !use_affinity {
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }

    // 5. 恢复目标窗口
    show_target_window(handle);

    // 6. JPEG 编码
    let base64_data = tokio::task::spawn_blocking(move || encode_jpeg_to_base64(&composite))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("编码线程异常: {}", e)))??;

    // 截图完成，关闭 WDA 恢复对外部截屏工具的可见性
    if wda_applied {
        disable_affinity_for_screenshot(hwnd);
    }

    Ok(CaptureResult {
        base64_data,
        width: virt_w,
        height: virt_h,
        offset_x: off_x,
        offset_y: off_y,
        monitors,
        windows,
    })
}

#[tauri::command]
pub async fn capture_hide_foreground(
    app_handle: tauri::AppHandle,
) -> Result<CaptureResult, AppError> {
    // 获取主窗口 HWND 用于临时启用/关闭 WDA
    let hwnd: isize = {
        #[cfg(windows)]
        {
            app_handle
                .get_webview_window("main")
                .and_then(|w| w.hwnd().ok().map(|h| h.0 as isize))
                .unwrap_or(0)
        }
        #[cfg(not(windows))]
        {
            0
        }
    };

    // 截图前临时启用 WDA
    let wda_applied = enable_affinity_for_screenshot(hwnd);
    let use_affinity = DISPLAY_AFFINITY_ACTIVE.load(Ordering::SeqCst);

    // 获取前台窗口 HWND
    let fg_hwnd: u64 = {
        #[cfg(windows)]
        {
            win32_enumerate::foreground_window() as u64
        }
        #[cfg(not(windows))]
        {
            0
        }
    };

    // 隐藏前台窗口
    if fg_hwnd != 0 {
        #[cfg(windows)]
        {
            win32_enumerate::show_window(fg_hwnd as isize, win32_enumerate::SW_HIDE);
        }
        let _ = tokio::task::spawn_blocking(sync_after_hide).await;
    }

    // 若未启用 WDA_EXCLUDEFROMCAPTURE，回退隐藏 FastClaw
    if !use_affinity {
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.hide();
            let _ = tokio::task::spawn_blocking(sync_after_hide).await;
        }
    }

    let (raw_result, windows_result) = tokio::join!(
        tokio::task::spawn_blocking(capture_all_monitors_raw),
        tokio::task::spawn_blocking(enumerate_windows),
    );

    let (composite, virt_w, virt_h, off_x, off_y, monitors) =
        raw_result.map_err(|e| AppError::Internal(anyhow::anyhow!("截图线程异常: {}", e)))??;

    let mut windows = windows_result
        .map_err(|e| AppError::Internal(anyhow::anyhow!("窗口枚举线程异常: {}", e)))??;
    // 排除 FastClaw 自身窗口
    windows.retain(|w| w.handle != hwnd as u64);

    // 恢复 FastClaw
    if !use_affinity {
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }

    // 恢复前台窗口
    if fg_hwnd != 0 {
        #[cfg(windows)]
        {
            win32_enumerate::show_window(fg_hwnd as isize, win32_enumerate::SW_SHOW);
        }
    }

    let base64_data = tokio::task::spawn_blocking(move || encode_jpeg_to_base64(&composite))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("编码线程异常: {}", e)))??;

    // 截图完成，关闭 WDA 恢复对外部截屏工具的可见性
    if wda_applied {
        disable_affinity_for_screenshot(hwnd);
    }

    Ok(CaptureResult {
        base64_data,
        width: virt_w,
        height: virt_h,
        offset_x: off_x,
        offset_y: off_y,
        monitors,
        windows,
    })
}

// ============================================================================
// 两步 IPC：capture_start + capture_get_all_monitor_pixels（各屏独立图像）
// ============================================================================

/// 各显示器单独捕获的结果（无合成）
struct PerScreenCaptures {
    captures: Vec<(image::RgbaImage, MonitorInfo)>,
    min_x: i32,
    min_y: i32,
    virt_w: u32,
    virt_h: u32,
}

/// 捕获每个显示器的独立图像，不做合成。
/// 返回的 captures 与 monitors 顺序一一对应。
fn capture_monitors_per_screen() -> Result<PerScreenCaptures, AppError> {
    let monitors = xcap::Monitor::all()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("获取显示器列表失败: {}", e)))?;

    if monitors.is_empty() {
        return Err(AppError::NotFound("未检测到显示器".into()));
    }

    struct MonMeta {
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        is_primary: bool,
        name: String,
    }

    let mut metas: Vec<MonMeta> = Vec::new();
    for mon in &monitors {
        metas.push(MonMeta {
            x: mon.x().unwrap_or(0),
            y: mon.y().unwrap_or(0),
            w: mon.width().unwrap_or(1920),
            h: mon.height().unwrap_or(1080),
            is_primary: mon.is_primary().unwrap_or(false),
            name: mon.name().unwrap_or_default(),
        });
    }

    let min_x = metas.iter().map(|m| m.x).min().unwrap_or(0);
    let min_y = metas.iter().map(|m| m.y).min().unwrap_or(0);
    let max_right = metas.iter().map(|m| m.x + m.w as i32).max().unwrap_or(0);
    let max_bottom = metas.iter().map(|m| m.y + m.h as i32).max().unwrap_or(0);
    let virt_w = (max_right - min_x) as u32;
    let virt_h = (max_bottom - min_y) as u32;

    // 单显示器短路
    if metas.len() == 1 {
        #[cfg(windows)]
        let img = hdr::capture_monitor_with_hdr_fallback(0, &monitors)?;
        #[cfg(not(windows))]
        let mut img = monitors[0]
            .capture_image()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("截图失败: {}", e)))?;
        #[cfg(target_os = "macos")]
        macos_edr::apply_edr_correction(&mut img);
        let info = MonitorInfo {
            x: metas[0].x,
            y: metas[0].y,
            width: metas[0].w,
            height: metas[0].h,
            is_primary: metas[0].is_primary,
            name: metas[0].name.clone(),
        };
        return Ok(PerScreenCaptures {
            captures: vec![(img, info)],
            min_x: metas[0].x,
            min_y: metas[0].y,
            virt_w: metas[0].w,
            virt_h: metas[0].h,
        });
    }

    // 多显示器：各自独立捕获，不合成
    let num_monitors = metas.len();
    let captures = std::thread::scope(|s| {
        let mut handles = Vec::new();
        for i in 0..num_monitors {
            let meta = &metas[i];
            let info = MonitorInfo {
                x: meta.x,
                y: meta.y,
                width: meta.w,
                height: meta.h,
                is_primary: meta.is_primary,
                name: meta.name.clone(),
            };
            handles.push(s.spawn(move || {
                let all = xcap::Monitor::all()
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("截图失败: {}", e)))?;
                #[cfg(windows)]
                let img = hdr::capture_monitor_with_hdr_fallback(i, &all)?;
                #[cfg(not(windows))]
                let mut img = all[i]
                    .capture_image()
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("截图失败: {}", e)))?;
                #[cfg(target_os = "macos")]
                macos_edr::apply_edr_correction(&mut img);
                Ok::<_, AppError>((img, info))
            }));
        }
        handles
            .into_iter()
            .map(|h| {
                h.join().unwrap_or_else(|e| {
                    Err(AppError::Internal(anyhow::anyhow!(
                        "截图线程 panic: {:?}",
                        e
                    )))
                })
            })
            .collect::<Result<Vec<_>, _>>()
    })?;

    Ok(PerScreenCaptures {
        captures,
        min_x,
        min_y,
        virt_w,
        virt_h,
    })
}

/// Step 1：按显示器独立截图，返回虚拟桌面元数据。
/// hide_self=true 时先真正 hide 主窗口再截（"隐藏窗口截图"路径）；xcap/BitBlt 不保证尊重
/// WDA_EXCLUDEFROMCAPTURE，故此路径不再用 WDA 替代 hide，避免截图像素里仍含本程序窗口。
/// 普通截图（hide_self=false）逻辑不变。
/// restore_self（默认 true）：截完是否立即恢复主窗口显示。
/// - 主窗口内遮罩（旧回退路径）需要 true：遮罩渲染在主窗口里，必须让它可见。
/// - 独立遮罩窗口路径传 false：主窗口保持隐藏、由遮罩窗口覆盖，遮罩关闭后再由前端恢复，
///   避免"隐藏→立即显示一下→被遮罩盖住"的三段式闪烁。
/// 各显示器 JPEG 字节暂存于 AppState.pending_screenshots。
#[tauri::command]
pub async fn capture_start(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, crate::state::AppState>,
    hide_self: Option<bool>,
    restore_self: Option<bool>,
    request_id: Option<String>,
    presentation: Option<String>,
) -> Result<CaptureMetadata, AppError> {
    let should_minimize = hide_self.unwrap_or(false);
    let should_restore = restore_self.unwrap_or(true);
    log::info!(
        "[Screenshot] capture_start hide_self={:?} restore_self={:?} → minimize={} restore={}",
        hide_self,
        restore_self,
        should_minimize,
        should_restore
    );

    // 获取主窗口 HWND（隐藏窗口截图时用于从窗口列表排除自身）
    let hwnd: isize = {
        #[cfg(windows)]
        {
            app_handle
                .get_webview_window("main")
                .and_then(|w| w.hwnd().ok().map(|h| h.0 as isize))
                .unwrap_or(0)
        }
        #[cfg(not(windows))]
        {
            0
        }
    };

    // 隐藏窗口截图：移出屏幕 + SW_HIDE，再加长 settle，避免 WebView2 半透明残影进截图。
    // 普通截图不进入此分支。
    if should_minimize {
        #[cfg(windows)]
        {
            if hwnd != 0 {
                let placement = win32_enumerate::hide_offscreen(hwnd);
                *HIDDEN_MAIN_PLACEMENT.lock().unwrap() = placement;
                log::info!(
                    "[Screenshot] 隐藏窗口截图已移出屏幕+SW_HIDE hwnd={} placement={:?}",
                    hwnd,
                    placement
                );
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.hide();
                }
            } else if let Some(window) = app_handle.get_webview_window("main") {
                log::warn!("[Screenshot] 隐藏窗口截图未取到 HWND，回退 window.hide()");
                let _ = window.hide();
            }
            let _ = tokio::task::spawn_blocking(sync_after_hide_self_for_capture).await;
        }
        #[cfg(not(windows))]
        {
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.hide();
                let _ = tokio::task::spawn_blocking(sync_after_hide_self_for_capture).await;
            }
        }
    }

    let (raw_result, windows_result) = tokio::join!(
        tokio::task::spawn_blocking(capture_monitors_per_screen),
        tokio::task::spawn_blocking(enumerate_windows),
    );

    let per_screen =
        raw_result.map_err(|e| AppError::Internal(anyhow::anyhow!("截图线程异常: {}", e)))??;

    let mut windows = windows_result
        .map_err(|e| AppError::Internal(anyhow::anyhow!("窗口枚举线程异常: {}", e)))??;
    // 隐藏窗口截图：排除自身窗口，用户目标是选择背景窗口
    // 普通截图：保留自身窗口以维持完整 Z-order，确保 hitTestWindows 正确匹配
    if should_minimize {
        windows.retain(|w| w.handle != hwnd as u64);
    }

    if should_minimize && should_restore {
        // 主窗口内遮罩路径需要立即恢复显示；独立遮罩窗口路径保持隐藏，由前端在遮罩关闭后恢复。
        restore_main_after_hide_capture(&app_handle);
    }

    let (images, monitor_infos): (Vec<_>, Vec<_>) = per_screen.captures.into_iter().unzip();
    // 多屏 JPEG 并行编码：先把每屏编码任务派发到阻塞线程池，再统一 await。
    // 旧实现是 for 循环里逐个 await，多屏时编码串行化，拖慢首帧出现。
    let encode_handles: Vec<_> = images
        .into_iter()
        .map(|img| tokio::task::spawn_blocking(move || encode_jpeg_raw(&img)))
        .collect();
    let mut jpeg_vec = Vec::with_capacity(encode_handles.len());
    for handle in encode_handles {
        let jpeg = handle
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("编码线程异常: {}", e)))??;
        jpeg_vec.push(jpeg);
    }

    *state.pending_screenshots.lock().unwrap() = jpeg_vec;

    let metadata = CaptureMetadata {
        request_id,
        presentation,
        width: per_screen.virt_w,
        height: per_screen.virt_h,
        offset_x: per_screen.min_x,
        offset_y: per_screen.min_y,
        monitors: monitor_infos,
        windows,
    };

    // 供独立遮罩窗口通过 capture_take_pending 一次性取走
    *state.pending_metadata.lock().unwrap() = Some(metadata.clone());

    Ok(metadata)
}

/// 独立遮罩窗口一次性取走本次截图的元数据 + 各屏 JPEG base64 图像。
/// capture_start 已把两者暂存于 AppState；此处取出并清空，避免遮罩窗口分两次 IPC。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingCapture {
    pub metadata: CaptureMetadata,
    /// 每个显示器的 JPEG base64 data URL（与 metadata.monitors 顺序对应）
    pub images: Vec<String>,
}

#[tauri::command]
pub async fn capture_take_pending(
    state: tauri::State<'_, crate::state::AppState>,
    request_id: Option<String>,
) -> Result<PendingCapture, AppError> {
    let metadata = {
        let mut pending = state.pending_metadata.lock().unwrap();
        if request_id.is_some()
            && pending.as_ref().and_then(|meta| meta.request_id.as_ref()) != request_id.as_ref()
        {
            return Err(AppError::NotFound("截图请求已失效".into()));
        }
        // A late-created overlay must not steal the inline fallback's pixels.
        if pending.as_ref().and_then(|meta| meta.presentation.as_deref()) == Some("inline") {
            return Err(AppError::NotFound("当前截图由主窗口处理".into()));
        }
        pending
            .take()
            .ok_or_else(|| AppError::NotFound("无待取截图，请先调用 capture_start".into()))?
    };

    let jpegs = std::mem::take(&mut *state.pending_screenshots.lock().unwrap());
    let images = jpegs
        .iter()
        .map(|jpeg| {
            format!(
                "data:image/jpeg;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(jpeg)
            )
        })
        .collect();

    Ok(PendingCapture { metadata, images })
}

/// Discard only this request's remaining pixels; a stale cleanup cannot clear a later capture.
#[tauri::command]
pub async fn capture_discard_pending(
    state: tauri::State<'_, crate::state::AppState>,
    request_id: String,
) -> Result<(), AppError> {
    let mut pending = state.pending_metadata.lock().unwrap();
    if pending.as_ref().and_then(|meta| meta.request_id.as_deref()) == Some(request_id.as_str()) {
        *pending = None;
        state.pending_screenshots.lock().unwrap().clear();
    }
    Ok(())
}

/// Step 2：取回所有显示器的 JPEG base64 data URL（与 capture_start 返回的 monitors 顺序对应）。
/// 注：Tauri v2 将 Vec<u8> 通过 JSON 序列化为 number[]，new Blob([number[]]) 会产生 ASCII 文本；
///     base64 字符串传输更可靠，直接用作 <img src>。
#[tauri::command]
pub async fn capture_get_all_monitor_pixels(
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<Vec<String>, AppError> {
    let jpegs = std::mem::take(&mut *state.pending_screenshots.lock().unwrap());
    if jpegs.is_empty() {
        return Err(AppError::NotFound(
            "无待取截图，请先调用 capture_start".into(),
        ));
    }
    Ok(jpegs
        .iter()
        .map(|jpeg| {
            format!(
                "data:image/jpeg;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(jpeg)
            )
        })
        .collect())
}
