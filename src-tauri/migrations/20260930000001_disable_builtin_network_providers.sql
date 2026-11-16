-- 公开版停用汇智网、私域网、本电脑，并清空其中保存的服务地址。
UPDATE model_providers
SET base_url = '',
    api_key_ref = '',
    extra_config = NULL,
    is_enabled = 0
WHERE provider_code IN ('HUIZHI', 'PRIVATE', 'LOCAL', 'NEWAPI');

UPDATE models
SET is_enabled = 0
WHERE provider_code IN ('HUIZHI', 'PRIVATE', 'LOCAL', 'NEWAPI');
