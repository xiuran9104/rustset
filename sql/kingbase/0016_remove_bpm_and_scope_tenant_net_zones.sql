-- 0016: remove the unsupported BPM surface and move network-zone management
-- into the system tenant page.

-- Network zones are tenant-owned. Existing rows belong to the first active
-- tenant so upgrades keep their current data.
ALTER TABLE public.cmdb_net_zone
    ADD COLUMN IF NOT EXISTS tenant_id bigint;

UPDATE public.cmdb_net_zone
SET tenant_id = COALESCE(
    (SELECT MIN(id) FROM public.system_tenant WHERE deleted = 0),
    1
)
WHERE tenant_id IS NULL;

ALTER TABLE public.cmdb_net_zone
    ALTER COLUMN tenant_id SET DEFAULT 1,
    ALTER COLUMN tenant_id SET NOT NULL;

CREATE INDEX IF NOT EXISTS idx_cmdb_net_zone_tenant_parent
    ON public.cmdb_net_zone (tenant_id, parent_id)
    WHERE deleted = 0;

-- The permissions remain usable from the tenant page; only the standalone
-- CMDB page is retired.
WITH tenant_page AS (
    SELECT id FROM public.system_menu
    WHERE deleted = 0 AND component = 'system/tenant/index'
    ORDER BY id LIMIT 1
), net_zone_page AS (
    SELECT id FROM public.system_menu
    WHERE deleted = 0 AND component = 'cmdb/net-zone/index'
)
UPDATE public.system_menu permission_menu
SET parent_id = tenant_page.id,
    updater = 'migration',
    update_time = now()
FROM tenant_page, net_zone_page
WHERE permission_menu.parent_id = net_zone_page.id
  AND permission_menu.permission LIKE 'cmdb:net-zone:%'
  AND permission_menu.deleted = 0;

UPDATE public.system_menu
SET deleted = 1, updater = 'migration', update_time = now()
WHERE deleted = 0 AND component = 'cmdb/net-zone/index';

-- BPM has frontend-only legacy pages and no Rust service. Remove the complete
-- menu/permission subtree and all grants instead of presenting dead entries.
WITH RECURSIVE bpm_menu AS (
    SELECT id
    FROM public.system_menu
    WHERE deleted = 0
      AND (path = '/bpm' OR permission LIKE 'bpm:%')
    UNION
    SELECT child.id
    FROM public.system_menu child
    JOIN bpm_menu parent ON child.parent_id = parent.id
    WHERE child.deleted = 0
), removed_grants AS (
    UPDATE public.system_role_menu role_menu
    SET deleted = 1, updater = 'migration', update_time = now()
    WHERE role_menu.deleted = 0
      AND role_menu.menu_id IN (SELECT id FROM bpm_menu)
    RETURNING role_menu.id
)
UPDATE public.system_menu menu
SET deleted = 1, updater = 'migration', update_time = now()
WHERE menu.id IN (SELECT id FROM bpm_menu)
  AND menu.deleted = 0;
