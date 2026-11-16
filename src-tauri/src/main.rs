// FastClaw 智能助手 · 主入口（修复版 v4）
//
// 修复历史：
// v1: spawn → block_on 修复 state not managed
// v2: install_default_drivers() 修复 fastkg sqlx::any panic
// v3: 从 fastclaw DB 读取 LLM/Embedding 配置，注入环境变量给 fastplan/fastkg
// v4: 修复 base_url /v1 重复拼接；修复 fastkg DB 路径(Windows)；
//     config.toml 不存在时自动生成；注入 FASTCLAW_EMBEDDING_DIM

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod commands;
mod core;
mod db;
mod http_server;
mod integrations;
mod kg_coordination;
mod services;
mod shared;
mod state;
mod utils;

use crate::core::config::ConfigManager;
use crate::core::session_plan::handler::AcpHandler;
use crate::db::Database;
use crate::state::AppState;
use std::sync::Arc;
use tauri::{Emitter, Manager, PhysicalPosition, Runtime, WebviewWindow};
use tauri_plugin_window_state::{AppHandleExt as WindowStateAppHandleExt, StateFlags};

/// 窗口状态：尺寸 / 最大化 / 全屏。
/// 不保存/恢复 POSITION：每次启动强制居中；不含 visible/decorations（由应用控制）。
fn window_state_flags() -> StateFlags {
    StateFlags::SIZE | StateFlags::MAXIMIZED | StateFlags::FULLSCREEN
}
/// 在 `process::exit` 之前显式落盘。插件默认只在 `RunEvent::Exit` 写文件，
/// 而本应用多处强制 `process::exit(0)`，会跳过 Exit，导致下次启动无法恢复上次大小。
fn persist_window_state(app: &tauri::AppHandle) {
    // 关闭流程会先 hide 再 exit：隐藏后 Win/WebView 可能上报 0 尺寸并污染插件缓存。
    // 已隐藏时跳过，避免把「上次正确尺寸」覆盖成 0（下次只能走自适应 1400×…）。
    if let Some(w) = app.get_webview_window("main") {
        match w.is_visible() {
            Ok(false) => {
                log::info!("[Window] 窗口已隐藏，跳过保存以免写入无效尺寸");
                return;
            }
            Err(e) => log::warn!("[Window] is_visible 失败: {e}，仍尝试保存"),
            Ok(true) => {}
        }
    }
    if let Err(e) = app.save_window_state(window_state_flags()) {
        log::warn!("[Window] 保存窗口状态失败: {e}");
    } else {
        log::info!("[Window] 窗口状态已保存");
    }
}

/// 存档是否含有效主窗口尺寸（宽高均 > 0）
fn has_valid_saved_window_state(cfg_dir: &std::path::Path) -> bool {
    let path = cfg_dir.join(".window-state.json");
    let Ok(bytes) = std::fs::read(&path) else {
        return false;
    };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return false;
    };
    let Some(main) = v.get("main") else {
        return false;
    };
    let w = main.get("width").and_then(|x| x.as_u64()).unwrap_or(0);
    let h = main.get("height").and_then(|x| x.as_u64()).unwrap_or(0);
    w > 0 && h > 0
}

/// 将应用图标挂到主窗口（含 Windows 任务栏 ICON_BIG / ICON_SMALL）。
/// 代码创建窗口不会自动继承 bundle icon；仅 set_icon 在部分环境下也不够，需 Win32 兜底。
fn apply_main_window_icon(window: &WebviewWindow, app: &tauri::AppHandle) {
    // 1) Tauri 内嵌默认图标
    if let Some(icon) = app.default_window_icon() {
        if let Err(e) = window.set_icon(icon.clone()) {
            log::warn!("[Window] set_icon(default) 失败: {e}");
        }
    } else {
        log::warn!("[Window] default_window_icon 为空");
    }

    // 2) 再强制用仓库内 icon.ico / png（需 Cargo feature image-ico / image-png）
    const ICON_ICO: &[u8] = include_bytes!("../icons/icon.ico");
    const ICON_PNG: &[u8] = include_bytes!("../icons/128x128.png");
    let embedded = tauri::image::Image::from_bytes(ICON_ICO).or_else(|e| {
        log::warn!("[Window] 解析 icon.ico 失败: {e}，尝试 png");
        tauri::image::Image::from_bytes(ICON_PNG)
    });
    match embedded {
        Ok(img) => {
            if let Err(e) = window.set_icon(img) {
                log::warn!("[Window] set_icon(embedded) 失败: {e}");
            } else {
                log::info!("[Window] set_icon(embedded) 成功");
            }
        }
        Err(e) => log::warn!("[Window] 解析内嵌图标失败: {e}"),
    }

    // 3) Windows：WM_SETICON 同时设置大/小图标，任务栏依赖 ICON_BIG
    #[cfg(windows)]
    {
        apply_win32_taskbar_icons(window);
    }
}

#[cfg(windows)]
fn apply_win32_taskbar_icons(window: &WebviewWindow) {
    let Ok(hwnd) = window.hwnd() else {
        log::warn!("[Window] 获取 HWND 失败，跳过 Win32 图标设置");
        return;
    };
    let hwnd = hwnd.0 as isize;

    extern "system" {
        fn GetAncestor(hwnd: isize, flags: u32) -> isize;
        fn LoadImageW(
            h_inst: isize,
            name: *const u16,
            type_: u32,
            cx: i32,
            cy: i32,
            fu_load: u32,
        ) -> isize;
        fn SendMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
    }
    const GA_ROOT: u32 = 2;
    // WebView 子窗口时取顶层 HWND，否则 WM_SETICON 不会反映到任务栏
    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    let target = if root != 0 { root } else { hwnd };

    // 将嵌入的 ico 落到临时文件，供 LoadImageW 使用
    let ico_path = std::env::temp_dir().join("fastclaw-app-icon.ico");
    if let Err(e) = std::fs::write(&ico_path, include_bytes!("../icons/icon.ico")) {
        log::warn!("[Window] 写入临时 icon.ico 失败: {e}");
        return;
    }
    let path_wide: Vec<u16> = ico_path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    const IMAGE_ICON: u32 = 1;
    const LR_LOADFROMFILE: u32 = 0x0010;
    const WM_SETICON: u32 = 0x0080;
    const ICON_SMALL: usize = 0;
    const ICON_BIG: usize = 1;

    unsafe {
        // cx/cy=0：使用 ICO 内实际尺寸，避免指定尺寸在资源中不存在导致加载失败
        let h_big = LoadImageW(0, path_wide.as_ptr(), IMAGE_ICON, 0, 0, LR_LOADFROMFILE);
        let h_small = LoadImageW(0, path_wide.as_ptr(), IMAGE_ICON, 32, 32, LR_LOADFROMFILE);
        let h_small = if h_small != 0 {
            h_small
        } else {
            LoadImageW(0, path_wide.as_ptr(), IMAGE_ICON, 16, 16, LR_LOADFROMFILE)
        };
        if h_big != 0 {
            SendMessageW(target, WM_SETICON, ICON_BIG, h_big);
            if target != hwnd {
                SendMessageW(hwnd, WM_SETICON, ICON_BIG, h_big);
            }
        } else {
            log::warn!("[Window] LoadImageW ICON_BIG 失败");
        }
        if h_small != 0 {
            SendMessageW(target, WM_SETICON, ICON_SMALL, h_small);
            if target != hwnd {
                SendMessageW(hwnd, WM_SETICON, ICON_SMALL, h_small);
            }
        } else {
            log::warn!("[Window] LoadImageW ICON_SMALL 失败");
        }
        if h_big != 0 || h_small != 0 {
            log::info!(
                "[Window] Win32 WM_SETICON 已应用 (hwnd={hwnd:#x} root={target:#x} big={} small={})",
                h_big != 0,
                h_small != 0
            );
        }
    }
}

/// 将窗口居中到主显示器工作区（排除任务栏）。最大化时不干预。
fn center_window_on_primary_work_area<R: Runtime>(window: &WebviewWindow<R>) {
    if window.is_maximized().unwrap_or(false) {
        return;
    }
    let Some(m) = window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| {
            window
                .available_monitors()
                .ok()
                .and_then(|ms| ms.into_iter().next())
        })
    else {
        return;
    };
    let scale = m.scale_factor();
    let wa = m.work_area();
    let ww = wa.size.width as f64 / scale;
    let wh = wa.size.height as f64 / scale;
    let Ok(outer) = window.outer_size() else {
        return;
    };
    let ow = outer.width as f64 / scale;
    let oh = outer.height as f64 / scale;
    let nx = wa.position.x as f64 / scale + ((ww - ow) / 2.0).max(0.0);
    let ny = wa.position.y as f64 / scale + ((wh - oh) / 2.0).max(0.0);
    let _ = window.set_position(tauri::LogicalPosition::new(nx, ny));
}

