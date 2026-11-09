-- 对外 MCP 服务：API Key 表 + 管理菜单
-- 密钥明文 mcp_ + 43 位 base62 仅在创建响应中出现一次；库里只存 SHA-256 hex 与前缀。

CREATE SEQUENCE IF NOT EXISTS public.system_mcp_api_key_seq
    START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;

CREATE TABLE IF NOT EXISTS public.system_mcp_api_key (
    id bigint DEFAULT nextval('public.system_mcp_api_key_seq') NOT NULL,
    name character varying(64) NOT NULL,
    -- SHA-256 hex（64 字符）
    api_key_hash character varying(64) NOT NULL,
    -- 明文前缀（如 mcp_a1b2c3d4），仅用于列表识别
    key_prefix character varying(16) NOT NULL,
    -- 绑定的系统用户（system_users.id）；权限与租户隔离继承该用户
    user_id bigint NOT NULL,
    enabled smallint DEFAULT 1 NOT NULL,
    expires_time timestamp without time zone,
    last_used_time timestamp without time zone,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint,
    CONSTRAINT system_mcp_api_key_pkey PRIMARY KEY (id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_system_mcp_api_key_hash
    ON public.system_mcp_api_key(api_key_hash) WHERE deleted=0;
CREATE INDEX IF NOT EXISTS idx_system_mcp_api_key_tenant
    ON public.system_mcp_api_key(tenant_id, id);

-- 菜单：基础设置(30240) 下新增 “MCP 服务” 页面 + 4 个按钮权限
LOCK TABLE public.system_menu IN SHARE ROW EXCLUSIVE MODE;

INSERT INTO public.system_menu
    (id, name, permission, type, sort, parent_id, path, icon, component, component_name,
     status, visible, keep_alive, always_show, creator, updater)
SELECT 30370, 'MCP 服务', '', 2, 8, 30240, '/system/mcp-key', 'lucide:plug',
       'system/mcp/key/index', 'SystemMcpKey', 0, true, true, true, 'system', 'system'
WHERE NOT EXISTS (SELECT 1 FROM public.system_menu WHERE id = 30370 AND deleted = 0);

INSERT INTO public.system_menu
    (id, name, permission, type, sort, parent_id, path, icon, component, component_name,
     status, visible, keep_alive, always_show, creator, updater)
SELECT m.id, m.name, m.permission, 3, m.sort, 30370, '', '', '', NULL,
       0, true, true, true, 'system', 'system'
FROM (VALUES
    (30371, '密钥查询', 'system:mcp-key:query', 1),
    (30372, '密钥创建', 'system:mcp-key:create', 2),
    (30373, '密钥更新', 'system:mcp-key:update', 3),
    (30374, '密钥删除', 'system:mcp-key:delete', 4)
) AS m(id, name, permission, sort)
WHERE NOT EXISTS (SELECT 1 FROM public.system_menu WHERE id = m.id AND deleted = 0);

-- 超管补授权（页面 + 按钮）
INSERT INTO public.system_role_menu (id, role_id, menu_id, creator, updater, deleted, tenant_id)
SELECT nextval('public.system_role_menu_seq'), r.id, m.menu_id, 'system', 'system', 0, 1
FROM public.system_role r
CROSS JOIN (VALUES (30370), (30371), (30372), (30373), (30374)) AS m(menu_id)
WHERE r.deleted = 0 AND r.status = 0 AND r.code = 'super_admin'
  AND NOT EXISTS (
      SELECT 1 FROM public.system_role_menu rm
      WHERE rm.role_id = r.id AND rm.menu_id = m.menu_id AND rm.deleted = 0
  );
