-- Module 2: 底部工具栏 — 点赞/踩
-- is_liked: 1=赞, -1=踩, NULL=未评价

ALTER TABLE messages ADD COLUMN is_liked INTEGER;
