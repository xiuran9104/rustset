-- 业务应用"归属账户"做实：主数据补负责人；云资源/物理资源/工单与
-- 业务应用建立结构化归属（application_id），名称保留为显示冗余。
-- 存量数据按同租户名称精确匹配自动认领，匹配不上留空待人工绑定。

ALTER TABLE public.infra_business_application
    ADD COLUMN IF NOT EXISTS owner_user_id bigint REFERENCES public.system_users(id);

ALTER TABLE public.infra_cloud_resource
    ADD COLUMN IF NOT EXISTS application_id bigint;
ALTER TABLE public.infra_physical_resource
    ADD COLUMN IF NOT EXISTS application_id bigint;
ALTER TABLE public.infra_resource_ticket
    ADD COLUMN IF NOT EXISTS application_id bigint;

CREATE INDEX IF NOT EXISTS idx_infra_cloud_resource_application
    ON public.infra_cloud_resource(tenant_id, application_id, id) WHERE deleted=0;
CREATE INDEX IF NOT EXISTS idx_infra_physical_resource_application
    ON public.infra_physical_resource(tenant_id, application_id, id) WHERE deleted=0;
CREATE INDEX IF NOT EXISTS idx_infra_resource_ticket_application
    ON public.infra_resource_ticket(tenant_id, application_id, id) WHERE deleted=0;

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid='public.infra_cloud_resource'::regclass
          AND conname='infra_cloud_resource_application_tenant_fk'
    ) THEN
        ALTER TABLE public.infra_cloud_resource
            ADD CONSTRAINT infra_cloud_resource_application_tenant_fk
            FOREIGN KEY (tenant_id, application_id)
            REFERENCES public.infra_business_application(tenant_id, id);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid='public.infra_physical_resource'::regclass
          AND conname='infra_physical_resource_application_tenant_fk'
    ) THEN
        ALTER TABLE public.infra_physical_resource
            ADD CONSTRAINT infra_physical_resource_application_tenant_fk
            FOREIGN KEY (tenant_id, application_id)
            REFERENCES public.infra_business_application(tenant_id, id);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid='public.infra_resource_ticket'::regclass
          AND conname='infra_ticket_application_tenant_fk'
    ) THEN
        ALTER TABLE public.infra_resource_ticket
            ADD CONSTRAINT infra_ticket_application_tenant_fk
            FOREIGN KEY (tenant_id, application_id)
            REFERENCES public.infra_business_application(tenant_id, id);
    END IF;
END $$;

-- 存量认领：同租户按 application_name = 业务应用名 精确匹配，只补空值
UPDATE public.infra_cloud_resource r
SET application_id = a.id, update_time = now()
FROM public.infra_business_application a
WHERE a.tenant_id = r.tenant_id AND a.deleted = 0
  AND r.deleted = 0 AND r.application_id IS NULL
  AND coalesce(r.application_name, '') <> '' AND r.application_name = a.name;

UPDATE public.infra_physical_resource r
SET application_id = a.id, update_time = now()
FROM public.infra_business_application a
WHERE a.tenant_id = r.tenant_id AND a.deleted = 0
  AND r.deleted = 0 AND r.application_id IS NULL
  AND coalesce(r.application_name, '') <> '' AND r.application_name = a.name;

UPDATE public.infra_resource_ticket t
SET application_id = a.id, update_time = now()
FROM public.infra_business_application a
WHERE a.tenant_id = t.tenant_id AND a.deleted = 0
  AND t.deleted = 0 AND t.application_id IS NULL
  AND coalesce(t.application_name, '') <> '' AND t.application_name = a.name;
