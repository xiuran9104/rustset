-- 0014: repair the 0009 CMDB permission codes. The original seed built
-- 'cmdb:' || action, producing truncated codes (cmdb:query, cmdb:create…)
-- instead of the cmdb:model:* / cmdb:instance:* codes the API enforces.

-- 1) Model-page buttons: prefix the truncated codes with cmdb:model:.
UPDATE system_menu m
SET permission = 'cmdb:model:' || split_part(m.permission, ':', 2)
FROM system_menu page
WHERE page.deleted = 0 AND page.component = 'cmdb/model/index'
  AND m.parent_id = page.id AND m.deleted = 0 AND m.type = 3
  AND m.permission IN ('cmdb:query', 'cmdb:create', 'cmdb:update', 'cmdb:delete');

-- 2) Instance-page buttons likewise.
UPDATE system_menu m
SET permission = 'cmdb:instance:' || split_part(m.permission, ':', 2)
FROM system_menu page
WHERE page.deleted = 0 AND page.component = 'cmdb/instance/index'
  AND m.parent_id = page.id AND m.deleted = 0 AND m.type = 3
  AND m.permission IN ('cmdb:query', 'cmdb:create', 'cmdb:update', 'cmdb:delete');

-- 3) Re-bind the cmdb_user role to its intended codes.
DELETE FROM system_role_menu
WHERE deleted = 0
  AND role_id IN (SELECT id FROM system_role WHERE code = 'cmdb_user' AND deleted = 0)
  AND menu_id IN (
      SELECT id FROM system_menu WHERE deleted = 0 AND permission LIKE 'cmdb:%'
  );

INSERT INTO system_role_menu (id, role_id, menu_id, creator, updater, deleted, tenant_id)
SELECT nextval('system_role_menu_seq'), role.id, menu.id, 'migration', 'migration', 0, role.tenant_id
FROM system_role role
CROSS JOIN (
    SELECT id FROM system_menu
    WHERE deleted = 0 AND type = 3 AND permission IN (
        'cmdb:model:query',
        'cmdb:instance:query', 'cmdb:instance:create', 'cmdb:instance:update'
    )
) menu
WHERE role.code = 'cmdb_user' AND role.deleted = 0
  AND NOT EXISTS (
      SELECT 1 FROM system_role_menu existing
      WHERE existing.deleted = 0 AND existing.role_id = role.id AND existing.menu_id = menu.id
  );

-- 4) cmdb_admin keeps every cmdb:* code (the corrected full codes included).
INSERT INTO system_role_menu (id, role_id, menu_id, creator, updater, deleted, tenant_id)
SELECT nextval('system_role_menu_seq'), role.id, menu.id, 'migration', 'migration', 0, role.tenant_id
FROM system_role role
CROSS JOIN (
    SELECT id FROM system_menu WHERE deleted = 0 AND permission LIKE 'cmdb:%' AND type = 3
) menu
WHERE role.code = 'cmdb_admin' AND role.deleted = 0
  AND NOT EXISTS (
      SELECT 1 FROM system_role_menu existing
      WHERE existing.deleted = 0 AND existing.role_id = role.id AND existing.menu_id = menu.id
  );

SELECT setval(
    'system_role_menu_seq',
    GREATEST(
        (SELECT COALESCE(MAX(id), 1) FROM system_role_menu),
        (SELECT last_value FROM system_role_menu_seq)
    ),
    true
);
