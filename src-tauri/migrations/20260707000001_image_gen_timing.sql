-- 图像生成耗时统计表
-- 记录每次图像生成的实际耗时，用于计算各模型的平均生成时间并展示给用户
CREATE TABLE IF NOT EXISTS image_gen_timing (
    id              TEXT PRIMARY KEY,
    model_id        TEXT NOT NULL,        -- 模型标识（如 "gpt-image-2"）
    provider_code   TEXT,                  -- 供应商代码（如 "AIARTMIRROR"）
    duration_ms     INTEGER NOT NULL,      -- 生成耗时（毫秒）
    image_size      TEXT,                  -- 图像尺寸（如 "1024x1024"）
    success         INTEGER NOT NULL DEFAULT 1,  -- 是否成功（1=成功，0=失败）
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_image_gen_timing_model ON image_gen_timing(model_id);
CREATE INDEX IF NOT EXISTS idx_image_gen_timing_created ON image_gen_timing(created_at);
