-- 智能体偏好参数：为 agent_management 增加 agent_preferences 列（JSON 字符串）
ALTER TABLE agent_management ADD COLUMN agent_preferences TEXT DEFAULT NULL;
