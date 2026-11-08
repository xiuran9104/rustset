-- CMDB core (veops/cmdb-inspired): user-defined models with typed
-- attributes, configuration-item instances stored as JSONB, and CI relations.
-- Design doc: docs/cmdb-roadmap.md.

CREATE SEQUENCE IF NOT EXISTS public.cmdb_model_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE IF NOT EXISTS public.cmdb_attribute_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE IF NOT EXISTS public.cmdb_instance_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE IF NOT EXISTS public.cmdb_relation_seq START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;

CREATE TABLE IF NOT EXISTS public.cmdb_model (
    id bigint DEFAULT nextval('public.cmdb_model_seq'::regclass) NOT NULL,
    name character varying(64) NOT NULL,
    code character varying(64) NOT NULL,
    description character varying(256),
    icon character varying(128),
    unique_key character varying(64),
    sort integer DEFAULT 0 NOT NULL,
    status smallint DEFAULT 0 NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    CONSTRAINT cmdb_model_pkey PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS idx_cmdb_model_code ON public.cmdb_model (code) WHERE deleted = 0;

CREATE TABLE IF NOT EXISTS public.cmdb_attribute (
    id bigint DEFAULT nextval('public.cmdb_attribute_seq'::regclass) NOT NULL,
    model_id bigint NOT NULL,
    name character varying(64) NOT NULL,
    code character varying(64) NOT NULL,
    attr_type character varying(32) NOT NULL,
    required boolean DEFAULT false NOT NULL,
    choices jsonb,
    default_value jsonb,
    show_in_list boolean DEFAULT true NOT NULL,
    sort integer DEFAULT 0 NOT NULL,
    status smallint DEFAULT 0 NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    CONSTRAINT cmdb_attribute_pkey PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS idx_cmdb_attribute_model
    ON public.cmdb_attribute (model_id) WHERE deleted = 0;

CREATE TABLE IF NOT EXISTS public.cmdb_instance (
    id bigint DEFAULT nextval('public.cmdb_instance_seq'::regclass) NOT NULL,
    model_id bigint NOT NULL,
    attributes jsonb DEFAULT '{}'::jsonb NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    CONSTRAINT cmdb_instance_pkey PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS idx_cmdb_instance_model
    ON public.cmdb_instance (model_id) WHERE deleted = 0;
CREATE INDEX IF NOT EXISTS idx_cmdb_instance_attributes
    ON public.cmdb_instance USING gin (attributes jsonb_path_ops);

CREATE TABLE IF NOT EXISTS public.cmdb_relation (
    id bigint DEFAULT nextval('public.cmdb_relation_seq'::regclass) NOT NULL,
    source_id bigint NOT NULL,
    target_id bigint NOT NULL,
    relation character varying(64) DEFAULT 'relates_to'::character varying NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    CONSTRAINT cmdb_relation_pkey PRIMARY KEY (id)
);
CREATE INDEX IF NOT EXISTS idx_cmdb_relation_source
    ON public.cmdb_relation (source_id) WHERE deleted = 0;
CREATE INDEX IF NOT EXISTS idx_cmdb_relation_target
    ON public.cmdb_relation (target_id) WHERE deleted = 0;

-- Menus: 配置管理 directory with 模型管理 and 实例管理 pages and
-- cmdb:* permission buttons, granted to super_admin.
SELECT setval(
    'system_menu_seq',
    GREATEST(
        (SELECT COALESCE(MAX(id), 1) FROM system_menu),
        (SELECT last_value FROM system_menu_seq)
    ),
    true
);

INSERT INTO system_menu (
    id, name, permission, type, sort, parent_id, path, icon, component,
    component_name, status, visible, keep_alive, always_show,
    creator, updater, deleted
)
SELECT
    nextval('system_menu_seq'), '配置管理', '', 1, 30, 0,
    '/cmdb', 'lucide:database', '', '',
    0, true, true, true, 'migration', 'migration', 0
WHERE NOT EXISTS (
    SELECT 1 FROM system_menu
    WHERE deleted = 0 AND parent_id = 0 AND path = '/cmdb'
);

WITH cmdb_root AS (
    SELECT id FROM system_menu
    WHERE deleted = 0 AND parent_id = 0 AND path = '/cmdb'
    ORDER BY id LIMIT 1
), pages(path, name, component, component_name, sort) AS (
    VALUES
        ('model', '模型管理', 'cmdb/model/index', 'CmdbModelManagement', 1),
        ('instance', '实例管理', 'cmdb/instance/index', 'CmdbInstanceManagement', 2)
)
INSERT INTO system_menu (
    id, name, permission, type, sort, parent_id, path, icon, component,
    component_name, status, visible, keep_alive, always_show,
    creator, updater, deleted
)
SELECT
    nextval('system_menu_seq'), pages.name, '', 2, pages.sort, cmdb_root.id,
    pages.path, 'lucide:shapes', pages.component, pages.component_name,
    0, true, true, true, 'migration', 'migration', 0
FROM pages CROSS JOIN cmdb_root
WHERE NOT EXISTS (
    SELECT 1 FROM system_menu
    WHERE deleted = 0 AND component = pages.component
);

WITH permission_defs(page_component, action, label, sort) AS (
    VALUES
        ('cmdb/model/index', 'query',   '模型查询', 1),
        ('cmdb/model/index', 'create',  '模型创建', 2),
        ('cmdb/model/index', 'update',  '模型更新', 3),
        ('cmdb/model/index', 'delete',  '模型删除', 4),
        ('cmdb/model/index', 'attribute:create', '属性创建', 5),
        ('cmdb/model/index', 'attribute:update', '属性更新', 6),
        ('cmdb/model/index', 'attribute:delete', '属性删除', 7),
        ('cmdb/instance/index', 'query',  '实例查询', 1),
        ('cmdb/instance/index', 'create', '实例创建', 2),
        ('cmdb/instance/index', 'update', '实例更新', 3),
        ('cmdb/instance/index', 'delete', '实例删除', 4)
), page AS (
    SELECT id, component FROM system_menu
    WHERE deleted = 0
      AND component IN ('cmdb/model/index', 'cmdb/instance/index')
)
INSERT INTO system_menu (
    id, name, permission, type, sort, parent_id, path, icon, component,
    component_name, status, visible, keep_alive, always_show,
    creator, updater, deleted
)
SELECT
    nextval('system_menu_seq'), permission_defs.label,
    'cmdb:' || permission_defs.action,
    3, permission_defs.sort, page.id, '', '', '', '',
    0, true, true, true, 'migration', 'migration', 0
FROM permission_defs
JOIN page ON page.component = permission_defs.page_component
WHERE NOT EXISTS (
    SELECT 1 FROM system_menu existing
    WHERE existing.deleted = 0
      AND existing.permission = 'cmdb:' || permission_defs.action
);

SELECT setval(
    'system_role_menu_seq',
    GREATEST(
        (SELECT COALESCE(MAX(id), 1) FROM system_role_menu),
        (SELECT last_value FROM system_role_menu_seq)
    ),
    true
);

WITH cmdb_menus AS (
    SELECT m.id
    FROM system_menu m
    WHERE m.deleted = 0
      AND (m.path = '/cmdb' AND m.parent_id = 0
           OR m.component IN ('cmdb/model/index', 'cmdb/instance/index')
           OR m.permission LIKE 'cmdb:%')
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
FROM super_roles role CROSS JOIN cmdb_menus menu
WHERE NOT EXISTS (
    SELECT 1 FROM system_role_menu existing
    WHERE existing.deleted = 0
      AND existing.role_id = role.id
      AND existing.menu_id = menu.id
);
