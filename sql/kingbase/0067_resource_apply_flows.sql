-- 统一申请中心：云资源申请 / 网络策略申请 流程化（与 0066 办公网IP申报同构）。
-- 架构：流程为审批权威（fromContext 动态审批人 + 条件会签），工单链为资源
-- 台账（create_ticket 全量 {{payload.x}} 插值、数字保型；ticket_approve 直通；
-- 到期提醒/回收/附件仍归工单链）。
--
-- 云资源申请 payload 契约（camelCase）：
--   人员：applicantName 姓名 / department 部门 / supervisorUsername 主管账号
--   规格：ecsName 资源名 / resourceType cloud|physical / cloudPlatformName 云平台
--         cloudRegion 云区域 / machineRoomName 物理机房 / projectName 业务项目
--         applicationName 归属应用 / contractName 合同 / hasSecurityProduct 0|1
--         securityProducts 安全产品ID逗号串 / resourceCount 数量 / cpuCores 核
--         memoryGb 内存 / ecsOs 系统 / ecsType 实例规格 / systemDisk 系统盘类型
--         systemDiskSizeGb 系统盘GB / dataDisk 数据盘 / ipAddress 预分配IP
--         expireAt 有效期 / reason 用途
-- 注：网络工单 ticketType 必须是 create（resourceType=network 表达网络类）——
-- 工单状态机对非 create 类型审批后直达 delivered 终态，会跳过交付环节。
-- 云资源流程止于供应确认（manual）：云工单审批后为 pending_provision，
-- 配置登记与交付（→ pending_delivery → delivered）按操作台语义留在工单页。
--
-- 网络策略申请 payload 契约：
--   人员：同上；策略：fwSourceZone/fwSourceAddress/fwSourcePort/fwFirewallName/
--         fwDestZone/fwDestAddress/fwDestPort/fwProtocol/fwDirection/
--         fwValidUntil/reason

-- 通知模板：云资源申请结果 / 网络策略申请结果。网络模板携带高危端口
-- 检测结论（check_risk_port 前置节点写入 context，通知参数插值引用）。
INSERT INTO public.system_notify_template
    (id, name, code, nickname, content, type, params, status, remark, creator, updater)
SELECT nextval('public.system_notify_template_seq'), '云资源申请通知', 'resource_apply_notice', '流程提醒',
       '云资源申请「{title}」：{applicantName}（{department}）申请 {resourceCount} 台 {cpuCores}核/{memoryGb}GB（{ecsName}），当前步骤「{stepName}」已处理，请到 运营流程 → 流程实例 跟进。',
       2, '["flowName","stepName","title","applicantName","department","resourceCount","cpuCores","memoryGb","ecsName"]', 0,
       '云资源申请流程通知', 'system', 'system'
WHERE NOT EXISTS (
    SELECT 1 FROM public.system_notify_template
    WHERE code = 'resource_apply_notice' AND deleted = 0
);
INSERT INTO public.system_notify_template
    (id, name, code, nickname, content, type, params, status, remark, creator, updater)
SELECT nextval('public.system_notify_template_seq'), '网络策略申请通知', 'network_apply_notice', '流程提醒',
       '网络策略申请「{title}」：{applicantName}（{department}）申请 {fwSourceAddress} → {fwDestAddress}:{fwDestPort}/{fwProtocol}，风险等级 {riskLevel}（命中高危端口规则：{riskPortRules}）。当前步骤「{stepName}」已处理，请到 运营流程 → 流程实例 跟进。',
       2, '["flowName","stepName","title","applicantName","department","fwSourceAddress","fwDestAddress","fwDestPort","fwProtocol","riskLevel","riskPortRules"]', 0,
       '网络策略申请流程通知', 'system', 'system'
WHERE NOT EXISTS (
    SELECT 1 FROM public.system_notify_template
    WHERE code = 'network_apply_notice' AND deleted = 0
);

-- 云资源申请流程：登记 → 主管动态审批 →（数量>4 附加安全会签）→ 创建工单
-- （参数插值、数字保型）→ 工单审批直通 → 人工供应确认 → 工单交付 → 结果通知。
INSERT INTO public.infra_flow_definition
    (name, description, status, definition, version, creator, updater, tenant_id)
