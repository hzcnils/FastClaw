-- 清空汇智网供应商的硬编码 api_key，后续由用户登录后动态获取
UPDATE model_providers SET api_key_ref = '' WHERE provider_code = 'HUIZHI';

-- 清空私域网供应商的 base_url 和 api_key_ref
UPDATE model_providers SET base_url = '', api_key_ref = '' WHERE provider_code = 'PRIVATE';

-- 将 llama.cpp 的默认 base_url 改为 http://localhost:11435/v1
UPDATE model_providers SET base_url = 'http://localhost:11435/v1' WHERE provider_code = 'LLAMACPP';

-- 将本电脑（LOCAL）的默认 base_url 改为 http://localhost:11434/v1，extra_config 重置为默认值
UPDATE model_providers SET base_url = 'http://localhost:11434/v1', extra_config = '{"local_backend":"Ollama","ollama_url":"http://localhost:11434/v1","openai_url":"http://localhost:11435/v1"}' WHERE provider_code = 'LOCAL';
