-- FastClaw · 模型网关功能
-- 新增 model_gateways 表 + model_providers 表 gateway_id 字段

-- 模型网关表
CREATE TABLE IF NOT EXISTS model_gateways (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    description     TEXT DEFAULT '',
    sort_order      INTEGER NOT NULL DEFAULT 0,
    is_enabled      INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- model_providers 新增网关关联字段
ALTER TABLE model_providers ADD COLUMN gateway_id TEXT;

-- 索引
CREATE INDEX IF NOT EXISTS idx_providers_gateway ON model_providers(gateway_id);

-- 预置默认网关数据
INSERT OR IGNORE INTO model_gateways (id, name, description, sort_order) VALUES
('lite-llm',          'LiteLLM',              '开源 LLM 网关代理', 1),
('one-api',           'One API',              'OpenAI 接口管理 & 分发系统', 2),
('new-api',           'New API',              '下一代 One API，AI 网关接口管理分发系统', 3),
('portkey',           'Portkey',              'LLM 网关、可观测性与护栏平台', 4),
('helicone',          'Helicone',             '面向 LLM 的观测与网关平台', 5),
('hermes',            'Hermes',               'LLM 统一网关', 6),
('godex',             'GodeX',                '多模型聚合网关', 7),
('any-llm-gateway',   'any-llm-gateway',      '通用 LLM 网关', 8),
('openrouter',        'OpenRouter',           '统一 LLM 接口路由平台', 9),
('4sapi',             '4SAPI',               '多模型 API 分发平台', 10),
('siliconflow',       '硅基流动',              '硅基流动 AI 模型网关', 11),
('kong-ai-gateway',   'Kong AI Gateway',      'Kong 官方 AI 网关', 12),
('cloudflare-ai-gw',  'Cloudflare AI Gateway','Cloudflare AI 网关服务', 13),
('aliyun-bailian',    '阿里云百炼网关',         '阿里云百炼大模型平台网关', 14),
('baidu-qianfan',     '百度千帆网关',           '百度千帆大模型平台网关', 15),
('volc-ark',          '火山方舟网关',           '火山引擎方舟大模型平台网关', 16);