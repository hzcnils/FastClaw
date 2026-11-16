use crate::core::config::{
    WebView2Config, DEFAULT_WEBVIEW2_MIN_VERSION, DEFAULT_WEBVIEW2_OFFLINE_INSTALLER_URL,
    DEFAULT_WEBVIEW2_ONLINE_INSTALLER_URL,
};
use webview2_checker::{is_version_sufficient, WebView2Checker};

/// WebView2 运行时版本检测 — 配置由 FastClaw 读取后传给独立检查器。
pub fn check_webview2_version(config: &WebView2Config) {
    let min_version = valid_min_version(&config.min_version);
    let online_url = valid_download_url(
        &config.online_installer_url,
        DEFAULT_WEBVIEW2_ONLINE_INSTALLER_URL,
        "online_installer_url",
    );
    let offline_url = valid_download_url(
        &config.offline_installer_url,
        DEFAULT_WEBVIEW2_OFFLINE_INSTALLER_URL,
        "offline_installer_url",
    );
    let timeout_secs = config.network_timeout_secs.clamp(1, 30);
    if timeout_secs != config.network_timeout_secs {
        log::warn!(
            "[WebView2] network_timeout_secs={} 超出 1..=30，已调整为 {}",
            config.network_timeout_secs,
            timeout_secs
        );
    }

    WebView2Checker::new()
        .min_version(min_version)
        .online_url(online_url)
        .offline_url(offline_url)
        .network_timeout_secs(timeout_secs)
        .check();
}

fn valid_min_version(value: &str) -> &str {
    let value = value.trim();
    if is_version_sufficient(value, value) {
        value
    } else {
        log::warn!(
            "[WebView2] min_version='{}' 无效，回退到 {}",
            value,
            DEFAULT_WEBVIEW2_MIN_VERSION
        );
        DEFAULT_WEBVIEW2_MIN_VERSION
    }
}

fn valid_download_url<'a>(value: &'a str, fallback: &'static str, field: &str) -> &'a str {
    let value = value.trim();
    if value.starts_with("http://") || value.starts_with("https://") {
        value
    } else {
        log::warn!("[WebView2] {}='{}' 无效，使用默认地址", field, value);
        fallback
    }
}
