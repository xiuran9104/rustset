-- 产品内流程引擎：定义 / 实例 / 步骤 三张表 + 运营流程菜单 + 通知模板与示例定义。
-- v1 模型：顺序步骤 + 步骤级条件跳过（condition 对 payload 求值，false → skipped）。
-- 与既有资源工单并存：工单可经 create_ticket 自动节点接入流程，v2 再吸收工单审批链。

CREATE SEQUENCE IF NOT EXISTS public.infra_flow_definition_seq
    START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE IF NOT EXISTS public.infra_flow_instance_seq
    START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;
CREATE SEQUENCE IF NOT EXISTS public.infra_flow_step_seq
    START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;

-- 流程定义（模板）：definition jsonb = {"steps":[{"key","name","type","config","condition"}]}
CREATE TABLE IF NOT EXISTS public.infra_flow_definition (
    id bigint DEFAULT nextval('public.infra_flow_definition_seq') NOT NULL,
    name character varying(128) NOT NULL,
    description text DEFAULT ''::text NOT NULL,
    -- 0=启用 1=停用（与 infra_approval_rule 一致）
    status smallint DEFAULT 0 NOT NULL,
    definition jsonb DEFAULT '{"steps":[]}'::jsonb NOT NULL,
    version integer DEFAULT 1 NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint,
    CONSTRAINT infra_flow_definition_pkey PRIMARY KEY (id, tenant_id)
);
CREATE INDEX IF NOT EXISTS idx_infra_flow_definition_tenant
    ON public.infra_flow_definition(tenant_id, id) WHERE deleted=0;

-- 流程实例：启动时快照定义（含版本），payload/context 供条件求值与动作取参
CREATE TABLE IF NOT EXISTS public.infra_flow_instance (
    id bigint DEFAULT nextval('public.infra_flow_instance_seq') NOT NULL,
    definition_id bigint NOT NULL,
    definition_snapshot jsonb DEFAULT '{}'::jsonb NOT NULL,
    title character varying(192) DEFAULT ''::character varying NOT NULL,
    -- 关联业务对象（可空），如 biz_type='ticket'
    biz_type character varying(64) DEFAULT ''::character varying NOT NULL,
    biz_id character varying(64) DEFAULT ''::character varying NOT NULL,
    payload jsonb DEFAULT '{}'::jsonb NOT NULL,
    context jsonb DEFAULT '{}'::jsonb NOT NULL,
    -- running|completed|cancelled|failed
    status character varying(24) DEFAULT 'running' NOT NULL,
    current_index integer DEFAULT 0 NOT NULL,
    starter character varying(64) DEFAULT ''::character varying NOT NULL,
    start_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    finish_time timestamp without time zone,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint,
    CONSTRAINT infra_flow_instance_pkey PRIMARY KEY (id, tenant_id)
);
CREATE INDEX IF NOT EXISTS idx_infra_flow_instance_tenant_status
    ON public.infra_flow_instance(tenant_id, status, id) WHERE deleted=0;

-- 步骤实例：一行 = 定义中一个步骤的运行时状态
CREATE TABLE IF NOT EXISTS public.infra_flow_step (
    id bigint DEFAULT nextval('public.infra_flow_step_seq') NOT NULL,
    instance_id bigint NOT NULL,
    step_key character varying(64) NOT NULL,
    step_index integer NOT NULL,
    name character varying(128) NOT NULL,
    -- approval|manual|auto|notification|ticket_wait
    type character varying(24) NOT NULL,
    -- pending|active|done|skipped|failed|cancelled
    status character varying(24) DEFAULT 'pending' NOT NULL,
    config jsonb DEFAULT '{}'::jsonb NOT NULL,
    result jsonb DEFAULT '{}'::jsonb NOT NULL,
    operator character varying(64) DEFAULT ''::character varying NOT NULL,
    comment text DEFAULT ''::text NOT NULL,
    -- {"role":"super_admin"} 或 {"userIds":[1,2]} 或 {"usernames":["admin"]}
    assignee jsonb DEFAULT '{}'::jsonb NOT NULL,
    timeout_at timestamp without time zone,
    retry_at timestamp without time zone,
    attempt_count integer DEFAULT 0 NOT NULL,
    started_at timestamp without time zone,
    finished_at timestamp without time zone,
    tenant_id bigint,
    CONSTRAINT infra_flow_step_pkey PRIMARY KEY (id, tenant_id)
);
CREATE INDEX IF NOT EXISTS idx_infra_flow_step_instance
    ON public.infra_flow_step(tenant_id, instance_id, step_index);
-- runner 扫描：等待外部推进的 active 步骤（审批超时 / auto 重试 / 工单等待）
CREATE INDEX IF NOT EXISTS idx_infra_flow_step_runner
    ON public.infra_flow_step(tenant_id, retry_at, timeout_at)
    WHERE status='active';

-- 菜单：运营流程(30344) 下新增 流程定义 / 流程实例 + 按钮权限
LOCK TABLE public.system_menu IN SHARE ROW EXCLUSIVE MODE;

INSERT INTO public.system_menu
    (id, name, permission, type, sort, parent_id, path, icon, component, component_name,
     status, visible, keep_alive, always_show, creator, updater)
