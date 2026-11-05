CREATE SEQUENCE IF NOT EXISTS public.infra_automation_credential_seq
    START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;

-- 防火墙自动化凭据：独立于云平台凭据，不共用密钥。
-- 存储 API 用户名 + SM4 加密密码，只有自动化框架读取。
CREATE TABLE IF NOT EXISTS public.infra_automation_credential (
    id bigint DEFAULT nextval('public.infra_automation_credential_seq') NOT NULL,
    -- 'firewall_provision' 等
    automation_type character varying(32) NOT NULL,
    api_username character varying(128) NOT NULL,
    -- SM4 加密存储，格式 enc:sm4:v2:{hex}
    api_password character varying(512) NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    tenant_id bigint,
    CONSTRAINT infra_automation_credential_pkey PRIMARY KEY (id, tenant_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS infra_automation_credential_tenant_id_key
    ON public.infra_automation_credential(tenant_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_infra_automation_credential_type
    ON public.infra_automation_credential(tenant_id, automation_type) WHERE deleted=0;

-- API 端点：新增 automation-config/test-connection 和 credential 端点
