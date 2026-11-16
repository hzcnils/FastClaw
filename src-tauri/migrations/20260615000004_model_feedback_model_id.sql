-- 评分表加 model_id 列（稳定键，不受 models.id 重导变化影响）
ALTER TABLE model_feedback ADD COLUMN model_id TEXT NOT NULL DEFAULT '';
