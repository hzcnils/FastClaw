// FastClaw · WSP 进程封装
// 封装 tokio::process::Child，提供启动和杀掉 exe 的能力

use crate::core::wsp::types::WspService;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use tokio::process::{Child, Command};

/// WSP 进程封装
pub struct WspProcess {
    child: Option<Child>,
    pid: u32,
}

impl WspProcess {
    /// 从已有的 Child 创建 WspProcess
    pub fn new(child: Child) -> Self {
        let pid = child.id().unwrap_or(0);
        Self {
            child: Some(child),
            pid,
        }
    }

    /// 创建虚拟进程（用于 GGUF 服务，不实际运行进程）
    pub fn new_virtual() -> Self {
        Self {
            child: None,
            pid: 0,
        }
    }

    /// 启动 WSP exe 进程
    #[allow(dead_code)]
    pub async fn spawn(service: &WspService) -> anyhow::Result<Self> {
        let args = service.parsed_args();
        let env_vars = service.parsed_env_vars();

        let mut cmd = Command::new(&service.exe_path);

        // 设置工作目录（默认用系统临时目录，避免 PyInstaller _MEI 散落）
        if let Some(ref wd) = service.work_dir {
            cmd.current_dir(wd);
        } else {
            cmd.current_dir(std::env::temp_dir());
        }

        // 设置启动参数
        cmd.args(&args);

        // 设置环境变量
        for (k, v) in &env_vars {
            cmd.env(k, v);
        }

        // 不继承标准输入，捕获 stdout/stderr 用于日志
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        #[cfg(windows)]
        {
            cmd.as_std_mut().creation_flags(0x08000000);
        }

        let child = cmd.spawn()?;
        let pid = child.id().unwrap_or(0);

        tracing::debug!("WSP 进程已启动: {} (PID: {:?})", service.name, pid);

        Ok(Self {
            child: Some(child),
            pid,
        })
    }

    /// 获取进程 PID
    pub fn pid(&self) -> u32 {
        self.pid
    }
    /// 杀掉进程（优先使用 sysinfo 根据 PID 杀死）
    pub async fn kill(&mut self) -> anyhow::Result<()> {
        // 如果是虚拟进程，直接返回成功
        if self.child.is_none() {
            tracing::info!("虚拟进程无需杀死");
            return Ok(());
        }

        let pid = self.pid;

        // 先尝试用 tokio 的 kill
        if let Err(e) = self.child.as_mut().unwrap().kill().await {
            tracing::warn!("tokio kill 失败: {}", e);
        }

        // 使用 sysinfo 强制杀死进程（Windows 上更可靠）
        #[cfg(windows)]
        {
            use std::process::Command as StdCommand;
            // Windows 上使用 taskkill /F /T 强制杀死进程树
            use std::os::windows::process::CommandExt;
            let output = StdCommand::new("taskkill")
                .args(&["/F", "/T", "/PID", &pid.to_string()])
                .creation_flags(0x08000000)
                .output();

            match output {
                Ok(out) => {
                    if out.status.success() {
                        tracing::info!("进程树已杀死 (PID: {})", pid);
                    } else {
                        let err = String::from_utf8_lossy(&out.stderr);
                        tracing::warn!("taskkill 失败: {}", err);
                    }
                }
                Err(e) => {
                    tracing::warn!("taskkill 执行失败: {}", e);
                }
            }
        }

        Ok(())
    }

    /// 尝试等待进程退出（非阻塞）
    pub fn try_wait(&mut self) -> anyhow::Result<Option<std::process::ExitStatus>> {
        // 如果是虚拟进程，返回 None（表示进程仍在运行）
        if self.child.is_none() {
            return Ok(None);
        }
        Ok(self.child.as_mut().unwrap().try_wait()?)
    }
}
