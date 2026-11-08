-- Backend authorization deliberately lets super_admin bypass individual
-- permission checks, but the Vben v-access directive consumes the explicit
-- permission list. Link the migrated menu tree to every enabled super_admin
-- role so frontend and backend semantics stay aligned.

SELECT setval(
    'system_role_menu_seq',
    GREATEST(
        (SELECT COALESCE(MAX(id), 1) FROM system_role_menu),
        (SELECT last_value FROM system_role_menu_seq)
    ),
    true
);

WITH asset_menus AS (
    SELECT root.id
    FROM system_menu root
    WHERE root.deleted = 0 AND root.path = '/asset-ops' AND root.parent_id = 0
    UNION
    SELECT child.id
    FROM system_menu child
    JOIN system_menu root ON root.id = child.parent_id
    WHERE root.deleted = 0 AND root.path = '/asset-ops' AND root.parent_id = 0
      AND child.deleted = 0
    UNION
    SELECT action.id
    FROM system_menu action
    JOIN system_menu page ON page.id = action.parent_id
    JOIN system_menu root ON root.id = page.parent_id
    WHERE root.deleted = 0 AND root.path = '/asset-ops' AND root.parent_id = 0
      AND page.deleted = 0 AND action.deleted = 0
), super_roles AS (
    SELECT id, tenant_id
    FROM system_role
    WHERE deleted = 0 AND status = 0 AND code = 'super_admin'
)
INSERT INTO system_role_menu (
    id, role_id, menu_id, creator, updater, deleted, tenant_id
)
SELECT
    nextval('system_role_menu_seq'), role.id, menu.id,
    'migration', 'migration', 0, role.tenant_id
FROM super_roles role
CROSS JOIN asset_menus menu
WHERE NOT EXISTS (
    SELECT 1
    FROM system_role_menu existing
    WHERE existing.deleted = 0
      AND existing.role_id = role.id
      AND existing.menu_id = menu.id
);
