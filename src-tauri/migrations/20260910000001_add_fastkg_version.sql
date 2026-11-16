-- 记录当前本地已安装的 FastKG 版本号（仅在安装包校验成功后更新）
INSERT OR IGNORE INTO settings (key, value, category, description)
VALUES ('fastkg_version', '', 'general', '当前本地已安装的 FastKG 版本号');
