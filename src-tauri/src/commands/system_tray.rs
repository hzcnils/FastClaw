// FastClaw · 系统托盘 Tauri Commands
use crate::core::error::AppError;
use crate::state::AppState;
use tauri::Manager;
use tauri::State;

pub static TRAY_ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[tauri::command]
pub async fn get_system_tray_enabled(state: State<'_, AppState>) -> Result<bool, AppError> {
    let result =
        sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'system_tray'")
            .fetch_optional(&*state.db.pool().await)
            .await?;

    match result {
        Some(v) => Ok(v == "1"),
        None => Ok(false),
    }
}

#[tauri::command]
pub async fn set_system_tray_enabled(
    app_handle: tauri::AppHandle,
    enabled: bool,
) -> Result<(), AppError> {
    if enabled {
        enable_system_tray(&app_handle)?;
        TRAY_ENABLED.store(true, std::sync::atomic::Ordering::SeqCst);
    } else {
        disable_system_tray(&app_handle)?;
        TRAY_ENABLED.store(false, std::sync::atomic::Ordering::SeqCst);
    }
    Ok(())
}

pub fn setup_system_tray_with_handle(app_handle: &tauri::AppHandle) -> Result<(), AppError> {
    // 防止重复创建托盘
    if app_handle.tray_by_id("main_tray").is_some() {
        return Ok(());
    }

    let show_item =
        tauri::menu::MenuItem::with_id(app_handle, "tray_show", "打开窗口", true, None::<&str>)
            .map_err(|e| AppError::Internal(e.into()))?;

    let quit_item =
        tauri::menu::MenuItem::with_id(app_handle, "tray_quit", "退出系统", true, None::<&str>)
            .map_err(|e| AppError::Internal(e.into()))?;

    let menu = tauri::menu::Menu::with_items(app_handle, &[&show_item, &quit_item])
        .map_err(|e| AppError::Internal(e.into()))?;

    let tray_icon = match app_handle.default_window_icon().cloned() {
        Some(icon) => icon,
        None => {
            log::error!("[Tray] 应用图标缺失，无法创建托盘");
            return Err(AppError::Internal(
                anyhow::anyhow!("应用图标未配置，无法创建系统托盘").into(),
            ));
        }
    };

    let _ = tauri::tray::TrayIconBuilder::with_id("main_tray")
        .tooltip("FastClaw 智能助手")
        .icon(tray_icon)
        .menu(&menu)
        .on_tray_icon_event(|app, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                ..
            } = event
            {
                if let Some(window) = app.app_handle().get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "tray_show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "tray_quit" => {
                    // 退出前同步清理子进程（FastPlan + KG + MCP + WSP）
                    let kg_pids = app.state::<crate::state::AppState>().kg_pids.clone();
                    let wsp = app.state::<crate::state::AppState>().wsp.clone();
                    let mcp = app.state::<crate::state::AppState>().mcp.clone();
                    tauri::async_runtime::block_on(async {
                        let recovery = app.state::<crate::state::AppState>().dependency_recovery.clone();
                        recovery.shutdown();
                        let _recovery_operation = recovery.operation.lock().await;
                        let _ = crate::commands::wsp::stop_fastplan_inner(&kg_pids).await;
                        let _ = crate::commands::wsp::stop_kg_inner(&kg_pids).await;
                        crate::core::mcp::disconnect_all_shared(&mcp).await;
                        let _ = wsp.shutdown().await;
                    });
                    // 标记全局关闭已完成
                    crate::state::SHUTDOWN_DONE.store(true, std::sync::atomic::Ordering::SeqCst);
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app_handle);

    setup_window_close_handler(app_handle)?;
    TRAY_ENABLED.store(true, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

fn setup_window_close_handler(app_handle: &tauri::AppHandle) -> Result<(), AppError> {
    if let Some(window) = app_handle.get_webview_window("main") {
        let window_clone = window.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if TRAY_ENABLED.load(std::sync::atomic::Ordering::SeqCst) {
                    api.prevent_close();
                    // 启动阶段（界面数据尚未加载完成）不隐藏窗口，仅阻止关闭
                    if crate::state::STARTUP_COMPLETE.load(std::sync::atomic::Ordering::SeqCst) {
                        let _ = window_clone.hide();
                    } else {
                        log::warn!("[Tray] 启动阶段收到 CloseRequested，已阻止关闭但不隐藏窗口");
                    }
                }
            }
        });
    }
    Ok(())
}

pub fn enable_system_tray(app_handle: &tauri::AppHandle) -> Result<(), AppError> {
    setup_system_tray_with_handle(app_handle)
}

