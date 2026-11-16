-- Module 5: 溯源面板 — 结果质量反馈表

CREATE TABLE IF NOT EXISTS quality_evaluations (
    id              TEXT PRIMARY KEY,
    message_id      TEXT NOT NULL,
    conversation_id TEXT NOT NULL,
    score           REAL NOT NULL,
    content         TEXT NOT NULL DEFAULT '',
    user_id         TEXT NOT NULL DEFAULT 'default-user',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_quality_eval_message ON quality_evaluations(message_id);
CREATE INDEX IF NOT EXISTS idx_quality_eval_conversation ON quality_evaluations(conversation_id);
