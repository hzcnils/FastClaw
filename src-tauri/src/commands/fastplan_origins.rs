/// Build the child allowlist without depending on WebView navigation completing.
/// Inputs are serialized URL origins, never full URLs or credentials.
pub(super) fn parent_origins(
    development: bool,
    dev_origin: Option<&str>,
    window_origin: Option<&str>,
) -> String {
    let mut origins = vec![
        "tauri://localhost",
        "http://tauri.localhost",
        "https://tauri.localhost",
    ];
    for origin in [dev_origin.filter(|_| development), window_origin]
        .into_iter()
        .flatten()
    {
        if !origin.is_empty() && origin != "null" && !origins.contains(&origin) {
            origins.push(origin);
        }
    }
    origins.join(",")
}

#[cfg(test)]
mod tests {
    use super::parent_origins;

    #[test]
    fn development_origin_is_present_before_window_navigation() {
        for window in [None, Some("null"), Some("http://tauri.localhost")] {
            let origins = parent_origins(true, Some("http://127.0.0.1:1420"), window);
            assert!(origins.split(',').any(|origin| origin == "http://127.0.0.1:1420"));
            assert!(!origins.split(',').any(|origin| origin == "null"));
        }
    }

    #[test]
    fn loaded_window_does_not_duplicate_configured_origin() {
        let dev = "http://localhost:1430";
        assert_eq!(
            parent_origins(true, Some(dev), None),
            parent_origins(true, Some(dev), Some(dev)),
        );
    }

    #[test]
    fn production_does_not_add_development_config() {
        assert_eq!(
            parent_origins(false, Some("http://127.0.0.1:1420"), None),
            "tauri://localhost,http://tauri.localhost,https://tauri.localhost",
        );
    }
}
