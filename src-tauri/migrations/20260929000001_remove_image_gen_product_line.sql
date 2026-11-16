-- 生图产品线下线：清理「图像生成 / 以图生图」相关的数据。
--
-- 配套改动：种子迁移 20260703163000_seed_aiartmirror_image_models.sql 已被**删除**
-- （迁移文件一旦落库就不可改，见 src/db/mod.rs 的 checksum 校验；删文件是安全的，
-- 那里有「清除已应用但文件已不存在的孤立迁移记录」的兜底）。
-- 因此：干净库不会再写入这些厂商/模型，已应用过种子迁移的老库靠本迁移清掉。

-- 1) 生图耗时统计表：只写不读回业务，产品线已删，直接 DROP（索引随表一起删）
DROP TABLE IF EXISTS image_gen_timing;

-- 2) aiartmirror 供应商与其 4 个生图模型
--    （gpt-image-2 / seedream-4.5 / sdxl / midjourney）
DELETE FROM models WHERE provider_code = 'AIARTMIRROR';
DELETE FROM model_providers WHERE provider_code = 'AIARTMIRROR';

-- 3) 兜底：任何历史遗留的 model_type='image' 行
DELETE FROM models WHERE model_type = 'image';
