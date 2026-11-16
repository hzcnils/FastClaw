-- FastClaw · 修复本地模型外键约束
-- 允许 provider_id 为 NULL，以便存储本地模型

-- 由于 SQLite 不支持直接修改列约束，需要重建表
-- 1. 创建新表
CREATE TABLE IF NOT EXISTS models_new (
    id              TEXT PRIMARY KEY,
    provider_id     TEXT,  -- 改为可为 NULL
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
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (provider_id) REFERENCES model_providers(id) ON DELETE CASCADE
);

-- 2. 复制数据（包含 model_label 字段）
INSERT INTO models_new SELECT 
    id, provider_id, model_id, display_name, model_type, 
    COALESCE(model_label, ''), -- 使用已有的 model_label 或默认值
    context_window, 
    max_output, input_price, output_price, supports_vision, supports_tools,
    is_enabled, is_default, 0, sort_order, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
    created_at, updated_at
FROM models;

-- 3. 删除旧表
DROP TABLE models;

-- 4. 重命名新表
ALTER TABLE models_new RENAME TO models;

-- 5. 重建索引
CREATE INDEX IF NOT EXISTS idx_models_provider ON models(provider_id);
CREATE INDEX IF NOT EXISTS idx_models_type ON models(model_type);
