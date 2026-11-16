-- FastClaw · 为 agent_management 表新增 json_data 字段，存储 Tinyflow 工作流图谱
ALTER TABLE agent_management ADD COLUMN json_data TEXT DEFAULT NULL;
