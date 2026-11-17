-- AI Agent 预设：可复用的 AI 代理配置（模型+提示词+工具+知识库），
-- 流程画布的 agent 节点引用预设 ID，不在节点内重复配置。

CREATE SEQUENCE IF NOT EXISTS public.ai_agent_preset_seq
    START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;

CREATE TABLE IF NOT EXISTS public.ai_agent_preset (
    id bigint DEFAULT nextval('public.ai_agent_preset_seq') NOT NULL,
    name character varying(128) NOT NULL,
    description text DEFAULT ''::text NOT NULL,
    -- 关联 ai.model_configs.id
    model_id bigint NOT NULL,
    system_prompt text DEFAULT ''::text NOT NULL,
    -- 绑定的工具 ID 数组（ai.tools.id）
    tool_ids jsonb DEFAULT '[]'::jsonb NOT NULL,
    -- 绑定的知识库 ID 数组（ai.knowledge_bases.id）
    knowledge_ids jsonb DEFAULT '[]'::jsonb NOT NULL,
    temperature double precision DEFAULT 0.7,
    max_tokens integer DEFAULT 4096,
    -- 0=启用 1=停用
    status smallint DEFAULT 0 NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint,
    CONSTRAINT ai_agent_preset_pkey PRIMARY KEY (id, tenant_id)
);

CREATE INDEX IF NOT EXISTS idx_ai_agent_preset_tenant
    ON public.ai_agent_preset(tenant_id, id) WHERE deleted=0;

-- 菜单：AI 大模型 → 控制台 → AI Agent 管理
LOCK TABLE public.system_menu IN SHARE ROW EXCLUSIVE MODE;

INSERT INTO public.system_menu
    (id, name, permission, type, sort, parent_id, path, icon, component, component_name,
     status, visible, keep_alive, always_show, creator, updater)
SELECT 30382, 'AI Agent 管理', '', 2, 0, 2760, 'agent-preset', 'lucide:bot',
       'ai/model/agent-preset/index', 'AiAgentPreset', 0, true, true, true, 'system', 'system'
WHERE NOT EXISTS (SELECT 1 FROM public.system_menu WHERE id = 30382 AND deleted = 0);

INSERT INTO public.system_menu
    (id, name, permission, type, sort, parent_id, path, icon, component, component_name,
     status, visible, keep_alive, always_show, creator, updater)
SELECT m.id, m.name, m.permission, 3, m.sort, 30382, '', '', '', NULL,
       0, true, true, true, 'system', 'system'
FROM (VALUES
    (30383, 'Agent查询', 'ai:agent-preset:query', 1),
    (30384, 'Agent管理', 'ai:agent-preset:create', 2)
) AS m(id, name, permission, sort)
WHERE NOT EXISTS (SELECT 1 FROM public.system_menu WHERE id = m.id AND deleted = 0);

INSERT INTO public.system_role_menu (id, role_id, menu_id, creator, updater, deleted, tenant_id)
SELECT nextval('public.system_role_menu_seq'), r.id, m.menu_id, 'system', 'system', 0, 1
FROM public.system_role r
CROSS JOIN (VALUES (30382),(30383),(30384)) AS m(menu_id)
WHERE r.deleted = 0 AND r.status = 0 AND r.code = 'super_admin'
  AND NOT EXISTS (
      SELECT 1 FROM public.system_role_menu rm
      WHERE rm.role_id = r.id AND rm.menu_id = m.menu_id AND rm.deleted = 0
  );
