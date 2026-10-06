-- Tenant ownership for cloud inventory, credentials and security products.
-- Existing rows remain quarantined with NULL ownership until reviewed.
LOCK TABLE public.infra_cloud_zone, public.infra_cloud_platform,
    public.infra_cloud_provider_config, public.infra_security_product
    IN SHARE ROW EXCLUSIVE MODE;

DO $$ DECLARE tbl text; BEGIN
    FOREACH tbl IN ARRAY ARRAY[
        'infra_cloud_zone', 'infra_cloud_platform',
        'infra_cloud_provider_config', 'infra_security_product'
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

CREATE UNIQUE INDEX IF NOT EXISTS infra_cloud_zone_tenant_id_key
    ON public.infra_cloud_zone(tenant_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS infra_cloud_platform_tenant_id_key
    ON public.infra_cloud_platform(tenant_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS infra_cloud_provider_config_tenant_id_key
    ON public.infra_cloud_provider_config(tenant_id, id);

DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='public.infra_cloud_platform'::regclass AND conname='infra_cloud_platform_zone_tenant_fk') THEN
        ALTER TABLE public.infra_cloud_platform ADD CONSTRAINT infra_cloud_platform_zone_tenant_fk
            FOREIGN KEY (tenant_id, zone_id) REFERENCES public.infra_cloud_zone(tenant_id, id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='public.infra_cloud_provider_config'::regclass AND conname='infra_cloud_config_zone_tenant_fk') THEN
        ALTER TABLE public.infra_cloud_provider_config ADD CONSTRAINT infra_cloud_config_zone_tenant_fk
            FOREIGN KEY (tenant_id, zone_id) REFERENCES public.infra_cloud_zone(tenant_id, id);
        ALTER TABLE public.infra_cloud_provider_config ADD CONSTRAINT infra_cloud_config_platform_tenant_fk
            FOREIGN KEY (tenant_id, platform_id) REFERENCES public.infra_cloud_platform(tenant_id, id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='public.infra_security_product'::regclass AND conname='infra_security_cloud_platform_tenant_fk') THEN
        ALTER TABLE public.infra_security_product ADD CONSTRAINT infra_security_cloud_platform_tenant_fk
            FOREIGN KEY (tenant_id, cloud_platform_id) REFERENCES public.infra_cloud_platform(tenant_id, id);
        ALTER TABLE public.infra_security_product ADD CONSTRAINT infra_security_machine_room_tenant_fk
            FOREIGN KEY (tenant_id, machine_room_id) REFERENCES public.infra_machine_room(tenant_id, id);
        ALTER TABLE public.infra_security_product ADD CONSTRAINT infra_security_provider_tenant_fk
            FOREIGN KEY (tenant_id, provider_id) REFERENCES public.infra_service_provider(tenant_id, id);
    END IF;
END $$;
