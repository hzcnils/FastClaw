-- FastClaw · 添加模型市场字段到 models 表
-- 用于支持本地模型上传功能

-- 添加模型市场相关字段
ALTER TABLE models ADD COLUMN model_name TEXT;
ALTER TABLE models ADD COLUMN model_category TEXT;
ALTER TABLE models ADD COLUMN model_desc TEXT;
ALTER TABLE models ADD COLUMN model_tags TEXT;
ALTER TABLE models ADD COLUMN model_path TEXT;
ALTER TABLE models ADD COLUMN model_url TEXT;
ALTER TABLE models ADD COLUMN is_local_model INTEGER NOT NULL DEFAULT 0;
ALTER TABLE models ADD COLUMN model_version TEXT;
ALTER TABLE models ADD COLUMN model_size TEXT;
