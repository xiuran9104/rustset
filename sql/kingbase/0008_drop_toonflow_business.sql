-- Drop the Toonflow media business (短剧工厂): schemas, menus, and role
-- grants. The general AI module (ai schema, /ai/* routes) is unaffected.
-- Decision recorded 2026-09-25; the Dioxus frontend that used these tables
-- was removed in the kairos-web-shell migration.

DROP SCHEMA IF EXISTS toonflow CASCADE;
DROP SCHEMA IF EXISTS toon CASCADE;
DROP SCHEMA IF EXISTS media CASCADE;

-- Remove the 短剧工厂 menu subtree (root path /toonflow, all descendants,
-- and any orphaned toon:* button permissions) plus its role grants.
WITH RECURSIVE toon_menus AS (
    SELECT id
    FROM system_menu
    WHERE deleted = 0 AND parent_id = 0 AND path = '/toonflow'
    UNION ALL
    SELECT m.id
    FROM system_menu m
    JOIN toon_menus t ON m.parent_id = t.id
    WHERE m.deleted = 0
),
toon_menu_ids AS (
    SELECT id FROM toon_menus
    UNION
    SELECT id FROM system_menu WHERE deleted = 0 AND permission LIKE 'toon:%'
)
DELETE FROM system_role_menu
WHERE menu_id IN (SELECT id FROM toon_menu_ids);

WITH RECURSIVE toon_menus AS (
    SELECT id
    FROM system_menu
    WHERE deleted = 0 AND parent_id = 0 AND path = '/toonflow'
    UNION ALL
    SELECT m.id
    FROM system_menu m
    JOIN toon_menus t ON m.parent_id = t.id
    WHERE m.deleted = 0
),
toon_menu_ids AS (
    SELECT id FROM toon_menus
    UNION
    SELECT id FROM system_menu WHERE deleted = 0 AND permission LIKE 'toon:%'
)
DELETE FROM system_menu
WHERE id IN (SELECT id FROM toon_menu_ids);
