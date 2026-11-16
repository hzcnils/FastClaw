-- ChatGLM 迁移 · messages 字段扩展
-- 翻译、深度思考、编辑状态、上下文清除标记

ALTER TABLE messages ADD COLUMN translation TEXT;
ALTER TABLE messages ADD COLUMN translation_lang TEXT;

ALTER TABLE messages ADD COLUMN think_content TEXT;

ALTER TABLE messages ADD COLUMN is_edited INTEGER NOT NULL DEFAULT 0;
ALTER TABLE messages ADD COLUMN edited_at TEXT;

ALTER TABLE messages ADD COLUMN context_cleared INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_messages_context_cleared ON messages(conversation_id, context_cleared);
