-- 去重 models：同 (provider_code, model_id) 仅保留 rowid 最小的一行
DELETE FROM models
WHERE rowid NOT IN (
  SELECT MIN(rowid) FROM models GROUP BY provider_code, model_id
);
