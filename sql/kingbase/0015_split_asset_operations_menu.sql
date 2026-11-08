-- 0015: split the single 资产运营 directory into five top-level modules,
-- positioned right after 仪表盘 (where 系统功能 used to sit), ordered by
-- function: 配置管理 → 基础设施 → 云管中心 → 资产中心 → 业务管理 → 运营流程.
-- Components (Vue files) and API routes are unchanged; only the menu tree
-- moves, so role grants by menu id stay valid.

SELECT setval(
    'system_menu_seq',
    GREATEST(
        (SELECT COALESCE(MAX(id), 1) FROM system_menu),
        (SELECT last_value FROM system_menu_seq)
    ),
    true
);

-- 1) Five new top-level directories.
INSERT INTO system_menu (
    id, name, permission, type, sort, parent_id, path, icon, component,
    component_name, status, visible, keep_alive, always_show,
    creator, updater, deleted
)
SELECT nextval('system_menu_seq'), dir.name, '', 1, dir.sort, 0,
       dir.path, dir.icon, '', '',
       0, true, true, true, 'migration', 'migration', 0
FROM (VALUES
    ('/infra-center', '基础设施', 11, 'lucide:server-cog'),
    ('/cloud-center', '云管中心', 12, 'lucide:cloud-cog'),
    ('/asset-center', '资产中心', 13, 'lucide:boxes'),
    ('/biz-center',   '业务管理', 14, 'lucide:briefcase'),
    ('/ops-center',   '运营流程', 15, 'lucide:workflow')
) AS dir(path, name, sort, icon)
WHERE NOT EXISTS (
    SELECT 1 FROM system_menu
    WHERE deleted = 0 AND parent_id = 0 AND path = dir.path
);

-- 配置管理 joins the front group as the first module.
UPDATE system_menu SET sort = 10 WHERE deleted = 0 AND parent_id = 0 AND path = '/cmdb';

-- Converge directory sorts even when this file re-runs over an already-
-- split database (the idempotent INSERT above would skip existing rows).
UPDATE system_menu dir
SET sort = v.sort, updater = 'migration', update_time = now()
FROM (VALUES
    ('/infra-center', 11), ('/cloud-center', 12), ('/asset-center', 13),
    ('/biz-center', 14), ('/ops-center', 15)
) AS v(path, sort)
WHERE dir.deleted = 0 AND dir.parent_id = 0 AND dir.path = v.path;

-- 2) Reparent pages by component (paths stay relative; full URLs change)
--    and give each page a clean functional sort within its module.
WITH dirs AS (
    SELECT path, id FROM system_menu
    WHERE deleted = 0 AND parent_id = 0 AND path IN
        ('/asset-center','/cloud-center','/infra-center','/biz-center','/ops-center')
), moves(component, dir_path, page_sort) AS (
    VALUES
        ('asset-ops/asset/index',               '/asset-center', 1),
        ('asset-ops/network-policy/index',      '/asset-center', 2),
        ('asset-ops/task/index',                '/asset-center', 3),
        ('asset-ops/risk/index',                '/asset-center', 4),
        ('asset-ops/cloud-platform/platform',   '/cloud-center', 1),
        ('asset-ops/cloud-platform/zone',       '/cloud-center', 2),
        ('asset-ops/cloud-provider-config/index','/cloud-center', 3),
        ('asset-ops/service-provider/index',    '/infra-center', 1),
        ('asset-ops/machine-room/index',        '/infra-center', 2),
        ('asset-ops/zone/index',                '/infra-center', 3),
        ('asset-ops/security-product/index',    '/infra-center', 4),
        ('asset-ops/business-application/index','/biz-center', 1),
        ('asset-ops/business-resource/index',   '/biz-center', 2),
        ('asset-ops/resource-ticket/index',     '/ops-center', 1),
        ('asset-ops/approval-rule/index',       '/ops-center', 2)
)
UPDATE system_menu page
SET parent_id = dirs.id, sort = moves.page_sort,
    updater = 'migration', update_time = now()
FROM moves JOIN dirs ON dirs.path = moves.dir_path
WHERE page.deleted = 0 AND page.type = 2
  AND page.component = moves.component;

-- 3) Retire the old 资产运营 root.
UPDATE system_menu
SET deleted = 1, updater = 'migration', update_time = now()
WHERE deleted = 0 AND parent_id = 0 AND path = '/asset-ops';

-- 4) Shift the yudao group one slot down so the modules sit where
--    系统功能 used to be.
UPDATE system_menu SET sort = 20 WHERE deleted = 0 AND parent_id = 0 AND path = '/system';
UPDATE system_menu SET sort = 21 WHERE deleted = 0 AND parent_id = 0 AND path = '/system/basic';
UPDATE system_menu SET sort = 22 WHERE deleted = 0 AND parent_id = 0 AND path = '/system/message';
UPDATE system_menu SET sort = 25 WHERE deleted = 0 AND parent_id = 0 AND path = '/infra';

-- 5) Grant the new directories to every role that could see the old root,
--    so existing page grants remain reachable.
INSERT INTO system_role_menu (
    id, role_id, menu_id, creator, updater, deleted, tenant_id
)
SELECT nextval('system_role_menu_seq'), grant_role.role_id, dir.id,
       'migration', 'migration', 0, grant_role.tenant_id
FROM system_menu old_root
JOIN system_role_menu grant_role
  ON grant_role.menu_id = old_root.id AND grant_role.deleted = 0
CROSS JOIN system_menu dir
WHERE old_root.path = '/asset-ops'
  AND dir.deleted = 0 AND dir.parent_id = 0
  AND dir.path IN ('/asset-center','/cloud-center','/infra-center','/biz-center','/ops-center')
  AND NOT EXISTS (
      SELECT 1 FROM system_role_menu existing
      WHERE existing.deleted = 0
        AND existing.role_id = grant_role.role_id
        AND existing.menu_id = dir.id
  );

-- super_admin always sees the new modules.
INSERT INTO system_role_menu (
    id, role_id, menu_id, creator, updater, deleted, tenant_id
)
SELECT nextval('system_role_menu_seq'), role.id, dir.id, 'migration', 'migration', 0, role.tenant_id
FROM system_role role
CROSS JOIN system_menu dir
WHERE role.code = 'super_admin' AND role.deleted = 0 AND role.status = 0
  AND dir.deleted = 0 AND dir.parent_id = 0
  AND dir.path IN ('/asset-center','/cloud-center','/infra-center','/biz-center','/ops-center')
  AND NOT EXISTS (
      SELECT 1 FROM system_role_menu existing
      WHERE existing.deleted = 0
        AND existing.role_id = role.id AND existing.menu_id = dir.id
  );

SELECT setval(
    'system_role_menu_seq',
    GREATEST(
        (SELECT COALESCE(MAX(id), 1) FROM system_role_menu),
        (SELECT last_value FROM system_role_menu_seq)
    ),
    true
);
