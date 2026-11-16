-- FastClaw · 默认供应商与模型数据同步
-- 数据来源: docs/model_providers.sql, docs/models.sql
-- 使用 INSERT OR REPLACE 确保所有数据库（新建或已有）都包含最新的默认数据

-- ============================================
-- 供应商默认数据
-- ============================================
INSERT OR REPLACE INTO model_providers (id, name, provider_type, base_url, api_key_ref, is_enabled, is_local, sort_order, extra_config, provider_code, source_category) VALUES
('claude',       'Anthropic Claude', 'anthropic', 'https://api.anthropic.com',                          NULL,                                                                             1, 0, 1,  NULL,                                                                                                                                                                               'CLAUDE',    'online'),
('codex',        'OpenAI',           'openai',    'https://api.openai.com/v1',                           NULL,                                                                             1, 0, 2,  NULL,                                                                                                                                                                               'OPENAI',    'online'),
('gemini',       'Google Gemini',    'openai',    'https://generativelanguage.googleapis.com',            NULL,                                                                             1, 0, 3,  NULL,                                                                                                                                                                               'GEMINI',    'online'),
('deepseek',     'DeepSeek',         'openai',    'https://api.deepseek.com/v1',                         NULL,                                                                             1, 0, 4,  NULL,                                                                                                                                                                               'DEEPSEEK',  'online'),
('qwen',         'Qwen',             'openai',    'https://dashscope.aliyuncs.com/compatible-mode/v1',    NULL,                                                                             1, 0, 5,  NULL,                                                                                                                                                                               'Q_WEN',     'online'),
('doubao',       '豆包',             'openai',    'https://ark.cn-beijing.volces.com/api/v3',            NULL,                                                                             1, 0, 6,  NULL,                                                                                                                                                                               'DOUYIN',    'online'),
('llamacpp',     'llama.cpp',        'openai',    'http://127.0.0.1:11435/v1',                           'no-key',                                                                         1, 1, 7,  NULL,                                                                                                                                                                               'LLAMACPP',  'local'),
('ollama',       'Ollama',           'ollama',    'http://localhost:11434/v1',                            NULL,                                                                             1, 1, 8,  NULL,                                                                                                                                                                               'OLLAMA',    'local'),
('openai_compat','汇智网',           'openai',    'http://113.201.2.47:3000/v1',                         NULL,                                                                             1, 0, 9,  NULL,                                                                                                                                                                               'NEWAPI',    'huizhi'),
('private',      '私有服务',         'openai',    '',                                                    NULL,                                                                             0, 0, 10, NULL,                                                                                                                                                                               'PRIVATE',   'local'),
('local_provider','本电脑',          'openai',    'http://127.0.0.1:11435/v1',                           NULL,                                                                             1, 1, 8,  '{"local_backend":"Ollama","ollama_url":"http://localhost:11434/v1","openai_url":"http://127.0.0.1:11435/v1"}',       'LOCAL',     'local');

-- ============================================
-- 模型默认数据
-- ============================================

