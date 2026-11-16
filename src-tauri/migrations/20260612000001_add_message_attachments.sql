-- 消息附件持久化：存储图片/截图的 base64 元数据，供回显使用
ALTER TABLE messages ADD COLUMN attachments TEXT;
