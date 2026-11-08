-- Physical servers register their rack position and, for platform-owned
-- nodes, the owning cloud platform. The cabinet stays a ledger field (text
-- cabinet number plus U range) until structured rack inventory management
-- is introduced; cloud_platform_id carries the cloud-owned attribution that
-- previously was silently dropped from the form payload.
LOCK TABLE public.infra_physical_resource, public.infra_cloud_platform IN SHARE ROW EXCLUSIVE MODE;

ALTER TABLE public.infra_physical_resource
    ADD COLUMN IF NOT EXISTS cloud_platform_id bigint,
    ADD COLUMN IF NOT EXISTS cabinet_no character varying(64),
    ADD COLUMN IF NOT EXISTS rack_start_u integer,
    ADD COLUMN IF NOT EXISTS rack_end_u integer;

CREATE INDEX IF NOT EXISTS idx_infra_physical_resource_platform
    ON public.infra_physical_resource(tenant_id, cloud_platform_id, id) WHERE deleted=0;

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid='public.infra_physical_resource'::regclass
          AND conname='infra_physical_resource_platform_tenant_fk'
    ) THEN
        ALTER TABLE public.infra_physical_resource
            ADD CONSTRAINT infra_physical_resource_platform_tenant_fk
            FOREIGN KEY (tenant_id, cloud_platform_id)
            REFERENCES public.infra_cloud_platform(tenant_id, id);
    END IF;
END $$;
