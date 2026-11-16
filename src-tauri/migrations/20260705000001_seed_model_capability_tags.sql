-- 根据模型能力字段自动添加固定标签（视觉/工具/推理/重排/嵌入）
-- model_label 为逗号分隔的标签字符串，仅追加缺省标签，不覆盖已有标签

-- 1. 视觉：具备视觉/多模态能力的模型
UPDATE models SET model_label = CASE
  WHEN model_label = '' OR model_label IS NULL THEN '视觉'
  WHEN instr(',' || model_label || ',', ',视觉,') = 0 THEN model_label || ',视觉'
  ELSE model_label
END
WHERE supports_vision = 1
   OR model_id LIKE '%vision%'
   OR model_id LIKE '%vl-%';

-- 2. 工具：具备工具/函数调用能力的模型
UPDATE models SET model_label = CASE
  WHEN model_label = '' OR model_label IS NULL THEN '工具'
  WHEN instr(',' || model_label || ',', ',工具,') = 0 THEN model_label || ',工具'
  ELSE model_label
END
WHERE supports_tools = 1;

-- 3. 推理：具备深度推理/思维链能力的模型
UPDATE models SET model_label = CASE
  WHEN model_label = '' OR model_label IS NULL THEN '推理'
  WHEN instr(',' || model_label || ',', ',推理,') = 0 THEN model_label || ',推理'
  ELSE model_label
END
WHERE model_id LIKE '%o1%'
   OR model_id LIKE '%o3%'
   OR model_id LIKE '%o4%'
   OR model_id LIKE '%r1%'
   OR model_id LIKE '%reasoning%'
   OR model_id LIKE '%qwq%'
   OR model_id LIKE 'deepseek-r%'
   OR model_id LIKE '%think%'
   OR model_id LIKE '%coder%'
   OR display_name LIKE '%Thinking%'
   OR display_name LIKE '%推理%';

-- 4. 嵌入：Embedding 模型
UPDATE models SET model_label = CASE
  WHEN model_label = '' OR model_label IS NULL THEN '嵌入'
  WHEN instr(',' || model_label || ',', ',嵌入,') = 0 THEN model_label || ',嵌入'
  ELSE model_label
END
WHERE model_type = 'embedding'
   OR model_id LIKE '%embed%'
   OR model_id LIKE '%bge-m3%'
   OR model_id LIKE '%bge-large%'
   OR model_id LIKE '%text-embedding%'
   OR display_name LIKE '%Embed%'
   OR display_name LIKE '%嵌入%';

-- 5. 重排：Rerank 模型
UPDATE models SET model_label = CASE
  WHEN model_label = '' OR model_label IS NULL THEN '重排'
  WHEN instr(',' || model_label || ',', ',重排,') = 0 THEN model_label || ',重排'
  ELSE model_label
END
WHERE model_id LIKE '%rerank%'
   OR model_id LIKE '%bge-reranker%'
   OR model_id LIKE '%cohere-rerank%'
   OR model_id LIKE '%jina-reranker%'
   OR display_name LIKE '%Rerank%'
   OR display_name LIKE '%重排%';

-- 6. 指定模型固定标签（覆盖自动匹配结果）
UPDATE models SET model_label = '视觉,推理,工具'   WHERE model_id = 'qwen3-235b-a22b';
UPDATE models SET model_label = '推理,工具'       WHERE model_id = 'deepseek-v4-pro';
UPDATE models SET model_label = '推理,工具,联网'   WHERE model_id = 'glm-5';
UPDATE models SET model_label = '推理,工具'       WHERE model_id = 'qwen3.7-max';
UPDATE models SET model_label = '推理,工具,联网'   WHERE model_id = 'glm-5.1';
UPDATE models SET model_label = '视觉,推理,工具'   WHERE model_id = 'qwen3-122b-a10b';
UPDATE models SET model_label = '视觉,推理,工具'   WHERE model_id = 'qwen3.5-35b-a3b';
UPDATE models SET model_label = '推理,工具'       WHERE model_id = 'qwq-32b';
