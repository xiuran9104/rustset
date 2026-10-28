-- Cloud resources bind to the cloud platform they run on. Region and zone
-- display names follow the platform's zone; the foreign key gives structured
-- provider/zone/platform traceability that the free-text columns lacked.
LOCK TABLE public.infra_cloud_resource, public.infra_cloud_platform IN SHARE ROW EXCLUSIVE MODE;

ALTER TABLE public.infra_cloud_resource
    ADD COLUMN IF NOT EXISTS cloud_platform_id bigint;

CREATE INDEX IF NOT EXISTS idx_infra_cloud_resource_platform
    ON public.infra_cloud_resource(tenant_id, cloud_platform_id, id) WHERE deleted=0;

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid='public.infra_cloud_resource'::regclass
          AND conname='infra_cloud_resource_platform_tenant_fk'
    ) THEN
        ALTER TABLE public.infra_cloud_resource
            ADD CONSTRAINT infra_cloud_resource_platform_tenant_fk
            FOREIGN KEY (tenant_id, cloud_platform_id)
            REFERENCES public.infra_cloud_platform(tenant_id, id);
    END IF;
END $$;