#[derive(Debug)]
enum SingleInstanceError {
    AlreadyRunning(u32),
    LockFile(std::io::Error),
}

/// 确保只有一个程序实例在运行。
///
/// 不能只判断锁文件中的 PID 是否存活：Windows 会复用 PID，崩溃遗留的锁文件
/// 可能恰好指向一个完全无关的系统进程，从而导致应用在启动阶段静默退出。
fn ensure_single_instance(app_data_dir: &std::path::Path) -> Result<(), SingleInstanceError> {
    let lock_path = app_data_dir.join(".fastclaw.lock");

    if let Ok(content) = std::fs::read_to_string(&lock_path) {
        if let Ok(pid) = content.trim().parse::<u32>() {
            // 必须确认是「本应用」仍在跑：PID 会被系统复用（例如微信 crashpad），
            // 只查 PID 存活会导致新实例误判已启动并立刻 exit(0)。
            if pid != std::process::id() && is_fastclaw_instance_alive(pid) {
                return Err(SingleInstanceError::AlreadyRunning(pid));
            }
        }
        // 锁文件过期（进程已不存在 / 非本应用）→ 清理
        let _ = std::fs::remove_file(&lock_path);
    }

    // 写入当前 PID
    std::fs::write(&lock_path, std::process::id().to_string())
        .map_err(SingleInstanceError::LockFile)?;
    Ok(())
}

/// 锁文件中的 PID 是否仍为 FastClaw 本进程（防 PID 复用误判）
fn is_fastclaw_instance_alive(pid: u32) -> bool {
    let mut s = sysinfo::System::new();
    s.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let Some(proc) = s.process(sysinfo::Pid::from(pid as usize)) else {
        return false;
    };
    let name = proc.name().to_string_lossy().to_ascii_lowercase();
    // Windows 上多为 FastClaw.exe；也兼容无后缀 / 调试名
    if name.contains("fastclaw") {
        return true;
    }
    // 再按可执行路径兜底（防止改名后的正式包）
    if let Some(exe) = proc.exe() {
        let exe_s = exe.to_string_lossy().to_ascii_lowercase();
        if exe_s.contains("fastclaw") {
            return true;
        }
    }
    false
}

/// 激活已有实例的窗口并将其置于前台
fn activate_existing_instance(pid: u32) {
    #[cfg(windows)]
    {
        use crate::utils::win32::{
            AllowSetForegroundWindow, BringWindowToTop, EnumWindows, GetWindowTextLengthW,
            GetWindowThreadProcessId, Hwnd, IsIconic, IsWindowVisible, Lparam, SetForegroundWindow,
            ShowWindow,
        };
        unsafe {
            const SW_RESTORE: i32 = 9;

            #[repr(C)]
            struct Ctx {
                pid: u32,
                hwnd: Hwnd,
            }

            unsafe extern "system" fn enum_callback(hwnd: Hwnd, lparam: Lparam) -> i32 {
                let ctx = &mut *(lparam as *mut Ctx);
                if IsWindowVisible(hwnd) == 0 {
                    return 1;
                }
                let mut wnd_pid = 0u32;
                GetWindowThreadProcessId(hwnd, &mut wnd_pid);
                if wnd_pid == ctx.pid && GetWindowTextLengthW(hwnd) > 0 {
                    ctx.hwnd = hwnd;
                    return 0; // 找到，停止枚举
                }
                1 // 继续
            }

            let mut ctx = Ctx { pid, hwnd: 0 };
            EnumWindows(Some(enum_callback), &mut ctx as *mut Ctx as Lparam);

            if ctx.hwnd != 0 {
                AllowSetForegroundWindow(pid);
                if IsIconic(ctx.hwnd) != 0 {
                    ShowWindow(ctx.hwnd, SW_RESTORE);
                }
                BringWindowToTop(ctx.hwnd);
                SetForegroundWindow(ctx.hwnd);
            }
        }
    }
    #[cfg(not(windows))]
    {
        log::info!("程序已在运行中 (PID: {})", pid);
    }
}

/// 多显示器切换 / 任务栏遮挡兜底：窗口状态插件在 `window_created`（主线程）里恢复上次位置，
/// 本函数通过 `run_on_main_thread` 排在插件恢复之后执行，检测窗口外框是否与任何显示器
/// 工作区（排除任务栏）相交，完全落在所有工作区之外则居中到主显示器工作区。
fn clamp_window_to_visible<R: Runtime>(window: &WebviewWindow<R>) {
    // 最大化窗口由 OS 管理位置，不干预
    if window.is_maximized().unwrap_or(false) {
        return;
    }
    let monitors = match window.available_monitors() {
        Ok(ms) if !ms.is_empty() => ms,
        _ => return,
    };
    let pos = match window.outer_position() {
        Ok(p) => p,
        Err(_) => return,
    };
    let size = match window.outer_size() {
        Ok(s) => s,
        Err(_) => return,
    };
    let on_screen = monitors.iter().any(|m| {
        let wa = m.work_area();
        let left = wa.position.x;
        let top = wa.position.y;
        let right = left + wa.size.width as i32;
        let bottom = top + wa.size.height as i32;
        let (px, py) = (pos.x, pos.y);
        let (pw, ph) = (size.width as i32, size.height as i32);
        px < right && px + pw > left && py < bottom && py + ph > top
    });
    if on_screen {
        return;
    }
    // 完全落在工作区之外：居中到主显示器（取不到则第一个）工作区
    let target = window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| monitors.into_iter().next());
    let Some(m) = target else { return };
    let wa = m.work_area();
    let nx = wa.position.x + ((wa.size.width as i32 - size.width as i32) / 2);
    let ny = wa.position.y + ((wa.size.height as i32 - size.height as i32) / 2);
    let _ = window.set_position(PhysicalPosition::new(nx, ny));
}

