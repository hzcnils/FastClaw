-- FastClaw · WSP 云服务厂商配置（文字识别 - 手写体识别）
-- 「对接云服务厂商」弹窗保存的阿里云 / 讯飞云凭据，供 WSP 页面调用云端手写体识别接口

CREATE TABLE IF NOT EXISTS wsp_cloud_providers (
    id                  TEXT PRIMARY KEY,                     -- 固定：cloud-{provider_code}-handwriting
    provider_code       TEXT NOT NULL UNIQUE,                 -- aliyun / xfyun
    name                TEXT NOT NULL,                        -- 厂商名称：阿里云 / 讯飞云
    display_name        TEXT NOT NULL,                        -- 模型显示名：阿里云-手写体识别
    access_key_id       TEXT,                                 -- 阿里云 AccessKeyId / 讯飞 AppId
    access_key_secret   TEXT,                                 -- 阿里云 AccessKeySecret / 讯飞 APISecret
    api_key             TEXT,                                 -- 讯飞 APIKey
    endpoint            TEXT NOT NULL DEFAULT '',             -- 服务地址
    region              TEXT,                                 -- 阿里云地域（cn-hangzhou 等）
    category            TEXT NOT NULL DEFAULT 'hr',           -- WSP 细分类别
    category_name       TEXT NOT NULL DEFAULT '文字识别',       -- WSP 一级分类
    sub_category_name   TEXT NOT NULL DEFAULT '手写体识别',     -- WSP 二级分类
    is_enabled          INTEGER NOT NULL DEFAULT 0,           -- 是否启用（保存后同步到 wsp_services）
    extra_config        TEXT,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at          TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_wsp_cloud_providers_code ON wsp_cloud_providers(provider_code);

INSERT OR IGNORE INTO wsp_cloud_providers
    (id, provider_code, name, display_name, access_key_id, access_key_secret, api_key, endpoint, region, category, category_name, sub_category_name, is_enabled)
VALUES
    ('cloud-aliyun-handwriting', 'aliyun', '阿里云', '阿里云-手写体识别', '', '', NULL,
     'https://ocr-api.cn-hangzhou.aliyuncs.com', 'cn-hangzhou', 'hr', '文字识别', '手写体识别', 0),
    ('cloud-xfyun-handwriting', 'xfyun', '讯飞云', '讯飞云-手写体识别', '', '', '',
     'https://webapi.xfyun.cn/v1/service/v1/ocr/handwriting', NULL, 'hr', '文字识别', '手写体识别', 0);
