-- Tenant ownership for provider/room inventory. Existing rows deliberately
-- remain NULL until an operator confirms their ownership.
LOCK TABLE public.infra_service_provider, public.infra_machine_room IN SHARE ROW EXCLUSIVE MODE;

ALTER TABLE public.infra_service_provider
    ADD COLUMN IF NOT EXISTS tenant_id bigint REFERENCES public.system_tenant(id);
ALTER TABLE public.infra_machine_room
    ADD COLUMN IF NOT EXISTS tenant_id bigint REFERENCES public.system_tenant(id);

CREATE UNIQUE INDEX IF NOT EXISTS infra_service_provider_tenant_id_key
    ON public.infra_service_provider(tenant_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS infra_machine_room_tenant_id_key
    ON public.infra_machine_room(tenant_id, id);
CREATE INDEX IF NOT EXISTS idx_infra_service_provider_tenant
    ON public.infra_service_provider(tenant_id, id) WHERE deleted = 0;
CREATE INDEX IF NOT EXISTS idx_infra_machine_room_tenant
    ON public.infra_machine_room(tenant_id, id) WHERE deleted = 0;

DROP TRIGGER IF EXISTS require_tenant ON public.infra_service_provider;
CREATE TRIGGER require_tenant
    BEFORE INSERT OR UPDATE OF tenant_id ON public.infra_service_provider
    FOR EACH ROW EXECUTE FUNCTION public.require_resource_tenant();
DROP TRIGGER IF EXISTS require_tenant ON public.infra_machine_room;
CREATE TRIGGER require_tenant
    BEFORE INSERT OR UPDATE OF tenant_id ON public.infra_machine_room
    FOR EACH ROW EXECUTE FUNCTION public.require_resource_tenant();

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'public.infra_machine_room'::regclass
          AND conname = 'infra_machine_room_provider_tenant_fk'
    ) THEN
        ALTER TABLE public.infra_machine_room
            ADD CONSTRAINT infra_machine_room_provider_tenant_fk
            FOREIGN KEY (tenant_id, provider_id)
            REFERENCES public.infra_service_provider(tenant_id, id);
    END IF;
END $$;
