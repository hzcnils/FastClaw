// FastClaw · Minimal background-task stub for migrated bash/tasks tools.
//
// Background-task stubs for bash and task tools.
// We provide type-compatible stubs here; full background-task infrastructure is deferred
// to a future infra phase.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Minimal background-task fields.
#[derive(Debug, Clone)]
pub struct BackgroundTask {
    pub id: String,
    pub command: String,
    pub status: TaskStatus,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// A simple in-memory task registry (stub).
pub struct TaskRegistry {
    tasks: Mutex<HashMap<String, BackgroundTask>>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self {
            tasks: Mutex::new(HashMap::new()),
        }
    }

    pub fn register(&self, task: BackgroundTask) {
        if let Ok(mut m) = self.tasks.lock() {
            m.insert(task.id.clone(), task);
        }
    }

    pub fn get(&self, id: &str) -> Option<BackgroundTask> {
        self.tasks.lock().ok()?.get(id).cloned()
    }

    pub fn update(&self, id: &str, status: TaskStatus) {
        if let Ok(mut m) = self.tasks.lock() {
            if let Some(t) = m.get_mut(id) {
                t.status = status;
            }
        }
    }
}

/// Shared background-task registry.
/// Tools call `.register(t)` on the shared registry,
/// so a fn returning Arc<TaskRegistry> works identically for callers.
pub fn global_registry() -> Arc<TaskRegistry> {
    use std::sync::OnceLock;
    static R: OnceLock<Arc<TaskRegistry>> = OnceLock::new();
    R.get_or_init(|| Arc::new(TaskRegistry::new())).clone()
}
