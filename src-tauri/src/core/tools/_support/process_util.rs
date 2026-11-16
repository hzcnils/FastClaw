// FastClaw · Minimal process helpers for tool subprocesses
//
// Provides the NoConsoleWindow trait so worktree/repl_tool/brief tool code
// compiles unchanged. The actual implementations are thin wrappers that
// suppress console windows on Windows.

/// Trait for hiding console windows on Windows (no-op on Unix).
pub trait NoConsoleWindow {
    fn hide_console_window(&mut self) -> &mut Self;
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(windows)]
impl NoConsoleWindow for std::process::Command {
    fn hide_console_window(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        self.creation_flags(CREATE_NO_WINDOW)
    }
}

#[cfg(windows)]
impl NoConsoleWindow for tokio::process::Command {
    fn hide_console_window(&mut self) -> &mut Self {
        self.creation_flags(CREATE_NO_WINDOW)
    }
}

#[cfg(not(windows))]
impl NoConsoleWindow for std::process::Command {
    fn hide_console_window(&mut self) -> &mut Self {
        self
    }
}

#[cfg(not(windows))]
impl NoConsoleWindow for tokio::process::Command {
    fn hide_console_window(&mut self) -> &mut Self {
        self
    }
}
