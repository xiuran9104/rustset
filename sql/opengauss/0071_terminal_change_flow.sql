-- 终端变更申请流程：终端台账（资产中心-终端设备）的退役/信息变更走
-- 原生申请——登记 → 部门主管动态审批（fromContext）→ update_terminal
-- （按 MAC 定位更新台账）→ 结果通知（复用办公网IP申报通知模板）。
-- payload 契约（camelCase）：
--   定位：macAddress（必填，台账唯一键）
--   操作：operation = retire（退役）| update（信息更新）
--   更新值（update 时可选覆盖，空串/缺省保留原值）：
--     newApplicantName / newDepartment / newIpAddress / newAccessPeriod
--   人员：applicantName（申请人）/ department / supervisorUsername

INSERT INTO public.infra_flow_definition
    (name, description, status, definition, version, creator, updater, tenant_id)
SELECT '终端变更申请流程',
       '终端台账变更：登记 → 部门主管动态审批（fromContext）→ update_terminal 更新台账（退役/信息变更）→ 结果通知',
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
           {"key":"update_terminal","name":"更新终端台账","type":"auto",
            "config":{"action":"update_terminal",
                      "params":{
                        "macAddress":"{{payload.macAddress}}",
                        "operation":"{{payload.operation}}",
                        "newApplicantName":"{{payload.newApplicantName}}",
                        "newDepartment":"{{payload.newDepartment}}",
                        "newIpAddress":"{{payload.newIpAddress}}",
                        "newAccessPeriod":"{{payload.newAccessPeriod}}"
                      }}},
           {"key":"notify_done","name":"变更结果通知","type":"notification",
            "config":{"assignee":{"role":"super_admin"},
                      "templateCode":"office_ip_apply_notice",
                      "params":{
                        "applicantName":"{{payload.applicantName}}",
                        "department":"{{payload.department}}",
                        "macAddress":"{{payload.macAddress}}",
                        "requestedIp":"{{payload.newIpAddress}}"
                      }}}
         ]
       }'::jsonb, 1, 'system', 'system', 1
WHERE NOT EXISTS (
    SELECT 1 FROM public.infra_flow_definition
    WHERE name = '终端变更申请流程' AND tenant_id = 1 AND deleted = 0
);
