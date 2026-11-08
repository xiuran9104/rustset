-- Ops agent (docs/cmdb-roadmap.md): register the Rust-executed operations
-- tools in ai.tools and preset a public 运维助理 chat role binding them.

INSERT INTO ai.tools (id, name, description, status, input_schema, executor, create_time, update_time)
SELECT 1784300000001, 'cmdb_model_list',
       '列出 CMDB 中所有已启用的模型及其属性数和实例数',
       1,
       '{"type":"object","properties":{},"required":[]}'::jsonb,
       '{"kind":"rust"}'::jsonb,
       (EXTRACT(EPOCH FROM now())*1000)::bigint,
       (EXTRACT(EPOCH FROM now())*1000)::bigint
WHERE NOT EXISTS (SELECT 1 FROM ai.tools WHERE name='cmdb_model_list');

INSERT INTO ai.tools (id, name, description, status, input_schema, executor, create_time, update_time)
SELECT 1784300000002, 'cmdb_instance_query',
       '按模型编码查询 CMDB 配置项实例，可带关键词过滤实例属性内容',
       1,
       '{"type":"object","properties":{"model_code":{"type":"string","description":"CMDB 模型编码，如 server / cloud_ecs"},"keyword":{"type":"string","description":"属性内容关键词，可选"},"limit":{"type":"integer","description":"返回条数，默认 10，最大 20"}},"required":["model_code"]}'::jsonb,
       '{"kind":"rust"}'::jsonb,
       (EXTRACT(EPOCH FROM now())*1000)::bigint,
       (EXTRACT(EPOCH FROM now())*1000)::bigint
WHERE NOT EXISTS (SELECT 1 FROM ai.tools WHERE name='cmdb_instance_query');

INSERT INTO ai.tools (id, name, description, status, input_schema, executor, create_time, update_time)
SELECT 1784300000003, 'asset_query',
       '查询资产采集台账（infra_asset），支持名称/IP/单位/应用关键词',
       1,
       '{"type":"object","properties":{"keyword":{"type":"string","description":"名称、IP、单位或应用关键词，可选"},"limit":{"type":"integer","description":"返回条数，默认 10，最大 20"}},"required":[]}'::jsonb,
       '{"kind":"rust"}'::jsonb,
       (EXTRACT(EPOCH FROM now())*1000)::bigint,
       (EXTRACT(EPOCH FROM now())*1000)::bigint
WHERE NOT EXISTS (SELECT 1 FROM ai.tools WHERE name='asset_query');

INSERT INTO ai.tools (id, name, description, status, input_schema, executor, create_time, update_time)
SELECT 1784300000004, 'ticket_query',
       '查询资源开通工单，可按状态（pending_approval/pending_provision/pending_delivery/delivered/rejected）和关键词过滤',
       1,
       '{"type":"object","properties":{"status":{"type":"string","description":"工单状态，可选"},"keyword":{"type":"string","description":"主机名或应用名关键词，可选"},"limit":{"type":"integer","description":"返回条数，默认 10，最大 20"}},"required":[]}'::jsonb,
       '{"kind":"rust"}'::jsonb,
       (EXTRACT(EPOCH FROM now())*1000)::bigint,
       (EXTRACT(EPOCH FROM now())*1000)::bigint
WHERE NOT EXISTS (SELECT 1 FROM ai.tools WHERE name='ticket_query');

INSERT INTO ai.tools (id, name, description, status, input_schema, executor, create_time, update_time)
SELECT 1784300000005, 'ticket_create',
       '创建云主机开通工单（ecs）。命中自动审批规则时会直接进入待配置状态；开通执行需在工单页进行或由自动规则触发',
       1,
       '{"type":"object","properties":{"ecs_name":{"type":"string","description":"主机名"},"cpu_cores":{"type":"integer","description":"CPU 核数，默认 4"},"memory_gb":{"type":"integer","description":"内存 GB，默认 8"},"resource_count":{"type":"integer","description":"数量，默认 1"},"ecs_type":{"type":"string","description":"规格，可选"},"ecs_os":{"type":"string","description":"操作系统镜像，可选"},"cloud_category":{"type":"string","description":"云类别（demo/aliyun/tencent），可选"},"cloud_region":{"type":"string","description":"地域，可选"},"application_name":{"type":"string","description":"所属应用，可选"},"applicant":{"type":"string","description":"申请人，可选"}},"required":["ecs_name"]}'::jsonb,
       '{"kind":"rust"}'::jsonb,
       (EXTRACT(EPOCH FROM now())*1000)::bigint,
       (EXTRACT(EPOCH FROM now())*1000)::bigint
WHERE NOT EXISTS (SELECT 1 FROM ai.tools WHERE name='ticket_create');

-- 公共“运维助理”聊天角色：绑定以上工具，任何用户可直接使用。
INSERT INTO ai.chat_roles (id, user_id, model_id, name, avatar, category, sort, description,
                           system_message, welcome_message, public_status, status,
                           knowledge_ids, tool_ids, create_time, update_time)
SELECT
    1784300000100,
    (SELECT COALESCE((SELECT id FROM system_users WHERE username='admin' AND deleted=0 ORDER BY id LIMIT 1), 0)),
    -- model_configs uses status=0 for enabled.
    (SELECT id FROM ai.model_configs WHERE type='chat' ORDER BY (status=0) DESC, id LIMIT 1),
    '运维助理', '', 'Agent', 100,
    'CMDB / 资产台账 / 资源工单的智能运维 Agent',
    '你是 RustSet 的运维助理 Agent。你可以：用 cmdb_model_list 和 cmdb_instance_query 查询 CMDB 模型与配置项；用 asset_query 查询资产采集台账；用 ticket_query 查询资源开通工单；用 ticket_create 创建云主机开通工单（会按自动审批规则流转）。回答保持简洁、基于工具返回的真实数据，不确定时先查询再回答。创建工单前先向用户确认规格（名称/CPU/内存/数量）。',
    '你好，我是运维助理。可以问我“CMDB 里有哪些模型”“查一下 web-prod 相关资产”或“帮我开一台 4 核 8G 的机器”。',
    true, 1,
    '{}',
    ARRAY(SELECT id FROM ai.tools WHERE name IN
        ('cmdb_model_list','cmdb_instance_query','asset_query','ticket_query','ticket_create')),
    (EXTRACT(EPOCH FROM now())*1000)::bigint,
    (EXTRACT(EPOCH FROM now())*1000)::bigint
WHERE NOT EXISTS (SELECT 1 FROM ai.chat_roles WHERE name='运维助理' AND public_status=true)
  -- chat_roles.model_id is a foreign key; the preset role is seeded when
  -- any chat model row exists (enabled = status 0 preferred).
  AND EXISTS (SELECT 1 FROM ai.model_configs WHERE type='chat');