-- Anthropic Claude
INSERT OR REPLACE INTO models (id, provider_code, model_id, display_name, model_type, model_label, context_window, max_output, input_price, output_price, supports_vision, supports_tools, is_enabled, is_default, is_local_model, sort_order, model_name, model_category, model_desc, model_tags, model_path, model_url, model_version, model_size, temperature, top_p, extra_config, mxcs) VALUES
('claude-opus-4-7',   'CLAUDE', 'claude-opus-4-7',   'Claude Opus 4.7',   'chat', '',  200000, 4096, NULL, NULL, 1, 1, 1, 1, 0, 1, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('claude-sonnet-4-6', 'CLAUDE', 'claude-sonnet-4-6', 'Claude Sonnet 4.6', 'chat', '',  200000, 4096, NULL, NULL, 1, 1, 1, 0, 0, 2, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('claude-haiku-4-5',  'CLAUDE', 'claude-haiku-4-5',  'Claude Haiku 4.5',  'chat', '',  200000, 4096, NULL, NULL, 1, 1, 1, 0, 0, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);

-- OpenAI
INSERT OR REPLACE INTO models (id, provider_code, model_id, display_name, model_type, model_label, context_window, max_output, input_price, output_price, supports_vision, supports_tools, is_enabled, is_default, is_local_model, sort_order, model_name, model_category, model_desc, model_tags, model_path, model_url, model_version, model_size, temperature, top_p, extra_config, mxcs) VALUES
('codex-gpt-4o',      'OPENAI', 'gpt-4o',      'GPT-4o',      'chat', '', 128000, 4096, NULL, NULL, 1, 1, 1, 1, 0, 1, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('codex-gpt-4o-mini', 'OPENAI', 'gpt-4o-mini', 'GPT-4o Mini', 'chat', '', 128000, 4096, NULL, NULL, 1, 1, 1, 0, 0, 2, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('codex-o3-mini',     'OPENAI', 'o3-mini',     'o3 Mini',     'chat', '', 200000, 4096, NULL, NULL, 0, 1, 1, 0, 0, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);

-- Google Gemini
INSERT OR REPLACE INTO models (id, provider_code, model_id, display_name, model_type, model_label, context_window, max_output, input_price, output_price, supports_vision, supports_tools, is_enabled, is_default, is_local_model, sort_order, model_name, model_category, model_desc, model_tags, model_path, model_url, model_version, model_size, temperature, top_p, extra_config, mxcs) VALUES
('gemini-2.5-pro',   'GEMINI', 'gemini-2.5-pro',   'Gemini 2.5 Pro',   'chat', '', 1048576, 4096, NULL, NULL, 1, 1, 1, 1, 0, 1, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('gemini-2.5-flash', 'GEMINI', 'gemini-2.5-flash', 'Gemini 2.5 Flash', 'chat', '', 1048576, 4096, NULL, NULL, 1, 1, 1, 0, 0, 2, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);

-- DeepSeek
INSERT OR REPLACE INTO models (id, provider_code, model_id, display_name, model_type, model_label, context_window, max_output, input_price, output_price, supports_vision, supports_tools, is_enabled, is_default, is_local_model, sort_order, model_name, model_category, model_desc, model_tags, model_path, model_url, model_version, model_size, temperature, top_p, extra_config, mxcs) VALUES
('deepseek-v4-pro',   'DEEPSEEK', 'deepseek-v4-pro',   'DeepSeek V4 Pro',   'chat', '', 131072, 8192, NULL, NULL, 0, 1, 1, 1, 0, 1, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('deepseek-v4-flash', 'DEEPSEEK', 'deepseek-v4-flash', 'DeepSeek V4 Flash', 'chat', '', 131072, 8192, NULL, NULL, 0, 1, 1, 0, 0, 2, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);

-- Qwen
INSERT OR REPLACE INTO models (id, provider_code, model_id, display_name, model_type, model_label, context_window, max_output, input_price, output_price, supports_vision, supports_tools, is_enabled, is_default, is_local_model, sort_order, model_name, model_category, model_desc, model_tags, model_path, model_url, model_version, model_size, temperature, top_p, extra_config, mxcs) VALUES
('qwen3-max',        'Q_WEN', 'qwen3-max',        'Qwen3 Max',        'chat', '', 131072, 4096, NULL, NULL, 0, 1, 1, 1, 0, 1, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('qwen3-plus',       'Q_WEN', 'qwen3-plus',       'Qwen3 Plus',       'chat', '', 131072, 4096, NULL, NULL, 0, 1, 1, 0, 0, 2, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('qwen3-coder-plus', 'Q_WEN', 'qwen3-coder-plus', 'Qwen3 Coder Plus', 'chat', '', 131072, 4096, NULL, NULL, 0, 1, 1, 0, 0, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);

-- 豆包
INSERT OR REPLACE INTO models (id, provider_code, model_id, display_name, model_type, model_label, context_window, max_output, input_price, output_price, supports_vision, supports_tools, is_enabled, is_default, is_local_model, sort_order, model_name, model_category, model_desc, model_tags, model_path, model_url, model_version, model_size, temperature, top_p, extra_config, mxcs) VALUES
('doubao-thinking-pro', 'DOUYIN', 'doubao-seed-1-6-thinking-pro', 'Doubao Thinking Pro', 'chat', '', 131072, 4096, NULL, NULL, 0, 1, 1, 1, 0, 1, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('doubao-pro',          'DOUYIN', 'doubao-seed-1-6-pro',          'Doubao Pro',          'chat', '', 131072, 4096, NULL, NULL, 0, 1, 1, 0, 0, 2, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('doubao-lite',         'DOUYIN', 'doubao-seed-1-6-lite',         'Doubao Lite',         'chat', '', 131072, 4096, NULL, NULL, 0, 1, 1, 0, 0, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);

-- 本电脑 (LOCAL) 模型
INSERT OR REPLACE INTO models (id, provider_code, model_id, display_name, model_type, model_label, context_window, max_output, input_price, output_price, supports_vision, supports_tools, is_enabled, is_default, is_local_model, sort_order, model_name, model_category, model_desc, model_tags, model_path, model_url, model_version, model_size, temperature, top_p, extra_config, mxcs) VALUES
('qwen-1', 'LOCAL', 'qwen', 'qwen', 'chat', '', 131072, 4096, NULL, NULL, 0, 1, 1, 1, 0, 1, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);

INSERT OR REPLACE INTO models (id, provider_code, model_id, display_name, model_type, model_label, context_window, max_output, input_price, output_price, supports_vision, supports_tools, is_enabled, is_default, is_local_model, sort_order, model_name, model_category, model_desc, model_tags, model_path, model_url, model_version, model_size, temperature, top_p, extra_config, mxcs) VALUES
('OLLAMA-qwen3:32b', 'LOCAL', 'qwen3:32b', 'qwen3:32b', 'chat', '本地', NULL, 2000, NULL, NULL, 0, 0, 1, 0, 1, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 0.2, 0.8, '{"apiKey":"qwen3:32b","azureDeploymentName":null,"dimension":null,"endpoint":null,"geminiLocation":null,"geminiProject":null,"imageQuality":null,"imageSize":null,"imageStyle":null,"responseLimit":2000,"secretKey":null,"source":"LOCAL"}', '{
  "模型标识": {
    "正式名称": "通义千问",
    "版本号": "Qwen-7B-Chat",
    "训练参数规模": "保密",
    "参数量大小": "7B",
    "所属模型类别": "大型语言模型"
  },
  "推理与对话能力": {
    "支持的推理模式": ["chat", "reasoning"],
    "对话上下文最大长度": 30720,
    "单次最大输入token": 30720,
    "最大输出token上限": 2048
  },
  "原生支持任务类型": ["对话", "向量", "代码", "图像描述生成", "函数调用", "工具调用"],
  "默认生成参数": {
    "温度": 0.8,
    "top_p": 0.95,
    "top_k": 50,
    "重复惩罚": 1.0,
    "生成长度": 2048
  },
  "运行架构": {
    "部署架构": "阿里云自研推理框架",
    "推理引擎": "TorchScript",
    "加载格式": "FP16 + INT8混合量化",
    "量化等级": "INT8"
  }
}');

INSERT OR REPLACE INTO models (id, provider_code, model_id, display_name, model_type, model_label, context_window, max_output, input_price, output_price, supports_vision, supports_tools, is_enabled, is_default, is_local_model, sort_order, model_name, model_category, model_desc, model_tags, model_path, model_url, model_version, model_size, temperature, top_p, extra_config, mxcs) VALUES
('OLLAMA-deepseek-r1:32b', 'LOCAL', 'deepseek-r1:32b', 'deepseek-r1:32b', 'chat', '本地', NULL, 2000, NULL, NULL, 0, 0, 1, 0, 1, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 0.2, 0.8, '{"apiKey":"deepseek-r1:32b","azureDeploymentName":null,"dimension":null,"endpoint":null,"geminiLocation":null,"geminiProject":null,"imageQuality":null,"imageSize":null,"imageStyle":null,"responseLimit":2000,"secretKey":null,"source":"LOCAL"}', NULL);

-- 汇智网 (NEWAPI) 模型
INSERT OR REPLACE INTO models (id, provider_code, model_id, display_name, model_type, model_label, context_window, max_output, input_price, output_price, supports_vision, supports_tools, is_enabled, is_default, is_local_model, sort_order, model_name, model_category, model_desc, model_tags, model_path, model_url, model_version, model_size, temperature, top_p, extra_config, mxcs) VALUES
('NEWAPI-qwen3-235b-a22b', 'NEWAPI', 'qwen3-235b-a22b', 'qwen3-235b-a22b', 'chat', 'New API 模型', NULL, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 0.2, 0.0, '{"apiKey":null,"azureDeploymentName":null,"dimension":null,"endpoint":null,"geminiLocation":null,"geminiProject":null,"imageQuality":null,"imageSize":null,"imageStyle":null,"responseLimit":null,"secretKey":null,"source":"NEWAPI"}', '{
  "模型标识": "Qwen3",
  "正式名称": "通义千问3",
  "版本号": "v3.0",
  "训练参数规模": "超大规模",
  "参数量大小": "1000亿+",
  "模型类别": "语言模型",
  "支持的推理模式": ["chat", "completion", "function_calling"],
  "对话上下文最大长度": 32768,
  "单次最大输入token": 32768,
  "最大输出token上限": 8192,
  "原生支持的任务类型": ["对话", "向量", "代码", "函数调用", "工具调用"],
  "默认推理温度": 0.7,
  "top_p": 0.9,
  "top_k": 50,
  "重复惩罚": 1.1,
  "生成长度": 2048,
  "运行部署架构": "Transformer",
  "推理引擎": "自研推理引擎",
  "加载格式": ["FP16", "INT4", "INT8"],
  "量化等级": "支持INT4/INT8量化"
}');

INSERT OR REPLACE INTO models (id, provider_code, model_id, display_name, model_type, model_label, context_window, max_output, input_price, output_price, supports_vision, supports_tools, is_enabled, is_default, is_local_model, sort_order, model_name, model_category, model_desc, model_tags, model_path, model_url, model_version, model_size, temperature, top_p, extra_config, mxcs) VALUES
('NEWAPI-deepseek-v4-pro', 'NEWAPI', 'deepseek-v4-pro', 'deepseek-v4-pro', 'llm', '', 8192, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('NEWAPI-glm-5', 'NEWAPI', 'glm-5', 'glm-5', 'llm', '', 8192, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('NEWAPI-qwen3.7-max', 'NEWAPI', 'qwen3.7-max', 'qwen3.7-max', 'llm', '', 8192, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('NEWAPI-glm-5.1', 'NEWAPI', 'glm-5.1', 'glm-5.1', 'llm', '', 8192, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('NEWAPI-qwen3.5-122b-a10b', 'NEWAPI', 'qwen3.5-122b-a10b', 'qwen3.5-122b-a10b', 'llm', '', 8192, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('NEWAPI-qwen3.5-35b-a3b', 'NEWAPI', 'qwen3.5-35b-a3b', 'qwen3.5-35b-a3b', 'llm', '', 8192, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('NEWAPI-qwq-32b', 'NEWAPI', 'qwq-32b', 'qwq-32b', 'llm', '', 8192, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('NEWAPI-ZHIPU/GLM-5.1', 'NEWAPI', 'ZHIPU/GLM-5.1', 'ZHIPU/GLM-5.1', 'llm', '', 8192, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);

-- 私域网 (PRIVATE) 模型
INSERT OR REPLACE INTO models (id, provider_code, model_id, display_name, model_type, model_label, context_window, max_output, input_price, output_price, supports_vision, supports_tools, is_enabled, is_default, is_local_model, sort_order, model_name, model_category, model_desc, model_tags, model_path, model_url, model_version, model_size, temperature, top_p, extra_config, mxcs) VALUES
('PRIVATE-qwen3-vl-plus', 'PRIVATE', 'qwen3-vl-plus', 'qwen3-vl-plus', 'llm', '', 8192, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('PRIVATE-qwen3.5-plus', 'PRIVATE', 'qwen3.5-plus', 'qwen3.5-plus', 'llm', '', 8192, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('PRIVATE-deepseek-v4-flash', 'PRIVATE', 'deepseek-v4-flash', 'deepseek-v4-flash', 'llm', '', 8192, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('PRIVATE-deepseek-v4-pro', 'PRIVATE', 'deepseek-v4-pro', 'deepseek-v4-pro', 'llm', '', 8192, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('PRIVATE-glm-5.1', 'PRIVATE', 'glm-5.1', 'glm-5.1', 'llm', '', 8192, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('PRIVATE-qwen3.7-max', 'PRIVATE', 'qwen3.7-max', 'qwen3.7-max', 'llm', '', 8192, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('PRIVATE-qwen3.5-122b-a10b', 'PRIVATE', 'qwen3.5-122b-a10b', 'qwen3.5-122b-a10b', 'llm', '', 8192, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('PRIVATE-qwen3.5-35b-a3b', 'PRIVATE', 'qwen3.5-35b-a3b', 'qwen3.5-35b-a3b', 'llm', '', 8192, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('PRIVATE-qwen3.6-27b', 'PRIVATE', 'qwen3.6-27b', 'qwen3.6-27b', 'llm', '', 8192, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
('PRIVATE-qwen3.6-35b-a3b', 'PRIVATE', 'qwen3.6-35b-a3b', 'qwen3.6-35b-a3b', 'llm', '', 8192, NULL, NULL, NULL, 0, 0, 1, 0, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);
