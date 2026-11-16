-- Module 1: 多模型回答对比 — 新增 group_id 字段
-- 标识同一轮对话的多个模型回答，支持 tabs/horizontal 切换展示

ALTER TABLE messages ADD COLUMN group_id TEXT;
CREATE INDEX IF NOT EXISTS idx_messages_group ON messages(group_id);
