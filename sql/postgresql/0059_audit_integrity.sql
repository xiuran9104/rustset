-- 审计完整性保护：SM3 算法标识 + 行摘要（docs/xinchuang-adaptation-plan.md §4.4.3 #6）。
--
-- infra_api_access_log 新增：
--   integrity_alg  固定 'SM3'，显式记录本行完整性算法（GB/T 32905-2016）；
--   record_digest  SM3(canonical row fields) 的十六进制摘要，由网关写入
--                  路径计算。对已有历史行 digest 为 NULL——完整性标识只对
--                  新写入的行生效，不回填（回填无法恢复当时的原始语义）。
--
-- 边界（如实登记）：行级摘要可检测字段篡改；行删除的检测需要哈希链或
-- 外部 WORM 存储，属于密评现场测评的整改范围，不在本迁移内。
-- 列均为可移植子集（varchar + 常量默认值），四种方言可直接翻译。

ALTER TABLE public.infra_api_access_log
    ADD COLUMN IF NOT EXISTS integrity_alg character varying(16) DEFAULT 'SM3';
ALTER TABLE public.infra_api_access_log
    ADD COLUMN IF NOT EXISTS record_digest character varying(64);

COMMENT ON COLUMN public.infra_api_access_log.integrity_alg IS
    '审计行完整性算法标识（SM3，GB/T 32905-2016）';
COMMENT ON COLUMN public.infra_api_access_log.record_digest IS
    'SM3(轨迹/方法/路径/UA/模块/起止时间/耗时/状态码/结果) 十六进制摘要';