SELECT '云资源申请流程',
       '申请中心：人员登记 → 部门主管动态审批（fromContext）→（数量>4 安全会签）→ 创建资源工单（参数全量插值）→ 审批直通 → 供应确认 → 结果通知（配置登记与交付在资源工单页操作）',
       0,
       '{
         "steps": [
           {"key":"fill_context","name":"登记申报信息","type":"auto",
            "config":{"action":"set_context",
                      "params":{"key":"supervisor_username","fromPayloadField":"supervisorUsername"}}},
           {"key":"dept_approve","name":"部门主管审批","type":"approval",
            "config":{"assignee":{"role":"super_admin"},
                      "fromContext":"supervisor_username",
                      "timeoutHours":48,"onTimeout":"escalate_notify"}},
           {"key":"sec_approve","name":"安全会签（高规格）","type":"approval",
            "config":{"assignee":{"role":"super_admin"},
                      "timeoutHours":24,"onTimeout":"auto_reject"},
            "condition":{"field":"resourceCount","op":"gt","value":4}},
           {"key":"create_ticket_record","name":"创建资源工单","type":"auto",
            "config":{"action":"create_ticket",
                      "params":{
                        "ecsName":"{{payload.ecsName}}",
                        "resourceType":"{{payload.resourceType}}",
                        "cloudPlatformName":"{{payload.cloudPlatformName}}",
                        "cloudRegion":"{{payload.cloudRegion}}",
                        "machineRoomName":"{{payload.machineRoomName}}",
                        "projectName":"{{payload.projectName}}",
                        "hasSecurityProduct":"{{payload.hasSecurityProduct}}",
                        "securityProducts":"{{payload.securityProducts}}",
                        "applicationName":"{{payload.applicationName}}",
                        "contractName":"{{payload.contractName}}",
                        "applicantName":"{{payload.applicantName}}",
                        "customerName":"{{payload.applicantName}}",
                        "resourceCount":"{{payload.resourceCount}}",
                        "cpuCores":"{{payload.cpuCores}}",
                        "memoryGb":"{{payload.memoryGb}}",
                        "ecsType":"{{payload.ecsType}}",
                        "ecsOs":"{{payload.ecsOs}}",
                        "systemDisk":"{{payload.systemDisk}}",
                        "systemDiskSizeGb":"{{payload.systemDiskSizeGb}}",
                        "dataDisk":"{{payload.dataDisk}}",
                        "ipAddress":"{{payload.ipAddress}}",
                        "expireAt":"{{payload.expireAt}}",
                        "remarks":"{{payload.reason}}"
                      }}},
           {"key":"do_approve","name":"工单审批直通","type":"auto",
            "config":{"action":"ticket_approve","params":{"approved":true,"comment":"流程审批完成，自动通过"}}},
           {"key":"confirm_provision","name":"资源供应确认","type":"manual",
            "config":{"assignee":{"role":"super_admin"},
                      "onTimeout":"escalate_notify"}},
           {"key":"notify_done","name":"申请结果通知","type":"notification",
            "config":{"assignee":{"role":"super_admin"},
                      "templateCode":"resource_apply_notice",
                      "params":{
                        "applicantName":"{{payload.applicantName}}",
                        "department":"{{payload.department}}",
                        "resourceCount":"{{payload.resourceCount}}",
                        "cpuCores":"{{payload.cpuCores}}",
                        "memoryGb":"{{payload.memoryGb}}",
                        "ecsName":"{{payload.ecsName}}"
                      }}}
         ]
       }'::jsonb, 1, 'system', 'system', 1
WHERE NOT EXISTS (
    SELECT 1 FROM public.infra_flow_definition
    WHERE name = '云资源申请流程' AND tenant_id = 1 AND deleted = 0
);

-- 网络策略申请流程：登记 → 高危端口检测前置（check_risk_port 写 context）→
-- 主管动态审批 →（命中高危端口则附加安全审批）→ 创建工单（fw 字段插值，风险
-- 等级取检测结论）→ 审批直通 → 防火墙下发（firewall_push 直接读 payload
-- 的 fw* 字段）→ 交付 → 通知（携带检测结论）。
INSERT INTO public.infra_flow_definition
    (name, description, status, definition, version, creator, updater, tenant_id)
