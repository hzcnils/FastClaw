-- Module 1 fix: 为历史消息补充 group_id
-- 策略：相邻 user 消息与其后紧跟的 assistant 消息共享同一 group_id
-- 单条消息（无配对）分配独立 group_id

-- 此迁移通过临时表 + 窗口函数重建 group_id 配对关系，仅处理 group_id IS NULL 的行

UPDATE messages SET group_id = lower(hex(randomblob(16)))
WHERE group_id IS NULL;
