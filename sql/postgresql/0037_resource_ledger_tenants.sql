-- Tenant ownership for cloud discovery and typed resource ledgers.
LOCK TABLE public.infra_cloud_asset, public.infra_cloud_resource,
    public.infra_physical_resource IN SHARE ROW EXCLUSIVE MODE;

DO $$ DECLARE tbl text; BEGIN
    FOREACH tbl IN ARRAY ARRAY[
        'infra_cloud_asset', 'infra_cloud_resource', 'infra_physical_resource'
    ] LOOP
        EXECUTE format(
            'ALTER TABLE public.%I ADD COLUMN IF NOT EXISTS tenant_id bigint REFERENCES public.system_tenant(id)',
            tbl
        );
        EXECUTE format(
            'CREATE INDEX IF NOT EXISTS %I ON public.%I(tenant_id, id) WHERE deleted=0',
            'idx_' || tbl || '_tenant', tbl
        );
        EXECUTE format('DROP TRIGGER IF EXISTS require_tenant ON public.%I', tbl);
        EXECUTE format(
            'CREATE TRIGGER require_tenant BEFORE INSERT OR UPDATE OF tenant_id ON public.%I FOR EACH ROW EXECUTE FUNCTION public.require_resource_tenant()',
            tbl
        );
    END LOOP;
END $$;

-- Discovery retries and concurrent submissions converge on one tenant-owned
-- cloud asset. Quarantined historical rows are excluded from this invariant.
CREATE UNIQUE INDEX IF NOT EXISTS idx_cloud_asset_tenant_instance_config
    ON public.infra_cloud_asset(tenant_id, instance_id, cloud_provider_config_id)
    NULLS NOT DISTINCT
    WHERE tenant_id IS NOT NULL AND deleted=0;

DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='public.infra_cloud_asset'::regclass AND conname='infra_cloud_asset_config_tenant_fk') THEN
        ALTER TABLE public.infra_cloud_asset ADD CONSTRAINT infra_cloud_asset_config_tenant_fk
            FOREIGN KEY (tenant_id, cloud_provider_config_id)
            REFERENCES public.infra_cloud_provider_config(tenant_id, id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='public.infra_cloud_resource'::regclass AND conname='infra_cloud_resource_config_tenant_fk') THEN
        ALTER TABLE public.infra_cloud_resource ADD CONSTRAINT infra_cloud_resource_config_tenant_fk
            FOREIGN KEY (tenant_id, cloud_provider_config_id)
            REFERENCES public.infra_cloud_provider_config(tenant_id, id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='public.infra_physical_resource'::regclass AND conname='infra_physical_resource_room_tenant_fk') THEN
        ALTER TABLE public.infra_physical_resource ADD CONSTRAINT infra_physical_resource_room_tenant_fk
            FOREIGN KEY (tenant_id, machine_room_id)
            REFERENCES public.infra_machine_room(tenant_id, id);
    END IF;
END $$;
