-- FastClaw · 编程模式数据表
-- Phase 1: 项目/会话/书签基础表

-- 注册的编程项目
CREATE TABLE IF NOT EXISTS coding_projects (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    root_path       TEXT NOT NULL UNIQUE,
    language        TEXT,
    last_opened_at  TEXT,
    is_pinned       INTEGER NOT NULL DEFAULT 0,
    description     TEXT,
    framework       TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 编程会话（对应一次 AI 编码对话）
CREATE TABLE IF NOT EXISTS coding_sessions (
    id              TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL,
    conversation_id TEXT,
    title           TEXT,
    model_id        TEXT,
    context_summary TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (project_id) REFERENCES coding_projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_coding_sessions_project ON coding_sessions(project_id);

-- 文件书签
CREATE TABLE IF NOT EXISTS coding_bookmarks (
    id              TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL,
    file_path       TEXT NOT NULL,
    line_number     INTEGER,
    label           TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (project_id) REFERENCES coding_projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_coding_bookmarks_project ON coding_bookmarks(project_id);
