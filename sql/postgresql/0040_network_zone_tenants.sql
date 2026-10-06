-- Network zones are tenant-owned inventory. Existing rows remain quarantined
-- with NULL ownership until an operator confirms their tenant.
LOCK TABLE public.infra_network_zone IN SHARE ROW EXCLUSIVE MODE;

ALTER TABLE public.infra_network_zone
    ADD COLUMN IF NOT EXISTS tenant_id bigint REFERENCES public.system_tenant(id);

CREATE UNIQUE INDEX IF NOT EXISTS infra_network_zone_tenant_id_key
    ON public.infra_network_zone(tenant_id, id);
CREATE INDEX IF NOT EXISTS idx_infra_network_zone_tenant
    ON public.infra_network_zone(tenant_id, priority, id) WHERE deleted=0;

DROP TRIGGER IF EXISTS require_tenant ON public.infra_network_zone;
CREATE TRIGGER require_tenant BEFORE INSERT OR UPDATE OF tenant_id
    ON public.infra_network_zone FOR EACH ROW
    EXECUTE FUNCTION public.require_resource_tenant();

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid='public.infra_network_zone'::regclass
          AND conname='infra_network_zone_cloud_platform_tenant_fk'
    ) THEN
        ALTER TABLE public.infra_network_zone
            ADD CONSTRAINT infra_network_zone_cloud_platform_tenant_fk
            FOREIGN KEY (tenant_id, cloud_platform_id)
            REFERENCES public.infra_cloud_platform(tenant_id, id);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid='public.infra_network_zone'::regclass
          AND conname='infra_network_zone_machine_room_tenant_fk'
    ) THEN
        ALTER TABLE public.infra_network_zone
            ADD CONSTRAINT infra_network_zone_machine_room_tenant_fk
            FOREIGN KEY (tenant_id, machine_room_id)
            REFERENCES public.infra_machine_room(tenant_id, id);
    END IF;
END $$;
