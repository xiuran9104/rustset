CREATE SEQUENCE IF NOT EXISTS public.infra_automation_config_seq
    START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;

CREATE SEQUENCE IF NOT EXISTS public.infra_automation_audit_seq
    START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;

-- 自动化执行配置：云资源供给与防火墙策略下发各一套独立开关。
-- 设计原则：
--   1. 两套独立凭据体系（不共用密钥）
--   2. 关闭时工单仅做审批留痕，人工执行
--   3. 开启时工单审批通过后自动调用受限 API 账号
--   4. 每次自动执行都写审计记录
CREATE TABLE IF NOT EXISTS public.infra_automation_config (
    id bigint DEFAULT nextval('public.infra_automation_config_seq') NOT NULL,
    -- 'cloud_provision' | 'firewall_provision'
    automation_type character varying(32) NOT NULL,
    -- false = 审批留痕 + 人工执行（默认）
    enabled smallint DEFAULT 0 NOT NULL,
    -- 云侧：引用 infra_cloud_provider_config 的凭据（最小权限服务账号）
    -- 防火墙侧：引用防火墙设备的独立 API 账号
    credential_ref bigint,
    -- 云侧：限制在指定资源池/项目/租户
    -- 防火墙侧：限制在专属策略组
    scope_config text,
    -- 熔断：连续失败 N 次自动断开
    circuit_breaker_threshold integer DEFAULT 3 NOT NULL,
    circuit_breaker_count integer DEFAULT 0 NOT NULL,
    circuit_breaker_tripped smallint DEFAULT 0 NOT NULL,
    last_executed_at timestamp without time zone,
    last_result character varying(512),
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint,
    CONSTRAINT infra_automation_config_pkey PRIMARY KEY (id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS infra_automation_config_tenant_id_key
    ON public.infra_automation_config(tenant_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_infra_automation_config_type
    ON public.infra_automation_config(tenant_id, automation_type) WHERE deleted=0;

-- 自动化执行审计：每次自动调用（成功/失败/跳过）都留痕
CREATE TABLE IF NOT EXISTS public.infra_automation_audit (
    id bigint DEFAULT nextval('public.infra_automation_audit_seq') NOT NULL,
    automation_type character varying(32) NOT NULL,
    ticket_id bigint,
    action character varying(64) NOT NULL,
    -- 'executed' | 'skipped_manual' | 'failed' | 'circuit_open'
    result character varying(32) NOT NULL,
    detail text,
    duration_ms integer,
    executor character varying(64) DEFAULT 'system'::character varying NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint,
    CONSTRAINT infra_automation_audit_pkey PRIMARY KEY (id, tenant_id)
);

CREATE INDEX IF NOT EXISTS infra_automation_audit_tenant_id_key
    ON public.infra_automation_audit(tenant_id, id);
CREATE INDEX IF NOT EXISTS idx_infra_automation_audit_ticket
    ON public.infra_automation_audit(tenant_id, ticket_id, id) WHERE deleted=0;

-- 默认插入两条配置（关闭状态）
INSERT INTO public.infra_automation_config
    (id, automation_type, enabled, creator, updater, tenant_id)
SELECT nextval('public.infra_automation_config_seq'), t.type, 0, 'system', 'system', 1
FROM (VALUES
    ('cloud_provision'),
    ('firewall_provision')
) AS t(type)
WHERE NOT EXISTS (
    SELECT 1 FROM public.infra_automation_config
    WHERE tenant_id = 1 AND automation_type = t.type AND deleted = 0
);
