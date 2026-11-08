-- 运营流程子菜单排序：网络策略工单排在审批规则之前
-- 资源工单(1) → 网络策略工单(2) → 审批规则(3)
UPDATE system_menu SET sort=2, update_time=now() WHERE id=30360 AND deleted=0;
UPDATE system_menu SET sort=3, update_time=now() WHERE id=30330 AND deleted=0;
