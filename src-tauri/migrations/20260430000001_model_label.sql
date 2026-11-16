-- FastClaw · 为 models 表添加 model_label 字段
-- 用于存储逗号分隔的模型标签（如 "代码,推理"），在模型管理页可配置

ALTER TABLE models ADD COLUMN model_label TEXT NOT NULL DEFAULT '';
