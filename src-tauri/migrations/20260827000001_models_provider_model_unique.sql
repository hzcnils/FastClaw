-- 模型同步/导入以 (provider_code, model_id) 作为业务唯一键。
-- 迁移历史重复项时保留最近更新的记录，并修复使用模型实例 ID 的引用。

CREATE TEMP TABLE fastclaw_model_duplicate_map (
    duplicate_id TEXT PRIMARY KEY,
    canonical_id TEXT NOT NULL
);

INSERT INTO fastclaw_model_duplicate_map (duplicate_id, canonical_id)
WITH ranked AS (
    SELECT
        id,
        FIRST_VALUE(id) OVER (
            PARTITION BY provider_code, model_id
            ORDER BY COALESCE(updated_at, created_at, '') DESC, rowid DESC
        ) AS canonical_id,
        ROW_NUMBER() OVER (
            PARTITION BY provider_code, model_id
            ORDER BY COALESCE(updated_at, created_at, '') DESC, rowid DESC
        ) AS duplicate_rank
    FROM models
    WHERE provider_code IS NOT NULL
)
SELECT id, canonical_id
FROM ranked
WHERE duplicate_rank > 1;

UPDATE messages
SET model_instance_id = (
    SELECT canonical_id
    FROM fastclaw_model_duplicate_map
    WHERE duplicate_id = messages.model_instance_id
)
WHERE model_instance_id IN (SELECT duplicate_id FROM fastclaw_model_duplicate_map);

UPDATE model_feedback
SET model_instance_id = (
    SELECT canonical_id
    FROM fastclaw_model_duplicate_map
    WHERE duplicate_id = model_feedback.model_instance_id
)
WHERE model_instance_id IN (SELECT duplicate_id FROM fastclaw_model_duplicate_map);

DELETE FROM models
WHERE id IN (SELECT duplicate_id FROM fastclaw_model_duplicate_map);

DROP TABLE fastclaw_model_duplicate_map;

CREATE UNIQUE INDEX IF NOT EXISTS idx_models_provider_model_unique
ON models(provider_code, model_id)
WHERE provider_code IS NOT NULL;
