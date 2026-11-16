-- FastClaw · 添加 provider_code 字段，重构 models 表外键关联
-- provider_code 是供应商的标准化标识代码（如 OPENAI, CLAUDE），
-- models 表改用 provider_code 关联 model_providers

-- Step 1: 给 model_providers 添加 provider_code 并回填
ALTER TABLE model_providers ADD COLUMN provider_code TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_providers_code ON model_providers(provider_code);

UPDATE model_providers SET provider_code = 'CLAUDE'          WHERE id = 'claude';
UPDATE model_providers SET provider_code = 'OPENAI'          WHERE id = 'codex';
UPDATE model_providers SET provider_code = 'GEMINI'          WHERE id = 'gemini';
UPDATE model_providers SET provider_code = 'DEEPSEEK'        WHERE id = 'deepseek';
UPDATE model_providers SET provider_code = 'Q_WEN'           WHERE id = 'qwen';
UPDATE model_providers SET provider_code = 'DOUYIN'          WHERE id = 'doubao';
UPDATE model_providers SET provider_code = 'LLAMACPP'        WHERE id = 'llamacpp';
UPDATE model_providers SET provider_code = 'OLLAMA'          WHERE id = 'ollama';
UPDATE model_providers SET provider_code = 'NEWAPI'          WHERE id = 'openai_compat';

-- Step 2: 重建 models 表，FK 从 provider_id REFERENCES id 改为 provider_code REFERENCES provider_code
PRAGMA foreign_keys = OFF;

CREATE TABLE models_new (
    id              TEXT PRIMARY KEY,
    provider_code   TEXT,
    model_id        TEXT NOT NULL,
    display_name    TEXT NOT NULL,
    model_type      TEXT NOT NULL DEFAULT 'chat',
    model_label     TEXT NOT NULL DEFAULT '',
    context_window  INTEGER,
    max_output      INTEGER,
    input_price     REAL,
    output_price    REAL,
    supports_vision INTEGER NOT NULL DEFAULT 0,
    supports_tools  INTEGER NOT NULL DEFAULT 0,
    is_enabled      INTEGER NOT NULL DEFAULT 1,
    is_default      INTEGER NOT NULL DEFAULT 0,
    is_local_model  INTEGER NOT NULL DEFAULT 0,
    sort_order      INTEGER NOT NULL DEFAULT 0,
    model_name      TEXT,
    model_category  TEXT,
    model_desc      TEXT,
    model_tags      TEXT,
    model_path      TEXT,
    model_url       TEXT,
    model_version   TEXT,
    model_size      TEXT,
    temperature     REAL,
    top_p           REAL,
    extra_config    TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (provider_code) REFERENCES model_providers(provider_code) ON DELETE CASCADE
);

INSERT INTO models_new SELECT
    m.id, p.provider_code, m.model_id, m.display_name, m.model_type,
    COALESCE(m.model_label, ''), m.context_window, m.max_output,
    m.input_price, m.output_price, m.supports_vision, m.supports_tools,
    m.is_enabled, m.is_default, COALESCE(m.is_local_model, 0), m.sort_order,
    m.model_name, m.model_category, m.model_desc, m.model_tags,
    m.model_path, m.model_url, m.model_version, m.model_size,
    m.temperature, m.top_p, m.extra_config,
    m.created_at, m.updated_at
FROM models m
LEFT JOIN model_providers p ON m.provider_id = p.id;

DROP TABLE models;
ALTER TABLE models_new RENAME TO models;

CREATE INDEX IF NOT EXISTS idx_models_provider ON models(provider_code);
CREATE INDEX IF NOT EXISTS idx_models_type ON models(model_type);

PRAGMA foreign_keys = ON;

-- Step 3: token_usage_logs 添加 provider_code 列（不改变存量数据，仅支持新记录写入）
ALTER TABLE token_usage_logs ADD COLUMN provider_code TEXT;