SELECT '网络策略申请流程',
       '申请中心：人员登记 → 高危端口检测（check_risk_port 前置）→ 部门主管动态审批（fromContext）→（命中高危端口则附加安全审批）→ 创建网络工单（fw 字段插值）→ 审批直通 → 防火墙策略下发 → 交付 → 结果通知',
       0,
       '{
         "steps": [
           {"key":"fill_context","name":"登记申报信息","type":"auto",
            "config":{"action":"set_context",
                      "params":{"key":"supervisor_username","fromPayloadField":"supervisorUsername"}}},
           {"key":"port_check","name":"高危端口检测","type":"auto",
            "config":{"action":"check_risk_port",
                      "params":{"port":"{{payload.fwDestPort}}",
                                "sourcePort":"{{payload.fwSourcePort}}",
                                "protocol":"{{payload.fwProtocol}}"}}},
           {"key":"dept_approve","name":"部门主管审批","type":"approval",
            "config":{"assignee":{"role":"super_admin"},
                      "fromContext":"supervisor_username",
                      "timeoutHours":48,"onTimeout":"escalate_notify"}},
           {"key":"sec_approve","name":"安全审批（高危端口）","type":"approval",
            "config":{"assignee":{"role":"super_admin"},
                      "timeoutHours":24,"onTimeout":"auto_reject"},
            "condition":{"field":"risk_port_hit","op":"eq","value":true}},
           {"key":"create_ticket_record","name":"创建网络工单","type":"auto",
            "config":{"action":"create_ticket",
                      "params":{
                        "ecsName":"{{payload.ecsName}}",
                        "resourceType":"network",
                        "ticketType":"create",
                        "riskLevel":"{{context.risk_level}}",
                        "applicantName":"{{payload.applicantName}}",
                        "customerName":"{{payload.applicantName}}",
                        "fwSourceZone":"{{payload.fwSourceZone}}",
                        "fwSourceAddress":"{{payload.fwSourceAddress}}",
                        "fwSourcePort":"{{payload.fwSourcePort}}",
                        "fwFirewallName":"{{payload.fwFirewallName}}",
                        "fwDestZone":"{{payload.fwDestZone}}",
                        "fwDestAddress":"{{payload.fwDestAddress}}",
                        "fwDestPort":"{{payload.fwDestPort}}",
                        "fwProtocol":"{{payload.fwProtocol}}",
                        "fwDirection":"{{payload.fwDirection}}",
                        "fwValidUntil":"{{payload.fwValidUntil}}",
                        "remarks":"{{payload.reason}}"
                      }}},
           {"key":"do_approve","name":"工单审批直通","type":"auto",
            "config":{"action":"ticket_approve","params":{"approved":true,"comment":"流程审批完成，自动通过"}}},
           {"key":"fw_push","name":"防火墙策略下发","type":"auto",
            "config":{"action":"firewall_push","params":{}}},
           {"key":"do_deliver","name":"工单交付","type":"auto",
            "config":{"action":"ticket_deliver","params":{"comment":"策略已下发，流程交付"}}},
           {"key":"notify_done","name":"申请结果通知","type":"notification",
            "config":{"assignee":{"role":"super_admin"},
                      "templateCode":"network_apply_notice",
                      "params":{
                        "applicantName":"{{payload.applicantName}}",
                        "department":"{{payload.department}}",
                        "fwSourceAddress":"{{payload.fwSourceAddress}}",
                        "fwDestAddress":"{{payload.fwDestAddress}}",
                        "fwDestPort":"{{payload.fwDestPort}}",
                        "fwProtocol":"{{payload.fwProtocol}}",
                        "riskLevel":"{{context.risk_level}}",
                        "riskPortRules":"{{context.risk_port_rules}}"
                      }}}
         ]
       }'::jsonb, 1, 'system', 'system', 1
WHERE NOT EXISTS (
    SELECT 1 FROM public.infra_flow_definition
    WHERE name = '网络策略申请流程' AND tenant_id = 1 AND deleted = 0
);
