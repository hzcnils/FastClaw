-- FastClaw · 删除编程模式数据表 & 将汇智网供应商代码从 NEWAPI 改为 HUIZHI

-- ============================================
-- Part 1: 删除编程模式（coding）相关数据表
-- 涉及 20260509000001_coding.sql 和 20260509000002_fpc.sql 中创建的表
-- 按外键依赖从子到父的顺序删除，FK 开启状态下安全
-- ============================================
DROP TABLE IF EXISTS fpc_tasks;
DROP TABLE IF EXISTS fpc_sessions;
DROP TABLE IF EXISTS coding_bookmarks;
DROP TABLE IF EXISTS coding_sessions;
DROP TABLE IF EXISTS coding_projects;

-- ============================================
-- Part 2: 将供应商"汇智网"的 provider_code 从 NEWAPI 改为 HUIZHI
-- 策略：先 INSERT 新的 HUIZHI 行（父键先存在），再更新子表，最后 DELETE 旧行
-- ============================================

-- 2.1 将 NEWAPI 供应商复制为新行，provider_code 改为 HUIZHI
INSERT INTO model_providers (id, name, provider_type, base_url, api_key_ref, is_enabled, is_local, sort_order, extra_config, provider_code, source_category, created_at, updated_at)
SELECT
    'openai_compat_huizhi',
    name,
    provider_type,
    base_url,
    api_key_ref,
    is_enabled,
    is_local,
    sort_order,
    extra_config,
    'HUIZHI',
    source_category,
    datetime('now'),
    datetime('now')
FROM model_providers
WHERE provider_code = 'NEWAPI';

-- 2.2 将 models 表从 NEWAPI 迁移到 HUIZHI（此时 HUIZHI 已在父表中，FK 检查通过）
UPDATE models SET
    provider_code = 'HUIZHI',
    id = REPLACE(id, 'NEWAPI-', 'HUIZHI-'),
    extra_config = REPLACE(extra_config, '"source":"NEWAPI"', '"source":"HUIZHI"')
WHERE provider_code = 'NEWAPI';

-- 2.3 更新 model_feedback 表
UPDATE model_feedback SET
    provider_code = 'HUIZHI',
    model_id = REPLACE(model_id, 'NEWAPI-', 'HUIZHI-')
WHERE provider_code = 'NEWAPI'
   OR model_id LIKE 'NEWAPI-%';

-- 2.4 删除旧的 NEWAPI 供应商（子表已迁移，FK CASCADE 不影响任何数据）
DELETE FROM model_providers WHERE provider_code = 'NEWAPI';
