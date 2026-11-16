use crate::{is_version_sufficient, NetworkDetector, NetworkEnvironment, NetworkProbe};
use std::ffi::{CStr, CString};
use std::fmt;
use std::os::raw::{c_char, c_int, c_void};
use std::process::Command;

/// Tauri v2 常用的 WebKitGTK 最低版本基线。
pub const DEFAULT_WEBKIT_MIN_VERSION: &str = "2.40.0";

/// WebKitGTK 4.1 的稳定 SONAME。
pub const WEBKITGTK_41_SONAME: &str = "libwebkit2gtk-4.1.so.0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebKitRuntimeInfo {
    pub version: String,
    pub soname: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WebKitDetectionError {
    NotLoadable { attempts: Vec<String> },
    MissingSymbol { soname: String, symbol: String },
}

impl fmt::Display for WebKitDetectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotLoadable { attempts } => {
                write!(formatter, "无法加载 WebKitGTK 4.1：{}", attempts.join("；"))
            }
            Self::MissingSymbol { soname, symbol } => {
                write!(formatter, "{soname} 缺少版本函数 {symbol}")
            }
        }
    }
}

impl std::error::Error for WebKitDetectionError {}

/// 动态加载 WebKitGTK，不要求检测程序在链接期依赖 WebKitGTK。
pub fn installed_webkit() -> Result<WebKitRuntimeInfo, WebKitDetectionError> {
    installed_webkit_from(&[WEBKITGTK_41_SONAME.to_string()])
}

/// Linux WebKitGTK 检测器。
///
/// 内网链接只在内网探针命中时使用，不会回退到公网链接。用户需要从链接手动下载、
/// 安装，然后回到提示框重新检测。
pub struct WebKitChecker {
    min_version: String,
    online_url: Option<String>,
    intranet_url: Option<String>,
    offline_url: Option<String>,
    detector: NetworkDetector,
    sonames: Vec<String>,
}

impl Default for WebKitChecker {
    fn default() -> Self {
        Self {
            min_version: DEFAULT_WEBKIT_MIN_VERSION.to_string(),
            online_url: None,
            intranet_url: None,
            offline_url: None,
            detector: NetworkDetector::default(),
            sonames: vec![WEBKITGTK_41_SONAME.to_string()],
        }
    }
}

impl WebKitChecker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn min_version(mut self, version: impl Into<String>) -> Self {
        self.min_version = version.into();
        self
    }

    /// 配置公网环境下打开的下载或帮助页面。
    pub fn online_url(mut self, url: impl Into<String>) -> Self {
        self.online_url = Some(url.into());
        self
    }

    /// 配置内网环境下打开的下载页面。内网命中时不会使用 `online_url`。
    pub fn intranet_url(mut self, url: impl Into<String>) -> Self {
        self.intranet_url = Some(url.into());
        self
    }

    /// 配置所有探针均失败时打开的页面，例如本机 `file://` 帮助文档。
    pub fn offline_url(mut self, url: impl Into<String>) -> Self {
        self.offline_url = Some(url.into());
        self
    }

    pub fn network_detector(mut self, detector: NetworkDetector) -> Self {
        self.detector = detector;
        self
    }

    pub fn intranet_probe(mut self, probe: NetworkProbe) -> Self {
        self.detector = self.detector.intranet_probe(probe);
        self
    }

    pub fn internet_probe(mut self, probe: NetworkProbe) -> Self {
        self.detector = self.detector.internet_probe(probe);
        self
    }

    pub fn network_timeout_secs(mut self, seconds: u64) -> Self {
        self.detector = self.detector.timeout_secs(seconds);
        self
    }

    /// 追加兼容 SONAME。默认只接受 WebKitGTK 4.1，不会把 4.0/6.0 当成兼容版本。
    pub fn compatible_soname(mut self, soname: impl Into<String>) -> Self {
        self.sonames.push(soname.into());
        self
    }

    /// 检测失败时引导用户到配置链接手动安装，并允许安装后重新检测。
    pub fn check(self) {
        loop {
            let problem = match installed_webkit_from(&self.sonames) {
                Ok(info) if is_version_sufficient(&info.version, &self.min_version) => return,
                Ok(info) => format!(
                    "检测到 WebKitGTK 版本为 {}，低于最低要求版本 {}。",
                    info.version, self.min_version
                ),
                Err(error) => error.to_string(),
            };

            let environment = self.detector.detect();
            let url = self.url_for(environment);
            let environment_name = match environment {
                NetworkEnvironment::Intranet => "内网",
                NetworkEnvironment::Internet => "公网",
                NetworkEnvironment::Offline => "离线",
            };

            let Some(url) = url else {
                show_information(
                    "缺少 WebKitGTK",
                    &format!(
                        "{problem}\n\n当前网络环境：{environment_name}\n没有为该环境配置安装链接，请联系管理员。"
                    ),
                );
                std::process::exit(1);
            };

            let message = format!(
                "{problem}\n\n当前网络环境：{environment_name}\n最低要求：WebKitGTK {}\n\n请打开以下页面，手动下载并安装适合当前 Linux 发行版和架构的软件包：\n{url}",
                self.min_version
            );
            if !ask_question("WebKitGTK 环境检查", &message, "打开下载页", "退出") {
                std::process::exit(1);
            }

            if !open_url(url) {
                show_information(
                    "无法打开下载页",
                    &format!("无法调用系统浏览器，请手动访问：\n{url}"),
                );
            }

            if !ask_question(
                "等待安装 WebKitGTK",
                "请在手动安装完成后点击「重新检测」。",
                "重新检测",
                "退出",
            ) {
                std::process::exit(1);
            }
        }
    }

    fn url_for(&self, environment: NetworkEnvironment) -> Option<&str> {
        match environment {
            NetworkEnvironment::Intranet => self.intranet_url.as_deref(),
            NetworkEnvironment::Internet => self.online_url.as_deref(),
            NetworkEnvironment::Offline => self.offline_url.as_deref(),
        }
    }
}

