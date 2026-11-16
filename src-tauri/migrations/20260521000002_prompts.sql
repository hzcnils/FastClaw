-- ChatGLM 迁移 · 提示词模板表（替代旧 Java 接口 /knowledgeRepository/jmisPrompt/*）

CREATE TABLE IF NOT EXISTS prompts (
    id              TEXT PRIMARY KEY,
    title           TEXT NOT NULL,
    content         TEXT NOT NULL,
    description     TEXT,
    category        TEXT,
    source          TEXT NOT NULL DEFAULT 'user',     -- 'system' | 'user' | 'shared'
    author          TEXT,
    is_official     INTEGER NOT NULL DEFAULT 0,
    download_count  INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_prompts_category ON prompts(category);
CREATE INDEX IF NOT EXISTS idx_prompts_source ON prompts(source);

CREATE TABLE IF NOT EXISTS prompt_downloads (
    id              TEXT PRIMARY KEY,
    prompt_id       TEXT NOT NULL,
    user_id         TEXT NOT NULL DEFAULT 'default-user',
    downloaded_at   TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (prompt_id) REFERENCES prompts(id) ON DELETE CASCADE,
    UNIQUE (prompt_id, user_id)
);
CREATE INDEX IF NOT EXISTS idx_prompt_downloads_user ON prompt_downloads(user_id);

-- 内置系统提示词种子
INSERT OR IGNORE INTO prompts (id, title, content, description, category, source, author, is_official) VALUES
    ('sys-prompt-explain', '通俗解释',
        '请用通俗易懂的语言解释以下内容，避免使用专业术语，并举一个生活中的例子帮助理解：\n\n{{topic}}',
        '把复杂概念讲清楚，适合学习与科普', 'study', 'system', 'FastClaw', 1),
    ('sys-prompt-summarize', '内容摘要',
        '请阅读以下内容，提炼核心观点，用 5 条要点概括，每条不超过 30 字：\n\n{{content}}',
        '快速摘要长文章 / 文档', 'office', 'system', 'FastClaw', 1),
    ('sys-prompt-translate-en', '中译英',
        '请把以下中文翻译为地道的英文，保持原意，避免直译：\n\n{{text}}',
        '中英互译', 'translate', 'system', 'FastClaw', 1),
    ('sys-prompt-translate-zh', '英译中',
        '请把以下英文翻译为流畅自然的中文，保持原意：\n\n{{text}}',
        '英中互译', 'translate', 'system', 'FastClaw', 1),
    ('sys-prompt-code-review', '代码评审',
        '请对以下代码进行评审，指出潜在 Bug、性能问题、安全隐患、可读性改进点，并给出修改建议：\n\n```\n{{code}}\n```',
        '代码质量审查', 'code', 'system', 'FastClaw', 1),
    ('sys-prompt-meeting-summary', '会议纪要',
        '请根据以下会议记录整理纪要，输出：1) 会议主题；2) 关键讨论点；3) 决议事项；4) 待办任务（含负责人与时间）：\n\n{{transcript}}',
        '生成结构化会议纪要', 'office', 'system', 'FastClaw', 1),
    ('sys-prompt-rewrite-formal', '正式化改写',
        '请把以下文本改写为正式、严谨的书面表达，保持原意：\n\n{{text}}',
        '把口语化文本改成正式文风', 'office', 'system', 'FastClaw', 1),
    ('sys-prompt-brainstorm', '头脑风暴',
        '围绕以下主题做发散性思考，给出至少 10 个不同角度的创意或方案，每条用一句话说明：\n\n{{topic}}',
        '快速产出多角度方案', 'creative', 'system', 'FastClaw', 1);
