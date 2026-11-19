-- 办公网IP申报流程：终端（MAC 地址）+ 人员信息登记 → 部门主管动态审批
-- → 网络管理员配置（分配 IP/录入端口，完成时备注分配结果）→ 开通交付确认
-- → 申报结果通知。生命周期与资源工单链同构：审批 → 配置 → 交付 → 完成。
-- 启用引擎的两个插值特性：
-- 1) fromContext 动态审批人：payload.supervisorUsername 经 set_context 写入
--    context，审批步骤激活时按点路径取用户名并入办理人（未配置回退超管角色）；
-- 2) 通知模板参数：步骤 config.params 先做 {{payload.x}} 插值，再按 {key}
--    替换模板占位符。

-- 申报通知模板：content 携带申报自定义占位符（params 数组同步登记）。
INSERT INTO public.system_notify_template
    (id, name, code, nickname, content, type, params, status, remark, creator, updater)
SELECT nextval('public.system_notify_template_seq'), '办公网IP申报通知', 'office_ip_apply_notice', '流程提醒',
       '办公网IP申报「{title}」：申报人 {applicantName}（{department}），设备 MAC：{macAddress}，申请IP：{requestedIp}。当前步骤「{stepName}」已处理，请到 运营流程 → 流程实例 跟进。',
       2, '["flowName","stepName","title","applicantName","department","macAddress","requestedIp"]', 0,
       '办公网IP申报流程通知', 'system', 'system'
WHERE NOT EXISTS (
    SELECT 1 FROM public.system_notify_template
    WHERE code = 'office_ip_apply_notice' AND deleted = 0
);

-- 流程定义（启用）。payload 契约（申报表单字段，camelCase）：
--   人员：applicantName 姓名 / department 部门 / employeeId 工号 / phone 联系电话
--   设备：macAddress MAC 地址 / deviceType 设备类型 / hostname 主机名
--   网络：requestedIp 申请IP（空=自动分配）/ networkZone 接入区域 /
--         accessPeriod 使用期限 / reason 用途说明
--   审批：supervisorUsername 部门主管系统账号（fromContext 动态审批人）
INSERT INTO public.infra_flow_definition
    (name, description, status, definition, version, creator, updater, tenant_id)
SELECT '办公网IP申报流程',
       '办公网终端 IP 申报：人员/设备（MAC）登记 → 部门主管动态审批（fromContext）→ 网络管理员配置（分配IP）→ 开通交付确认 → 申报结果通知（模板参数插值）',
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
           {"key":"assign_ip","name":"配置-分配IP","type":"manual",
            "config":{"assignee":{"role":"super_admin"},
                      "timeoutHours":72,"onTimeout":"escalate_notify"}},
           {"key":"confirm_open","name":"交付-开通确认","type":"manual",
            "config":{"assignee":{"role":"super_admin"}}},
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
       }'::jsonb, 1, 'system', 'system', 1
WHERE NOT EXISTS (
    SELECT 1 FROM public.infra_flow_definition
    WHERE name = '办公网IP申报流程' AND tenant_id = 1 AND deleted = 0
);
