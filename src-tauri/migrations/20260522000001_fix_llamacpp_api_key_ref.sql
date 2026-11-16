-- 将 llama.cpp 供应商的 api_key_ref 改为 "no-key"（本地模型不需要外部 API key）
UPDATE model_providers SET api_key_ref = 'no-key' WHERE id = 'llamacpp';
