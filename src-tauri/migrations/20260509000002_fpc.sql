-- FastClaw · FPC 代码生成管线
-- Phase 6: 会话 + 任务表

CREATE TABLE IF NOT EXISTS fpc_sessions (
    id              TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL,
    title           TEXT,
    requirements    TEXT NOT NULL DEFAULT '',
    status          TEXT NOT NULL DEFAULT 'planning',
    current_step    TEXT,
    progress_pct    REAL NOT NULL DEFAULT 0.0,
    framework       TEXT,
    tech_stack      TEXT,
    output_dir      TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (project_id) REFERENCES coding_projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_fpc_sessions_project ON fpc_sessions(project_id);

CREATE TABLE IF NOT EXISTS fpc_tasks (
    id              TEXT PRIMARY KEY,
    session_id      TEXT NOT NULL,
    title           TEXT NOT NULL,
    description     TEXT,
    kind            TEXT NOT NULL DEFAULT 'custom',
    module          TEXT,
    complexity      TEXT NOT NULL DEFAULT 'medium',
    status          TEXT NOT NULL DEFAULT 'pending',
    sort_order      INTEGER NOT NULL DEFAULT 0,
    file_path       TEXT,
    code_content    TEXT,
    dependencies    TEXT,
    error_message   TEXT,
    started_at      TEXT,
    completed_at    TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (session_id) REFERENCES fpc_sessions(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_fpc_tasks_session ON fpc_tasks(session_id, sort_order);
CREATE INDEX IF NOT EXISTS idx_fpc_tasks_status ON fpc_tasks(status);