fn main() {
    crate::utils::logging::init_logging();

    // 获取应用数据目录，用于配置回退和单实例检查。
    let app_data_dir = {
        let dir = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("com.fastclaw.app");
        std::fs::create_dir_all(&dir).ok();
        dir
    };
    // WebView 创建前必须完成运行时检查，因此 config.json 也要在 Builder 之前加载。
    // 后续 setup 复用同一个 ConfigManager，避免启动和运行阶段读取两份配置。
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| app_data_dir.clone());
    let app_config = Arc::new(ConfigManager::new(&exe_dir).expect("应用配置加载失败"));
    log::info!(
        "[Config] 配置文件路径: {}",
        app_config.file_path().display()
    );

    #[cfg(windows)]
    {
        // 尽早设置 AUMID，任务栏图标分组依赖它（尤其是无安装包的 cargo/tauri dev 进程）
        extern "system" {
            fn SetCurrentProcessExplicitAppUserModelID(app_id: *const u16) -> i32;
        }
        let aumid: Vec<u16> = "com.fastclaw.app\0".encode_utf16().collect();
        unsafe {
            let hr = SetCurrentProcessExplicitAppUserModelID(aumid.as_ptr());
            if hr < 0 {
                log::warn!("[Window] SetCurrentProcessExplicitAppUserModelID 失败: {hr:#x}");
            }
        }
        let initial_config = app_config.get_initial_clone();
        crate::utils::webview2::check_webview2_version(&initial_config.webview2);
    }
    log::info!("FastClaw v{} 启动中...", env!("CARGO_PKG_VERSION"));

    match ensure_single_instance(&app_data_dir) {
        Ok(()) => {}
        Err(SingleInstanceError::AlreadyRunning(pid)) => {
            log::info!(
                "检测到已运行的 FastClaw 实例 (PID {})，激活现有窗口后退出",
                pid
            );
            activate_existing_instance(pid);
            std::process::exit(0);
        }
        Err(SingleInstanceError::LockFile(e)) => {
            log::error!(
                "单实例锁文件创建失败 ({}): {}",
                app_data_dir.join(".fastclaw.lock").display(),
                e
            );
            // 锁文件不可写不应让程序无提示退出；继续启动，并保留错误日志供诊断。
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        // 记住主窗口尺寸/位置/最大化/全屏等状态，跨重启恢复。首次启动（或版本更新后首次）
        // 无存档时，由下方 run_on_main_thread 闭包按主显示器工作区计算自适应默认尺寸；
        // 恢复后若完全落到当前显示器工作区之外（如拔掉外接屏），由下方 clamp_window_to_visible 兜底居中。
        // 不跟踪 VISIBLE/DECORATIONS：启动时 visible(false) + decorations(false) 由应用控制。
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(window_state_flags())
                .build(),
        )
        .setup(move |app| {
            let handle = app.handle().clone();

            // 程序更新后首次启动：清除旧窗口状态文件，走自动尺寸配置；同版本其他启动恢复记忆。
            // 以 tauri.conf.json 的 version 作为版本标记——发版必须 bump 该字段才会触发重置。
            // cfg_dir 提到版本检查块之外，供下方 run_on_main_thread 闭包判断 .window-state.json 存档是否存在。
            let cfg_dir_opt = app.path().app_config_dir().ok();
            {
                let current_ver = app.package_info().version.to_string();
                if let Some(cfg_dir) = &cfg_dir_opt {
                    let state_path = cfg_dir.join(".window-state.json");
                    let ver_path = cfg_dir.join(".window-state-version");
                    if std::fs::read_to_string(&ver_path).unwrap_or_default().trim() != current_ver {
                        let _ = std::fs::remove_file(&state_path);
                        let _ = std::fs::write(&ver_path, &current_ver);
                    } else if !has_valid_saved_window_state(cfg_dir) {
                        // 无效存档（如 hide 后写入的 0×0）清掉，避免既不恢复也不走自适应
                        let _ = std::fs::remove_file(&state_path);
                    }
                }
            }

            // ── 编程创建主窗口（替代 tauri.conf.json 中的 windows 配置）──
            // Tauri WebView 默认阻止 window.open，外链需要显式放行。
            // inner_size(1400, 900) 仅作为窗口创建占位值：有存档时由 window-state 插件
            // 恢复上次关闭状态，无存档（首启 / 版本更新后）时由下方自适应尺寸覆盖。
            // 注意：windows 配置为空时必须手动 .icon()，否则 Windows 任务栏会显示通用空白图标。
            const DEFAULT_W: f64 = 1400.0;
            const DEFAULT_H: f64 = 900.0;

            let mut builder = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("FastClaw - 智能助手")
            .inner_size(DEFAULT_W, DEFAULT_H)
            .min_inner_size(400.0, 300.0)
            .resizable(true)
            .decorations(false)
            // 右键粘贴等会调用 clipboard.readText；开启后 WebView2 自动放行剪贴板读权限，不弹系统询问框
            .enable_clipboard_access()
            .on_new_window(|_url, _features| {
                tauri::webview::NewWindowResponse::Allow
            })
            // 拦截 WebView 原生下载（含 <audio> 更多选项 → 下载），弹出另存为，默认本机「下载」目录
            .on_download(|webview, event| {
                use tauri::webview::DownloadEvent;
                use tauri_plugin_dialog::DialogExt;
                match event {
                    DownloadEvent::Requested { url, destination } => {
                        let app = webview.app_handle().clone();
                        let suggested = destination
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| s.to_string())
                            .filter(|s| !s.is_empty() && s != "download")
                            .or_else(|| {
                                url.path_segments()
                                    .and_then(|mut s| s.next_back())
                                    .map(|s| s.to_string())
                                    .filter(|s| !s.is_empty() && !s.contains(':'))
                            })
                            .unwrap_or_else(|| "tts_audio.mp3".to_string());

                        let ext = std::path::Path::new(&suggested)
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("mp3")
                            .to_string();

                        let mut dialog = app
                            .dialog()
                            .file()
                            .set_file_name(&suggested)
                            .add_filter("Audio", &[&ext, "mp3", "wav", "ogg", "m4a"]);

                        if let Ok(dl) = app.path().download_dir() {
                            dialog = dialog.set_directory(dl);
                        }

                        match dialog.blocking_save_file() {
                            Some(path) => {
                                if let Some(p) = path.as_path() {
                                    *destination = p.to_path_buf();
                                    true
                                } else {
                                    false
                                }
                            }
                            None => false,
                        }
                    }
                    DownloadEvent::Finished { url, path, success } => {
                        log::info!(
                            "[Download] finished url={} path={:?} success={}",
                            url,
                            path,
                            success
                        );
                        true
                    }
                    _ => true,
                }
            })
            .visible(false);

            // 创建前挂上图标（比事后 set_icon 更可靠）
            const ICON_ICO: &[u8] = include_bytes!("../icons/icon.ico");
            if let Some(icon) = handle.default_window_icon() {
                builder = builder.icon(icon.clone()).expect("设置窗口图标失败");
            } else if let Ok(img) = tauri::image::Image::from_bytes(ICON_ICO) {
                builder = builder.icon(img).expect("设置窗口图标失败");
            }

            let window = builder.build().expect("无法创建主窗口");

            // 创建后先设一次；显示后再设一次（任务栏图标常在 show 后才生效）
            apply_main_window_icon(&window, &handle);

            // 多显示器切换 / 任务栏遮挡兜底：窗口状态插件在 `window_created`（主线程）里
            // 恢复上次尺寸，这里同样排到主线程、必在其后执行，再强制居中到主显示器工作区。
            let window_clone = window.clone();
            let handle_for_icon = handle.clone();
            if let Err(e) = window.run_on_main_thread(move || {
                // 窗口创建时为 visible(false)，须在此显式显示。此闭包经 run_on_main_thread
                // 排在 window-state 插件恢复之后执行，故首帧即在正确尺寸，不闪默认尺寸。
                // 无存档（首次启动 / 用户已删 .window-state.json）时，用主显示器工作区自适应默认尺寸。
                // setup 阶段 monitor 可能尚未枚举，故在此窗口已创建后计算，确保正确且不暴涨。
                let have_state = cfg_dir_opt
                    .as_ref()
                    .map(|d| has_valid_saved_window_state(d))
                    .unwrap_or(false);
                if !have_state {
                    if let Some(m) = window_clone.primary_monitor().ok().flatten() {
                        let scale = m.scale_factor();
                        let wa = m.work_area();
                        let ww = wa.size.width as f64 / scale;
                        let wh = wa.size.height as f64 / scale;
                        let h = (wh - 200.0).max(600.0);            // 默认高度 = 工作区高 - 上下各 100px
                        let w = (ww * 0.85).clamp(900.0, 1400.0);   // 宽度：大屏 1400 / 小屏按工作区 85% 收窄
                        let _ = window_clone.set_size(tauri::LogicalSize::new(w, h));
                    }
                }
                // 每次启动都居中（含有存档恢复尺寸之后）
                center_window_on_primary_work_area(&window_clone);
                let _ = window_clone.show();
                let _ = window_clone.set_focus();
                clamp_window_to_visible(&window_clone);
                // show 后再设图标，确保任务栏拿到 ICON_BIG
                apply_main_window_icon(&window_clone, &handle_for_icon);
            }) {
                log::warn!("[Window] run_on_main_thread 调度失败: {:?}，尝试直接显示主窗口", e);
                let _ = window.show();
                let _ = window.set_focus();
            }

            // 兜底：若主线程回调因时序未真正显示，延迟再 show 一次，避免进程在任务管理器里、窗口却一直隐藏
            let window_fallback = window.clone();
            let handle_fallback = handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(800));
                if let Err(e) = window_fallback.show() {
                    log::warn!("[Window] 延迟显示主窗口失败: {}", e);
                } else {
                    let _ = window_fallback.set_focus();
                }
                // 延迟再刷一次任务栏图标（部分机器 show 后图标仍被重置）
                let win_for_icon = window_fallback.clone();
                let _ = window_fallback.run_on_main_thread(move || {
                    apply_main_window_icon(&win_for_icon, &handle_fallback);
                });
            });

            // ── WebView2 权限拦截：自动放行麦克风 / 剪贴板（消除 tauri.localhost 权限弹窗）──
            // 原理：前端 getUserMedia / clipboard.readText 触发时 WebView2 发出 PermissionRequested，
            // 在此拦截并直接设置为 ALLOW，跳过浏览器原生权限询问 UI。
            #[cfg(target_os = "windows")]
            {
                use webview2_com::Microsoft::Web::WebView2::Win32::*;
                use webview2_com::PermissionRequestedEventHandler;

                // with_webview 提供 PlatformWebview（wry::WebView），
                // 其 .controller() 方法返回 ICoreWebView2Controller COM 接口。
                if let Err(e) = window.with_webview(|webview| {
                    unsafe {
                        let controller = webview.controller();
                        match controller.CoreWebView2() {
                            Ok(core) => {
                                // token 用于后续可能的取消注册（应用生命周期内保持有效）
                                let mut token: i64 = 0;
                                let handler: ICoreWebView2PermissionRequestedEventHandler =
                                    PermissionRequestedEventHandler::create(Box::new(
                                        move |_sender: Option<ICoreWebView2>,
                                              args: Option<ICoreWebView2PermissionRequestedEventArgs>| {
                                            if let Some(args) = args {
                                                let mut kind = COREWEBVIEW2_PERMISSION_KIND_UNKNOWN_PERMISSION;
                                                if args.PermissionKind(&mut kind).is_ok()
                                                    && (kind == COREWEBVIEW2_PERMISSION_KIND_MICROPHONE
                                                        || kind
                                                            == COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ)
                                                {
                                                    log::info!(
                                                        "[WebView2] 自动放行权限请求: kind={:?}",
                                                        kind
                                                    );
                                                    args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW).ok();
                                                }
                                            }
                                            Ok(())
                                        },
                                    ));
                                if let Err(hr) = core.add_PermissionRequested(&handler, &mut token) {
                                    log::warn!("[WebView2] 注册 PermissionRequested 回调失败: {:?}", hr);
                                } else {
                                    log::info!(
                                        "[WebView2] ✅ PermissionRequested 拦截已注册，麦克风/剪贴板权限将自动放行"
                                    );
                                }
                            }
                            Err(hr) => {
                                log::warn!("[WebView2] 获取 CoreWebView2 失败: {:?}", hr);
                            }
                        }
                    }
                }) {
                    log::warn!("[WebView2] with_webview 调用失败: {:?}", e);
                }
            }

            let app_data_dir = app.path().app_data_dir()
                .expect("无法获取应用数据目录");
            std::fs::create_dir_all(&app_data_dir).expect("无法创建应用数据目录");

            tauri::async_runtime::block_on(async move {
                let db = Database::init()
                    .await
                    .expect("数据库初始化失败");

                // 分类依据数据
                #[derive(Debug, Clone, serde::Deserialize)]
                struct CategoryChild {
                    #[serde(deserialize_with = "deser_id_str")]
                    id: i64,
                    name: String,
                    code: String,
                }
                #[derive(Debug, Clone, serde::Deserialize)]
                struct CategoryType {
                    #[serde(deserialize_with = "deser_id_str")]
                    id: i64,
                    name: String,
                    code: String,
                    children: Vec<CategoryChild>,
                }
                fn deser_id_str<'de, D>(deserializer: D) -> Result<i64, D::Error>
                where D: serde::Deserializer<'de>
                {
                    use serde::de;
                    let v: serde_json::Value = serde::Deserialize::deserialize(deserializer)?;
                    match v {
                        serde_json::Value::Number(n) => n.as_i64().ok_or_else(|| de::Error::custom("not a valid integer")),
                        serde_json::Value::String(s) => s.parse::<i64>().map_err(|_| de::Error::custom(format!("cannot parse '{}' as integer", s))),
                        _ => Err(de::Error::custom("expected number or string")),
                    }
                }
                // ── 异步调用 fastci API 获取分类树 + 同步模型数据（不阻塞启动）──
                let fastci_base_url = std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|d| d.join("config.json")))
                    .and_then(|p| std::fs::read_to_string(&p).ok())
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                    .and_then(|v| v.get("remote_servers")?.get("fastci_base_url")?.as_str().map(String::from))
                    .unwrap_or_else(|| crate::core::config::DEFAULT_FASTCI_BASE_URL.to_string());
                let db_for_wsp = db.clone();
                tokio::spawn(async move {
                    if fastci_base_url.trim().is_empty() {
                        return;
                    }
                    // 1. 获取分类树
                    // reqwest::get(format!("{}/api/wsp/list", fastci_base_url)).await { /api/wsp/category/tree
                    let category_types: Vec<CategoryType> =
                        match reqwest::get(format!("{}/api/wsp/category/tree", fastci_base_url)).await {
                            Ok(resp) => match resp.text().await {
                                Ok(body) => {
                                    // log::info!("[fastci] 分类树 API 返回:\n{}", body);
                                    match serde_json::from_str::<serde_json::Value>(&body) {
                                        Ok(json) => json
                                            .get("result")
                                            .and_then(|data| {
                                                serde_json::from_value::<Vec<CategoryType>>(data.clone()).ok()
                                            })
                                            .or_else(|| {
                                                serde_json::from_value::<Vec<CategoryType>>(json).ok()
                                            })
                                            .unwrap_or_default(),
                                        Err(e) => {
                                            log::warn!("[fastci] 分类树 JSON 解析失败: {}", e);
                                            Vec::new()
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("[fastci] 分类树 API 响应读取失败: {}", e);
                                    Vec::new()
                                }
                            },
                            Err(e) => {
                                log::warn!("[fastci] 分类树 API 请求失败: {}", e);
                                Vec::new()
                            }
                        };
                    log::info!("[fastci] 分类树加载完成: {} 个分类", category_types.len());

                    // 分类树失败时不做全量同步：sync_wsp_market_models 会先 DELETE 再插入，
                    // 无分类名写入会把本地已有的 categoryName/subCategoryName 清空。
                    if category_types.is_empty() {
                        log::warn!(
                            "[fastci] 分类树为空，跳过市场模型全量同步，保留本地已有分类名与模型数据"
                        );
                    } else {
                        // 2. 获取模型列表（仅在分类树可用时同步）
                        if let Ok(resp) = reqwest::get(format!("{}/api/wsp/list", fastci_base_url)).await {
                            if let Ok(body) = resp.text().await {
                                // log::info!("[fastci] 模型列表 API 返回:\n{}", body);
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                                    if let Some(arr) = json.get("result").and_then(|d| d.as_array()) {
                                        let mut models: Vec<crate::db::repo::models::WspMarketModel> = Vec::new();
                                        for item in arr {
                                            match serde_json::from_value::<crate::db::repo::models::WspMarketModel>(item.clone()) {
                                                Ok(m) => models.push(m),
                                                Err(e) => log::warn!("[fastci] 跳过无效模型 (id={:?}): {}", item.get("id").and_then(|v| v.as_str()).unwrap_or("?"), e),
                                            }
                                        }

                                        for m in &mut models {
                                            for ct in &category_types {
                                                if m.categoryId == ct.id {
                                                    m.categoryName = Some(ct.name.clone());
                                                    for child in &ct.children {
                                                        if m.subCategoryId == child.id {
                                                            m.subCategoryName = Some(child.name.clone());
                                                            break;
                                                        }
                                                    }
                                                    break;
                                                }
                                            }
                                        }

                                        if !models.is_empty() {
                                            log::info!("[fastci] 同步: {} 个模型", models.len());
                                            if let Err(e) = crate::db::repo::models::sync_wsp_market_models(&*db_for_wsp.pool().await, &models).await {
                                                log::warn!("[fastci] 写入失败: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                });

                inject_provider_env_vars(&*db.pool().await).await;

                // ── 应用配置已在 WebView2 检查前加载，此处继续复用 ──
                {
                    let config = app_config.read().await;
                    crate::commands::wsp::configure_dependency_preferred_url("qdrant", &config.services.qdrant);
                    crate::commands::wsp::configure_dependency_preferred_url("llama_server", &config.services.llama_server);
                    crate::commands::wsp::configure_dependency_preferred_url("fastkg", &config.services.fastkg);
                    crate::commands::wsp::configure_dependency_preferred_url("fastplan", &config.services.fastplan);
                }

                let kg_pids: std::sync::Arc<tokio::sync::RwLock<crate::state::KgServicePids>> =
                    Default::default();
                let mut app_state = AppState::new(db.clone(), kg_pids.clone(), app_config.clone())
                    .await
                    .expect("应用状态初始化失败");

                // ── 从 DB 加载已保存的 MCP Server 到运行时 ──
                {
                    let pool = db.pool().await;
                    if let Ok(servers) = crate::db::repo::mcp::list(&*pool, None).await {
                        let mut auto_start_ids: Vec<String> = Vec::new();
                        {
                            let mut manager = app_state.mcp.write().await;
                            for (id, raw_config) in &servers {
                                if let Ok(config) = crate::commands::mcp::parse_mcp_config(raw_config) {
                                    // auto_start 此前只被解析和存储、没有任何消费者，
                                    // 标了自动启动的 server 也停在 Stopped，而
                                    // list_all_tools() 只返回 Running 的——问答链路的
                                    // MCP 池因此恒为空。这里补上消费者。
                                    let wants_auto_start = config.auto_start && config.enabled;
                                    manager.register_server(id.clone(), config);
                                    if wants_auto_start {
                                        auto_start_ids.push(id.clone());
                                    }
                                }
                            }
                            log::info!("📦 从 DB 加载了 {} 个 MCP Server 到运行时", servers.len());
                        }
                        // 写锁已释放再连接：连接是 I/O，不能持写锁做
                        for id in auto_start_ids {
                            let mcp = app_state.mcp.clone();
                            tauri::async_runtime::spawn(async move {
                                match crate::core::mcp::connect_server_shared(&mcp, &id).await {
                                    Ok(_) => log::info!("🚀 MCP Server 已自动连接: {}", id),
                                    Err(e) => log::warn!("MCP Server {} 自动连接失败: {}", id, e),
                                }
                            });
                        }
                    }
                }

                // ── 系统托盘初始化 ──
                let tray_setting = sqlx::query_scalar::<_, String>(
                    "SELECT value FROM settings WHERE key = 'system_tray'"
                )
                .fetch_optional(&*db.pool().await)
                .await
                .ok()
                .flatten();

                let tray_enabled = tray_setting.as_deref() == Some("1");

                if tray_enabled {
                    if let Err(e) = commands::system_tray::setup_system_tray_with_handle(&handle) {
                        log::warn!("[系统托盘] 初始化失败: {}", e);
                    } else {
                        log::info!("[系统托盘] 已启用，关闭窗口将最小化到托盘");
                    }
                }

                // ── BERT 意图路由模型加载 ──
                {
                    let intent_model_dir = app_data_dir.join("intent-model");
                    // 首次启动：从资源目录复制模型文件到数据目录
                    if !intent_model_dir.exists() {
                        if let Err(e) = std::fs::create_dir_all(&intent_model_dir) {
                            log::warn!("[IntentRouter] 无法创建模型目录: {}", e);
                        } else {
                            let resource_dir = handle
                                .path()
                                .resource_dir()
                                .unwrap_or_default()
                                .join("intent-model");
                            for file_name in &["config.json", "model.safetensors", "tokenizer.json"] {
                                let src = resource_dir.join(file_name);
                                let dst = intent_model_dir.join(file_name);
                                if src.exists() && !dst.exists() {
                                    if let Err(e) = std::fs::copy(&src, &dst) {
                                        log::warn!("[IntentRouter] 复制 {} 失败: {}", file_name, e);
                                    } else {
                                        log::info!("[IntentRouter] 已复制 {} 到数据目录", file_name);
                                    }
                                }
                            }
                        }
                    }
                    if intent_model_dir.join("model.safetensors").exists() {
                        match crate::core::intent::IntentRouter::load(
                            &intent_model_dir,
                            crate::core::intent::IntentConfig::default(),
                        ) {
                            Ok(router) => {
                                log::info!("[IntentRouter] BERT 模型加载成功，意图路由已启用");
                                app_state.intent_router = Some(std::sync::Arc::new(router));
                            }
                            Err(e) => {
                                log::warn!("[IntentRouter] BERT 模型加载失败: {e}，意图路由已禁用");
                            }
                        }
                    } else {
                        log::info!(
                            "[IntentRouter] 模型目录不存在 ({}), 意图路由未启用",
                            intent_model_dir.display()
                        );
                    }
                }

                // 确保 kg_storage 目录存在（HTTP 文件上传 fallback 需要）
                {
                    let kg_storage = app_data_dir.join("kg_storage");
                    std::fs::create_dir_all(&kg_storage).ok();
                }

                // ── 异步自动启动标记了 auto_start 的 WSP 服务（不阻塞主流程）──
                {
                    let wsp_for_auto = app_state.wsp.clone();
                    tokio::spawn(async move {
                        if let Err(e) = wsp_for_auto.start_all_auto().await {
                            log::warn!("[WSP] 自动启动失败: {}", e);
                        }
                    });
                }

                let db_for_http = app_state.db.clone();
                // ── 注册默认的窗口关闭事件（非托盘模式时清理子进程） ──
                {
                    let kg_pids_for_close = kg_pids.clone();
                    let wsp_for_close = app_state.wsp.clone();
                    let mcp_for_close = app_state.mcp.clone();
                    let cleanup_started =
                        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                    if let Some(window) = handle.get_webview_window("main") {
                        let w = window.clone();
                        window.on_window_event(move |event| {
                            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                                // 启动阶段：阻止任何关闭/隐藏窗口的行为
                                if !crate::state::STARTUP_COMPLETE.load(std::sync::atomic::Ordering::SeqCst) {
                                    log::warn!("[Main] 启动阶段收到 CloseRequested，已阻止");
                                    api.prevent_close();
                                    return;
                                }
                                // 托盘已启用：CloseRequested 由托盘 handler 处理（隐藏到托盘），此处放行
                                if crate::commands::system_tray::TRAY_ENABLED
                                    .load(std::sync::atomic::Ordering::SeqCst)
                                {
                                    return;
                                }
                                // 防止重入：w2.close() 会再次触发 CloseRequested
                                if cleanup_started
                                    .swap(true, std::sync::atomic::Ordering::SeqCst)
                                {
                                    log::info!("[Shutdown] 清理已完成（重入检测），放行关闭");
                                    return;
                                }
                                // 前端已通过 exit_app 完成清理，直接退出
                                if crate::state::SHUTDOWN_DONE
                                    .load(std::sync::atomic::Ordering::SeqCst)
                                {
                                    log::info!("[Shutdown] 前端已完成清理，直接退出");
                                    persist_window_state(&w.app_handle());
                                    std::process::exit(0);
                                }
                                log::info!("[Shutdown] 窗口关闭，开始清理子进程...");
                                api.prevent_close();
                                let kg = kg_pids_for_close.clone();
                                let wsp = wsp_for_close.clone();
                                let mcp = mcp_for_close.clone();
                                let w2 = w.clone();
                                // KG 接管停止最多需要依次等待三个注册进程退出。
                                // 超时保护提高到 15 秒，避免安全清理尚未完成就强制退出。
                                let cleanup_done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                                let cleanup_done_clone = cleanup_done.clone();
                                let app_for_timeout = w.app_handle().clone();
                                std::thread::spawn(move || {
                                    std::thread::sleep(std::time::Duration::from_secs(15));
                                    if !cleanup_done_clone.load(std::sync::atomic::Ordering::SeqCst) {
                                        log::error!("[Shutdown] 清理超时，强制退出");
                                        persist_window_state(&app_for_timeout);
                                        std::process::exit(0);
                                    }
                                });
                                tauri::async_runtime::block_on(async {
                                    let recovery = w2.app_handle().state::<AppState>().dependency_recovery.clone();
                                    recovery.shutdown();
                                    let _recovery_operation = recovery.operation.lock().await;
                                    log::info!("[Shutdown] 终止 FastPlan 进程...");
                                    let _ = crate::commands::wsp::stop_fastplan_inner(&kg).await;
                                    log::info!("[Shutdown] 停止 KG 服务...");
                                    let _ = crate::commands::wsp::stop_kg_inner(&kg).await;
                                    log::info!("[Shutdown] 断开 MCP 连接...");
                                    crate::core::mcp::disconnect_all_shared(&mcp).await;
                                    log::info!("[Shutdown] 停止 WSP 服务...");
                                    let _ = wsp.shutdown().await;
                                });
                                cleanup_done.store(true, std::sync::atomic::Ordering::SeqCst);
                                log::info!("[Shutdown] 清理完成，退出程序");
                                persist_window_state(&w2.app_handle());
                                std::process::exit(0);
                            }
                        });
                    }
                }

                // ── ACP 处理器（桌面端「停止生成」与规划确认用）──
                {
                    let (acp_handler, _event_rx) = crate::core::session_plan::handler::FastClawAcpHandler::new(
                        db.clone(),
                        app_state.llm.clone(),
                        app_state.mcp.clone(),
                        app_state.kg_pids.clone(),
                    );
                    app_state.acp_handler = Some(acp_handler.clone());
                }

                handle.manage(app_state);
                let download_database = Arc::new(
                    crate::integrations::download_center_plugin::HostSharedDatabase(db.clone()),
                );
                handle.plugin(tauri_plugin_download_center::init_with_install_hook(download_database,
                    Default::default(), Some(Arc::new(core::dependency_services::install::KgInstallHook))))
                    .expect("download-center plugin initialization failed");
                log::info!("FastClaw 初始化完成 ✅");
                crate::state::STARTUP_COMPLETE.store(true, std::sync::atomic::Ordering::SeqCst);


                // ── HTTP Server: 动态端口 fallback（端口范围从 config.json 读取）──
                // ★ 使用 Notify 确保 setup 不会在 HTTP 服务器端口绑定之前完成，
                //    避免前端首次请求时服务器尚未就绪导致 "Network error"。
                let http_ready = std::sync::Arc::new(tokio::sync::Notify::new());
                let http_ready_for_server = http_ready.clone();
                let final_router = crate::http_server::create_router(app_config.clone()).with_state(db_for_http);
                let app_handle_for_port = handle.clone();
                let http_port_arc = app_handle_for_port.state::<AppState>().http_port.clone();
                let app_config_for_http = app_config.clone();

                tokio::spawn(async move {
                    let cfg = app_config_for_http.read().await;
                    let port_start = cfg.http_server.port_start;
                    let port_end = cfg.http_server.port_end;
                    let bind_host = cfg.http_server.host.clone();
                    drop(cfg);
                    let mut bound_port: Option<u16> = None;
                    for port in port_start..=port_end {
                        match tokio::net::TcpListener::bind(
                            format!("{}:{}", bind_host, port)
                        ).await {
                            Ok(listener) => {
                                bound_port = Some(port);
                                http_port_arc.store(port, std::sync::atomic::Ordering::SeqCst);
                                // ★ 通知主线程：端口已绑定，可以安全放行前端请求
                                http_ready_for_server.notify_one();
                                log::info!("[HTTP Server] REST API 已启动: http://127.0.0.1:{}", port);
                                // 通知前端实际端口
                                let _ = app_handle_for_port.emit("http-port-bound", serde_json::json!({ "port": port }));
                                if let Err(e) = axum::serve(listener, final_router).await {
                                    log::error!("[HTTP Server] 运行出错: {}", e);
                                }
                                break;
                            }
                            Err(e) => {
                                log::warn!("[HTTP Server] 端口 {} 绑定失败: {}，尝试下一个", port, e);
                            }
                        }
                    }
                    if bound_port.is_none() {
                        log::error!("[HTTP Server] 所有端口 ({}-{}) 绑定失败，REST API 不可用", port_start, port_end);
                        // 即使绑定失败也通知，避免 setup 永远等待
                        http_ready_for_server.notify_one();
                    }
                });

                // ★ 等待 HTTP 服务器端口绑定完成再结束 setup（最多 5 秒超时）
                //    超时不阻断启动——极端情况下端口全部被占，程序仍能打开但 REST API 不可用。
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    http_ready.notified(),
                ).await;

                // 五个依赖服务统一由后端恢复管理器启动，确保 HTTP 入口已就绪。
                let recovery = handle.state::<AppState>().dependency_recovery.clone();
                recovery.start(handle.clone()).await;

                // ── 网络连通性检测 ──
                let db_for_connectivity = db.clone();
                tokio::spawn(async move {
                    let check_url = crate::core::constants::CONNECTIVITY_CHECK_URL;
                    let client = reqwest::Client::builder()
                        .timeout(std::time::Duration::from_secs(5))
                        .build();
                    let connected = match client {
                        Ok(client) => {
                            match client.head(check_url).send().await {
                                Ok(resp) => resp.status().is_success(),
                                Err(_) => {
                                    // HEAD 失败回退 GET
                                    match client.get(check_url).send().await {
                                        Ok(resp) => resp.status().is_success(),
                                        Err(_) => false,
                                    }
                                }
                            }
                        }
                        Err(_) => false,
                    };

                    let value = if connected { "true" } else { "false" };
                    let _ = sqlx::query(
                        "INSERT OR REPLACE INTO settings (key, value, category, updated_at) VALUES ('internet_connected', ?, 'general', datetime('now'))"
                    )
                    .bind(value)
                    .execute(&*db_for_connectivity.pool().await)
                    .await;

                    log::info!("[Connectivity] 网络状态: {}", if connected { "在线" } else { "离线" });
                });

            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::fastplan_connection::get_fastplan_connection,
            commands::fastplan_connection::provide_fastplan_resume_data,
            commands::fastplan_connection::get_fastplan_resume_data,
            commands::fastplan_connection::get_fastplan_model_config,
            // ── 对话 ──
            commands::chat::send_message_stream,
            commands::chat::get_conversations,
            commands::chat::get_user_message_count,
            commands::chat::get_messages,
            commands::chat::get_messages_page,
            commands::chat::create_conversation,
            commands::chat::update_conversation,
            commands::chat::update_conversation_extra_config,
            commands::chat::delete_conversation,
            commands::chat::delete_message,
            commands::chat::delete_message_turn,
            commands::chat::like_message,
            commands::chat::favorite_message,
            commands::chat::get_favorite_messages,
            commands::chat::clear_conversation_messages,
            commands::chat::branch_conversation,
            commands::chat::submit_quality_evaluation,
            commands::chat::get_quality_evaluations,
            commands::chat::stop_stream,
            commands::chat::save_user_message,
            commands::chat::save_assistant_message,
            commands::chat::update_message_content,
            commands::chat::translate_message,
            commands::chat::generate_topic_title,
            commands::chat::open_external,
            commands::chat::clear_conversation_context,
            commands::chat::get_recent_models,
            // ── File2Md OCR ──
            commands::file2md::file2md_ocr,
            commands::file2md::file2md_ocr_by_path,
            commands::file2md::file2md_cleanup_temp,
            commands::file2md::check_file2md_exists,
            // ── 提示词模板 ──
            commands::prompts::list_prompts,
            commands::prompts::list_my_prompts,
            commands::prompts::create_prompt,
            commands::prompts::update_prompt,
            commands::prompts::delete_prompt,
            commands::prompts::download_prompt,
            commands::prompts::apply_prompt,
            commands::screenshot::capture_fullscreen,
            commands::screenshot::capture_raw,
            commands::screenshot::capture_fullscreen_with_windows,
            commands::screenshot::recapture_with_window_hidden,
            commands::screenshot::enumerate_window_list,
            commands::screenshot::capture_hide_foreground,
            commands::screenshot::capture_start,
            commands::screenshot::capture_get_all_monitor_pixels,
            commands::screenshot::capture_take_pending,
            commands::screenshot::capture_discard_pending,

            commands::models::providers::list_providers,
            commands::models::providers::list_provider_summaries,
            commands::models::providers::get_provider_runtime_config,
            commands::models::providers::add_provider,
            commands::models::providers::update_provider,
            commands::models::providers::delete_provider,
            commands::models::gateways::list_gateways,
            commands::models::gateways::add_gateway,
            commands::models::gateways::update_gateway,
            commands::models::gateways::delete_gateway,
            commands::models::gateways::list_gateway_providers,
            commands::models::gateways::list_gateways_with_providers,
            commands::models::catalog::list_models,
            commands::models::list_models_tree,
            commands::models::list_all_providers_with_models,
            commands::models::providers::test_connection,
            commands::models::catalog::fetch_remote_models,
            commands::models::catalog::save_remote_models,
            commands::models::catalog::update_models,
            commands::models::catalog::delete_models,
            commands::models::upload_local_model,
            commands::models::list_local_models,
            commands::models::mark_model_downloaded,
            commands::models::sync_wsp_market,
            commands::models::list_wsp_market,
            commands::models::import_wsp_models,
            commands::models::prepare_wsp_download,
            commands::models::complete_wsp_download,
            commands::models::delete_downloaded_model,
            commands::models::sync::sync_preview,
            commands::models::sync::sync_execute,
            commands::models::sync::sync_independent_models,
            commands::models::catalog::list_models_for_export,
            commands::models::import_export::import_models_json,
            commands::models::import_export::export_models,
            commands::models::ratings::get_model_ratings,
            commands::models::ratings::get_model_ranking,
            commands::assistants::list_assistants,
            commands::assistants::create_assistant,
            commands::assistants::update_assistant,
            commands::assistants::delete_assistant,
            commands::assistants::get_roles,
            commands::assistants::get_role,
            commands::assistants::create_role,
            commands::assistants::update_role,
            commands::assistants::delete_role,
            commands::assistants::activate_role,
            commands::assistants::deactivate_role,
            commands::assistants::get_active_role,
            commands::assistants::get_role_stats,
            commands::assistants::publish_role,
            commands::assistants::unpublish_role,
            commands::assistants::clone_role,
            commands::assistants::list_square_roles,
            commands::assistants::apply_square_role,
            commands::assistants::star_square_role,
            commands::agents::list_agents,
            commands::agents::get_agent,
            commands::agents::create_agent,
            commands::agents::update_agent,
            commands::agents::delete_agent,
            commands::agents::import_agents,
            commands::agents::save_agent_icon,
            commands::agents::get_agent_icon,
            commands::agents::check_agent_name,
            // Phase 2 Agent Skill CRUD（替代旧 ES 命令）
            commands::skills::list_local_skills,
            commands::skills::get_local_skill,
            commands::skills::create_local_skill,
            commands::skills::update_local_skill,
            commands::skills::delete_local_skill,
            commands::skills::toggle_local_skill,
            commands::skills::import_skills_from_folder,
            commands::skills::import_skills_from_github,
            commands::skills::get_skill_template,
            commands::skills::expand_skill,
            commands::mcp::list_mcp_servers,
            commands::mcp::add_mcp_server,
            commands::mcp::remove_mcp_server,
            commands::mcp::connect_mcp_server,
            commands::mcp::disconnect_mcp_server,
            commands::mcp::list_mcp_tools,
            commands::mcp::call_mcp_tool,
            commands::mcp::update_mcp_server,
            commands::mcp::refresh_mcp_tools,
            commands::mcp::test_mcp_tool,
            // 本地工具库（替代 FastPlan /api/admin/tools）
            commands::tools::list_local_tools,
            commands::tools::get_local_tool,
            commands::tools::create_local_tool,
            commands::tools::update_local_tool,
            commands::tools::delete_local_tool,
            commands::tools::toggle_local_tool,
            commands::session_plan::set_remote_ui_model,
            commands::session_plan::set_conversation_task_plan,
            commands::session_plan::set_conversation_execution_mode,
            commands::session_plan::send_conversation_plan_control,
            commands::session_plan::sync_desktop_plan_user_message,
            commands::session_plan::sync_desktop_plan_assistant_message,
            commands::wsp::list_wsp_services,
            commands::wsp::get_wsp_status,
            commands::wsp::start_wsp_service,
            commands::wsp::stop_wsp_service,
            commands::wsp::restart_wsp_service,
            commands::wsp::start_all_wsp_services,
            commands::wsp::start_all_auto,
            commands::wsp::stop_all_wsp_services,
            commands::wsp::start_knowledge_service,
            commands::wsp::stop_knowledge_service,
            commands::wsp::stop_kg_for_update,
            commands::wsp::check_kg_service_files,
            commands::wsp::start_knowledge_service_async,
            commands::wsp::check_kg_service_running,
            commands::wsp::check_kg_service_ready,
            commands::wsp::get_kg_shared_status,
            commands::wsp::validate_kg_shared_runtime,
            commands::wsp::ensure_kg_shared_lease,
            commands::wsp::prepare_app_download,
            commands::wsp::complete_app_download,
            commands::wsp::check_fastkg_version,
            commands::wsp::upload_fastkg_service,
            commands::wsp::get_internet_status,
            commands::wsp::get_wsp_logs,
            commands::wsp::call_wsp_endpoint,
            commands::wsp::register_wsp_service,
            commands::wsp::update_wsp_service,
            commands::wsp::edit_wsp_service,
            commands::wsp::remove_wsp_service,
            commands::wsp::get_dependency_services_status,
            commands::wsp::start_dependency_service,
            core::dependency_services::manager::get_dependency_recovery_status,
            core::dependency_services::manager::get_dependency_recovery_history,
            core::dependency_services::manager::get_dependency_diagnostics,
            core::dependency_services::manager::retry_dependency_service,
            core::dependency_services::manager::pause_dependency_recovery,
            core::dependency_services::manager::set_dependency_recovery_settings,
            commands::wsp::stop_dependency_service,
            commands::wsp::stop_all_dependency_services,
            commands::wsp::update_dependency_service_config,
            commands::cloud_providers::list_cloud_providers,
            commands::cloud_providers::save_cloud_provider,
            commands::cloud_providers::cloud_ocr_recognize,
            commands::cost::get_usage_stats,
            commands::cost::get_budgets,
            commands::cost::set_budget,
            commands::settings::get_settings,
            commands::settings::update_setting,
            commands::settings::update_setting_with_category,
            commands::settings::web_search_test,
            commands::settings::update_setting_with_description,
            // ── 应用配置（config.json）──
            commands::config::get_app_config,
            commands::config::update_app_config,
            commands::config::update_app_config_field,
            commands::config::reset_service_url,
            commands::config::reset_remote_server_url,
            commands::system::get_system_info,
            commands::system::browse_filesystem,
            commands::local_knowledge::inspect_drop_paths,
            commands::local_knowledge::list_local_source_files,
            commands::local_knowledge::open_local_source_folder,
            commands::system::export_data,
            commands::system::import_data,
            commands::system::delete_file,
            commands::system::copy_file,
            commands::system::reset_data,
            commands::system::clear_cache,
            commands::export::export_pdf,
            commands::export::export_docx,
            commands::system::open_directory,
            commands::system::get_app_root_dir,
            commands::system::get_exe_dir,
            commands::system::get_db_dir,
            commands::system::backup_directory,
            commands::system::select_backup_file,
            commands::system::restore_from_backup,
            commands::system::get_backup_file_info,
            commands::system::check_disk_space,
            commands::system::restart_app,
            commands::system::proxy_http_request,
            commands::system::get_http_port,
            commands::system::check_internet_connectivity,
            commands::system_tray::get_system_tray_enabled,
            commands::system_tray::set_system_tray_enabled,
            commands::system_tray::exit_app,
            commands::system_tray::save_main_window_state,
            commands::system_tray::hide_to_tray,
            commands::system_tray::show_main_window,
            commands::system_tray::maximize_window,
            commands::system_tray::restore_window,
            commands::system_tray::drag_window_if_held,
            // ── 网页抓取（搜索结果外链预览：iframe 嵌入判定 + 正文抓取）──
            fetch_web_page,
        ])
        .run(tauri::generate_context!())
        .expect("FastClaw 启动失败");
}

// =================================================================================
// ★ 搜索结果点击外链时由前端调用的抓取命令：桌面 UA + 8s 超时 + 跟随重定向。
// 返回结构化结果，由前端按 `embeddable` 分流进 iframe（直嵌）或阅读器（srcdoc）。
// =================================================================================
#[tauri::command]
async fn fetch_web_page(url: String) -> Result<serde_json::Value, String> {
    use futures::stream::StreamExt;

    let url = url.trim().to_string();
    let lower = url.to_ascii_lowercase();
    // 只放行 http/https，避免 file:// 等本地协议或内网探测（弱防护）
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err("仅支持 http/https 链接".into());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                     (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| format!("构建请求客户端失败：{e}"))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("抓取失败：{e}"))?;

    let final_url = resp.url().to_string();
    let status = resp.status().as_u16();
    let headers: std::collections::HashMap<String, String> = resp
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_lowercase(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let content_type = headers.get("content-type").cloned().unwrap_or_default();
    let is_html = content_type.is_empty()
        || content_type.contains("text/html")
        || content_type.contains("application/xhtml+xml");

    // 流式读取正文并限流 8MB，避免超大页面把内存打爆
    const MAX_HTML: usize = 8 * 1024 * 1024;
    let mut body: Vec<u8> = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(b) => {
                body.extend_from_slice(&b);
                if body.len() >= MAX_HTML {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    let html = if is_html {
        String::from_utf8_lossy(&body).into_owned()
    } else {
        // 非 HTML：返回空正文，前端直接提示走系统浏览器
        String::new()
    };

    let embeddable = if !is_html {
        false
    } else {
        is_embeddable(&headers)
    };

    Ok(serde_json::json!({
        "status": status,
        "headers": headers,
        "html": html,
        "final_url": final_url,
        "embeddable": embeddable,
    }))
}

/// 依据 X-Frame-Options 与 CSP frame-ancestors 判断目标页能否被 iframe 嵌入。
/// 缺这两个头则视为可嵌入（保守放行，靠前端 `<iframe sandbox>` 兜底安全）。
fn is_embeddable(headers: &std::collections::HashMap<String, String>) -> bool {
    let mut ok = true;
    if let Some(xfo) = headers.get("x-frame-options") {
        let v = xfo.trim().to_ascii_uppercase();
        if v == "DENY" || v == "SAMEORIGIN" || v.starts_with("ALLOW-FROM") {
            ok = false;
        }
    }
    for key in ["content-security-policy", "content-security-policy-report-only"] {
        if let Some(csp) = headers.get(key) {
            if csp.to_ascii_lowercase().contains("frame-ancestors") {
                ok = false;
            }
        }
    }
    ok
}

// =================================================================================
// ★ 辅助：去掉 base_url 末尾的 /v1
// =================================================================================
fn strip_trailing_v1(url: &str) -> String {
    url.trim_end_matches('/')
        .trim_end_matches("/v1")
        .trim_end_matches('/')
        .to_string()
}

// =================================================================================
// ★ 从 fastclaw DB 读取 LLM/Embedding 配置，注入环境变量
// =================================================================================
async fn inject_provider_env_vars(pool: &sqlx::SqlitePool) {
    // ─── 1. 读取第一个已启用的 LLM provider ───────────────────────
    let provider = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT base_url, provider_type, api_key_ref \
         FROM model_providers \
         WHERE is_enabled = 1 \
         ORDER BY sort_order LIMIT 1",
    )
    .fetch_optional(pool)
    .await;

    if let Ok(Some((base_url, provider_type, api_key))) = provider {
        log::info!(
            "[配置注入] 找到 LLM provider: type={}, base_url={}",
            provider_type,
            base_url
        );

        if let Some(key) = &api_key {
            if !key.is_empty() {
                std::env::set_var("FASTCLAW_LLM_API_KEY", key);
                std::env::set_var("OPENAI_API_KEY", key);
                log::info!(
                    "[配置注入] API Key 已注入 ({}...)",
                    &key[..key.len().min(8)]
                );
            }
        }

        if !base_url.is_empty() {
            let cleaned = strip_trailing_v1(&base_url);
            std::env::set_var("FASTCLAW_LLM_BASE_URL", &cleaned);
            log::info!("[配置注入] LLM Base URL: {} (原始: {})", cleaned, base_url);
        }
    } else {
        log::warn!("[配置注入] 未找到已启用的 LLM provider，fastplan 将使用默认配置");
    }

    // ─── 2. 读取默认模型 ──────────────────────────────────────────
    let default_model =
        sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'default_model'")
            .fetch_optional(pool)
            .await;

    let mut model_resolved = false;

    if let Ok(Some(model_str)) = default_model {
        let model = model_str.trim_matches('"').to_string();
        if model != "null" && !model.is_empty() {
            let real_model = sqlx::query_scalar::<_, String>(
                "SELECT model_id FROM models WHERE id = ? OR model_id = ? LIMIT 1",
            )
            .bind(&model)
            .bind(&model)
            .fetch_optional(pool)
            .await;

            if let Ok(Some(m)) = real_model {
                std::env::set_var("FASTCLAW_LLM_MODEL", &m);
                log::info!("[配置注入] 默认模型: {}", m);
                model_resolved = true;
            } else {
                std::env::set_var("FASTCLAW_LLM_MODEL", &model);
                log::info!("[配置注入] 默认模型 (直接使用): {}", model);
                model_resolved = true;
            }
        }
    }

    if !model_resolved {
        let fallback = sqlx::query_scalar::<_, String>(
            "SELECT model_id FROM models WHERE is_default = 1 AND is_enabled = 1 ORDER BY sort_order ASC, id ASC LIMIT 1",
        )
        .fetch_optional(pool)
        .await;

        if let Ok(Some(m)) = fallback {
            std::env::set_var("FASTCLAW_LLM_MODEL", &m);
            log::info!("[配置注入] 默认模型 (fallback从models表): {}", m);
        } else {
            let any_model = sqlx::query_scalar::<_, String>(
                "SELECT model_id FROM models WHERE is_enabled = 1 ORDER BY sort_order ASC, id ASC LIMIT 1",
            )
            .fetch_optional(pool)
            .await;

            if let Ok(Some(m)) = any_model {
                std::env::set_var("FASTCLAW_LLM_MODEL", &m);
                log::info!("[配置注入] 默认模型 (兜底): {}", m);
            }
        }
    }

    // ─── 3. 读取 Embedding 配置 ──────────────────────────────────
    let embedding_model =
        sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'embedding_model'")
            .fetch_optional(pool)
            .await;

    if let Ok(Some(emb)) = embedding_model {
        let emb = emb.trim_matches('"').to_string();
        if !emb.is_empty() && emb != "null" {
            std::env::set_var("FASTCLAW_EMBEDDING_MODEL", &emb);
            log::info!("[配置注入] Embedding 模型: {}", emb);
        }
    }

    let embedding_dim = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = 'embedding_dimension'",
    )
    .fetch_optional(pool)
    .await;

    if let Ok(Some(dim_str)) = embedding_dim {
        let dim = dim_str.trim_matches('"').to_string();
        if !dim.is_empty() && dim != "null" {
            std::env::set_var("FASTCLAW_EMBEDDING_DIM", &dim);
            log::info!("[配置注入] Embedding 维度: {}", dim);
        }
    }
    if std::env::var("FASTCLAW_EMBEDDING_DIM").is_err() {
        std::env::set_var("FASTCLAW_EMBEDDING_DIM", "1024");
        log::info!("[配置注入] Embedding 维度 (默认): 1024");
    }

    // ─── 4. 查找 Embedding 类型的 provider ──────────────────────
    let emb_provider = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT base_url, api_key_ref \
         FROM model_providers \
         WHERE provider_type = 'embedding' AND is_enabled = 1 \
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await;

    if let Ok(Some((emb_url, emb_key))) = emb_provider {
        let emb_url_clean = strip_trailing_v1(&emb_url);
        let full_emb_url = format!("{}/v1/embeddings", emb_url_clean);
        std::env::set_var("FASTCLAW_EMBEDDING_URL", &full_emb_url);
        std::env::set_var("FASTKG_EMBEDDING_URL", &full_emb_url);
        log::info!("[配置注入] Embedding URL: {}", full_emb_url);

        if let Some(key) = emb_key {
            if !key.is_empty() {
                std::env::set_var("FASTCLAW_EMBEDDING_API_KEY", &key);
            }
        }
    } else {
        if let Ok(base) = std::env::var("FASTCLAW_LLM_BASE_URL") {
            let emb_url = format!("{}/v1/embeddings", base);
            std::env::set_var("FASTCLAW_EMBEDDING_URL", &emb_url);
            std::env::set_var("FASTKG_EMBEDDING_URL", &emb_url);
            log::info!("[配置注入] Embedding URL (从LLM推导): {}", emb_url);
        }
    }

    // ─── 5. 汇总日志 ────────────────────────────────────────────
    log::info!("[配置注入] === 环境变量汇总 ===");
    for key in &[
        "FASTCLAW_LLM_BASE_URL",
        "FASTCLAW_LLM_MODEL",
        "FASTCLAW_EMBEDDING_URL",
        "FASTCLAW_EMBEDDING_MODEL",
        "FASTCLAW_EMBEDDING_DIM",
    ] {
        match std::env::var(key) {
            Ok(v) => log::info!("[配置注入]   {} = {}", key, v),
            Err(_) => log::warn!("[配置注入]   {} = (未设置)", key),
        }
    }
    if let Ok(k) = std::env::var("FASTCLAW_LLM_API_KEY") {
        log::info!(
            "[配置注入]   FASTCLAW_LLM_API_KEY = {}...",
            &k[..k.len().min(8)]
        );
    }
}
