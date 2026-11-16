// FastClaw · 统一日志与崩溃记录
// 桥接 log → tracing, 文件 + stderr 双输出, 自定义 panic hook

use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use tracing_appender::rolling::RollingFileAppender;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::{fmt, prelude::*, EnvFilter, Registry};

fn crash_log_dir() -> PathBuf {
    // 不要写到 src-tauri/：`tauri dev` 会把日志当成源码变动并整包重编
    let dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.fastclaw.app")
        .join("crash-logs");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

// ─── 自定义日志 Writer（基于 Mutex<RollingFileAppender>，确保写入可靠） ───

struct LogWriter {
    inner: Mutex<RollingFileAppender>,
}

struct LogWriterGuard<'a> {
    inner: std::sync::MutexGuard<'a, RollingFileAppender>,
}

impl Write for LogWriterGuard<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl<'a> MakeWriter<'a> for LogWriter {
    type Writer = LogWriterGuard<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        LogWriterGuard {
            inner: self.inner.lock().expect("LogWriter 锁失败"),
        }
    }
}

// ─── panic hook ───

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic_info| {
        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        let payload = panic_info.payload();
        let msg = if let Some(s) = payload.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "[non-string payload]".to_string()
        };

        let backtrace = std::backtrace::Backtrace::force_capture();

        let crash_content = format!(
            "=== FASTCLAW CRASH REPORT ===\n\
             Timestamp (Unix): {}\n\
             Location:          {}\n\
             Payload:           {}\n\
             Thread:            {:?}\n\
             \n\
             Stack Backtrace:\n\
             {:#?}\n\
             \n=== END CRASH REPORT ===\n",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            location,
            msg,
            std::thread::current().name().unwrap_or("<unnamed>"),
            backtrace,
        );

        let crash_path = crash_log_dir().join(format!(
            "crash_{}.log",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        ));

        match std::fs::File::create(&crash_path) {
            Ok(mut f) => {
                let _ = f.write_all(crash_content.as_bytes());
                let _ = f.flush();
            }
            Err(e) => {
                eprintln!(
                    "[CRASH] Cannot write crash log to {}: {}",
                    crash_path.display(),
                    e
                );
            }
        }

        default_hook(panic_info);
    }));
}

// ─── 初始化 ───

pub fn init_logging() {
    let log_dir = crash_log_dir();
    let file_appender = tracing_appender::rolling::hourly(&log_dir, "fastclaw.log");
    let log_writer = LogWriter {
        inner: Mutex::new(file_appender),
    };

    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_writer(log_writer);

    let stderr_layer = fmt::layer().with_target(false).with_writer(std::io::stderr);

    let filter = EnvFilter::builder()
        .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
        .with_env_var("RUST_LOG")
        .parse_lossy("sqlx=warn,FastClaw=debug");

    Registry::default()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .init();

    install_panic_hook();

    // 测试日志写入
    tracing::info!(
        "[logging] 日志系统初始化完成, 日志目录: {}",
        log_dir.display()
    );
}