fn installed_webkit_from(sonames: &[String]) -> Result<WebKitRuntimeInfo, WebKitDetectionError> {
    let mut attempts = Vec::new();
    for soname in sonames {
        let library = match DynamicLibrary::open(soname) {
            Ok(library) => library,
            Err(error) => {
                attempts.push(format!("{soname}: {error}"));
                continue;
            }
        };

        let major = library.version_function(soname, "webkit_get_major_version")?;
        let minor = library.version_function(soname, "webkit_get_minor_version")?;
        let micro = library.version_function(soname, "webkit_get_micro_version")?;
        let version = unsafe { format!("{}.{}.{}", major(), minor(), micro()) };
        return Ok(WebKitRuntimeInfo {
            version,
            soname: soname.clone(),
        });
    }
    Err(WebKitDetectionError::NotLoadable { attempts })
}

struct DynamicLibrary(*mut c_void);

impl DynamicLibrary {
    fn open(soname: &str) -> Result<Self, String> {
        let name = CString::new(soname).map_err(|_| "SONAME 包含 NUL 字节".to_string())?;
        unsafe {
            dlerror();
            let handle = dlopen(name.as_ptr(), RTLD_NOW | RTLD_LOCAL);
            if handle.is_null() {
                Err(last_dl_error())
            } else {
                Ok(Self(handle))
            }
        }
    }

    fn version_function(
        &self,
        soname: &str,
        symbol: &str,
    ) -> Result<unsafe extern "C" fn() -> u32, WebKitDetectionError> {
        let symbol_name = CString::new(symbol).expect("static symbol has no NUL");
        unsafe {
            dlerror();
            let pointer = dlsym(self.0, symbol_name.as_ptr());
            if pointer.is_null() {
                return Err(WebKitDetectionError::MissingSymbol {
                    soname: soname.to_string(),
                    symbol: symbol.to_string(),
                });
            }
            Ok(std::mem::transmute::<
                *mut c_void,
                unsafe extern "C" fn() -> u32,
            >(pointer))
        }
    }
}

impl Drop for DynamicLibrary {
    fn drop(&mut self) {
        unsafe {
            dlclose(self.0);
        }
    }
}

const RTLD_LOCAL: c_int = 0;
const RTLD_NOW: c_int = 2;

#[link(name = "dl")]
extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
    fn dlerror() -> *const c_char;
}

unsafe fn last_dl_error() -> String {
    let error = dlerror();
    if error.is_null() {
        "未知动态加载错误".to_string()
    } else {
        CStr::from_ptr(error).to_string_lossy().into_owned()
    }
}

fn ask_question(title: &str, message: &str, accept: &str, cancel: &str) -> bool {
    let zenity = Command::new("zenity")
        .args([
            "--question",
            "--title",
            title,
            "--text",
            message,
            "--ok-label",
            accept,
            "--cancel-label",
            cancel,
        ])
        .status();
    if let Ok(status) = zenity {
        return status.success();
    }

    let kdialog = Command::new("kdialog")
        .args([
            "--title",
            title,
            "--yes-label",
            accept,
            "--no-label",
            cancel,
            "--yesno",
            message,
        ])
        .status();
    if let Ok(status) = kdialog {
        return status.success();
    }

    let accept_button = format!("{accept}:0");
    let cancel_button = format!("{cancel}:1");
    let yad = Command::new("yad")
        .args([
            "--question",
            "--title",
            title,
            "--text",
            message,
            "--button",
            &accept_button,
            "--button",
            &cancel_button,
        ])
        .status();
    if let Ok(status) = yad {
        return status.success();
    }

    eprintln!("{title}\n\n{message}\n");
    false
}

fn show_information(title: &str, message: &str) {
    for (program, args) in [
        (
            "zenity",
            vec!["--info", "--title", title, "--text", message],
        ),
        ("kdialog", vec!["--title", title, "--msgbox", message]),
        ("yad", vec!["--info", "--title", title, "--text", message]),
    ] {
        match Command::new(program).args(args).status() {
            Ok(_) => return,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => continue,
        }
    }
    eprintln!("{title}\n\n{message}");
}

fn open_url(url: &str) -> bool {
    if !url.starts_with("http://") && !url.starts_with("https://") && !url.starts_with("file://") {
        return false;
    }
    for (program, args) in [("xdg-open", vec![url]), ("gio", vec!["open", url])] {
        match Command::new(program).args(args).spawn() {
            Ok(_) => return true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => continue,
        }
    }
    false
}