SELECT m.id, m.name, '', 2, m.sort, 30344, m.path, m.icon, m.component, m.component_name,
       0, true, true, true, 'system', 'system'
FROM (VALUES
    (30362, '流程定义', 5, 'flow-definition', 'lucide:list-checks', 'asset-ops/flow-definition/index', 'FlowDefinition'),
    (30363, '流程实例', 6, 'flow-instance', 'lucide:git-branch', 'asset-ops/flow-instance/index', 'FlowInstance')
) AS m(id, name, sort, path, icon, component, component_name)
WHERE NOT EXISTS (SELECT 1 FROM public.system_menu WHERE id = m.id AND deleted = 0);

INSERT INTO public.system_menu
    (id, name, permission, type, sort, parent_id, path, icon, component, component_name,
     status, visible, keep_alive, always_show, creator, updater)
SELECT m.id, m.name, m.permission, 3, m.sort, m.parent, '', '', '', NULL,
       0, true, true, true, 'system', 'system'
FROM (VALUES
    (30364, '定义查询', 'infra:flow-definition:query', 1, 30362),
    (30365, '定义管理', 'infra:flow-definition:create', 2, 30362),
    (30366, '实例查询', 'infra:flow-instance:query', 1, 30363),
    (30367, '实例发起', 'infra:flow-instance:start', 2, 30363),
    (30368, '实例操作', 'infra:flow-instance:operate', 3, 30363)
) AS m(id, name, permission, sort, parent)
WHERE NOT EXISTS (SELECT 1 FROM public.system_menu WHERE id = m.id AND deleted = 0);

INSERT INTO public.system_role_menu (id, role_id, menu_id, creator, updater, deleted, tenant_id)
SELECT nextval('public.system_role_menu_seq'), r.id, m.menu_id, 'system', 'system', 0, 1
FROM public.system_role r
CROSS JOIN (VALUES (30362),(30363),(30364),(30365),(30366),(30367),(30368)) AS m(menu_id)
WHERE r.deleted = 0 AND r.status = 0 AND r.code = 'super_admin'
  AND NOT EXISTS (
      SELECT 1 FROM public.system_role_menu rm
      WHERE rm.role_id = r.id AND rm.menu_id = m.menu_id AND rm.deleted = 0
  );

-- 通知模板：步骤待办 / 超时升级
INSERT INTO public.system_notify_template
    (id, name, code, nickname, content, type, params, status, remark, creator, updater)
SELECT nextval('public.system_notify_template_seq'), '流程步骤待办', 'flow_step_todo', '流程提醒',
       '流程「{flowName}」的步骤「{stepName}」等待您处理：{title}。请到 运营流程 → 流程实例 处理。',
       2, '["flowName","stepName","title"]', 0, '流程引擎步骤待办通知', 'system', 'system'
WHERE NOT EXISTS (
    SELECT 1 FROM public.system_notify_template
    WHERE code = 'flow_step_todo' AND deleted = 0
);
INSERT INTO public.system_notify_template
    (id, name, code, nickname, content, type, params, status, remark, creator, updater)
SELECT nextval('public.system_notify_template_seq'), '流程步骤超时', 'flow_step_timeout', '流程提醒',
       '流程「{flowName}」的步骤「{stepName}」已超时（实例：{title}），请尽快处理或关注升级结果。',
       2, '["flowName","stepName","title"]', 0, '流程引擎步骤超时通知', 'system', 'system'
WHERE NOT EXISTS (
    SELECT 1 FROM public.system_notify_template
    WHERE code = 'flow_step_timeout' AND deleted = 0
);

-- 示例流程定义（停用状态，供管理员参考复制）：网络开通标准流程
-- 步骤：安全审批（高风险才走二级主管审批）→ 防火墙策略下发(dry_run 可配) → 通知 → 人工确认交付
INSERT INTO public.infra_flow_definition
    (name, description, status, definition, version, creator, updater, tenant_id)
SELECT '网络开通标准流程（示例）', '示例：安全审批 →（高风险时）主管复审 → 防火墙下发 → 完成通知 → 人工确认交付', 1,
       '{
         "steps": [
           {"key":"sec_approve","name":"安全审批","type":"approval",
            "config":{"assignee":{"role":"super_admin"},"timeoutHours":48,"onTimeout":"escalate_notify"},
            "condition":null},
           {"key":"mgr_approve","name":"主管复审（高风险）","type":"approval",
            "config":{"assignee":{"role":"super_admin"},"timeoutHours":24,"onTimeout":"auto_reject"},
            "condition":{"field":"riskLevel","op":"eq","value":"high"}},
           {"key":"fw_push","name":"防火墙策略下发","type":"auto",
            "config":{"action":"firewall_push","params":{"ticketField":"payload"}},
            "condition":null},
           {"key":"notify_done","name":"完成通知","type":"notification",
            "config":{"assignee":{"role":"super_admin"},"templateCode":"flow_step_todo"},
            "condition":null},
           {"key":"confirm","name":"人工确认交付","type":"manual",
            "config":{"assignee":{"role":"super_admin"}},
            "condition":null}
         ]
       }'::jsonb, 1, 'system', 'system', 1
WHERE NOT EXISTS (
    SELECT 1 FROM public.infra_flow_definition
    WHERE name = '网络开通标准流程（示例）' AND tenant_id = 1 AND deleted = 0
);
