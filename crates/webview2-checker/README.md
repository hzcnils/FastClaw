# webview2-checker

Windows WebView2 与 Linux WebKitGTK 运行时检测库，可在 Tauri 或其他桌面程序启动前检查运行时是否存在、版本是否满足要求，并根据插件判断出的内网、公网或离线环境引导用户手动安装。

## 功能

| 平台 | 检测方式 | 用户引导 |
|---|---|---|
| Windows | 查询 HKLM/HKCU 中的 WebView2 版本 | 原生 `MessageBoxW`，打开在线或离线地址 |
| Linux | 动态加载 `libwebkit2gtk-4.1.so.0` 并调用官方版本函数 | 通过 `zenity`、`kdialog` 或 `yad` 打开调用方配置的页面，安装后可重新检测 |

网络环境由插件自行判断，支持组合以下探针：

- TCP：检查指定主机和端口是否可连接。
- DNS：检查指定内网或公网域名是否能解析。
- HTTP：向内网 HTTP 健康检查地址发送 `HEAD` 请求。
- 每组探针可配置为“任一成功”或“全部成功”。
- 内网探针优先；内网命中时，即使公网也可达，仍使用内网链接。

Linux 内网模式只负责打开配置链接，不会自动下载软件包、运行包管理器或请求管理员权限。

## 添加依赖

```toml
[target.'cfg(any(windows, target_os = "linux"))'.dependencies]
webview2-checker = { path = "../webview2-checker" }
```

## Windows

```rust
#[cfg(windows)]
fn check_webview() {
    use webview2_checker::{NetworkDetector, NetworkProbe, WebView2Checker};

    let network = NetworkDetector::new()
        // 探测实际在线下载域名；不可达时使用内网下载地址。
        .internet_probe(NetworkProbe::tcp("go.microsoft.com:443"))
        .timeout_secs(3);

    WebView2Checker::new()
        .min_version("120.0.0.0")
        .online_url("https://go.microsoft.com/fwlink/p/?LinkId=2124703")
        .offline_url("http://192.168.1.10:7000/WebView2.exe")
        .network_detector(network)
        .check();
}
```

Windows 端保持在线/非在线二态：公网探针任一成功时使用 `online_url`，否则使用
`offline_url`。这里的 `offline_url` 表示无法直连互联网时提供的内网下载地址；库不会
继续验证该地址，也不会自动下载安装包。未显式设置 `network_detector` 时继续使用默认
公网探针，以兼容原有调用。

## Linux：库内调用

```rust
#[cfg(target_os = "linux")]
fn check_webkit() {
    use webview2_checker::{NetworkDetector, NetworkProbe, ProbePolicy, WebKitChecker};

    let network = NetworkDetector::new()
        // 多个内网判断方式，任一成功即认为在内网。
        .intranet_probe(NetworkProbe::tcp("192.168.1.10:7000"))
        .intranet_probe(NetworkProbe::dns("packages.company.local"))
        .intranet_probe(NetworkProbe::http(
            "http://packages.company.local/health",
        ))
        .intranet_policy(ProbePolicy::Any)
        // 公网也可以配置多个探针。
        .internet_probe(NetworkProbe::tcp("223.5.5.5:53"))
        .internet_probe(NetworkProbe::tcp("1.1.1.1:53"))
        .internet_policy(ProbePolicy::Any)
        .timeout_secs(2);

    WebKitChecker::new()
        .min_version("2.40.0")
        .network_detector(network)
        .intranet_url("http://packages.company.local/webkitgtk")
        .online_url("https://webkitgtk.org/")
        .offline_url("file:///opt/my-app/help/webkitgtk.html")
        .check();
}
```

链接选择严格对应检测结果：

| 检测结果 | 使用链接 | 回退行为 |
|---|---|---|
| `Intranet` | `intranet_url` | 不回退到公网链接 |
| `Internet` | `online_url` | 不回退到内网链接 |
| `Offline` | `offline_url` | 未配置则只显示错误 |

`NetworkProbe::Http` 为保持零额外依赖，只支持 `http://`。要探测 HTTPS 服务，请使用其主机和 443 端口配置 `NetworkProbe::Tcp("host:443")`。

## Linux：独立启动器

如果 Tauri 主程序本身动态链接 WebKitGTK，WebKitGTK 完全缺失时，系统动态链接器可能在进入 Rust `main()` 之前终止程序。因此发行版应让桌面入口启动本项目提供的独立检测器，再由检测器启动真正的应用。

```bash
webview-runtime-launcher \
  --app /opt/my-app/my-app-bin \
  --min-version 2.40.0 \
  --intranet-url http://packages.company.local/webkitgtk \
  --online-url https://support.example.com/webkitgtk \
  --offline-url file:///opt/my-app/help/webkitgtk.html \
  --intranet-tcp 192.168.1.10:7000 \
  --intranet-dns packages.company.local \
  --intranet-http http://packages.company.local/health \
  --no-default-internet-probes \
  --timeout-secs 2 \
  -- --application-argument
```

探针参数均可重复。没有配置公网探针时，启动器默认使用 `8.8.8.8:53` 和 `223.5.5.5:53`；严格内网环境可以传入 `--no-default-internet-probes`，确保不发起默认公网探测。`--` 后面的参数会原样转发给主程序。检测通过后，启动器等待主程序结束并返回相同退出码。

## Linux 检测行为

默认只接受 WebKitGTK 4.1 的稳定 SONAME：

```text
libwebkit2gtk-4.1.so.0
```

检测器动态解析以下函数并获得实际加载的运行库版本：

```text
webkit_get_major_version
webkit_get_minor_version
webkit_get_micro_version
```

默认最低版本为 `2.40.0`。可以通过 `.min_version(...)` 覆盖。默认不会把 WebKitGTK 4.0 或 WebKitGTK 6.0 当作 4.1 的兼容替代品。

检测失败或版本过低时：

1. 插件检测内网、公网或离线状态。
2. 显示实际加载错误、当前环境和最低版本。
3. 用户点击“打开下载页”。
4. 插件通过 `xdg-open`，失败时回退到 `gio open`。
5. 用户手动下载并安装。
6. 用户点击“重新检测”；满足要求后继续启动应用。

## 查询 API

```rust
#[cfg(windows)]
let version = webview2_checker::installed_version();

#[cfg(target_os = "linux")]
match webview2_checker::installed_webkit() {
    Ok(info) => println!("{}: {}", info.soname, info.version),
    Err(error) => eprintln!("{error}"),
}
```

通用 API：

| API | 说明 |
|---|---|
| `is_version_sufficient(current, minimum)` | 严格解析并逐段比较版本 |
| `NetworkDetector::detect()` | 返回 `Intranet`、`Internet` 或 `Offline` |
| `NetworkProbe::tcp(...)` | TCP 连通性探针 |
| `NetworkProbe::dns(...)` | DNS 解析探针 |
| `NetworkProbe::http(...)` | 内网 HTTP HEAD 探针 |

## 平台限制

目前支持 Windows 和 Linux。其他平台会在编译期报错。
