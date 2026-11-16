-- 模型评分：消息级点赞/点踩沉淀到模型维度（多用户友好）
CREATE TABLE IF NOT EXISTS model_feedback (
    id            TEXT PRIMARY KEY,
    user_id       TEXT NOT NULL DEFAULT 'default-user',
    model_id      TEXT NOT NULL,           -- 对应 messages.model_id / models.model_id
    message_id    TEXT NOT NULL,
    signal        INTEGER NOT NULL,        -- 1=赞, -1=踩
    quality_score REAL,                    -- 预留：TraceSource 的 0~10 质量分
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(user_id, message_id),           -- 同一用户对同一条消息只保留一条
    FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_model_feedback_model ON model_feedback(model_id);
CREATE INDEX IF NOT EXISTS idx_model_feedback_user  ON model_feedback(user_id);
