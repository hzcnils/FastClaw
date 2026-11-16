-- 创建统一的"本电脑"供应商（合并 Ollama 和 llama.cpp）
INSERT OR IGNORE INTO model_providers (id, name, provider_type, base_url, api_key_ref, is_enabled, is_local, sort_order, provider_code, source_category, extra_config)
VALUES ('local_provider', '本电脑', 'ollama', 'http://localhost:11434/v1', NULL, 1, 1, 8, 'LOCAL', 'local',
        '{"local_backend":"Ollama","ollama_url":"http://localhost:11434/v1","openai_url":"http://127.0.0.1:11435/v1"}');

-- 将原有 Ollama 和 llama.cpp 的模型迁移到 LOCAL 供应商下
UPDATE models SET provider_code = 'LOCAL' WHERE provider_code IN ('OLLAMA', 'LLAMACPP');
