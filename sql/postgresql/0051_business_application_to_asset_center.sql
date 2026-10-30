-- 业务应用归入资产中心：它是资源归属的归集台账（应用 + IP/端口端点），
-- 云资源的业务项目、工单的应用系统均指向它。业务管理顶级菜单随之退役。
-- 幂等：按菜单 ID 精确更新，组件路径与权限码不变。

-- 资产中心：发现资产(1) 云资源(2) 物理资源(3) 业务应用(4)
--           扫描任务(5) 资产核查(6) 网络策略(7) 风险管理(8)
UPDATE system_menu SET parent_id=30342, sort=4, update_time=now() WHERE id=30251 AND deleted=0;
UPDATE system_menu SET sort=5, update_time=now() WHERE id=30254 AND deleted=0;
UPDATE system_menu SET sort=6, update_time=now() WHERE id=30350 AND deleted=0;
UPDATE system_menu SET sort=7, update_time=now() WHERE id=30311 AND deleted=0;
UPDATE system_menu SET sort=8, update_time=now() WHERE id=30255 AND deleted=0;

-- 业务管理（30343）子项已全部迁出，软删退役
UPDATE system_menu SET deleted=1, update_time=now() WHERE id=30343 AND deleted=0;