pub fn disable_system_tray(_app_handle: &tauri::AppHandle) -> Result<(), AppError> {
    if _app_handle.tray_by_id("main_tray").is_some() {
        let _ = _app_handle.remove_tray_by_id("main_tray");
    }
    TRAY_ENABLED.store(false, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn save_main_window_state(app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_window_state::{AppHandleExt, StateFlags};
    // 必须在 hide 之前调用：隐藏后尺寸可能变成 0，污染存档
    if let Some(w) = app_handle.get_webview_window("main") {
        if !w.is_visible().unwrap_or(true) {
            return Err("窗口已隐藏，无法保存有效尺寸".into());
        }
    }
    let flags = StateFlags::SIZE | StateFlags::MAXIMIZED | StateFlags::FULLSCREEN;
    app_handle
        .save_window_state(flags)
        .map_err(|e| e.to_string())?;
    log::info!("[Window] 窗口状态已保存 (save_main_window_state)");
    Ok(())
}

#[tauri::command]
pub async fn exit_app(app_handle: tauri::AppHandle) {
    let recovery = app_handle.state::<crate::state::AppState>().dependency_recovery.clone();
    recovery.shutdown();
    let _recovery_operation = recovery.operation.lock().await;
    let kg_pids = app_handle.state::<crate::state::AppState>().kg_pids.clone();
    let wsp = app_handle.state::<crate::state::AppState>().wsp.clone();
    let mcp = app_handle.state::<crate::state::AppState>().mcp.clone();
    let _ = crate::commands::wsp::stop_fastplan_inner(&kg_pids).await;
    let _ = crate::commands::wsp::stop_kg_inner(&kg_pids).await;
    crate::core::mcp::disconnect_all_shared(&mcp).await;
    let _ = wsp.shutdown().await;
    // 标记全局关闭已完成，避免 CloseRequested handler 重复清理
    crate::state::SHUTDOWN_DONE.store(true, std::sync::atomic::Ordering::SeqCst);
    // 仅在窗口仍可见时落盘；hide 之后保存会把缓存写成 0 尺寸覆盖正确存档
    if let Some(w) = app_handle.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) {
            use tauri_plugin_window_state::{AppHandleExt, StateFlags};
            let flags = StateFlags::SIZE | StateFlags::MAXIMIZED | StateFlags::FULLSCREEN;
            if let Err(e) = app_handle.save_window_state(flags) {
                log::warn!("[Window] exit_app 保存窗口状态失败: {e}");
            } else {
                log::info!("[Window] exit_app 窗口状态已保存");
            }
        } else {
            log::info!("[Window] exit_app 跳过保存（窗口已隐藏，依赖关闭前已落盘）");
        }
    }
    app_handle.exit(0);
}

#[tauri::command]
pub fn hide_to_tray(app_handle: tauri::AppHandle) -> Result<(), AppError> {
    if let Some(window) = app_handle.get_webview_window("main") {
        // 与 show_main_window 对称：无边框主窗口上优先 Win32 SW_HIDE，避免仅 window.hide() 无效。
        #[cfg(windows)]
        {
            const SW_HIDE: i32 = 0;
            if let Ok(hwnd) = window.hwnd() {
                unsafe { crate::utils::win32::ShowWindow(hwnd.0 as isize, SW_HIDE) };
            } else {
                let _ = window.hide();
            }
        }
        #[cfg(not(windows))]
        {
            let _ = window.hide();
        }
    }
    Ok(())
}

#[tauri::command]
pub fn show_main_window(app_handle: tauri::AppHandle) -> Result<(), AppError> {
    // 优先走截图模块的还原：若「隐藏窗口截图」把窗口挪出屏幕，需先还原位置再显示。
    crate::commands::screenshot::restore_main_after_hide_capture(&app_handle);
    Ok(())
}

#[tauri::command]
pub fn maximize_window(app_handle: tauri::AppHandle) -> Result<(), AppError> {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.maximize();
    }
    Ok(())
}

#[tauri::command]
pub fn restore_window(app_handle: tauri::AppHandle) -> Result<(), AppError> {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.unmaximize();
    }
    Ok(())
}

/// 标题栏拖拽（带物理左键守卫）
///
/// 背景：tao 的 start_dragging 在 Windows 上是 PostMessageW(WM_NCLBUTTONDOWN, HTCAPTION)
/// 进入 Win32 模态移动循环，该循环只认"之后到来"的 WM_LBUTTONUP 作为退出信号。
/// 快速点击（按下即松）时，真实 mouse-up 先于合成的 NC-down 派发（硬件输入优先于
/// posted 消息），移动循环启动后再也等不到退出信号 → 拖拽锁死，直到用户再点一次。
///
/// 修复：进入拖拽前用 GetAsyncKeyState(VK_LBUTTON) 检查物理左键是否仍按着，
/// check + act 在同一次主线程调用内完成，残余竞态窗口仅微秒级。
/// 返回是否实际触发了拖拽（false = 左键已松开，按点击处理）。
#[tauri::command]
pub fn drag_window_if_held(window: tauri::WebviewWindow) -> bool {
    #[cfg(windows)]
    {
        // VK_LBUTTON = 0x01；返回值最高位为 1 表示按键当前处于按下状态
        unsafe extern "system" {
            fn GetAsyncKeyState(vkey: i32) -> i16;
        }
        let held = (unsafe { GetAsyncKeyState(0x01) } as u16) & 0x8000 != 0;
        if held {
            // 失败无恢复动作，与前端旧实现的静默吞错保持一致
            let _ = window.start_dragging();
        }
        held
    }
    #[cfg(not(windows))]
    {
        let _ = window.start_dragging();
        true
    }
}
