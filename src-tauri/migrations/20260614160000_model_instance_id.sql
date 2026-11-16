-- 模型评分按实例区分：messages 增加 model_instance_id，model_feedback 改用实例 ID 聚合
ALTER TABLE messages ADD COLUMN model_instance_id TEXT;

-- 重建 model_feedback，将 model_id 替换为 model_instance_id（表内无历史数据，可直接重建）
DROP TABLE IF EXISTS model_feedback;
CREATE TABLE model_feedback (
    id                 TEXT PRIMARY KEY,
    user_id            TEXT NOT NULL DEFAULT 'default-user',
    model_instance_id  TEXT NOT NULL,
    message_id         TEXT NOT NULL,
    signal             INTEGER NOT NULL,
    quality_score      REAL,
    created_at         TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at         TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(user_id, message_id),
    FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_model_feedback_instance ON model_feedback(model_instance_id);
CREATE INDEX IF NOT EXISTS idx_model_feedback_user     ON model_feedback(user_id);
