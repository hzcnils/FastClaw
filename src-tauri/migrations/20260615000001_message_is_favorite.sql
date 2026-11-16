-- 消息收藏标记：0=未收藏, 1=已收藏
ALTER TABLE messages ADD COLUMN is_favorite INTEGER NOT NULL DEFAULT 0;
