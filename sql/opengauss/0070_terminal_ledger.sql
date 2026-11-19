-- 终端台账：办公网IP申报完成的终端（MAC/人员/分配IP）落资产中心。
-- 申报流程末尾由 register_terminal 自动动作入账（幂等：同 MAC upsert），
-- 分配 IP 从 assign_ip 步骤备注中提取。资产中心新增「终端设备」页面。

CREATE SEQUENCE IF NOT EXISTS public.infra_terminal_seq
    START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;

CREATE TABLE IF NOT EXISTS public.infra_terminal (
    id bigint DEFAULT nextval('public.infra_terminal_seq'::regclass) NOT NULL,
    mac_address character varying(64) NOT NULL,
    hostname character varying(128),
    device_type character varying(32),
    ip_address character varying(64),
    network_zone character varying(64),
    access_period character varying(32),
    applicant_name character varying(64),
    department character varying(128),
    employee_id character varying(64),
    phone character varying(32),
    reason text,
    flow_instance_id bigint,
    status character varying(32) DEFAULT 'active' NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint,
    CONSTRAINT infra_terminal_pkey PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_infra_terminal_tenant
    ON public.infra_terminal(tenant_id, mac_address) WHERE deleted = 0;

-- 0066 办公网IP申报流程追加「终端入账」步骤（交付确认后、通知前）。
-- 幂等：定义已含 register_terminal 时跳过（JSON 整串覆盖，与种子同步演进）。
UPDATE public.infra_flow_definition
   SET definition = '{
         "steps": [
           {"key":"fill_context","name":"登记申报信息","type":"auto",
            "config":{"action":"set_context",
                      "params":{"key":"supervisor_username","fromPayloadField":"supervisorUsername"}}},
           {"key":"dept_approve","name":"部门主管审批","type":"approval",
            "config":{"assignee":{"role":"super_admin"},
                      "fromContext":"supervisor_username",
                      "timeoutHours":48,"onTimeout":"escalate_notify"}},
           {"key":"assign_ip","name":"配置-分配IP","type":"manual",
            "config":{"assignee":{"role":"super_admin"},
                      "timeoutHours":72,"onTimeout":"escalate_notify"}},
           {"key":"confirm_open","name":"交付-开通确认","type":"manual",
            "config":{"assignee":{"role":"super_admin"}}},
           {"key":"register_terminal","name":"终端入账","type":"auto",
            "config":{"action":"register_terminal","params":{}}},
           {"key":"notify_result","name":"申报结果通知","type":"notification",
            "config":{"assignee":{"role":"super_admin"},
                      "templateCode":"office_ip_apply_notice",
                      "params":{
                        "applicantName":"{{payload.applicantName}}",
                        "department":"{{payload.department}}",
                        "macAddress":"{{payload.macAddress}}",
                        "requestedIp":"{{payload.requestedIp}}"
                      }}}
         ]
       }'::jsonb,
       description = '办公网终端 IP 申报：登记 → 主管动态审批（fromContext）→ 配置（分配IP）→ 开通交付确认 → 终端自动入账（资产中心-终端设备）→ 申报结果通知',
       version = version + 1, update_time = now(), updater = 'system'
 WHERE name = '办公网IP申报流程' AND tenant_id = 1 AND deleted = 0
   AND NOT (definition -> 'steps' @> '[{"key":"register_terminal"}]'::jsonb);

-- 资产中心新增「终端设备」页面（排在云资源前，员工资产高频查看）+ 查询按钮。
INSERT INTO public.system_menu
    (id, name, permission, type, sort, parent_id, path, icon, component, component_name,
     status, visible, keep_alive, always_show, creator, updater)
SELECT m.id, m.name, m.permission, m.type, m.sort, 30342, m.path, m.icon, m.component, m.component_name,
       0, true, true, true, 'system', 'system'
FROM (VALUES
    (30386, '终端设备', '', 2, 1, 'terminal', 'lucide:monitor-smartphone', 'asset-ops/terminal/index', 'TerminalLedger'),
    (30387, '终端查询', 'infra:terminal:query', 3, 1, '', '', '', NULL)
) AS m(id, name, permission, type, sort, path, icon, component, component_name)
WHERE NOT EXISTS (SELECT 1 FROM public.system_menu WHERE id = m.id AND deleted = 0);
-- 页面 sort=1 需让位：既有资产中心子菜单固定目标排序（幂等，重复执行结果一致）。
UPDATE public.system_menu m
   SET sort = s.new_sort, update_time = now()
FROM (VALUES (30243, 2), (30252, 3), (30345, 4), (30251, 5),
             (30254, 6), (30350, 7), (30311, 8), (30255, 9)) AS s(id, new_sort)
 WHERE m.id = s.id AND m.parent_id = 30342 AND m.deleted = 0 AND m.sort <> s.new_sort;

INSERT INTO public.system_role_menu (id, role_id, menu_id, creator, updater, deleted, tenant_id)
SELECT nextval('public.system_role_menu_seq'), r.id, m.menu_id, 'system', 'system', 0, 1
FROM public.system_role r
CROSS JOIN (VALUES (30386),(30387)) AS m(menu_id)
WHERE r.deleted = 0 AND r.status = 0 AND r.code = 'super_admin'
  AND NOT EXISTS (
      SELECT 1 FROM public.system_role_menu
      WHERE role_id = r.id AND menu_id = m.menu_id AND deleted = 0
  );
