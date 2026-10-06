-- Tenant ownership for business applications and their network endpoints.
LOCK TABLE public.infra_business_application, public.infra_application_endpoint
    IN SHARE ROW EXCLUSIVE MODE;

ALTER TABLE public.infra_business_application
    ADD COLUMN IF NOT EXISTS tenant_id bigint REFERENCES public.system_tenant(id);
ALTER TABLE public.infra_application_endpoint
    ADD COLUMN IF NOT EXISTS tenant_id bigint REFERENCES public.system_tenant(id);

CREATE UNIQUE INDEX IF NOT EXISTS infra_business_application_tenant_id_key
    ON public.infra_business_application(tenant_id, id);
CREATE INDEX IF NOT EXISTS idx_infra_business_application_tenant
    ON public.infra_business_application(tenant_id, id) WHERE deleted=0;
CREATE INDEX IF NOT EXISTS idx_infra_application_endpoint_tenant
    ON public.infra_application_endpoint(tenant_id, business_application_id) WHERE deleted=0;

DROP TRIGGER IF EXISTS require_tenant ON public.infra_business_application;
CREATE TRIGGER require_tenant BEFORE INSERT OR UPDATE OF tenant_id
    ON public.infra_business_application FOR EACH ROW
    EXECUTE FUNCTION public.require_resource_tenant();
DROP TRIGGER IF EXISTS require_tenant ON public.infra_application_endpoint;
CREATE TRIGGER require_tenant BEFORE INSERT OR UPDATE OF tenant_id
    ON public.infra_application_endpoint FOR EACH ROW
    EXECUTE FUNCTION public.require_resource_tenant();

DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='public.infra_application_endpoint'::regclass AND conname='infra_endpoint_application_tenant_fk') THEN
        ALTER TABLE public.infra_application_endpoint
            ADD CONSTRAINT infra_endpoint_application_tenant_fk
            FOREIGN KEY (tenant_id, business_application_id)
            REFERENCES public.infra_business_application(tenant_id, id);
    END IF;
END $$;
