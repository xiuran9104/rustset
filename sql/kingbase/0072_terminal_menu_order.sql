-- 终端设备菜单排序调整：排在「发现资产」之后（0070 曾置顶，按反馈调整）。
-- 固定资产中心目标排序（幂等，重复执行结果一致）。

UPDATE public.system_menu m
   SET sort = s.new_sort, update_time = now(), updater = 'system'
FROM (VALUES
    (30243, 1), -- 发现资产
    (30386, 2), -- 终端设备
    (30252, 3), -- 云资源
    (30345, 4), -- 物理资源
    (30251, 5), -- 业务应用
    (30254, 6), -- 扫描任务
    (30350, 7), -- 资产核查
    (30311, 8), -- 网络策略
    (30255, 9)  -- 风险管理
) AS s(id, new_sort)
 WHERE m.id = s.id AND m.parent_id = 30342 AND m.deleted = 0 AND m.sort <> s.new_sort;
