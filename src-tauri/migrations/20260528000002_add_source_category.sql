-- 添加供应商来源分类字段：online(联网)、local(本地)、fastci(FastCI)
ALTER TABLE model_providers ADD COLUMN source_category TEXT NOT NULL DEFAULT 'online';

-- 添加默认"私域网"供应商（局域网模型对接）
INSERT OR IGNORE INTO model_providers (id, name, provider_type, base_url, api_key_ref, is_enabled, is_local, sort_order, provider_code, source_category) VALUES
('private', '私域网', 'openai', '', NULL, 1, 0, 10, 'PRIVATE', 'local');

-- 汇智网(NEWAPI)：来源类型改为汇智网，API地址改为汇智网网关
UPDATE model_providers SET source_category = 'huizhi', base_url = 'http://113.201.2.47:3000/v1', api_key_ref = NULL WHERE provider_code = 'NEWAPI';

-- 本电脑(Ollama/llama.cpp)：来源类型改为本地
UPDATE model_providers SET source_category = 'local' WHERE provider_code IN ('OLLAMA', 'LLAMACPP');
