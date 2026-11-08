-- 菜单信息架构调整：按"资源供给 → 资源台账 → 运营流程"主线重排。
-- 供给链（服务商/资源区/机房/平台/凭据）整体归入基础设施；
-- 资源台账（云资源/物理资源）归入资产中心；云管中心空壳退役。
-- 幂等：按菜单 ID 精确更新，重复执行无副作用；组件路径与权限码不变，
-- 角色-菜单按 ID 关联自动跟随。

-- 基础设施：服务商(1) 云资源区(2) 物理机房(3) 云平台(4) 云厂商对接(5) 安全产品(6) 网络区域(7)
UPDATE system_menu SET sort=1, update_time=now() WHERE id=30244 AND deleted=0;
UPDATE system_menu SET parent_id=30340, sort=2, update_time=now() WHERE id=30246 AND deleted=0;
UPDATE system_menu SET sort=3, update_time=now() WHERE id=30245 AND deleted=0;
UPDATE system_menu SET parent_id=30340, sort=4, update_time=now() WHERE id=30247 AND deleted=0;
UPDATE system_menu SET parent_id=30340, sort=5, update_time=now() WHERE id=30248 AND deleted=0;
UPDATE system_menu SET sort=6, update_time=now() WHERE id=30250 AND deleted=0;
UPDATE system_menu SET sort=7, update_time=now() WHERE id=30249 AND deleted=0;

-- 云管中心（30341）子项已全部迁出，软删退役
UPDATE system_menu SET deleted=1, update_time=now() WHERE id=30341 AND deleted=0;

-- 资产中心：发现资产(1) 云资源(2) 物理资源(3) 扫描任务(4) 资产核查(5) 网络策略(6) 风险管理(7)
UPDATE system_menu SET name='发现资产', sort=1, update_time=now() WHERE id=30243 AND deleted=0;
UPDATE system_menu SET parent_id=30342, sort=2, update_time=now() WHERE id=30252 AND deleted=0;
UPDATE system_menu SET parent_id=30342, sort=3, update_time=now() WHERE id=30345 AND deleted=0;
UPDATE system_menu SET sort=4, update_time=now() WHERE id=30254 AND deleted=0;
UPDATE system_menu SET sort=5, update_time=now() WHERE id=30350 AND deleted=0;
UPDATE system_menu SET sort=6, update_time=now() WHERE id=30311 AND deleted=0;
UPDATE system_menu SET sort=7, update_time=now() WHERE id=30255 AND deleted=0;
