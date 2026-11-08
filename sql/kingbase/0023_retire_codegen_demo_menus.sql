-- 0023: retire the upstream codegen demo showcase (代码生成案例 and its
-- demo01/02/03 + testDemo pages). These are yudao template samples outside
-- the RustSet product scope, and 代码生成案例/testDemo had no view file at
-- all, leaving broken entries in the navigation.

WITH RECURSIVE doomed AS (
    SELECT id FROM system_menu
    WHERE deleted = 0 AND parent_id = 2 AND name = '代码生成案例' AND path = 'demo'
    UNION ALL
    SELECT m.id FROM system_menu m
    JOIN doomed d ON m.parent_id = d.id
    WHERE m.deleted = 0
)
UPDATE system_menu SET deleted = 1, updater = 'migration', update_time = now()
WHERE id IN (SELECT id FROM doomed) AND deleted = 0;

-- The grant pass must not filter on menu.deleted: it also cleans up when the
-- menu rows were already soft-deleted by an earlier run of this migration.
WITH RECURSIVE doomed AS (
    SELECT id FROM system_menu
    WHERE parent_id = 2 AND name = '代码生成案例' AND path = 'demo'
    UNION ALL
    SELECT m.id FROM system_menu m
    JOIN doomed d ON m.parent_id = d.id
)
UPDATE system_role_menu g SET deleted = 1, updater = 'migration', update_time = now()
WHERE g.deleted = 0 AND g.menu_id IN (SELECT id FROM doomed);
