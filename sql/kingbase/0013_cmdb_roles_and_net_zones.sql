-- 0013: CMDB default roles + organization network-zone tree.

-- ============================================================
-- Part A: CMDB default roles. cmdb_admin manages models and
-- attributes; cmdb_user browses and maintains instances.
-- ============================================================
SELECT setval(
    'system_menu_seq',
    GREATEST(
        (SELECT COALESCE(MAX(id), 1) FROM system_menu),
        (SELECT last_value FROM system_menu_seq)
    ),
    true
);

WITH cmdb_pages AS (
    SELECT id FROM system_menu
    WHERE deleted = 0 AND component IN ('cmdb/model/index', 'cmdb/instance/index')
), cmdb_actions AS (
    SELECT id FROM system_menu
    WHERE deleted = 0 AND permission LIKE 'cmdb:%' AND type = 3
)
INSERT INTO system_role (id, name, code, sort, data_scope, status, type, tenant_id, creator, updater)
SELECT
    nextval('system_role_seq'), 'CMDB 管理员', 'cmdb_admin', 20, 1, 0, 1, 0, 'migration', 'migration'
WHERE NOT EXISTS (SELECT 1 FROM system_role WHERE code = 'cmdb_admin' AND deleted = 0);

INSERT INTO system_role (id, name, code, sort, data_scope, status, type, tenant_id, creator, updater)
SELECT
    nextval('system_role_seq'), 'CMDB 用户', 'cmdb_user', 21, 1, 0, 1, 0, 'migration', 'migration'
WHERE NOT EXISTS (SELECT 1 FROM system_role WHERE code = 'cmdb_user' AND deleted = 0);

-- cmdb_admin: everything cmdb:*.
INSERT INTO system_role_menu (id, role_id, menu_id, creator, updater, deleted, tenant_id)
SELECT nextval('system_role_menu_seq'), role.id, menu.id, 'migration', 'migration', 0, role.tenant_id
FROM system_role role
CROSS JOIN (SELECT id FROM system_menu WHERE deleted = 0 AND permission LIKE 'cmdb:%' AND type = 3) menu
CROSS JOIN (SELECT id FROM system_menu WHERE deleted = 0 AND parent_id = 0 AND path = '/cmdb') root
WHERE role.code = 'cmdb_admin' AND role.deleted = 0
  AND NOT EXISTS (
      SELECT 1 FROM system_role_menu existing
      WHERE existing.deleted = 0 AND existing.role_id = role.id AND existing.menu_id = menu.id
  );

-- cmdb_user: query-only on both pages plus instance create/update.
INSERT INTO system_role_menu (id, role_id, menu_id, creator, updater, deleted, tenant_id)
SELECT nextval('system_role_menu_seq'), role.id, menu.id, 'migration', 'migration', 0, role.tenant_id
FROM system_role role
CROSS JOIN (
    SELECT id FROM system_menu WHERE deleted = 0 AND permission IN (
        'cmdb:model:query',
        'cmdb:instance:query', 'cmdb:instance:create', 'cmdb:instance:update'
    ) AND type = 3
) menu
CROSS JOIN (SELECT id FROM system_menu WHERE deleted = 0 AND parent_id = 0 AND path = '/cmdb') root
WHERE role.code = 'cmdb_user' AND role.deleted = 0
  AND NOT EXISTS (
      SELECT 1 FROM system_role_menu existing
      WHERE existing.deleted = 0 AND existing.role_id = role.id AND existing.menu_id = menu.id
  );

-- ============================================================
-- Part B: organization network-zone tree. Companies / subsidiaries /
-- departments carry network segments (CIDR); asset intake matches an
-- IP against the most specific segment to auto-fill ownership.
-- ============================================================
CREATE SEQUENCE IF NOT EXISTS public.cmdb_net_zone_seq
    START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;

