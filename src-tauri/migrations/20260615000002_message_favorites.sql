-- 消息收藏独立表：收藏快照独立于 messages，删除对话后收藏仍保留
CREATE TABLE IF NOT EXISTS message_favorites (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL DEFAULT 'default-user',
    message_id TEXT,
    conversation_id TEXT,
    conversation_title TEXT NOT NULL DEFAULT '未命名对话',
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    model_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
