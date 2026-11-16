#[cfg(target_os = "linux")]
use std::process::Command;

#[cfg(target_os = "linux")]
use webview2_checker::{NetworkDetector, NetworkProbe, WebKitChecker};

#[cfg(target_os = "linux")]
fn main() {
    if let Err(error) = run() {
        eprintln!("webview-runtime-launcher: {error}");
        std::process::exit(2);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("webview-runtime-launcher 仅用于 Linux");
}

#[cfg(target_os = "linux")]
fn run() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let mut app = None;
    let mut app_arguments = Vec::new();
    let mut checker = WebKitChecker::new();
    let mut detector = NetworkDetector::new();
    let mut has_internet_probe = false;
    let mut use_default_internet_probes = true;

    while let Some(argument) = arguments.next() {
        if argument == "--" {
            app_arguments.extend(arguments);
            break;
        }
        match argument.as_str() {
            "--app" => app = Some(next_value("--app", &mut arguments)?),
            "--min-version" => {
                checker = checker.min_version(next_value("--min-version", &mut arguments)?)
            }
            "--online-url" => {
                checker = checker.online_url(next_value("--online-url", &mut arguments)?)
            }
            "--intranet-url" => {
                checker = checker.intranet_url(next_value("--intranet-url", &mut arguments)?)
            }
            "--offline-url" => {
                checker = checker.offline_url(next_value("--offline-url", &mut arguments)?)
            }
            "--intranet-tcp" => {
                detector = detector.intranet_probe(NetworkProbe::tcp(next_value(
                    "--intranet-tcp",
                    &mut arguments,
                )?))
            }
            "--internet-tcp" => {
                has_internet_probe = true;
                detector = detector.internet_probe(NetworkProbe::tcp(next_value(
                    "--internet-tcp",
                    &mut arguments,
                )?))
            }
            "--intranet-dns" => {
                detector = detector.intranet_probe(NetworkProbe::dns(next_value(
                    "--intranet-dns",
                    &mut arguments,
                )?))
            }
            "--internet-dns" => {
                has_internet_probe = true;
                detector = detector.internet_probe(NetworkProbe::dns(next_value(
                    "--internet-dns",
                    &mut arguments,
                )?))
            }
            "--intranet-http" => {
                detector = detector.intranet_probe(NetworkProbe::http(next_value(
                    "--intranet-http",
                    &mut arguments,
                )?))
            }
            "--internet-http" => {
                has_internet_probe = true;
                detector = detector.internet_probe(NetworkProbe::http(next_value(
                    "--internet-http",
                    &mut arguments,
                )?))
            }
            "--timeout-secs" => {
                let seconds = next_value("--timeout-secs", &mut arguments)?
                    .parse::<u64>()
                    .map_err(|_| "--timeout-secs 必须是整数".to_string())?;
                detector = detector.timeout_secs(seconds);
            }
            "--no-default-internet-probes" => use_default_internet_probes = false,
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            unknown => return Err(format!("未知参数：{unknown}")),
        }
    }

    let app = app.ok_or_else(|| "必须提供 --app <可执行文件>".to_string())?;
    if use_default_internet_probes && !has_internet_probe {
        detector = detector
            .internet_probe(NetworkProbe::tcp("8.8.8.8:53"))
            .internet_probe(NetworkProbe::tcp("223.5.5.5:53"));
    }
    checker.network_detector(detector).check();

    let status = Command::new(app)
        .args(app_arguments)
        .status()
        .map_err(|error| format!("无法启动主程序：{error}"))?;
    std::process::exit(status.code().unwrap_or(1));
}

#[cfg(target_os = "linux")]
fn next_value(
    name: &str,
    arguments: &mut std::iter::Skip<std::env::Args>,
) -> Result<String, String> {
    arguments.next().ok_or_else(|| format!("{name} 缺少参数值"))
}

#[cfg(target_os = "linux")]
fn print_help() {
    println!(
        "用法：webview-runtime-launcher --app <程序> [检测选项] [-- <程序参数>]\n\
         \n检测选项：\n\
         --min-version <版本>\n\
         --online-url <公网下载页>\n\
         --intranet-url <内网下载页>\n\
         --offline-url <离线帮助页>\n\
         --intranet-tcp <主机:端口>（可重复）\n\
         --internet-tcp <主机:端口>（可重复）\n\
         --intranet-dns <主机名>（可重复）\n\
         --internet-dns <主机名>（可重复）\n\
         --intranet-http <http://地址>（可重复）\n\
         --internet-http <http://地址>（可重复）\n\
         --no-default-internet-probes\n\
         --timeout-secs <秒>"
    );
}
