-- 资源工单链原生化：流程为审批权威，工单页退化为台账 + 供应/交付/到期操作台。
-- 三件事：
-- 1) 种子「资源变更申请流程」（申请中心路径，覆盖 resize/migration/stop 等
--    变更类工单，create 类已由 0067 云资源申请流程承载）；
-- 2) 种子「资源工单审批流程（台账触发）」：API/MCP 建单路径的原生审批流——
--    工单已存在，无 create_ticket/交付步骤，供应交付仍在工单页操作；
-- 3) 影子机制退役：停用 0063 的「资源交付流程（标准）」（FLOW_TICKET_MODE
--    门控移除，见 ticket.rs）。
--
-- 资源变更申请 payload 契约（camelCase）：
--   人员：applicantName / department / supervisorUsername（主管动态审批）
--   工单：ticketType（resize|disk_expand|network_change|renew|migration|stop|
--         start|reclaim）/ ecsName 工单名 / resourceType cloud|physical
--         riskLevel normal|high|critical（migration/stop/reclaim 应为 high+）
--   变更：targetResourceId / targetResourceType / targetConfig 变更内容
--         maintenanceWindow 维护窗口 / allowInterruption / backupConfirmed /
--         rollbackPlan 回滚方案 / retentionUntil 回收保留期 / expireAt 续期至
--         reason 用途说明

-- 变更结果通知模板。
INSERT INTO public.system_notify_template
    (id, name, code, nickname, content, type, params, status, remark, creator, updater)
SELECT nextval('public.system_notify_template_seq'), '资源变更申请通知', 'ticket_change_notice', '流程提醒',
       '资源变更申请「{title}」（{ticketTypeLabel}）：{applicantName}（{department}）提交 {ecsName}，当前步骤「{stepName}」已处理，请到 运营流程 → 流程实例 跟进。',
       2, '["flowName","stepName","title","applicantName","department","ticketTypeLabel","ecsName"]', 0,
       '资源变更申请流程通知', 'system', 'system'
WHERE NOT EXISTS (
    SELECT 1 FROM public.system_notify_template
    WHERE code = 'ticket_change_notice' AND deleted = 0
);

-- 资源变更申请流程（申请中心 / 工单页创建弹窗的变更类路径）。
INSERT INTO public.infra_flow_definition
    (name, description, status, definition, version, creator, updater, tenant_id)
SELECT '资源变更申请流程',
       '变更类工单原生化：登记 → 主管动态审批（fromContext）→（高风险附加安全审批）→ 创建工单（变更字段全量插值）→ 审批直通 → 变更结果通知；供应/交付在资源工单页操作',
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
           {"key":"sec_approve","name":"安全审批（高风险）","type":"approval",
            "config":{"assignee":{"role":"super_admin"},
                      "timeoutHours":24,"onTimeout":"auto_reject"},
            "condition":{"field":"riskLevel","op":"in","value":["high","critical"]}},
           {"key":"create_ticket_record","name":"创建变更工单","type":"auto",
            "config":{"action":"create_ticket",
                      "params":{
                        "ecsName":"{{payload.ecsName}}",
                        "ticketType":"{{payload.ticketType}}",
                        "resourceType":"{{payload.resourceType}}",
                        "riskLevel":"{{payload.riskLevel}}",
                        "applicantName":"{{payload.applicantName}}",
                        "customerName":"{{payload.applicantName}}",
                        "targetResourceId":"{{payload.targetResourceId}}",
                        "targetConfig":"{{payload.targetConfig}}",
                        "maintenanceWindow":"{{payload.maintenanceWindow}}",
                        "allowInterruption":"{{payload.allowInterruption}}",
                        "backupConfirmed":"{{payload.backupConfirmed}}",
                        "rollbackPlan":"{{payload.rollbackPlan}}",
                        "retentionUntil":"{{payload.retentionUntil}}",
                        "expireAt":"{{payload.expireAt}}",
                        "remarks":"{{payload.reason}}"
                      }}},
           {"key":"do_approve","name":"工单审批直通","type":"auto",
            "config":{"action":"ticket_approve","params":{"approved":true,"comment":"流程审批完成，自动通过"}}},
           {"key":"notify_done","name":"变更结果通知","type":"notification",
            "config":{"assignee":{"role":"super_admin"},
                      "templateCode":"ticket_change_notice",
                      "params":{
                        "applicantName":"{{payload.applicantName}}",
                        "department":"{{payload.department}}",
                        "ticketTypeLabel":"{{payload.ticketType}}",
                        "ecsName":"{{payload.ecsName}}"
                      }}}
         ]
       }'::jsonb, 1, 'system', 'system', 1
WHERE NOT EXISTS (
    SELECT 1 FROM public.infra_flow_definition
    WHERE name = '资源变更申请流程' AND tenant_id = 1 AND deleted = 0
);

-- 资源工单审批流程（台账触发）：API/MCP 建单后原生启动，payload=工单 JSON。
-- 无 create_ticket（工单已存在）与交付步骤——供应/交付/到期由工单页操作。
INSERT INTO public.infra_flow_definition
    (name, description, status, definition, version, creator, updater, tenant_id)
SELECT '资源工单审批流程（台账触发）',
       '台账路径原生审批：主管审批 →（高风险附加安全审批）→ 工单审批直通 → 审批结果通知。由工单创建 API 自动启动（幂等 biz 绑定）',
       0,
       '{
         "steps": [
           {"key":"dept_approve","name":"资源管理员审批","type":"approval",
            "config":{"assignee":{"role":"super_admin"},
                      "timeoutHours":48,"onTimeout":"escalate_notify"}},
           {"key":"sec_approve","name":"安全审批（高风险）","type":"approval",
            "config":{"assignee":{"role":"super_admin"},
                      "timeoutHours":24,"onTimeout":"auto_reject"},
            "condition":{"field":"riskLevel","op":"in","value":["high","critical"]}},
           {"key":"do_approve","name":"工单审批直通","type":"auto",
            "config":{"action":"ticket_approve","params":{"approved":true,"comment":"流程审批完成，自动通过"}}},
           {"key":"notify_done","name":"审批结果通知","type":"notification",
            "config":{"assignee":{"role":"super_admin"},
                      "templateCode":"ticket_change_notice",
                      "params":{
                        "applicantName":"{{payload.applicantName}}",
                        "department":"{{payload.department}}",
                        "ticketTypeLabel":"{{payload.ticketType}}",
                        "ecsName":"{{payload.ecsName}}"
                      }}}
         ]
       }'::jsonb, 1, 'system', 'system', 1
WHERE NOT EXISTS (
    SELECT 1 FROM public.infra_flow_definition
    WHERE name = '资源工单审批流程（台账触发）' AND tenant_id = 1 AND deleted = 0
);

-- 影子机制退役：停用 0063 的「资源交付流程（标准）」（保留行供历史实例追溯）。
UPDATE public.infra_flow_definition
   SET status = 1, update_time = now(), updater = 'system'
 WHERE name = '资源交付流程（标准）' AND tenant_id = 1 AND deleted = 0 AND status = 0;
