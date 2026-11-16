-- 补建：消息收藏表的唯一约束（user_id + message_id），使 upsert ON CONFLICT 能正确触发
CREATE UNIQUE INDEX IF NOT EXISTS idx_message_favorites_user_msg ON message_favorites(user_id, message_id);
