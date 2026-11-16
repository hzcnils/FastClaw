-- FastClaw · 个人助理角色系统
-- 创建角色主表、工具绑定关联表及广场公开表

CREATE TABLE IF NOT EXISTS roles (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    description     TEXT,
    avatar          TEXT,
    system_prompt   TEXT NOT NULL DEFAULT '',
    default_model   TEXT,
    temperature     REAL,
    context_turns   INTEGER NOT NULL DEFAULT 10,
    memory_mode     TEXT NOT NULL DEFAULT 'global',
    allow_publish   INTEGER NOT NULL DEFAULT 1,
    is_active       INTEGER NOT NULL DEFAULT 0,
    is_published    INTEGER NOT NULL DEFAULT 0,
    source_type     TEXT NOT NULL DEFAULT 'user',
    source_id       TEXT,
    square_category TEXT,
    square_desc     TEXT,
    published_at    TEXT,
    notes           TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_roles_is_active    ON roles(is_active);
CREATE INDEX IF NOT EXISTS idx_roles_is_published ON roles(is_published);
CREATE INDEX IF NOT EXISTS idx_roles_source_type  ON roles(source_type);

CREATE TABLE IF NOT EXISTS role_agents (
    role_id    TEXT NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    agent_id   TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (role_id, agent_id)
);

CREATE TABLE IF NOT EXISTS role_mcps (
    role_id    TEXT NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    mcp_id     TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (role_id, mcp_id)
);

CREATE TABLE IF NOT EXISTS role_skills (
    role_id    TEXT NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    skill_id   TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (role_id, skill_id)
);

CREATE TABLE IF NOT EXISTS role_knowledge_bases (
    role_id           TEXT NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    knowledge_base_id TEXT NOT NULL,
    sort_order        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (role_id, knowledge_base_id)
);

-- 广场公开角色表（不存储 system_prompt，保护隐私）
CREATE TABLE IF NOT EXISTS square_roles (
    id           TEXT PRIMARY KEY,
    origin_id    TEXT,
    name         TEXT NOT NULL,
    description  TEXT,
    avatar       TEXT,
    category     TEXT NOT NULL DEFAULT 'other',
    author_name  TEXT,
    author_id    TEXT,
    use_count    INTEGER NOT NULL DEFAULT 0,
    star_count   INTEGER NOT NULL DEFAULT 0,
    tool_summary TEXT,
    published_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_square_roles_category     ON square_roles(category);
CREATE INDEX IF NOT EXISTS idx_square_roles_use_count    ON square_roles(use_count DESC);
CREATE INDEX IF NOT EXISTS idx_square_roles_star_count   ON square_roles(star_count DESC);
CREATE INDEX IF NOT EXISTS idx_square_roles_published_at ON square_roles(published_at DESC);

-- 内置广场精选角色（演示数据）
INSERT OR IGNORE INTO square_roles (id, name, description, category, author_name, use_count, star_count, tool_summary, published_at, updated_at) VALUES
(
    'sq-builtin-001',
    '专业写作助手',
    '擅长各类文体写作、润色与创意表达，让你的文字更生动有力。',
    'writing',
    'FastClaw 官方',
    1280,
    328,
    '{}',
    datetime('now'),
    datetime('now')
),
(
    'sq-builtin-002',
    '编程专家',
    '精通多种编程语言，擅长代码审查、调试和架构设计，给出可运行的示例代码。',
    'coding',
    'FastClaw 官方',
    2100,
    512,
    '{}',
    datetime('now'),
    datetime('now')
),
(
    'sq-builtin-003',
    '高效工作助手',
    '专注提升工作效率，善用结构化输出、优先级管理和任务分解帮你高效完成工作。',
    'work',
    'FastClaw 官方',
    890,
    215,
    '{}',
    datetime('now'),
    datetime('now')
),
(
    'sq-builtin-004',
    '学习辅导老师',
    '耐心细致地解答学习疑惑，用通俗易懂的方式讲解复杂概念，适合各阶段学习者。',
    'education',
    'FastClaw 官方',
    670,
    183,
    '{}',
    datetime('now'),
    datetime('now')
),
(
    'sq-builtin-005',
    '生活百科顾问',
    '涵盖生活方方面面的实用建议，从健康饮食到旅行规划，做你的贴心生活助手。',
    'life',
    'FastClaw 官方',
    450,
    97,
    '{}',
    datetime('now'),
    datetime('now')
);
