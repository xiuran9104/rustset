-- 平台作为 MCP 客户端：登记的外部 MCP 服务器（Streamable HTTP）。
-- 其工具由 /ai/mcp-server/sync 同步进 ai.tools（executor={"kind":"mcp",...}）。
-- 鉴权令牌用 SealingSecrets 封存（enc:sm4:v3:/v2:），接口永不回显。
-- 遵循 ai 模块惯例：bigint epoch id/millis 时间、status=1 启用、硬删。

CREATE TABLE IF NOT EXISTS ai.mcp_server (
    id bigint NOT NULL,
    name text NOT NULL,
    url text NOT NULL,
    -- 鉴权头名（Authorization / X-Api-Key 等）
    auth_header character varying(64) DEFAULT 'Authorization'::character varying NOT NULL,
    -- SealingSecrets 封存密文；空串 = 无鉴权
    auth_token character varying(512) DEFAULT ''::character varying NOT NULL,
    status integer DEFAULT 1 NOT NULL,
    -- 最近一次 initialize 返回（缓存展示用）
    server_info jsonb DEFAULT '{}'::jsonb NOT NULL,
    tool_count integer DEFAULT 0 NOT NULL,
    last_sync_time bigint,
    last_error text DEFAULT ''::text NOT NULL,
    remark text DEFAULT ''::text NOT NULL,
    create_time bigint NOT NULL,
    update_time bigint NOT NULL,
    CONSTRAINT ai_mcp_server_pkey PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_mcp_server_name ON ai.mcp_server(name);
CREATE INDEX IF NOT EXISTS idx_ai_mcp_server_status ON ai.mcp_server(status);

-- 菜单：AI 大模型 → 控制台(2760) 下新增 “MCP 服务器” + 5 个按钮权限
LOCK TABLE public.system_menu IN SHARE ROW EXCLUSIVE MODE;

INSERT INTO public.system_menu
    (id, name, permission, type, sort, parent_id, path, icon, component, component_name,
     status, visible, keep_alive, always_show, creator, updater)
SELECT 30375, 'MCP 服务器', '', 2, 1, 2760, 'mcp-server', 'lucide:plug',
       'ai/model/mcpServer/index.vue', 'AiMcpServer', 0, true, true, true, 'system', 'system'
WHERE NOT EXISTS (SELECT 1 FROM public.system_menu WHERE id = 30375 AND deleted = 0);

INSERT INTO public.system_menu
    (id, name, permission, type, sort, parent_id, path, icon, component, component_name,
     status, visible, keep_alive, always_show, creator, updater)
SELECT m.id, m.name, m.permission, 3, m.sort, 30375, '', '', '', NULL,
       0, true, true, true, 'system', 'system'
FROM (VALUES
    (30376, '服务器查询', 'ai:mcp-server:query', 1),
    (30377, '服务器创建', 'ai:mcp-server:create', 2),
    (30378, '服务器更新', 'ai:mcp-server:update', 3),
    (30379, '服务器删除', 'ai:mcp-server:delete', 4),
    (30380, '工具同步', 'ai:mcp-server:sync', 5)
) AS m(id, name, permission, sort)
WHERE NOT EXISTS (SELECT 1 FROM public.system_menu WHERE id = m.id AND deleted = 0);

-- 超管补授权（页面 + 按钮）
INSERT INTO public.system_role_menu (id, role_id, menu_id, creator, updater, deleted, tenant_id)
SELECT nextval('public.system_role_menu_seq'), r.id, m.menu_id, 'system', 'system', 0, 1
FROM public.system_role r
CROSS JOIN (VALUES (30375), (30376), (30377), (30378), (30379), (30380)) AS m(menu_id)
WHERE r.deleted = 0 AND r.status = 0 AND r.code = 'super_admin'
  AND NOT EXISTS (
      SELECT 1 FROM public.system_role_menu rm
      WHERE rm.role_id = r.id AND rm.menu_id = m.menu_id AND rm.deleted = 0
  );
