-- 云服务厂商模型改为「按页面（二级分类）」区分
-- 原方案把二级分类写死为「手写体识别」，其它页面保存的云模型无法在本页下拉中回显；
-- 现将 wsp_services.id 改为 cloud-{厂商代码}-{二级分类名}，保存时按当前页面生成。
-- 清理旧的 handwriting 专用行及其运行时状态。

DELETE FROM wsp_runtime_status WHERE service_id LIKE 'cloud-%-handwriting';
DELETE FROM wsp_services WHERE id LIKE 'cloud-%-handwriting';
