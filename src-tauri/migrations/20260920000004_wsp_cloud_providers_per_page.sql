-- 云服务厂商配置改为「按页面（二级分类）独立」
-- 原表 provider_code 为 UNIQUE，全站每个厂商仅一行，导致通用文字识别页会共用（读到手写体识别页的）
-- AccessKeyId 等凭据。现改为 UNIQUE(provider_code, sub_category_name)，各页面的凭据 / 服务地址互不影响。
-- 表主键 id 统一为 cloud-{provider_code}-{sub_category_name}，与 wsp_services.id 保持一致。

CREATE TABLE IF NOT EXISTS wsp_cloud_providers_new (
    id                  TEXT PRIMARY KEY,
    provider_code       TEXT NOT NULL,
    name                TEXT NOT NULL,
    display_name        TEXT NOT NULL,
    access_key_id       TEXT,
    access_key_secret   TEXT,
    api_key             TEXT,
    endpoint            TEXT NOT NULL DEFAULT '',
    region              TEXT,
    category            TEXT NOT NULL DEFAULT 'hr',
    category_name       TEXT NOT NULL DEFAULT '文字识别',
    sub_category_name   TEXT NOT NULL DEFAULT '',
    is_enabled          INTEGER NOT NULL DEFAULT 0,
    extra_config        TEXT,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at          TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (provider_code, sub_category_name)
);

INSERT OR IGNORE INTO wsp_cloud_providers_new
    (id, provider_code, name, display_name, access_key_id, access_key_secret, api_key,
     endpoint, region, category, category_name, sub_category_name, is_enabled, extra_config,
     created_at, updated_at)
SELECT
    'cloud-' || provider_code || '-' || sub_category_name,
    provider_code, name, display_name, access_key_id, access_key_secret, api_key,
    endpoint, region, category, category_name, sub_category_name, is_enabled, extra_config,
    created_at, updated_at
FROM wsp_cloud_providers
WHERE sub_category_name <> '';

DROP TABLE wsp_cloud_providers;

ALTER TABLE wsp_cloud_providers_new RENAME TO wsp_cloud_providers;

CREATE INDEX IF NOT EXISTS idx_wsp_cloud_providers_code ON wsp_cloud_providers(provider_code);
CREATE INDEX IF NOT EXISTS idx_wsp_cloud_providers_page ON wsp_cloud_providers(sub_category_name);
