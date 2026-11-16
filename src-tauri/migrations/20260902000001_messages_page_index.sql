-- 游标分页索引：支持 (conversation_id, created_at, id) keyset 查询（方案 3）。
-- 前端滚动到顶按游标逐页加载更早消息，替代原先 limit 硬编码的全量截断。
CREATE INDEX IF NOT EXISTS idx_messages_conversation_page ON messages(conversation_id, created_at DESC, id DESC);
