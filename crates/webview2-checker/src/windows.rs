use crate::{is_version_sufficient, NetworkDetector, NetworkEnvironment, NetworkProbe};

/// 默认最低 WebView2 版本。
pub const DEFAULT_MIN_VERSION: &str = "86.0.622.0";

/// 微软官方 WebView2 Evergreen 下载地址。
pub const MS_DOWNLOAD_URL: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";

/// WebView2 版本检测配置构建器。
pub struct WebView2Checker {
    min_version: String,
    online_url: String,
    offline_url: Option<String>,
    network_detector: NetworkDetector,
}

impl Default for WebView2Checker {
    fn default() -> Self {
        Self {
            min_version: DEFAULT_MIN_VERSION.to_string(),
            online_url: MS_DOWNLOAD_URL.to_string(),
            offline_url: None,
            network_detector: NetworkDetector::default(),
        }
    }
}

impl WebView2Checker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn min_version(mut self, version: impl Into<String>) -> Self {
        self.min_version = version.into();
        self
    }

    pub fn online_url(mut self, url: impl Into<String>) -> Self {
        self.online_url = url.into();
        self
    }

    pub fn offline_url(mut self, url: impl Into<String>) -> Self {
        self.offline_url = Some(url.into());
        self
    }

    /// 增加一个公网连通性探针。
    ///
    /// 默认探针仍然保留；如需完全替换默认探针，请使用 [`Self::network_detector`]。
    pub fn internet_probe(mut self, probe: NetworkProbe) -> Self {
        self.network_detector = self.network_detector.internet_probe(probe);
        self
    }

    /// 使用调用方提供的网络判断器替换默认公网探针。
    pub fn network_detector(mut self, detector: NetworkDetector) -> Self {
        self.network_detector = detector;
        self
    }

    pub fn network_timeout_secs(mut self, seconds: u64) -> Self {
        self.network_detector = self.network_detector.timeout_secs(seconds);
        self
    }

    pub fn check(self) {
        match installed_version() {
            None => self.show_and_exit("缺少必要组件", "未检测到 Microsoft Edge WebView2 运行时。"),
            Some(version) if !is_version_sufficient(&version, &self.min_version) => self
                .show_and_exit(
                    "WebView2 版本过低",
                    &format!(
                        "检测到 Microsoft Edge WebView2 版本为 {version}，\n低于最低要求版本 {}。",
                        self.min_version
                    ),
                ),
            Some(_) => {}
        }
    }

    fn show_and_exit(&self, title: &str, reason: &str) -> ! {
        let url = self.resolve_url();
        let message = format!(
            "{reason}\n\n请下载安装或更新后重启程序。\n\n【提示】推荐以「管理员身份运行」方式安装。\n\n下载地址：\n{url}\n\n点击「确定」将打开下载页面并退出；\n点击「取消」直接退出。"
        );
        if show_ok_cancel_dialog(title, &message) {
            open_url(&url);
        }
        std::process::exit(1)
    }

    fn resolve_url(&self) -> String {
        let internet_available = self.network_detector.detect() == NetworkEnvironment::Internet;
        match &self.offline_url {
            Some(offline) if !internet_available => offline.clone(),
            _ => self.online_url.clone(),
        }
    }
}

/// 读取当前机器已安装的 WebView2 版本。
pub fn installed_version() -> Option<String> {
    use winreg::{enums::*, RegKey};

    const GUID: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    let paths = [
        format!("SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{GUID}"),
        format!("SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{GUID}"),
    ];

    for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        let root = RegKey::predef(hive);
        for path in &paths {
            if let Ok(key) = root.open_subkey(path) {
                if let Ok(version) = key.get_value::<String, _>("pv") {
                    if !version.is_empty() && version != "0.0.0.0" {
                        return Some(version);
                    }
                }
            }
        }
    }
    None
}

fn show_ok_cancel_dialog(title: &str, message: &str) -> bool {
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(
            hwnd: *mut core::ffi::c_void,
            text: *const u16,
            caption: *const u16,
            utype: u32,
        ) -> i32;
    }
    fn to_wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
    let caption = to_wide(title);
    let text = to_wide(message);
    let result = unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            0x00000031, // MB_OKCANCEL | MB_ICONWARNING
        )
    };
    result == 1
}

fn open_url(url: &str) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let _ = std::process::Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", url])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::WebView2Checker;
    use crate::{NetworkDetector, NetworkProbe};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn reachable_internet_probe_selects_online_url() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let accept = thread::spawn(move || listener.accept().unwrap());
        let checker = WebView2Checker::new()
            .online_url("https://example.com/online")
            .offline_url("http://intranet.example/offline")
            .network_detector(
                NetworkDetector::new().internet_probe(NetworkProbe::tcp(address.to_string())),
            );

        assert_eq!(checker.resolve_url(), "https://example.com/online");
        accept.join().unwrap();
    }

    #[test]
    fn unreachable_internet_probe_selects_offline_url() {
        let checker = WebView2Checker::new()
            .online_url("https://example.com/online")
            .offline_url("http://intranet.example/offline")
            .network_detector(
                NetworkDetector::new()
                    .internet_probe(NetworkProbe::tcp("127.0.0.1:0"))
                    .timeout_secs(0),
            );

        assert_eq!(checker.resolve_url(), "http://intranet.example/offline");
    }
}
