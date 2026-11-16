-- 更新 settings 表 language 字段默认值为“简体中文”
-- 去除旧数据中的双引号包裹，统一存储格式
UPDATE settings SET value = '"简体中文"' WHERE key = 'language' ;
