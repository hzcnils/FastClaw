//! 私有进程凭证不包含令牌。只凭创建记录及完整指纹恢复自身孤儿进程。
use super::{capture_process_facts, identity_matches_process, ProcessIdentity};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    process::Child,
    sync::{Arc, Mutex},
};

#[derive(Serialize, Deserialize)]
struct Receipt {
    owner_pid: u32,
    owner_started_at: u64,
    identity: ProcessIdentity,
}

#[derive(Default)]
pub struct OwnedProcesses {
    directory: Mutex<Option<PathBuf>>,
    children: Mutex<HashMap<String, Child>>,
    output: Arc<Mutex<HashMap<String, (u32, VecDeque<String>)>>>,
}

impl OwnedProcesses {
    pub fn capture_output(&self, id: &str, child: &mut Child) {
        let pid = child.id();
        self.output
            .lock()
            .unwrap()
            .insert(id.into(), (pid, VecDeque::new()));
        let streams: Vec<Box<dyn std::io::Read + Send>> = child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>)
            .into_iter()
            .chain(
                child
                    .stderr
                    .take()
                    .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
            )
            .collect();
        for mut stream in streams {
            let output = self.output.clone();
            let id = id.to_string();
            std::thread::spawn(move || {
                let mut buffer = [0; 512];
                let mut line = Vec::new();
                while let Ok(length) = stream.read(&mut buffer) {
                    if length == 0 {
                        break;
                    }
                    for byte in &buffer[..length] {
                        if *byte == b'\n' || line.len() >= 2048 {
                            let text = redact(&String::from_utf8_lossy(&line));
                            eprintln!("[{id}] {text}");
                            let mut output = output.lock().unwrap();
                            if let Some((current_pid, lines)) = output.get_mut(&id) {
                                if *current_pid == pid {
                                    lines.push_back(text);
                                    while lines.len() > 40 {
                                        lines.pop_front();
                                    }
                                }
                            }
                            line.clear();
                        }
                        if *byte != b'\n' {
                            line.push(*byte);
                        }
                    }
                }
            });
        }
    }

    pub fn diagnostics(&self, id: &str) -> Vec<String> {
        self.output
            .lock()
            .unwrap()
            .get(id)
            .map(|(_, lines)| lines.iter().cloned().collect())
            .unwrap_or_default()
    }
    pub fn running(&self, id: &str) -> bool {
        self.children
            .lock()
            .unwrap()
            .get_mut(id)
            .is_some_and(|child| matches!(child.try_wait(), Ok(None)))
    }
    pub fn configure(&self, directory: PathBuf) -> std::io::Result<()> {
        std::fs::create_dir_all(&directory)?;
        *self.directory.lock().unwrap() = Some(directory);
        Ok(())
    }

    pub fn retain(&self, id: &str, child: Child) -> Result<(), Child> {
        let mut children = self.children.lock().unwrap();
        if let Some(previous) = children.get_mut(id) {
            if !matches!(previous.try_wait(), Ok(Some(_))) {
                return Err(child);
            }
        }
        children.insert(id.into(), child);
        Ok(())
    }

    pub fn save(&self, id: &str, identity: &ProcessIdentity) -> std::io::Result<()> {
        if !super::recovery::SERVICE_IDS.contains(&id) {
            return Err(std::io::ErrorKind::InvalidInput.into());
        }
        let directory = self.directory.lock().unwrap();
        let Some(directory) = directory.as_ref() else {
            return Ok(());
        };
        let owner = capture_process_facts(&[std::process::id()])
            .remove(&std::process::id())
            .ok_or_else(|| std::io::Error::other("无法保存进程凭证：当前应用身份不可读取"))?;
        let receipt = Receipt {
            owner_pid: std::process::id(),
            owner_started_at: owner.process_started_at,
            identity: identity.clone(),
        };
        let temporary = directory.join(format!("{id}.tmp"));
        std::fs::write(&temporary, serde_json::to_vec(&receipt)?)?;
        std::fs::rename(temporary, directory.join(format!("{id}.json")))
    }

    pub fn orphan(&self, id: &str, expected: &Path) -> Result<Option<ProcessIdentity>, String> {
        if !super::recovery::SERVICE_IDS.contains(&id) {
            return Err("未知服务".into());
        }
        let directory = self.directory.lock().unwrap();
        let Some(directory) = directory.as_ref() else {
            return Ok(None);
        };
        let bytes = match std::fs::read(directory.join(format!("{id}.json"))) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        let receipt: Receipt = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        let facts = capture_process_facts(&[receipt.owner_pid, receipt.identity.pid]);
        if facts
            .get(&receipt.owner_pid)
            .is_some_and(|owner| owner.process_started_at == receipt.owner_started_at)
        {
            return Ok(None);
        }
        if facts.get(&receipt.owner_pid).is_none() && process_exists(receipt.owner_pid) {
            return Err("原应用进程身份不可读取，拒绝接管".into());
        }
        if !identity_matches_process(
            &receipt.identity,
            facts.get(&receipt.identity.pid),
            expected,
        ) {
            if process_exists(receipt.identity.pid) {
                return Err("历史服务进程身份不匹配，拒绝接管".into());
            }
            return Ok(None);
        }
        Ok(Some(receipt.identity))
    }

    /// 持有 Child 时可直接终止该子进程，不使用端口或未经验证的 PID。
    pub fn stop_child(&self, id: &str) -> std::io::Result<bool> {
        let mut children = self.children.lock().unwrap();
        let Some(child) = children.get_mut(id) else {
            return Ok(false);
        };
        if child.try_wait()?.is_none() {
            child.kill()?;
            child.wait()?;
        }
        children.remove(id);
        Ok(true)
    }

    pub fn forget(&self, id: &str) {
        self.children.lock().unwrap().remove(id);
        if let Some(directory) = self.directory.lock().unwrap().as_ref() {
            if super::recovery::SERVICE_IDS.contains(&id) {
                let _ = std::fs::remove_file(directory.join(format!("{id}.json")));
            }
        }
    }
}

fn redact(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    if [
        "token",
        "api_key",
        "api key",
        "authorization",
        "password",
        "service-key",
    ]
    .iter()
    .any(|word| lower.contains(word))
    {
        "[已隐藏可能包含凭据的输出]".into()
    } else {
        line.trim_end_matches('\r').to_string()
    }
}

fn process_exists(pid: u32) -> bool {
    let mut system = sysinfo::System::new();
    system.refresh_processes(
        sysinfo::ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(pid)]),
        true,
    );
    system.process(sysinfo::Pid::from_u32(pid)).is_some()
}

