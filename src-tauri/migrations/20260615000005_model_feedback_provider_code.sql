-- 评分表加 provider_code 列，支持按服务商区分评分
ALTER TABLE model_feedback ADD COLUMN provider_code TEXT NOT NULL DEFAULT '';
