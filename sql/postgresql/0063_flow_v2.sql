-- 流程引擎 V2：DAG 出口列 + 工单链吸收的种子交付流程。
-- next jsonb：[{"when":{field,op,value},"to":"key"},...] 顺序评估第一个命中；
-- 空数组 = 线性顺序（V1 兼容）。列默认空数组，存量步骤不受影响。

ALTER TABLE public.infra_flow_step ADD COLUMN IF NOT EXISTS next jsonb DEFAULT '[]'::jsonb NOT NULL;

-- 种子：资源交付流程（标准）——工单创建钩子（FLOW_TICKET_MODE）自动启动。
-- DAG 分支：network 走 防火墙直通链；其他走 供应→人工确认→交付链。
-- 会签示例：安全审批 multi=all（所有收件人通过才完成；any-one 语义删 multi 字段即可）。
INSERT INTO public.infra_flow_definition
    (name, description, status, definition, version, creator, updater, tenant_id)
SELECT '资源交付流程（标准）',
       '工单链吸收 V2：创建工单自动启动（FLOW_TICKET_MODE）；审批→DAG 分支→供应/防火墙→交付',
       0,
       '{
         "steps": [
           {"key":"sec_approve","name":"安全审批（会签）","type":"approval",
            "config":{"assignee":{"role":"super_admin","multi":"all"}},
            "next":[{"to":"route"}]},
           {"key":"route","name":"分支路由","type":"notification",
            "config":{"assignee":{"role":"super_admin"}},
            "next":[
              {"when":{"field":"resourceType","op":"eq","value":"network"},"to":"fw_push"},
              {"to":"do_provision"}
            ]},
           {"key":"fw_push","name":"工单审批+防火墙下发","type":"auto",
            "config":{"action":"ticket_approve","params":{"approved":true}},
            "next":[{"to":"fw_firewall"}]},
           {"key":"fw_firewall","name":"防火墙策略","type":"auto",
            "config":{"action":"firewall_push","params":{}},
            "next":[{"to":"do_deliver"}]},
           {"key":"do_provision","name":"工单审批+资源供应","type":"auto",
            "config":{"action":"ticket_approve","params":{"approved":true}},
            "next":[{"to":"provision"}]},
           {"key":"provision","name":"Tofu 供应","type":"auto",
            "config":{"action":"ticket_provision","params":{}},
            "next":[{"to":"confirm"}]},
           {"key":"confirm","name":"确认交付","type":"manual",
            "config":{"assignee":{"role":"super_admin"}},
            "next":[{"to":"do_deliver"}]},
           {"key":"do_deliver","name":"工单交付","type":"auto",
            "config":{"action":"ticket_deliver","params":{}}}
         ]
       }'::jsonb, 1, 'system', 'system', 1
WHERE NOT EXISTS (
    SELECT 1 FROM public.infra_flow_definition
    WHERE name = '资源交付流程（标准）' AND tenant_id = 1 AND deleted = 0
);
