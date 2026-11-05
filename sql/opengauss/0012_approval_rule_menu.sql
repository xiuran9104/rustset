-- Approval-rule management UI: an 资产运营 page with infra:approval-rule:*
-- permission buttons (the API landed in 0010).

SELECT setval(
    'system_menu_seq',
    GREATEST(
        (SELECT COALESCE(MAX(id), 1) FROM system_menu),
        (SELECT last_value FROM system_menu_seq)
    ),
    true
);

WITH root AS (
    SELECT id FROM system_menu
    WHERE deleted = 0 AND parent_id = 0 AND path = '/asset-ops'
    ORDER BY id LIMIT 1
)
INSERT INTO system_menu (
    id, name, permission, type, sort, parent_id, path, icon, component,
    component_name, status, visible, keep_alive, always_show,
    creator, updater, deleted
)
SELECT
    nextval('system_menu_seq'), '审批规则', '', 2, 108, root.id,
    'approval-rule', 'lucide:git-pull-request-arrow', 'asset-ops/approval-rule/index',
    'AssetOpsApprovalRule', 0, true, true, true,
    'migration', 'migration', 0
FROM root
WHERE NOT EXISTS (
    SELECT 1 FROM system_menu
    WHERE deleted = 0 AND component = 'asset-ops/approval-rule/index'
);

WITH permission_defs(action, label, sort) AS (
    VALUES
        ('query',  '审批规则查询', 1),
        ('create', '审批规则创建', 2),
        ('update', '审批规则更新', 3),
        ('delete', '审批规则删除', 4)
), page AS (
    SELECT id FROM system_menu
    WHERE deleted = 0 AND component = 'asset-ops/approval-rule/index'
)
INSERT INTO system_menu (
    id, name, permission, type, sort, parent_id, path, icon, component,
    component_name, status, visible, keep_alive, always_show,
    creator, updater, deleted
)
SELECT
    nextval('system_menu_seq'), permission_defs.label,
    'infra:approval-rule:' || permission_defs.action,
    3, permission_defs.sort, page.id, '', '', '', '',
    0, true, true, true, 'migration', 'migration', 0
FROM permission_defs CROSS JOIN page
WHERE NOT EXISTS (
    SELECT 1 FROM system_menu existing
    WHERE existing.deleted = 0
      AND existing.permission = 'infra:approval-rule:' || permission_defs.action
);

SELECT setval(
    'system_role_menu_seq',
    GREATEST(
        (SELECT COALESCE(MAX(id), 1) FROM system_role_menu),
        (SELECT last_value FROM system_role_menu_seq)
    ),
    true
);

WITH rule_menus AS (
    SELECT id FROM system_menu
    WHERE deleted = 0
      AND (component = 'asset-ops/approval-rule/index'
           OR permission LIKE 'infra:approval-rule:%')
), super_roles AS (
    SELECT id, tenant_id FROM system_role
    WHERE deleted = 0 AND status = 0 AND code = 'super_admin'
)
INSERT INTO system_role_menu (
    id, role_id, menu_id, creator, updater, deleted, tenant_id
)
SELECT
    nextval('system_role_menu_seq'), role.id, menu.id,
    'migration', 'migration', 0, role.tenant_id
FROM super_roles role CROSS JOIN rule_menus menu
WHERE NOT EXISTS (
    SELECT 1 FROM system_role_menu existing
    WHERE existing.deleted = 0
      AND existing.role_id = role.id
      AND existing.menu_id = menu.id
);
