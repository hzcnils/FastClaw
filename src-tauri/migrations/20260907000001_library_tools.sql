-- 本地工具库（HTTP / SKILL 等），替代 FastPlan /api/admin/tools
CREATE TABLE IF NOT EXISTS library_tools (
    id            TEXT PRIMARY KEY NOT NULL,
    name          TEXT NOT NULL UNIQUE,
    display_name  TEXT NOT NULL,
    description   TEXT NOT NULL DEFAULT '',
    tool_type     TEXT NOT NULL,
    input_schema  TEXT NOT NULL DEFAULT '{}',
    config        TEXT NOT NULL DEFAULT '{}',
    is_active     INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_library_tools_type ON library_tools(tool_type);
CREATE INDEX IF NOT EXISTS idx_library_tools_active ON library_tools(is_active);
