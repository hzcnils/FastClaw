-- 为 agent_management 表新增 Agent 管理完善字段
-- 添加状态、可见范围、标签、创建人、封面、功能描述、输入输出等字段

-- 先创建 agent_management 表（如果不存在）
CREATE TABLE IF NOT EXISTS agent_management (
    "id" TEXT,
    "agent_name" TEXT NOT NULL,
    "agent_description" TEXT,
    "agent_icon" TEXT,
    "agent_prompt" TEXT,
    "agent_type" TEXT,
    "agent_model" TEXT,
    "agent_knowledgebase" TEXT,
    "created_time" TEXT NOT NULL DEFAULT (datetime('now')),
    "update_time" TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY ("id")
);

-- 添加新字段
ALTER TABLE agent_management ADD COLUMN agent_status TEXT NOT NULL DEFAULT 'enabled';
ALTER TABLE agent_management ADD COLUMN agent_visibility TEXT NOT NULL DEFAULT 'all';
ALTER TABLE agent_management ADD COLUMN agent_visibility_roles TEXT DEFAULT NULL;
ALTER TABLE agent_management ADD COLUMN agent_tags TEXT DEFAULT NULL;
ALTER TABLE agent_management ADD COLUMN agent_creator TEXT DEFAULT NULL;
ALTER TABLE agent_management ADD COLUMN agent_cover TEXT DEFAULT NULL;
ALTER TABLE agent_management ADD COLUMN agent_function_desc TEXT DEFAULT NULL;
ALTER TABLE agent_management ADD COLUMN agent_input_params TEXT DEFAULT NULL;
ALTER TABLE agent_management ADD COLUMN agent_output_results TEXT DEFAULT NULL;
