-- 业务应用生命周期与网络策略归属：
-- 1) 应用增加状态（0=在用，1=已归档），回收联动自动归档；
-- 2) 网络策略绑定业务应用并增加状态：归档=记录保留可查，
--    删除=联动防火墙设备的真实变更，两者分离。
LOCK TABLE public.infra_business_application, public.infra_network_policy IN SHARE ROW EXCLUSIVE MODE;

ALTER TABLE public.infra_business_application
    ADD COLUMN IF NOT EXISTS status smallint DEFAULT 0 NOT NULL;
COMMENT ON COLUMN public.infra_business_application.status IS '0=在用 1=已归档';

ALTER TABLE public.infra_network_policy
    ADD COLUMN IF NOT EXISTS application_id bigint,
    ADD COLUMN IF NOT EXISTS status smallint DEFAULT 0 NOT NULL;
COMMENT ON COLUMN public.infra_network_policy.status IS '0=在用 1=已归档（记录保留）';

-- 复合租户外键前提：网络策略表补 (tenant_id, id) 唯一键
CREATE UNIQUE INDEX IF NOT EXISTS infra_network_policy_tenant_id_key
    ON public.infra_network_policy(tenant_id, id);
CREATE INDEX IF NOT EXISTS idx_infra_network_policy_application
    ON public.infra_network_policy(tenant_id, application_id, id) WHERE deleted=0;

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid='public.infra_network_policy'::regclass
          AND conname='infra_network_policy_application_tenant_fk'
    ) THEN
        ALTER TABLE public.infra_network_policy
            ADD CONSTRAINT infra_network_policy_application_tenant_fk
            FOREIGN KEY (tenant_id, application_id)
            REFERENCES public.infra_business_application(tenant_id, id);
    END IF;
END $$;
