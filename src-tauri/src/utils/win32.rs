//! 统一的 Win32 user32/dwmapi FFI 声明。
//! 避免在多个模块重复 `extern "system"` 且签名不一致（clashing extern）。

#![cfg(windows)]

pub type Bool = i32;
pub type Hwnd = isize;
pub type Lparam = isize;

pub type WndEnumProc = unsafe extern "system" fn(Hwnd, Lparam) -> Bool;

pub const TRUE: Bool = 1;
pub const FALSE: Bool = 0;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[link(name = "user32")]
extern "system" {
    pub fn EnumWindows(lp_enum_func: Option<WndEnumProc>, l_param: Lparam) -> Bool;
    pub fn IsWindowVisible(hwnd: Hwnd) -> Bool;
    pub fn ShowWindow(hwnd: Hwnd, n_cmd_show: i32) -> Bool;
    pub fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
    pub fn IsIconic(hwnd: Hwnd) -> Bool;
    pub fn SetForegroundWindow(hwnd: Hwnd) -> Bool;
    pub fn GetWindowTextLengthW(hwnd: Hwnd) -> i32;
    pub fn AllowSetForegroundWindow(pid: u32) -> Bool;
    pub fn BringWindowToTop(hwnd: Hwnd) -> Bool;
    pub fn GetWindowTextW(hwnd: Hwnd, lp_string: *mut u16, n_max_count: i32) -> i32;
    pub fn GetWindowRect(hwnd: Hwnd, lp_rect: *mut Rect) -> Bool;
    pub fn GetForegroundWindow() -> Hwnd;
    pub fn GetWindowLongW(hwnd: Hwnd, n_index: i32) -> i32;
    pub fn SetWindowPos(
        hwnd: Hwnd,
        hwnd_insert_after: Hwnd,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> Bool;
}

#[link(name = "dwmapi")]
extern "system" {
    pub fn DwmGetWindowAttribute(
        hwnd: Hwnd,
        dw_attribute: u32,
        pv_attribute: *mut Rect,
        cb_attribute: u32,
    ) -> i32;
    pub fn DwmFlush() -> i32;
}
