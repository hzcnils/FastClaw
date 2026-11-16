//! Windows WebView2 与 Linux WebKitGTK 运行时检测。

#[cfg(not(any(windows, target_os = "linux")))]
compile_error!("webview2-checker only supports Windows and Linux");

mod network;

pub use network::{is_online, NetworkDetector, NetworkEnvironment, NetworkProbe, ProbePolicy};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{installed_version, WebView2Checker, DEFAULT_MIN_VERSION, MS_DOWNLOAD_URL};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{
    installed_webkit, WebKitChecker, WebKitDetectionError, WebKitRuntimeInfo,
    DEFAULT_WEBKIT_MIN_VERSION, WEBKITGTK_41_SONAME,
};

/// 逐段数字比较两个版本号。
///
/// 当 `current >= minimum` 时返回 `true`。版本字段必须全部是数字；非法版本返回
/// `false`。Windows 四段版本和 Linux 三段版本均受支持。
pub fn is_version_sufficient(current: &str, minimum: &str) -> bool {
    let Some(cur) = parse_version(current) else {
        return false;
    };
    let Some(min) = parse_version(minimum) else {
        return false;
    };

    let len = cur.len().max(min.len());
    for i in 0..len {
        let current = cur.get(i).copied().unwrap_or(0);
        let required = min.get(i).copied().unwrap_or(0);
        match current.cmp(&required) {
            std::cmp::Ordering::Greater => return true,
            std::cmp::Ordering::Less => return false,
            std::cmp::Ordering::Equal => {}
        }
    }
    true
}

fn parse_version(version: &str) -> Option<Vec<u64>> {
    if version.is_empty() {
        return None;
    }

    version
        .split('.')
        .map(|part| {
            if part.is_empty() {
                None
            } else {
                part.parse::<u64>().ok()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::is_version_sufficient;

    #[test]
    fn compares_three_and_four_part_versions() {
        assert!(is_version_sufficient("2.40", "2.40.0"));
        assert!(is_version_sufficient("2.42.5", "2.40.0"));
        assert!(is_version_sufficient("120.0.1.0", "120.0.0.9"));
        assert!(!is_version_sufficient("2.39.9", "2.40.0"));
    }

    #[test]
    fn rejects_malformed_versions() {
        assert!(!is_version_sufficient("2.bad.5", "2.40.0"));
        assert!(!is_version_sufficient("2.40.0", "2..40"));
        assert!(!is_version_sufficient("", "2.40.0"));
    }
}
