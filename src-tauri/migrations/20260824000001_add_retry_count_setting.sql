-- 基础参数：模型调用重试次数（默认 5，0 表示不重试）
INSERT OR IGNORE INTO settings (key, value, category, description) VALUES
    ('retry_count', '5', 'general', '智能对话模型调用失败时的重试次数');
