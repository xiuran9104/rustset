-- 资产中心排序调整：网络策略排到物理资源之后（按反馈）。
-- 固定目标排序（幂等，重复执行结果一致）。

UPDATE public.system_menu m
   SET sort = s.new_sort, update_time = now(), updater = 'system'
FROM (VALUES
    (30243, 1), -- 发现资产
    (30386, 2), -- 终端设备
    (30252, 3), -- 云资源
    (30345, 4), -- 物理资源
    (30311, 5), -- 网络策略
    (30251, 6), -- 业务应用
    (30254, 7), -- 扫描任务
    (30350, 8), -- 资产核查
    (30255, 9)  -- 风险管理
) AS s(id, new_sort)
 WHERE m.id = s.id AND m.parent_id = 30342 AND m.deleted = 0 AND m.sort <> s.new_sort;
