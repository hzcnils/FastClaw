-- FastClaw v2.0.0 初始化数据库
-- 此文件由 sqlx migrate 自动执行

-- 开启 WAL 模式
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

-- ==================== 用户与系统 ====================

CREATE TABLE IF NOT EXISTS users (
    id              TEXT PRIMARY KEY,
    username        TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL DEFAULT '',
    avatar_path     TEXT,
    role            TEXT NOT NULL DEFAULT 'user',
    password_hash   TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS settings (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL,
    category        TEXT NOT NULL DEFAULT 'general',
    description     TEXT,
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ==================== 模型与供应商 ====================

CREATE TABLE IF NOT EXISTS model_providers (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    provider_type   TEXT NOT NULL,
    base_url        TEXT NOT NULL,
    api_key_ref     TEXT,
    is_enabled      INTEGER NOT NULL DEFAULT 1,
    is_local        INTEGER NOT NULL DEFAULT 0,
    sort_order      INTEGER NOT NULL DEFAULT 0,
    extra_config    TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS models (
    id              TEXT PRIMARY KEY,
    provider_id     TEXT NOT NULL,
    model_id        TEXT NOT NULL,
    display_name    TEXT NOT NULL,
    model_type      TEXT NOT NULL DEFAULT 'chat',
    context_window  INTEGER,
    max_output      INTEGER,
    input_price     REAL,
    output_price    REAL,
    supports_vision INTEGER NOT NULL DEFAULT 0,
    supports_tools  INTEGER NOT NULL DEFAULT 0,
    is_enabled      INTEGER NOT NULL DEFAULT 1,
    is_default      INTEGER NOT NULL DEFAULT 0,
    sort_order      INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (provider_id) REFERENCES model_providers(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_models_provider ON models(provider_id);
CREATE INDEX IF NOT EXISTS idx_models_type ON models(model_type);

-- ==================== 对话与消息 ====================

CREATE TABLE IF NOT EXISTS conversations (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL,
    title           TEXT NOT NULL DEFAULT '新对话',
    model_id        TEXT,
    assistant_id    TEXT,
    mode            TEXT NOT NULL DEFAULT 'normal',
    is_pinned       INTEGER NOT NULL DEFAULT 0,
    is_archived     INTEGER NOT NULL DEFAULT 0,
    summary         TEXT,
    extra_config    TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_conversations_user ON conversations(user_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_conversations_assistant ON conversations(assistant_id);

CREATE TABLE IF NOT EXISTS messages (
    id              TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    role            TEXT NOT NULL,
    content         TEXT NOT NULL,
    model_id        TEXT,
    parent_id       TEXT,
    token_input     INTEGER,
    token_output    INTEGER,
    duration_ms     INTEGER,
    status          TEXT NOT NULL DEFAULT 'completed',
    error_message   TEXT,
    extra_data      TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_messages_conversation ON messages(conversation_id, created_at);
CREATE INDEX IF NOT EXISTS idx_messages_parent ON messages(parent_id);

CREATE TABLE IF NOT EXISTS message_attachments (
    id              TEXT PRIMARY KEY,
    message_id      TEXT NOT NULL,
    file_name       TEXT NOT NULL,
    file_path       TEXT NOT NULL,
    file_size       INTEGER NOT NULL,
    mime_type       TEXT NOT NULL,
    file_type       TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_attachments_message ON message_attachments(message_id);

-- ==================== 知识库 ====================

CREATE TABLE IF NOT EXISTS kb_collections (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL,
    name            TEXT NOT NULL,
    description     TEXT,
    embedding_model TEXT NOT NULL DEFAULT 'bge-m3',
    chunk_strategy  TEXT NOT NULL DEFAULT 'semantic',
    chunk_size      INTEGER NOT NULL DEFAULT 512,
    chunk_overlap   INTEGER NOT NULL DEFAULT 50,
    vector_collection_id TEXT,
    doc_count       INTEGER NOT NULL DEFAULT 0,
    chunk_count     INTEGER NOT NULL DEFAULT 0,
    is_enabled      INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_kb_collections_user ON kb_collections(user_id);

CREATE TABLE IF NOT EXISTS kb_documents (
    id              TEXT PRIMARY KEY,
    collection_id   TEXT NOT NULL,
    file_name       TEXT NOT NULL,
    file_path       TEXT NOT NULL,
    file_size       INTEGER NOT NULL,
    file_type       TEXT NOT NULL,
    source_url      TEXT,
    char_count      INTEGER,
    chunk_count     INTEGER NOT NULL DEFAULT 0,
    qa_count        INTEGER NOT NULL DEFAULT 0,
    status          TEXT NOT NULL DEFAULT 'pending',
    error_message   TEXT,
    extra_meta      TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (collection_id) REFERENCES kb_collections(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_kb_documents_collection ON kb_documents(collection_id);
CREATE INDEX IF NOT EXISTS idx_kb_documents_status ON kb_documents(status);

CREATE TABLE IF NOT EXISTS kb_chunks (
    id              TEXT PRIMARY KEY,
    document_id     TEXT NOT NULL,
    collection_id   TEXT NOT NULL,
    content         TEXT NOT NULL,
    chunk_index     INTEGER NOT NULL,
    token_count     INTEGER,
    source_page     INTEGER,
    source_section  TEXT,
    extra_meta      TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (document_id) REFERENCES kb_documents(id) ON DELETE CASCADE,
    FOREIGN KEY (collection_id) REFERENCES kb_collections(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_kb_chunks_document ON kb_chunks(document_id, chunk_index);
CREATE INDEX IF NOT EXISTS idx_kb_chunks_collection ON kb_chunks(collection_id);

CREATE TABLE IF NOT EXISTS kb_qa_pairs (
    id              TEXT PRIMARY KEY,
    document_id     TEXT NOT NULL,
    collection_id   TEXT NOT NULL,
    question        TEXT NOT NULL,
    answer          TEXT NOT NULL,
    source_chunk_id TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (document_id) REFERENCES kb_documents(id) ON DELETE CASCADE,
    FOREIGN KEY (collection_id) REFERENCES kb_collections(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_kb_qa_collection ON kb_qa_pairs(collection_id);

CREATE TABLE IF NOT EXISTS conversation_kb_links (
    id              TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    collection_id   TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
    FOREIGN KEY (collection_id) REFERENCES kb_collections(id) ON DELETE CASCADE,
    UNIQUE(conversation_id, collection_id)
);

-- ==================== 三层记忆 ====================

CREATE TABLE IF NOT EXISTS memory_episodic (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL,
    conversation_id TEXT,
    content         TEXT NOT NULL,
    importance      REAL NOT NULL DEFAULT 0.5,
    access_count    INTEGER NOT NULL DEFAULT 0,
    last_accessed   TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_memory_episodic_user ON memory_episodic(user_id, importance DESC);

CREATE TABLE IF NOT EXISTS memory_semantic (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL,
    entity_type     TEXT NOT NULL,
    subject         TEXT NOT NULL,
    predicate       TEXT NOT NULL,
    object          TEXT NOT NULL,
    confidence      REAL NOT NULL DEFAULT 0.8,
    source          TEXT,
    source_id       TEXT,
    is_active       INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_memory_semantic_user ON memory_semantic(user_id, entity_type);
CREATE INDEX IF NOT EXISTS idx_memory_semantic_subject ON memory_semantic(subject);

CREATE TABLE IF NOT EXISTS memory_procedural (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL,
    action_type     TEXT NOT NULL,
    action_name     TEXT NOT NULL,
    action_params   TEXT,
    frequency       INTEGER NOT NULL DEFAULT 1,
    last_used       TEXT NOT NULL DEFAULT (datetime('now')),
    success_rate    REAL NOT NULL DEFAULT 1.0,
    avg_duration_ms INTEGER,
    context_summary TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_memory_procedural_user ON memory_procedural(user_id, frequency DESC);
CREATE INDEX IF NOT EXISTS idx_memory_procedural_type ON memory_procedural(action_type);

-- ==================== AI 助手 ====================

CREATE TABLE IF NOT EXISTS assistants (
    id              TEXT PRIMARY KEY,
    user_id         TEXT,
    name            TEXT NOT NULL,
    description     TEXT,
    avatar          TEXT,
    system_prompt   TEXT NOT NULL,
    model_id        TEXT,
    category        TEXT NOT NULL DEFAULT 'general',
    temperature     REAL,
    top_p           REAL,
    max_tokens      INTEGER,
    is_builtin      INTEGER NOT NULL DEFAULT 0,
    is_pinned       INTEGER NOT NULL DEFAULT 0,
    use_count       INTEGER NOT NULL DEFAULT 0,
    sort_order      INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_assistants_category ON assistants(category);
CREATE INDEX IF NOT EXISTS idx_assistants_user ON assistants(user_id);

CREATE TABLE IF NOT EXISTS assistant_skill_links (
    id              TEXT PRIMARY KEY,
    assistant_id    TEXT NOT NULL,
    skill_id        TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (assistant_id) REFERENCES assistants(id) ON DELETE CASCADE,
    FOREIGN KEY (skill_id) REFERENCES skills(id) ON DELETE CASCADE,
    UNIQUE(assistant_id, skill_id)
);

CREATE TABLE IF NOT EXISTS assistant_mcp_links (
    id              TEXT PRIMARY KEY,
    assistant_id    TEXT NOT NULL,
    mcp_server_id   TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (assistant_id) REFERENCES assistants(id) ON DELETE CASCADE,
    FOREIGN KEY (mcp_server_id) REFERENCES mcp_servers(id) ON DELETE CASCADE,
    UNIQUE(assistant_id, mcp_server_id)
);

-- ==================== Skills 与 MCP ====================

CREATE TABLE IF NOT EXISTS skills (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL,
    description     TEXT,
    version         TEXT NOT NULL DEFAULT '1.0.0',
    author          TEXT,
    source          TEXT NOT NULL DEFAULT 'builtin',
    executor_type   TEXT NOT NULL DEFAULT 'deno',
    entry_point     TEXT,
    icon            TEXT,
    category        TEXT NOT NULL DEFAULT 'general',
    is_enabled      INTEGER NOT NULL DEFAULT 1,
    config_schema   TEXT,
    config_values   TEXT,
    install_path    TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_skills_category ON skills(category);

CREATE TABLE IF NOT EXISTS mcp_servers (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    description     TEXT,
    server_type     TEXT NOT NULL DEFAULT 'stdio',
    command         TEXT,
    url             TEXT,
    args            TEXT,
    env_vars        TEXT,
    is_enabled      INTEGER NOT NULL DEFAULT 1,
    status          TEXT NOT NULL DEFAULT 'disconnected',
    tools_cache     TEXT,
    source          TEXT NOT NULL DEFAULT 'manual',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ==================== FastPlan 规划任务 ====================

CREATE TABLE IF NOT EXISTS plan_tasks (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL,
    conversation_id TEXT,
    user_intent     TEXT NOT NULL,
    dag_json        TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'planning',
    total_nodes     INTEGER NOT NULL DEFAULT 0,
    completed_nodes INTEGER NOT NULL DEFAULT 0,
    current_node_id TEXT,
    result_summary  TEXT,
    error_message   TEXT,
    started_at      TEXT,
    completed_at    TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_plan_tasks_user ON plan_tasks(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_plan_tasks_status ON plan_tasks(status);

CREATE TABLE IF NOT EXISTS plan_node_logs (
    id              TEXT PRIMARY KEY,
    task_id         TEXT NOT NULL,
    node_id         TEXT NOT NULL,
    node_type       TEXT NOT NULL,
    node_label      TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending',
    input_data      TEXT,
    output_data     TEXT,
    agent_model     TEXT,
    tools_used      TEXT,
    token_used      INTEGER,
    duration_ms     INTEGER,
    error_message   TEXT,
    user_feedback   TEXT,
    started_at      TEXT,
    completed_at    TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_plan_node_logs_task ON plan_node_logs(task_id);

-- ==================== 费用追踪 ====================

CREATE TABLE IF NOT EXISTS token_usage_logs (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL,
    conversation_id TEXT,
    message_id      TEXT,
    provider_id     TEXT NOT NULL,
    model_id        TEXT NOT NULL,
    token_input     INTEGER NOT NULL DEFAULT 0,
    token_output    INTEGER NOT NULL DEFAULT 0,
    cost_usd        REAL NOT NULL DEFAULT 0.0,
    call_type       TEXT NOT NULL DEFAULT 'chat',
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_token_usage_user_date ON token_usage_logs(user_id, created_at);
CREATE INDEX IF NOT EXISTS idx_token_usage_model ON token_usage_logs(model_id);

CREATE TABLE IF NOT EXISTS cost_budgets (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL,
    budget_type     TEXT NOT NULL,
    limit_usd       REAL NOT NULL,
    action_on_limit TEXT NOT NULL DEFAULT 'warn',
    downgrade_model TEXT,
    is_enabled      INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_cost_budgets_user ON cost_budgets(user_id);

-- ==================== 浏览器剪藏 ====================

CREATE TABLE IF NOT EXISTS web_clips (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL,
    collection_id   TEXT,
    title           TEXT NOT NULL,
    url             TEXT NOT NULL,
    content         TEXT NOT NULL,
    summary         TEXT,
    tags            TEXT,
    is_synced       INTEGER NOT NULL DEFAULT 0,
    synced_doc_id   TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_web_clips_user ON web_clips(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_web_clips_sync ON web_clips(is_synced);

-- ==================== 审计日志 ====================

CREATE TABLE IF NOT EXISTS audit_logs (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL,
    action          TEXT NOT NULL,
    direction       TEXT NOT NULL,
    target          TEXT NOT NULL,
    data_type       TEXT,
    data_size       INTEGER,
    detail          TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_audit_logs_user ON audit_logs(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_logs_direction ON audit_logs(direction);

-- ==================== 动态菜单 ====================

CREATE TABLE IF NOT EXISTS custom_menus (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL,
    parent_id       TEXT,
    title           TEXT NOT NULL,
    icon            TEXT,
    menu_type       TEXT NOT NULL DEFAULT 'url',
    target_url      TEXT,
    html_content    TEXT,
    skill_id        TEXT,
    sort_order      INTEGER NOT NULL DEFAULT 0,
    is_visible      INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_custom_menus_user ON custom_menus(user_id, sort_order);

-- ==================== 多渠道 ====================

CREATE TABLE IF NOT EXISTS channels (
    id              TEXT PRIMARY KEY,
    channel_type    TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL,
    is_enabled      INTEGER NOT NULL DEFAULT 0,
    config          TEXT,
    status          TEXT NOT NULL DEFAULT 'disconnected',
    last_active     TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ==================== Gateway 状态 ====================

CREATE TABLE IF NOT EXISTS gateway_status_logs (
    id              TEXT PRIMARY KEY,
    event_type      TEXT NOT NULL,
    pid             INTEGER,
    uptime_seconds  INTEGER,
    crash_reason    TEXT,
    error_detail    TEXT,
    cpu_percent     REAL,
    memory_mb       REAL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_gateway_status_time ON gateway_status_logs(created_at DESC);

-- ==================== WSP 本地微服务管理 ====================

CREATE TABLE IF NOT EXISTS wsp_services (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL,
    description     TEXT,
    version         TEXT NOT NULL DEFAULT '1.0.0',
    category        TEXT NOT NULL DEFAULT 'general',
    exe_path        TEXT NOT NULL,
    work_dir        TEXT,
    args            TEXT,
    env_vars        TEXT,
    protocol        TEXT NOT NULL DEFAULT 'http',
    host            TEXT NOT NULL DEFAULT '127.0.0.1',
    port            INTEGER NOT NULL,
    base_path       TEXT NOT NULL DEFAULT '/',
    ws_path         TEXT,
    health_endpoint TEXT NOT NULL DEFAULT '/health',
    cpu_limit       REAL,
    memory_limit_mb INTEGER,
    gpu_required    INTEGER NOT NULL DEFAULT 0,
    gpu_device_id   INTEGER,
    auto_start      INTEGER NOT NULL DEFAULT 0,
    start_delay_ms  INTEGER NOT NULL DEFAULT 0,
    start_priority  INTEGER NOT NULL DEFAULT 50,
    restart_policy  TEXT NOT NULL DEFAULT 'on_failure',
    max_restarts    INTEGER NOT NULL DEFAULT 3,
    restart_cooldown_ms INTEGER NOT NULL DEFAULT 5000,
    health_check_interval_ms INTEGER NOT NULL DEFAULT 10000,
    health_check_timeout_ms  INTEGER NOT NULL DEFAULT 3000,
    startup_timeout_ms       INTEGER NOT NULL DEFAULT 30000,
    depends_on      TEXT,
    is_enabled      INTEGER NOT NULL DEFAULT 1,
    is_installed    INTEGER NOT NULL DEFAULT 1,
    sort_order      INTEGER NOT NULL DEFAULT 0,
    extra_config    TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_wsp_services_category ON wsp_services(category);
CREATE INDEX IF NOT EXISTS idx_wsp_services_priority ON wsp_services(auto_start DESC, start_priority);

CREATE TABLE IF NOT EXISTS wsp_runtime_status (
    service_id      TEXT PRIMARY KEY,
    pid             INTEGER,
    status          TEXT NOT NULL DEFAULT 'stopped',
    cpu_percent     REAL,
    memory_mb       REAL,
    gpu_memory_mb   REAL,
    request_count   INTEGER NOT NULL DEFAULT 0,
    error_count     INTEGER NOT NULL DEFAULT 0,
    avg_latency_ms  REAL,
    restart_count   INTEGER NOT NULL DEFAULT 0,
    last_health_check TEXT,
    last_health_ok  INTEGER NOT NULL DEFAULT 0,
    started_at      TEXT,
    stopped_at      TEXT,
    error_message   TEXT,
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (service_id) REFERENCES wsp_services(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS wsp_event_logs (
    id              TEXT PRIMARY KEY,
    service_id      TEXT NOT NULL,
    event_type      TEXT NOT NULL,
    pid             INTEGER,
    exit_code       INTEGER,
    crash_reason    TEXT,
    cpu_percent     REAL,
    memory_mb       REAL,
    duration_ms     INTEGER,
    detail          TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (service_id) REFERENCES wsp_services(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_wsp_events_service ON wsp_event_logs(service_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_wsp_events_type ON wsp_event_logs(event_type);

CREATE TABLE IF NOT EXISTS wsp_endpoints (
    id              TEXT PRIMARY KEY,
    service_id      TEXT NOT NULL,
    method          TEXT NOT NULL DEFAULT 'POST',
    path            TEXT NOT NULL,
    display_name    TEXT NOT NULL,
    description     TEXT,
    request_schema  TEXT,
    response_schema TEXT,
    is_streaming    INTEGER NOT NULL DEFAULT 0,
    timeout_ms      INTEGER NOT NULL DEFAULT 30000,
    category        TEXT,
    sort_order      INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (service_id) REFERENCES wsp_services(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_wsp_endpoints_service ON wsp_endpoints(service_id);

CREATE TABLE IF NOT EXISTS wsp_call_logs (
    id              TEXT PRIMARY KEY,
    service_id      TEXT NOT NULL,
    endpoint_id     TEXT,
    user_id         TEXT NOT NULL,
    conversation_id TEXT,
    method          TEXT NOT NULL,
    path            TEXT NOT NULL,
    status_code     INTEGER,
    request_size    INTEGER,
    response_size   INTEGER,
    latency_ms      INTEGER,
    is_success      INTEGER NOT NULL DEFAULT 1,
    error_message   TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (service_id) REFERENCES wsp_services(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_wsp_calls_service ON wsp_call_logs(service_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_wsp_calls_user ON wsp_call_logs(user_id, created_at DESC);

-- ==================== 初始数据 ====================

INSERT OR IGNORE INTO users (id, username, display_name, role)
VALUES ('default-user', 'default', 'FastClaw 用户', 'admin');

INSERT OR IGNORE INTO settings (key, value, category, description) VALUES
    ('theme',           '"system"',     'general',   '主题：light / dark / system / transparent'),
    ('language',        '"zh-CN"',      'general',   '界面语言'),
    ('font_size',       '14',           'general',   '字体大小（px）'),
    ('auto_start',      'false',        'general',   '开机自启动'),
    ('global_shortcut', '"Alt+Space"',  'shortcut',  '全局唤醒快捷键'),
    ('send_shortcut',   '"Enter"',      'shortcut',  '发送消息快捷键'),
    ('default_model',   'null',         'model',     '默认对话模型 ID'),
    ('temperature',     '0.7',          'model',     '默认 Temperature'),
    ('top_p',           '0.9',          'model',     '默认 TopP'),
    ('max_tokens',      '4096',         'model',     '默认 MaxTokens'),
    ('smart_routing',   'false',        'model',     '智能模型路由开关'),
    ('chunk_strategy',  '"semantic"',   'knowledge', '默认切片策略'),
    ('chunk_size',      '512',          'knowledge', '默认切片大小'),
    ('chunk_overlap',   '50',           'knowledge', '默认切片重叠'),
    ('embedding_model', '"bge-m3"',     'knowledge', '默认 Embedding 模型'),
    ('search_top_k',    '5',            'knowledge', '默认检索 Top-K'),
    ('search_engine',   '"tavily"',     'search',    '联网搜索引擎'),
    ('search_results',  '5',            'search',    '搜索结果数量'),
    ('webdav_url',      'null',         'backup',    'WebDAV 备份地址'),
    ('webdav_encrypt',  'true',         'backup',    'WebDAV 备份加密开关'),
    ('local_only_mode', 'false',        'general',   '完全本地化模式开关');

INSERT OR IGNORE INTO channels (id, channel_type, display_name, is_enabled, status)
VALUES ('ch-webchat', 'webchat', 'WebChat 网页对话', 1, 'connected');

-- WSP 动态菜单默认数据
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-asr-offline', 'default-user', NULL, '离线语音识别', '离线语音识别', 'iframe', '/WebResource/view/audioRecognition/asr/index.html', '{"category":"asr"}', NULL, 1, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-asr-dictation', 'default-user', NULL, '语音听写', '语音听写', 'iframe', '/WebResource/view/audioRecognition/audioWrite/index.html', '{"category":"asr"}', NULL, 2, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-asr-meeting', 'default-user', NULL, '会议纪要整理', '会议纪要整理', 'iframe', '/WebResource/view/audioRecognition/meetingDisposal/index.html', '{"category":"asr"}', NULL, 3, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-asr-tts', 'default-user', NULL, '语音合成', '语音合成', 'iframe', '/WebResource/view/audioRecognition/tts/index.html', '{"category":"asr"}', NULL, 4, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-asr-universal-file', 'default-user', NULL, '通用ASR文件', '通用ASR文件', 'iframe', '/WebResource/view/audioRecognition/univeralASRFile/index.html', '{"category":"asr"}', NULL, 5, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-asr-universal-stream', 'default-user', NULL, '通用ASR流式', '通用ASR流式', 'iframe', '/WebResource/view/audioRecognition/univeralASRStreaming/index.html', '{"category":"asr"}', NULL, 6, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-asr-universal-tts', 'default-user', NULL, '通用TTS', '通用TTS', 'iframe', '/WebResource/view/audioRecognition/univeralTTS/index.html', '{"category":"asr"}', NULL, 7, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-ocr-general', 'default-user', NULL, '通用文字识别', '通用文字识别', 'iframe', '/WebResource/view/textRecognition/ocr/index.html', '{"category":"ocr"}', NULL, 1, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-ocr-handwriting', 'default-user', NULL, '手写体识别', '手写体识别', 'iframe', '/WebResource/view/textRecognition/handwriting/index.html', '{"category":"ocr"}', NULL, 2, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-ocr-table', 'default-user', NULL, '表格结构识别', '表格结构识别', 'iframe', '/WebResource/view/textRecognition/tableStructure/index.html', '{"category":"ocr"}', NULL, 3, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-ocr-receipt', 'default-user', NULL, '票据识别', '票据识别', 'iframe', '/WebResource/view/textRecognition/bill/index.html', '{"category":"ocr"}', NULL, 4, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-ocr-plate', 'default-user', NULL, '车牌识别', '车牌识别', 'iframe', '/WebResource/view/textRecognition/licensePlate/index.html', '{"category":"ocr"}', NULL, 5, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-ocr-multilingual', 'default-user', NULL, '多语言识别', '多语言识别', 'iframe', '/WebResource/view/textRecognition/multilingual/index.html', '{"category":"ocr"}', NULL, 6, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-ocr-universal', 'default-user', NULL, '通用OCR', '通用OCR', 'iframe', '/WebResource/view/textRecognition/univeralOCR/index.html', '{"category":"ocr"}', NULL, 7, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-img-understand', 'default-user', NULL, '图像理解', '图像理解', 'iframe', '/WebResource/view/imageRecognition/imageComprehend/index.html', '{"category":"image"}', NULL, 1, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-img-all-recognition', 'default-user', NULL, '万物识别', '万物识别', 'iframe', '/WebResource/view/imageRecognition/allRecognition/index.html', '{"category":"image"}', NULL, 2, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-img-portrait-cutout', 'default-user', NULL, '人像抠图', '人像抠图', 'iframe', '/WebResource/view/imageRecognition/persionRecognition/index.html', '{"category":"image"}', NULL, 3, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-img-face', 'default-user', NULL, '人脸检测', '人脸检测', 'iframe', '/WebResource/view/imageRecognition/faceRecognition/index.html', '{"category":"image"}', NULL, 5, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-img-gesture', 'default-user', NULL, '手势识别', '手势识别', 'iframe', '/WebResource/view/imageRecognition/gestureRecognition/index.html', '{"category":"image"}', NULL, 6, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-img-detection', 'default-user', NULL, '图像检测', '图像检测', 'iframe', '/WebResource/view/imageRecognition/digitalPersionTesting/index.html', '{"category":"image"}', NULL, 7, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-img-matting', 'default-user', NULL, '图像抠图', '图像抠图', 'iframe', '/WebResource/view/imageRecognition/imageMatting/index.html', '{"category":"image"}', NULL, 8, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-img-old-photo', 'default-user', NULL, '照片上色', '老照片上色', 'iframe', '/WebResource/view/imageRecognition/colorOldPhotos/index.html', '{"category":"image"}', NULL, 9, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-img-card', 'default-user', NULL, '证件识别', '证件识别', 'iframe', '/WebResource/view/imageRecognition/cardRec/index.html', '{"category":"image"}', NULL, 10, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-video-summary', 'default-user', NULL, '视频摘要', '视频摘要', 'iframe', '/WebResource/view/videoAnalyse/videoSchema/index.html', '{"category":"video"}', NULL, 1, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-video-extract-audio', 'default-user', NULL, '视频抽取音频', '音频提取', 'iframe', '/WebResource/view/videoAnalyse/videoOutputaudio/index.html', '{"category":"video"}', NULL, 2, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-video-tracking', 'default-user', NULL, '视频跟踪', '视频跟踪', 'iframe', '/WebResource/view/videoAnalyse/videoTailAfter/index.html', '{"category":"video"}', NULL, 3, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-text-correction', 'default-user', NULL, '文本纠错', '文本纠错', 'iframe', '/WebResource/view/textMining/textErrorHandling/index.html', '{"category":"text"}', NULL, 1, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-text-comparison', 'default-user', NULL, '文本对比', '文本对比', 'iframe', '/WebResource/view/textMining/textContrast/index.html', '{"category":"text"}', NULL, 2, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-translate-text', 'default-user', NULL, '文本翻译', '文本翻译', 'iframe', '/WebResource/view/multiLanguageTranslate/textTranslate/index.html', '{"category":"translate"}', NULL, 1, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-translate-doc', 'default-user', NULL, '文档翻译', '文档翻译', 'iframe', '/WebResource/view/multiLanguageTranslate/docmentTranslate/index.html', '{"category":"translate"}', NULL, 2, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-translate-image', 'default-user', NULL, '图片翻译', '图片翻译', 'iframe', '/WebResource/view/multiLanguageTranslate/imageTranslate/index.html', '{"category":"translate"}', NULL, 3, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-translate-audio', 'default-user', NULL, '音频翻译', '音频翻译', 'iframe', '/WebResource/view/multiLanguageTranslate/aaudioTranslate/index.html', '{"category":"translate"}', NULL, 4, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-doc-bid-deviation', 'default-user', NULL, '标书偏离表检查', '偏离表检查', 'iframe', '/WebResource/view/documentOutput/tenderDeviationinspect/index.html', '{"category":"document"}', NULL, 2, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
INSERT OR IGNORE INTO custom_menus (id, user_id, parent_id, title, icon, menu_type, target_url, html_content, skill_id, sort_order, is_visible, created_at, updated_at) VALUES ('wsp-doc-pdf-to-word', 'default-user', NULL, 'PDF转Word', 'PDF转Word', 'iframe', '/WebResource/view/documentOutput/pdfRecognition/index.html', '{"category":"document"}', NULL, 3, 1, '2026-03-31 10:22:15', '2026-03-31 10:22:15');