CREATE TABLE IF NOT EXISTS public.cmdb_net_zone (
    id bigint DEFAULT nextval('public.cmdb_net_zone_seq'::regclass) NOT NULL,
    name character varying(128) NOT NULL,
    parent_id bigint DEFAULT 0 NOT NULL,
    zone_type character varying(32) DEFAULT 'company'::character varying NOT NULL,
    cidr character varying(64),
    sort integer DEFAULT 0 NOT NULL,
    description character varying(256),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    CONSTRAINT cmdb_net_zone_pkey PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS idx_cmdb_net_zone_parent
    ON public.cmdb_net_zone (parent_id) WHERE deleted = 0;

-- Ownership columns for inventory auto-attribution.
ALTER TABLE public.infra_asset
    ADD COLUMN IF NOT EXISTS net_zone_id bigint,
    ADD COLUMN IF NOT EXISTS ownership_source character varying(32)
        DEFAULT 'manual'::character varying;

COMMENT ON COLUMN public.infra_asset.ownership_source IS
    'manual | segment — how organization_name was assigned';

-- ============================================================
-- Part D: the asset → network-policy jump needs an indexed IP lookup.
-- ============================================================
CREATE INDEX IF NOT EXISTS idx_infra_network_policy_source_ip
    ON public.infra_network_policy (source_ip) WHERE deleted = 0;
CREATE INDEX IF NOT EXISTS idx_infra_network_policy_dest_ip
    ON public.infra_network_policy (destination_ip) WHERE deleted = 0;

-- Menu: 组织网段 under 配置管理.
INSERT INTO system_menu (
    id, name, permission, type, sort, parent_id, path, icon, component,
    component_name, status, visible, keep_alive, always_show,
    creator, updater, deleted
)
WITH root AS (
    SELECT id FROM system_menu
    WHERE deleted = 0 AND parent_id = 0 AND path = '/cmdb'
    ORDER BY id LIMIT 1
)
SELECT
    nextval('system_menu_seq'), '组织网段', '', 2, 3, root.id,
    'net-zone', 'lucide:network', 'cmdb/net-zone/index',
    'CmdbNetZone', 0, true, true, true,
    'migration', 'migration', 0
FROM root
WHERE NOT EXISTS (
    SELECT 1 FROM system_menu
    WHERE deleted = 0 AND component = 'cmdb/net-zone/index'
);

INSERT INTO system_menu (
    id, name, permission, type, sort, parent_id, path, icon, component,
    component_name, status, visible, keep_alive, always_show,
    creator, updater, deleted
)
WITH page AS (
    SELECT id FROM system_menu
    WHERE deleted = 0 AND component = 'cmdb/net-zone/index'
), permission_defs(action, label, sort) AS (
    VALUES ('query', '组织网段查询', 1), ('create', '组织网段创建', 2),
           ('update', '组织网段更新', 3), ('delete', '组织网段删除', 4)
)
SELECT
    nextval('system_menu_seq'), permission_defs.label,
    'cmdb:net-zone:' || permission_defs.action,
    3, permission_defs.sort, page.id, '', '', '', '',
    0, true, true, true, 'migration', 'migration', 0
FROM permission_defs CROSS JOIN page
WHERE NOT EXISTS (
    SELECT 1 FROM system_menu existing
    WHERE existing.deleted = 0
      AND existing.permission = 'cmdb:net-zone:' || permission_defs.action
);

-- Grant the new page + permissions to both CMDB roles and super_admin.
WITH net_zone_menus AS (
    SELECT id FROM system_menu
    WHERE deleted = 0
      AND (component = 'cmdb/net-zone/index'
           OR permission LIKE 'cmdb:net-zone:%')
), grant_roles AS (
    SELECT id, tenant_id FROM system_role
    WHERE deleted = 0 AND status = 0 AND code IN ('super_admin', 'cmdb_admin', 'cmdb_user')
)
INSERT INTO system_role_menu (
    id, role_id, menu_id, creator, updater, deleted, tenant_id
)
SELECT nextval('system_role_menu_seq'), role.id, menu.id, 'migration', 'migration', 0, role.tenant_id
FROM grant_roles role CROSS JOIN net_zone_menus menu
WHERE NOT EXISTS (
    SELECT 1 FROM system_role_menu existing
    WHERE existing.deleted = 0
      AND existing.role_id = role.id AND existing.menu_id = menu.id
);

SELECT setval(
    'system_role_menu_seq',
    GREATEST(
        (SELECT COALESCE(MAX(id), 1) FROM system_role_menu),
        (SELECT last_value FROM system_role_menu_seq)
    ),
    true
);
