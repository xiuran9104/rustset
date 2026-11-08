-- Service providers own cloud zones, and machine rooms sit inside one zone
-- (provider -> zone -> room). The room's provider_id column is removed:
-- provider attribution flows exclusively through the zone. Both links stay
-- nullable in the database so rooms can be inventoried before a zone exists
-- and zones can be registered before a provider is chosen; the UI requires
-- the provider on zone creation and the zone on room creation.
LOCK TABLE public.infra_cloud_zone, public.infra_machine_room, public.infra_service_provider IN SHARE ROW EXCLUSIVE MODE;

ALTER TABLE public.infra_cloud_zone
    ADD COLUMN IF NOT EXISTS provider_id bigint;

ALTER TABLE public.infra_machine_room
    ADD COLUMN IF NOT EXISTS zone_id bigint;

CREATE INDEX IF NOT EXISTS idx_infra_cloud_zone_provider
    ON public.infra_cloud_zone(tenant_id, provider_id, id) WHERE deleted=0;
CREATE INDEX IF NOT EXISTS idx_infra_machine_room_zone
    ON public.infra_machine_room(tenant_id, zone_id, id) WHERE deleted=0;

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid='public.infra_cloud_zone'::regclass
          AND conname='infra_cloud_zone_provider_tenant_fk'
    ) THEN
        ALTER TABLE public.infra_cloud_zone
            ADD CONSTRAINT infra_cloud_zone_provider_tenant_fk
            FOREIGN KEY (tenant_id, provider_id)
            REFERENCES public.infra_service_provider(tenant_id, id);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid='public.infra_machine_room'::regclass
          AND conname='infra_machine_room_cloud_zone_tenant_fk'
    ) THEN
        ALTER TABLE public.infra_machine_room
            ADD CONSTRAINT infra_machine_room_cloud_zone_tenant_fk
            FOREIGN KEY (tenant_id, zone_id)
            REFERENCES public.infra_cloud_zone(tenant_id, id);
    END IF;
END $$;

ALTER TABLE public.infra_machine_room
    DROP CONSTRAINT IF EXISTS infra_machine_room_provider_tenant_fk;
ALTER TABLE public.infra_machine_room
    DROP COLUMN IF EXISTS provider_id;
