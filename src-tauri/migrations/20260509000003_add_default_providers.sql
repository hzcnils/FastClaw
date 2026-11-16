-- 添加默认供应商数据
INSERT OR IGNORE INTO model_providers (id, name, provider_type, base_url, api_key_ref, is_enabled, is_local, sort_order) VALUES
('claude', 'Anthropic Claude', 'anthropic', 'https://api.anthropic.com', NULL, 1, 0, 1),
('codex', 'OpenAI', 'openai', 'https://api.openai.com/v1', NULL, 1, 0, 2),
('gemini', 'Google Gemini', 'openai', 'https://generativelanguage.googleapis.com', NULL, 1, 0, 3),
('deepseek', 'DeepSeek', 'openai', 'https://api.deepseek.com/v1', NULL, 1, 0, 4),
('qwen', 'Qwen', 'openai', 'https://dashscope.aliyuncs.com/compatible-mode/v1', NULL, 1, 0, 5),
('doubao', '豆包', 'openai', 'https://ark.cn-beijing.volces.com/api/v3', NULL, 1, 0, 6),
('llamacpp', 'llama.cpp', 'openai', 'http://127.0.0.1:11435/v1', NULL, 1, 1, 7),
('ollama', 'Ollama', 'ollama', 'http://localhost:11434/v1', NULL, 1, 1, 8),
('openai_compat', 'newApi', 'openai', '', NULL, 1, 0, 9);

-- 添加默认模型数据
INSERT OR IGNORE INTO models (id, provider_id, model_id, display_name, model_type, model_label, context_window, max_output, supports_vision, supports_tools, is_enabled, is_default, sort_order) VALUES
-- Anthropic Claude
('claude-opus-4-7', 'claude', 'claude-opus-4-7', 'Claude Opus 4.7', 'chat', '', 200000, 4096, 1, 1, 1, 1, 1),
('claude-sonnet-4-6', 'claude', 'claude-sonnet-4-6', 'Claude Sonnet 4.6', 'chat', '', 200000, 4096, 1, 1, 1, 0, 2),
('claude-haiku-4-5', 'claude', 'claude-haiku-4-5', 'Claude Haiku 4.5', 'chat', '', 200000, 4096, 1, 1, 1, 0, 3),
-- OpenAI
('codex-gpt-4o', 'codex', 'gpt-4o', 'GPT-4o', 'chat', '', 128000, 4096, 1, 1, 1, 1, 1),
('codex-gpt-4o-mini', 'codex', 'gpt-4o-mini', 'GPT-4o Mini', 'chat', '', 128000, 4096, 1, 1, 1, 0, 2),
('codex-o3-mini', 'codex', 'o3-mini', 'o3 Mini', 'chat', '', 200000, 4096, 0, 1, 1, 0, 3),
-- Google Gemini
('gemini-2.5-pro', 'gemini', 'gemini-2.5-pro', 'Gemini 2.5 Pro', 'chat', '', 1048576, 4096, 1, 1, 1, 1, 1),
('gemini-2.5-flash', 'gemini', 'gemini-2.5-flash', 'Gemini 2.5 Flash', 'chat', '', 1048576, 4096, 1, 1, 1, 0, 2),
-- DeepSeek
('deepseek-v4-pro', 'deepseek', 'deepseek-v4-pro', 'DeepSeek V4 Pro', 'chat', '', 131072, 8192, 0, 1, 1, 1, 1),
('deepseek-v4-flash', 'deepseek', 'deepseek-v4-flash', 'DeepSeek V4 Flash', 'chat', '', 131072, 8192, 0, 1, 1, 0, 2),
-- Qwen
('qwen3-max', 'qwen', 'qwen3-max', 'Qwen3 Max', 'chat', '', 131072, 4096, 0, 1, 1, 1, 1),
('qwen3-plus', 'qwen', 'qwen3-plus', 'Qwen3 Plus', 'chat', '', 131072, 4096, 0, 1, 1, 0, 2),
('qwen3-coder-plus', 'qwen', 'qwen3-coder-plus', 'Qwen3 Coder Plus', 'chat', '', 131072, 4096, 0, 1, 1, 0, 3),
-- 豆包
('doubao-thinking-pro', 'doubao', 'doubao-seed-1-6-thinking-pro', 'Doubao Thinking Pro', 'chat', '', 131072, 4096, 0, 1, 1, 1, 1),
('doubao-pro', 'doubao', 'doubao-seed-1-6-pro', 'Doubao Pro', 'chat', '', 131072, 4096, 0, 1, 1, 0, 2),
('doubao-lite', 'doubao', 'doubao-seed-1-6-lite', 'Doubao Lite', 'chat', '', 131072, 4096, 0, 1, 1, 0, 3),
-- llamacpp
('qwen-1', 'llamacpp', 'qwen', 'qwen', 'chat', '', 131072, 4096, 0, 1, 1, 1, 1);
